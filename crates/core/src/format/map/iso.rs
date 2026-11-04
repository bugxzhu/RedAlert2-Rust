// SPDX-License-Identifier: GPL-3.0-only

//! `[IsoMapPack5]` 解压后的等距瓦片记录流(11 字节记录)。
//!
//! 布局(研究 2026-11-04-map-pack-and-binary-sections.md §3;XCC
//! `cc_structures.h:207-216` `t_iso_map_pack_entry` + 34.4M 语料记录验证):
//! `{u16 x; u16 y; i16 tile(−1=无瓦片); u8 zero1; u8 zero2; u8 sub_tile;
//! u8 z; u8 zero3}`。记录数 ≤ (2W−1)·H([Map]Size = 0,0,W,H);
//! 流长 %11 ∈ {0,4}(4 = EA/FinalAlert 的全零尾)。
//! **存储的 (x,y) 是权威键**:4 张 TS 代官方图省略 void 格(riverram/
//! frstbite/hillbtwn/isleofoades,§3.6),缺失的行走格按无瓦片处理;
//! 行走顺序仅作数量上界,不作定位依据。该 (x,y) 坐标与 [Terrain]/
//! [Structures]/[Waypoints] 的菱形坐标恒等(研究 §3.4 的 749/749 等实证)。

use crate::format::FormatError;

/// 一条瓦片记录(三个恒零垫字节已校验,不存储)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsoTile {
    pub x: u16,
    pub y: u16,
    pub tile: i16,
    pub sub_tile: u8,
    pub z: u8,
}

/// 稀疏瓦片网格:按存储 (x,y) 排序索引(排序 Vec + 二分,无 HashMap)。
pub struct IsoGrid {
    records: Vec<IsoTile>,
    walk_capacity: usize,
}

fn cell_key(x: u16, y: u16) -> u64 {
    ((y as u64) << 16) | x as u64
}

