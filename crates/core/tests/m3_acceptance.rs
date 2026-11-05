// SPDX-License-Identifier: GPL-3.0-only
//! M3 里程碑验收:807 图全量 (theater, tile_id, sub) → 文件闭合 + 缺文件槽
//! 精确账 + 六剧场现存瓦片全量零 panic + 甲骨文锚点。
//! 运行:RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test m3_acceptance -- --ignored

use ra2_core::format::map::mapdoc::decode_map_text;
use ra2_core::format::map::meta::Theater;
use ra2_core::format::map::MapFile;
use ra2_core::format::mix::MixArchive;
use ra2_core::format::theater::{theater_spec, SlotFile, TheaterAssets, TheaterIni};
use ra2_core::format::theater_tmp::TsTmpFile;
use std::collections::BTreeMap;

fn assets_dir() -> std::path::PathBuf {
    std::env::var("RA2_ASSETS_DIR")
        .expect("需要环境变量 RA2_ASSETS_DIR")
        .into()
}

/// 递归收集 RA2_ASSETS_DIR 下全部 .map(与 map_samples 同构;测试代码复制,不共享)。
fn map_files() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![assets_dir()];
    while let Some(dir) = stack.pop() {
        let entries =
            std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("读目录 {}: {e}", dir.display()));
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().map(|x| x == "map").unwrap_or(false) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn theater_index(t: Theater) -> usize {
    match t {
        Theater::Temperate => 0,
        Theater::Snow => 1,
        Theater::Urban => 2,
        Theater::Desert => 3,
        Theater::NewUrban => 4,
        Theater::Lunar => 5,
    }
}

/// 剧场上下文:全槽位文件状态(INI 全枚举)。
enum SlotState {
    Reserved,
    File(String),
}

fn theater_slots(assets: &TheaterAssets, theater: Theater) -> Vec<SlotState> {
    let spec = theater_spec(theater);
    let ini_bytes = assets.lookup(spec.ini_name).expect("剧场 INI 可达");
    let ini = TheaterIni::parse(&decode_map_text(ini_bytes)).expect("剧场 INI 可解析");
    assert_eq!(ini.slot_count(), spec.capacity, "{}", spec.ini_name);
    let mut files = Vec::with_capacity(ini.slot_count());
    for idx in 0..ini.slot_count() {
        files.push(match ini.file_name(idx, spec.ext) {
            Some(SlotFile::File(name)) => SlotState::File(name),
            Some(SlotFile::Reserved) => SlotState::Reserved,
            None => panic!("界内槽位 {idx} 返回 None(不可能)"),
        });
    }
    files
}

