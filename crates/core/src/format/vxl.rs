// SPDX-License-Identifier: GPL-3.0-only

//! VXL 体素模型(.vxl)。
//!
//! 布局(研究文档 2026-11-03-vxl-hva-layouts.md §1;XCC `vxl_file.{h,cpp}` +
//! 真实样本双向验证):802 字节文件头 + `S`×28 字节节头 + 连续节体 +
//! `S`×92 字节节尾(恰在文件尾结束)。
//! 节体 = 各节 span_start 表(4*cx*cy 个 i32)+ span_end 表 + span 数据,
//! 三个偏移均为**节体区**相对偏移(所有节共享同一原点)。
//! 列序 j = y*cx + x;空列 = start/end 同时为 -1;
//! 非空列数据:`while z < cz { skip:u8; count:u8; z+=skip; count×{color:u8,
//! normal:u8}; z+=count; pad:u8(消耗不校验) }`,体素段+跳过恰覆盖 cz 层。
//! 法线表由节尾 `normal_type` 选择(2 = TS 36 项,其余 = RA2 244 项;
//! 方向向量表属渲染层,本模块只透传索引)。

use super::bytes::ByteReader;
use super::FormatError;

const VXL_MAGIC: &[u8; 16] = b"Voxel Animation\0";

/// 单个体素:z 层号 + 头部调色板颜色索引 + 法线索引。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VxlVoxel {
    pub z: u8,
    pub color: u8,
    pub normal: u8,
}

/// 一列(j = y*cx + x)。空列为空 `Vec`。
pub struct VxlColumn {
    pub voxels: Vec<VxlVoxel>,
}

/// 一个节(section/limb)的解码结果。
pub struct VxlSection {
    pub name: String,
    pub dims: [u8; 3], // cx, cy, cz
    /// 法线表选择:2 = TS 36 项表,其余 = RA2 244 项表(渲染层裁决)。
    pub normal_type: i8,
    /// 体素棱长缩放(真库常为 1/12;XCC 自家写出的文件可为 0)。
    pub scale: f32,
    /// 静止姿态 3x4 变换(真库常非零;勿假设全零,也勿拒绝全零)。
    pub transform: [[f32; 4]; 3],
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub columns: Vec<VxlColumn>,
}

/// VXL 文件(文件头 magic/one/unknown 已校验,不存储)。
pub struct VxlFile {
    /// 256 项 RGB 调色板(XCC 渲染实际用外部调色板;此处按字节存档)。
    pub palette: [[u8; 3]; 256],
    pub sections: Vec<VxlSection>,
}

