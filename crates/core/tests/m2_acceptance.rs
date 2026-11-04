// SPDX-License-Identifier: GPL-3.0-only
//! M2 里程碑验收:18 张代表图(官方协作/TS 代官方/遭遇战/社区中文/
//! RMG 产物/退化样例)逐张字节级甲骨文 + 坐标恒等核对。
//! 运行:RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test m2_acceptance -- --ignored

use ra2_core::format::map::meta::Theater;
use ra2_core::format::map::objects::ObjectClass;
use ra2_core::format::map::MapFile;

fn assets_dir() -> std::path::PathBuf {
    std::env::var("RA2_ASSETS_DIR")
        .expect("需要环境变量 RA2_ASSETS_DIR")
        .into()
}

fn read_map(rel: &str) -> Vec<u8> {
    std::fs::read(assets_dir().join(rel)).unwrap_or_else(|e| panic!("读取地图 {rel}: {e}"))
}

struct Oracle {
    rel: &'static str,
    theater: Theater,
    w: i64,
    h: i64,
    recs: usize,
    structures: usize,
    units: usize,
    infantry: usize,
    aircraft: usize,
    terrain: usize,
    smudge: usize,
    malformed: usize,
    /// Some((pw, ph, 字节数)) = 预览存在;None = 无预览。
    preview: Option<(u32, u32, usize)>,
    /// [Basic]Name(仅 ASCII 名钉死;中文名的解码在 map_samples 单独锁定)。
    name: Option<&'static str>,
    orphans: usize,
}