/// 807 图全量闭合(spec §6-1)。四类违例必须为零:
/// A 界外 / B 引用未出货槽 / C 引用保留槽 / E 现存文件解析失败;
/// 类 D(sub ≥ cbx·cby)仅允许 MadHQ 机生成图白名单(研究 §3),
/// 界 ≤4 图 / ≤500 槽为防增长宽裕界;首跑实测(2026-11-05,807 图):
/// d_maps = 0 图 / d_total = 0 槽——语料零 sub 违例,白名单实际为空,
/// ≤4/≤500 纯为防增长界。
#[test]
#[ignore]
fn m3_full_closure_807_maps() {
    let assets = TheaterAssets::open(&assets_dir()).unwrap();
    let theaters = [
        Theater::Temperate,
        Theater::Snow,
        Theater::Urban,
        Theater::Desert,
        Theater::NewUrban,
        Theater::Lunar,
    ];
    // 每剧场:槽位文件状态 + 现存文件 → cbx·cby 解析缓存
    let mut slots: Vec<Vec<SlotState>> = Vec::with_capacity(6);
    let mut slot_counts: Vec<BTreeMap<String, u32>> = Vec::with_capacity(6);
    for t in theaters {
        slots.push(theater_slots(&assets, t));
        slot_counts.push(BTreeMap::new());
    }
    let mut d_maps: BTreeMap<String, usize> = BTreeMap::new();
    let mut d_total = 0usize;
    let mut total_maps = 0usize;
    for p in map_files() {
        total_maps += 1;
        let data = std::fs::read(&p).unwrap();
        let m = MapFile::parse(&data).unwrap_or_else(|e| panic!("{}: {e:?}", p.display()));
        let ti = theater_index(m.map_meta.theater);
        for r in m.tiles.records() {
            if r.tile < 0 {
                continue;
            }
            let idx = r.tile as usize;
            if idx >= slots[ti].len() {
                panic!(
                    "类 A: {} tile_id {idx} 越界(槽位 {})",
                    p.display(),
                    slots[ti].len()
                );
            }
            let name = match &slots[ti][idx] {
                SlotState::Reserved => {
                    panic!("类 C: {} 引用保留槽 {idx}", p.display());
                }
                SlotState::File(name) => name,
            };
            let blob = match assets.lookup(name) {
                Some(b) => b,
                None => panic!("类 B: {} 引用未出货槽 {idx}({name})", p.display()),
            };
            let counts = slot_counts[ti].entry(name.clone()).or_insert_with(|| {
                match TsTmpFile::parse(blob) {
                    Ok(t) => t.slot_count() as u32,
                    Err(e) => panic!("类 E: {} 解析 {name}: {e:?}", p.display()),
                }
            });
            if (r.sub_tile as u32) >= *counts {
                let key = p.display().to_string();
                assert!(
                    key.contains("MadHQ"),
                    "类 D 非 MadHQ 图 sub 违例: {key} tile {idx} sub {}",
                    r.sub_tile
                );
                *d_maps.entry(key).or_insert(0) += 1;
                d_total += 1;
            }
        }
    }
    assert_eq!(total_maps, 807, "语料 807 图(变动时同步更新)");
    assert!(
        d_maps.len() <= 4,
        "MadHQ 残余白名单 ≤4 图,实得 {} 张: {d_maps:?}",
        d_maps.len()
    );
    assert!(
        d_total <= 500,
        "sub 残余违例总数 ≤500(防增长界),实得 {d_total}"
    );
}

/// 缺文件槽精确账(spec §6-1,研究 §8):按 INI 全枚举(非仅引用),
/// File(name) 而资产查无的槽位计数 = TEMPERATE 0 / SNOW 78 / URBAN 0 /
/// DESERT 0 / NEWURBAN 383 / LUNAR 44;SNOW 缺失槽全部 ≥838
/// (ZMM Cliff Ice 系,研究 §3)。语料零引用已由闭合测试类 B 保证。
#[test]
#[ignore]
fn m3_unshipped_slot_census_exact() {
    let assets = TheaterAssets::open(&assets_dir()).unwrap();
    for (theater, missing_want) in [
        (Theater::Temperate, 0usize),
        (Theater::Snow, 78),
        (Theater::Urban, 0),
        (Theater::Desert, 0),
        (Theater::NewUrban, 383),
        (Theater::Lunar, 44),
    ] {
        let spec = theater_spec(theater);
        let slots = theater_slots(&assets, theater);
        let mut missing = 0usize;
        let mut missing_min = usize::MAX;
        for (idx, s) in slots.iter().enumerate() {
            if let SlotState::File(name) = s {
                if assets.lookup(name).is_none() {
                    missing += 1;
                    missing_min = missing_min.min(idx);
                }
            }
        }
        assert_eq!(missing, missing_want, "{} 缺文件槽", spec.ini_name);
        if theater == Theater::Snow && missing > 0 {
            assert!(missing_min >= 838, "SNOW 缺失槽应 ≥838,首例 {missing_min}");
        }
    }
}

