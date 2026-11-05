// SPDX-License-Identifier: GPL-3.0-only
//! 真实资产集成测试:标准样本集提取 + 各格式真字节断言(Task 3-8 逐步追加)。
//! 需要 RA2_ASSETS_DIR 指向游戏目录。运行:
//! RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test format_samples -- --ignored

use ra2_core::format::mix::MixArchive;

fn assets_dir() -> std::path::PathBuf {
    std::env::var("RA2_ASSETS_DIR")
        .expect("需要环境变量 RA2_ASSETS_DIR")
        .into()
}

fn fixtures_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/samples")
}

/// 最小 SHA-256(FIPS 180-4),仅测试用:对提取字节做 oracle 哈希断言。
fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for block in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for i in 0..8 {
        out[i * 4..i * 4 + 4].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

fn sha16(data: &[u8]) -> String {
    let d = sha256(data);
    (0..8).map(|i| format!("{:02x}", d[i])).collect()
}

/// 逐级按名下降 MIX 链并读取条目。每层先复制为自有字节再解析下一层
/// (MixArchive 拥有数据,借用的切片不能跨层存活)。
fn read_from_chain(chain: &[&str], name: &str) -> Vec<u8> {
    let mut bytes = std::fs::read(assets_dir().join(chain[0]))
        .unwrap_or_else(|e| panic!("读取顶层 {}: {e}", chain[0]));
    for level in &chain[1..] {
        let mix = MixArchive::parse(bytes).expect("解析嵌套 MIX");
        bytes = mix
            .read(level)
            .unwrap_or_else(|e| panic!("按名读取 {level}: {e}"))
            .to_vec();
    }
    let mix = MixArchive::parse(bytes).expect("解析目标 MIX");
    mix.read(name)
        .unwrap_or_else(|e| panic!("按名读取 {name}: {e}"))
        .to_vec()
}

/// 自愈式取样本:本地 fixtures/samples 已有且哈希相符则复用;否则从真库链
/// 提取并断言哈希后落盘(先写临时名再原子替换,避免并行测试读到半截文件)。
fn ensure_sample(chain: &[&str], name: &str, expect_sha16: &str) -> Vec<u8> {
    let path = fixtures_dir().join(name);
    if let Ok(existing) = std::fs::read(&path) {
        if sha16(&existing) == expect_sha16 {
            return existing;
        }
    }
    let data = read_from_chain(chain, name);
    assert_eq!(
        sha16(&data),
        expect_sha16,
        "{name} 的 SHA-256 与取证记录不符"
    );
    // 临时名保留完整文件名+".part":同 stem 不同扩展名的样本(1tnk.vxl/1tnk.hva)
    // 不会互相争抢同一个 tmp 路径(并行测试安全的前提)。
    let tmp = path.with_file_name(format!("{name}.part"));
    std::fs::write(&tmp, &data).unwrap();
    std::fs::rename(&tmp, &path).unwrap();
    data
}

/// 计划标准样本集(来源链与哈希=计划规格依据 §7,全部 2026-11-03 按名核验)。
type SampleSpec = (&'static [&'static str], &'static str, &'static str);

const SAMPLES: &[SampleSpec] = &[
    (
        &["ra2.mix", "sidec01.mix"],
        "pwrlvl.shp",
        "e0837c1520ff9538",
    ),
    (
        &["ra2md.mix", "conqmd.mix"],
        "mindanimr.shp",
        "ef4617cf48248f0c",
    ),
    (
        &["ra2md.mix", "conqmd.mix"],
        "gunfirey.shp",
        "4425f5bdf013e1a9",
    ),
    (&["ra2.mix", "snow.mix"], "burnt01.sno", "bc1a23700e17563f"),
    (&["ra2.mix", "snow.mix"], "lobrdb08.sno", "7a1309fc8c192d82"),
    (
        &["ra2md.mix", "localmd.mix"],
        "mpyscrn.pal",
        "62cc843355a1bcc9",
    ),
    (&["ra2.mix", "local.mix"], "key.ini", "ad7fba6d2737ed9b"),
    (
        &["ra2md.mix", "localmd.mix"],
        "schp.hva",
        "68f97095812ab96a",
    ),
    (&["langmd.mix"], "ra2md.csf", "8a0705c9694c4376"),
    (&["ra2.mix", "local.mix"], "text1.aud", "99470c6ce81ebe53"),
    (&["ra2.mix", "local.mix"], "1tnk.vxl", "45e3bb0e6413589f"),
    (
        &["ra2.mix", "local.mix"],
        "1tnkbarl.vxl",
        "f45b93d65a2774fc",
    ),
    (&["ra2.mix", "local.mix"], "1tnk.hva", "ca89519836d92d31"),
    (
        &["ra2.mix", "local.mix"],
        "1tnkbarl.hva",
        "2088f15a367a607d",
    ),
];