impl IsoGrid {
    /// 从解压后的记录流解析。`w`/`h` 来自 [Map]Size(调用方已保证 ≥ 1;
    /// 此处再防御性钳制,敌意值不 panic)。
    pub fn parse(data: &[u8], w: u16, h: u16) -> Result<Self, FormatError> {
        match data.len() % 11 {
            0 => {}
            4 => {
                if data[data.len() - 4..] != [0, 0, 0, 0] {
                    return Err(FormatError::Invalid("瓦片流 4 字节尾不全为零".to_string()));
                }
            }
            _ => {
                return Err(FormatError::Invalid(format!(
                    "瓦片流长度 {} 对 11 取模非法(合法余数 0/4)",
                    data.len()
                )));
            }
        }
        let w = w.max(1);
        let h = h.max(1);
        // 行走容量 (2W−1)·H:u128 计算防敌意尺寸溢出(照 HVA u128 教训)
        let capacity = (2u128 * w as u128 - 1) * h as u128;
        let n = data.len() / 11;
        if n as u128 > capacity {
            return Err(FormatError::Invalid(format!(
                "瓦片记录 {n} 超过行走容量 {capacity} = (2·{w}−1)·{h}"
            )));
        }
        let max_xy = w as u32 + h as u32 - 1;
        let mut records = Vec::with_capacity(n);
        for i in 0..n {
            let b = &data[i * 11..i * 11 + 11];
            if b[6] != 0 || b[7] != 0 || b[10] != 0 {
                return Err(FormatError::Invalid(format!("记录 {i} 的恒零垫字节非零")));
            }
            let x = u16::from_le_bytes([b[0], b[1]]);
            let y = u16::from_le_bytes([b[2], b[3]]);
            if x == 0 || y == 0 || x as u32 > max_xy || y as u32 > max_xy {
                return Err(FormatError::Invalid(format!(
                    "记录 {i} 的坐标 ({x},{y}) 越出菱形界 1..={max_xy}"
                )));
            }
            records.push(IsoTile {
                x,
                y,
                tile: i16::from_le_bytes([b[4], b[5]]),
                sub_tile: b[8],
                z: b[9],
            });
        }
        // (x,y) 唯一(语料 34.4M 记录零重复;重复即畸形)+ 排序建二分索引
        records.sort_by_key(|t| cell_key(t.x, t.y));
        for pair in records.windows(2) {
            if cell_key(pair[0].x, pair[0].y) == cell_key(pair[1].x, pair[1].y) {
                return Err(FormatError::Invalid(format!(
                    "坐标 ({},{}) 重复",
                    pair[0].x, pair[0].y
                )));
            }
        }
        Ok(IsoGrid {
            records,
            walk_capacity: capacity as usize,
        })
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// 行走容量上界 (2W−1)·H(统计/交叉验证用)。
    pub fn walk_capacity(&self) -> usize {
        self.walk_capacity
    }

    pub fn records(&self) -> &[IsoTile] {
        &self.records
    }

    /// 按存储 (x,y) 查格(二分);None = 该格无记录(缺格/无瓦片)。
    pub fn cell(&self, x: u16, y: u16) -> Option<&IsoTile> {
        let key = cell_key(x, y);
        self.records
            .binary_search_by_key(&key, |t| cell_key(t.x, t.y))
            .ok()
            .map(|i| &self.records[i])
    }

    /// 最大非 −1 tile 索引(剧场交叉验证用;全空/全 −1 返回 None)。
    pub fn max_tile_index(&self) -> Option<i16> {
        self.records
            .iter()
            .map(|t| t.tile)
            .filter(|&t| t >= 0)
            .max()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(x: u16, y: u16, tile: i16, sub: u8, z: u8) -> [u8; 11] {
        let mut b = [0u8; 11];
        b[0..2].copy_from_slice(&x.to_le_bytes());
        b[2..4].copy_from_slice(&y.to_le_bytes());
        b[4..6].copy_from_slice(&tile.to_le_bytes());
        b[8] = sub;
        b[9] = z;
        b
    }

    #[test]
    fn parses_real_c1a01md_records() {
        // 规格依据 §5:c1a01md 块 0 的 rec0/rec1 原始字节 + 4 零尾
        let mut data = Vec::new();
        data.extend_from_slice(&[
            0x57, 0x00, 0x01, 0x00, 0x3C, 0x01, 0x00, 0x00, 0x01, 0x02, 0x00,
        ]);
        data.extend_from_slice(&[
            0x56, 0x00, 0x02, 0x00, 0x3C, 0x01, 0x00, 0x00, 0x02, 0x02, 0x00,
        ]);
        data.extend_from_slice(&[0, 0, 0, 0]);
        let grid = IsoGrid::parse(&data, 87, 71).unwrap();
        assert_eq!(grid.len(), 2);
        assert_eq!(grid.walk_capacity(), (2 * 87 - 1) * 71);
        let r0 = grid.cell(87, 1).unwrap();
        assert_eq!((r0.tile, r0.sub_tile, r0.z), (316, 1, 2));
        let r1 = grid.cell(86, 2).unwrap();
        assert_eq!((r1.tile, r1.sub_tile, r1.z), (316, 2, 2));
        assert!(grid.cell(1, 1).is_none());
        assert_eq!(grid.max_tile_index(), Some(316));
    }

    #[test]
    fn no_tail_stream_is_accepted() {
        // 社区产图器在 11·records 处精确截止(研究 §3.3)
        let data = rec(3, 4, 10, 0, 2);
        let grid = IsoGrid::parse(&data, 10, 10).unwrap();
        assert_eq!(grid.len(), 1);
        assert_eq!(grid.cell(3, 4).unwrap().tile, 10);
    }

    #[test]
    fn rejects_nonzero_pad_bytes() {
        let mut data = rec(3, 4, 10, 0, 2);
        data[6] = 1;
        assert!(matches!(
            IsoGrid::parse(&data, 10, 10),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_nonzero_four_byte_tail() {
        let mut data = Vec::new();
        data.extend_from_slice(&rec(3, 4, 10, 0, 2));
        data.extend_from_slice(&[0, 0, 0, 1]);
        assert!(matches!(
            IsoGrid::parse(&data, 10, 10),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_bad_stream_length() {
        let data = [0u8; 24]; // %11 = 2
        assert!(matches!(
            IsoGrid::parse(&data, 10, 10),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_count_over_walk_capacity() {
        // w=1,h=1 的行走容量 = 1:两条记录即超界(indicator.map 的恰 1 条是上界内)
        let mut data = Vec::new();
        data.extend_from_slice(&rec(1, 1, 0, 0, 0));
        data.extend_from_slice(&rec(1, 1, 1, 0, 0));
        assert!(matches!(
            IsoGrid::parse(&data, 1, 1),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_out_of_range_xy() {
        // w=10,h=10 → 菱形界 1..=19;x=20 越界;x=0 越界
        assert!(matches!(
            IsoGrid::parse(&rec(20, 1, 0, 0, 0), 10, 10),
            Err(FormatError::Invalid(_))
        ));
        assert!(matches!(
            IsoGrid::parse(&rec(0, 1, 0, 0, 0), 10, 10),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_duplicate_xy() {
        let mut data = Vec::new();
        data.extend_from_slice(&rec(3, 4, 10, 0, 0));
        data.extend_from_slice(&rec(3, 4, 11, 0, 0));
        assert!(matches!(
            IsoGrid::parse(&data, 10, 10),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn accepts_legacy_subset_of_walk() {
        // §3.6:riverram 等四图的记录数少于行走容量(缺格 = 存储子集)
        let mut data = Vec::new();
        data.extend_from_slice(&rec(10, 1, 5, 0, 0));
        data.extend_from_slice(&rec(9, 2, 6, 1, 3));
        data.extend_from_slice(&rec(8, 3, -1, 0, 1));
        let grid = IsoGrid::parse(&data, 7, 4).unwrap(); // 菱形界 1..=7+4−1=10,容量 (2·7−1)·4 = 52
        assert_eq!(grid.len(), 3);
        assert_eq!(grid.walk_capacity(), 52);
        assert!(grid.cell(7, 4).is_none()); // 行走格缺省无瓦片
        assert_eq!(grid.cell(9, 2).unwrap().sub_tile, 1);
        assert_eq!(grid.max_tile_index(), Some(6)); // −1 不参与最大值
    }

    #[test]
    fn max_tile_index_all_minus_one_is_none() {
        let data = rec(2, 2, -1, 0, 0);
        let grid = IsoGrid::parse(&data, 10, 10).unwrap();
        assert_eq!(grid.max_tile_index(), None);
    }
}