/// 六剧场现存 TS TMP 全量零 panic(spec §6-2)+ snow 分布精确账
/// (研究 §3-5;INI 槽位反查口径:isosnow 459 + expandmd01#C31B4D61 323 +
/// isosnomd 88 + snow.mix 16 = 886 现存 = 964 − 78 缺失)。
/// 注意:容器结构判别(parse Ok 计数)口径**不可用**——isosnow 共 1013 条
/// 含非瓦片精灵(2026-11-05 探针裁定)。
/// 分布失配时:只读探针重测、更新常数、commit 注明,不得静默放宽(M2 惯例)。
#[test]
#[ignore]
fn m3_existing_tiles_parse_zero_panic_and_snow_distribution() {
    fn load(name: &str) -> MixArchive {
        let p = assets_dir().join(name);
        let data = std::fs::read(&p).unwrap_or_else(|e| panic!("读取 {}: {e}", p.display()));
        MixArchive::parse(data).expect("解析 MIX")
    }
    let ra2 = load("ra2.mix");
    let ra2md = load("ra2md.mix");
    let expand = load("expandmd01.mix");
    let isosnow = MixArchive::parse(ra2.read("isosnow.mix").unwrap().to_vec()).unwrap();
    let snow_mix = MixArchive::parse(ra2.read("snow.mix").unwrap().to_vec()).unwrap();
    let isosnomd = MixArchive::parse(ra2md.read("isosnomd.mix").unwrap().to_vec()).unwrap();
    let anon = MixArchive::parse(expand.read_by_id(0xC31B_4D61).unwrap().to_vec()).unwrap();
    // snow 分布精确账(研究 §3-5;**INI 槽位反查口径**,2026-11-05 探针裁定:
    // 964−78 缺失 = 886 现存 = isosnow 459 + expandmd01#C31B4D61 323 +
    // isosnomd 88 + snow.mix 16。容器结构判别口径会数错——isosnow 共 1013 条
    // 含非瓦片精灵——禁止用"容器内 parse Ok 计数"替代)
    let assets = TheaterAssets::open(&assets_dir()).unwrap();
    let snow_ini =
        TheaterIni::parse(&decode_map_text(assets.lookup("snowmd.ini").unwrap())).unwrap();
    let mut by_origin: BTreeMap<String, usize> = BTreeMap::new();
    let mut missing = 0usize;
    for idx in 0..snow_ini.slot_count() {
        if let Some(SlotFile::File(name)) = snow_ini.file_name(idx, "sno") {
            if assets.lookup(&name).is_some() {
                let o = assets.origin(&name).unwrap().to_string();
                *by_origin.entry(o).or_insert(0) += 1;
            } else {
                missing += 1;
            }
        }
    }
    assert_eq!(missing, 78, "snow 缺文件槽(与缺槽精确账测试互证)");
    assert_eq!(
        by_origin.get("isosnow.mix"),
        Some(&459),
        "分布键: {by_origin:?}"
    );
    assert_eq!(by_origin.get("isosnomd.mix"), Some(&88));
    assert_eq!(by_origin.get("snow.mix"), Some(&16));
    let anon_n = by_origin
        .iter()
        .find(|(k, _)| k.starts_with("expandmd01.mix#"))
        .map(|(_, &v)| v)
        .unwrap_or(0);
    assert_eq!(anon_n, 323, "匿名嵌套 mix 分布键: {by_origin:?}");
    assert_eq!(by_origin.values().sum::<usize>(), 886);
    // snow 四容器全量零 panic(分布口径之外的结构层兜底)
    for mix in [&isosnow, &snow_mix, &isosnomd, &anon] {
        for e in mix.entries() {
            let _ = TsTmpFile::parse(mix.read_by_id(e.id).unwrap());
        }
    }
    // clear01.sno 字节闭合(spec §6-2 抽样):1872 = 16+4·1+52+900+900
    let clear = TsTmpFile::parse(isosnow.read("clear01.sno").unwrap()).unwrap();
    assert_eq!(clear.dims(), (1, 1, 60, 30));
    assert_eq!(clear.slot_count(), 1);
    let s = clear.slot(0).unwrap();
    assert_eq!(s.diamond_pixels().len(), 900);
    assert_eq!(s.z_data().map(|z| z.len()), Some(900));
    // 其余剧场容器全量尝试解析:只求零 panic(Err = SHP 精灵等非瓦片,合法)
    // (偏差记录:切片类型标注——两组容器名单长度 5/10 不同,数组元组类型不齐)
    let other_theaters: [(&MixArchive, &[&str]); 2] = [
        (
            &ra2,
            &[
                "isogen.mix",
                "isotemp.mix",
                "isourb.mix",
                "temperat.mix",
                "urban.mix",
            ],
        ),
        (
            &ra2md,
            &[
                "isodes.mix",
                "isolun.mix",
                "isoubn.mix",
                "urbann.mix",
                "isogenmd.mix",
                "isotemmd.mix",
                "isourbmd.mix",
                "isodesmd.mix",
                "isolunmd.mix",
                "isoubnmd.mix",
            ],
        ),
    ];
    for (mix, names) in other_theaters {
        for name in names {
            let m = MixArchive::parse(mix.read(name).unwrap().to_vec()).unwrap();
            for e in m.entries() {
                let _ = TsTmpFile::parse(m.read_by_id(e.id).unwrap());
            }
        }
    }
}