/// 提取标准样本集:每个样本按名走真实 MIX 链、断言 SHA-256 甲骨文、落盘
/// fixtures/samples(gitignored)。该测试同时是后续各格式真测的前置。
#[test]
#[ignore]
fn extract_standard_samples_and_verify_oracle_hashes() {
    std::fs::create_dir_all(fixtures_dir()).unwrap();
    for (chain, name, sha) in SAMPLES {
        let data = ensure_sample(chain, name, sha);
        assert_eq!(sha16(&data), *sha);
    }
}

/// pwrlvl.shp:最小 1 帧未压缩 SHP(研究文档 §1.5 逐字节验证)。
/// 像素:fe×10 00×4 fe×2(16 字节,帧内嵌 4 个透明像素)。
/// (brief/研究文档原文 fe×9+fe×3 系笔误;已按哈希钉死样本的原始字节改正)
#[test]
#[ignore]
fn shp_pwrlvl_minimal_uncompressed() {
    use ra2_core::format::shp::ShpFile;
    let data = ensure_sample(
        &["ra2.mix", "sidec01.mix"],
        "pwrlvl.shp",
        "e0837c1520ff9538",
    );
    assert_eq!(data.len(), 48);
    let shp = ShpFile::parse(&data).unwrap();
    assert_eq!((shp.cx, shp.cy), (8, 2));
    assert_eq!(shp.frames.len(), 1);
    let f = &shp.frames[0];
    assert_eq!(
        (f.header.x, f.header.y, f.header.cx, f.header.cy),
        (0, 0, 8, 2)
    );
    assert_eq!(f.header.flags, 1); // 仅 bit0;bit1 未置 → 未压缩
    assert_eq!(f.header.offset, 32); // 8 + 24,恰在表尾
    let mut expected = vec![0xFEu8; 10];
    expected.extend_from_slice(&[0u8; 4]);
    expected.extend_from_slice(&[0xFEu8; 2]);
    assert_eq!(f.pixels, expected);
}

/// mindanimr.shp:8 帧全压缩(flags=3),偏移严格递增、帧行链后有 0 填充。
#[test]
#[ignore]
fn shp_mindanimr_8_compressed_frames() {
    use ra2_core::format::shp::ShpFile;
    let data = ensure_sample(
        &["ra2md.mix", "conqmd.mix"],
        "mindanimr.shp",
        "ef4617cf48248f0c",
    );
    assert_eq!(data.len(), 1144);
    let shp = ShpFile::parse(&data).unwrap();
    assert_eq!((shp.cx, shp.cy), (30, 22));
    assert_eq!(shp.frames.len(), 8);
    let expect = [
        (7i16, 5i16, 14u16, 8u16, 0xC8u32),
        (7, 5, 15, 9, 0x138),
        (6, 5, 17, 9, 0x1C0),
        (4, 4, 20, 11, 0x260),
        (5, 4, 19, 11, 0x320),
        (10, 7, 8, 5, 0x3B0),
        (9, 6, 10, 6, 0x3E8),
        (8, 6, 12, 7, 0x428),
    ];
    for (f, e) in shp.frames.iter().zip(expect) {
        assert_eq!(
            (
                f.header.x,
                f.header.y,
                f.header.cx,
                f.header.cy,
                f.header.offset
            ),
            e
        );
        assert_eq!(f.header.flags, 3);
        assert_eq!(f.pixels.len(), (e.2 as usize) * (e.3 as usize));
    }
}

