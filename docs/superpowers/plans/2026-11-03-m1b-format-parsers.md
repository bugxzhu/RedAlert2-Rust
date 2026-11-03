# M1b 真实样本格式解析器(SHP/TMP/CSF/VXL/HVA/AUD + MIX 摘要位)实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 M0+M1a 的格式层基础上补齐六类资产解析器:SHP(TS/RA2 精灵)、TMP(RA2 剧场瓦片)、CSF(字符串表)、AUD(IMA ADPCM 音频)、VXL(体素模型)、HVA(逐帧节变换);同时让 MixArchive 接受 SHA-1 摘要位索引(0x00010000,解锁 38 个摘要位嵌套库)、打磨 ra2-mix CLI 的退出码契约,并以真实资产全链路测试验收。全部解析器对真实游戏字节零 panic、全量 oracle 断言。

**Architecture:** 延续 cargo workspace:`ra2-core`(纯解析库,零外部依赖)新增 `format/{rle_zero_ts,shp,tmp,csf,aud,vxl,hva}.rs`;MIX 摘要位在既有 `format/mix.rs` 内小幅扩展;真实资产测试统一放 `crates/core/tests/format_samples.rs`(自愈式样本提取:文件缺失或哈希不符时从真实 MIX 链按名重提,先写临时名再原子替换,并行测试安全)。工作分支 `feat/m1b-format-parsers`(已检出,基于 main 3b827d2)。

**Tech Stack:** Rust stable(edition 2021),零第三方依赖,GitHub Actions CI(沿用 M0 的 ci.yml)。

**规格依据(计划中所有字节级常量的出处;实现时不需要再考证):**

三份研究文档(权威,均经真实样本逐字节验证):

- `docs/superpowers/research/2026-11-03-shp-tmp-layouts.md`(SHP §1、RA2 TMP §2)
- `docs/superpowers/research/2026-11-03-csf-aud-layouts.md`(CSF §1、AUD §2)
- `docs/superpowers/research/2026-11-03-vxl-hva-layouts.md`(VXL §1、HVA §2)

本计划编写时(2026-11-03 晚)又对全部测试常量做了一轮独立字节取证(对 `E:\hongjing\Reunion 2023` 只读脚本核验,17/17 链路哈希闭合)。新证据与两处研究文档勘误如下,**冲突一律按下述结论执行**:

1. **AUD 步长表不是"标准 IMA 表"**(研究文档 §2.3 措辞有误):XCC `aud_decode.cpp:8-20` 的 `aud_ima_step_table[89]` 是 Westwood 变体——index 27 为 **97**(标准表是 98),尾部为 `7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767`(标准表到 7130…32762)。**以源码为准原样嵌入**;研究文档发布的全部数值(首 8 样本、min/max、终态)在该表下逐一复现。
2. **AUD 终态标注勘误**:研究文档 §2.5 "State after chunk[0] = (50,-77)" 实为**整文件解码完的终态**(chunk[0] 之后实测为 (52,445))。计划以终态与跨块样本窗口断言,不暴露中间状态,无碍。
3. **摘要位嵌套库全部无 LMD**(inventory.tsv 实测 38 个 digest 链逐一验证;isourbmd/urbann/isourb 均无):骨架 Task 1 "reading one tile name" 改为**按首条目 ID 读取**并钉死 id/size(0x8CAA79E6 / 1757768)。
4. **摘要位 iso 库里没有可验证的 TMP**:isourbmd/isotemmd/isodes/urbann/isourb/isosnow 等摘要位库的全部条目既不符合已验证的 RA2 TMP 布局(8B 头+24B 矩形项)也不符合 TS TMP。研究文档验证的两个 .sno 瓦片实际位于**明文** `ra2.mix → snow.mix`(有 LMD,可按名,343 个 TMP 条目)。骨架 Task 4 的"摘要位剧场库取一瓦片"改为 **snow.mix 按名甲骨文**;摘要位链路由 Task 1/Task 10 在容器层覆盖;摘要位 iso 库内容识别登记为 M2(地图里程碑)取证任务。
5. **isourbmd 的 20 字节"摘要"槽实测为结构化值** `[1,1,60,30,20]`(5×i32 LE),不像 SHA-1 随机字节;但 `body_size == 文件长 - (10 + count*12 + 20)` 精确成立,证明 +20 偏移正确。解析器照 XCC 语义跳过不校验(零依赖,不引入 SHA-1;摘要内容校验留待未来另立项)。
6. **lobrdb08.sno 哈希疑虑解除**:当前磁盘字节 SHA-256 = `7a1309fc8c192d82…` 与研究文档 §6 记录一致(2026-11-03 复测),计划按此常量断言。
7. **真实样本来源链**(全部按名解析验证通过,大小即文件字节数):

| 样本 | MIX 链(逐级按名) | SHA-256 前 16 hex |
|---|---|---|
| pwrlvl.shp | ra2.mix → sidec01.mix | e0837c1520ff9538 |
| mindanimr.shp | ra2md.mix → conqmd.mix | ef4617cf48248f0c |
| gunfirey.shp | ra2md.mix → conqmd.mix | 4425f5bdf013e1a9 |
| burnt01.sno | ra2.mix → snow.mix | bc1a23700e17563f |
| lobrdb08.sno | ra2.mix → snow.mix | 7a1309fc8c192d82 |
| mpyscrn.pal | ra2md.mix → localmd.mix | 62cc843355a1bcc9 |
| key.ini | ra2.mix → local.mix | ad7fba6d2737ed9b |
| schp.hva | ra2md.mix → localmd.mix | 68f97095812ab96a |
| ra2md.csf | langmd.mix | 8a0705c9694c4376 |
| text1.aud | ra2.mix → local.mix | 99470c6ce81ebe53 |
| 1tnk.vxl | ra2.mix → local.mix | 45e3bb0e6413589f |
| 1tnkbarl.vxl | ra2.mix → local.mix | f45b93d65a2774fc |
| 1tnk.hva | ra2.mix → local.mix | ca89519836d92d31 |
| 1tnkbarl.hva | ra2.mix → local.mix | 2088f15a367a607d |
| isourbmd.mix(摘要位) | ra2md.mix | 10e3e119d7040b95 |

8. **真库负样本**(Task 1 的 Unsupported 断言用):`ra2md.mix → loadmd.mix` flags=0x00030000(摘要+加密);`ra2md.mix → sidec02md.mix` flags=0x00020000(Blowfish 加密);`langmd.mix → audiomd.mix` flags=0xBFC002E4(未知高位)。

## Global Constraints

- 许可证:GPL-3.0-only。规格来源为 GPL 的 XCC 源码(triatomic/XCC-Tacitus)与用户自有样本的字节级逆向研究——从语义用 Rust 重写,不逐行翻译;涉及 XCC 的模块头注释注明出处。
- 仓库不提交任何 EA 资产;`fixtures/` 已 gitignore;真实资产测试用环境变量 `RA2_ASSETS_DIR` 指向游戏目录(值 `"E:\hongjing\Reunion 2023"`),测试标 `#[ignore]`。
- 每个 src 文件第一行:`// SPDX-License-Identifier: GPL-3.0-only`
- crate 布局遵循 spec §3;解析器全部接受内存字节切片,零 IO、**零 panic**(所有切片/索引用 ByteReader 或显式边界检查,含对 u32 计数做乘法溢出防护);错误复用 `FormatError`(`Unsupported` = 合法但本版本不支持,`Invalid` = 畸形结构)。
- 确定性:解析器无 HashMap/HashSet;f32 仅用于 VXL/HVA 字段原值的 LE 存取,不做浮点运算决策。
- 终审 carry-forward(a) 在本计划正式关闭:SHP/TMP 行链解码**以声明行宽为输出上限**(行内截断 + 恰好 cx*cy 字节),不存在无界增长。
- 每个任务收尾:`cargo fmt --all`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿后才 commit;涉真实资产的任务另跑 `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored` 全绿。
- commit 用 conventional commits(中文描述可)。
- 零第三方依赖。SHA-256 只出现在测试(自写 FIPS 180-4 实现);解析层对摘要只跳过不校验。
- 本计划不做:Blowfish 加密索引(0x00020000 拒绝)、WS 音频(compression=1 拒绝)、TS 变体 TMP、SHP 阴影帧配对逻辑(YAGNI,view 层职责)、调色板索引上色(渲染里程碑)、INI get_float(rules 里程碑,终审转交项)、摘要位 iso 库内容识别(M2 取证任务)。

---

### Task 1: MixArchive 摘要位(0x00010000)支持 + 真库负样本

**Files:**
- Modify: `crates/core/src/format/mix.rs`(parse 与测试;文档注释)
- Modify: `crates/core/tests/mix_integration.rs`(追加 2 个 `#[ignore]` 测试)

**Interfaces:**
- Consumes: 现有 `MixArchive::parse(data: Vec<u8>) -> Result<Self, FormatError>`、`ByteReader`、`mix_name_id`(均不改签名)
- Produces: `MixArchive::parse` 语义扩展——flags 含 0x00010000 时接受(索引后跳过 20 字节摘要块,`data_start += 20`);flags 含 0x00020000 或任何其他未知位 → `FormatError::Unsupported`。`read`/`find`/`entries` 等公开 API 不变。

- [ ] **Step 1: 写失败测试(替换 `mix.rs` 测试模块中的 `rejects_encrypted_index`,并追加)**

现有 `rejects_encrypted_index` 用 flags=0x00010000 断言 Unsupported——**语义已翻转,必须重写**。测试模块最终状态(在既有 `sample_mix` 助手之后追加助手,测试共 7 个):

```rust
    /// 摘要位样本:flags=0x00010000,索引与数据体之间有 20 字节摘要块。
    fn sample_digest_mix() -> Vec<u8> {
        let id_hello = mix_name_id("HELLO");
        let body: Vec<u8> = b"HELLO".to_vec();
        let mut v = Vec::new();
        v.extend_from_slice(&0x0001_0000u32.to_le_bytes()); // flags: 摘要位
        v.extend_from_slice(&1u16.to_le_bytes()); // count
        v.extend_from_slice(&(body.len() as u32).to_le_bytes()); // body_size
        v.extend_from_slice(&id_hello.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(&[0xABu8; 20]); // 摘要块(内容不校验)
        v.extend_from_slice(&body);
        v
    }

    #[test]
    fn accepts_sha1_digest_flag() {
        let mix = MixArchive::parse(sample_digest_mix()).unwrap();
        assert_eq!(mix.entries().len(), 1);
        assert_eq!(mix.read("HELLO").unwrap(), b"HELLO");
    }

    #[test]
    fn rejects_encrypted_index() {
        let mut data = sample_mix();
        data[0..4].copy_from_slice(&0x0002_0000u32.to_le_bytes());
        assert!(matches!(
            MixArchive::parse(data),
            Err(FormatError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_digest_plus_encrypted() {
        // 加密位优先裁决:0x00030000 报 Unsupported(加密分支)
        let mut data = sample_mix();
        data[0..4].copy_from_slice(&0x0003_0000u32.to_le_bytes());
        assert!(matches!(
            MixArchive::parse(data),
            Err(FormatError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_unknown_flag_bits() {
        let mut data = sample_mix();
        data[0..4].copy_from_slice(&0x0004_0000u32.to_le_bytes());
        assert!(matches!(
            MixArchive::parse(data),
            Err(FormatError::Unsupported(_))
        ));
    }
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p ra2-core mix`
Expected: 编译通过但 `accepts_sha1_digest_flag` 失败(现 parse 对 flags!=0 一律 Unsupported)

- [ ] **Step 3: 实现(整体替换 `parse` 函数与 struct 文档注释)**

`MixArchive` 的文档注释改为:

```rust
/// TS/RA2 时代 MIX 容器(明文索引或 SHA-1 摘要位索引)。
///
/// 布局:`[u32 flags][u16 count][u32 body_size]` + count×12 字节索引项
/// `{id, offset, size}`,随后(若 flags 含 0x00010000)是 20 字节摘要块,
/// 然后是文件体;`data_start = 10 + count*12 (+ 20)`。
/// flags 位语义(XCC mix_file.cpp:35-58;真库负样本见计划规格依据 §8):
/// 0x00010000 = SHA-1 摘要位(本实现只跳过不校验——零依赖,不引入 SHA-1;
/// 实测 isourbmd.mix 的摘要槽内容为结构化值 [1,1,60,30,20],见计划规格依据 §5);
/// 0x00020000 = Blowfish 加密索引(拒绝);其余位未知(拒绝)。
```

`parse` 整体替换为:

```rust
    pub fn parse(data: Vec<u8>) -> Result<Self, FormatError> {
        const FLAG_SHA1_DIGEST: u32 = 0x0001_0000;
        const FLAG_ENCRYPTED: u32 = 0x0002_0000;
        let mut r = ByteReader::new(&data);
        let flags = r.read_u32_le()?;
        if flags & FLAG_ENCRYPTED != 0 {
            return Err(FormatError::Unsupported(format!(
                "加密 MIX 索引暂不支持 (flags=0x{flags:08X})"
            )));
        }
        if flags & !(FLAG_SHA1_DIGEST | FLAG_ENCRYPTED) != 0 {
            return Err(FormatError::Unsupported(format!(
                "未知 MIX 标志位 (flags=0x{flags:08X})"
            )));
        }
        let has_digest = flags & FLAG_SHA1_DIGEST != 0;
        let count = r.read_u16_le()? as usize;
        let _body_size = r.read_u32_le()?;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            entries.push(MixEntry {
                id: r.read_u32_le()?,
                offset: r.read_u32_le()?,
                size: r.read_u32_le()?,
            });
        }
        if has_digest {
            // SHA-1 摘要块:明文索引之后、数据体之前(XCC mix_file.cpp)。
            // 只定位不校验——校验需要 SHA-1 实现,超出零依赖范围。
            r.skip(20)?;
        }
        let data_start = r.position();
        for e in &entries {
            let end = e.offset as u64 + e.size as u64;
            if end > (data.len() - data_start) as u64 {
                return Err(FormatError::Invalid(format!(
                    "条目越界: id=0x{:08X} offset={} size={}",
                    e.id, e.offset, e.size
                )));
            }
        }
        Ok(MixArchive {
            data,
            entries,
            data_start,
        })
    }
```

- [ ] **Step 4: 运行单元测试通过**

Run: `cargo test -p ra2-core mix`
Expected: 7 passed(parses_header_and_index、finds_and_reads_by_name、accepts_sha1_digest_flag、rejects_encrypted_index、rejects_digest_plus_encrypted、rejects_unknown_flag_bits、rejects_entry_out_of_range)

- [ ] **Step 5: 追加真实资产测试(`crates/core/tests/mix_integration.rs` 末尾)**

```rust
/// 真库裁决:ra2md.mix 内嵌的 isourbmd.mix(城市剧场库)flags=0x00010000,
/// 索引后 20 字节摘要块、data_start += 20(body_size == 文件长-714 精确成立)。
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
        let blob = ra2md.read(name).unwrap_or_else(|e| panic!("读 {name}: {e}"));
        match MixArchive::parse(blob.to_vec()) {
            Err(FormatError::Unsupported(_)) => {}
            // MixArchive 未派生 Debug,不能整体 {other:?};按分支报错
            Err(e) => panic!("{name}({why})应报 Unsupported,实际 {e:?}"),
            Ok(_) => panic!("{name}({why})应报 Unsupported,实际解析成功"),
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
```

文件头部 `use ra2_core::format::mix::MixArchive;` 之后追加:

```rust
use ra2_core::format::FormatError;
```

