// SPDX-License-Identifier: GPL-3.0-only
//! 真实资产集成测试(M3 剧场层)。需要 RA2_ASSETS_DIR。运行:
//! RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test theater_samples -- --ignored

use ra2_core::format::map::mapdoc::decode_map_text;
use ra2_core::format::mix::MixArchive;
use ra2_core::format::theater::TheaterIni;

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

/// 六份 md INI 真测:localmd 五份 + expandmd01 的 urbannmd。
/// 容量 838/1081/964/726/198/1175(localmd 版)/1189(expandmd01 版,依据 §3
/// 探针复算);LastTilesInSet 活键零出现(REA 勘误语料断言;七份 INI 各带
/// 22–24 行 `;LastTilesInSet = N` 注释模板,不计);localmd urbannmd
/// 的 TilesInSet = o 脏数据实证(atoi → 0)。
#[test]
#[ignore]
fn six_md_theater_inis_capacities_and_last_tiles_census() {
    let ra2md = load("ra2md.mix");
    let localmd = MixArchive::parse(ra2md.read("localmd.mix").unwrap().to_vec()).unwrap();
    let expand = load("expandmd01.mix");
    for (container, name, cap) in [
        (&localmd, "temperatmd.ini", 838),
        (&localmd, "urbanmd.ini", 1081),
        (&localmd, "snowmd.ini", 964),
        (&localmd, "desertmd.ini", 726),
        (&localmd, "lunarmd.ini", 198),
        (&localmd, "urbannmd.ini", 1175),
        (&expand, "urbannmd.ini", 1189),
    ] {
        let blob = container
            .read(name)
            .unwrap_or_else(|e| panic!("读 {name}: {e}"));
        let text = decode_map_text(blob);
        let ini = TheaterIni::parse(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(ini.slot_count(), cap, "{name}");
        assert!(
            !text.lines().any(|l| {
                let t = l.trim_start();
                !t.starts_with(';') && t.to_ascii_lowercase().contains("lasttilesinset")
            }),
            "{name} 出现活键 LastTilesInSet(REA 勘误语料断言;注释模板行不计)"
        );
    }
    // 脏数据实证:localmd urbannmd 的 TilesInSet = o(研究 §3;第 1374 行逐字节)
    let raw = String::from_utf8_lossy(localmd.read("urbannmd.ini").unwrap());
    assert!(
        raw.contains("TilesInSet = o"),
        "set0105 脏数据应存在(研究 §3)"
    );
}