/// gunfirey.shp:8 帧含 3 个空帧(f4-f6)与按位置配对的阴影帧 f7(rgb 全黑)。
#[test]
#[ignore]
fn shp_gunfirey_empty_and_shadow_frames() {
    use ra2_core::format::shp::ShpFile;
    let data = ensure_sample(
        &["ra2md.mix", "conqmd.mix"],
        "gunfirey.shp",
        "4425f5bdf013e1a9",
    );
    assert_eq!(data.len(), 4000);
    let shp = ShpFile::parse(&data).unwrap();
    assert_eq!((shp.cx, shp.cy), (97, 94));
    assert_eq!(shp.frames.len(), 8);
    for i in 4..=6 {
        let f = &shp.frames[i];
        assert_eq!((f.header.cx, f.header.cy, f.header.offset), (0, 0, 0));
        assert!(f.pixels.is_empty());
        assert_eq!(f.header.flags, 3); // 空帧 flags 仍非 0——勿以 flags 判空
    }
    assert_eq!(shp.frames[0].pixels.len(), 97 * 90);
    assert_eq!(shp.frames[7].header.rgb, [0, 0, 0]); // 阴影帧(位置配对留给 view 层)
    assert_eq!(shp.frames[7].pixels.len(), 94 * 94);
}

/// burnt01.sno(TS SHP 剧场装饰精灵,M1b 误判已纠正,研究 §4):120x120 画布,
/// 2 帧(1 图形 29x17 灼痕 + 1 空)。
/// 像素向量:343 个非零;首 16 像素 = 10 零 + b3 b7 bb 00 00 b4(2026-11-03 取证)。
#[test]
#[ignore]
fn shp_ts_burnt01_sno_sprite() {
    use ra2_core::format::shp_ts::ShpTsFile;
    let data = ensure_sample(&["ra2.mix", "snow.mix"], "burnt01.sno", "bc1a23700e17563f");
    assert_eq!(data.len(), 520);
    let tmp = ShpTsFile::parse(&data).unwrap();
    assert_eq!((tmp.cx, tmp.cy), (120, 120));
    assert_eq!(tmp.frames.len(), 2);
    let t0 = &tmp.frames[0];
    assert_eq!(
        (t0.header.x, t0.header.y, t0.header.cx, t0.header.cy),
        (44, 64, 29, 17)
    );
    assert_eq!(t0.header.flags, 3);
    assert_eq!(t0.header.rgba, [0, 0, 1, 0]);
    assert_eq!(t0.header.data_offset, 56);
    assert_eq!(t0.pixels.len(), 29 * 17);
    assert_eq!(t0.pixels.iter().filter(|&&p| p != 0).count(), 343);
    assert_eq!(
        &t0.pixels[..16],
        &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xB3, 0xB7, 0xBB, 0, 0, 0xB4]
    );
    let t1 = &tmp.frames[1];
    assert_eq!(
        (t1.header.cx, t1.header.cy, t1.header.data_offset),
        (0, 0, 0)
    );
    assert!(t1.pixels.is_empty());
    assert_eq!(t1.header.reserved, 0);
}

