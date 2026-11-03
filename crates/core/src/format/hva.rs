// SPDX-License-Identifier: GPL-3.0-only

//! HVA 逐帧节变换(.hva)。
//!
//! 布局(研究文档 2026-11-03-vxl-hva-layouts.md §2):24 字节头
//! `{char id[16](3ds max 导出路径残片,忽略); i32 c_frames; i32 c_sections}` +
//! `S`×16 字节节名槽(首 NUL 截断,尾部垃圾不校验)+ `F*S`×48 字节矩阵。
//! 矩阵按 **帧主序** `[frame][section]` 排布(XCC 读取器语义;其写入器按节
//! 主序写是已知 bug hva_file.cpp:87,勿复刻)。每个矩阵 12×f32 = 3 行 × 4 列,
//! 行主序:前 3 列为旋转,第 4 列为平移。
//! 精确尺寸等式:file_size == 24 + (48*F + 16) * S,F>0,S>0。

use super::bytes::ByteReader;
use super::FormatError;

/// HVA 文件。`matrices[frame][section]`,每项 3×4 行主序。
pub struct HvaFile {
    pub frames: u32,
    pub section_names: Vec<String>,
    pub matrices: Vec<Vec<[[f32; 4]; 3]>>,
}

impl HvaFile {
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        if data.len() < 24 {
            return Err(FormatError::UnexpectedEof {
                needed: 24,
                remaining: data.len(),
            });
        }
        let mut r = ByteReader::new(data);
        let _id = r.read_bytes(16)?; // 导出管线残片(如 "D:\RA2X\MODELS\"),不解析
        let frames = r.read_u32_le()?;
        let sections = r.read_u32_le()?;
        if frames == 0 || sections == 0 {
            return Err(FormatError::Invalid(format!(
                "HVA frames={frames} sections={sections},都必须 > 0"
            )));
        }
        // 尺寸等式用 u64,防 u32 计数在 usize 上溢出
        let expected = 24u64 + 16 * sections as u64 + 48 * frames as u64 * sections as u64;
        if data.len() as u64 != expected {
            // 勘误:brief 原文格式串混用 `{}` 与内联捕获 `{expected}` 却未传
            // 位置实参,无法编译(compile error);此处补传 data.len(),消息文本不变。
            return Err(FormatError::Invalid(format!(
                "HVA 尺寸等式不符: 文件 {} 字节,应为 {expected}(F={frames}, S={sections})",
                data.len()
            )));
        }
        let s = sections as usize;
        let f = frames as usize;
        let names_region = 24 + 16 * s;
        let mut r2 = ByteReader::new(&data[24..names_region]);
        let mut section_names = Vec::with_capacity(s);
        for _ in 0..s {
            let raw = r2.read_bytes(16)?;
            let end = raw.iter().position(|&b| b == 0).unwrap_or(16);
            section_names.push(String::from_utf8_lossy(&raw[..end]).into_owned());
        }
        let mut matrices = Vec::with_capacity(f);
        // 勘误:brief 原文读名用独立游标 r2 却未推进 r(仍停在偏移 24,即名区
        // 开头),矩阵会误吃名区字节;此处按其自身布局注释(矩阵起于 24+16*S)
        // 跳过名区,行为与布局文档一致。
        r.skip(16 * s)?;
        for _ in 0..f {
            let mut frame = Vec::with_capacity(s);
            for _ in 0..s {
                let mut m = [[0f32; 4]; 3];
                // clippy needless_range_loop 等价改写:仍按行主序逐个读 12×f32
                for row in &mut m {
                    for cell in row {
                        let bytes = r.read_bytes(4)?;
                        *cell = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    }
                }
                frame.push(m);
            }
            matrices.push(frame);
        }
        Ok(HvaFile {
            frames,
            section_names,
            matrices,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hva_bytes(f: u32, s: u32, names: &[&str], values: &[[f32; 12]]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"C:\\3DSMAX\\MESHE\0"); // 16 字节 id 残片
        v.extend_from_slice(&f.to_le_bytes());
        v.extend_from_slice(&s.to_le_bytes());
        for name in names {
            let mut slot = [0u8; 16];
            let bytes = name.as_bytes();
            slot[..bytes.len()].copy_from_slice(bytes);
            v.extend_from_slice(&slot);
        }
        for m in values {
            for x in m {
                v.extend_from_slice(&x.to_le_bytes());
            }
        }
        v
    }

    #[test]
    fn frame_major_ordering() {
        // XCC 写入器按节主序写是其已知 bug;真库是帧主序——本测试锁定:
        // 第 2 个写入值属于 (帧0,节1),第 3 个属于 (帧1,节0)
        let m00 = [1.0f32; 12];
        let mut m01 = [0.0f32; 12];
        m01[3] = 7.5; // 平移列,便于辨认
        let m10 = [2.0f32; 12];
        let m11 = [3.0f32; 12];
        let data = hva_bytes(2, 2, &["A", "B"], &[m00, m01, m10, m11]);
        let hva = HvaFile::parse(&data).unwrap();
        assert_eq!(hva.frames, 2);
        assert_eq!(hva.section_names, ["A", "B"]);
        assert_eq!(hva.matrices[0][0][0][3], 1.0); // m00
        assert_eq!(hva.matrices[0][1][0][3], 7.5); // m01
        assert_eq!(hva.matrices[1][0][0][0], 2.0); // m10(节主序会误读成 m01)
        assert_eq!(hva.matrices[1][1][0][0], 3.0); // m11
    }

    #[test]
    fn names_trim_at_first_nul() {
        // 真库名槽尾部有垃圾字节(1tnk.hva: "B-BODY\0" 后跟 17 00 ...),截断不校验
        let mut v = Vec::new();
        v.extend_from_slice(&[0u8; 16]);
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes());
        let mut slot = [0u8; 16];
        slot[..6].copy_from_slice(b"B-BODY");
        slot[6] = 0; // 终止 NUL
        slot[7] = 0x17; // 垃圾
        slot[12] = 0xEE; // 垃圾
        v.extend_from_slice(&slot);
        for _ in 0..12 {
            v.extend_from_slice(&1.0f32.to_le_bytes());
        }
        let hva = HvaFile::parse(&v).unwrap();
        assert_eq!(hva.section_names, ["B-BODY"]);
    }

    #[test]
    fn rejects_zero_frames() {
        let data = hva_bytes(0, 2, &["A", "B"], &[]);
        assert!(matches!(
            HvaFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_size_mismatch() {
        let mut data = hva_bytes(2, 2, &["A", "B"], &[[1.0f32; 12]; 4]);
        data.pop();
        assert!(matches!(
            HvaFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }
}
