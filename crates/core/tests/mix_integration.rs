// SPDX-License-Identifier: GPL-3.0-only
//! 真实资产集成测试。需要环境变量 RA2_ASSETS_DIR 指向游戏目录(含原版 MIX)。
//! 运行:RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core -- --ignored

use ra2_core::format::mix::MixArchive;

fn assets_dir() -> std::path::PathBuf {
    std::env::var("RA2_ASSETS_DIR")
        .expect("需要环境变量 RA2_ASSETS_DIR")
        .into()
}

fn load(name: &str) -> MixArchive {
    let path = assets_dir().join(name);
    let data = std::fs::read(&path).unwrap_or_else(|e| panic!("读取 {}: {e}", path.display()));
    MixArchive::parse(data).expect("解析 MIX")
}

/// ares.mix 实测布局(计划文档规格依据):count=4,首条目 id/offset/size,首文件为 CSF。
#[test]
#[ignore]
fn ares_mix_matches_verified_layout() {
    let mix = load("ares.mix");
    assert_eq!(mix.entries().len(), 4);
    let e0 = &mix.entries()[0];
    assert_eq!(e0.id, 0x8F96_AFF1);
    assert_eq!(e0.offset, 0);
    assert_eq!(e0.size, 1760);
    let data = mix.read_by_id(0x8F96_AFF1).expect("读取首条目");
    assert_eq!(&data[0..4], b" FSC", "首条目应为 CSF 字符串表");
}

/// rulesmd.ini 必须能按名从 expandmd01.mix 中找到(裁决 mix_name_id 正确性;
/// 实测 ID 0x8218F9F4。注意:rulesmd.ini 位于 expandmd01.mix,不在 ra2md.mix——
/// 后者的 local mix database.dat 列出的 26 个文件名里没有它)。
#[test]
#[ignore]
fn expandmd01_contains_rulesmd_ini() {
    let mix = load("expandmd01.mix");
    let e = mix
        .find("rulesmd.ini")
        .expect("mix_name_id 或 MIX 解析有误");
    assert_eq!(e.id, 0x8218_F9F4);
}

/// 社区重打包的扩张 MIX 应包含 xmd(本地混合数据库)。
#[test]
#[ignore]
fn expandmd01_contains_local_mix_database() {
    let mix = load("expandmd01.mix");
    assert!(mix.find("local mix database.dat").is_some());
}

/// 真库全量裁决:三库 local mix database.dat 列出的全部文件名(76 个)都能按名命中索引。
/// LMD 为 XCC 二进制格式:头部之后是 NUL 分隔的纯文件名列表(不含 ID)。
#[test]
#[ignore]
fn lmd_names_resolve_in_all_real_mixes() {
    for (file, expect_names) in [
        ("ra2md.mix", 26),
        ("expandmd01.mix", 38),
        ("langmd.mix", 12),
    ] {
        let mix = load(file);
        let lmd = mix.read("local mix database.dat").expect("读取 LMD");
        let names: Vec<&str> = lmd
            .split(|&b| b == 0)
            .filter(|t| {
                t.len() >= 3
                    && t.iter().all(|&b| b.is_ascii_graphic() || b == b' ')
                    && t.contains(&b'.')
                    && !t.starts_with(b"XCC")
            })
            .map(|t| std::str::from_utf8(t).unwrap())
            .collect();
        assert_eq!(names.len(), expect_names, "{file} LMD 名单数不符");
        for n in &names {
            assert!(mix.find(n).is_some(), "{file}: 按名找不到 {n}");
        }
    }
}

/// 从 expandmd01.mix 提取 rulesmd.ini 并解析(MIX+INI 联合裁决)。
/// 注:任务简报原文写 ra2md.mix,但实测(见上方 expandmd01_contains_rulesmd_ini)
/// rulesmd.ini 位于 expandmd01.mix,ra2md.mix 索引中无此 ID。
#[test]
#[ignore]
fn rulesmd_ini_parses_from_mix() {
    use ra2_core::format::ini::IniDocument;
    let mix = load("expandmd01.mix");
    let text = std::str::from_utf8(mix.read("rulesmd.ini").expect("提取 rulesmd.ini"))
        .expect("rulesmd.ini 应为 UTF-8/ASCII 文本");
    let doc = IniDocument::parse(text);
    assert!(doc.section("General").is_some());
    assert!(doc.section("InfantryTypes").is_some());
    assert!(doc.section("VehicleTypes").is_some());
    assert!(doc.section("BuildingTypes").is_some());
    assert!(doc.sections.len() > 100, "rulesmd.ini 应有大量节");
}