/// lobrdb08.sno(TS SHP 剧场装饰精灵,M1b 误判已纠正,研究 §4):180x120 画布,
/// 6 帧(1 图形 120x69 菱形 + 5 空);t0 reserved 带小地图绿(旧"radar"字节)。
#[test]
#[ignore]
fn shp_ts_lobrdb08_sno_diamond() {
    use ra2_core::format::shp_ts::ShpTsFile;
    let data = ensure_sample(&["ra2.mix", "snow.mix"], "lobrdb08.sno", "7a1309fc8c192d82");
    assert_eq!(data.len(), 3360);
    let tmp = ShpTsFile::parse(&data).unwrap();
    assert_eq!((tmp.cx, tmp.cy), (180, 120));
    assert_eq!(tmp.frames.len(), 6);
    let t0 = &tmp.frames[0];
    assert_eq!(
        (t0.header.cx, t0.header.cy, t0.header.data_offset),
        (0, 0, 0)
    );
    assert_eq!(t0.header.flags, 4);
    assert_eq!(t0.header.reserved, 0x0012_FB00); // 字节 00 FB 12 00 的小端 u32
    let t1 = &tmp.frames[1];
    assert_eq!(
        (t1.header.x, t1.header.y, t1.header.cx, t1.header.cy),
        (30, 20, 120, 69)
    );
    assert_eq!(t1.header.flags, 3);
    assert_eq!(t1.header.rgba, [0, 0, 6, 0]); // 蓝通道 6(旧 unknown_d)
    assert_eq!(t1.header.data_offset, 152);
    assert_eq!(t1.pixels.len(), 120 * 69);
    assert_eq!(t1.pixels.iter().filter(|&&p| p != 0).count(), 2788);
    for t in &tmp.frames[2..] {
        assert!(t.pixels.is_empty());
    }
}

/// 真库甲骨文:直接从 ra2.mix → snow.mix 按名提取精灵(该库有 LMD,名单含
/// 全部 .sno 装饰精灵;研究文档的两个样本即出自此库)。
/// 注:任务骨架原想从摘要位剧场库(isourbmd 等)取样本,但实测 38 个摘要位
/// 嵌套库全部无 LMD、其条目也不符合已验证的 TS SHP 帧布局(计划规格依据 §4);
/// 摘要位链路由 Task 1 与 Task 10 的容器层测试覆盖。
#[test]
#[ignore]
fn shp_ts_sprite_reads_by_name_from_snow_mix() {
    use ra2_core::format::shp_ts::ShpTsFile;
    let data = read_from_chain(&["ra2.mix", "snow.mix"], "burnt01.sno");
    assert_eq!(sha16(&data), "bc1a23700e17563f");
    let tmp = ShpTsFile::parse(&data).unwrap();
    assert_eq!(tmp.frames.len(), 2);
    assert_eq!(tmp.frames[0].pixels.len(), 29 * 17);
}

/// ra2md.csf 全量走查(langmd.mix;研究文档 §1.4,2026-11-03 独立复验):
/// 5280 标签 = 4076 'STR ' + 1204 'STRW';4 个空值标签;流恰在 EOF 结束。
#[test]
#[ignore]
fn csf_ra2md_full_parse() {
    use ra2_core::format::csf::CsfDocument;
    let data = ensure_sample(&["langmd.mix"], "ra2md.csf", "8a0705c9694c4376");
    assert_eq!(data.len(), 336_648);
    let csf = CsfDocument::parse(&data).unwrap();
    assert_eq!(csf.labels.len(), 5280);
    let plain = csf
        .labels
        .iter()
        .filter(|l| l.value.is_some() && l.extra.is_none())
        .count();
    let strw = csf.labels.iter().filter(|l| l.extra.is_some()).count();
    let empty_value = csf
        .labels
        .iter()
        .filter(|l| l.value.as_deref() == Some(""))
        .count();
    assert_eq!(plain, 4076); // 含 4 个空值('STR ' + cb_str=0)
    assert_eq!(strw, 1204);
    assert_eq!(empty_value, 4); // GUI:Blank / TXT_NONE / GUI:Empty / STT:Empty
    let l0 = &csf.labels[0];
    assert_eq!(l0.name, "THEME:Intro");
    assert_eq!(l0.value.as_deref(), Some("开场"));
    let vox = csf.find("VOX:ceva001").expect("应有 VOX:ceva001");
    assert_eq!(vox.extra.as_deref(), Some(b"ceva001c".as_slice()));
    assert_eq!(
        vox.value.as_deref().map(|v| v.encode_utf16().count()),
        Some(12)
    );
}

