// SPDX-License-Identifier: GPL-3.0-only

//! 剧场瓦片索引层(M3):INI 表反查、剧场常数表、资产装载器。
//!
//! 出处:索引空间规则 = 裁决后研究 2026-11-05-theater-tile-mapping.md §3
//! (tile_id = 剧场 INI 枚举位;md 整体替换 base;六剧场容量);REA 勘误
//! (2026-11-05 控制器,Init_Secondary_Mixes 0x5301A0 / MixFile find
//! 0x5B4430 / Theater::Array VA 0x7E1B78):挂载序首中即胜、顺序探查、
//! LastTilesInSet 引擎规则、.ubn 扩展名与六份 md INI 名互证。
//!
//! 裁定补遗(2026-11-06,ra2-decomp isotype.cpp:695-708 + gamemd .data
//! 剧场记录表 0x3E1B80..0x3E1E80):缺 TilesInSet 键 = Get_Int 默认 −1
//! = 停探查(与缺段同线);LastTilesInSet ≠ TilesInSet 为瓦片插入重映射
//! 指令(Count/Offset 簿记,tile_count += last),−1 视为无;NewUrban 的
//! 挂载 token 为 URBANN(0x3E1D68)→ urbann.mix 属引擎挂载面(短 ISO token
//! ISOTEM/ISOSNO 系 DOS 8.3 限长所致)。

