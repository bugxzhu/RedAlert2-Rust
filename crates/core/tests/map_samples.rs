// SPDX-License-Identifier: GPL-3.0-only
//! 真实地图集成测试(M2):地图直接是 RA2_ASSETS_DIR 下的散文件(不经 MIX),
//! 中文/GBK 文件名按相对路径直接 join。运行:
//! RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test map_samples -- --ignored

use ra2_core::format::ini::IniDocument;
use ra2_core::format::map::mapdoc::decode_map_text;

fn assets_dir() -> std::path::PathBuf {
    std::env::var("RA2_ASSETS_DIR")
        .expect("需要环境变量 RA2_ASSETS_DIR")
        .into()
}

fn read_map(rel: &str) -> Vec<u8> {
    std::fs::read(assets_dir().join(rel)).unwrap_or_else(|e| panic!("读取地图 {rel}: {e}"))
}

/// GBK/UTF-8 解码策略的真图锁定(普查 §1 编码普查 + 规格依据 §10):
/// 随机地图.map 是真 UTF-8;城市海啸.map 的 Name 是 GBK;终极海战防守图.map
/// 的对象段 Name 带截断 GBK 序列。三者都必须无损解析出 IniDocument 且 Name 正确。
#[test]
#[ignore]
fn decode_policy_over_real_maps() {
    // UTF-8 优先
    let doc = IniDocument::parse(&decode_map_text(&read_map("Maps/Custom/随机地图.map")));
    assert_eq!(doc.value("Basic", "Name"), Some("随机地图"));
    // GBK 回退
    let doc = IniDocument::parse(&decode_map_text(&read_map("Maps/MadHQ/城市海啸.map")));
    assert_eq!(doc.value("Basic", "Name"), Some("海嘯"));
    // 截断 GBK:整文件有损解码后仍成档。偏差记录:brief 原文称该图 [Basic] 内
    // 有两个 Name= 键(last-wins 取 GBK 截断序列),实测 [Basic] 只有一个纯
    // ASCII 拼音 Name;GBK 截断序列在该图对象段(如 [01000014])的 Name 上,
    // 截断替换符在那里锁定,断言行为不变。
    let doc = IniDocument::parse(&decode_map_text(&read_map("Maps/DDLY/终极海战防守图.map")));
    assert_eq!(
        doc.value("Basic", "Name"),
        Some("jiangweidajihaizhanfangshoutu")
    );
    let name = doc.value("01000014", "Name").expect("应有 Name");
    assert!(name.contains('\u{FFFD}'), "截断序列应产生替换符: {name}");
    assert!(name.contains("玩家援"));
}

/// 递归收集 RA2_ASSETS_DIR 下全部 .map(807 张;普查 §1 的语料构成)。
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

use ra2_core::format::map::meta::Theater;
use ra2_core::format::map::MapFile;

/// 全量遍历(spec §6-3):807 张图全部 Ok、零 panic。容错规则已覆盖全部
/// 已知语料异常(截断预览/缺段/损坏行/孤儿行/负尺寸),逐张 Err 即失败;
/// 失败时打印首张问题图名与错误,便于定位。
#[test]
#[ignore]
fn corpus_walk_807_maps_all_parse() {
    use ra2_core::format::FormatError;
    let mut total = 0usize;
    let mut errs: Vec<(String, FormatError)> = Vec::new();
    for p in map_files() {
        total += 1;
        let data = std::fs::read(&p).unwrap_or_else(|e| panic!("读 {}: {e}", p.display()));
        if let Err(e) = MapFile::parse(&data) {
            errs.push((p.display().to_string(), e));
        }
    }
    assert_eq!(total, 807, "语料应有 807 张 .map(游戏目录变动时同步更新)");
    assert!(
        errs.is_empty(),
        "{} 张解析失败;首张:{}\n{:#?}",
        errs.len(),
        errs[0].0,
        errs[0].1
    );
}

