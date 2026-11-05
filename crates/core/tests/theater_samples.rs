// SPDX-License-Identifier: GPL-3.0-only
//! 真实资产集成测试(M3 剧场层)。需要 RA2_ASSETS_DIR。运行:
//! RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test theater_samples -- --ignored

use ra2_core::format::map::mapdoc::decode_map_text;
use ra2_core::format::mix::MixArchive;
use ra2_core::format::theater::TheaterAssets;
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

/// 装载器真测:全语料 open + 跨容器按名查 clear01.sno(isosnow,依据 §4 锚点)
/// + urbannmd.ini 双版本取 expandmd01 版(36,725 B,容量 1189;挂载序首中即胜)。
#[test]
#[ignore]
fn loader_opens_real_assets_and_resolves_across_containers() {
    let a = TheaterAssets::open(&assets_dir()).unwrap();
    let clear = a.lookup("clear01.sno").expect("clear01.sno 应可达");
    assert_eq!(clear.len(), 1_872);
    // isosnow.mix 被拦截名单截流挂自身位(pos 1000+),origin = 本名
    // (2026-11-05 全装载序模拟探针:41 容器/21 拦截命中,clear01 → isosnow.mix)
    assert_eq!(a.origin("clear01.sno"), Some("isosnow.mix"));
    let ini_blob = a.lookup("urbannmd.ini").expect("urbannmd.ini 应可达");
    assert_eq!(
        ini_blob.len(),
        36_725,
        "应取 expandmd01 版而非 localmd 的 32,614 B"
    );
    let ini = TheaterIni::parse(&decode_map_text(ini_blob)).unwrap();
    assert_eq!(ini.slot_count(), 1189);
}

/// 嵌套回退层真测:expandmd01 匿名嵌套 #C31B4D61 可按 id 直查
/// (1018 项/9,987,814 B,2026-11-05 探针实测;研究 §3-5:内含 323 snow 瓦片)。
#[test]
#[ignore]
fn loader_descends_into_anonymous_nested_mix() {
    let a = TheaterAssets::open(&assets_dir()).unwrap();
    let blob = a
        .lookup_id(0xC31B_4D61)
        .expect("匿名嵌套 mix #C31B4D61 应可按 id 直查");
    assert_eq!(blob.len(), 9_987_814);
}
