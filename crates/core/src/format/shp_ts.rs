// SPDX-License-Identifier: GPL-3.0-only

//! TS SHP 精灵帧容器(t_shp_ts,XCC 术语;.sno/.tem 等剧场装饰精灵即此格式)。
//!
//! 出处:布局与判别按裁决后研究 2026-11-05-theater-tile-mapping.md §4/§6
//! (XCC `t_shp_ts` 语义重写;burnt01/lobrdb08 甲骨文帧偏移精确闭合)。
//! **M1b 误判纠正**:本文件前身 tmp.rs 曾把该 8B+24B 布局当成"RA2 剧场瓦片",
//! 实为 TS SHP 帧格式——帧头 24B = `{x,y i16; cx,cy u16; flags u32; rgba
//! [u8;4]; reserved u32; data_offset u32}`。TS 真瓦片(TS TMP,16B i32 头 +
//! 槽表)是另一种容器,由 theater_tmp.rs 承担,勿混用。
//!
//! 帧数据 = RLEZeroTS 行链(与 shp.rs 同方案)或 RAW cx·cy 字节(研究 §6:
//! 两形态无歧义交叉——先严格 RLE,失败且 span==cx·cy 才取 RAW)。空帧约定
//! `cx==cy==data_offset==0`(此时 flags 仍非 0,勿以 flags 判空)。

use super::bytes::ByteReader;
use super::rle_zero_ts::rle_zero_ts_decode;
use super::FormatError;

/// 语料 SHP 最大画布 253×242(研究 §8);上限给 277 倍余量防炸(spec"分配前上界")。
pub(crate) const MAX_SHP_CANVAS_PIXELS: usize = 4096 * 4096;

/// 单帧帧头(字段语义按研究 §4 重标)。
pub struct ShpTsFrameHeader {
    pub x: i16,
    pub y: i16,
    pub cx: u16,
    pub cy: u16,
    /// 观测多数 3、个别 4(bit0|bit1);语义未裁决,原样暴露。
    pub flags: u32,
    /// +0x0C..0x10 四字节;旧"unknown_d"即其蓝通道字节(研究 §4)。
    pub rgba: [u8; 4],
    /// +0x10..0x14(旧 radar[3]+_pad;lobrdb08 t0 = 0x0012FB00)。
    pub reserved: u32,
    /// 帧数据的绝对文件偏移;空帧为 0。
    pub data_offset: u32,
}

/// 一帧:帧头 + 解码后的调色板索引(行主序,恰好 `cx*cy` 字节;空帧为空)。
pub struct ShpTsFrame {
    pub header: ShpTsFrameHeader,
    pub pixels: Vec<u8>,
}

/// TS SHP 文件。
pub struct ShpTsFile {
    pub zero: u16,
    pub cx: u16,
    pub cy: u16,
    pub frames: Vec<ShpTsFrame>,
}