/// T9 重钉(M3,研究 §3):SNOW 图最大 tile 索引受**剧场 INI 枚举容量**约束
/// (snowmd.ini,964 槽;M2 版"目录序 mix 求和 10016"是目录序规则被推翻前的
/// 失真口径)。锚点:语料 106 张 SNOW 图 max tile = 963 = 容量−1;963 号槽 =
/// clifxl04.sno(set0082 Cliff Ice LowPatch 末文件,位于 isosnomd)。
#[test]
#[ignore]
fn snow_tile_index_within_theater_ini_capacity() {
    use ra2_core::format::map::mapdoc::decode_map_text;
    use ra2_core::format::mix::MixArchive;
    use ra2_core::format::theater::{SlotFile, TheaterIni};
    // snowmd.ini:ra2md → localmd(显式链,不依赖 Task 5 装载器)
    let ra2md = MixArchive::parse(std::fs::read(assets_dir().join("ra2md.mix")).unwrap()).unwrap();
    let localmd = MixArchive::parse(ra2md.read("localmd.mix").unwrap().to_vec()).unwrap();
    let ini = TheaterIni::parse(&decode_map_text(localmd.read("snowmd.ini").unwrap())).unwrap();
    assert_eq!(ini.slot_count(), 964);
    assert_eq!(
        ini.file_name(963, "sno"),
        Some(SlotFile::File("clifxl04.sno".into()))
    );
    // 963 号槽文件现存(isosnomd)
    let isosnomd = MixArchive::parse(ra2md.read("isosnomd.mix").unwrap().to_vec()).unwrap();
    assert!(isosnomd.read("clifxl04.sno").is_ok());
    // 语料:106 张 SNOW 图,max tile 恰为容量−1
    let mut snow_maps = 0usize;
    let mut max_tile: i16 = -1;
    for p in map_files() {
        let data = std::fs::read(&p).unwrap();
        let Ok(m) = MapFile::parse(&data) else {
            continue;
        };
        if m.map_meta.theater != Theater::Snow {
            continue;
        }
        snow_maps += 1;
        if let Some(t) = m.tiles.max_tile_index() {
            max_tile = max_tile.max(t);
        }
    }
    assert!(
        snow_maps >= 100,
        "Maps/** 下应有 106 张 SNOW 图,实得 {snow_maps}"
    );
    assert!(max_tile >= 0, "至少一张 SNOW 图应有瓦片");
    assert_eq!(max_tile, 963, "语料实测最大 SNOW tile(交流意见.map)");
}

/// 截断 PreviewPack 组(规格依据 §8):实测 18 张 DDLY 图在完整位图后带
/// 异常尾块(此处列 7 张代表,含 base64 %4==2 截尾与块头撒谎两类形态),
/// 干净前缀恰为 3·pw·ph → 全部 Ok 且预览 Some(2026-11-04 审查者逐图复核);
/// templateMap.map(无 IsoMapPack5)预览前缀 0 → None,全语料唯一
/// preview_truncated==true。
#[test]
#[ignore]
fn truncated_preview_group_degrades_gracefully() {
    for rel in [
        "Maps/DDLY/一夫当关万夫莫开.map",
        "Maps/DDLY/城郊之战.map",
        "Maps/DDLY/对峙一块地.map",
        "Maps/DDLY/终极茅厕.map",
        "Maps/DDLY/肉人大作战八人图.map",
        "Maps/DDLY/肉人大战6人图.map",
        "Maps/DDLY/降维打击三人防守图.map",
    ] {
        let m = MapFile::parse(&read_map(rel)).unwrap_or_else(|e| panic!("{rel}: {e:?}"));
        let img = m
            .preview
            .as_ref()
            .unwrap_or_else(|| panic!("{rel} 的干净前缀应为完整位图"));
        assert_eq!(img.rgb.len(), 3 * img.width as usize * img.height as usize);
    }
    let m = MapFile::parse(&read_map(
        "Resources/RandomMapGenerator_RA2/templateMap.map",
    ))
    .unwrap();
    assert!(m.tiles.is_empty());
    assert!(m.preview.is_none());
    assert!(m.pack_stats.preview_truncated);
}
