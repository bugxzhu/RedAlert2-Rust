// SPDX-License-Identifier: GPL-3.0-only

//! RA2 剧场地形瓦片容器(.sno/.tem/.urb;RA2 isometric 布局)。
//!
//! 注意:这是从用户自有样本逆向得到的 RA2 剧场布局(研究文档
//! 2026-11-03-shp-tmp-layouts.md §2;XCC 没有 RA2 TMP 读取器)。它与 XCC 的
//! TS TMP(`t_tmp_ts_header`,16 字节 i32 头 + 52 字节瓦片头)不是同一种
//! 容器,勿混用;日后若要支持 TS 瓦片应另立模块并以头部判别。
//!
//! 布局(两个 .sno 样本完全自洽,`data_start = 8 + 24*tiles` 线性精确成立):
//! 8 字节头 `{u16 zero=0; u16 width; u16 height; u16 tiles}` +
//! `tiles`×24 字节矩形项 `{i16 x; i16 y; u16 cx; u16 cy; u32 unknown_a;
//! u16 _b; u16 unknown_d; u8 radar[3]; u8 _pad; u32 data_offset}`。
//! 图形数据为 RLEZeroTS 行链(与 SHP 同方案,含同样的 +1 零游程怪癖);
//! 空瓦片约定 `cx==cy==data_offset==0`。

use super::bytes::ByteReader;
use super::rle_zero_ts::rle_zero_ts_decode;
use super::FormatError;

/// 单个矩形项(存储字段;`_b`/`_pad` 观测恒 0,跳过不存储)。
pub struct TmpRa2Tile {
    pub x: i16,
    pub y: i16,
    pub cx: u16,
    pub cy: u16,
    /// 观测:多数图形项 3、个别 4;空项也非 0(语义未知,按名保留)。
    pub unknown_a: u32,
    /// 观测:空项 0;图形项 1..=6(疑似地形/坡度类别)。
    pub unknown_d: u16,
    /// 疑似小地图 RGB(观测:除 lobrdb08 t0 = `00 fb 12` 外恒 0)。
    pub radar: [u8; 3],
    /// RLE 数据的绝对文件偏移;0 = 空瓦片。
    pub data_offset: u32,
}

/// 一个矩形项 + 解码后的调色板索引(行主序,恰好 `cx*cy` 字节;空瓦片为空)。
pub struct TmpRa2TileDecoded {
    pub header: TmpRa2Tile,
    pub pixels: Vec<u8>,
}

/// RA2 剧场瓦片文件:图形矩形可任意摆放(`x,y` 为画布内位置),不逐瓦片展开菱形。
pub struct TmpRa2File {
    pub width: u16,
    pub height: u16,
    pub tiles: Vec<TmpRa2TileDecoded>,
}