/// text1.aud(22050Hz 单声道 16-bit;研究文档 §2.5,2026-11-03 XCC 表复验):
/// 2 块(1024+562 样本),首 8 样本、跨块窗口、全文件极值全部钉死。
#[test]
#[ignore]
fn aud_text1_ima_decode() {
    use ra2_core::format::aud::Aud;
    let data = ensure_sample(&["ra2.mix", "local.mix"], "text1.aud", "99470c6ce81ebe53");
    assert_eq!(data.len(), 821);
    let aud = Aud::parse(&data).unwrap();
    assert_eq!(aud.header.sample_rate, 22050);
    assert_eq!(aud.header.size_in, 809);
    assert_eq!(aud.header.size_out, 3172);
    assert_eq!(aud.chunks.len(), 2);
    assert_eq!(aud.chunks[0].id, 0x0000_DEAF);
    assert_eq!((aud.chunks[0].size_in, aud.chunks[0].size_out), (512, 2048));
    assert_eq!((aud.chunks[1].size_in, aud.chunks[1].size_out), (281, 1124));
    assert_eq!(aud.samples.len(), 1586);
    assert_eq!(&aud.samples[..8], &[11, 41, -22, 114, -179, 452, -543, 649]);
    // 跨块持续性裁决:样本 1024 起为块 1;若按块复位状态,这里不会是 [-217, -97, 12, 708]
    assert_eq!(&aud.samples[1024..1028], &[-217, -97, 12, 708]);
    assert_eq!(aud.samples.iter().copied().max(), Some(17293));
    assert_eq!(aud.samples.iter().copied().min(), Some(-11630));
}

/// 1tnkbarl.vxl:1 节 BARREL01,9x2x2,18 列(4 空),28 体素;
/// span j=0 原始字节 00 02 39 0F 39 22 02(研究文档 §3 逐字节);
/// 尾部 rest-pose 变换非零、scale=1/12、调色板端点 (255,0,246)/(255,0,255)。
#[test]
#[ignore]
fn vxl_1tnkbarl_single_section() {
    use ra2_core::format::vxl::{VxlFile, VxlVoxel};
    let data = ensure_sample(
        &["ra2.mix", "local.mix"],
        "1tnkbarl.vxl",
        "f45b93d65a2774fc",
    );
    assert_eq!(data.len(), 1164);
    let vxl = VxlFile::parse(&data).unwrap();
    assert_eq!(vxl.sections.len(), 1);
    let s = &vxl.sections[0];
    assert_eq!(s.name, "BARREL01");
    assert_eq!(s.dims, [9, 2, 2]);
    assert_eq!(s.normal_type, 2);
    assert_eq!(s.columns.len(), 18);
    assert_eq!(s.columns.iter().filter(|c| c.voxels.is_empty()).count(), 4);
    let total: usize = s.columns.iter().map(|c| c.voxels.len()).sum();
    assert_eq!(total, 28);
    assert_eq!(
        s.columns[0].voxels,
        [
            VxlVoxel {
                z: 0,
                color: 0x39,
                normal: 15
            },
            VxlVoxel {
                z: 1,
                color: 0x39,
                normal: 34
            }
        ]
    );
    assert!(s.columns[5].voxels.is_empty());
    assert!(s.columns[6].voxels.is_empty());
    assert!((s.scale - 1.0 / 12.0).abs() < 1e-6);
    assert_eq!(vxl.palette[0], [255, 0, 246]);
    assert_eq!(vxl.palette[255], [255, 0, 255]);
    // 尾部静止姿态(2026-11-03 精确取证):非零平移
    assert!((s.transform[0][3] - 0.169_375_33).abs() < 1e-6);
    assert!((s.transform[1][3] - (-1.730_145)).abs() < 1e-5);
    assert!((s.transform[2][3] - 11.604_487).abs() < 1e-5);
    assert!((s.min[0] - 3.816_355_5).abs() < 1e-5);
    assert!((s.max[0] - 12.707_98).abs() < 1e-5);
}