impl ShpTsFile {
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        let mut r = ByteReader::new(data);
        let zero = r.read_u16_le()?;
        if zero != 0 {
            return Err(FormatError::Invalid(format!("SHP 头 zero={zero},应为 0")));
        }
        let cx = r.read_u16_le()?;
        let cy = r.read_u16_le()?;
        let frame_count = r.read_u16_le()? as usize;
        if frame_count == 0 || frame_count > 10000 {
            return Err(FormatError::Invalid(format!(
                "SHP 帧数 {frame_count} 超出 1..=10000"
            )));
        }
        let mut headers = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            let x = r.read_u16_le()? as i16;
            let y = r.read_u16_le()? as i16;
            let fh_cx = r.read_u16_le()?;
            let fh_cy = r.read_u16_le()?;
            let flags = r.read_u32_le()?;
            let rgba = [r.read_u8()?, r.read_u8()?, r.read_u8()?, r.read_u8()?];
            let reserved = r.read_u32_le()?;
            let data_offset = r.read_u32_le()?;
            headers.push(ShpTsFrameHeader {
                x,
                y,
                cx: fh_cx,
                cy: fh_cy,
                flags,
                rgba,
                reserved,
                data_offset,
            });
        }
        let table_end = r.position(); // = 8 + 24*frame_count
        let file_len = data.len();
        for h in &headers {
            let empty = h.cx == 0 && h.cy == 0 && h.data_offset == 0;
            if empty {
                continue;
            }
            if h.cx == 0 || h.cy == 0 || h.data_offset == 0 {
                return Err(FormatError::Invalid(
                    "帧矩形与数据偏移必须同时为 0(空)或同时非 0".to_string(),
                ));
            }
            if (h.data_offset as usize) < table_end || (h.data_offset as usize) >= file_len {
                return Err(FormatError::Invalid(format!(
                    "帧数据偏移 {} 越界(表尾 {table_end},文件长 {file_len})",
                    h.data_offset
                )));
            }
            if i32::from(h.x) + i32::from(h.cx) > i32::from(cx)
                || i32::from(h.y) + i32::from(h.cy) > i32::from(cy)
            {
                return Err(FormatError::Invalid(format!(
                    "帧矩形 ({},{},{},{}) 超出画布 {cx}x{cy}",
                    h.x, h.y, h.cx, h.cy
                )));
            }
        }
        // 每帧数据区间:此后第一个更大的帧偏移(空帧 offset=0 不参与)或文件尾
        let mut ends = vec![file_len; frame_count];
        for (i, h) in headers.iter().enumerate() {
            let start = h.data_offset as usize;
            if let Some(o) = headers[i + 1..]
                .iter()
                .map(|n| n.data_offset as usize)
                .find(|&o| o > start)
            {
                ends[i] = o;
            }
        }
        let mut frames = Vec::with_capacity(frame_count);
        for (h, end) in headers.into_iter().zip(ends) {
            let empty = h.cx == 0 && h.cy == 0 && h.data_offset == 0;
            let pixels = if empty {
                Vec::new()
            } else {
                let start = h.data_offset as usize;
                let span = end - start;
                let need = h.cx as usize * h.cy as usize;
                if need > MAX_SHP_CANVAS_PIXELS {
                    return Err(FormatError::Invalid(format!(
                        "SHP 画布 {}x{} 共 {need} 像素,超分配前上界 {MAX_SHP_CANVAS_PIXELS}",
                        h.cx, h.cy
                    )));
                }
                let src = &data[start..end];
                let mut pixels = Vec::with_capacity(need);
                // 判别式(研究 §4/§6):先严格 RLE;失败且 span==cx·cy → RAW;
                // 失败且 span≠cx·cy → 透传 Err。RLE 可解恒取 RLE。
                match rle_zero_ts_decode(src, h.cx as usize, h.cy as usize, &mut pixels) {
                    Ok(_) => pixels,
                    Err(e) => {
                        if span == need {
                            pixels.clear();
                            pixels.extend_from_slice(&data[start..start + need]);
                            pixels
                        } else {
                            return Err(e);
                        }
                    }
                }
            };
            frames.push(ShpTsFrame { header: h, pixels });
        }
        Ok(ShpTsFile {
            zero,
            cx,
            cy,
            frames,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn frame_header(
        x: i16,
        y: i16,
        cx: u16,
        cy: u16,
        flags: u32,
        rgba: [u8; 4],
        reserved: u32,
        offset: u32,
    ) -> [u8; 24] {
        let mut b = [0u8; 24];
        b[0..2].copy_from_slice(&x.to_le_bytes());
        b[2..4].copy_from_slice(&y.to_le_bytes());
        b[4..6].copy_from_slice(&cx.to_le_bytes());
        b[6..8].copy_from_slice(&cy.to_le_bytes());
        b[8..12].copy_from_slice(&flags.to_le_bytes());
        b[12..16].copy_from_slice(&rgba);
        b[16..20].copy_from_slice(&reserved.to_le_bytes());
        b[20..24].copy_from_slice(&offset.to_le_bytes());
        b
    }

    fn shp_bytes(canvas: (u16, u16), headers: &[[u8; 24]], body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&0u16.to_le_bytes()); // zero
        v.extend_from_slice(&canvas.0.to_le_bytes());
        v.extend_from_slice(&canvas.1.to_le_bytes());
        v.extend_from_slice(&(headers.len() as u16).to_le_bytes());
        for h in headers {
            v.extend_from_slice(h);
        }
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn parses_frame_and_decodes_rle() {
        // 旧 tmp.rs parses_tiles_and_decodes_rle 的迁移(burnt01 形态):
        // 帧头 flags=3(旧 unknown_a)、rgba=[0,0,1,0](旧 unknown_d=1 的蓝通道)。
        // 勘误(实现时实测):brief 原稿只列 1 个帧头却断言 data_offset=56
        // ——甲骨文 56=8+24·2 需要两个帧头才闭合,单帧形态表尾 32、文件长 43,
        // offset 检查直接 Err("帧数据偏移 56 越界")。按迁移源(旧测试
        // 图形项 + 空项两项)与 burnt01 甲骨文恢复两帧形态,断言随之取 2。
        let body: Vec<u8> = vec![
            0x06, 0x00, 0x31, 0x00, 0x01, 0x01, 0x05, 0x00, 0x32, 0x00, 0x04,
        ];
        let data = shp_bytes(
            (60, 60),
            &[
                frame_header(10, 20, 4, 2, 3, [0, 0, 1, 0], 0, 56),
                frame_header(0, 0, 0, 0, 4, [0; 4], 0, 0),
            ],
            &body,
        );
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!((f.cx, f.cy), (60, 60));
        assert_eq!(f.frames.len(), 2);
        let t = &f.frames[0];
        assert_eq!((t.header.x, t.header.y), (10, 20));
        assert_eq!(t.header.flags, 3);
        assert_eq!(t.header.rgba, [0, 0, 1, 0]);
        assert_eq!(t.header.reserved, 0);
        assert_eq!(t.header.data_offset, 56);
        assert_eq!(t.pixels, vec![0x31, 0, 0x01, 0, 0x32, 0, 0, 0]);
        // 空帧:像素为空,flags 仍非 0(勿以 flags 判空)
        assert_eq!(f.frames[1].header.flags, 4);
        assert!(f.frames[1].pixels.is_empty());
    }

    #[test]
    fn rejects_bad_zero_field() {
        let mut data = shp_bytes(
            (60, 60),
            &[frame_header(0, 0, 4, 2, 3, [0; 4], 0, 56)],
            &[0u8; 8],
        );
        data[0..2].copy_from_slice(&9u16.to_le_bytes());
        assert!(matches!(
            ShpTsFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_zero_frames() {
        let data = shp_bytes((60, 60), &[], &[]);
        assert!(matches!(
            ShpTsFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_inconsistent_empty_frame() {
        // cx>0 但 data_offset=0:三字段必须同时为 0 或同时非 0
        let data = shp_bytes((60, 60), &[frame_header(0, 0, 4, 2, 3, [0; 4], 0, 0)], &[]);
        assert!(matches!(
            ShpTsFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_rect_out_of_canvas() {
        let data = shp_bytes(
            (60, 60),
            &[frame_header(57, 0, 4, 2, 3, [0; 4], 0, 56)],
            &[0u8; 8],
        );
        assert!(matches!(
            ShpTsFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_offset_before_table() {
        let data = shp_bytes(
            (60, 60),
            &[frame_header(0, 0, 4, 2, 3, [0; 4], 0, 16)],
            &[0u8; 8],
        );
        assert!(matches!(
            ShpTsFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn raw_fallback_when_rle_fails_and_span_exact() {
        // RAW 形态(研究 §6):span 恰 == cx·cy 且严格 RLE 失败(首行 count=2 < 4)
        let body = [0x02, 0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let data = shp_bytes((8, 2), &[frame_header(0, 0, 4, 2, 1, [0; 4], 0, 32)], &body);
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!(f.frames[0].pixels, body.to_vec());
    }

    #[test]
    fn rle_wins_when_span_equals_need() {
        // 优先序锁定:全透明帧的 RLE 行链(块头 4B + 零游程)span 也可恰 == cx·cy;
        // 此时必须按 RLE 解码为全 0,不得误取 RAW 原始字节
        let body = [0x04, 0x00, 0x00, 0x04];
        let data = shp_bytes((4, 1), &[frame_header(0, 0, 4, 1, 3, [0; 4], 0, 32)], &body);
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!(f.frames[0].pixels, vec![0, 0, 0, 0]);
    }

    #[test]
    fn err_when_rle_fails_and_span_mismatch() {
        // 严格 RLE 失败(截断)且 span(6)≠ cx·cy(8)→ 透传 Err
        let body = [0x08, 0x00, 0xAA, 0xBB, 0xCC, 0xDD];
        let data = shp_bytes((8, 2), &[frame_header(0, 0, 4, 2, 3, [0; 4], 0, 32)], &body);
        assert!(matches!(
            ShpTsFile::parse(&data),
            Err(FormatError::UnexpectedEof { .. })
        ));
    }

    #[test]
    fn rejects_canvas_over_allocation_cap() {
        // 敌意 ~40 B blob 谎报画布 65535×65535(u16² ≈ 4.29e9 像素):
        // 分配前上界拦截,恒 Err,不得先做巨型 Vec 预留
        let data = shp_bytes(
            (65535, 65535),
            &[frame_header(0, 0, 65535, 65535, 3, [0; 4], 0, 32)],
            &[0u8; 8],
        );
        assert!(matches!(
            ShpTsFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }
}