const ORACLES: &[Oracle] = &[
    // —— spec §6-2 的代表集:官方协作(4 剧场)+ TS 代官方 4 张(短记录)
    Oracle {
        rel: "Maps/Cooperative/c1a01md.map",
        theater: Theater::Temperate,
        w: 87,
        h: 71,
        recs: 12283,
        structures: 65,
        units: 10,
        infantry: 14,
        aircraft: 0,
        terrain: 749,
        smudge: 0,
        malformed: 0,
        preview: Some((162, 59, 28674)),
        name: None,
        orphans: 0,
    },
    Oracle {
        rel: "Maps/Cooperative/c1a03md.map",
        theater: Theater::Snow,
        w: 110,
        h: 80,
        recs: 17520,
        structures: 48,
        units: 0,
        infantry: 0,
        aircraft: 0,
        terrain: 1227,
        smudge: 0,
        malformed: 0,
        preview: Some((208, 69, 43056)),
        name: None,
        orphans: 0,
    },
    Oracle {
        rel: "Maps/Cooperative/c2s02md.map",
        theater: Theater::Desert,
        w: 90,
        h: 103,
        recs: 18437,
        structures: 54,
        units: 2,
        infantry: 9,
        aircraft: 0,
        terrain: 500,
        smudge: 0,
        malformed: 0,
        preview: Some((170, 92, 46920)),
        name: None,
        orphans: 0,
    },
    Oracle {
        rel: "Maps/Cooperative/c1a02md.map",
        theater: Theater::NewUrban,
        w: 105,
        h: 110,
        recs: 22990,
        structures: 255,
        units: 56,
        infantry: 44,
        aircraft: 3,
        terrain: 1154,
        smudge: 0,
        malformed: 0,
        preview: Some((200, 97, 58200)),
        name: None,
        orphans: 0,
    },
    Oracle {
        rel: "Maps/Standard/moonpatr.map",
        theater: Theater::Lunar,
        w: 60,
        h: 62,
        recs: 7378,
        structures: 11,
        units: 0,
        infantry: 0,
        aircraft: 0,
        terrain: 0,
        smudge: 0,
        malformed: 0,
        preview: Some((111, 52, 17316)),
        name: None,
        orphans: 0,
    },
    // TS 代官方:记录数 < (2W−1)·H(缺 void 格,§3.6)
    Oracle {
        rel: "Maps/Standard/riverram.map",
        theater: Theater::Temperate,
        w: 52,
        h: 61,
        recs: 6255,
        structures: 0,
        units: 0,
        infantry: 0,
        aircraft: 0,
        terrain: 128,
        smudge: 0,
        malformed: 0,
        preview: Some((95, 49, 13965)),
        name: None,
        orphans: 0,
    },
    Oracle {
        rel: "Maps/Standard/frstbite.map",
        theater: Theater::Snow,
        w: 100,
        h: 100,
        recs: 19898,
        structures: 18,
        units: 0,
        infantry: 0,
        aircraft: 0,
        terrain: 190,
        smudge: 0,
        malformed: 0,
        preview: Some((188, 87, 49068)),
        name: None,
        orphans: 0,
    },
    Oracle {
        rel: "Maps/Standard/hillbtwn.map",
        theater: Theater::Urban,
        w: 60,
        h: 68,
        recs: 8081,
        structures: 129,
        units: 76,
        infantry: 0,
        aircraft: 0,
        terrain: 266,
        smudge: 0,
        malformed: 0,
        preview: Some((111, 57, 18981)),
        name: None,
        orphans: 0,
    },
    Oracle {
        rel: "Maps/Standard/isleofoades.map",
        theater: Theater::Temperate,
        w: 98,
        h: 85,
        recs: 16557,
        structures: 8,
        units: 0,
        infantry: 0,
        aircraft: 0,
        terrain: 358,
        smudge: 0,
        malformed: 0,
        preview: Some((186, 75, 41850)),
        name: None,
        orphans: 0,
    },
    // —— 语料极值/社区/中文图名
    Oracle {
        rel: "Maps/MadHQ/2+1=3.map",
        theater: Theater::Temperate,
        w: 272,
        h: 158,
        recs: 85794,
        structures: 35,
        units: 0,
        infantry: 0,
        aircraft: 0,
        terrain: 836,
        smudge: 0,
        malformed: 0,
        preview: Some((544, 158, 257856)),
        name: Some("Trinity (2-8)"),
        orphans: 0,
    },
    Oracle {
        rel: "Maps/MadHQ/避暑山庄.map",
        theater: Theater::Temperate,
        w: 399,
        h: 100,
        recs: 79700,
        structures: 650,
        units: 40,
        infantry: 93,
        aircraft: 0,
        terrain: 3883,
        smudge: 0,
        malformed: 0,
        preview: Some((798, 100, 239400)),
        name: None,
        orphans: 0,
    },
    Oracle {
        rel: "Maps/MadHQ/陋室空堂.map",
        theater: Theater::Temperate,
        w: 200,
        h: 200,
        recs: 79800,
        structures: 589,
        units: 28,
        infantry: 0,
        aircraft: 0,
        terrain: 12562,
        smudge: 122,
        malformed: 0,
        preview: Some((400, 200, 240000)),
        name: None,
        orphans: 0,
    },
    Oracle {
        rel: "Maps/DDLY/城郊之战.map",
        theater: Theater::Urban,
        w: 90,
        h: 90,
        recs: 16110,
        structures: 285,
        units: 0,
        infantry: 0,
        aircraft: 0,
        terrain: 486,
        smudge: 0,
        malformed: 0,
        preview: Some((180, 90, 48600)),
        name: Some("No name"),
        orphans: 0,
    },
    Oracle {
        rel: "Maps/DDLY/终极海战防守图.map",
        theater: Theater::Temperate,
        w: 130,
        h: 80,
        recs: 20720,
        structures: 201,
        units: 1,
        infantry: 0,
        aircraft: 0,
        terrain: 0,
        smudge: 0,
        malformed: 0,
        preview: Some((260, 80, 62400)),
        name: None,
        orphans: 0,
    },
    // —— 真实损坏行
    Oracle {
        rel: "Maps/MadHQ/勇士与巨龙.map",
        theater: Theater::Temperate,
        w: 137,
        h: 140,
        recs: 38220,
        structures: 160,
        units: 0,
        infantry: 0,
        aircraft: 0,
        terrain: 697,
        smudge: 270,
        malformed: 1,
        preview: Some((274, 140, 115080)),
        name: Some("Weathered Alliance (2-7)"),
        orphans: 0,
    },
    // —— RMG 产物
    Oracle {
        rel: "Resources/RandomMapGenerator_RA2/MapUnits/NEWURBAN/1,1,1,1,01,spawn.map",
        theater: Theater::NewUrban,
        w: 30,
        h: 30,
        recs: 1770,
        structures: 1,
        units: 0,
        infantry: 0,
        aircraft: 0,
        terrain: 9,
        smudge: 0,
        malformed: 0,
        preview: Some((60, 30, 5400)),
        name: Some("No name"),
        orphans: 44,
    },
    // —— 退化/缺段样例
    Oracle {
        rel: "Resources/RandomMapGenerator_RA2/MapUnits/DESERT/indicator.map",
        theater: Theater::Desert,
        w: 1,
        h: 1,
        recs: 1,
        structures: 0,
        units: 0,
        infantry: 0,
        aircraft: 0,
        terrain: 0,
        smudge: 0,
        malformed: 0,
        preview: Some((2, 1, 6)),
        name: Some("No name"),
        orphans: 0,
    },
    Oracle {
        rel: "Resources/RandomMapGenerator_RA2/templateMap.map",
        theater: Theater::Temperate,
        w: 50,
        h: 50,
        recs: 0,
        structures: 0,
        units: 0,
        infantry: 0,
        aircraft: 0,
        terrain: 0,
        smudge: 0,
        malformed: 0,
        preview: None,
        name: Some("Random Map"),
        orphans: 0,
    },
];