impl TmpRa2File {
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        let mut r = ByteReader::new(data);
        let zero = r.read_u16_le()?;
        if zero != 0 {
            return Err(FormatError::Invalid(format!("TMP 头 zero={zero},应为 0")));
        }
        let width = r.read_u16_le()?;
        let height = r.read_u16_le()?;
        let tiles_count = r.read_u16_le()? as usize;
        if tiles_count == 0 {
            return Err(FormatError::Invalid("TMP 瓦片数为 0".to_string()));
        }
        let mut headers = Vec::with_capacity(tiles_count);
        for _ in 0..tiles_count {
            let x = r.read_u16_le()? as i16;
            let y = r.read_u16_le()? as i16;
            let cx = r.read_u16_le()?;
            let cy = r.read_u16_le()?;
            let unknown_a = r.read_u32_le()?;
            // 勘误(2026-11-03 样本直读复核):条目步长 24 字节,unknown_a 之后
            // 只有一个零 u16 间隙(_b),没有 _c——brief 原文多列一个 u16,字段
            // 合计 26 字节,与实测步长相矛盾(两样本 offset 56=8+24·2、
            // 152=8+24·6 线性精确成立;radar 字节恰在 +0x10 处 `00 fb 12`)。
            let _b = r.read_u16_le()?;
            let unknown_d = r.read_u16_le()?;
            let radar = [r.read_u8()?, r.read_u8()?, r.read_u8()?];
            let _pad = r.read_u8()?;
            let data_offset = r.read_u32_le()?;
            headers.push(TmpRa2Tile {
                x,
                y,
                cx,
                cy,
                unknown_a,
                unknown_d,
                radar,
                data_offset,
            });
        }
        let table_end = r.position(); // = 8 + 24*tiles
        let file_len = data.len();
        for h in &headers {
            let empty = h.cx == 0 && h.cy == 0 && h.data_offset == 0;
            if empty {
                continue;
            }
            if h.cx == 0 || h.cy == 0 || h.data_offset == 0 {
                return Err(FormatError::Invalid(
                    "瓦片矩形与数据偏移必须同时为 0(空)或同时非 0".to_string(),
                ));
            }
            if (h.data_offset as usize) < table_end || (h.data_offset as usize) >= file_len {
                return Err(FormatError::Invalid(format!(
                    "瓦片数据偏移 {} 越界(表尾 {table_end},文件长 {file_len})",
                    h.data_offset
                )));
            }
            if i32::from(h.x) + i32::from(h.cx) > i32::from(width)
                || i32::from(h.y) + i32::from(h.cy) > i32::from(height)
            {
                return Err(FormatError::Invalid(format!(
                    "瓦片矩形 ({},{},{},{}) 超出画布 {width}x{height}",
                    h.x, h.y, h.cx, h.cy
                )));
            }
        }
        let mut tiles = Vec::with_capacity(tiles_count);
        for h in headers {
            let empty = h.cx == 0 && h.cy == 0 && h.data_offset == 0;
            let pixels = if empty {
                Vec::new()
            } else {
                // RLE 行链以 count 自链,尾随 0 填充自然不被读取;传到文件尾即可
                let src = data.get(h.data_offset as usize..).ok_or_else(|| {
                    FormatError::Invalid(format!("瓦片数据偏移 {} 越界", h.data_offset))
                })?;
                let mut pixels = Vec::with_capacity(h.cx as usize * h.cy as usize);
                rle_zero_ts_decode(src, h.cx as usize, h.cy as usize, &mut pixels)?;
                pixels
            };
            tiles.push(TmpRa2TileDecoded { header: h, pixels });
        }
        Ok(TmpRa2File {
            width,
            height,
            tiles,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile_entry(
        x: i16,
        y: i16,
        cx: u16,
        cy: u16,
        unknown_a: u32,
        unknown_d: u16,
        offset: u32,
    ) -> [u8; 24] {
        let mut b = [0u8; 24];
        b[0..2].copy_from_slice(&x.to_le_bytes());
        b[2..4].copy_from_slice(&y.to_le_bytes());
        b[4..6].copy_from_slice(&cx.to_le_bytes());
        b[6..8].copy_from_slice(&cy.to_le_bytes());
        b[8..12].copy_from_slice(&unknown_a.to_le_bytes());
        // _b/_c 保持 0
        b[14..16].copy_from_slice(&unknown_d.to_le_bytes());
        // radar/_pad 保持 0
        b[20..24].copy_from_slice(&offset.to_le_bytes());
        b
    }

    fn tmp_bytes(w: u16, h: u16, headers: &[[u8; 24]], body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&0u16.to_le_bytes()); // zero
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.extend_from_slice(&(headers.len() as u16).to_le_bytes());
        for t in headers {
            v.extend_from_slice(t);
        }
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn parses_tiles_and_decodes_rle() {
        // 画布 60x60:图形瓦片 4x2 @数据 56(表尾,8+24*2),空瓦片跟随。
        // 勘误:brief 原body `05 00 31 00 01 06 00 32 00 04` 自相矛盾——行 0 块
        // count=5 消耗 5 字节后,行 1 块头声明 count=6 但只剩 5 字节(任何解码
        // 语义下都 EOF)。按断言像素 [31 00 01 00 | 32 00 00 00] 反推自洽编码:
        // 行 0 = 31 字面量 + 00 01 零游程 1 + 01 字面量(count=6),行 1 = 32
        // 字面量 + 00 04 零游程 4 在行宽处截断(count=5,触发 +1 怪癖)。
        let body: Vec<u8> = vec![
            0x06, 0x00, 0x31, 0x00, 0x01, 0x01, 0x05, 0x00, 0x32, 0x00, 0x04,
        ];
        let data = tmp_bytes(
            60,
            60,
            &[
                tile_entry(10, 20, 4, 2, 3, 1, 56),
                tile_entry(0, 0, 0, 0, 4, 0, 0),
            ],
            &body,
        );
        let tmp = TmpRa2File::parse(&data).unwrap();
        assert_eq!((tmp.width, tmp.height), (60, 60));
        assert_eq!(tmp.tiles.len(), 2);
        let t0 = &tmp.tiles[0];
        assert_eq!((t0.header.x, t0.header.y), (10, 20));
        assert_eq!(t0.header.unknown_a, 3);
        assert_eq!(t0.header.unknown_d, 1);
        assert_eq!(t0.header.data_offset, 56);
        assert_eq!(t0.pixels, vec![0x31, 0, 0x01, 0, 0x32, 0, 0, 0]);
        assert!(tmp.tiles[1].pixels.is_empty());
    }

    #[test]
    fn rejects_bad_zero_field() {
        let mut data = tmp_bytes(60, 60, &[tile_entry(0, 0, 4, 2, 3, 1, 56)], &[0u8; 8]);
        data[0..2].copy_from_slice(&9u16.to_le_bytes());
        assert!(matches!(
            TmpRa2File::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_zero_tiles() {
        let data = tmp_bytes(60, 60, &[], &[]);
        assert!(matches!(
            TmpRa2File::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_inconsistent_empty_tile() {
        // cx>0 但 data_offset=0:三字段必须同时为 0 或同时非 0
        let data = tmp_bytes(60, 60, &[tile_entry(0, 0, 4, 2, 3, 1, 0)], &[]);
        assert!(matches!(
            TmpRa2File::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_rect_out_of_canvas() {
        let data = tmp_bytes(60, 60, &[tile_entry(57, 0, 4, 2, 3, 1, 56)], &[0u8; 8]);
        assert!(matches!(
            TmpRa2File::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_offset_before_table() {
        let data = tmp_bytes(60, 60, &[tile_entry(0, 0, 4, 2, 3, 1, 16)], &[0u8; 8]);
        assert!(matches!(
            TmpRa2File::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }
}
