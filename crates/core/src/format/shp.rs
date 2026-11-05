// SPDX-License-Identifier: GPL-3.0-only

//! TS/RA2 时代 SHP 精灵(.shp,XCC 称 `shp_ts`)。
//!
//! 布局(研究文档 2026-11-03-shp-tmp-layouts.md §1,真实样本逐字节验证):
//! 8 字节文件头 `{u16 zero=0; u16 cx; u16 cy; u16 frames}` + `frames`×24 字节
//! 帧头 `{i16 x; i16 y; u16 cx; u16 cy; u32 flags; u8 rgb[3]; u8 alpha;
//! u32 zero2; u32 offset}`。`flags` bit1(值 2)=帧数据为 RLEZeroTS 压缩;
//! bit0 观测恒置 1(语义未知,忽略)。空帧约定 `cx==cy==offset==0`(此时
//! flags 仍非 0,勿以 flags 判空)。阴影帧按位置配对(`i >= frames/2`),
//! 本层不做配对——留给 view 层。

use super::bytes::ByteReader;
use super::rle_zero_ts::rle_zero_ts_decode;
use super::shp_ts::MAX_SHP_CANVAS_PIXELS;
use super::FormatError;

/// 单帧帧头(`alpha`/`zero2` 已读入校验/忽略,不存储)。
pub struct ShpFrameHeader {
    pub x: i16,
    pub y: i16,
    pub cx: u16,
    pub cy: u16,
    pub flags: u32,
    /// 每帧 RGB 三元组(XCC 读取但不用于解码)。
    pub rgb: [u8; 3],
    /// 帧数据的绝对文件偏移;空帧为 0。
    pub offset: u32,
}

/// 一帧:帧头 + 解码后的调色板索引(行主序,恰好 `cx*cy` 字节;空帧为空)。
pub struct ShpFrame {
    pub header: ShpFrameHeader,
    pub pixels: Vec<u8>,
}

/// SHP 文件。
pub struct ShpFile {
    pub cx: u16,
    pub cy: u16,
    pub frames: Vec<ShpFrame>,
}