#[test]
#[ignore]
fn m2_representative_maps_full_oracle() {
    for o in ORACLES {
        let m = MapFile::parse(&read_map(o.rel)).unwrap_or_else(|e| panic!("{}: {e:?}", o.rel));
        assert_eq!(m.map_meta.theater, o.theater, "{}", o.rel);
        assert_eq!(
            (m.map_meta.size[2], m.map_meta.size[3]),
            (o.w, o.h),
            "{}",
            o.rel
        );
        assert_eq!(m.tiles.len(), o.recs, "{}", o.rel);
        assert_eq!(
            m.objects.count(ObjectClass::Structure),
            o.structures,
            "{}",
            o.rel
        );
        assert_eq!(m.objects.count(ObjectClass::Unit), o.units, "{}", o.rel);
        assert_eq!(
            m.objects.count(ObjectClass::Infantry),
            o.infantry,
            "{}",
            o.rel
        );
        assert_eq!(
            m.objects.count(ObjectClass::Aircraft),
            o.aircraft,
            "{}",
            o.rel
        );
        assert_eq!(
            m.objects.count(ObjectClass::Terrain),
            o.terrain,
            "{}",
            o.rel
        );
        assert_eq!(m.objects.count(ObjectClass::Smudge), o.smudge, "{}", o.rel);
        assert_eq!(m.objects.malformed, o.malformed, "{}", o.rel);
        match o.preview {
            Some((pw, ph, bytes)) => {
                let img = m
                    .preview
                    .as_ref()
                    .unwrap_or_else(|| panic!("{} 应有预览", o.rel));
                assert_eq!((img.width, img.height), (pw, ph), "{}", o.rel);
                assert_eq!(img.rgb.len(), bytes, "{}", o.rel);
            }
            None => assert!(m.preview.is_none(), "{} 应无预览", o.rel),
        }
        if let Some(n) = o.name {
            assert_eq!(
                m.basic.as_ref().unwrap().name.as_deref(),
                Some(n),
                "{}",
                o.rel
            );
        }
        assert_eq!(m.orphan_key_values, o.orphans, "{}", o.rel);
        assert!(m.digest.is_some(), "{} 应有 [Digest]", o.rel);
    }

    // 坐标恒等(研究 §3.4):c1a01md 的全部 [Terrain] 键逐格命中瓦片记录
    // (749/749;[Terrain] 键 Y·1000+X 与 IsoMapPack5 存储 (x,y) 同一坐标系)
    let m = MapFile::parse(&read_map("Maps/Cooperative/c1a01md.map")).unwrap();
    let mut terrain_cells = 0usize;
    for o in &m.objects.all {
        if o.class != ObjectClass::Terrain {
            continue;
        }
        let (x, y) = o.cell().expect("Terrain 坐标键应可解码");
        assert!(
            m.tiles.cell(x as u16, y as u16).is_some(),
            "Terrain 格 ({x},{y}) 无瓦片记录"
        );
        terrain_cells += 1;
    }
    assert_eq!(terrain_cells, 749);
}
