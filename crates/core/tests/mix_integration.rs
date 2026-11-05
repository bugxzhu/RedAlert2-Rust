// SPDX-License-Identifier: GPL-3.0-only
//! 真实资产集成测试。需要环境变量 RA2_ASSETS_DIR 指向游戏目录(含原版 MIX)。
//! 运行:RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core -- --ignored

use ra2_core::format::mix::MixArchive;
use ra2_core::format::FormatError;

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

/// 真库裁决:ra2md.mix 内嵌的 isourbmd.mix(城市剧场库)flags=0x00010000。
/// 摘要位为工具侧咨询位,引擎从不跳 20(REA 0x5B3C20 裁决);工具在 body_size
/// 末尾预留 20 字节(body_size == 文件长-694-20 精确成立,条目偏移相对索引尾)。
/// 实测 38 个摘要位嵌套库全部无 LMD,故按首条目 ID 读取(计划规格依据 §3)。
/// 其 57 个条目内容不属于已验证的 RA2 TMP 布局(计划规格依据 §4),此处只裁决容器层。
#[test]
#[ignore]
fn isourbmd_digest_mix_descends_and_reads_entry() {
    let ra2md = load("ra2md.mix");
    let blob = ra2md
        .read("isourbmd.mix")
        .expect("ra2md.mix 应含 isourbmd.mix(LMD 名单内有)");
    assert_eq!(blob.len(), 5_198_826);
    let iso = MixArchive::parse(blob.to_vec()).expect("摘要位应被接受");
    assert_eq!(iso.entries().len(), 57);
    assert_eq!(iso.entries()[0].id, 0x8CAA_79E6);
    assert_eq!(iso.entries()[0].size, 1_757_768);
    let first = iso.read_by_id(iso.entries()[0].id).unwrap();
    assert_eq!(first.len(), 1_757_768);
}

/// 真库负样本:加密位/未知位的嵌套库必须明确报 Unsupported(计划规格依据 §8)。
#[test]
#[ignore]
fn real_mixes_with_encrypted_or_unknown_flags_are_unsupported() {
    let ra2md = load("ra2md.mix");
    for (name, why) in [
        ("loadmd.mix", "摘要+加密 0x00030000"),
        ("sidec02md.mix", "Blowfish 加密 0x00020000"),
    ] {
        let blob = ra2md
            .read(name)
            .unwrap_or_else(|e| panic!("读 {name}: {e}"));
        match MixArchive::parse(blob.to_vec()) {
            Err(FormatError::Unsupported(_)) => {}
            // MixArchive 未实现 Debug,故拆开两臂分别报告(Brief 原写法 {other:?} 无法编译)
            Err(e) => panic!("{name}({why})应报 Unsupported,实际 {e:?}"),
            Ok(_) => panic!("{name}({why})应报 Unsupported,实际得到 Ok"),
        }
    }
    // 未知高位(非 digest/非加密)
    let langmd = load("langmd.mix");
    let audiomd = langmd
        .read("audiomd.mix")
        .expect("langmd.mix 应含 audiomd.mix");
    assert!(matches!(
        MixArchive::parse(audiomd.to_vec()),
        Err(FormatError::Unsupported(_))
    ));
}

/// 递归枚举容器(与 2026-11-05 计划依据 §2 探针同口径):顶层 = RA2_ASSETS_DIR
/// 下全部 *.mix(含 langmd/thememd/language/ares),每层把"能被 MixArchive::parse
/// 接受"的条目作为嵌套容器下钻,深度 ≤4(顶层为深度 0)。摘要位修复后语料必须
/// 全绿:46 容器 = 27 置位 + 19 朴素;引擎从不跳 20(REA 0x5B3C20 裁决),
/// 置位容器数据区一律从索引尾起算。
#[test]
#[ignore]
fn all_parseable_mix_containers_recurse_green() {
    #[derive(Default)]
    struct Stats {
        total: usize,
        flagged: usize,
        plain: usize,
    }
    fn walk(data: &[u8], depth: usize, stats: &mut Stats) {
        let mix = match MixArchive::parse(data.to_vec()) {
            Ok(m) => m,
            Err(_) => return,
        };
        stats.total += 1;
        if mix.digest_flag() {
            stats.flagged += 1;
        } else {
            stats.plain += 1;
        }
        if depth >= 4 {
            return;
        }
        for e in mix.entries() {
            let blob = mix.read_by_id(e.id).unwrap().to_vec();
            walk(&blob, depth + 1, stats);
        }
    }
    let mut stats = Stats::default();
    let mut tops: Vec<std::path::PathBuf> = std::fs::read_dir(assets_dir())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "mix"))
        .collect();
    tops.sort();
    assert_eq!(tops.len(), 7, "顶层 mix 文件数(安装变动时同步更新)");
    for p in &tops {
        walk(&std::fs::read(p).unwrap(), 0, &mut stats);
    }
    assert_eq!(stats.total, 46, "可解析容器总数(依据 §2 遍历口径)");
    assert_eq!(stats.flagged, 27, "摘要位置位容器数");
    assert_eq!(stats.plain, 19, "朴素容器数");
    // 内容锚:修复后 isosnow 的 clear01.sno 读取长度精确(错位 +20 时必然失败)
    let ra2 = load("ra2.mix");
    let isosnow = MixArchive::parse(ra2.read("isosnow.mix").unwrap().to_vec()).unwrap();
    assert_eq!(isosnow.read("clear01.sno").unwrap().len(), 1_872);
}