/// 1tnk.vxl:1 节 B-BODY,31x15x11,465 列(14 空),2227 体素。
#[test]
#[ignore]
fn vxl_1tnk_465_spans_2227_voxels() {
    use ra2_core::format::vxl::VxlFile;
    let data = ensure_sample(&["ra2.mix", "local.mix"], "1tnk.vxl", "45e3bb0e6413589f");
    assert_eq!(data.len(), 12537);
    let vxl = VxlFile::parse(&data).unwrap();
    assert_eq!(vxl.sections.len(), 1);
    let s = &vxl.sections[0];
    assert_eq!(s.name, "B-BODY");
    assert_eq!(s.dims, [31, 15, 11]);
    assert_eq!(s.columns.len(), 465);
    assert_eq!(s.columns.iter().filter(|c| c.voxels.is_empty()).count(), 14);
    let total: usize = s.columns.iter().map(|c| c.voxels.len()).sum();
    assert_eq!(total, 2227);
    assert!((s.scale - 1.0 / 12.0).abs() < 1e-6);
}

/// schp.hva:F=2,S=3(CYLINDER19/CYLINDER57/DUMMY01);
/// 帧 1 节 0 = 绕 Z 轴 30° 旋转,平移跨帧恒定(帧主序的独立证据,2026-11-03 精确取证)。
#[test]
#[ignore]
fn hva_schp_frame_major_two_frames() {
    use ra2_core::format::hva::HvaFile;
    let data = ensure_sample(
        &["ra2md.mix", "localmd.mix"],
        "schp.hva",
        "68f97095812ab96a",
    );
    assert_eq!(data.len(), 360);
    let hva = HvaFile::parse(&data).unwrap();
    assert_eq!(hva.frames, 2);
    assert_eq!(hva.section_names, ["CYLINDER19", "CYLINDER57", "DUMMY01"]);
    // matrices[1][0]:30° 绕 Z 旋转(容差按 f32 存储精度取)
    let m = &hva.matrices[1][0];
    assert!((m[0][0] - 0.866_025_45).abs() < 1e-6);
    assert!((m[0][1] - 0.5).abs() < 1e-6);
    assert!((m[1][0] - (-0.500_000_06)).abs() < 1e-6);
    assert!((m[2][2] - 1.0).abs() < 1e-5);
    assert!((m[0][3] - 55.021_42).abs() < 1e-3);
    assert!((m[1][3] - 1.923_307_9).abs() < 1e-4);
    assert!((m[2][3] - 233.498_37).abs() < 1e-3);
    // 平移跨帧恒定、旋转变化 → 证实帧主序(若节主序,平移会跳变)
    assert!((hva.matrices[0][0][0][3] - hva.matrices[1][0][0][3]).abs() < 1e-3);
    assert!((hva.matrices[0][0][0][0] - hva.matrices[1][0][0][0]).abs() > 0.1);
    // matrices[0][1](CYLINDER57)平移
    let m01 = &hva.matrices[0][1];
    assert!((m01[0][3] - (-523.366_4)).abs() < 1e-3);
    assert!((m01[1][3] - (-8.210_362)).abs() < 1e-5);
    assert!((m01[2][3] - 161.296_81).abs() < 1e-3);
}

/// 1tnk.hva:F=1,S=1,名 "B-BODY"(槽尾带垃圾字节);矩阵对角 ≈ 1,带平移。
#[test]
#[ignore]
fn hva_1tnk_single_frame() {
    use ra2_core::format::hva::HvaFile;
    let data = ensure_sample(&["ra2.mix", "local.mix"], "1tnk.hva", "ca89519836d92d31");
    assert_eq!(data.len(), 88);
    let hva = HvaFile::parse(&data).unwrap();
    assert_eq!(hva.frames, 1);
    assert_eq!(hva.section_names, ["B-BODY"]);
    let m = &hva.matrices[0][0];
    assert!((m[0][0] - 0.999_999_9).abs() < 1e-6);
    assert!((m[1][1] - 0.999_999_9).abs() < 1e-6);
    assert!((m[2][2] - 0.999_999_9).abs() < 1e-6);
    assert!((m[0][3] - (-191.924_22)).abs() < 1e-2);
    assert!((m[1][3] - (-0.065_056_756)).abs() < 1e-6);
    assert!((m[2][3] - 81.487_076).abs() < 1e-4);
}