/// 甲骨文(spec §6-4):六容量经装载器互证(含 urbannmd 取 expandmd01 版)、
/// 四锚点反查 + 文件现存、[0]=clear01、md 替换(base snow.ini 798 < md 964)。
/// burnt01/lobrdb08 帧偏移锚点在 format_samples(Task 2 迁移)已钉,不重复。
#[test]
#[ignore]
fn m3_oracle_anchors() {
    let assets = TheaterAssets::open(&assets_dir()).unwrap();
    for (theater, cap) in [
        (Theater::Temperate, 838usize),
        (Theater::Snow, 964),
        (Theater::Urban, 1081),
        (Theater::Desert, 726),
        (Theater::NewUrban, 1189),
        (Theater::Lunar, 198),
    ] {
        let spec = theater_spec(theater);
        let ini =
            TheaterIni::parse(&decode_map_text(assets.lookup(spec.ini_name).unwrap())).unwrap();
        assert_eq!(ini.slot_count(), cap, "{}", spec.ini_name);
    }
    // urbannmd 双版本:装载器首中即胜取 expandmd01(36,725 B > localmd 32,614 B)
    assert_eq!(assets.lookup("urbannmd.ini").unwrap().len(), 36_725);
    // 四锚点 + [0]=Clear01
    let snow = TheaterIni::parse(&decode_map_text(assets.lookup("snowmd.ini").unwrap())).unwrap();
    assert_eq!(
        snow.file_name(963, "sno"),
        Some(SlotFile::File("clifxl04.sno".into()))
    );
    assert!(assets.lookup("clifxl04.sno").is_some());
    assert_eq!(snow.set_of(963).unwrap().number, 82);
    // [0] 锚:INI FileName 原文保留大小写(探针:snow[0]=Clear01.sno)
    assert_eq!(
        snow.file_name(0, "sno"),
        Some(SlotFile::File("Clear01.sno".into()))
    );
    let nu = TheaterIni::parse(&decode_map_text(assets.lookup("urbannmd.ini").unwrap())).unwrap();
    assert_eq!(
        nu.file_name(1174, "ubn"),
        Some(SlotFile::File("sfbrdg01.ubn".into()))
    );
    assert!(assets.lookup("sfbrdg01.ubn").is_some());
    let tem =
        TheaterIni::parse(&decode_map_text(assets.lookup("temperatmd.ini").unwrap())).unwrap();
    assert_eq!(
        tem.file_name(837, "tem"),
        Some(SlotFile::File("ariz01.tem".into()))
    );
    assert!(assets.lookup("ariz01.tem").is_some());
    assert_eq!(tem.set_of(837).unwrap().number, 81);
    let lun = TheaterIni::parse(&decode_map_text(assets.lookup("lunarmd.ini").unwrap())).unwrap();
    assert_eq!(
        lun.file_name(197, "lun"),
        Some(SlotFile::File("Crevas06.lun".into()))
    );
    assert!(assets.lookup("Crevas06.lun").is_some());
    assert_eq!(lun.set_of(197).unwrap().number, 84);
    // md 整体替换 base:base snow.ini(ra2 → local)容量 798 < md 964(研究 §3-2)
    let base = TheaterIni::parse(&decode_map_text(assets.lookup("snow.ini").unwrap())).unwrap();
    assert_eq!(base.slot_count(), 798);
    assert!(base.slot_count() < snow.slot_count());
}