impl ShpFile {
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
            let rgb = [r.read_u8()?, r.read_u8()?, r.read_u8()?];
            let _alpha = r.read_u8()?;
            let zero2 = r.read_u32_le()?;
            if zero2 != 0 {
                return Err(FormatError::Invalid(format!("帧头 zero2={zero2},应为 0")));
            }
            let offset = r.read_u32_le()?;
            headers.push(ShpFrameHeader {
                x,
                y,
                cx: fh_cx,
                cy: fh_cy,
                flags,
                rgb,
                offset,
            });
        }
        let table_end = r.position(); // = 8 + 24*frame_count
        let file_len = data.len();
        // 先做全部结构校验(XCC shp_ts_file.cpp:101-106),再解码
        for h in &headers {
            let empty = h.cx == 0 && h.cy == 0 && h.offset == 0;
            if empty {
                continue;
            }
            if (h.offset as usize) < table_end || (h.offset as usize) >= file_len {
                return Err(FormatError::Invalid(format!(
                    "帧数据偏移 {} 越界(表尾 {table_end},文件长 {file_len})",
                    h.offset
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
        // 每个压缩帧的数据上界:此后第一个更大的帧偏移(空帧 offset=0 不参与)
        // 或文件尾;两者之间可能有 2-7 字节 0 填充,count 链是唯一权威。
        let mut ends = vec![file_len; frame_count];
        for (i, h) in headers.iter().enumerate() {
            let start = h.offset as usize;
            if let Some(o) = headers[i + 1..]
                .iter()
                .map(|n| n.offset as usize)
                .find(|&o| o > start)
            {
                ends[i] = o;
            }
        }
        let mut frames = Vec::with_capacity(frame_count);
        for (h, end) in headers.into_iter().zip(ends) {
            let empty = h.cx == 0 && h.cy == 0 && h.offset == 0;
            let pixels = if empty {
                Vec::new()
            } else if h.flags & 2 != 0 {
                // 压缩帧:RLEZeroTS 行链
                let start = h.offset as usize;
                let src = data.get(start..end).ok_or_else(|| {
                    FormatError::Invalid(format!("帧数据区间 {start}..{end} 越界"))
                })?;
                let need = h.cx as usize * h.cy as usize;
                if need > MAX_SHP_CANVAS_PIXELS {
                    return Err(FormatError::Invalid(format!(
                        "SHP 画布 {}x{} 共 {need} 像素,超分配前上界 {MAX_SHP_CANVAS_PIXELS}",
                        h.cx, h.cy
                    )));
                }
                let mut pixels = Vec::with_capacity(need);
                rle_zero_ts_decode(src, h.cx as usize, h.cy as usize, &mut pixels)?;
                pixels
            } else {
                // 未压缩帧:恰好 cx*cy 字节原始调色板索引
                let start = h.offset as usize;
                let need = h.cx as usize * h.cy as usize;
                let src = data.get(start..start + need).ok_or_else(|| {
                    FormatError::Invalid(format!(
                        "未压缩帧需要 {need} 字节,文件仅剩 {}",
                        file_len - start
                    ))
                })?;
                src.to_vec()
            };
            frames.push(ShpFrame { header: h, pixels });
        }
        Ok(ShpFile { cx, cy, frames })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_header(x: i16, y: i16, cx: u16, cy: u16, flags: u32, offset: u32) -> [u8; 24] {
        let mut b = [0u8; 24];
        b[0..2].copy_from_slice(&x.to_le_bytes());
        b[2..4].copy_from_slice(&y.to_le_bytes());
        b[4..6].copy_from_slice(&cx.to_le_bytes());
        b[6..8].copy_from_slice(&cy.to_le_bytes());
        b[8..12].copy_from_slice(&flags.to_le_bytes());
        // rgb(3) + alpha(1) + zero2(4) 保持 0
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
    fn parses_uncompressed_frame() {
        // 画布 4x2,1 帧 4x2 未压缩,数据紧跟表尾(offset=32)
        let body = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        let data = shp_bytes((4, 2), &[frame_header(0, 0, 4, 2, 1, 32)], &body);
        let shp = ShpFile::parse(&data).unwrap();
        assert_eq!((shp.cx, shp.cy), (4, 2));
        assert_eq!(shp.frames.len(), 1);
        assert_eq!(shp.frames[0].header.flags, 1);
        assert_eq!(shp.frames[0].pixels, body);
    }

    #[test]
    fn parses_compressed_frame_with_clamp() {
        // 画布 4x4,1 帧 4x2 压缩(flags=3):行 1 的尾零游程多发 1,截断到行宽
        // (两块 count 均为 5=2 头+3 数据;brief 原文第二块 0x06 超出缓冲、
        //  期望行 0 的 0x01 系游程长度字节误抄,按已验证语义改为补齐的透明像素)
        let body: Vec<u8> = vec![0x05, 0x00, 0xAA, 0x00, 0x01, 0x05, 0x00, 0xBB, 0x00, 0x04];
        let data = shp_bytes((4, 4), &[frame_header(0, 0, 4, 2, 3, 32)], &body);
        let shp = ShpFile::parse(&data).unwrap();
        let f = &shp.frames[0];
        assert_eq!(f.header.flags, 3);
        assert_eq!(f.pixels, vec![0xAA, 0, 0, 0, 0xBB, 0, 0, 0]);
    }

    #[test]
    fn empty_frame_has_no_pixels() {
        // 帧 0 未压缩 2x1;帧 1 空帧(cx=cy=offset=0,flags 仍非 0)
        // (两帧帧头表尾 = 8+24*2 = 56,offset 须 ≥ 56;brief 原文沿用单帧的 32)
        let body = [0x55, 0x66];
        let data = shp_bytes(
            (4, 2),
            &[
                frame_header(0, 0, 2, 1, 1, 56),
                frame_header(0, 0, 0, 0, 3, 0),
            ],
            &body,
        );
        let shp = ShpFile::parse(&data).unwrap();
        assert_eq!(shp.frames.len(), 2);
        assert_eq!(shp.frames[0].pixels, body);
        assert!(shp.frames[1].pixels.is_empty());
        assert_eq!(shp.frames[1].header.flags, 3);
    }

    #[test]
    fn rejects_bad_zero_field() {
        let mut data = shp_bytes((4, 2), &[frame_header(0, 0, 4, 2, 1, 32)], &[0u8; 8]);
        data[0..2].copy_from_slice(&7u16.to_le_bytes());
        assert!(matches!(
            ShpFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_zero_frames() {
        let data = shp_bytes((4, 2), &[], &[]);
        assert!(matches!(
            ShpFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_frame_out_of_canvas() {
        // 4x2 的帧放在 x=3 → 超出画布 4
        let data = shp_bytes((4, 2), &[frame_header(3, 0, 4, 2, 1, 32)], &[0u8; 8]);
        assert!(matches!(
            ShpFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_offset_before_table() {
        // offset 指向帧头表内部
        let data = shp_bytes((4, 2), &[frame_header(0, 0, 4, 2, 1, 16)], &[0u8; 8]);
        assert!(matches!(
            ShpFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_canvas_over_allocation_cap() {
        // 敌意 ~40 B blob 谎报画布 65535×65535:压缩帧分配前上界拦截,
        // 恒 Err,不得先做巨型 Vec 预留
        let data = shp_bytes(
            (65535, 65535),
            &[frame_header(0, 0, 65535, 65535, 3, 32)],
            &[0u8; 8],
        );
        assert!(matches!(
            ShpFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }
}