use crate::format::crc::mix_name_id;
use crate::format::ini::{parse_int_prefix, IniDocument};
use crate::format::map::meta::Theater;
use crate::format::mix::{MixArchive, MixEntry};
use crate::format::FormatError;
use std::collections::BTreeMap;

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
            if doc.section(&sec).is_none() {
                break; // 首个缺失段即停(REA 勘误:引擎顺序探查)
            }
            // 引擎 Get_Int(section,"TilesInSet",-1):缺键或字面 −1 → 停探查
            // (isotype.cpp:695-698);键在值脏 → atoi 0 → 继续。我们的
            // IniDocument::get_int 会把"值脏"也归默认值,故此处必须用
            // value()+parse_int_prefix 直写,以区分两条路径。
            let Some(tiles_str) = doc.value(&sec, "TilesInSet") else {
                break; // 缺键 = Get_Int 默认 −1 = 与缺段同线停
            };
            let tiles_raw = parse_int_prefix(tiles_str).unwrap_or(0); // 引擎 atoi:键在值脏 → 0
            if tiles_raw == -1 {
                break; // 字面 −1(含 "-1x" 等 atoi 前缀)同线停
            }
            // 引擎直加原值(可为负倒退/超 99);此处夹紧为有意偏差:
            // 敌意输入护栏,语料实测全 0..99,该分支语料死路。
            let tiles = tiles_raw.clamp(0, 99) as usize;
            if let Some(lts_raw) = doc.value(&sec, "LastTilesInSet") {
                // 引擎语义:LastTilesInSet ≠ TilesInSet = 瓦片插入重映射指令
                // (Count=tile_count+last,Offset=tiles−last,tile_count+=last;
                // isotype.cpp:700-708),非数据错误;−1(含 Get_Int 缺省路径)
                // 视为无。重映射未实现 → Unsupported 守卫,全语料零出现。
                let lts = parse_int_prefix(lts_raw).unwrap_or(0);
                if lts != -1 && lts != tiles_raw {
                    return Err(FormatError::Unsupported(format!(
                        "{sec}: LastTilesInSet={lts_raw} != TilesInSet={tiles_str}(瓦片插入重映射未实现,isotype.cpp:700-708)"
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

// ============ 装载器层(TheaterAssets;引擎挂载序,首中即胜)============

/// 嵌套下钻体量上限(spec"体量上限防炸";语料最大嵌套容器约 35.7 MB)。
pub const MAX_DESCEND_BYTES: usize = 256 * 1024 * 1024;

/// 嵌套深度上限(spec §4"深度 ≤3";顶层挂载为深度 0)。
const MAX_DEPTH: usize = 3;

/// 每剧场 mix 名单(研究表 1 语料观测;挂载序在 ecache/elocal 之后,
/// 列表内顺序仅为确定性)。localmd.mix/local.mix 的拦截名另有专位。
/// urbann.mix(2026-11-06 裁定复位):gamemd .data 剧场记录表
/// 0x3E1B80..0x3E1E80 的 NEWURBAN 记录挂载 token = URBANN(0x3E1D68),
/// 即引擎会拼出 urbann.mix 并在语料 ra2md 内命中(实测 2,692,234 B 嵌套);
/// urbannmd.mix 为 md 侧运行时拼接名(URBANN+"md"),语料缺货 = 无害占位,
/// 保留。短 ISO token(ISOTEM/ISOSNO/ISOUBN)系 DOS 8.3 限长。
const THEATER_MIX_NAMES: [&str; 20] = [
    "isogen.mix",
    "isotemp.mix",
    "isosnow.mix",
    "isourb.mix",
    "temperat.mix",
    "urban.mix",
    "snow.mix",
    "isodes.mix",
    "isolun.mix",
    "isoubn.mix",
    "urbann.mix",
    "urbannmd.mix",
    "isogenmd.mix",
    "isotemmd.mix",
    "isourbmd.mix",
    "isosnomd.mix",
    "isodesmd.mix",
    "isolunmd.mix",
    "isoubnmd.mix",
    "snowmd.mix",
];

/// 一个已挂载容器(数据 + 解析后的索引表)。
struct Mounted {
    label: String,
    data: Vec<u8>,
    data_start: usize,
    entries: Vec<MixEntry>,
    pos: usize,
    depth: usize,
    seq: usize,
}

/// id → 数据索引项(start/len 为容器 data 内绝对区间)。
struct IndexEntry {
    id: u32,
    container: usize,
    start: usize,
    len: usize,
}

/// 剧场资产全集(引擎挂载序装载;查询首中即胜)。
pub struct TheaterAssets {
    containers: Vec<Mounted>,
    index: Vec<IndexEntry>,
}

/// 便宜前置判定(避免对每个数据条目做全量 MIX 解析;真正校验仍由
/// MixArchive::parse 完成)。
fn mix_preflight(blob: &[u8]) -> bool {
    if blob.len() < 10 {
        return false;
    }
    let flags = u32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]);
    let count = u16::from_le_bytes([blob[4], blob[5]]);
    if flags & 0x0002_0000 != 0 || flags & !0x0003_0000 != 0 {
        return false;
    }
    10 + 12 * count as usize <= blob.len()
}

/// 先序下钻:容器自身先入册,再递归嵌套(拦截名单命中者截流到
/// holdback_hits,由 open 在其专位挂载)。装载器非解析热路径,克隆为清晰让路。
#[allow(clippy::too_many_arguments)] // 冷路径/草图既定形(8 参,默认阈值 7;偏差记录已录)
fn walk(
    data: Vec<u8>,
    label: String,
    pos: usize,
    depth: usize,
    seq: &mut usize,
    out: &mut Vec<Mounted>,
    holdback_hits: &mut BTreeMap<&'static str, Vec<u8>>,
    holdback_ids: &BTreeMap<u32, &'static str>,
) {
    let mix = match MixArchive::parse(data.clone()) {
        Ok(m) => m,
        Err(_) => return,
    };
    let data_start = mix.data_start();
    let entries: Vec<MixEntry> = mix.entries().to_vec();
    let s = *seq;
    *seq += 1;
    let owned = data.clone(); // 偏差记录:直白写法,替代 out.last() 二次克隆
    out.push(Mounted {
        label: label.clone(), // 偏差记录:子标签仍需原 label(差 2 裁定;格式 name#HEXID8)
        data,
        data_start,
        entries: entries.clone(),
        pos,
        depth,
        seq: s,
    });
    if depth >= MAX_DEPTH {
        return;
    }
    for e in &entries {
        let start = data_start + e.offset as usize;
        let end = start + e.size as usize;
        if end > owned.len() {
            continue;
        }
        let blob = &owned[start..end];
        if blob.len() > MAX_DESCEND_BYTES || !mix_preflight(blob) {
            continue;
        }
        if let Some(&name) = holdback_ids.get(&e.id) {
            holdback_hits.entry(name).or_insert_with(|| blob.to_vec());
            continue;
        }
        walk(
            blob.to_vec(),
            format!("{label}#{:08X}", e.id),
            pos,
            depth + 1,
            seq,
            out,
            holdback_hits,
            holdback_ids,
        );
    }
}

impl TheaterAssets {
    /// 按引擎挂载序装载(spec §4 + REA 勘误;见模块注释)。
    pub fn open(assets_dir: &std::path::Path) -> Result<Self, FormatError> {
        let mut roots: Vec<(usize, String, Vec<u8>)> = Vec::new();
        // expandmd99..expandmd00(pos = 99−nn,高位先)
        for nn in (0..=99u32).rev() {
            let name = format!("expandmd{nn:02}.mix");
            if let Ok(data) = std::fs::read(assets_dir.join(&name)) {
                roots.push((99 - nn as usize, name, data));
            }
        }
        // ra2md / ra2(必需)
        for (pos, name) in [(100usize, "ra2md.mix"), (101, "ra2.mix")] {
            let data = std::fs::read(assets_dir.join(name))
                .map_err(|e| FormatError::Invalid(format!("必需资产 {name} 读取失败: {e}")))?;
            roots.push((pos, name.to_string(), data));
        }
        // cachemd / cache(可选,缺席即跳)
        for (pos, name) in [(102usize, "cachemd.mix"), (103, "cache.mix")] {
            if let Ok(data) = std::fs::read(assets_dir.join(name)) {
                roots.push((pos, name.to_string(), data));
            }
        }
        // 根目录 ecache*/elocal*(文件名升序)
        let mut ecaches: Vec<String> = std::fs::read_dir(assets_dir)
            .map_err(|e| FormatError::Invalid(format!("资产目录不可读: {e}")))?
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".mix") && (n.starts_with("ecache") || n.starts_with("elocal")))
            .collect();
        ecaches.sort();
        for (i, name) in ecaches.iter().enumerate() {
            if let Ok(data) = std::fs::read(assets_dir.join(name)) {
                roots.push((106 + i, name.clone(), data));
            }
        }
        // 拦截名单(localmd/local/每剧场):id → 名
        let mut holdback_ids: BTreeMap<u32, &'static str> = BTreeMap::new();
        holdback_ids.insert(mix_name_id("localmd.mix"), "localmd.mix");
        holdback_ids.insert(mix_name_id("local.mix"), "local.mix");
        for name in THEATER_MIX_NAMES {
            holdback_ids.insert(mix_name_id(name), name);
        }
        // 先序下钻全部挂载根
        let mut out: Vec<Mounted> = Vec::new();
        let mut holdback_hits: BTreeMap<&'static str, Vec<u8>> = BTreeMap::new();
        let mut seq = 0usize;
        for (pos, name, data) in roots {
            walk(
                data,
                name,
                pos,
                0,
                &mut seq,
                &mut out,
                &mut holdback_hits,
                &holdback_ids,
            );
        }
        // 拦截名按自身挂载位回挂(localmd 104 / local 105 / 每剧场 1000+)
        for (pos, name) in [(104usize, "localmd.mix"), (105, "local.mix")] {
            if let Some(data) = holdback_hits.remove(name) {
                walk(
                    data,
                    name.to_string(),
                    pos,
                    0,
                    &mut seq,
                    &mut out,
                    &mut holdback_hits,
                    &holdback_ids,
                );
            }
        }
        for (i, name) in THEATER_MIX_NAMES.iter().enumerate() {
            if let Some(data) = holdback_hits.remove(name) {
                walk(
                    data,
                    name.to_string(),
                    1000 + i,
                    0,
                    &mut seq,
                    &mut out,
                    &mut holdback_hits,
                    &holdback_ids,
                );
            }
        }
        // (pos, depth, seq) 排序 → 顺序收集 → 按 id 稳定排序 → 每组取首个
        out.sort_by_key(|r| (r.pos, r.depth, r.seq));
        let mut candidates: Vec<(u32, usize, usize, usize)> = Vec::new();
        for (ci, rec) in out.iter().enumerate() {
            for e in &rec.entries {
                candidates.push((
                    e.id,
                    ci,
                    rec.data_start + e.offset as usize,
                    e.size as usize,
                ));
            }
        }
        candidates.sort_by_key(|c| c.0); // 稳定:同 id 保装载序首个 = 首中即胜
        let mut index: Vec<IndexEntry> = Vec::new();
        for (id, container, start, len) in candidates {
            if index.last().map(|last| last.id) == Some(id) {
                continue;
            }
            index.push(IndexEntry {
                id,
                container,
                start,
                len,
            });
        }
        Ok(TheaterAssets {
            containers: out,
            index,
        })
    }

    fn slot_of(&self, id: u32) -> Option<(usize, &IndexEntry)> {
        let i = self.index.binary_search_by_key(&id, |e| e.id).ok()?;
        Some((self.index[i].container, &self.index[i]))
    }

    /// 按名查找(文件名大小写/斜杠规则同 mix_name_id)。
    pub fn lookup(&self, name: &str) -> Option<&[u8]> {
        self.lookup_id(mix_name_id(name))
    }

    /// 按 id 直查(匿名嵌套库等)。
    pub fn lookup_id(&self, id: u32) -> Option<&[u8]> {
        let (c, e) = self.slot_of(id)?;
        Some(&self.containers[c].data[e.start..e.start + e.len])
    }

    /// 诊断:命中容器标签(如 ra2.mix / ra2.mix#0F5D1D99)。
    pub fn origin(&self, name: &str) -> Option<&str> {
        let (c, _) = self.slot_of(mix_name_id(name))?;
        Some(self.containers[c].label.as_str())
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
        // 键在值脏(TilesInSet=o → atoi 0)→ 继续不吹(引擎 Get_Int 只在
        // 缺键/−1 时停,isotype.cpp:695-698;localmd urbannmd set0105 实测);
        // 夹紧为有意偏差:引擎直加原值(150/−3 均原样入账),语料全 0..99 死路
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
        // 顺序探测语义下 0081 单段不可达(0000 缺即停)——用 81 个零贡献段
        // 铺满 0000..0080(须带 TilesInSet=0:缺键即停,引擎 Get_Int −1 语义),
        // set0081 first_index=0
        let mut text = String::new();
        for i in 0..81 {
            text.push_str(&format!("[TileSet{i:04}]\nTilesInSet=0\n"));
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
    fn missing_tiles_in_set_stops_probe() {
        // 引擎 Get_Int(section,"TilesInSet",-1):缺键 → −1 → 停探查
        // (isotype.cpp:695-698),与缺段同线;TileSet0001 不参与
        let t = ini("[TileSet0000]\nName=Ghost\n\
             [TileSet0001]\nFileName=bbb\nTilesInSet=2\n");
        assert_eq!(t.slot_count(), 0);
        assert!(t.set_of(0).is_none());
        // 字面 −1(及 atoi 前缀 "-1x")同线停
        for raw in ["-1", "-1x"] {
            let t = ini(&format!(
                "[TileSet0000]\nFileName=a\nTilesInSet={raw}\n\
             [TileSet0001]\nFileName=b\nTilesInSet=2\n"
            ));
            assert_eq!(t.slot_count(), 0, "TilesInSet={raw} 应停探查");
        }
    }

    #[test]
    fn last_tiles_in_set_mismatch_is_unsupported() {
        // 引擎语义:LastTilesInSet != TilesInSet = 瓦片插入重映射指令
        // (Count=tile_count+last,Offset=tiles−last,isotype.cpp:700-708),
        // 非数据错误;重映射未实现 → Unsupported 守卫
        let bad = TheaterIni::parse("[TileSet0000]\nFileName=a\nTilesInSet=3\nLastTilesInSet=5\n")
            .unwrap_err();
        assert!(matches!(bad, FormatError::Unsupported(_)));
        // 值相等、缺失、或 −1(Get_Int 默认语义,视为无)→ 正常
        let ok = ini("[TileSet0000]\nFileName=a\nTilesInSet=3\nLastTilesInSet=3\n");
        assert_eq!(ok.slot_count(), 3);
        let neg = ini("[TileSet0000]\nFileName=a\nTilesInSet=3\nLastTilesInSet=-1\n");
        assert_eq!(neg.slot_count(), 3);
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

    // —— 装载器单测辅助(临时目录;tag 隔离并行测试)——
    fn temp_assets(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ra2-theater-{tag}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// 构造 flags=0 的最小合法 MIX(按名条目)。
    fn make_mix(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        let body_len: usize = entries.iter().map(|(_, c)| c.len()).sum();
        v.extend_from_slice(&(body_len as u32).to_le_bytes());
        let mut off = 0u32;
        for (name, c) in entries {
            v.extend_from_slice(&mix_name_id(name).to_le_bytes());
            v.extend_from_slice(&off.to_le_bytes());
            v.extend_from_slice(&(c.len() as u32).to_le_bytes());
            off += c.len() as u32;
        }
        for (_, c) in entries {
            v.extend_from_slice(c);
        }
        v
    }

    /// 同 make_mix,但 id 由调用方给定(匿名嵌套 mix 用)。
    fn make_mix_raw(entries: &[(u32, &[u8])]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        let body_len: usize = entries.iter().map(|(_, c)| c.len()).sum();
        v.extend_from_slice(&(body_len as u32).to_le_bytes());
        let mut off = 0u32;
        for (id, c) in entries {
            v.extend_from_slice(&id.to_le_bytes());
            v.extend_from_slice(&off.to_le_bytes());
            v.extend_from_slice(&(c.len() as u32).to_le_bytes());
            off += c.len() as u32;
        }
        for (_, c) in entries {
            v.extend_from_slice(c);
        }
        v
    }

    fn put(dir: &std::path::Path, name: &str, bytes: &[u8]) {
        std::fs::write(dir.join(name), bytes).unwrap();
    }

    #[test]
    fn first_match_wins_expand_over_ra2md_over_ra2() {
        // REA 勘误:首中即胜,挂载序 expandmd* > ra2md > ra2
        let d = temp_assets("first-match");
        put(&d, "ra2.mix", &make_mix(&[("probe.dat", b"from-ra2")]));
        put(&d, "ra2md.mix", &make_mix(&[("probe.dat", b"from-ra2md")]));
        put(
            &d,
            "expandmd01.mix",
            &make_mix(&[("probe.dat", b"from-expand")]),
        );
        let a = TheaterAssets::open(&d).unwrap();
        assert_eq!(a.lookup("probe.dat").unwrap(), b"from-expand");
        assert_eq!(a.origin("probe.dat"), Some("expandmd01.mix"));
    }

    #[test]
    fn cachemd_beats_localmd() {
        // cachemd(pos 102)> localmd(pos 104,自 ra2md 条目拦截回挂)
        let d = temp_assets("cachemd");
        let localmd_blob = make_mix(&[("probe.dat", b"from-localmd")]);
        put(&d, "ra2.mix", &make_mix(&[]));
        put(
            &d,
            "ra2md.mix",
            &make_mix(&[("localmd.mix", &localmd_blob)]),
        );
        put(
            &d,
            "cachemd.mix",
            &make_mix(&[("probe.dat", b"from-cachemd")]),
        );
        let a = TheaterAssets::open(&d).unwrap();
        assert_eq!(a.lookup("probe.dat").unwrap(), b"from-cachemd");
    }

    #[test]
    fn anonymous_nested_mix_descends() {
        // expandmd01 匿名嵌套 #C31B4D61 的必需路径缩小版
        let d = temp_assets("anon-nested");
        let inner = make_mix(&[("deep.dat", b"nested")]);
        put(&d, "ra2.mix", &make_mix_raw(&[(0xC31B_4D61, &inner)]));
        put(&d, "ra2md.mix", &make_mix(&[]));
        let a = TheaterAssets::open(&d).unwrap();
        assert_eq!(a.lookup("deep.dat").unwrap(), b"nested");
        assert!(a.origin("deep.dat").unwrap().contains("C31B4D61"));
    }

    #[test]
    fn depth_guard_stops_at_three() {
        // ra2(0) → m1(1) → m2(2) → m3(3) → m4(4 超限不下钻):d3 可查,d4 不可查
        let d = temp_assets("depth");
        let m4 = make_mix(&[("d4.dat", b"x4")]);
        let m3 = make_mix_raw(&[(0x1111_1111, &m4), (mix_name_id("d3.dat"), b"x3")]);
        let m2 = make_mix_raw(&[(0x2222_2222, &m3)]);
        let m1 = make_mix_raw(&[(0x3333_3333, &m2)]);
        put(&d, "ra2.mix", &make_mix_raw(&[(0x4444_4444, &m1)]));
        put(&d, "ra2md.mix", &make_mix(&[]));
        let a = TheaterAssets::open(&d).unwrap();
        assert_eq!(a.lookup("d3.dat").unwrap(), b"x3");
        assert!(a.lookup("d4.dat").is_none());
    }

    #[test]
    fn missing_required_root_is_err() {
        let d = temp_assets("missing-root");
        put(&d, "ra2.mix", &make_mix(&[])); // 缺 ra2md.mix → Invalid
        assert!(matches!(
            TheaterAssets::open(&d),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn max_descend_bytes_const_pinned() {
        // 语料最大嵌套容器约 35.7 MB;256 MiB 为防炸护栏(依据 §1)
        assert_eq!(MAX_DESCEND_BYTES, 268_435_456);
    }
}