- [ ] **Step 6: 运行全部集成测试**

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core -- --ignored`
Expected: 7 passed(既有 5 + 本任务 2)

- [ ] **Step 7: Commit**

```bash
git add crates/core/
git commit -m "feat(core): MIX 摘要位索引(0x00010000)支持,拒绝加密/未知位"
```

---

### Task 2: 真实样本标准集提取与 SHA-256 甲骨文(fixtures 自愈)

**Files:**
- Create: `crates/core/tests/format_samples.rs`(后续 Task 3-8 的真实资产测试都追加在此文件)

**Interfaces:**
- Consumes: `MixArchive`(含 Task 1 摘要位语义;本任务的标准样本链全是明文库,摘要位只用于 Task 1)
- Produces(供 Task 3-8 的集成测试直接调用):
  - `fn assets_dir() -> std::path::PathBuf`(读 `RA2_ASSETS_DIR`)
  - `fn fixtures_dir() -> std::path::PathBuf`(`<crate>/../../fixtures/samples`)
  - `fn sha16(data: &[u8]) -> String`(SHA-256 前 16 hex;FIPS 180-4 手写实现,仅测试用)
  - `fn read_from_chain(chain: &[&str], name: &str) -> Vec<u8>`(逐级按名下降;名字→ID 走 `mix_name_id`,即 LMD 名单裁决路径)
  - `fn ensure_sample(chain: &[&str], name: &str, expect_sha16: &str) -> Vec<u8>`(本地已有且哈希相符则直接用;否则从链上提取、断言哈希、原子写入 fixtures/samples)

- [ ] **Step 1: 写 `crates/core/tests/format_samples.rs`(完整文件)**

```rust
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
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/samples")
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
    for block in msg.chunks_exact(64) {
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
    (&["ra2.mix", "sidec01.mix"], "pwrlvl.shp", "e0837c1520ff9538"),
    (&["ra2md.mix", "conqmd.mix"], "mindanimr.shp", "ef4617cf48248f0c"),
    (&["ra2md.mix", "conqmd.mix"], "gunfirey.shp", "4425f5bdf013e1a9"),
    (&["ra2.mix", "snow.mix"], "burnt01.sno", "bc1a23700e17563f"),
    (&["ra2.mix", "snow.mix"], "lobrdb08.sno", "7a1309fc8c192d82"),
    (&["ra2md.mix", "localmd.mix"], "mpyscrn.pal", "62cc843355a1bcc9"),
    (&["ra2.mix", "local.mix"], "key.ini", "ad7fba6d2737ed9b"),
    (&["ra2md.mix", "localmd.mix"], "schp.hva", "68f97095812ab96a"),
    (&["langmd.mix"], "ra2md.csf", "8a0705c9694c4376"),
    (&["ra2.mix", "local.mix"], "text1.aud", "99470c6ce81ebe53"),
    (&["ra2.mix", "local.mix"], "1tnk.vxl", "45e3bb0e6413589f"),
    (&["ra2.mix", "local.mix"], "1tnkbarl.vxl", "f45b93d65a2774fc"),
    (&["ra2.mix", "local.mix"], "1tnk.hva", "ca89519836d92d31"),
    (&["ra2.mix", "local.mix"], "1tnkbarl.hva", "2088f15a367a607d"),
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
```

- [ ] **Step 2: 运行提取测试**

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test format_samples -- --ignored`
Expected: 1 passed;`fixtures/samples/` 下出现 14 个与规格依据 §7 哈希一致的文件(已有的 9 个会被断言哈希后复用/覆盖为相同字节)

- [ ] **Step 3: 全套回归**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace && RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored`
Expected: fmt/clippy 零告警;合成 34 passed;ignored 8 passed

- [ ] **Step 4: Commit**

```bash
git add crates/core/tests/format_samples.rs
git commit -m "test(core): 真实样本标准集提取与 SHA-256 甲骨文(fixtures 自愈)"
```

---

### Task 3: RLEZeroTS 行链解码 + SHP 解析器

**Files:**
- Create: `crates/core/src/format/rle_zero_ts.rs`
- Create: `crates/core/src/format/shp.rs`
- Modify: `crates/core/src/format/mod.rs`(追加 `pub mod rle_zero_ts;` 与 `pub mod shp;`)
- Modify: `crates/core/tests/format_samples.rs`(追加 3 个真实资产测试)

**Interfaces:**
- Consumes: `FormatError`、`ByteReader`(Task 2 of plan-1)
- Produces:
  - `pub fn rle_zero_ts_decode(src: &[u8], cx: usize, cy: usize, out: &mut Vec<u8>) -> Result<usize, FormatError>` — 返回写入字节数(Task 4 的 TMP 复用)
  - `pub struct ShpFrameHeader { pub x: i16, pub y: i16, pub cx: u16, pub cy: u16, pub flags: u32, pub rgb: [u8; 3], pub offset: u32 }`
  - `pub struct ShpFrame { pub header: ShpFrameHeader, pub pixels: Vec<u8> }`
  - `pub struct ShpFile { pub cx: u16, pub cy: u16, pub frames: Vec<ShpFrame> }`
  - `ShpFile::parse(data: &[u8]) -> Result<Self, FormatError>`
- 语义要点(研究文档 §1.3-§1.4,勿改):flags bit1(值 2)=RLEZeroTS,否则未压缩原始 `cx*cy` 字节;空帧 = `cx==cy==offset==0`(flags 仍非 0);零游程在行宽 `cx` 处截断、块内剩余 token 照常消耗;容忍行块后 2-7 字节 0 填充;阴影帧靠位置配对(`i >= frames/2`),**本层不实现配对**(YAGNI,view 层职责);行链输出以声明 `cx*cy` 为硬上限(终审 carry-forward a 的关闭点)。

- [ ] **Step 1: 写 `rle_zero_ts.rs` 失败测试(文件含测试一起写,TDD 以 Step 2 的编译失败确认)**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! RLEZeroTS 行链解压(SHP 压缩帧与 RA2 TMP 瓦片共用;语义对齐 XCC
//! `shp_decode.cpp:748-778` 的 `RLEZeroTSDecompress`)。
//!
//! 数据是 `cy` 个背靠背行块:每块 `u16 count`(LE,含自身 2 字节)后跟
//! `count-2` 个数据字节。数据字节非 0 即调色板索引像素;为 0 则下一字节是
//! 零游程(透明)长度。已知编码器怪癖(研究文档 §1.3):
//! 1. 行尾透明像素会多发一个 +1 的零游程——在行宽 `cx` 处截断,块内剩余
//!    token 照常消耗但不输出(XCC 同语义);
//! 2. 行块链之后可能有 2-7 字节 0 填充——`count` 链是唯一权威,自然跳过。

use super::FormatError;

/// 解码 `src` 中的 `cy` 个行块追加到 `out`(行主序,恰好追加 `cx*cy` 字节;
/// 行不足处以透明像素 0 补齐)。返回写入的字节数。
pub fn rle_zero_ts_decode(
    src: &[u8],
    cx: usize,
    cy: usize,
    out: &mut Vec<u8>,
) -> Result<usize, FormatError> {
    let start = out.len();
    let mut sp = 0usize;
    for row in 0..cy {
        if src.len() - sp < 2 {
            return Err(FormatError::UnexpectedEof {
                needed: 2,
                remaining: src.len() - sp,
            });
        }
        let count = u16::from_le_bytes([src[sp], src[sp + 1]]) as usize;
        // 行块下限 4(块头 2 + 至少 2 个数据字节;研究文档 §3 错误清单)
        if count < 4 {
            return Err(FormatError::Invalid(format!(
                "RLEZeroTS 行 {row} 的 count={count} 过小"
            )));
        }
        let data_end = sp + count;
        if data_end > src.len() {
            return Err(FormatError::UnexpectedEof {
                needed: count,
                remaining: src.len() - sp,
            });
        }
        sp += 2;
        let mut x = 0usize;
        while sp < data_end {
            let v = src[sp];
            sp += 1;
            if v == 0 {
                if sp >= data_end {
                    return Err(FormatError::Invalid(format!(
                        "RLEZeroTS 行 {row} 的零游程长度字节缺失"
                    )));
                }
                let run = src[sp] as usize;
                sp += 1;
                let take = run.min(cx - x);
                out.extend(std::iter::repeat_n(0u8, take));
                x += take;
            } else if x < cx {
                out.push(v);
                x += 1;
            }
        }
        // 行未填满(XCC 语义:缓冲区剩余保持透明)
        out.extend(std::iter::repeat_n(0u8, cx - x));
    }
    Ok(out.len() - start)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(src: &[u8], cx: usize, cy: usize) -> Result<Vec<u8>, FormatError> {
        let mut out = Vec::new();
        rle_zero_ts_decode(src, cx, cy, &mut out).map(|_| out)
    }

    #[test]
    fn literals_and_zero_run() {
        // 行宽 4:字面量 'A',零游程 2,'B'
        let src = [0x06, 0x00, b'A', 0x00, 0x02, b'B'];
        assert_eq!(decode(&src, 4, 1).unwrap(), vec![b'A', 0, 0, b'B']);
    }

    #[test]
    fn zero_run_clamped_at_row_width() {
        // 编码器怪癖:行尾零游程多发 1(应写 3,实发 4)——截断到行宽
        let src = [0x05, 0x00, b'A', 0x00, 0x04];
        assert_eq!(decode(&src, 4, 1).unwrap(), vec![b'A', 0, 0, 0]);
    }

    #[test]
    fn rows_are_independent_blocks() {
        let src = [0x04, 0x00, 0x11, 0x22, 0x04, 0x00, 0x33, 0x44];
        assert_eq!(decode(&src, 2, 2).unwrap(), vec![0x11, 0x22, 0x33, 0x44]);
    }

    #[test]
    fn tokens_after_row_full_are_consumed() {
        // 零游程 5 → 截断为行宽 2;块内还剩一个字面量 'Z'——消耗但不输出
        let src = [0x05, 0x00, 0x00, 0x05, b'Z'];
        assert_eq!(decode(&src, 2, 1).unwrap(), vec![0, 0]);
    }

    #[test]
    fn row_underfill_pads_transparent() {
        // 块只有 2 个字面量,行宽 4 → 其余补 0(count 链权威,不足处透明)
        let src = [0x04, 0x00, 0x77, 0x88];
        assert_eq!(decode(&src, 4, 1).unwrap(), vec![0x77, 0x88, 0, 0]);
    }

    #[test]
    fn rejects_count_below_four() {
        let src = [0x02, 0x00];
        assert!(matches!(decode(&src, 4, 1), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_truncated_block() {
        // count 声明 8 字节块但只剩块头
        let src = [0x08, 0x00, 0x11];
        assert!(matches!(
            decode(&src, 4, 1),
            Err(FormatError::UnexpectedEof { .. })
        ));
    }
}
```

- [ ] **Step 2: 写 `shp.rs` 失败测试 + 实现(同文件;先在 `mod.rs` 追加两个 `pub mod`)**

`crates/core/src/format/mod.rs` 在 `pub mod pal;` 之后追加:

```rust
pub mod rle_zero_ts;
pub mod shp;
```

`shp.rs` 完整文件:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! TS/RA2 时代 SHP 精灵(.shp,XCC 称 `shp_ts`)。
//!
//! 布局(研究文档 2026-11-03-shp-tmp-layouts.md §1,真实样本逐字节验证):
//! 8 字节文件头 `{u16 zero=0; u16 cx; u16 cy; u16 frames}` + `frames`×24 字节
//! 帧头 `{i16 x; i16 y; u16 cx; u16 cy; u32 flags; u8 rgb[3]; u8 alpha;
//! u32 zero2; u32 offset}`。`flags` bit1(值 2)=帧数据为 RLEZeroTS 压缩;
//! bit0 观测恒置 1(语义未知,忽略)。空帧约定 `cx==cy==offset==0`(此时
//! flags 仍非 0,勿以 flags 判空)。阴影帧按位置配对(`i >= frames/2`),
//! 本层不做配对——留给 view 层。

use super::bytes::ByteReader;
use super::rle_zero_ts::rle_zero_ts_decode;
use super::FormatError;

/// 单帧帧头(`alpha`/`zero2` 已读入校验/忽略,不存储)。
pub struct ShpFrameHeader {
    pub x: i16,
    pub y: i16,
    pub cx: u16,
    pub cy: u16,
    pub flags: u32,
    /// 每帧 RGB 三元组(XCC 读取但不用于解码)。
    pub rgb: [u8; 3],
    /// 帧数据的绝对文件偏移;空帧为 0。
    pub offset: u32,
}

/// 一帧:帧头 + 解码后的调色板索引(行主序,恰好 `cx*cy` 字节;空帧为空)。
pub struct ShpFrame {
    pub header: ShpFrameHeader,
    pub pixels: Vec<u8>,
}

/// SHP 文件。
pub struct ShpFile {
    pub cx: u16,
    pub cy: u16,
    pub frames: Vec<ShpFrame>,
}

impl ShpFile {
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        let mut r = ByteReader::new(data);
        let zero = r.read_u16_le()?;
        if zero != 0 {
            return Err(FormatError::Invalid(format!("SHP 头 zero={zero},应为 0")));
        }
        let cx = r.read_u16_le()?;
        let cy = r.read_u16_le()?;
        let frame_count = r.read_u16_le()? as usize;
        if frame_count == 0 || frame_count > 10000 {
            return Err(FormatError::Invalid(format!(
                "SHP 帧数 {frame_count} 超出 1..=10000"
            )));
        }
        let mut headers = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            let x = r.read_u16_le()? as i16;
            let y = r.read_u16_le()? as i16;
            let fh_cx = r.read_u16_le()?;
            let fh_cy = r.read_u16_le()?;
            let flags = r.read_u32_le()?;
            let rgb = [r.read_u8()?, r.read_u8()?, r.read_u8()?];
            let _alpha = r.read_u8()?;
            let zero2 = r.read_u32_le()?;
            if zero2 != 0 {
                return Err(FormatError::Invalid(format!(
                    "帧头 zero2={zero2},应为 0"
                )));
            }
            let offset = r.read_u32_le()?;
            headers.push(ShpFrameHeader {
                x,
                y,
                cx: fh_cx,
                cy: fh_cy,
                flags,
                rgb,
                offset,
            });
        }
        let table_end = r.position(); // = 8 + 24*frame_count
        let file_len = data.len();
        // 先做全部结构校验(XCC shp_ts_file.cpp:101-106),再解码
        for h in &headers {
            let empty = h.cx == 0 && h.cy == 0 && h.offset == 0;
            if empty {
                continue;
            }
            if (h.offset as usize) < table_end || (h.offset as usize) >= file_len {
                return Err(FormatError::Invalid(format!(
                    "帧数据偏移 {} 越界(表尾 {table_end},文件长 {file_len})",
                    h.offset
                )));
            }
            if i32::from(h.x) + i32::from(h.cx) > i32::from(cx)
                || i32::from(h.y) + i32::from(h.cy) > i32::from(cy)
            {
                return Err(FormatError::Invalid(format!(
                    "帧矩形 ({},{},{},{}) 超出画布 {cx}x{cy}",
                    h.x, h.y, h.cx, h.cy
                )));
            }
        }
        // 每个压缩帧的数据上界:此后第一个更大的帧偏移(空帧 offset=0 不参与)
        // 或文件尾;两者之间可能有 2-7 字节 0 填充,count 链是唯一权威。
        let mut ends = vec![file_len; frame_count];
        for (i, h) in headers.iter().enumerate() {
            let start = h.offset as usize;
            if let Some(o) = headers[i + 1..]
                .iter()
                .map(|n| n.offset as usize)
                .find(|&o| o > start)
            {
                ends[i] = o;
            }
        }
        let mut frames = Vec::with_capacity(frame_count);
        for (h, end) in headers.into_iter().zip(ends) {
            let empty = h.cx == 0 && h.cy == 0 && h.offset == 0;
            let pixels = if empty {
                Vec::new()
            } else if h.flags & 2 != 0 {
                // 压缩帧:RLEZeroTS 行链
                let start = h.offset as usize;
                let src = data.get(start..end).ok_or_else(|| {
                    FormatError::Invalid(format!("帧数据区间 {start}..{end} 越界"))
                })?;
                let mut pixels = Vec::with_capacity(h.cx as usize * h.cy as usize);
                rle_zero_ts_decode(src, h.cx as usize, h.cy as usize, &mut pixels)?;
                pixels
            } else {
                // 未压缩帧:恰好 cx*cy 字节原始调色板索引
                let start = h.offset as usize;
                let need = h.cx as usize * h.cy as usize;
                let src = data.get(start..start + need).ok_or_else(|| {
                    FormatError::Invalid(format!(
                        "未压缩帧需要 {need} 字节,文件仅剩 {}",
                        file_len - start
                    ))
                })?;
                src.to_vec()
            };
            frames.push(ShpFrame { header: h, pixels });
        }
        Ok(ShpFile { cx, cy, frames })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_header(x: i16, y: i16, cx: u16, cy: u16, flags: u32, offset: u32) -> [u8; 24] {
        let mut b = [0u8; 24];
        b[0..2].copy_from_slice(&x.to_le_bytes());
        b[2..4].copy_from_slice(&y.to_le_bytes());
        b[4..6].copy_from_slice(&cx.to_le_bytes());
        b[6..8].copy_from_slice(&cy.to_le_bytes());
        b[8..12].copy_from_slice(&flags.to_le_bytes());
        // rgb(3) + alpha(1) + zero2(4) 保持 0
        b[20..24].copy_from_slice(&offset.to_le_bytes());
        b
    }

    fn shp_bytes(canvas: (u16, u16), headers: &[[u8; 24]], body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&0u16.to_le_bytes()); // zero
        v.extend_from_slice(&canvas.0.to_le_bytes());
        v.extend_from_slice(&canvas.1.to_le_bytes());
        v.extend_from_slice(&(headers.len() as u16).to_le_bytes());
        for h in headers {
            v.extend_from_slice(h);
        }
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn parses_uncompressed_frame() {
        // 画布 4x2,1 帧 4x2 未压缩,数据紧跟表尾(offset=32)
        let body = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        let data = shp_bytes((4, 2), &[frame_header(0, 0, 4, 2, 1, 32)], &body);
        let shp = ShpFile::parse(&data).unwrap();
        assert_eq!((shp.cx, shp.cy), (4, 2));
        assert_eq!(shp.frames.len(), 1);
        assert_eq!(shp.frames[0].header.flags, 1);
        assert_eq!(shp.frames[0].pixels, body);
    }

    #[test]
    fn parses_compressed_frame_with_clamp() {
        // 画布 4x4,1 帧 4x2 压缩(flags=3):行 0 零游程 1 后行未满补透明;
        // 行 1 的尾零游程多发 1,截断到行宽
        let body: Vec<u8> = vec![0x05, 0x00, 0xAA, 0x00, 0x01, 0x05, 0x00, 0xBB, 0x00, 0x04];
        let data = shp_bytes((4, 4), &[frame_header(0, 0, 4, 2, 3, 32)], &body);
        let shp = ShpFile::parse(&data).unwrap();
        let f = &shp.frames[0];
        assert_eq!(f.header.flags, 3);
        assert_eq!(f.pixels, vec![0xAA, 0, 0, 0, 0xBB, 0, 0, 0]);
    }

    #[test]
    fn empty_frame_has_no_pixels() {
        // 帧 0 未压缩 2x1;帧 1 空帧(cx=cy=offset=0,flags 仍非 0)
        let body = [0x55, 0x66];
        let data = shp_bytes(
            (4, 2),
            &[
                frame_header(0, 0, 2, 1, 1, 56),
                frame_header(0, 0, 0, 0, 3, 0),
            ],
            &body,
        );
        let shp = ShpFile::parse(&data).unwrap();
        assert_eq!(shp.frames.len(), 2);
        assert_eq!(shp.frames[0].pixels, body);
        assert!(shp.frames[1].pixels.is_empty());
        assert_eq!(shp.frames[1].header.flags, 3);
    }

    #[test]
    fn rejects_bad_zero_field() {
        let mut data = shp_bytes((4, 2), &[frame_header(0, 0, 4, 2, 1, 32)], &[0u8; 8]);
        data[0..2].copy_from_slice(&7u16.to_le_bytes());
        assert!(matches!(ShpFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_zero_frames() {
        let data = shp_bytes((4, 2), &[], &[]);
        assert!(matches!(ShpFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_frame_out_of_canvas() {
        // 4x2 的帧放在 x=3 → 超出画布 4
        let data = shp_bytes((4, 2), &[frame_header(3, 0, 4, 2, 1, 32)], &[0u8; 8]);
        assert!(matches!(ShpFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_offset_before_table() {
        // offset 指向帧头表内部
        let data = shp_bytes((4, 2), &[frame_header(0, 0, 4, 2, 1, 16)], &[0u8; 8]);
        assert!(matches!(ShpFile::parse(&data), Err(FormatError::Invalid(_))));
    }
}
```

- [ ] **Step 3: 运行单元测试通过**

Run: `cargo test -p ra2-core rle_zero_ts && cargo test -p ra2-core shp`
Expected: 各 7 passed

- [ ] **Step 4: 追加真实资产测试(`crates/core/tests/format_samples.rs` 末尾)**

```rust
/// pwrlvl.shp:最小 1 帧未压缩 SHP(研究文档 §1.5;像素拆分经哈希钉死样本
/// 直读复核,勘误为 fe×10 00×4 fe×2)。
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
    assert_eq!((f.header.x, f.header.y, f.header.cx, f.header.cy), (0, 0, 8, 2));
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
            (f.header.x, f.header.y, f.header.cx, f.header.cy, f.header.offset),
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
```

- [ ] **Step 5: 运行真实资产测试**

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core shp -- --ignored`
Expected: 3 passed(真实 3;单元 7 已由 Step 3 覆盖,--ignored 不重复运行)

- [ ] **Step 6: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 合成 48 passed

```bash
git add crates/core/
git commit -m "feat(core): RLEZeroTS 行链解码与 SHP 解析器"
```

---

### Task 4: RA2 剧场 TMP 瓦片解析器

**Files:**
- Create: `crates/core/src/format/tmp.rs`
- Modify: `crates/core/src/format/mod.rs`(追加 `pub mod tmp;`)
- Modify: `crates/core/tests/format_samples.rs`(追加 3 个真实资产测试)

**Interfaces:**
- Consumes: `rle_zero_ts_decode`(Task 3)、`ByteReader`、`FormatError`
- Produces:
  - `pub struct TmpRa2Tile { pub x: i16, pub y: i16, pub cx: u16, pub cy: u16, pub unknown_a: u32, pub unknown_d: u16, pub radar: [u8; 3], pub data_offset: u32 }`
  - `pub struct TmpRa2TileDecoded { pub header: TmpRa2Tile, pub pixels: Vec<u8> }`
  - `pub struct TmpRa2File { pub width: u16, pub height: u16, pub tiles: Vec<TmpRa2TileDecoded> }`
  - `TmpRa2File::parse(data: &[u8]) -> Result<Self, FormatError>`
- 语义要点(研究文档 §2;这是 RA2 .sno 布局、从用户自有样本逆向,NOT XCC 的 TS TMP——代码注释必须写明):8B 头 `{u16 zero=0; u16 width; u16 height; u16 tiles}` + tiles×24B 矩形项 + RLEZeroTS 数据(与 SHP 同方案、同怪癖);空瓦片 = `cx==cy==data_offset==0`(三者须同时为 0 或同时非 0);图形矩形任意摆放(如 lobrdb08 是 180×120 画布内 120×69 菱形),不逐瓦片展开菱形。未知字段按名保留(`unknown_a`/`unknown_d`/`radar`),不设 `reserved: [u8; N]`。

- [ ] **Step 1: 写 `tmp.rs`(测试 + 实现,Step 2 以编译失败确认 TDD 起点)**

`crates/core/src/format/mod.rs` 追加:

```rust
pub mod tmp;
```

`tmp.rs` 完整文件:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! RA2 剧场地形瓦片容器(.sno/.tem/.urb;RA2 isometric 布局)。
//!
//! 注意:这是从用户自有样本逆向得到的 RA2 剧场布局(研究文档
//! 2026-11-03-shp-tmp-layouts.md §2;XCC 没有 RA2 TMP 读取器)。它与 XCC 的
//! TS TMP(`t_tmp_ts_header`,16 字节 i32 头 + 52 字节瓦片头)不是同一种
//! 容器,勿混用;日后若要支持 TS 瓦片应另立模块并以头部判别。
//!
//! 布局(两个 .sno 样本完全自洽,`data_start = 8 + 24*tiles` 线性精确成立):
//! 8 字节头 `{u16 zero=0; u16 width; u16 height; u16 tiles}` +
//! `tiles`×24 字节矩形项 `{i16 x; i16 y; u16 cx; u16 cy; u32 unknown_a;
//! u16 _b; u16 unknown_d; u8 radar[3]; u8 _pad; u32 data_offset}`
//! (勘误:初稿字段清单多列一个 `u16 _c` 凑成 26 字节,与 24 字节步长矛盾;
//! radar 实测在 +0x10,unknown_d 在 +0x0E,data_offset=8+24·N 线性精确)。
//! 图形数据为 RLEZeroTS 行链(与 SHP 同方案,含同样的 +1 零游程怪癖);
//! 空瓦片约定 `cx==cy==data_offset==0`。

use super::bytes::ByteReader;
use super::rle_zero_ts::rle_zero_ts_decode;
use super::FormatError;

/// 单个矩形项(存储字段;`_b`/`_c`/`_pad` 观测恒 0,跳过不存储)。
pub struct TmpRa2Tile {
    pub x: i16,
    pub y: i16,
    pub cx: u16,
    pub cy: u16,
    /// 观测:多数图形项 3、个别 4;空项也非 0(语义未知,按名保留)。
    pub unknown_a: u32,
    /// 观测:空项 0;图形项 1..=6(疑似地形/坡度类别)。
    pub unknown_d: u16,
    /// 疑似小地图 RGB(观测:除 lobrdb08 t0 = `00 fb 12` 外恒 0)。
    pub radar: [u8; 3],
    /// RLE 数据的绝对文件偏移;0 = 空瓦片。
    pub data_offset: u32,
}

/// 一个矩形项 + 解码后的调色板索引(行主序,恰好 `cx*cy` 字节;空瓦片为空)。
pub struct TmpRa2TileDecoded {
    pub header: TmpRa2Tile,
    pub pixels: Vec<u8>,
}

/// RA2 剧场瓦片文件:图形矩形可任意摆放(`x,y` 为画布内位置),不逐瓦片展开菱形。
pub struct TmpRa2File {
    pub width: u16,
    pub height: u16,
    pub tiles: Vec<TmpRa2TileDecoded>,
}

impl TmpRa2File {
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        let mut r = ByteReader::new(data);
        let zero = r.read_u16_le()?;
        if zero != 0 {
            return Err(FormatError::Invalid(format!("TMP 头 zero={zero},应为 0")));
        }
        let width = r.read_u16_le()?;
        let height = r.read_u16_le()?;
        let tiles_count = r.read_u16_le()? as usize;
        if tiles_count == 0 {
            return Err(FormatError::Invalid("TMP 瓦片数为 0".to_string()));
        }
        let mut headers = Vec::with_capacity(tiles_count);
        for _ in 0..tiles_count {
            let x = r.read_u16_le()? as i16;
            let y = r.read_u16_le()? as i16;
            let cx = r.read_u16_le()?;
            let cy = r.read_u16_le()?;
            let unknown_a = r.read_u32_le()?;
            // 只有一个零 u16 间隙(_b)——勘误:初稿多读一个 _c 会变 26 字节步长,
            // 与 data_offset=8+24·N 线性算术及 radar@+0x10 实测矛盾。
            let _b = r.read_u16_le()?;
            let unknown_d = r.read_u16_le()?;
            let radar = [r.read_u8()?, r.read_u8()?, r.read_u8()?];
            let _pad = r.read_u8()?;
            let data_offset = r.read_u32_le()?;
            headers.push(TmpRa2Tile {
                x,
                y,
                cx,
                cy,
                unknown_a,
                unknown_d,
                radar,
                data_offset,
            });
        }
        let table_end = r.position(); // = 8 + 24*tiles
        let file_len = data.len();
        for h in &headers {
            let empty = h.cx == 0 && h.cy == 0 && h.data_offset == 0;
            if empty {
                continue;
            }
            if h.cx == 0 || h.cy == 0 || h.data_offset == 0 {
                return Err(FormatError::Invalid(
                    "瓦片矩形与数据偏移必须同时为 0(空)或同时非 0".to_string(),
                ));
            }
            if (h.data_offset as usize) < table_end || (h.data_offset as usize) >= file_len {
                return Err(FormatError::Invalid(format!(
                    "瓦片数据偏移 {} 越界(表尾 {table_end},文件长 {file_len})",
                    h.data_offset
                )));
            }
            if i32::from(h.x) + i32::from(h.cx) > i32::from(width)
                || i32::from(h.y) + i32::from(h.cy) > i32::from(height)
            {
                return Err(FormatError::Invalid(format!(
                    "瓦片矩形 ({},{},{},{}) 超出画布 {width}x{height}",
                    h.x, h.y, h.cx, h.cy
                )));
            }
        }
        let mut tiles = Vec::with_capacity(tiles_count);
        for h in headers {
            let empty = h.cx == 0 && h.cy == 0 && h.data_offset == 0;
            let pixels = if empty {
                Vec::new()
            } else {
                // RLE 行链以 count 自链,尾随 0 填充自然不被读取;传到文件尾即可
                let src = data.get(h.data_offset as usize..).ok_or_else(|| {
                    FormatError::Invalid(format!("瓦片数据偏移 {} 越界", h.data_offset))
                })?;
                let mut pixels = Vec::with_capacity(h.cx as usize * h.cy as usize);
                rle_zero_ts_decode(src, h.cx as usize, h.cy as usize, &mut pixels)?;
                pixels
            };
            tiles.push(TmpRa2TileDecoded { header: h, pixels });
        }
        Ok(TmpRa2File {
            width,
            height,
            tiles,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile_entry(
        x: i16,
        y: i16,
        cx: u16,
        cy: u16,
        unknown_a: u32,
        unknown_d: u16,
        offset: u32,
    ) -> [u8; 24] {
        let mut b = [0u8; 24];
        b[0..2].copy_from_slice(&x.to_le_bytes());
        b[2..4].copy_from_slice(&y.to_le_bytes());
        b[4..6].copy_from_slice(&cx.to_le_bytes());
        b[6..8].copy_from_slice(&cy.to_le_bytes());
        b[8..12].copy_from_slice(&unknown_a.to_le_bytes());
        // _b/_c 保持 0
        b[14..16].copy_from_slice(&unknown_d.to_le_bytes());
        // radar/_pad 保持 0
        b[20..24].copy_from_slice(&offset.to_le_bytes());
        b
    }

    fn tmp_bytes(w: u16, h: u16, headers: &[[u8; 24]], body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&0u16.to_le_bytes()); // zero
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.extend_from_slice(&(headers.len() as u16).to_le_bytes());
        for t in headers {
            v.extend_from_slice(t);
        }
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn parses_tiles_and_decodes_rle() {
        // 画布 60x60:图形瓦片 4x2 @数据 56(表尾,8+24*2),空瓦片跟随。
        // 勘误:初稿 body `05 00 31 00 01 06 00 32 00 04` 行 1 块 count=6 但
        // 只剩 5 字节(EOF 矛盾);按断言像素反推自洽编码:
        // 行 0 = 31 + 零游程1 + 01(count=6),行 1 = 32 + 零游程4 截断(count=5)。
        let body: Vec<u8> = vec![
            0x06, 0x00, 0x31, 0x00, 0x01, 0x01, 0x05, 0x00, 0x32, 0x00, 0x04,
        ];
        let data = tmp_bytes(
            60,
            60,
            &[
                tile_entry(10, 20, 4, 2, 3, 1, 56),
                tile_entry(0, 0, 0, 0, 4, 0, 0),
            ],
            &body,
        );
        let tmp = TmpRa2File::parse(&data).unwrap();
        assert_eq!((tmp.width, tmp.height), (60, 60));
        assert_eq!(tmp.tiles.len(), 2);
        let t0 = &tmp.tiles[0];
        assert_eq!((t0.header.x, t0.header.y), (10, 20));
        assert_eq!(t0.header.unknown_a, 3);
        assert_eq!(t0.header.unknown_d, 1);
        assert_eq!(t0.header.data_offset, 56);
        assert_eq!(t0.pixels, vec![0x31, 0, 0x01, 0, 0x32, 0, 0, 0]);
        assert!(tmp.tiles[1].pixels.is_empty());
    }

    #[test]
    fn rejects_bad_zero_field() {
        let mut data = tmp_bytes(60, 60, &[tile_entry(0, 0, 4, 2, 3, 1, 56)], &[0u8; 8]);
        data[0..2].copy_from_slice(&9u16.to_le_bytes());
        assert!(matches!(TmpRa2File::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_zero_tiles() {
        let data = tmp_bytes(60, 60, &[], &[]);
        assert!(matches!(TmpRa2File::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_inconsistent_empty_tile() {
        // cx>0 但 data_offset=0:三字段必须同时为 0 或同时非 0
        let data = tmp_bytes(60, 60, &[tile_entry(0, 0, 4, 2, 3, 1, 0)], &[]);
        assert!(matches!(TmpRa2File::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_rect_out_of_canvas() {
        let data = tmp_bytes(60, 60, &[tile_entry(57, 0, 4, 2, 3, 1, 56)], &[0u8; 8]);
        assert!(matches!(TmpRa2File::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_offset_before_table() {
        let data = tmp_bytes(60, 60, &[tile_entry(0, 0, 4, 2, 3, 1, 16)], &[0u8; 8]);
        assert!(matches!(TmpRa2File::parse(&data), Err(FormatError::Invalid(_))));
    }
}
```

- [ ] **Step 2: 运行确认失败再通过**

Run: `cargo test -p ra2-core tmp`
Expected: 6 passed

- [ ] **Step 3: 追加真实资产测试(`crates/core/tests/format_samples.rs` 末尾)**

```rust
/// burnt01.sno:120x120 画布,2 项(1 图形 29x17 灼痕 + 1 空)。
/// 像素向量:343 个非零;首 16 像素 = 10 零 + b3 b7 bb 00 00 b4(2026-11-03 取证)。
#[test]
#[ignore]
fn tmp_burnt01_sno_blob() {
    use ra2_core::format::tmp::TmpRa2File;
    let data = ensure_sample(&["ra2.mix", "snow.mix"], "burnt01.sno", "bc1a23700e17563f");
    assert_eq!(data.len(), 520);
    let tmp = TmpRa2File::parse(&data).unwrap();
    assert_eq!((tmp.width, tmp.height), (120, 120));
    assert_eq!(tmp.tiles.len(), 2);
    let t0 = &tmp.tiles[0];
    assert_eq!(
        (t0.header.x, t0.header.y, t0.header.cx, t0.header.cy),
        (44, 64, 29, 17)
    );
    assert_eq!(t0.header.unknown_a, 3);
    assert_eq!(t0.header.unknown_d, 1);
    assert_eq!(t0.header.radar, [0, 0, 0]);
    assert_eq!(t0.header.data_offset, 56);
    assert_eq!(t0.pixels.len(), 29 * 17);
    assert_eq!(t0.pixels.iter().filter(|&&p| p != 0).count(), 343);
    assert_eq!(
        &t0.pixels[..16],
        &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xB3, 0xB7, 0xBB, 0, 0, 0xB4]
    );
    let t1 = &tmp.tiles[1];
    assert_eq!((t1.header.cx, t1.header.cy, t1.header.data_offset), (0, 0, 0));
    assert!(t1.pixels.is_empty());
}

/// lobrdb08.sno:180x120 画布,6 项(1 图形 120x69 菱形 + 5 空);t0 带小地图绿。
#[test]
#[ignore]
fn tmp_lobrdb08_sno_diamond() {
    use ra2_core::format::tmp::TmpRa2File;
    let data = ensure_sample(&["ra2.mix", "snow.mix"], "lobrdb08.sno", "7a1309fc8c192d82");
    assert_eq!(data.len(), 3360);
    let tmp = TmpRa2File::parse(&data).unwrap();
    assert_eq!((tmp.width, tmp.height), (180, 120));
    assert_eq!(tmp.tiles.len(), 6);
    let t0 = &tmp.tiles[0];
    assert_eq!((t0.header.cx, t0.header.cy, t0.header.data_offset), (0, 0, 0));
    assert_eq!(t0.header.unknown_a, 4);
    assert_eq!(t0.header.radar, [0x00, 0xFB, 0x12]); // 小地图亮绿
    let t1 = &tmp.tiles[1];
    assert_eq!(
        (t1.header.x, t1.header.y, t1.header.cx, t1.header.cy),
        (30, 20, 120, 69)
    );
    assert_eq!(t1.header.unknown_a, 3);
    assert_eq!(t1.header.unknown_d, 6);
    assert_eq!(t1.header.data_offset, 152);
    assert_eq!(t1.pixels.len(), 120 * 69);
    assert_eq!(t1.pixels.iter().filter(|&&p| p != 0).count(), 2788);
    for t in &tmp.tiles[2..] {
        assert!(t.pixels.is_empty());
    }
}

/// 真库甲骨文:直接从 ra2.mix → snow.mix 按名提取瓦片(该库有 LMD,名单含
/// 全部 .sno 瓦片;研究文档的两个样本即出自此库)。
/// 注:任务骨架原想从摘要位剧场库(isourbmd 等)取瓦片,但实测 38 个摘要位
/// 嵌套库全部无 LMD、其条目也不符合已验证的 RA2 TMP 布局(计划规格依据 §4);
/// 摘要位链路由 Task 1 与 Task 10 的容器层测试覆盖。
#[test]
#[ignore]
fn tmp_tile_reads_by_name_from_snow_mix() {
    use ra2_core::format::tmp::TmpRa2File;
    let data = read_from_chain(&["ra2.mix", "snow.mix"], "burnt01.sno");
    assert_eq!(sha16(&data), "bc1a23700e17563f");
    let tmp = TmpRa2File::parse(&data).unwrap();
    assert_eq!(tmp.tiles.len(), 2);
    assert_eq!(tmp.tiles[0].pixels.len(), 29 * 17);
}
```

- [ ] **Step 4: 运行真实资产测试**

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core tmp -- --ignored`
Expected: 3 passed(真实 3;单元 6 已由 Step 2 覆盖,--ignored 不重复运行)

- [ ] **Step 5: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 合成 54 passed

```bash
git add crates/core/
git commit -m "feat(core): RA2 剧场 TMP 瓦片解析器"
```

---

### Task 5: CSF 字符串表解析器

**Files:**
- Create: `crates/core/src/format/csf.rs`
- Modify: `crates/core/src/format/mod.rs`(追加 `pub mod csf;`)
- Modify: `crates/core/tests/format_samples.rs`(追加 1 个真实资产测试)

**Interfaces:**
- Consumes: `ByteReader`、`FormatError`
- Produces:
  - `pub const CSF_MAGIC: u32`(0x4353_4620,磁盘字节 `" FSC"`)
  - `pub struct CsfLabel { pub name: String, pub value: Option<String>, pub extra: Option<Vec<u8>> }`
  - `pub struct CsfDocument { pub labels: Vec<CsfLabel> }`
  - `CsfDocument::parse(data: &[u8]) -> Result<Self, FormatError>`、`CsfDocument::find(&self, name: &str) -> Option<&CsfLabel>`
- 语义要点(研究文档 §1,勿改):只信 `count1`(唯一循环边界);`flags2/language` 不透明不校验;`flags&1==0` 的无值标签 → `value=None`;空值标签(cb_str=0)是 `Some("")`;值 = u16 LE 逐码元 XOR 0xFFFF 后按 UTF-16 解释(lossy,零 panic);`WRTS` 携带未取反原始 ANSI extra;标签流后必须恰为 EOF(硬错误,研究文档建议严于 XCC 的宽容截断);重复名保留文件顺序、查找取首个(ra2md.csf 实测无重复,文档化该选择)。

- [ ] **Step 1: 写 `csf.rs`(测试 + 实现)**

`crates/core/src/format/mod.rs` 追加:

```rust
pub mod csf;
```

`csf.rs` 完整文件:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! CSF 本地化字符串表(文件魔数在盘上为 ASCII `" FSC"`)。
//!
//! 布局(研究文档 2026-11-03-csf-aud-layouts.md §1;XCC `csf_file.{h,cpp}`
//! 双向验证,ra2md.csf 全量走查通过):
//! 24 字节头 `{i32 magic=" FSC"=0x4353_4620; i32 flags1(不透明,勿校验);
//! i32 count1=标签数(唯一循环边界);i32 count2(只写不读,忽略);i32 zero;
//! i32 language(不透明)}`。随后 `count1` 个标签背靠背无填充:
//! `{i32 " LBL"=0x4C42_4C20; i32 flags(bit0=有值); i32 name_len; name 原始
//! 字节(不取反);[i32 " RTS"=0x5354_5220 或 "WRTS"=0x5354_5257;
//! i32 cb_str(UTF-16 码元数,不是字节数);2*cb_str 字节 UTF-16LE 逐码元
//! 取反存储;WRTS 另有 i32 extra_len + extra 原始字节]}`。
//! 字节恒等式(写入器):file_size == 24 + Σ(20 + name_len + 2*cb_str [+4+extra])。
//! 解码:u16 LE 读出后逐码元 XOR 0xFFFF,按 UTF-16 解释(lossy:不配对代理
//! 替换为 U+FFFD——M1b 无重写需求,选择有损以保零 panic)。
//! 重复名:保留文件顺序(查找取首个);ra2md.csf 实测无重复。

use super::bytes::ByteReader;
use super::FormatError;

/// 文件魔数:磁盘字节 `20 46 53 43`(" FSC")按 u32 LE 读出。
pub const CSF_MAGIC: u32 = 0x4353_4620;
const LBL_MAGIC: u32 = 0x4C42_4C20; // " LBL"
const STR_MAGIC: u32 = 0x5354_5220; // " RTS"
const STRW_MAGIC: u32 = 0x5354_5257; // "WRTS"

/// 一个标签。`value` 为 `None` 仅当 flags bit0==0(无值标签);
/// 空值标签是 `Some("")`。`extra` 仅 'WRTS' 标签存在(原始 ANSI 字节,不取反)。
pub struct CsfLabel {
    pub name: String,
    pub value: Option<String>,
    pub extra: Option<Vec<u8>>,
}

/// 解析后的字符串表(保留文件顺序)。
pub struct CsfDocument {
    pub labels: Vec<CsfLabel>,
}

impl CsfDocument {
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        let mut r = ByteReader::new(data);
        let magic = r.read_u32_le()?;
        if magic != CSF_MAGIC {
            return Err(FormatError::Invalid(format!(
                "CSF 魔数不符: 0x{magic:08X}"
            )));
        }
        let _flags1 = r.read_u32_le()?;
        let count1 = r.read_u32_le()? as usize;
        let _count2 = r.read_u32_le()?;
        let _zero = r.read_u32_le()?;
        let _language = r.read_u32_le()?;
        let mut labels = Vec::new();
        for i in 0..count1 {
            let lbl = r.read_u32_le()?;
            if lbl != LBL_MAGIC {
                return Err(FormatError::Invalid(format!(
                    "标签 {i} 魔数不符: 0x{lbl:08X}"
                )));
            }
            let flags = r.read_u32_le()?;
            let name_len = r.read_u32_le()? as usize;
            let name = String::from_utf8_lossy(r.read_bytes(name_len)?).into_owned();
            let (value, extra) = if flags & 1 != 0 {
                let str_magic = r.read_u32_le()?;
                let cb_str = r.read_u32_le()? as usize;
                let need = cb_str
                    .checked_mul(2)
                    .ok_or_else(|| FormatError::Invalid("字符串长度溢出".to_string()))?;
                let raw = r.read_bytes(need)?;
                let units: Vec<u16> = raw
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]) ^ 0xFFFF)
                    .collect();
                let value = String::from_utf16_lossy(&units);
                let extra = if str_magic == STRW_MAGIC {
                    let extra_len = r.read_u32_le()? as usize;
                    Some(r.read_bytes(extra_len)?.to_vec())
                } else if str_magic == STR_MAGIC {
                    None
                } else {
                    return Err(FormatError::Invalid(format!(
                        "字符串魔数不符: 0x{str_magic:08X}"
                    )));
                };
                (Some(value), extra)
            } else {
                (None, None)
            };
            labels.push(CsfLabel { name, value, extra });
        }
        if r.remaining() != 0 {
            return Err(FormatError::Invalid(format!(
                "标签流结束后仍有 {} 字节",
                r.remaining()
            )));
        }
        Ok(CsfDocument { labels })
    }

    /// 按名精确查找(大小写敏感;保留文件顺序,取首个)。
    pub fn find(&self, name: &str) -> Option<&CsfLabel> {
        self.labels.iter().find(|l| l.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 值编码:逐 UTF-16 码元取反后按 LE 拼字节(XCC encode 的逆)。
    fn inv_bytes(s: &str) -> Vec<u8> {
        s.encode_utf16()
            .flat_map(|u| (u ^ 0xFFFF).to_le_bytes())
            .collect()
    }

    fn label_blob(name: &str, value: Option<&str>, extra: Option<&[u8]>) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&LBL_MAGIC.to_le_bytes());
        match value {
            Some(s) => {
                v.extend_from_slice(&1u32.to_le_bytes()); // flags bit0=1
                v.extend_from_slice(&(name.len() as u32).to_le_bytes());
                v.extend_from_slice(name.as_bytes());
                let magic = if extra.is_some() {
                    STRW_MAGIC
                } else {
                    STR_MAGIC
                };
                v.extend_from_slice(&magic.to_le_bytes());
                v.extend_from_slice(&(s.encode_utf16().count() as u32).to_le_bytes());
                v.extend_from_slice(&inv_bytes(s));
                if let Some(e) = extra {
                    v.extend_from_slice(&(e.len() as u32).to_le_bytes());
                    v.extend_from_slice(e);
                }
            }
            None => {
                v.extend_from_slice(&0u32.to_le_bytes()); // flags bit0=0:无值
                v.extend_from_slice(&(name.len() as u32).to_le_bytes());
                v.extend_from_slice(name.as_bytes());
            }
        }
        v
    }

    fn csf_bytes(labels: &[Vec<u8>]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&CSF_MAGIC.to_le_bytes());
        v.extend_from_slice(&3u32.to_le_bytes()); // flags1(不透明)
        v.extend_from_slice(&(labels.len() as u32).to_le_bytes()); // count1
        v.extend_from_slice(&(labels.len() as u32).to_le_bytes()); // count2
        v.extend_from_slice(&0u32.to_le_bytes()); // zero
        v.extend_from_slice(&0u32.to_le_bytes()); // language
        for l in labels {
            v.extend_from_slice(l);
        }
        v
    }

    #[test]
    fn decodes_inverted_utf16_value() {
        let data = csf_bytes(&[label_blob("GUI:OK", Some("确定"), None)]);
        let doc = CsfDocument::parse(&data).unwrap();
        assert_eq!(doc.labels.len(), 1);
        assert_eq!(doc.labels[0].name, "GUI:OK");
        assert_eq!(doc.labels[0].value.as_deref(), Some("确定"));
        assert!(doc.labels[0].extra.is_none());
        assert!(doc.find("GUI:OK").is_some());
        assert!(doc.find("GUI:不存在的键").is_none());
    }

    #[test]
    fn label_without_value_maps_to_none() {
        let data = csf_bytes(&[label_blob("GUI:None", None, None)]);
        let doc = CsfDocument::parse(&data).unwrap();
        assert_eq!(doc.labels[0].value, None);
        assert!(doc.labels[0].extra.is_none());
    }

    #[test]
    fn empty_value_label_maps_to_empty_string() {
        // 空值仍花 8 字节('STR ' 魔数 + i32 0),是 Some("") 而非 None
        let data = csf_bytes(&[label_blob("GUI:Blank", Some(""), None)]);
        let doc = CsfDocument::parse(&data).unwrap();
        assert_eq!(doc.labels[0].value.as_deref(), Some(""));
    }

    #[test]
    fn strw_label_carries_extra() {
        let data = csf_bytes(&[label_blob("VOX:ceva001", Some("值"), Some(b"ceva001c"))]);
        let doc = CsfDocument::parse(&data).unwrap();
        assert_eq!(doc.labels[0].value.as_deref(), Some("值"));
        assert_eq!(
            doc.labels[0].extra.as_deref(),
            Some(b"ceva001c".as_slice())
        );
    }

    #[test]
    fn rejects_bad_file_magic() {
        let mut data = csf_bytes(&[label_blob("A", Some("b"), None)]);
        data[0..4].copy_from_slice(&0u32.to_le_bytes());
        assert!(matches!(
            CsfDocument::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_bad_label_magic_midstream() {
        let mut data = csf_bytes(&[label_blob("A", Some("b"), None)]);
        let pos = 24 + 4; // 首标签的 " LBL" 位置
        data[pos..pos + 4].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        assert!(matches!(
            CsfDocument::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_trailing_bytes() {
        let mut data = csf_bytes(&[label_blob("A", Some("b"), None)]);
        data.push(0);
        assert!(matches!(
            CsfDocument::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }
}
```

- [ ] **Step 2: 运行单元测试通过**

Run: `cargo test -p ra2-core csf`
Expected: 7 passed

- [ ] **Step 3: 追加真实资产测试(`crates/core/tests/format_samples.rs` 末尾)**

```rust
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
```

- [ ] **Step 4: 运行真实资产测试**

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core csf -- --ignored`
Expected: 1 passed(真实 1;单元 7 已由前步覆盖,--ignored 不重复运行)

- [ ] **Step 5: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 合成 61 passed

```bash
git add crates/core/
git commit -m "feat(core): CSF 字符串表解析器"
```

---

### Task 6: AUD IMA ADPCM 解码器

**Files:**
- Create: `crates/core/src/format/aud.rs`
- Modify: `crates/core/src/format/mod.rs`(追加 `pub mod aud;`)
- Modify: `crates/core/tests/format_samples.rs`(追加 1 个真实资产测试)

**Interfaces:**
- Consumes: `ByteReader`、`FormatError`
- Produces:
  - `pub const COMPRESSION_IMA: u8`(0x63)
  - `pub struct AudHeader { pub sample_rate: u16, pub size_in: u32, pub size_out: u32, pub flags: u8, pub compression: u8 }`
  - `pub struct AudChunkHeader { pub size_in: u16, pub size_out: u16, pub id: i32 }`
  - `pub struct Aud { pub header: AudHeader, pub chunks: Vec<AudChunkHeader>, pub samples: Vec<i16> }`
  - `Aud::parse(data: &[u8]) -> Result<Self, FormatError>`
- 语义要点(研究文档 §2 + 规格依据 §1/§2 勘误,勿改):步长表用 XCC 变体(勿改"标准"表);低半字节先;状态 (index, sample) 文件级跨块持续,init(0,0) 一次、无块内前导;块 id 只记录不校验;立体声(flags bit0)与 compression=1(WS)→ `Unsupported`;`size_in == len-12`、采样率 8000..=48000、IMA 要求 `flags & 0xFE == 2` → 否则 `Invalid`;Σ块 size_in/size_out 与头部交叉校验;`samples` 用 `Vec::new()` 自然增长(不按未验证的 size_out 预留巨型容量)。

- [ ] **Step 1: 写 `aud.rs`(测试 + 实现)**

`crates/core/src/format/mod.rs` 追加:

```rust
pub mod aud;
```

`aud.rs` 完整文件:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! AUD 音频(IMA ADPCM,compression=0x63)。
//!
//! 布局(研究文档 2026-11-03-csf-aud-layouts.md §2;XCC `aud_file.cpp` /
//! `aud_decode.cpp` 源码逐行核对,text1.aud 全样本验证):
//! 12 字节头 `{u16 samplerate(8000..=48000); u32 size_in(全部块头+载荷);
//! u32 size_out(解码 PCM 字节); i8 flags(bit0 立体声,bit1 16-bit);
//! i8 compression}`。随后背靠背块:`{u16 size_in(载荷字节); u16 size_out
//! (解码字节); i32 id=0x0000DEAF(XCC 从不校验,仅记录)}` + 载荷。
//!
//! IMA 解码要点:
//! - **步长表是 XCC/Westwood 变体,不是标准 IMA 表**(index 27 为 97 非 98,
//!   尾项 32767 非 32762;XCC aud_decode.cpp:8-20 原样嵌入,勿"修正");
//! - 低半字节先(偶数样本取低半字节);
//! - 预测器状态 (index, sample) **文件级、跨块持续**(init(0,0) 一次;
//!   块内无前导,与 WAV IMA 不同;按块复位会从第二块起出垃圾)。
//!
//! 范围外(显式 Unsupported):立体声(flags bit0)、compression=1(WS 编解码器)。

use super::bytes::ByteReader;
use super::FormatError;

pub const COMPRESSION_IMA: u8 = 0x63;
const CHUNK_ID: i32 = 0x0000_DEAF; // 仅记录,不校验(XCC 同)

/// XCC `aud_decode.cpp:8-20` 的步长表(Westwood 变体,勿改成"标准"IMA 表)。
const IMA_STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60,
    66, 73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371,
    408, 449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707,
    1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132,
    7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623,
    27086, 29794, 32767,
];
const IMA_INDEX_ADJUST: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];

/// AUD 文件头(存储字段)。
pub struct AudHeader {
    pub sample_rate: u16,
    pub size_in: u32,
    pub size_out: u32,
    pub flags: u8,
    pub compression: u8,
}

/// 块头(存储字段)。
pub struct AudChunkHeader {
    pub size_in: u16,
    pub size_out: u16,
    pub id: i32,
}

/// 解析并解码后的 AUD。
pub struct Aud {
    pub header: AudHeader,
    pub chunks: Vec<AudChunkHeader>,
    /// 解码后的单声道样本(16-bit PCM,每样本一个 i16)。
    pub samples: Vec<i16>,
}

impl Aud {
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        if data.len() < 12 {
            return Err(FormatError::UnexpectedEof {
                needed: 12,
                remaining: data.len(),
            });
        }
        let mut r = ByteReader::new(data);
        let sample_rate = r.read_u16_le()?;
        let size_in = r.read_u32_le()?;
        let size_out = r.read_u32_le()?;
        let flags = r.read_u8()?;
        let compression = r.read_u8()?;
        if !(8000..=48000).contains(&sample_rate) {
            return Err(FormatError::Invalid(format!(
                "采样率 {sample_rate} 超出 8000..=48000"
            )));
        }
        if size_in as usize != data.len() - 12 {
            return Err(FormatError::Invalid(format!(
                "size_in={size_in} 与文件长不符(应为 {})",
                data.len() - 12
            )));
        }
        if flags & 1 != 0 {
            return Err(FormatError::Unsupported("立体声 AUD 暂不支持".to_string()));
        }
        if compression != COMPRESSION_IMA {
            return Err(FormatError::Unsupported(format!(
                "暂不支持的 AUD 压缩 0x{compression:02X}(仅 IMA 0x63;WS 编解码器不在本里程碑)"
            )));
        }
        if flags & 0xFE != 2 {
            return Err(FormatError::Invalid(format!(
                "IMA AUD flags={flags} 应为 2(16-bit mono)"
            )));
        }
        // 块游走 + 解码(状态跨块持续,init(0,0) 一次)
        let mut chunks = Vec::new();
        let mut samples: Vec<i16> = Vec::new();
        let mut index: i32 = 0;
        let mut sample: i32 = 0;
        let mut sp = 12usize;
        while sp < data.len() {
            if data.len() - sp < 8 {
                return Err(FormatError::UnexpectedEof {
                    needed: 8,
                    remaining: data.len() - sp,
                });
            }
            let size_in_chunk = u16::from_le_bytes([data[sp], data[sp + 1]]);
            let size_out_chunk = u16::from_le_bytes([data[sp + 2], data[sp + 3]]);
            let id = i32::from_le_bytes([
                data[sp + 4],
                data[sp + 5],
                data[sp + 6],
                data[sp + 7],
            ]);
            let payload_start = sp + 8;
            let payload_end = payload_start + size_in_chunk as usize;
            if payload_end > data.len() {
                return Err(FormatError::UnexpectedEof {
                    needed: size_in_chunk as usize,
                    remaining: data.len() - payload_start,
                });
            }
            let payload = &data[payload_start..payload_end];
            chunks.push(AudChunkHeader {
                size_in: size_in_chunk,
                size_out: size_out_chunk,
                id,
            });
            // 每样本 4-bit 码,低半字节先;状态持续
            for i in 0..(size_out_chunk as usize / 2) {
                let byte = payload.get(i >> 1).ok_or_else(|| {
                    FormatError::Invalid("块载荷不足以解码全部样本".to_string())
                })?;
                let code = if i % 2 == 0 { byte & 0x0F } else { byte >> 4 };
                let step = IMA_STEP_TABLE[index as usize];
                let mut delta = step >> 3;
                if code & 1 != 0 {
                    delta += step >> 2;
                }
                if code & 2 != 0 {
                    delta += step >> 1;
                }
                if code & 4 != 0 {
                    delta += step;
                }
                if code & 8 != 0 {
                    sample -= delta;
                    if sample < -32768 {
                        sample = -32768;
                    }
                } else {
                    sample += delta;
                    if sample > 32767 {
                        sample = 32767;
                    }
                }
                index = (index + IMA_INDEX_ADJUST[(code & 7) as usize]).clamp(0, 88);
                samples.push(sample as i16);
            }
            sp = payload_end;
        }
        // 总量交叉校验(研究文档 §2.6.7 建议)
        let sum_in: usize = chunks.iter().map(|c| 8 + c.size_in as usize).sum();
        let sum_out: usize = chunks.iter().map(|c| c.size_out as usize).sum();
        if sum_in != size_in as usize {
            return Err(FormatError::Invalid(format!(
                "块 size_in 总和 {sum_in} 与头 size_in={size_in} 不符"
            )));
        }
        if sum_out != size_out as usize || samples.len() * 2 != size_out as usize {
            return Err(FormatError::Invalid(format!(
                "块 size_out 总和 {sum_out}(解码 {} 字节)与头 size_out={size_out} 不符",
                samples.len() * 2
            )));
        }
        Ok(Aud {
            header: AudHeader {
                sample_rate,
                size_in,
                size_out,
                flags,
                compression,
            },
            chunks,
            samples,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aud_bytes(chunks: &[(&[u8], u16)], flags: u8, compression: u8) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&22050u16.to_le_bytes());
        let size_in: u32 = chunks.iter().map(|(p, _)| 8 + p.len() as u32).sum();
        let size_out: u32 = chunks.iter().map(|(_, o)| *o as u32).sum();
        v.extend_from_slice(&size_in.to_le_bytes());
        v.extend_from_slice(&size_out.to_le_bytes());
        v.push(flags);
        v.push(compression);
        for (payload, out) in chunks {
            v.extend_from_slice(&(payload.len() as u16).to_le_bytes());
            v.extend_from_slice(out);
            v.extend_from_slice(&CHUNK_ID.to_le_bytes());
            v.extend_from_slice(payload);
        }
        v
    }

    #[test]
    fn decodes_two_samples_from_one_byte() {
        // index=0(step=7),code 7(低半字节):delta=0+1+3+7=11 → sample 11;
        // index→8(step 16),code 0:delta=2 → sample 13
        let data = aud_bytes(&[(&[0x07], 4)], 2, COMPRESSION_IMA);
        let aud = Aud::parse(&data).unwrap();
        assert_eq!(aud.samples, [11, 13]);
        assert_eq!(aud.chunks.len(), 1);
        assert_eq!(aud.chunks[0].id, 0x0000_DEAF);
        assert_eq!(aud.header.sample_rate, 22050);
    }

    #[test]
    fn state_carries_across_chunks() {
        // 块 0 末状态 (index=7, sample=13);块 1 首字节 0x07:
        // step=14:delta=1+3+7+14=25 → 38;再 step=31:delta=3+7+15+31=56 → 94
        // 若按块复位状态,块 1 会从 (0,0) 重新解码,得不到 [38, 94]
        let data = aud_bytes(&[(&[0x07], 4), (&[0x07], 4)], 2, COMPRESSION_IMA);
        let aud = Aud::parse(&data).unwrap();
        assert_eq!(aud.samples, [11, 13, 38, 94]);
        assert_eq!(aud.chunks.len(), 2);
    }

    #[test]
    fn rejects_stereo() {
        let data = aud_bytes(&[(&[0x07], 4)], 3, COMPRESSION_IMA); // flags bit0=1
        assert!(matches!(
            Aud::parse(&data),
            Err(FormatError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_non_ima_compression() {
        // WS(=1)不在本里程碑
        let data = aud_bytes(&[(&[0x07], 4)], 0, 1);
        assert!(matches!(
            Aud::parse(&data),
            Err(FormatError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_flags_compression_mismatch() {
        // IMA 要求 bit1 置位(flags 2/3);flags=0 为畸形
        let data = aud_bytes(&[(&[0x07], 4)], 0, COMPRESSION_IMA);
        assert!(matches!(
            Aud::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_size_in_mismatch() {
        let mut data = aud_bytes(&[(&[0x07], 4)], 2, COMPRESSION_IMA);
        let bad = 10u32.to_le_bytes(); // 实际 8+1=9
        data[2..6].copy_from_slice(&bad);
        assert!(matches!(
            Aud::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_rate_out_of_range() {
        let mut data = aud_bytes(&[(&[0x07], 4)], 2, COMPRESSION_IMA);
        data[0..2].copy_from_slice(&1000u16.to_le_bytes());
        assert!(matches!(
            Aud::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn accepts_empty_audio() {
        let data = aud_bytes(&[], 2, COMPRESSION_IMA);
        let aud = Aud::parse(&data).unwrap();
        assert!(aud.chunks.is_empty());
        assert!(aud.samples.is_empty());
    }
}
```

- [ ] **Step 2: 运行单元测试通过**

Run: `cargo test -p ra2-core aud`
Expected: 8 passed

- [ ] **Step 3: 追加真实资产测试(`crates/core/tests/format_samples.rs` 末尾)**

```rust
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
    assert_eq!(
        &aud.samples[..8],
        &[11, 41, -22, 114, -179, 452, -543, 649]
    );
    // 跨块持续性裁决:样本 1024 起为块 1;若按块复位状态,这里不会是 [-217, -97, 12, 708]
    assert_eq!(&aud.samples[1024..1028], &[-217, -97, 12, 708]);
    assert_eq!(aud.samples.iter().copied().max(), Some(17293));
    assert_eq!(aud.samples.iter().copied().min(), Some(-11630));
}
```

- [ ] **Step 4: 运行真实资产测试**

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core aud -- --ignored`
Expected: 1 passed(真实 1;单元 8 已由前步覆盖,--ignored 不重复运行)

- [ ] **Step 5: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 合成 69 passed

```bash
git add crates/core/
git commit -m "feat(core): AUD IMA ADPCM 解码器(XCC 变体步长表)"
```

---

### Task 7: VXL 体素模型解析器

**Files:**
- Create: `crates/core/src/format/vxl.rs`
- Modify: `crates/core/src/format/mod.rs`(追加 `pub mod vxl;`)
- Modify: `crates/core/tests/format_samples.rs`(追加 2 个真实资产测试)

**Interfaces:**
- Consumes: `ByteReader`、`FormatError`
- Produces:
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct VxlVoxel { pub z: u8, pub color: u8, pub normal: u8 }`
  - `pub struct VxlColumn { pub voxels: Vec<VxlVoxel> }`(空列为空 Vec)
  - `pub struct VxlSection { pub name: String, pub dims: [u8; 3], pub normal_type: i8, pub scale: f32, pub transform: [[f32; 4]; 3], pub min: [f32; 3], max: [f32; 3], pub columns: Vec<VxlColumn> }`
  - `pub struct VxlFile { pub palette: [[u8; 3]; 256], pub sections: Vec<VxlSection> }`
  - `VxlFile::parse(data: &[u8]) -> Result<Self, FormatError>`
- 语义要点(研究文档 §1,勿改):802B 头(magic `"Voxel Animation\0"`、one==1、counts 相等、size 等式、unknown==0x1F10 全部校验)+ S×28B 节头(名字截首 NUL、section_i==i 校验;one/zero 宽松)+ 节体(span_start/end 表各 4*cx*cy 个 i32,体相对所有节共享原点)+ S×92B 节尾恰在 EOF;空列 = start/end 同时 -1(单独 -1 → Invalid);span 记录 `while z < cz { skip; count; count×{color,normal}; pad }`,pad 消耗不校验(XCC 同),skip 越层顶/段越窗/提前耗尽 → Invalid;f32 只做 LE 存取。

- [ ] **Step 1: 写 `vxl.rs`(测试 + 实现)**

`crates/core/src/format/mod.rs` 追加:

```rust
pub mod vxl;
```

`vxl.rs` 完整文件:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! VXL 体素模型(.vxl)。
//!
//! 布局(研究文档 2026-11-03-vxl-hva-layouts.md §1;XCC `vxl_file.{h,cpp}` +
//! 真实样本双向验证):802 字节文件头 + `S`×28 字节节头 + 连续节体 +
//! `S`×92 字节节尾(恰在文件尾结束)。
//! 节体 = 各节 span_start 表(4*cx*cy 个 i32)+ span_end 表 + span 数据,
//! 三个偏移均为**节体区**相对偏移(所有节共享同一原点)。
//! 列序 j = y*cx + x;空列 = start/end 同时为 -1;
//! 非空列数据:`while z < cz { skip:u8; count:u8; z+=skip; count×{color:u8,
//! normal:u8}; z+=count; pad:u8(消耗不校验) }`,体素段+跳过恰覆盖 cz 层。
//! 法线表由节尾 `normal_type` 选择(2 = TS 36 项,其余 = RA2 244 项;
//! 方向向量表属渲染层,本模块只透传索引)。

use super::bytes::ByteReader;
use super::FormatError;

const VXL_MAGIC: &[u8; 16] = b"Voxel Animation\0";

/// 单个体素:z 层号 + 头部调色板颜色索引 + 法线索引。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VxlVoxel {
    pub z: u8,
    pub color: u8,
    pub normal: u8,
}

/// 一列(j = y*cx + x)。空列为空 `Vec`。
pub struct VxlColumn {
    pub voxels: Vec<VxlVoxel>,
}

/// 一个节(section/limb)的解码结果。
pub struct VxlSection {
    pub name: String,
    pub dims: [u8; 3], // cx, cy, cz
    /// 法线表选择:2 = TS 36 项表,其余 = RA2 244 项表(渲染层裁决)。
    pub normal_type: i8,
    /// 体素棱长缩放(真库常为 1/12;XCC 自家写出的文件可为 0)。
    pub scale: f32,
    /// 静止姿态 3x4 变换(真库常非零;勿假设全零,也勿拒绝全零)。
    pub transform: [[f32; 4]; 3],
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub columns: Vec<VxlColumn>,
}

/// VXL 文件(文件头 magic/one/unknown 已校验,不存储)。
pub struct VxlFile {
    /// 256 项 RGB 调色板(XCC 渲染实际用外部调色板;此处按字节存档)。
    pub palette: [[u8; 3]; 256],
    pub sections: Vec<VxlSection>,
}

impl VxlFile {
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        let file_len = data.len();
        if file_len < 802 {
            return Err(FormatError::UnexpectedEof {
                needed: 802,
                remaining: file_len,
            });
        }
        if &data[0..16] != VXL_MAGIC {
            return Err(FormatError::Invalid("VXL 魔数不符".to_string()));
        }
        let mut r = ByteReader::new(&data[16..]);
        let one = r.read_u32_le()? as i32;
        if one != 1 {
            return Err(FormatError::Invalid(format!("VXL 头 one={one},应为 1")));
        }
        let c_headers = r.read_u32_le()?;
        let c_tailers = r.read_u32_le()?;
        if c_headers != c_tailers {
            return Err(FormatError::Invalid(format!(
                "节头数 {c_headers} 与节尾数 {c_tailers} 不等"
            )));
        }
        if c_headers == 0 {
            return Err(FormatError::Invalid("VXL 节数为 0".to_string()));
        }
        let s = c_headers as usize;
        let body_size = r.read_u32_le()? as usize;
        let unknown = r.read_u16_le()? as i16;
        if unknown != 0x1F10 {
            return Err(FormatError::Invalid(format!(
                "VXL 头 unknown=0x{unknown:04X},应为 0x1F10"
            )));
        }
        let mut palette = [[0u8; 3]; 256];
        for (i, slot) in palette.iter_mut().enumerate() {
            *slot = [data[34 + i * 3], data[35 + i * 3], data[36 + i * 3]];
        }
        // 三个区布局(精确等式,XCC vxl_file.cpp:19)
        let headers_start = 802usize;
        let body_start = headers_start + 28 * s;
        let tailer_start = file_len
            .checked_sub(92 * s)
            .ok_or_else(|| FormatError::Invalid("文件长不足以容纳节尾数组".to_string()))?;
        if body_start.checked_add(body_size) != Some(tailer_start) {
            return Err(FormatError::Invalid(format!(
                "尺寸等式不符: 802+28*{s}+{body_size} != 文件长-92*{s} ({tailer_start})"
            )));
        }
        // 节头(名字截首 NUL,尾部垃圾不校验;section_i 必须等于下标)
        let mut names = Vec::with_capacity(s);
        let mut r2 = ByteReader::new(&data[headers_start..body_start]);
        for i in 0..s {
            let raw = r2.read_bytes(16)?;
            let end = raw.iter().position(|&b| b == 0).unwrap_or(16);
            let name = String::from_utf8_lossy(&raw[..end]).into_owned();
            let section_i = r2.read_u32_le()? as i32;
            if section_i != i as i32 {
                return Err(FormatError::Invalid(format!(
                    "节 {i} 的 section_i={section_i}"
                )));
            }
            let _one = r2.read_u32_le()?; // 真库为 1;XCC 自身不校验,此处宽松
            let _zero = r2.read_u32_le()?; // 真库为 0;同上
            names.push(name);
        }
        // 节尾(92 字节/节,恰在文件尾结束;节 i 在 file_len-92*(S-i))
        struct TailerRaw {
            span_start_ofs: usize,
            span_end_ofs: usize,
            span_data_ofs: usize,
            scale: f32,
            transform: [[f32; 4]; 3],
            min: [f32; 3],
            max: [f32; 3],
            dims: [u8; 3],
            normal_type: i8,
        }
        let mut tailers = Vec::with_capacity(s);
        for i in 0..s {
            let base = tailer_start + 92 * i;
            let t = &data[base..base + 92];
            let i32_at = |off: usize| -> i32 {
                i32::from_le_bytes([t[off], t[off + 1], t[off + 2], t[off + 3]])
            };
            let f32_at = |off: usize| -> f32 {
                f32::from_le_bytes([t[off], t[off + 1], t[off + 2], t[off + 3]])
            };
            let (sso, seo, sdo) = (i32_at(0), i32_at(4), i32_at(8));
            if sso < 0 || seo < 0 || sdo < 0 {
                return Err(FormatError::Invalid(format!("节 {i} 的 span 偏移为负")));
            }
            let mut transform = [[0f32; 4]; 3];
            for (row, trow) in transform.iter_mut().enumerate() {
                for (col, cell) in trow.iter_mut().enumerate() {
                    *cell = f32_at(16 + 4 * (row * 4 + col));
                }
            }
            tailers.push(TailerRaw {
                span_start_ofs: sso as usize,
                span_end_ofs: seo as usize,
                span_data_ofs: sdo as usize,
                scale: f32_at(12),
                transform,
                min: [f32_at(64), f32_at(68), f32_at(72)],
                max: [f32_at(76), f32_at(80), f32_at(84)],
                dims: [t[88], t[89], t[90]],
                normal_type: t[91] as i8,
            });
        }
        // 逐节解码列
        let body = &data[body_start..tailer_start];
        let mut sections = Vec::with_capacity(s);
        for i in 0..s {
            let t = &tailers[i];
            let [cx, cy, cz] = t.dims;
            let columns_count = cx as usize * cy as usize;
            let (start_ofs, end_ofs, data_ofs) =
                (t.span_start_ofs, t.span_end_ofs, t.span_data_ofs);
            if start_ofs > body_size
                || end_ofs > body_size
                || data_ofs > body_size
                || end_ofs < start_ofs
                || end_ofs - start_ofs != 4 * columns_count
            {
                return Err(FormatError::Invalid(format!(
                    "节 {i} 的 span 表布局不符(需两表各 4*{columns_count} 字节,体区内)"
                )));
            }
            let ends_end = end_ofs + 4 * columns_count;
            if ends_end > body_size {
                return Err(FormatError::Invalid(format!(
                    "节 {i} 的 span_end 表越界"
                )));
            }
            let i32s = |slice: &[u8]| -> Vec<i32> {
                slice
                    .chunks_exact(4)
                    .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect()
            };
            let starts = i32s(&body[start_ofs..end_ofs]);
            let ends = i32s(&body[end_ofs..ends_end]);
            let mut columns = Vec::with_capacity(columns_count);
            for (j, (&sstart, &send)) in starts.iter().zip(ends.iter()).enumerate() {
                if sstart == -1 && send == -1 {
                    columns.push(VxlColumn {
                        voxels: Vec::new(),
                    });
                    continue;
                }
                if sstart < 0 || send < 0 {
                    return Err(FormatError::Invalid(format!(
                        "节 {i} 列 {j}:-1 必须成对出现"
                    )));
                }
                if send < sstart {
                    return Err(FormatError::Invalid(format!(
                        "节 {i} 列 {j}:span_end < span_start"
                    )));
                }
                let win_start = data_ofs + sstart as usize;
                let win_end = data_ofs + send as usize + 1;
                if win_end > body_size {
                    return Err(FormatError::Invalid(format!(
                        "节 {i} 列 {j}:span 窗口越界"
                    )));
                }
                let win = &body[win_start..win_end];
                let mut voxels = Vec::new();
                let mut z: usize = 0;
                let mut wp = 0usize;
                while z < cz as usize {
                    if win.len() - wp < 2 {
                        return Err(FormatError::Invalid(format!(
                            "节 {i} 列 {j}:span 提前耗尽"
                        )));
                    }
                    let skip = win[wp] as usize;
                    let count = win[wp + 1] as usize;
                    wp += 2;
                    z += skip;
                    if z > cz as usize {
                        return Err(FormatError::Invalid(format!(
                            "节 {i} 列 {j}:skip 越过层顶"
                        )));
                    }
                    if win.len() - wp < 2 * count + 1 {
                        return Err(FormatError::Invalid(format!(
                            "节 {i} 列 {j}:体素段越界"
                        )));
                    }
                    for k in 0..count {
                        voxels.push(VxlVoxel {
                            z: (z + k) as u8,
                            color: win[wp + 2 * k],
                            normal: win[wp + 2 * k + 1],
                        });
                    }
                    z += count;
                    wp += 2 * count + 1; // 末尾 pad 字节:消耗不校验(XCC 同)
                }
                columns.push(VxlColumn { voxels });
            }
            sections.push(VxlSection {
                name: names[i].clone(),
                dims: t.dims,
                normal_type: t.normal_type,
                scale: t.scale,
                transform: t.transform,
                min: t.min,
                max: t.max,
                columns,
            });
        }
        Ok(VxlFile { palette, sections })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造 1 节最小 VXL:cx=2, cy=1, cz=2;
    /// 列 0 = col0 字节串(体素段 + 终止空段),列 1 = 空(-1/-1)。
    fn sample_vxl_with(col0: &[u8]) -> Vec<u8> {
        let (cx, cy, cz) = (2u8, 1u8, 2u8);
        let lists_len = 4 * cx as usize * cy as usize; // 8
        let mut body = Vec::new();
        body.extend_from_slice(&0i32.to_le_bytes()); // span_start[0]
        body.extend_from_slice(&(-1i32).to_le_bytes()); // span_start[1] 空
        body.extend_from_slice(&((col0.len() - 1) as i32).to_le_bytes()); // span_end[0]
        body.extend_from_slice(&(-1i32).to_le_bytes()); // span_end[1] 空
        body.extend_from_slice(col0);
        let body_size = body.len();

        let mut v = Vec::new();
        v.extend_from_slice(VXL_MAGIC); // 0..16
        v.extend_from_slice(&1u32.to_le_bytes()); // 16..20 one
        v.extend_from_slice(&1u32.to_le_bytes()); // 20..24 c_section_headers
        v.extend_from_slice(&1u32.to_le_bytes()); // 24..28 c_section_tailers
        v.extend_from_slice(&(body_size as u32).to_le_bytes()); // 28..32 size
        v.extend_from_slice(&0x1F10u16.to_le_bytes()); // 32..34 unknown
        v.resize(802, 0); // 34..802 调色板全零
        let mut sec = [0u8; 28];
        sec[0..4].copy_from_slice(b"TEST");
        // section_i(+16)=0 保持;one(+20)=1
        sec[20..24].copy_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&sec); // 802..830
        v.extend_from_slice(&body); // 830..830+body_size
        let mut t = [0u8; 92];
        t[0..4].copy_from_slice(&0i32.to_le_bytes()); // span_start_ofs
        t[4..8].copy_from_slice(&(lists_len as i32).to_le_bytes()); // span_end_ofs
        t[8..12].copy_from_slice(&((2 * lists_len) as i32).to_le_bytes()); // span_data_ofs
        t[12..16].copy_from_slice(&(1.0f32 / 12.0).to_le_bytes()); // scale
        t[88] = cx;
        t[89] = cy;
        t[90] = cz;
        t[91] = 2; // normal_type
        v.extend_from_slice(&t);
        v
    }

    fn sample_vxl() -> Vec<u8> {
        // 列 0:skip 0, count 1, 体素 (0x39, 0x0F), pad;终止空段 skip 1, count 0, pad
        sample_vxl_with(&[0x00, 0x01, 0x39, 0x0F, 0x01, 0x01, 0x00, 0x00])
    }

    #[test]
    fn decodes_spans_and_empty_columns() {
        let vxl = VxlFile::parse(&sample_vxl()).unwrap();
        assert_eq!(vxl.sections.len(), 1);
        let s = &vxl.sections[0];
        assert_eq!(s.name, "TEST");
        assert_eq!(s.dims, [2, 1, 2]);
        assert_eq!(s.normal_type, 2);
        assert_eq!(s.columns.len(), 2);
        assert_eq!(
            s.columns[0].voxels,
            [VxlVoxel {
                z: 0,
                color: 0x39,
                normal: 0x0F
            }]
        );
        assert!(s.columns[1].voxels.is_empty());
        assert!((s.scale - 1.0 / 12.0).abs() < 1e-9);
        assert_eq!(vxl.palette[0], [0, 0, 0]);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut data = sample_vxl();
        data[0] ^= 0xFF;
        assert!(matches!(VxlFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_one_not_1() {
        let mut data = sample_vxl();
        data[16..20].copy_from_slice(&2u32.to_le_bytes());
        assert!(matches!(VxlFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_unknown_not_1f10() {
        let mut data = sample_vxl();
        data[32..34].copy_from_slice(&0u16.to_le_bytes());
        assert!(matches!(VxlFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_counts_mismatch() {
        let mut data = sample_vxl();
        data[24..28].copy_from_slice(&2u32.to_le_bytes()); // c_section_tailers=2
        assert!(matches!(VxlFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_size_equation_mismatch() {
        let mut data = sample_vxl();
        data[28..32].copy_from_slice(&25u32.to_le_bytes()); // 实际 body_size=24
        assert!(matches!(VxlFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_skip_past_top_layer() {
        // skip=3 直接越过 cz=2
        let data = sample_vxl_with(&[0x03, 0x01, 0x39, 0x0F, 0x01, 0x01, 0x00, 0x00]);
        assert!(matches!(VxlFile::parse(&data), Err(FormatError::Invalid(_))));
    }
}
```

- [ ] **Step 2: 运行单元测试通过**

Run: `cargo test -p ra2-core vxl`
Expected: 7 passed

- [ ] **Step 3: 追加真实资产测试(`crates/core/tests/format_samples.rs` 末尾)**

```rust
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
```

- [ ] **Step 4: 运行真实资产测试**

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core vxl -- --ignored`
Expected: 2 passed(真实 2;单元 7 已由前步覆盖,--ignored 不重复运行)

- [ ] **Step 5: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 合成 76 passed

```bash
git add crates/core/
git commit -m "feat(core): VXL 体素模型解析器"
```

---

### Task 8: HVA 逐帧节变换解析器

**Files:**
- Create: `crates/core/src/format/hva.rs`
- Modify: `crates/core/src/format/mod.rs`(追加 `pub mod hva;`)
- Modify: `crates/core/tests/format_samples.rs`(追加 2 个真实资产测试)

**Interfaces:**
- Consumes: `ByteReader`、`FormatError`
- Produces:
  - `pub struct HvaFile { pub frames: u32, pub section_names: Vec<String>, pub matrices: Vec<Vec<[[f32; 4]; 3]>> }`(`matrices[frame][section]`,帧主序;每项 3×4 行主序,前 3 列旋转、第 4 列平移)
  - `HvaFile::parse(data: &[u8]) -> Result<Self, FormatError>`
- 语义要点(研究文档 §2,勿改):精确尺寸等式 `file_size == 24 + (48*F + 16)*S`(用 u64 比较,防 u32 计数溢出);F>0、S>0;头 16 字节 id 是 3ds max 导出路径残片,不解析;节名 16 字节槽截首 NUL、尾部垃圾不校验;**帧主序**存储(XCC 写入器的节主序是已知 bug,勿复刻)。

- [ ] **Step 1: 写 `hva.rs`(测试 + 实现)**

`crates/core/src/format/mod.rs` 追加:

```rust
pub mod hva;
```

`hva.rs` 完整文件:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! HVA 逐帧节变换(.hva)。
//!
//! 布局(研究文档 2026-11-03-vxl-hva-layouts.md §2):24 字节头
//! `{char id[16](3ds max 导出路径残片,忽略); i32 c_frames; i32 c_sections}`
//! + `S`×16 字节节名槽(首 NUL 截断,尾部垃圾不校验)+ `F*S`×48 字节矩阵。
//! 矩阵按 **帧主序** `[frame][section]` 排布(XCC 读取器语义;其写入器按节
//! 主序写是已知 bug hva_file.cpp:87,勿复刻)。每个矩阵 12×f32 = 3 行 × 4 列,
//! 行主序:前 3 列为旋转,第 4 列为平移。
//! 精确尺寸等式:file_size == 24 + (48*F + 16) * S,F>0,S>0。

use super::bytes::ByteReader;
use super::FormatError;

/// HVA 文件。`matrices[frame][section]`,每项 3×4 行主序。
pub struct HvaFile {
    pub frames: u32,
    pub section_names: Vec<String>,
    pub matrices: Vec<Vec<[[f32; 4]; 3]>>,
}

impl HvaFile {
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        if data.len() < 24 {
            return Err(FormatError::UnexpectedEof {
                needed: 24,
                remaining: data.len(),
            });
        }
        let mut r = ByteReader::new(data);
        let _id = r.read_bytes(16)?; // 导出管线残片(如 "D:\RA2X\MODELS\"),不解析
        let frames = r.read_u32_le()?;
        let sections = r.read_u32_le()?;
        if frames == 0 || sections == 0 {
            return Err(FormatError::Invalid(format!(
                "HVA frames={frames} sections={sections},都必须 > 0"
            )));
        }
        // 尺寸等式用 u64,防 u32 计数在 usize 上溢出
        let expected = 24u64 + 16 * sections as u64 + 48 * frames as u64 * sections as u64;
        if data.len() as u64 != expected {
            return Err(FormatError::Invalid(format!(
                "HVA 尺寸等式不符: 文件 {} 字节,应为 {expected}(F={frames}, S={sections})"
            )));
        }
        let s = sections as usize;
        let f = frames as usize;
        let names_region = 24 + 16 * s;
        let mut r2 = ByteReader::new(&data[24..names_region]);
        let mut section_names = Vec::with_capacity(s);
        for _ in 0..s {
            let raw = r2.read_bytes(16)?;
            let end = raw.iter().position(|&b| b == 0).unwrap_or(16);
            section_names.push(String::from_utf8_lossy(&raw[..end]).into_owned());
        }
        let mut matrices = Vec::with_capacity(f);
        for _ in 0..f {
            let mut frame = Vec::with_capacity(s);
            for _ in 0..s {
                let mut m = [[0f32; 4]; 3];
                for row in 0..3 {
                    for col in 0..4 {
                        let bytes = r.read_bytes(4)?;
                        m[row][col] =
                            f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    }
                }
                frame.push(m);
            }
            matrices.push(frame);
        }
        Ok(HvaFile {
            frames,
            section_names,
            matrices,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hva_bytes(f: u32, s: u32, names: &[&str], values: &[[f32; 12]]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"C:\\3DSMAX\\MESHE\0"); // 16 字节 id 残片
        v.extend_from_slice(&f.to_le_bytes());
        v.extend_from_slice(&s.to_le_bytes());
        for name in names {
            let mut slot = [0u8; 16];
            let bytes = name.as_bytes();
            slot[..bytes.len()].copy_from_slice(bytes);
            v.extend_from_slice(&slot);
        }
        for m in values {
            for x in m {
                v.extend_from_slice(&x.to_le_bytes());
            }
        }
        v
    }

    #[test]
    fn frame_major_ordering() {
        // XCC 写入器按节主序写是其已知 bug;真库是帧主序——本测试锁定:
        // 第 2 个写入值属于 (帧0,节1),第 3 个属于 (帧1,节0)
        let m00 = [1.0f32; 12];
        let mut m01 = [0.0f32; 12];
        m01[3] = 7.5; // 平移列,便于辨认
        let m10 = [2.0f32; 12];
        let m11 = [3.0f32; 12];
        let data = hva_bytes(2, 2, &["A", "B"], &[m00, m01, m10, m11]);
        let hva = HvaFile::parse(&data).unwrap();
        assert_eq!(hva.frames, 2);
        assert_eq!(hva.section_names, ["A", "B"]);
        assert_eq!(hva.matrices[0][0][0][3], 1.0); // m00
        assert_eq!(hva.matrices[0][1][0][3], 7.5); // m01
        assert_eq!(hva.matrices[1][0][0][0], 2.0); // m10(节主序会误读成 m01)
        assert_eq!(hva.matrices[1][1][0][0], 3.0); // m11
    }

    #[test]
    fn names_trim_at_first_nul() {
        // 真库名槽尾部有垃圾字节(1tnk.hva: "B-BODY\0" 后跟 17 00 ...),截断不校验
        let mut v = Vec::new();
        v.extend_from_slice(&[0u8; 16]);
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes());
        let mut slot = [0u8; 16];
        slot[..6].copy_from_slice(b"B-BODY");
        slot[6] = 0; // 终止 NUL
        slot[7] = 0x17; // 垃圾
        slot[12] = 0xEE; // 垃圾
        v.extend_from_slice(&slot);
        for _ in 0..12 {
            v.extend_from_slice(&1.0f32.to_le_bytes());
        }
        let hva = HvaFile::parse(&v).unwrap();
        assert_eq!(hva.section_names, ["B-BODY"]);
    }

    #[test]
    fn rejects_zero_frames() {
        let data = hva_bytes(0, 2, &["A", "B"], &[]);
        assert!(matches!(
            HvaFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_size_mismatch() {
        let mut data = hva_bytes(2, 2, &["A", "B"], &[[1.0f32; 12]; 4]);
        data.pop();
        assert!(matches!(
            HvaFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }
}
```

- [ ] **Step 2: 运行单元测试通过**

Run: `cargo test -p ra2-core hva`
Expected: 4 passed

- [ ] **Step 3: 追加真实资产测试(`crates/core/tests/format_samples.rs` 末尾)**

```rust
/// schp.hva:F=2,S=3(CYLINDER19/CYLINDER57/DUMMY01);
/// 帧 1 节 0 = 绕 Z 轴 30° 旋转,平移跨帧恒定(帧主序的独立证据,2026-11-03 精确取证)。
#[test]
#[ignore]
fn hva_schp_frame_major_two_frames() {
    use ra2_core::format::hva::HvaFile;
    let data = ensure_sample(&["ra2md.mix", "localmd.mix"], "schp.hva", "68f97095812ab96a");
    assert_eq!(data.len(), 360);
    let hva = HvaFile::parse(&data).unwrap();
    assert_eq!(hva.frames, 2);
    assert_eq!(hva.section_names, ["CYLINDER19", "CYLINDER57", "DUMMY01"]);
    // matrices[1][0]:30° 绕 Z 旋转(容差按 f32 存储精度取)
    let m = &hva.matrices[1][0];
    assert!((m[0][0] - 0.866_025_448).abs() < 1e-6);
    assert!((m[0][1] - 0.5).abs() < 1e-6);
    assert!((m[1][0] - (-0.500_000_06)).abs() < 1e-6);
    assert!((m[2][2] - 1.0).abs() < 1e-5);
    assert!((m[0][3] - 55.021_419_5).abs() < 1e-3);
    assert!((m[1][3] - 1.923_307_9).abs() < 1e-4);
    assert!((m[2][3] - 233.498_367).abs() < 1e-3);
    // 平移跨帧恒定、旋转变化 → 证实帧主序(若节主序,平移会跳变)
    assert!((hva.matrices[0][0][0][3] - hva.matrices[1][0][0][3]).abs() < 1e-3);
    assert!((hva.matrices[0][0][0][0] - hva.matrices[1][0][0][0]).abs() > 0.1);
    // matrices[0][1](CYLINDER57)平移
    let m01 = &hva.matrices[0][1];
    assert!((m01[0][3] - (-523.366_394)).abs() < 1e-3);
    assert!((m01[1][3] - (-8.210_362_43)).abs() < 1e-5);
    assert!((m01[2][3] - 161.296_814).abs() < 1e-3);
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
    assert!((m[0][0] - 0.999_999_881).abs() < 1e-6);
    assert!((m[1][1] - 0.999_999_881).abs() < 1e-6);
    assert!((m[2][2] - 0.999_999_881).abs() < 1e-6);
    assert!((m[0][3] - (-191.924_225)).abs() < 1e-2);
    assert!((m[1][3] - (-0.065_056_756_1)).abs() < 1e-6);
    assert!((m[2][3] - 81.487_075_8).abs() < 1e-4);
}
```

- [ ] **Step 4: 运行真实资产测试**

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core hva -- --ignored`
Expected: 2 passed(真实 2;单元 4 已由前步覆盖,--ignored 不重复运行)

- [ ] **Step 5: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 合成 80 passed

```bash
git add crates/core/
git commit -m "feat(core): HVA 逐帧节变换解析器"
```

---

### Task 9: ra2-mix CLI 打磨批(退出码契约)

**Files:**
- Modify: `tools/ra2-mix/src/main.rs`(整体替换)
- Modify: `tools/ra2-mix/tests/cli.rs`(整体替换)

**Interfaces:**
- Consumes: `MixArchive`(含 Task 1 摘要位)
- Produces: 退出码契约收紧后的 `ra2-mix`:
  - 0 成功;1 找不到文件/解析失败/写失败;2 用法错误(含非 UTF-8 参数)
  - usage 裁决先于任何文件 IO(未知子命令即使 MIX 路径不可读也退出 2)
  - stdout 写失败且 `ErrorKind::BrokenPipe`(如 `| head`)→ 视为成功退出 0;其他写失败退出 1
  - extract 落盘失败 → eprintln + 退出 1,不 panic
  - 非 UTF-8 参数 → 退出 2,不 panic(`args_os` 替代会 panic 的 `args`)

- [ ] **Step 1: 写失败测试(`tools/ra2-mix/tests/cli.rs` 整体替换;每测试独立临时目录,修掉计划一遗留的共享路径竞争)**

```rust
// SPDX-License-Identifier: GPL-3.0-only

use std::process::Command;

fn bin() -> std::path::PathBuf {
    env!("CARGO_BIN_EXE_ra2-mix").into()
}

/// 每个测试用独立临时目录(并行测试互不踩踏)。
fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("ra2-mix-cli-{tag}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn make_mix(path: &std::path::Path) {
    use ra2_core::format::crc::mix_name_id;
    let body: Vec<u8> = b"HELLO".to_vec();
    let mut v = Vec::new();
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&(body.len() as u32).to_le_bytes());
    v.extend_from_slice(&mix_name_id("HELLO").to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&(body.len() as u32).to_le_bytes());
    v.extend_from_slice(&body);
    std::fs::write(path, v).unwrap();
}

#[test]
fn list_and_cat_work() {
    let dir = temp_dir("list-cat");
    let mix_path = dir.join("t.mix");
    make_mix(&mix_path);

    let out = Command::new(bin())
        .args(["list", mix_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let expect_id = format!("{:08X}", ra2_core::format::crc::mix_name_id("HELLO"));
    assert!(stdout.contains(&expect_id));

    let out = Command::new(bin())
        .args(["cat", mix_path.to_str().unwrap(), "hello"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"HELLO");
}

#[test]
fn missing_entry_exits_1() {
    let dir = temp_dir("missing");
    let mix_path = dir.join("t.mix");
    make_mix(&mix_path);
    let out = Command::new(bin())
        .args(["cat", mix_path.to_str().unwrap(), "nope.ini"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn unknown_subcommand_exits_2_before_io() {
    // MIX 路径故意不可读:退出码 2 证明 usage 裁决先于文件 IO
    let out = Command::new(bin())
        .args(["frobnicate", "Z:/肯定不存在/nope.mix"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn extract_to_dir_writes_file() {
    let dir = temp_dir("extract");
    let mix_path = dir.join("t.mix");
    make_mix(&mix_path);
    let out_dir = dir.join("out");
    let out = Command::new(bin())
        .args([
            "extract",
            mix_path.to_str().unwrap(),
            "HELLO",
            "-o",
            out_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let written = std::fs::read(out_dir.join("HELLO")).expect("extract 应写文件");
    assert_eq!(written, b"HELLO");
}

#[test]
fn extract_write_failure_exits_1() {
    // -o 指向一个已存在的普通文件:create_dir_all 失败 → 退出 1,不 panic
    let dir = temp_dir("writefail");
    let mix_path = dir.join("t.mix");
    make_mix(&mix_path);
    let blocker = dir.join("blocker");
    std::fs::write(&blocker, b"x").unwrap();
    let out = Command::new(bin())
        .args([
            "extract",
            mix_path.to_str().unwrap(),
            "HELLO",
            "-o",
            blocker.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn non_utf8_arg_exits_2_not_panic() {
    // 旧实现 std::env::args() 遇非法 UTF-8 会 panic(退出 101)
    #[cfg(windows)]
    let bad = {
        use std::os::windows::ffi::OsStringExt;
        std::ffi::OsString::from_wide(&[0xD800, 0x006D, 0x0069, 0x0078]) // 未配对代理 + "mix"
    };
    #[cfg(unix)]
    let bad = {
        use std::os::unix::ffi::OsStringExt;
        std::ffi::OsString::from_vec(vec![0xFF, b'm', b'i', b'x'])
    };
    let dir = temp_dir("utf8");
    let mix_path = dir.join("t.mix");
    make_mix(&mix_path);
    let out = Command::new(bin())
        .args([bad, mix_path.into_os_string()])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p ra2-mix`
Expected: `unknown_subcommand_exits_2_before_io`、`extract_write_failure_exits_1`、`non_utf8_arg_exits_2_not_panic` 失败(现实现先 IO 后分发、写失败 panic、args() panic)

- [ ] **Step 3: 实现(`tools/ra2-mix/src/main.rs` 整体替换)**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! ra2-mix:MIX 档案命令行工具(list / extract / cat)。
//!
//! 退出码契约:0 成功;1 找不到文件/解析失败/写失败;2 用法错误(含非 UTF-8 参数)。

use ra2_core::format::mix::MixArchive;
use std::io::Write;
use std::process::ExitCode;

fn usage() -> ! {
    eprintln!("用法: ra2-mix list <file.mix>");
    eprintln!("      ra2-mix extract <file.mix> <文件名> -o <目录|->");
    eprintln!("      ra2-mix cat <file.mix> <文件名>");
    std::process::exit(2);
}

/// 把字节写 stdout;下游提前关闭(BrokenPipe,如 `| head`)视为成功。
fn write_stdout(data: &[u8]) -> ExitCode {
    let mut out = std::io::stdout();
    match out.write_all(data).and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("写 stdout 失败: {e}");
            ExitCode::from(1)
        }
    }
}

enum Action<'a> {
    List,
    Cat(&'a str),
    Extract(&'a str, &'a str),
}

fn main() -> ExitCode {
    // std::env::args() 遇非 UTF-8 参数会 panic;改用 args_os,非 UTF-8 按用法错误处理
    let mut args = Vec::new();
    for a in std::env::args_os().skip(1) {
        match a.into_string() {
            Ok(s) => args.push(s),
            Err(_) => usage(),
        }
    }
    // 先裁决用法,再触碰文件 IO(未知子命令即使 MIX 不可读也必须退出 2)
    let (mix_path, action) = match args.as_slice() {
        [cmd, mix_path, rest @ ..] => match (cmd.as_str(), rest) {
            ("list", []) => (mix_path.as_str(), Action::List),
            ("cat", [name]) => (mix_path.as_str(), Action::Cat(name.as_str())),
            ("extract", [name, flag, dir]) if flag == "-o" => {
                if dir == "-" {
                    (mix_path.as_str(), Action::Cat(name.as_str()))
                } else {
                    (mix_path.as_str(), Action::Extract(name.as_str(), dir.as_str()))
                }
            }
            _ => usage(),
        },
        _ => usage(),
    };
    run(mix_path, action)
}

fn run(mix_path: &str, action: Action<'_>) -> ExitCode {
    let data = match std::fs::read(mix_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("读取 {mix_path} 失败: {e}");
            return ExitCode::from(1);
        }
    };
    let mix = match MixArchive::parse(data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("解析失败: {e}");
            return ExitCode::from(1);
        }
    };
    match action {
        Action::List => {
            // 先攒齐再一次性写出:list 中途断管不该留下半截 panic
            let mut listing = String::new();
            for e in mix.entries() {
                listing.push_str(&format!("{:08X} {:>10} {:>10}\n", e.id, e.offset, e.size));
            }
            write_stdout(listing.as_bytes())
        }
        Action::Cat(name) => match mix.read(name) {
            Ok(d) => write_stdout(d),
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        },
        Action::Extract(name, dir) => match mix.read(name) {
            Ok(d) => {
                let path = std::path::Path::new(dir).join(name);
                if let Some(parent) = path.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        eprintln!("创建目录 {} 失败: {e}", parent.display());
                        return ExitCode::from(1);
                    }
                }
                match std::fs::write(&path, d) {
                    Ok(()) => {
                        // 用 eprintln:println! 在管道关闭时会 panic,破坏退出码契约
                        eprintln!("已写入 {}", path.display());
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("写 {} 失败: {e}", path.display());
                        ExitCode::from(1)
                    }
                }
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        },
    }
}
```

- [ ] **Step 4: 运行全部测试通过**

Run: `cargo test -p ra2-mix && cargo test --workspace`
Expected: ra2-mix 6 passed;workspace 合成 84 passed

- [ ] **Step 5: BrokenPipe 手工验收(无法稳定自动化;Git Bash)**

```bash
cargo run -q -p ra2-mix -- list "E:\hongjing\Reunion 2023\ra2.mix" | head -3
echo "exit=${PIPESTATUS[0]}"
```
Expected: 打印 3 行索引后,`exit=0`(旧实现为 101 panic)

- [ ] **Step 6: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 全绿

```bash
git add tools/
git commit -m "fix(tools): ra2-mix CLI 打磨(退出码契约/BrokenPipe/非 UTF-8 参数)"
```

---

### Task 10: M1b 里程碑验收(全链路真实资产)

**Files:**
- Create: `crates/core/tests/m1b_acceptance.rs`
- Modify: `README.md`(追加"格式层"小节;仓库现无里程碑表,不新建)

**Interfaces:**
- Consumes: 全部六个解析器(`ShpFile`/`TmpRa2File`/`CsfDocument`/`Aud`/`VxlFile`/`HvaFile`)+ `MixArchive`(摘要位)+ `Palette` + `FormatError`
- Produces: `#[ignore]` 全链路验收测试 `m1b_full_pipeline_over_real_assets`——摘要位 MIX 链 + 六类解析器全部吃真字节;README 记录格式层能力

- [ ] **Step 1: 写 `crates/core/tests/m1b_acceptance.rs`(完整文件)**

```rust
// SPDX-License-Identifier: GPL-3.0-only
//! M1b 全链路验收(真实资产):摘要位 MIX 链 + 六类解析器全部吃真字节。
//! 运行:RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test m1b_acceptance -- --ignored

use ra2_core::format::aud::Aud;
use ra2_core::format::csf::CsfDocument;
use ra2_core::format::hva::HvaFile;
use ra2_core::format::mix::MixArchive;
use ra2_core::format::pal::Palette;
use ra2_core::format::shp::ShpFile;
use ra2_core::format::tmp::TmpRa2File;
use ra2_core::format::vxl::VxlFile;
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

/// 逐级按名下降 MIX 链读取条目(与 format_samples.rs 同款助手;测试 crate 各自独立)。
fn read_from_chain(chain: &[&str], name: &str) -> Vec<u8> {
    let mut bytes =
        std::fs::read(assets_dir().join(chain[0])).unwrap_or_else(|e| panic!("读顶层: {e}"));
    for level in &chain[1..] {
        let mix = MixArchive::parse(bytes).expect("解析嵌套 MIX");
        bytes = mix.read(level).unwrap_or_else(|e| panic!("读 {level}: {e}")).to_vec();
    }
    let mix = MixArchive::parse(bytes).expect("解析目标 MIX");
    mix.read(name).unwrap_or_else(|e| panic!("读 {name}: {e}")).to_vec()
}

#[test]
#[ignore]
fn m1b_full_pipeline_over_real_assets() {
    // 1) 摘要位链:ra2md.mix → isourbmd.mix(0x00010000)→ 按 ID 读取首条目
    let ra2md = load("ra2md.mix");
    let iso = MixArchive::parse(ra2md.read("isourbmd.mix").unwrap().to_vec())
        .expect("摘要位应被接受");
    assert_eq!(iso.entries().len(), 57);
    assert!(!iso.read_by_id(iso.entries()[0].id).unwrap().is_empty());
    // 加密位真库仍拒
    assert!(matches!(
        MixArchive::parse(ra2md.read("sidec02md.mix").unwrap().to_vec()),
        Err(FormatError::Unsupported(_))
    ));

    // 2) SHP 全链:ra2md.mix → conqmd.mix → gunfirey.shp(按名,真库直读)
    let conq = MixArchive::parse(ra2md.read("conqmd.mix").unwrap().to_vec()).unwrap();
    let shp = ShpFile::parse(conq.read("gunfirey.shp").unwrap()).unwrap();
    assert_eq!(shp.frames.len(), 8);
    assert!(shp.frames[0].pixels.iter().all(|&p| (p as usize) < 256)); // 调色板索引域

    // 3) TMP:ra2.mix → snow.mix → burnt01.sno(按名;该库 LMD 名单裁决)
    let ra2 = load("ra2.mix");
    let snow = MixArchive::parse(ra2.read("snow.mix").unwrap().to_vec()).unwrap();
    let tmp = TmpRa2File::parse(snow.read("burnt01.sno").unwrap()).unwrap();
    assert_eq!(tmp.tiles.len(), 2);
    assert_eq!(tmp.tiles[0].pixels.len(), 29 * 17);

    // 4) CSF:langmd.mix → ra2md.csf
    let langmd = load("langmd.mix");
    let csf = CsfDocument::parse(langmd.read("ra2md.csf").unwrap()).unwrap();
    assert_eq!(csf.labels.len(), 5280);
    assert_eq!(csf.labels[0].name, "THEME:Intro");

    // 5) VXL + HVA + AUD:ra2.mix → local.mix
    let local = MixArchive::parse(ra2.read("local.mix").unwrap().to_vec()).unwrap();
    let vxl = VxlFile::parse(local.read("1tnk.vxl").unwrap()).unwrap();
    assert_eq!(vxl.sections[0].columns.len(), 465);
    let hva = HvaFile::parse(local.read("1tnk.hva").unwrap()).unwrap();
    assert_eq!(hva.section_names, ["B-BODY"]);
    let aud = Aud::parse(local.read("text1.aud").unwrap()).unwrap();
    assert_eq!(aud.samples.len(), 1586);

    // 6) 调色板:ra2md.mix → localmd.mix → mpyscrn.pal(768 字节,VXL/SHP 的邻近断言:
    //    SHP/TMP/VXL 产出的都是 0..=255 调色板索引,渲染上色属 view 层)
    let localmd = MixArchive::parse(ra2md.read("localmd.mix").unwrap().to_vec()).unwrap();
    let pal = Palette::parse(localmd.read("mpyscrn.pal").unwrap()).unwrap();
    assert_eq!(pal.rgba(0).len(), 4);
}
```

- [ ] **Step 2: 运行验收测试**

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test m1b_acceptance -- --ignored`
Expected: 1 passed

- [ ] **Step 3: README 追加格式层小节(插在 `## ra2-mix` 之前)**

```markdown
## 格式层(M1b)

`ra2-core::format` 现可解析:MIX(明文/SHA-1 摘要位索引;加密位明确拒绝)、
INI、PAL、SHP(TS/RA2,RLEZeroTS)、TMP(RA2 剧场布局)、CSF、
AUD(IMA ADPCM,XCC 变体步长表)、VXL、HVA。
规格依据见 `docs/superpowers/research/`,实施计划见 `docs/superpowers/plans/`。
```

- [ ] **Step 4: 全套终验**

Run: `cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 全绿;合成 84 passed

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored`
Expected: 21 passed(计划一 5 + 本计划 16)

- [ ] **Step 5: Commit**

```bash
git add crates/core/tests/m1b_acceptance.rs README.md
git commit -m "test(core): M1b 全链路真实资产验收 + README 格式层说明"
```

---

## 完成定义(计划级验收)

1. `cargo test --workspace` 全绿(84 个合成测试,含 6 个 CLI 测试);`RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored` 全绿(21 个真实资产测试,含 17/17 哈希闭合的样本提取与全链路验收)。
2. `cargo fmt --all --check` 与 `cargo clippy --all-targets -- -D warnings` 零告警。
3. 六类解析器对真实字节零 panic(单元 + 真实双覆盖;所有切片/索引显式边界检查)。
4. 仓库不含任何 EA 资产(`fixtures/` 已忽略;`git ls-files` 审计通过)。
5. 骨架里程碑验收(计划 Task 10)通过:摘要位 MIX 链可用、六个解析器全部在真实资产上出数、调色板邻近断言成立。

## 附录:Self-Review(计划自查,2026-11-03)

**spec 覆盖检查**(对照设计文档 §2/§3/§6 与里程碑表 M1):

- §2 法律红线:全部规格来自 GPL 的 XCC 源码语义 + 用户自有样本字节逆向,代码只做语义重写,模块头注明出处——Task 3-8 各文件头注释 ✓
- §3 架构:新模块全部落在 `crates/core/src/format/`,零 IO、零第三方依赖,CLI 仍在 `tools/ra2-mix` ✓
- §6 测试策略:合成样本单测(CI 可跑)+ 真实资产 `#[ignore]` 测试(`RA2_ASSETS_DIR`)+ 哈希基线(不入库资产)✓;oracle 对拍(格式层)由 17/17 链路哈希闭合与逐字节断言承担 ✓
- M1 验收项"对用户原版 MIX 全量解析零 panic":Task 1 负样本(加密/未知位)+ 六解析器边界错误路径覆盖;完整"全量遍历所有 MIX"留待 M2 地图里程碑(本计划已把摘要位解锁作为前置)——记录为有意推迟,非遗漏 ✓
- 骨架 10 个任务全部落地:1 摘要位 / 2 fixtures / 3 SHP / 4 TMP / 5 CSF / 6 AUD / 7 VXL / 8 HVA / 9 CLI / 10 验收 ✓

**placeholder 扫描**:全文代码块无 `todo!`/`unimplemented!`/`...` 省略/"同 Task N"引用;每个测试与实现均为完整可编译文件或完整替换函数。测试常量全部来自研究文档或本计划规格依据 §1-§8 的取证记录(2026-11-03 对真机资产与现役 fixtures 复验,17/17 哈希闭合)✓

**跨任务类型一致性检查**:

- `rle_zero_ts_decode(src, cx, cy, out) -> Result<usize, FormatError>`:Task 3 定义,Task 4(SHP/TMP 双消费)签名一致 ✓
- `ensure_sample/read_from_chain/sha16`:Task 2 定义,Task 3-8 调用签名一致 ✓
- `FormatError` 三变体用途全计划统一(UnexpectedEof=截断,Unsupported=加密/立体声/WS/未知位/未知压缩,Invalid=结构畸形)✓
- 计数核对:合成 84(29 core 既有 + 3 mix + 14 rle/shp + 6 tmp + 7 csf + 8 aud + 7 vxl + 4 hva + 6 cli)、ignored 21(5 既有 + 2 + 1 + 3 + 3 + 1 + 1 + 2 + 2 + 1)——与各任务 Run 步骤的期望值逐一相符 ✓
- 摘要位语义翻转一处既有断言(计划一 `rejects_encrypted_index` 用 0x00010000)已在 Task 1 显式重写 ✓

**范围守卫复核**:无 Blowfish、无 TS-TMP 变体、无渲染/上色、无 get_float、无 SHA-1 实现、无阴影帧配对——全部按 Global Constraints 排除并注明归属里程碑 ✓

**已知风险与执行提示**:

- 测试常量若在执行期失配(如某样本被游戏目录更新),按 Task 2 的自愈设计:先复现失败,再用只读脚本对真库重哈希,更新规格依据 §7 的常量并在 commit message 注明——不得静默放宽断言。
- clippy 若对个别手写循环报 `needless_range_loop`,优先改写为 `iter().zip()/enumerate()`(计划代码已按此风格预写);不得用 `#[allow]` 压告警。







