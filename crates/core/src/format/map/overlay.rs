// SPDX-License-Identifier: GPL-3.0-only

//! `[OverlayPack]` / `[OverlayDataPack]` 解压后的双层 512×512 网格。
//!
//! 布局(研究 2026-11-04-map-pack-and-binary-sections.md §4):两段各解压到
//! **恰好 262144 B** = 行主序 512×512(`x = i & 511, y = i >> 9`;XCC
//! `write_map:586-593` 把内部紧凑流展开成满格再 LCW)——不是菱形行走序。
//! OverlayPack[i] = 类型 id(0xFF = 空),OverlayDataPack[i] = 帧/状态
//! (0 是合法值)。两层独立:46/540 真图在 type==0xFF 的格下有非零 data
//! (陈旧帧字节,含 EA 官图)——访问语义:**type==0xFF 的格 frame 恒 None**
//! (XCC `overlay_data_decode4` 同语义,map_ts_encoder.cpp:221-241)。
//! 菱形坐标 1..=W+H−1 直接当格坐标用(真图 max(W+H) = 499 < 512,恒在格内;
//! 越界格按空格处理,零 panic)。

use crate::format::FormatError;

pub const OVERLAY_GRID_LEN: usize = 512 * 512;

/// 双层覆盖网格(类型层 + 帧/状态层)。
pub struct OverlayLayers {
    types: Box<[u8; OVERLAY_GRID_LEN]>,
    frames: Box<[u8; OVERLAY_GRID_LEN]>,
}

impl OverlayLayers {
    /// 两层解压字节各须恰为 262144(不在中途截断/补零)。
    ///
    /// 经堆上 `Vec` 转换(`Box::try_from(Vec)` 原地重解释),避免 debug 构建
    /// 把 512KB 数组经栈中转导致测试线程溢栈。
    pub fn parse(types: &[u8], frames: &[u8]) -> Result<Self, FormatError> {
        let types: Box<[u8; OVERLAY_GRID_LEN]> =
            Box::try_from(types.to_vec()).map_err(|v: Vec<u8>| {
                FormatError::Invalid(format!(
                    "OverlayPack 解压长 {},应为 {OVERLAY_GRID_LEN}",
                    v.len()
                ))
            })?;
        let frames: Box<[u8; OVERLAY_GRID_LEN]> =
            Box::try_from(frames.to_vec()).map_err(|v: Vec<u8>| {
                FormatError::Invalid(format!(
                    "OverlayDataPack 解压长 {},应为 {OVERLAY_GRID_LEN}",
                    v.len()
                ))
            })?;
        Ok(OverlayLayers { types, frames })
    }

    fn index(x: u16, y: u16) -> Option<usize> {
        if x >= 512 || y >= 512 {
            return None;
        }
        Some((y as usize) << 9 | x as usize)
    }

    /// 类型 id:0xFF = 空;越界格(敌意坐标)返回 0xFF。
    pub fn type_at(&self, x: u16, y: u16) -> u8 {
        match Self::index(x, y) {
            Some(i) => self.types[i],
            None => 0xFF,
        }
    }

    /// 帧/状态:类型 0xFF 的格(含越界格)返回 None——陈旧 data 字节按格式
    /// 语义忽略,不计为损坏。
    pub fn frame_at(&self, x: u16, y: u16) -> Option<u8> {
        match Self::index(x, y) {
            Some(i) if self.types[i] != 0xFF => Some(self.frames[i]),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layers() -> OverlayLayers {
        let mut types = vec![0xFFu8; OVERLAY_GRID_LEN];
        let mut frames = vec![0xEEu8; OVERLAY_GRID_LEN];
        // 格 (3,2):类型 7、帧 3
        types[3 + (2 << 9)] = 7;
        frames[3 + (2 << 9)] = 3;
        // 格 (4,2):类型 0xFF(空)但帧非零——陈旧字节(46/540 真图形态)
        frames[4 + (2 << 9)] = 9;
        OverlayLayers::parse(&types, &frames).unwrap()
    }

    #[test]
    fn row_major_indexing_roundtrip() {
        let layers = layers();
        assert_eq!(layers.type_at(3, 2), 7);
        assert_eq!(layers.type_at(0, 0), 0xFF);
        assert_eq!(layers.frame_at(3, 2), Some(3));
    }

    #[test]
    fn stale_data_under_empty_type_is_ignored() {
        // type==0xFF 且 data≠0:.frame 恒 None(陈旧字节忽略,不报损坏)
        let layers = layers();
        assert_eq!(layers.type_at(4, 2), 0xFF);
        assert_eq!(layers.frame_at(4, 2), None);
    }

    #[test]
    fn out_of_range_cell_behaves_as_empty() {
        let layers = layers();
        assert_eq!(layers.type_at(600, 0), 0xFF);
        assert_eq!(layers.frame_at(600, 0), None);
    }

    #[test]
    fn rejects_short_types() {
        assert!(matches!(
            OverlayLayers::parse(&[0u8; OVERLAY_GRID_LEN - 1], &[0u8; OVERLAY_GRID_LEN]),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_short_frames() {
        assert!(matches!(
            OverlayLayers::parse(&[0u8; OVERLAY_GRID_LEN], &[0u8; 5]),
            Err(FormatError::Invalid(_))
        ));
    }
}
