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
