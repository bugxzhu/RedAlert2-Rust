// SPDX-License-Identifier: GPL-3.0-only

//! 剧场瓦片索引层(M3):INI 表反查、剧场常数表、资产装载器。
//!
//! 出处:索引空间规则 = 裁决后研究 2026-11-05-theater-tile-mapping.md §3
//! (tile_id = 剧场 INI 枚举位;md 整体替换 base;六剧场容量);REA 勘误
//! (2026-11-05 控制器,Init_Secondary_Mixes 0x5301A0 / MixFile find
//! 0x5B4430 / Theater::Array VA 0x7E1B78):挂载序首中即胜、顺序探查、
//! LastTilesInSet 引擎规则、.ubn 扩展名与六份 md INI 名互证。

use crate::format::ini::{parse_int_prefix, IniDocument};
use crate::format::map::meta::Theater;
use crate::format::FormatError;

/// 剧场常数表项(研究 §3/§8;容量为 md INI 的 TilesInSet 运行时累加和)。
pub struct TheaterSpec {
    pub ini_name: &'static str,
    pub ext: &'static str,
    pub capacity: usize,
}

/// 目录名 → {生效 INI 名, 扩展名, 容量}(urbannmd 采 expandmd01 版,容量 1189)。
pub fn theater_spec(theater: Theater) -> TheaterSpec {
    match theater {
        Theater::Temperate => TheaterSpec {
            ini_name: "temperatmd.ini",
            ext: "tem",
            capacity: 838,
        },
        Theater::Urban => TheaterSpec {
            ini_name: "urbanmd.ini",
            ext: "urb",
            capacity: 1081,
        },
        Theater::Snow => TheaterSpec {
            ini_name: "snowmd.ini",
            ext: "sno",
            capacity: 964,
        },
        Theater::Desert => TheaterSpec {
            ini_name: "desertmd.ini",
            ext: "des",
            capacity: 726,
        },
        Theater::NewUrban => TheaterSpec {
            ini_name: "urbannmd.ini",
            ext: "ubn",
            capacity: 1189,
        },
        Theater::Lunar => TheaterSpec {
            ini_name: "lunarmd.ini",
            ext: "lun",
            capacity: 198,
        },
    }
}

/// 一个槽位的文件形态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotFile {
    /// FileName + %02d(1 基序数)+ 扩展名。
    File(String),
    /// blankFileName 且 TilesInSet>0 的保留槽(set0006 = [General] BLACKTILE
    /// 形态):占索引、无自有文件,语料从不引用(研究 §3)。
    Reserved,
}

/// 集合信息(set_of 返回;所有字段 owned,无借用)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheaterSetInfo {
    pub number: u16,
    /// 集内 [TileSetNNNN] 的 Name= 键(缺失为 None)。
    pub name: Option<String>,
    /// 该集首个全局槽位号。
    pub first_index: usize,
    pub tiles: usize,
}

#[derive(Debug, Clone)]
struct TheaterSet {
    number: u16,
    name: Option<String>,
    /// FileName 原文;None = blank 集。
    file_base: Option<String>,
    tiles: usize,
    first_index: usize,
}

/// 剧场 INI 瓦片索引表。
#[derive(Debug, Clone)]
pub struct TheaterIni {
    sets: Vec<TheaterSet>,
    slot_count: usize,
}

