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