impl VxlFile {
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        let file_len = data.len();
        if file_len < 802 {
            return Err(FormatError::UnexpectedEof {
                needed: 802,
                remaining: file_len,
            });
        }
        if &data[0..16] != VXL_MAGIC {
            return Err(FormatError::Invalid("VXL 魔数不符".to_string()));
        }
        let mut r = ByteReader::new(&data[16..]);
        let one = r.read_u32_le()? as i32;
        if one != 1 {
            return Err(FormatError::Invalid(format!("VXL 头 one={one},应为 1")));
        }
        let c_headers = r.read_u32_le()?;
        let c_tailers = r.read_u32_le()?;
        if c_headers != c_tailers {
            return Err(FormatError::Invalid(format!(
                "节头数 {c_headers} 与节尾数 {c_tailers} 不等"
            )));
        }
        if c_headers == 0 {
            return Err(FormatError::Invalid("VXL 节数为 0".to_string()));
        }
        let s = c_headers as usize;
        let body_size = r.read_u32_le()? as usize;
        let unknown = r.read_u16_le()? as i16;
        if unknown != 0x1F10 {
            return Err(FormatError::Invalid(format!(
                "VXL 头 unknown=0x{unknown:04X},应为 0x1F10"
            )));
        }
        let mut palette = [[0u8; 3]; 256];
        for (i, slot) in palette.iter_mut().enumerate() {
            *slot = [data[34 + i * 3], data[35 + i * 3], data[36 + i * 3]];
        }
        // 三个区布局(精确等式,XCC vxl_file.cpp:19)
        let headers_start = 802usize;
        let body_start = headers_start + 28 * s;
        let tailer_start = file_len
            .checked_sub(92 * s)
            .ok_or_else(|| FormatError::Invalid("文件长不足以容纳节尾数组".to_string()))?;
        if body_start.checked_add(body_size) != Some(tailer_start) {
            return Err(FormatError::Invalid(format!(
                "尺寸等式不符: 802+28*{s}+{body_size} != 文件长-92*{s} ({tailer_start})"
            )));
        }
        // 节头(名字截首 NUL,尾部垃圾不校验;section_i 必须等于下标)
        let mut names = Vec::with_capacity(s);
        let mut r2 = ByteReader::new(&data[headers_start..body_start]);
        for i in 0..s {
            let raw = r2.read_bytes(16)?;
            let end = raw.iter().position(|&b| b == 0).unwrap_or(16);
            let name = String::from_utf8_lossy(&raw[..end]).into_owned();
            let section_i = r2.read_u32_le()? as i32;
            if section_i != i as i32 {
                return Err(FormatError::Invalid(format!(
                    "节 {i} 的 section_i={section_i}"
                )));
            }
            let _one = r2.read_u32_le()?; // 真库为 1;XCC 自身不校验,此处宽松
            let _zero = r2.read_u32_le()?; // 真库为 0;同上
            names.push(name);
        }
        // 节尾(92 字节/节,恰在文件尾结束;节 i 在 file_len-92*(S-i))
        struct TailerRaw {
            span_start_ofs: usize,
            span_end_ofs: usize,
            span_data_ofs: usize,
            scale: f32,
            transform: [[f32; 4]; 3],
            min: [f32; 3],
            max: [f32; 3],
            dims: [u8; 3],
            normal_type: i8,
        }
        let mut tailers = Vec::with_capacity(s);
        for i in 0..s {
            let base = tailer_start + 92 * i;
            let t = &data[base..base + 92];
            let i32_at = |off: usize| -> i32 {
                i32::from_le_bytes([t[off], t[off + 1], t[off + 2], t[off + 3]])
            };
            let f32_at = |off: usize| -> f32 {
                f32::from_le_bytes([t[off], t[off + 1], t[off + 2], t[off + 3]])
            };
            let (sso, seo, sdo) = (i32_at(0), i32_at(4), i32_at(8));
            if sso < 0 || seo < 0 || sdo < 0 {
                return Err(FormatError::Invalid(format!("节 {i} 的 span 偏移为负")));
            }
            let mut transform = [[0f32; 4]; 3];
            for (row, trow) in transform.iter_mut().enumerate() {
                for (col, cell) in trow.iter_mut().enumerate() {
                    *cell = f32_at(16 + 4 * (row * 4 + col));
                }
            }
            tailers.push(TailerRaw {
                span_start_ofs: sso as usize,
                span_end_ofs: seo as usize,
                span_data_ofs: sdo as usize,
                scale: f32_at(12),
                transform,
                min: [f32_at(64), f32_at(68), f32_at(72)],
                max: [f32_at(76), f32_at(80), f32_at(84)],
                dims: [t[88], t[89], t[90]],
                normal_type: t[91] as i8,
            });
        }
        // 逐节解码列
        let body = &data[body_start..tailer_start];
        let mut sections = Vec::with_capacity(s);
        for i in 0..s {
            let t = &tailers[i];
            let [cx, cy, cz] = t.dims;
            let columns_count = cx as usize * cy as usize;
            let (start_ofs, end_ofs, data_ofs) =
                (t.span_start_ofs, t.span_end_ofs, t.span_data_ofs);
            if start_ofs > body_size
                || end_ofs > body_size
                || data_ofs > body_size
                || end_ofs < start_ofs
                || end_ofs - start_ofs != 4 * columns_count
            {
                return Err(FormatError::Invalid(format!(
                    "节 {i} 的 span 表布局不符(需两表各 4*{columns_count} 字节,体区内)"
                )));
            }
            let ends_end = end_ofs + 4 * columns_count;
            if ends_end > body_size {
                return Err(FormatError::Invalid(format!("节 {i} 的 span_end 表越界")));
            }
            // as_chunks 而非 chunks_exact+map:clippy 1.99 deny-by-default lint
            // chunks_exact_to_as_chunks(行为等价——两者同样丢弃尾部不足 4 字节的余量,
            // 此处表长恒为 4 的倍数,余量恒为空)。
            let i32s = |slice: &[u8]| -> Vec<i32> {
                slice
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|&c| i32::from_le_bytes(c))
                    .collect()
            };
            let starts = i32s(&body[start_ofs..end_ofs]);
            let ends = i32s(&body[end_ofs..ends_end]);
            let mut columns = Vec::with_capacity(columns_count);
            for (j, (&sstart, &send)) in starts.iter().zip(ends.iter()).enumerate() {
                if sstart == -1 && send == -1 {
                    columns.push(VxlColumn { voxels: Vec::new() });
                    continue;
                }
                if sstart < 0 || send < 0 {
                    return Err(FormatError::Invalid(format!(
                        "节 {i} 列 {j}:-1 必须成对出现"
                    )));
                }
                if send < sstart {
                    return Err(FormatError::Invalid(format!(
                        "节 {i} 列 {j}:span_end < span_start"
                    )));
                }
                let win_start = data_ofs + sstart as usize;
                let win_end = data_ofs + send as usize + 1;
                if win_end > body_size {
                    return Err(FormatError::Invalid(format!("节 {i} 列 {j}:span 窗口越界")));
                }
                let win = &body[win_start..win_end];
                let mut voxels = Vec::new();
                let mut z: usize = 0;
                let mut wp = 0usize;
                while z < cz as usize {
                    if win.len() - wp < 2 {
                        return Err(FormatError::Invalid(format!("节 {i} 列 {j}:span 提前耗尽")));
                    }
                    let skip = win[wp] as usize;
                    let count = win[wp + 1] as usize;
                    wp += 2;
                    z += skip;
                    if z > cz as usize {
                        return Err(FormatError::Invalid(format!("节 {i} 列 {j}:skip 越过层顶")));
                    }
                    if win.len() - wp < 2 * count + 1 {
                        return Err(FormatError::Invalid(format!("节 {i} 列 {j}:体素段越界")));
                    }
                    for k in 0..count {
                        voxels.push(VxlVoxel {
                            z: (z + k) as u8,
                            color: win[wp + 2 * k],
                            normal: win[wp + 2 * k + 1],
                        });
                    }
                    z += count;
                    wp += 2 * count + 1; // 末尾 pad 字节:消耗不校验(XCC 同)
                }
                columns.push(VxlColumn { voxels });
            }
            sections.push(VxlSection {
                name: names[i].clone(),
                dims: t.dims,
                normal_type: t.normal_type,
                scale: t.scale,
                transform: t.transform,
                min: t.min,
                max: t.max,
                columns,
            });
        }
        Ok(VxlFile { palette, sections })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造 1 节最小 VXL:cx=2, cy=1, cz=2;
    /// 列 0 = col0 字节串(体素段 + 终止空段),列 1 = 空(-1/-1)。
    fn sample_vxl_with(col0: &[u8]) -> Vec<u8> {
        let (cx, cy, cz) = (2u8, 1u8, 2u8);
        let lists_len = 4 * cx as usize * cy as usize; // 8
        let mut body = Vec::new();
        body.extend_from_slice(&0i32.to_le_bytes()); // span_start[0]
        body.extend_from_slice(&(-1i32).to_le_bytes()); // span_start[1] 空
        body.extend_from_slice(&((col0.len() - 1) as i32).to_le_bytes()); // span_end[0]
        body.extend_from_slice(&(-1i32).to_le_bytes()); // span_end[1] 空
        body.extend_from_slice(col0);
        let body_size = body.len();

        let mut v = Vec::new();
        v.extend_from_slice(VXL_MAGIC); // 0..16
        v.extend_from_slice(&1u32.to_le_bytes()); // 16..20 one
        v.extend_from_slice(&1u32.to_le_bytes()); // 20..24 c_section_headers
        v.extend_from_slice(&1u32.to_le_bytes()); // 24..28 c_section_tailers
        v.extend_from_slice(&(body_size as u32).to_le_bytes()); // 28..32 size
        v.extend_from_slice(&0x1F10u16.to_le_bytes()); // 32..34 unknown
        v.resize(802, 0); // 34..802 调色板全零
        let mut sec = [0u8; 28];
        sec[0..4].copy_from_slice(b"TEST");
        // section_i(+16)=0 保持;one(+20)=1
        sec[20..24].copy_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&sec); // 802..830
        v.extend_from_slice(&body); // 830..830+body_size
        let mut t = [0u8; 92];
        t[0..4].copy_from_slice(&0i32.to_le_bytes()); // span_start_ofs
        t[4..8].copy_from_slice(&(lists_len as i32).to_le_bytes()); // span_end_ofs
        t[8..12].copy_from_slice(&((2 * lists_len) as i32).to_le_bytes()); // span_data_ofs
        t[12..16].copy_from_slice(&(1.0f32 / 12.0).to_le_bytes()); // scale
        t[88] = cx;
        t[89] = cy;
        t[90] = cz;
        t[91] = 2; // normal_type
        v.extend_from_slice(&t);
        v
    }

    fn sample_vxl() -> Vec<u8> {
        // 列 0:skip 0, count 1, 体素 (0x39, 0x0F), pad;终止空段 skip 1, count 0, pad
        sample_vxl_with(&[0x00, 0x01, 0x39, 0x0F, 0x01, 0x01, 0x00, 0x00])
    }

    #[test]
    fn decodes_spans_and_empty_columns() {
        let vxl = VxlFile::parse(&sample_vxl()).unwrap();
        assert_eq!(vxl.sections.len(), 1);
        let s = &vxl.sections[0];
        assert_eq!(s.name, "TEST");
        assert_eq!(s.dims, [2, 1, 2]);
        assert_eq!(s.normal_type, 2);
        assert_eq!(s.columns.len(), 2);
        assert_eq!(
            s.columns[0].voxels,
            [VxlVoxel {
                z: 0,
                color: 0x39,
                normal: 0x0F
            }]
        );
        assert!(s.columns[1].voxels.is_empty());
        assert!((s.scale - 1.0 / 12.0).abs() < 1e-9);
        assert_eq!(vxl.palette[0], [0, 0, 0]);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut data = sample_vxl();
        data[0] ^= 0xFF;
        assert!(matches!(
            VxlFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_one_not_1() {
        let mut data = sample_vxl();
        data[16..20].copy_from_slice(&2u32.to_le_bytes());
        assert!(matches!(
            VxlFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_unknown_not_1f10() {
        let mut data = sample_vxl();
        data[32..34].copy_from_slice(&0u16.to_le_bytes());
        assert!(matches!(
            VxlFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_counts_mismatch() {
        let mut data = sample_vxl();
        data[24..28].copy_from_slice(&2u32.to_le_bytes()); // c_section_tailers=2
        assert!(matches!(
            VxlFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_size_equation_mismatch() {
        let mut data = sample_vxl();
        data[28..32].copy_from_slice(&25u32.to_le_bytes()); // 实际 body_size=24
        assert!(matches!(
            VxlFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_skip_past_top_layer() {
        // skip=3 直接越过 cz=2
        let data = sample_vxl_with(&[0x03, 0x01, 0x39, 0x0F, 0x01, 0x01, 0x00, 0x00]);
        assert!(matches!(
            VxlFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }
}