impl TheaterIni {
    /// 解析剧场瓦片 INI(引擎逐序探测语义,见上文语义要点)。
    pub fn parse(text: &str) -> Result<Self, FormatError> {
        let doc = IniDocument::parse(text);
        let mut sets = Vec::new();
        let mut slot_count = 0usize;
        let mut ordinal = 0usize;
        loop {
            let sec = format!("TileSet{ordinal:04}");
            let Some(section) = doc.section(&sec) else {
                break; // 首个缺失段即停(REA 勘误:引擎顺序探查)
            };
            let _ = section;
            let tiles_raw = doc.get_int(&sec, "TilesInSet", 0);
            let tiles = tiles_raw.clamp(0, 99) as usize;
            if let Some(lts_raw) = doc.value(&sec, "LastTilesInSet") {
                // 引擎按 LastTilesInSet 推进索引并映射 extras(REA 勘误);
                // 全语料零出现(依据 §3),死路不实现 → Unsupported
                let lts = parse_int_prefix(lts_raw).unwrap_or(0);
                if lts != tiles_raw {
                    return Err(FormatError::Unsupported(format!(
                        "{sec}: LastTilesInSet={lts_raw} != TilesInSet={tiles_raw}(引擎规则未实现,研究 §3)"
                    )));
                }
            }
            let file_base = doc
                .value(&sec, "FileName")
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_string);
            if file_base.is_some() || tiles > 0 {
                let name = doc.value(&sec, "Name").map(str::to_string);
                sets.push(TheaterSet {
                    number: ordinal as u16,
                    name,
                    file_base,
                    tiles,
                    first_index: slot_count,
                });
                slot_count += tiles;
            }
            ordinal += 1;
        }
        Ok(TheaterIni { sets, slot_count })
    }

    /// 全局槽位总数(= 六剧场容量常数;Task 8 与 theater_spec 互证)。
    pub fn slot_count(&self) -> usize {
        self.slot_count
    }

    /// 全局槽位 → 文件名;`None` = 界外。
    pub fn file_name(&self, index: usize, ext: &str) -> Option<SlotFile> {
        let set = self.sets.iter().rev().find(|s| s.first_index <= index)?;
        let ordinal = index - set.first_index;
        if ordinal >= set.tiles {
            return None;
        }
        match &set.file_base {
            Some(base) => Some(SlotFile::File(format!("{base}{:02}.{ext}", ordinal + 1))),
            None => Some(SlotFile::Reserved),
        }
    }

    /// 全局槽位 → 所属集合(owned;界外 None,含落入零 tiles 集阴影区的索引)。
    pub fn set_of(&self, index: usize) -> Option<TheaterSetInfo> {
        let set = self.sets.iter().rev().find(|s| s.first_index <= index)?;
        if index - set.first_index >= set.tiles {
            return None;
        }
        Some(TheaterSetInfo {
            number: set.number,
            name: set.name.clone(),
            first_index: set.first_index,
            tiles: set.tiles,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ini(text: &str) -> TheaterIni {
        TheaterIni::parse(text).unwrap()
    }

    #[test]
    fn sequential_probe_and_gap_terminates() {
        // TileSet0000..0002 连续贡献;0003 缺失即停;0004 不参与(REA 勘误)
        let t = ini("[TileSet0000]\nFileName=aaa\nTilesInSet=2\n\
             [TileSet0001]\nFileName=bbb\nTilesInSet=1\n\
             [TileSet0002]\nFileName=ccc\nTilesInSet=3\n\
             [TileSet0004]\nFileName=ddd\nTilesInSet=9\n");
        assert_eq!(t.slot_count(), 6);
        assert_eq!(
            t.file_name(5, "sno"),
            Some(SlotFile::File("ccc03.sno".into()))
        );
        assert_eq!(t.file_name(6, "sno"), None);
    }

    #[test]
    fn atoi_prefix_and_clamp() {
        // 出厂脏数据 TilesInSet=o → 0(研究 §3,localmd urbannmd set0105 实测);
        // >99 截 99(语料无,规则照研究);负值钳 0
        let t = ini("[TileSet0000]\nFileName=a\nTilesInSet=o\n\
             [TileSet0001]\nFileName=b\nTilesInSet=12x\n\
             [TileSet0002]\nFileName=c\nTilesInSet=150\n\
             [TileSet0003]\nFileName=d\nTilesInSet=-3\n");
        assert_eq!(t.slot_count(), 111); // 0 + 12 + 99(150 截)+ 0
        assert_eq!(t.set_of(0).unwrap().number, 1);
    }

    #[test]
    fn blank_zero_tiles_contribute_nothing() {
        let t = ini("[TileSet0000]\nFileName=aaa\nTilesInSet=2\n\
             [TileSet0001]\nTilesInSet=0\n\
             [TileSet0002]\nFileName=bbb\nTilesInSet=1\n");
        assert_eq!(t.slot_count(), 3);
        assert_eq!(
            t.file_name(2, "sno"),
            Some(SlotFile::File("bbb01.sno".into()))
        );
    }

    #[test]
    fn reserved_blank_set_occupies_slot() {
        // set0006 BLACKTILE 形态:blank + TilesInSet=1 → 占位,Reserved 无文件
        let t = ini("[TileSet0000]\nFileName=aaa\nTilesInSet=2\n\
             [TileSet0001]\nTilesInSet=1\n");
        assert_eq!(t.slot_count(), 3);
        assert_eq!(t.file_name(2, "sno"), Some(SlotFile::Reserved));
        assert!(t.file_name(3, "sno").is_none());
    }

    #[test]
    fn file_name_one_based_ordinals() {
        // 锚点反推:tem[837]=ariz01.tem 为 set0081 唯一文件 → 集内序数 1 基
        let t = ini("[TileSet0000]\nFileName=ariz\nTilesInSet=2\n");
        assert_eq!(
            t.file_name(0, "tem"),
            Some(SlotFile::File("ariz01.tem".into()))
        );
        assert_eq!(
            t.file_name(1, "tem"),
            Some(SlotFile::File("ariz02.tem".into()))
        );
    }

    #[test]
    fn set_of_returns_info() {
        // 顺序探测语义下 0081 单段不可达(0000 缺即停)——用 81 个无键占位段
        // 铺满 0000..0080(零贡献,同 missing_keys 路径),set0081 first_index=0
        let mut text = String::new();
        for i in 0..81 {
            text.push_str(&format!("[TileSet{i:04}]\n"));
        }
        text.push_str("[TileSet0081]\nName=USS Arizona\nFileName=ariz\nTilesInSet=1\n");
        let t = ini(&text);
        let s = t.set_of(0).unwrap();
        assert_eq!(s.number, 81);
        assert_eq!(s.name.as_deref(), Some("USS Arizona"));
        assert_eq!((s.first_index, s.tiles), (0, 1));
        assert!(t.set_of(1).is_none());
    }

    #[test]
    fn keys_and_sections_case_insensitive() {
        let t = ini("[tileset0000]\nfilename=AAA\ntilesinset=2\nname=Mixed\n");
        assert_eq!(t.slot_count(), 2);
        assert_eq!(
            t.file_name(0, "sno"),
            Some(SlotFile::File("AAA01.sno".into()))
        );
    }

    #[test]
    fn missing_keys_lenient() {
        // 缺 FileName/TilesInSet → 空集 0 贡献、探查继续(spec §5 宽容)
        let t = ini("[TileSet0000]\nName=Ghost\n\
             [TileSet0001]\nFileName=bbb\nTilesInSet=2\n");
        assert_eq!(t.slot_count(), 2);
        assert_eq!(t.set_of(0).unwrap().number, 1);
    }

    #[test]
    fn last_tiles_in_set_mismatch_is_unsupported() {
        // REA 勘误引擎规则:LastTilesInSet != TilesInSet → Unsupported(死路)
        let bad = TheaterIni::parse("[TileSet0000]\nFileName=a\nTilesInSet=3\nLastTilesInSet=5\n")
            .unwrap_err();
        assert!(matches!(bad, FormatError::Unsupported(_)));
        // 值相等或缺失 → 正常
        let ok = ini("[TileSet0000]\nFileName=a\nTilesInSet=3\nLastTilesInSet=3\n");
        assert_eq!(ok.slot_count(), 3);
    }

    #[test]
    fn theater_spec_table_six_theaters() {
        // 研究 §3/§8 容量常数 + 扩展名(REA Theater::Array 0x7E1B78 互证)
        let cases = [
            (Theater::Temperate, "temperatmd.ini", "tem", 838),
            (Theater::Urban, "urbanmd.ini", "urb", 1081),
            (Theater::Snow, "snowmd.ini", "sno", 964),
            (Theater::Desert, "desertmd.ini", "des", 726),
            (Theater::NewUrban, "urbannmd.ini", "ubn", 1189),
            (Theater::Lunar, "lunarmd.ini", "lun", 198),
        ];
        for (theater, ini_name, ext, cap) in cases {
            let s = theater_spec(theater);
            assert_eq!(s.ini_name, ini_name);
            assert_eq!(s.ext, ext);
            assert_eq!(s.capacity, cap);
        }
    }
}
