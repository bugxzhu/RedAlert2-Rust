# M2 地图/地形解析层(四包段解码 + 对象段 + MapFile + ra2-map CLI)实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 M1b 格式层之上组装地图解析层:任意用户自有 `.map` → 可查询数据模型 `MapFile`(IsoMapPack5 瓦片网格、双 Overlay 网格、Preview 原样 RGB、六类对象段、logic 段 tokenize、元数据/摘要),新增从规范语义重写的 LZO1X 安全解压器,落地 IniDocument 七项普查适配与内存预检加固,交付 `tools/ra2-map` CLI(info / extract-tile),807 张真实地图全量解析零 panic。

**Architecture:** 延续 cargo workspace:`ra2-core` 新增 `format/lzo1x.rs`(与 lcw.rs 平级)与 `format/map/` 模块族(`mod/mapdoc/gbk/pack/iso/overlay/preview/objects/meta`),复用 IniDocument / lcw_decode / Palette / crc32;`tools/ra2-map` 新 CLI crate(PNG 编码器纯 std 写在工具侧,CRC32 复用 core)。`lcw.rs` 增加带消耗量返回的包装函数供包段循环核对。工作分支 `feat/m2-map-terrain`(基于 main,执行时按 superpowers:using-git-worktrees 建立隔离工作区)。

**Tech Stack:** Rust stable(edition 2021,本机 rustc 1.99.1 已验证),零第三方依赖,GitHub Actions CI(沿用既有 ci.yml)。

**规格依据(计划中所有字节级常量的出处;实现时不需要再考证):**

两份研究文档(权威,均经真实样本逐字节验证):

- `docs/superpowers/research/2026-11-04-map-pack-and-binary-sections.md`(pack 容器 §1-2、LZO1X §2.2、IsoMapPack5 §3、Overlay §4、Preview §5、剧场链 §7、预检钩子 §8、语料验证 §12、指纹 §13)
- `docs/superpowers/research/2026-11-04-map-ini-survey.md`(ini.rs 兼容裁决 §3/§7、七项适配清单 §8、字段普查 §4)

本计划编写时(2026-11-04)又对全部测试常量做了独立字节取证:计划作者实现了 Rust 版 LZO1X 移植并跑通 18 个手造操作码向量,再用它对 §13 五张指纹图(c1a01md/c1a03md/c2s02md/c1a02md/moonpatr)做四包段全链解码,chunk 数与解压字节数**全部逐项复现**(如 c1a01md iso 17 块/135117 B、preview 4 块/28674 B = 3·162·59,overlay 32 块/262144 B);Vector A 另与研究会话留下的 Python 实现(`pack_probe.lzo1x_decompress`,540 图 34.4M 记录语料扫过)双实现对拍一致。新证据与两处研究文档勘误如下,**冲突一律按下述结论执行**:

1. **指纹图名勘误**:研究文档 §13 的"城墙之战.map"实为 `Maps\DDLY\城郊之战.map`(sha16 `bfa85ec60aae9841`,URBAN 90×90,§13 整行指纹逐项复现)。DDLY 下另有一张尺寸完全相同的同名近义图,勿混用;本计划一律用 `城郊之战.map`。
2. **语料尺寸极值勘误**:研究文档 §8-4/§12 的"W+H = 898(798×100)"与"最大瓦片流 1,889,210 B(2+1=3.map 544×158)"不成立——全 807 图实测 **max(W+H) = 499**(`Maps\MadHQ\避暑山庄.map` 与 `非常之观.map`,均 399×100),**最大瓦片流 = `2+1=3.map`(272×158)943,738 B** = 11·543·158+4。这不动任何判定公式([Map]Size 逐图给界),只影响文档里的"语料极值"表述与 300×300 兜底常量的裕量判断(943,738 < 1,976,704 ✓)。
3. **真实 LZO 向量(Vector A)**:`Maps\Standard\moonpatr.map` 的 `[PreviewPack]` 第 3 块(末块):块头 (size_in, size_out) = **(940, 932)**,载荷 940 字节(十六进制全文钉死在 Task 1 测试常量 `REAL_PREVIEW_CHUNK_HEX`),解压产出 932 字节,产出 SHA-256 前 16 hex = **`2805014e9aa0295d`**,产出首 16 字节 = `94 92 7e 90 8e 7b 7a 89 7e 61 88 7a 68 88 7d 85`;载荷 sha16 = `9ead542168482906`(仅供复核)。该块是"255 游程扩展字面量 + EOF 标记"的天然实例(载荷首 4 字节 `00 00 00 95`:token 0 + 3 个零 + 终止字节 0x95 → 3·255+15+149+3 = 932)。Rust/Python 双实现产出哈希一致。
4. **OverlayPack chunk 0 手工样例复现**:`c1a01md.map` 的 `[OverlayPack]` 首块原始字节 `09 00 00 20 81 FF FE FE 1F FF 81 FF 80`(size_in=9, size_out=8192;LCW:1 字面量 FF + 8190 填充 FF + 1 字面量 FF + 终止)与研究文档 §2.1 逐字节一致,已钉进 Task 4 测试。
5. **IsoMapPack5 记录样例复现**:`c1a01md` 块 0 块头 `eb 0f 00 20`(3819, 8192),载荷操作码 `00 05` 后 rec0 = `57 00 01 00 3c 01 00 00 01 02 00`(x=87,y=1,tile=316,sub=1,z=2)、rec1 = `56 00 02 00 3c 01 00 00 02 02 00`;末记录 `48 00 9d 00 00 00 00 00 00 02 00` 后跟 4 零尾巴 `00 00 00 00`。与研究文档 §3.5 一致。
6. **剧场调色板链**(extract-tile 用):`ra2.mix`(明文,22 项)→ 嵌套调色板库 **id `0x3B5A96DE`**(明文,44 项,按名可读)→ `isotem.pal`/`isosno.pal`/`isourb.pal` 各 768 B,sha16 分别 **`5d6e40fcd11a592a` / `9f411f95251d7e62` / `b1f4c15b7b911307`**;`temperat.pal` == `snow.pal`(sha16 `5903b69868b84f49`)。**`isodes.pal`/`isoubn.pal`/`isolun.pal` 在本机安装中缺席**(研究文档 §14-1 结论复现)——extract-tile 对 DESERT/NEWURBAN/LUNAR 图报错退出 1(提示缺调色板),验收用 TEMPERATE/SNOW/URBAN 图。
7. **剧场交叉验证上界**(spec §6-5):`snow.mix`(ra2.mix,379 项)含 **343 个 RA2 TMP 文件、共 6804 瓦片**;`SNOWMD.MIX`(ra2md.mix,82 项)含 81 个 TMP、3212 瓦片;合计 **10016 瓦片 → 合法 tile 索引上界 10015**。语料 106 张 SNOW 图实测最大 tile = **963**(`Maps\MadHQ\交流意见.map`)。
8. **截断 PreviewPack 7 张名单**(%4 == 2,全在 `Maps\DDLY`):`一夫当关万夫莫开` / `城郊之战` / `对峙一块地` / `终极茅厕` / `肉人大作战八人图` / `肉人大战6人图` / `降维打击三人防守图`。其中**城郊之战的干净前缀恰为 48600 B = 3·180·90**(7 个干净块后跟异常末块 size_in=8、size_out=56994)——按 §5 契约它仍产出完整预览;其余 6 张前缀不足 3·pw·ph → "无预览"。全 807 图 base64 字符集零违规;缺 `[Digest]` 的 3 张 = `小块地之田野之隔` / `小块地宝箱大战` / `沙漠坦克遭遇战4人图`;缺 `[IsoMapPack5]` 的 1 张 = `Resources\RandomMapGenerator_RA2\templateMap.map`。
9. **INI 取证补钉**:勇士与巨龙.map 第 6187 行原文 `69=NeutralGAWEAT,256,138,264,0,None,1,0,1,0,0,None,None,None,1,0`(16 字段,owner+type 丢逗号),该图 `[Structures]` 共 160 项;RMG `1,1,1,1,01,spawn.map` 首个段前孤儿键值行 **44** 行、瓦片记录 1770 = 59·30 精确;`indicator.map` `[Map]Size=0,0,1,1`、瓦片记录恰 1 条、`[Preview]Size=0,0,2,1` → 预览恰 6 B;`fcrash_backup.map` 无 `[Basic]` 段(38 段)。digest 样本 `1=qSt1w6I0r+9EWkfWl7XFZMt0Vo4=` 解码 20 B:hex `a92b75c3a234afef445a47d697b5c564cb74568e`。
10. **GBK 字节钉死**(Task 3 测试向量):`城市海啸.map` 的 Name 原始 GBK 字节 `ba a3 87 5b` → "海嘯";`随机地图.map` 是真 UTF-8 `e9 9a 8f e6 9c ba e5 9c b0 e5 9b be` → "随机地图"(证明 UTF-8 优先序不可反);`分而治之.map` Name `[4]分而治之` = `5b 34 5d b7 d6 b6 f8 d6 ce d6 ae`;截断序列 `31 cd e6 bc d2 d4 ae be`(`终极海战防守图.map`)→ 有损解码 `1玩家援\u{FFFD}`(Python gbk/replace 对拍一致)。
11. **ISOTEMP.MIX 形态**(决定 extract-tile 设计):`ISOTEMP.MIX` 不是单文件 blob,而是 **1035 项的摘要位 MIX**(flags=0x00010000;ISODES/ISOSNOW/ISOURB 同构)。tile 索引 → 具体位图的映射(首项 51984 B、内部为递增 u32 偏移表)留 M3 取证;因此 M2 的 `extract-tile` 输出该格 60×30 菱形的**调色板占位渲染**(填充色 = 剧场 ISO 调色板第 `tile % 256` 项),在 `--help` 与 README 中如实注明,真瓦片位图归 M3。
12. **lcw_decode 返回值语义**:现有 `lcw_decode(src, out) -> Ok(累计产出长度)` 不返回源消耗量;包段循环需要 `consumed == size_in` 核对 → Task 4 在 `lcw.rs` 增加 `lcw_decode_consumed`(核心循环改名搬移,原函数变包装),既有 M1b 调用点(shp.rs/tmp.rs)零改动。

## Global Constraints

- 许可证:GPL-3.0-only。规格来源为 GPL 的 XCC 源码、Linux 内核 lzo1x_decompress_safe.c(GPL-2.0)与用户自有样本的字节级逆向研究——从语义用 Rust 重写,不逐行翻译;涉及来源的模块头注释注明出处。
- 仓库不提交任何 EA 资产;`fixtures/` 已 gitignore;真实资产测试用环境变量 `RA2_ASSETS_DIR` 指向游戏目录(值 `"E:\hongjing\Reunion 2023"`),测试标 `#[ignore]`。
- 每个 src/tests 文件第一行:`// SPDX-License-Identifier: GPL-3.0-only`
- 解析器全部接受内存字节切片,零 IO、**零 panic**(所有切片/索引用 ByteReader 或显式边界检查;宽计数算术用显式宽类型/checked 算术——debug 构建溢出检查同样不得 panic,照 M1b HVA u128 教训)。敌意输入(截断、坏 base64、尺寸谎报、重复段)一律 Err,不无界分配:所有容量分配前先用输入长度对声明值推导上界。
- 错误复用 `FormatError` 三变体,语义全计划统一:`UnexpectedEof` = 截断,`Unsupported` = 合法但本版本不支持(未知剧场等),`Invalid` = 畸形。
- 确定性:核心解析器无 HashMap/HashSet(稀疏 (x,y) 索引用"排序 Vec + 二分");f32 仅作 [Lighting] 字段存取,不做浮点决策。
- Rust 不在 PATH:所有 cargo/rustc 命令前 `export PATH="$HOME/.cargo/bin:$PATH"`。
- 计数基线(2026-11-04 实测):`cargo test --workspace` = **85 合成**(79 core + 6 cli);`RA2_ASSETS_DIR=... cargo test --workspace -- --ignored` = **21**(13 format_samples + 1 m1b_acceptance + 7 mix_integration)。每任务 Run 步骤给出期望累计值,最终 **合成 188 / ignored 27**。
  > **计数勘误(2026-11-04,Task 3 审查 Critical 修复)**:gbk_lossy lead 0xFF 越界 panic 的修复附带 1 个回归测试 → Task 3 后合成计数 118+1=**119**,此后每任务 Run 步骤的期望累计值整体 +1(136→137 / 146→147 / 151→152 / 160→161 / 169→170 / 180→181 / 188→**189**);ignored 链不变(22/25/26/27)。
  >
  > **计数勘误 2(2026-11-04,Task 9 实现时实证,控制器已独立复核)**:Task 9 简报 Step-2 实含 **12 个测试函数而非 11**(mod.rs 12 个 `#[test]`,控制器 grep 复核),且 pack.rs lenient 契约修复附带 1 个回归钉(`preview_lenient_stops_at_size_in_lie`,pack.rs:591)→ Task 9 后合成计数 170+12+1=**183**(非 181);ignored 链 22+3(map_samples 新增 corpus_walk/snow_tile_index/truncated_preview_group)=**25**(原计划此处为 22)。后续链整体移位:Task 10 期望合成 **191**/ignored 26,Task 11 合成 191/ignored 27。终账以 **191/27** 为准。
  >
  > **计数勘误 3(2026-11-04,Task 10 审查 Important-1 修复)**:调色板 6 位→8 位换算修复(pal.rs 新增 `rgb8` + 测试 `converts_6bit_to_8bit`,提交 6df81fe)附带 1 个合成测试 → Task 10 后合成计数 191+1=**192**;ignored 不变(26)。终账修正为 **192/27**。
- 每个任务收尾:`cargo fmt --all`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿后才 commit;涉真实资产的任务另跑 `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored` 全绿。clippy 新近 deny 级 lint 靠预避免(切片分块用 `as_chunks` 风格,循环用迭代器/zip;不得 `#[allow]` 压告警)。
- commit 用 conventional commits(中文描述可),每条 commit 末尾附:`Co-Authored-By: Claude Code <noreply@anthropic.com>`(用第二个 `-m` 传入)。
- 零第三方依赖;SHA-256 只出现在测试(自写 FIPS 180-4 实现);`[Digest]` 只读取不校验。
- 本计划不做(spec §3 边界):渲染/wgpu(M3+)、地图写入、触发器/任务逻辑语义化(logic 段仅 tokenize 保留原始键值行)、tile 索引 → TMP 位图解析(M3 取证,见规格依据 §11)、rules 依赖语义、get_float(rules 里程碑)、TS 变体 TMP、Blowfish MIX。`[Header]Waypoint1..8`/`StartX`/`StartY` 语义未知(研究 §9)→ 只按 get_int 原样透传,不解释。

---

### Task 1: LZO1X 安全解压器(`format/lzo1x.rs`)

**Files:**
- Create: `crates/core/src/format/lzo1x.rs`
- Modify: `crates/core/src/format/mod.rs`(追加 `pub mod lzo1x;`,置于 `pub mod lcw;` 之后)

**Interfaces:**
- Consumes: `FormatError`(`crates/core/src/format/mod.rs`,不改)
- Produces(供 Task 4 pack.rs 调用):
  - `pub fn lzo1x_decode(src: &[u8], out_cap: usize, out: &mut Vec<u8>) -> Result<usize, FormatError>` — 解码一个完整 LZO1X 流,从 `out.len()` 起追加至多 `out_cap` 字节,返回产出字节数。成功 ⇒ 输入恰好耗尽(eof_found 语义);"产出 ≠ 块头 size_out" 由调用方核对。错误映射:输入过读/输出越界 → `UnexpectedEof`;回溯越界(lookbehind)、EOF 标记长度 ≠ 3、EOF 后残留字节、255 计数溢出 → `Invalid`。
- 语义要点(内核 `lib/lzo/lzo1x_decompress_safe.c` + lzodefs.h,勿改):版本头 `17,<ver>` 按 `in_len >= 5 && 首 == 17` 识别,有无都接受(ver 仅启用 2020 零游程扩展);首字节 > 17 走 `t = 首-17` 特例(<4 转 match_next,否则整段字面量);操作码族 = state-0 字面量(t+3,t==0 时 255 扩展)/ M1(state∉{0,4},恒长 2)/ M2 短(state==4,基准偏移 0x800)/ M2(t≥64,(t>>5)+1)/ M3(32..63,(t&31)+2,=2 时 255 扩展)/ M4(16..31,总偏移 = ((tok&8)<<11)+(next>>2)+0x4000,=2 时 255 扩展,且**首 le16 与重读 le16 共享字节**——扫描从 next 首字节开始,扩展后 next 整体重读);EOF 由 m_pos==op 且 t==3 判定;所有字面量拷贝统一 `NEED_IP(t+3)`(3 字节余量使循环头至少可读一个操作码);M4 零游程扩展(仅版本头流,next&0xFFFC==0xFFFC && tok&0xF8==0x18,长度 = (tok&7)|(count<<3) + 4)照内核实现。

- [ ] **Step 1: 写 `lzo1x.rs` 失败测试 + 实现(同文件;`mod.rs` 先追加 `pub mod lzo1x;`;TDD 以 Step 2 的编译失败确认)**

`crates/core/src/format/mod.rs` 在 `pub mod lcw;` 之后追加:

```rust
pub mod lzo1x;
```

`crates/core/src/format/lzo1x.rs` 完整文件(实现 + 19 个测试;Vector A 十六进制全文来自规格依据 §3,2026-11-04 从 moonpatr.map `[PreviewPack]` 块 2 逐字节提取):

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! LZO1X 解压(miniLZO 兼容;`[IsoMapPack5]` / `[PreviewPack]` 的 codec)。
//!
//! 依据 Linux 内核 `lib/lzo/lzo1x_decompress_safe.c`(GPL-2.0,
//! Markus F.X.J. Oberhumer;CVE-2014-4608 加固后的 "safe" 变体)的规范语义
//! 用 Rust 重写。对应关系:NEED_IP / NEED_OP / TEST_LB 逐一映射为显式边界
//! 检查;输入过读/输出越界 → `UnexpectedEof`;回溯越界(lookbehind)、EOF
//! 标记长度 ≠ 3、EOF 后残留字节、255 计数溢出 → `Invalid`。
//! 流可以 2 字节版本头 `17,<ver>` 开头(内核按 `in_len >= 5 && 首字节 == 17`
//! 识别,带与不带都接受;<ver> 仅启用内核 2020 的零游程扩展)。EOF 由
//! "源==目标"的匹配(m_pos == op,要求 t == 3)判定;成功返回即输入恰好耗尽。

use super::FormatError;

/// M2 短匹配(state==4 路径)的基准偏移(内核 lzodefs.h M2_MAX_OFFSET)。
const M2_MAX_OFFSET: usize = 0x0800;
/// 内核零游程扩展的最小长度(lzodefs.h MIN_ZERO_RUN_LENGTH)。
const MIN_ZERO_RUN_LENGTH: usize = 4;
/// 内核 MAX_255_COUNT:255 游程计数溢出防护上限(usize::MAX / 255 - 2)。
const MAX_255_COUNT: usize = usize::MAX / 255 - 2;

fn eof(needed: usize, remaining: usize) -> FormatError {
    FormatError::UnexpectedEof { needed, remaining }
}

fn invalid(msg: &str) -> FormatError {
    FormatError::Invalid(msg.to_string())
}

/// 拷贝 `count` 个字面量。对应内核所有字面量拷贝点共用的
/// `NEED_IP(t + 3); NEED_OP(t); do *op++ = *ip++; while (--t)`——3 字节余量
/// 保证循环头至少还能读出一个操作码。
fn copy_literals(
    src: &[u8],
    ip: &mut usize,
    out: &mut Vec<u8>,
    op: &mut usize,
    out_cap: usize,
    count: usize,
) -> Result<(), FormatError> {
    if src.len() - *ip < count + 3 {
        return Err(eof(count + 3, src.len() - *ip));
    }
    if out_cap - *op < count {
        return Err(eof(count, out_cap - *op));
    }
    for _ in 0..count {
        out.push(src[*ip]);
        *ip += 1;
        *op += 1;
    }
    Ok(())
}

/// 255 游程扩展(内核三处同构的 `while (*ip == 0) { ip++; NEED_IP(1); }`):
/// 返回(扫过的零字节数, 终止字节)。消费不含终止字节。
fn read_255_run(src: &[u8], ip: &mut usize) -> Result<(usize, u8), FormatError> {
    let mut zeros = 0usize;
    while *ip < src.len() && src[*ip] == 0 {
        *ip += 1;
        zeros += 1;
    }
    if src.len() - *ip < 1 {
        return Err(eof(1, 0));
    }
    if zeros > MAX_255_COUNT {
        return Err(invalid("LZO 255 游程计数溢出"));
    }
    let term = src[*ip];
    *ip += 1;
    Ok((zeros, term))
}

/// 从 `m_pos` 逐字节正拷 `count` 字节(允许源/目的重叠,与内核字节路径语义
/// 一致)。`m_pos < op` 不变式由调用方的 checked_sub 保证。
fn copy_match(
    out: &mut Vec<u8>,
    base: usize,
    m_pos: usize,
    op: &mut usize,
    out_cap: usize,
    count: usize,
) -> Result<(), FormatError> {
    if out_cap - *op < count {
        return Err(eof(count, out_cap - *op));
    }
    for i in 0..count {
        out.push(out[base + m_pos + i]);
        *op += 1;
    }
    Ok(())
}

/// 解码一个完整 LZO1X 流,把不超过 `out_cap` 字节追加到 `out`(自 `out.len()`
/// 起计),返回产出字节数。成功 ⇒ 输入恰好耗尽;"产出 ≠ 块头 size_out"
/// 由调用方核对。
pub fn lzo1x_decode(src: &[u8], out_cap: usize, out: &mut Vec<u8>) -> Result<usize, FormatError> {
    let in_len = src.len();
    if in_len < 3 {
        return Err(eof(3, in_len));
    }
    let base = out.len();
    let mut ip = 0usize;
    let mut bitstream_version = 0u8;
    // 内核 :58-63 的版本头识别
    if in_len >= 5 && src[0] == 17 {
        bitstream_version = src[1];
        ip = 2;
    }
    let mut op = 0usize; // 相对 base 的输出位置
    let mut state = 0usize;

    // 首字节特例(内核 :65-72):t = 首 - 17,<4 转 match_next,否则整段字面量
    if src[ip] > 17 {
        let t = (src[ip] - 17) as usize;
        ip += 1;
        state = if t < 4 { t } else { 4 };
        copy_literals(src, &mut ip, out, &mut op, out_cap, t)?;
    }
    loop {
        if src.len() - ip < 1 {
            return Err(eof(1, 0));
        }
        let tok = src[ip];
        ip += 1;
        let mut t = tok as usize;
        let mut next;
        let mut m_pos;
        if t < 16 {
            if state == 0 {
                if t == 0 {
                    let (zeros, term) = read_255_run(src, &mut ip)?;
                    t += zeros.wrapping_mul(255) + 15 + term as usize;
                }
                t += 3;
                copy_literals(src, &mut ip, out, &mut op, out_cap, t)?;
                state = 4;
                continue;
            } else if state != 4 {
                // M1:2 字节匹配,长度恒 2,偏移 ≤ 1+63+255·4
                next = t & 3;
                if src.len() - ip < 1 {
                    return Err(eof(1, 0));
                }
                let back = 1 + (t >> 2) + ((src[ip] as usize) << 2);
                ip += 1;
                m_pos = op.checked_sub(back).ok_or_else(|| invalid("LZO 回溯越界"))?;
                if out_cap - op < 2 {
                    return Err(eof(2, out_cap - op));
                }
                out.push(out[base + m_pos]);
                out.push(out[base + m_pos + 1]);
                op += 2;
                state = next;
                copy_literals(src, &mut ip, out, &mut op, out_cap, next)?;
                continue;
            } else {
                // M2 短匹配(state==4),偏移 > M2_MAX_OFFSET
                next = t & 3;
                if src.len() - ip < 1 {
                    return Err(eof(1, 0));
                }
                let back = 1 + M2_MAX_OFFSET + (t >> 2) + ((src[ip] as usize) << 2);
                ip += 1;
                m_pos = op.checked_sub(back).ok_or_else(|| invalid("LZO 回溯越界"))?;
                t = 3;
            }
        } else if t >= 64 {
            // M2:2 字节常规短匹配,长度 (t>>5)+1,偏移 1..=2044
            next = t & 3;
            if src.len() - ip < 1 {
                return Err(eof(1, 0));
            }
            let back = 1 + ((t >> 2) & 7) + ((src[ip] as usize) << 3);
            ip += 1;
            m_pos = op.checked_sub(back).ok_or_else(|| invalid("LZO 回溯越界"))?;
            t = (t >> 5) + 1;
        } else if t >= 32 {
            // M3:2 字节 + u16 偏移,长度 (t&31)+2(=2 时 255 扩展)
            t = (t & 31) + 2;
            if t == 2 {
                let (zeros, term) = read_255_run(src, &mut ip)?;
                t += zeros.wrapping_mul(255) + 31 + term as usize;
                if src.len() - ip < 2 {
                    return Err(eof(2, src.len() - ip));
                }
            }
            m_pos = op.checked_sub(1).ok_or_else(|| invalid("LZO 回溯越界"))?;
            next = u16::from_le_bytes([src[ip], src[ip + 1]]) as usize;
            ip += 2;
            m_pos = m_pos
                .checked_sub(next >> 2)
                .ok_or_else(|| invalid("LZO 回溯越界"))?;
            next &= 3;
        } else {
            // M4(16..=31):总偏移 = ((tok&8)<<11) + (next>>2) + 0x4000
            if src.len() - ip < 2 {
                return Err(eof(2, src.len() - ip));
            }
            next = u16::from_le_bytes([src[ip], src[ip + 1]]) as usize;
            if (next & 0xFFFC) == 0xFFFC && (tok & 0xF8) == 0x18 && bitstream_version != 0 {
                // 内核 2020 零游程扩展(仅版本头流):4 字节 [tok][lo][hi][count]
                if src.len() - ip < 3 {
                    return Err(eof(3, src.len() - ip));
                }
                t = ((tok & 7) as usize) | ((src[ip + 2] as usize) << 3);
                t += MIN_ZERO_RUN_LENGTH;
                if out_cap - op < t {
                    return Err(eof(t, out_cap - op));
                }
                for _ in 0..t {
                    out.push(0);
                    op += 1;
                }
                next &= 3;
                ip += 3;
                state = next;
                copy_literals(src, &mut ip, out, &mut op, out_cap, next)?;
                continue;
            }
            m_pos = op
                .checked_sub(((tok & 8) as usize) << 11)
                .ok_or_else(|| invalid("LZO 回溯越界"))?;
            t = ((tok & 7) as usize) + 2;
            if t == 2 {
                // 255 扩展:首 le16 与重读 le16 共享字节——扫描从 next 的首字节
                // 开始(kernel :187-203),扩展后 next 整体重读
                let (zeros, term) = read_255_run(src, &mut ip)?;
                t += zeros.wrapping_mul(255) + 7 + term as usize;
                if src.len() - ip < 2 {
                    return Err(eof(2, src.len() - ip));
                }
                next = u16::from_le_bytes([src[ip], src[ip + 1]]) as usize;
            }
            ip += 2;
            m_pos = m_pos
                .checked_sub(next >> 2)
                .ok_or_else(|| invalid("LZO 回溯越界"))?;
            next &= 3;
            if m_pos == op {
                // EOF 标记:源==目标的匹配(内核 eof_found)
                if t != 3 {
                    return Err(invalid(&format!("LZO EOF 标记匹配长度 {t} != 3")));
                }
                if ip != src.len() {
                    return Err(invalid(&format!("LZO EOF 后仍剩 {} 字节", src.len() - ip)));
                }
                return Ok(op);
            }
            m_pos = m_pos
                .checked_sub(0x4000)
                .ok_or_else(|| invalid("LZO 回溯越界"))?;
        }
        copy_match(out, base, m_pos, &mut op, out_cap, t)?;
        state = next;
        copy_literals(src, &mut ip, out, &mut op, out_cap, next)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 解码便捷封装(与正式 API 相同,只是替测试攒 out)。
    fn dec(src: &[u8], cap: usize) -> Result<Vec<u8>, FormatError> {
        let mut out = Vec::new();
        lzo1x_decode(src, cap, &mut out).map(|_| out)
    }

    fn eq_ok(v: &Result<Vec<u8>, FormatError>, want: &[u8]) -> bool {
        matches!(v, Ok(o) if o == want)
    }

    /// 最小 SHA-256(FIPS 180-4),仅测试用(Vector A 的产出哈希断言)。
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
        let mut o = [0u8; 32];
        for i in 0..8 {
            o[i * 4..i * 4 + 4].copy_from_slice(&h[i].to_be_bytes());
        }
        o
    }

    fn sha16(data: &[u8]) -> String {
        let d = sha256(data);
        (0..8).map(|i| format!("{:02x}", d[i])).collect()
    }

    fn hex_bytes(s: &str) -> Vec<u8> {
        s.as_bytes()
            .chunks_exact(2)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
            .collect()
    }

    #[test]
    fn decodes_first_token_literal_run() {
        // 0x15 = 17+4:首字节特例的整段字面量路径(t=4,NEED_IP(4+3) 恰含 EOF 标记)
        let v = dec(&[0x15, b'A', b'B', b'C', b'D', 0x11, 0, 0], 100);
        assert!(eq_ok(&v, b"ABCD"));
    }

    #[test]
    fn decodes_in_loop_literal_run() {
        // token 12 < 16 且 state==0:t = 12+3 = 15 字面量
        let mut src = vec![0x0Cu8];
        src.extend_from_slice(b"0123456789abcde");
        src.extend_from_slice(&[0x11, 0, 0]);
        let v = dec(&src, 100);
        assert!(eq_ok(&v, b"0123456789abcde"));
    }

    #[test]
    fn decodes_255_extended_literal_run() {
        // token 0,无零扩展(zeros=0,term=0x1A=26):t = 15+26+3 = 44
        let mut src = vec![0x00u8, 0x1A];
        src.extend_from_slice(b"X".repeat(44).as_slice());
        src.extend_from_slice(&[0x11, 0, 0]);
        let v = dec(&src, 200);
        assert!(eq_ok(&v, &vec![b'X'; 44]));
    }

    #[test]
    fn decodes_255_extended_literal_run_with_zeros() {
        // token 0,1 个零字节(zeros=1,term=4):t = 255+15+4+3 = 277
        let mut src = vec![0x00u8, 0x00, 0x04];
        src.extend_from_slice(b"Y".repeat(277).as_slice());
        src.extend_from_slice(&[0x11, 0, 0]);
        let v = dec(&src, 400);
        assert!(eq_ok(&v, &vec![b'Y'; 277]));
    }

    #[test]
    fn decodes_m2_short_match() {
        // 0xC0:back = 1+((0xC0>>2)&7)+(0<<3) = 1,len = (0xC0>>5)+1 = 7 → ABCD + 7×'D'
        let v = dec(&[0x15, b'A', b'B', b'C', b'D', 0xC0, 0x00, 0x11, 0, 0], 100);
        assert!(eq_ok(&v, b"ABCDDDDDDDD"));
    }

    #[test]
    fn decodes_m2_with_next_literal_then_m1() {
        // 0xC1(next=1)+字面量 'E' → state=1;token 0x04 走 M1:back = 1+(4>>2)+0 = 2,
        // 拷 2 字节;锁定 match_next 的尾部字面量与 M1 的 state 记账
        let v = dec(
            &[0x15, b'A', b'B', b'C', b'D', 0xC1, 0x00, b'E', 0x04, 0x00, 0x11, 0, 0],
            100,
        );
        assert!(eq_ok(&v, b"ABCDDDDDDDDEDE"));
    }

    #[test]
    fn decodes_m3_match() {
        // 0x21:len = (0x21&31)+2 = 3,next=4 → 偏移 = 1+4>>2 = 2,重叠拷出 CDC
        let v = dec(&[0x15, b'A', b'B', b'C', b'D', 0x21, 0x04, 0x00, 0x11, 0, 0], 100);
        assert!(eq_ok(&v, b"ABCDCDC"));
    }

    #[test]
    fn decodes_m3_255_extended() {
        // 0x20:len = 2 → 255 扩展(zeros=0,term=24):len = 2+31+24 = 57,next=4 → 偏移 2;
        // 重叠拷出 C/D 交替序列,末字节(下标 60)为 'C'
        let src = [0x15u8, b'A', b'B', b'C', b'D', 0x20, 0x18, 0x04, 0x00, 0x11, 0, 0];
        let v = dec(&src, 200);
        assert!(matches!(&v, Ok(o) if o.len() == 4 + 57 && o[..4] == *b"ABCD" && o[4] == b'C' && o[60] == b'C'));
    }

    #[test]
    fn decodes_m4_long_match_and_255_extension() {
        // M4 总偏移 = ((tok&8)<<11)+(next>>2)+0x4000 ≥ 16385:v8(tok=0x11,bit3=0,
        // next=4 → 偏移 16385)、v9(tok=0x18:bit3 置位,255 扩展 len 13,重读 next=4
        // → 偏移 32769;注意首 le16 读 (b1,b2)、扩展重读 (b2,b3),操作码 4 字节
        // [18 04 04 00])。先用 M3(len 33,偏移 2)增长过 0x8000。
        let mut src = vec![0x15u8, b'A', b'B', b'C', b'D'];
        let k = (32_900 - 4 + 32) / 33;
        for _ in 0..k {
            src.extend_from_slice(&[0x3F, 0x04, 0x00]);
        }
        let mut expected = b"ABCD".to_vec();
        for _ in 0..k {
            let n = expected.len();
            for i in 0..33 {
                expected.push(expected[n - 2 + i]);
            }
        }
        src.extend_from_slice(&[0x11, 0x04, 0x00]);
        let n = expected.len();
        for i in 0..3 {
            expected.push(expected[n - 0x4000 - 1 + i]);
        }
        src.extend_from_slice(&[0x18, 0x04, 0x04, 0x00]);
        let n = expected.len();
        for i in 0..13 {
            expected.push(expected[n - 0x8000 - 1 + i]);
        }
        src.extend_from_slice(&[0x11, 0x00, 0x00]);
        let mut out = Vec::new();
        let r = lzo1x_decode(&src, 100_000, &mut out);
        assert!(matches!(r, Ok(len) if len == expected.len()));
        assert_eq!(out, expected);
    }

    #[test]
    fn decodes_zero_run_with_version_header() {
        // 版本头 17,02;零游程操作码 0x18-0x1F 不能作首字节(>17 会走字面量特例),
        // 故先放一段字面量。next 两字节必须满足 next&0xFFFC==0xFFFC(FC FF)。
        // len = (0x18&7)|((0x05)<<3) + 4 = 44 个零
        let v = dec(
            &[0x11, 0x02, 0x15, b'A', b'B', b'C', b'D', 0x18, 0xFC, 0xFF, 0x05, 0x11, 0x00, 0x00],
            100,
        );
        let mut want = b"ABCD".to_vec();
        want.extend_from_slice(&[0u8; 44]);
        assert!(eq_ok(&v, &want));
    }

    #[test]
    fn decodes_m2_short_large_offset() {
        // state==4 路径(t<16):先字面量(state=4)→ M2(back 1)→ 字面量(state=0
        // 再转字面量分支)→ M3×55 增长 → 字面量 → M2 短匹配(back = 1+2048+1 = 2050)
        let mut src = vec![0xEFu8]; // 239-17 = 222 字面量
        src.extend_from_slice(&(0u8..222).collect::<Vec<u8>>());
        src.extend_from_slice(&[0xC0, 0x00]); // back 1,len 7,next 0
        src.extend_from_slice(&[0x0F]); // state 0 → 字面量 18
        src.extend_from_slice(&(200u8..218).collect::<Vec<u8>>());
        for _ in 0..55 {
            src.extend_from_slice(&[0x3F, 0x04, 0x00]); // len 33,偏移 2
        }
        src.extend_from_slice(&[0x0F]); // state 0 → 字面量 18
        src.extend_from_slice(&(0u8..18).collect::<Vec<u8>>());
        // op = 222+7+18+55*33+18 = 2080;M2 短:back = 1+2048+(0x04>>2)+0 = 2050
        src.extend_from_slice(&[0x04, 0x00]);
        src.extend_from_slice(&[0x11, 0x00, 0x00]);
        let mut out = Vec::new();
        let r = lzo1x_decode(&src, 10_000, &mut out);
        let n = out.len();
        assert!(matches!(r, Ok(m) if m == n));
        assert_eq!(n, 222 + 7 + 18 + 55 * 33 + 18 + 3);
        assert_eq!(out[n - 3..], out[n - 3 - 2050..n - 3 - 2050 + 3]);
    }

    #[test]
    fn rejects_eof_marker_with_length_not_3() {
        // tok 0x12:m_pos==op 满足但 t = (2)+2 = 4 ≠ 3 → Invalid(内核 LZO_E_ERROR)
        let v = dec(&[0x15, b'A', b'B', b'C', b'D', 0x12, 0x00, 0x00], 100);
        assert!(matches!(v, Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_trailing_bytes_after_eof() {
        // EOF 标记后残留字节 → Invalid(内核 LZO_E_INPUT_NOT_CONSUMED)
        let v = dec(&[0x15, b'A', b'B', b'C', b'D', 0x11, 0x00, 0x00, 0x01], 100);
        assert!(matches!(v, Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_truncated_literal_run() {
        // NEED_IP(4+3):只剩 4 字节 → UnexpectedEof
        let v = dec(&[0x15, b'A', b'B', b'C', b'D'], 100);
        assert!(matches!(v, Err(FormatError::UnexpectedEof { .. })));
    }

    #[test]
    fn rejects_output_overrun() {
        // 产出超过 out_cap → UnexpectedEof(内核 LZO_E_OUTPUT_OVERRUN)
        let v = dec(&[0x15, b'A', b'B', b'C', b'D', 0x11, 0, 0], 2);
        assert!(matches!(v, Err(FormatError::UnexpectedEof { .. })));
    }

    #[test]
    fn rejects_lookbehind_via_m3_offset() {
        // M3 偏移 (0x14>>2)=5 > op-1=4 → 回溯越界 → Invalid(内核 TEST_LB)
        let v = dec(&[0x15, b'A', b'B', b'C', b'D', 0x21, 0x14, 0x00], 100);
        assert!(matches!(v, Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_tiny_input() {
        // in_len < 3 → UnexpectedEof(内核 :55-56)
        let v = dec(&[0x11], 100);
        assert!(matches!(v, Err(FormatError::UnexpectedEof { .. })));
    }

    #[test]
    fn decodes_version_header_empty_stream() {
        // [17,00] 版本头 + [11 00 00] EOF → Ok(空产出);首字节 0x11 ≤ 17 进主循环
        let v = dec(&[0x11, 0x00, 0x11, 0x00, 0x00], 100);
        assert!(eq_ok(&v, b""));
    }

    /// 真实流向量(规格依据 §3):moonpatr.map [PreviewPack] 块 2(末块),
    /// 块头 (940, 932)。载荷以 255 游程扩展字面量开头(00 00 00 95 → 3 零 +
    /// 终止 0x95 → 3·255+15+149+3 = 932 个字面量),`11 00 00` EOF 结尾。
    /// 2026-11-04 从真图逐字节提取,Rust/Python 双实现产出哈希一致。
    const REAL_PREVIEW_CHUNK_HEX: &str = "000000009594927E908E7B7A897E61887A68887D858E7C868F7F8592777C897277857E829373768B71738D767896777A9C7A7DA3777AA46B6E9B61649365689A64679C5B5E935A5D946F72B17779C07577C07678C47274C26C6FBC7376CA797CD47476CD7274CD7A7DD77779D17678CC6F72C16F71BD7173BE6D6FB56F72B57D7FC48A8BD07B7EBC777AB37275AA7578A961648F6A6D9564678B5659787173917A7D97787A906B6D805F62745D61726A6E7E7E83907D838F7F86917F8C917F7C8D7F48867F3B847E5887806589815F88856B8D805F8880688984668C846B8B7F3D837D227F7E22818038857C5D877E818D8090917E8A908087927E85917C818F4E54644B50615D6173747689797B8E7274876265784B4F62595C6E747787787D8B7D82907B828D7F8890828C93747C856F7780808A90838E928795968A9797798488687278758084899496757F84798388828B90848D93839093819092808D928088927D838F7E8491757A89767A8A75788A6063795D5F795F617E747698797CA38B8DBB8D8FC17577A95D60916D70A96F72AE5F639E6669A96164A67376C3696CB76C6EBF5B5FAA575AA76E70CB787ADB7376D2797BDB7476D46C6FC47375CB6B6DBD686BB66E71BC686BAF7174B76669A663669E616499686B9E7376A76E719E6D7099696C91595C7C60638073769177799074778A7074856C7080797E8D7C818F7C828E7F8791838A94828E937F8A8F858D94889B978A9D98889C96899E968A9F968AA0958EA4998DA4998A999583778E7F6D8A7D858C7C8D8B808D90818A927F87908087928187927B808D696E7D727686737587828497797B8E5053666264775C5F72575A6D6E71827B7F8E7F849373798679808C787F8A5D6470606873707982838C93848D93869095717A8151596459626B656E76575F69646C75838B928189917F87907E85907F86917C828E7C828F7D82917B7F8F797D8E777A8C76788E6C6F89737593797B9E7679A07A7DA88385B69192CA8A8CC67A7DB77679B67274B57A7DC46F72B97477C36265AE6C6FC07A7DD8686BC05C5FB26A6DCB787BE57073D5696CC5696CC27477D07174C87578C9797CCB7578C2787AC16F71B07C7FBE787BB57275AA777AAC797BAB7A7DA87A7DA46669896B6E8B767893787B91787B8E797D8F7B7F907E82927D82907E85917F8691757C867179836F7981747F85778287838E918995968794948390908693938B9997889595889695889A96859694828E90828B91808A908089917E86907D848F7D838F7B818E7C81907D80907E80927C7E91787A8D110000";

    #[test]
    fn decodes_real_preview_chunk_from_moonpatr() {
        let payload = hex_bytes(REAL_PREVIEW_CHUNK_HEX);
        assert_eq!(payload.len(), 940);
        let mut out = Vec::new();
        let produced = lzo1x_decode(&payload, 932, &mut out).expect("真块应解码成功");
        assert_eq!(produced, 932);
        assert_eq!(out.len(), 932);
        assert_eq!(sha16(&out), "2805014e9aa0295d");
        assert_eq!(
            &out[..16],
            &[0x94, 0x92, 0x7e, 0x90, 0x8e, 0x7b, 0x7a, 0x89, 0x7e, 0x61, 0x88, 0x7a, 0x68, 0x88, 0x7d, 0x85]
        );
    }
}
```

- [ ] **Step 2: 运行单元测试通过**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p ra2-core lzo1x`
Expected: 19 passed(18 个手造向量 + 1 个真流向量;所有操作码族字面量/M1/M2/M2 短/M3/M3 扩展/M4/M4 扩展/零游程/EOF/敌意路径全覆盖)

- [ ] **Step 3: 真图锚定复核(非测试步骤,验证实现仍咬合真实字节)**

Task 9 的 map_samples 会再全链验证;此处先手动确认移植没有在转写中变形:

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core lzo1x -- --ignored` 此时无 ignored 测试,预期 `0 test results`(可跳过);真正的锚定在 Task 9 Step 3(五张 §13 指纹图全链)。本步仅确认 `cargo build -p ra2-core` 零告警:

Run: `cargo clippy -p ra2-core --all-targets -- -D warnings`
Expected: 零告警

- [ ] **Step 4: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 合成 104 passed(85 + 19)

```bash
git add crates/core/src/format/lzo1x.rs crates/core/src/format/mod.rs
git commit -m "feat(core): LZO1X 安全解压器(内核 safe 变体规范语义重写,真流向量锚定)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 2: IniDocument 普查适配(段头注释 / atoi 前缀 / 孤儿行计数 / 不变量锁定)

**Files:**
- Modify: `crates/core/src/format/ini.rs`(parse 的段头判定、get_int、结构体加孤儿行计数;测试模块追加)

**Interfaces:**
- Consumes: 无新依赖
- Produces(供 Task 3/9 使用):
  - `IniDocument::parse(text: &str) -> IniDocument` 行为扩展:段头 = 行首 `[` 且行内存在 `]`,取 `[` 与**最后一个 `]`** 之间内容 trim(容忍尾随注释/空白,普查 §3-5 的真实样例 `[FlakWeaponE6]\t\t; This belongs to Flak Cannon`);空段名 `[]` 跳过。
  - `pub orphan_key_values: usize`(IniDocument 新字段,Sync 无影响)= 首个段之前被丢弃的 `key=value` 行数(RMG MapUnit 半图策略:继续丢弃但计数上报,普查 §8-4)。
  - `get_int(section, key, default)` 改为原版 atoi 风格前缀解析(普查 §8-2 的锁定点):跳过前导空白,可选 `+`/`-`,十进制数字前缀;`900;2000` → 900,`12.5` → 12,`abc`/空 → default,超出 i64 饱和到 `i64::MAX/MIN`。存储值保持原样不变(M1b 测试锁定值内 `;` 原样保留)。
  - 其余行为零变化:ci 合并、last-wins、`Vec<(String,String)>` 有序 entries、get_bool 不动(普查 §7 裁决 "keep")。

- [ ] **Step 1: 写失败测试(`ini.rs` 测试模块追加 7 个)**

在 `ini.rs` 的 `mod tests` 末尾追加:

```rust
    #[test]
    fn headers_accept_trailing_comments_and_whitespace() {
        // 普查 §3-5 真实样例:Maps\DDLY\全新油桶一块地.map:156。旧行为会把
        // 8 个键值错挂到前一个段 [NAFLAK] 上
        let doc = IniDocument::parse(
            "[NAFLAK]\nDamage=55\n[FlakWeaponE6]\t\t; This belongs to Flak Cannon\nROF=15\n",
        );
        assert_eq!(doc.value("FlakWeaponE6", "ROF"), Some("15"));
        assert_eq!(doc.value("NAFLAK", "Damage"), Some("55"));
        assert_eq!(doc.value("NAFLAK", "ROF"), None);
        // 行尾空白变体
        let doc = IniDocument::parse("[SomeSection]   \nK=V\n");
        assert_eq!(doc.value("SomeSection", "K"), Some("V"));
        // 未闭合的 `[` 仍按垃圾行跳过(既有行为回归)
        let doc = IniDocument::parse("[未闭合\nK=V\n");
        assert_eq!(doc.sections.len(), 0);
    }

    #[test]
    fn get_int_parses_atoi_prefix() {
        // 普查 §3-10 的游戏内证据:Soylent=900;2000 读作 900
        let doc = IniDocument::parse(
            "[S]\nA=900;2000\nB=  42ab\nC=-13x\nD=+7\nE=12.5\nF=abc\nG=\nH=0  ; 注释\n",
        );
        assert_eq!(doc.get_int("S", "A", -1), 900);
        assert_eq!(doc.get_int("S", "B", -1), 42);
        assert_eq!(doc.get_int("S", "C", -1), -13);
        assert_eq!(doc.get_int("S", "D", -1), 7);
        assert_eq!(doc.get_int("S", "E", -1), 12); // atoi 在 '.' 截断
        assert_eq!(doc.get_int("S", "F", -1), -1); // 无数字 → default
        assert_eq!(doc.get_int("S", "G", -1), -1); // 空值 → default
        assert_eq!(doc.get_int("S", "H", -1), 0);
    }

    #[test]
    fn get_int_huge_number_saturates() {
        // 敌意长数字:饱和不 panic(debug 溢出检查下也不得 panic)
        let doc = IniDocument::parse("[S]\nA=99999999999999999999999999\nB=-99999999999999999999999999\n");
        assert_eq!(doc.get_int("S", "A", 0), i64::MAX);
        assert_eq!(doc.get_int("S", "B", 0), i64::MIN);
    }

    #[test]
    fn orphan_lines_before_first_section_are_counted() {
        // 普查 §3-4:12 张 RMG NEWURBAN 半图以孤儿键值行开头(如 1,1,1,1,01,spawn.map
        // 首段前 44 行)——继续丢弃但计数上报
        let doc = IniDocument::parse("Size=0,0,60,30\n1=AAAA\n2=BBBB\n[Map]\nTheater=SNOW\n");
        assert_eq!(doc.orphan_key_values, 3);
        assert_eq!(doc.sections.len(), 1);
    }

    #[test]
    fn entries_preserve_file_order() {
        // 普查 §8-5 载荷不变量锁定:entries 必须保持文件顺序(包段重组按 key 序
        // 重排,对象段按下标枚举——绝不允许换 HashMap/排序)
        let doc = IniDocument::parse("[IsoMapPack5]\n3=C\n1=A\n2=B\n");
        let sec = doc.section("IsoMapPack5").unwrap();
        let keys: Vec<&str> = sec.entries.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["3", "1", "2"]);
    }

    #[test]
    fn duplicate_key_last_wins_within_section() {
        // 普查 §8-6:语料 0 重复键,但行为必须冻结(与游戏一致)
        let doc = IniDocument::parse("[S]\nA=1\nA=2\n");
        assert_eq!(doc.value("S", "A"), Some("2"));
        assert_eq!(doc.section("S").unwrap().entries.len(), 1);
    }

    #[test]
    fn empty_brackets_section_is_skipped() {
        // 旧行为会创建空名段;新判定(空名跳过)更严,语料零 `[]`,行为冻结
        let doc = IniDocument::parse("[]\nA=1\n[Real]\nB=2\n");
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(doc.value("Real", "B"), Some("2"));
    }
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p ra2-core ini`
Expected: 编译失败(`orphan_key_values` 字段不存在);注释掉编译错误后可确认 4 个新行为测试失败(段头注释/get_int 前缀/孤儿计数/`[]`)

- [ ] **Step 3: 实现(`ini.rs` 三处修改)**

struct 定义改为:

```rust
pub struct IniDocument {
    pub sections: Vec<IniSection>,
    /// 首个段之前被丢弃的 key=value 行数(RMG MapUnit 半图;普查 §8-4:
    /// 继续丢弃但计数上报)
    pub orphan_key_values: usize,
}
```

`parse` 开头与段头判定替换(原 `ini.rs:15-40` 一带):

```rust
    pub fn parse(text: &str) -> IniDocument {
        let mut doc = IniDocument {
            sections: Vec::new(),
            orphan_key_values: 0,
        };
        let mut current: Option<usize> = None;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') {
                continue;
            }
            // 段头:行首 '[' 且行内出现 ']'(取最后一个 ']'——普查 §3-5 的
            // 真实样例段头带尾随 tab/注释);空名跳过
            let header_name = if line.starts_with('[') {
                line.rfind(']')
                    .map(|end| line[1..end].trim().to_string())
                    .filter(|n| !n.is_empty())
            } else {
                None
            };
            if let Some(name) = header_name {
                match doc
                    .sections
                    .iter()
                    .position(|s| s.name.eq_ignore_ascii_case(&name))
                {
                    Some(i) => current = Some(i),
                    None => {
                        doc.sections.push(IniSection {
                            name,
                            entries: Vec::new(),
                        });
                        current = Some(doc.sections.len() - 1);
                    }
                }
            } else if let Some((k, v)) = line.split_once('=') {
                let key = k.trim();
                if key.is_empty() {
                    continue;
                }
                if let Some(i) = current {
                    let entries = &mut doc.sections[i].entries;
                    match entries
                        .iter_mut()
                        .find(|(ek, _)| ek.eq_ignore_ascii_case(key))
                    {
                        Some(slot) => slot.1 = v.trim().to_string(),
                        None => entries.push((key.to_string(), v.trim().to_string())),
                    }
                } else {
                    doc.orphan_key_values += 1;
                }
            }
            // 其余行(垃圾)跳过
        }
        doc
    }
```

`get_int` 替换 + 新增私有解析函数:

```rust
    pub fn get_int(&self, section: &str, key: &str, default: i64) -> i64 {
        self.value(section, key)
            .and_then(parse_int_prefix)
            .unwrap_or(default)
    }
```

(放在 `impl IniDocument` 块外、`impl IniSection` 前)

```rust
/// 原版 atoi 风格前缀解析(普查 §8-2):跳过前导空白,可选 +/-,十进制数字
/// 前缀;无数字返回 None;超出 i64 饱和(checked 算术,debug 下零 panic)。
/// pub 供 map/meta.rs 的 CSV 字段解析复用。
pub fn parse_int_prefix(v: &str) -> Option<i64> {
    let t = v.trim_start();
    let b = t.as_bytes();
    let mut i = 0usize;
    let neg = match b.first() {
        Some(b'-') => {
            i = 1;
            true
        }
        Some(b'+') => {
            i = 1;
            false
        }
        _ => false,
    };
    let mut acc: i128 = 0;
    let mut digits = 0usize;
    for &c in &b[i..] {
        if !c.is_ascii_digit() {
            break;
        }
        match acc.checked_mul(10).and_then(|a| a.checked_add((c - b'0') as i128)) {
            Some(a) => acc = a,
            None => return Some(if neg { i64::MIN } else { i64::MAX }),
        }
        digits += 1;
    }
    if digits == 0 {
        return None;
    }
    let signed = if neg { -acc } else { acc };
    if signed > i64::MAX as i128 {
        Some(i64::MAX)
    } else if signed < i64::MIN as i128 {
        Some(i64::MIN)
    } else {
        Some(signed as i64)
    }
}
```

- [ ] **Step 4: 运行单元测试通过(既有 4 个测试零改动全绿)**

Run: `cargo test -p ra2-core ini`
Expected: 11 passed(既有 4:sections_case_insensitive_and_merged / values_trimmed_and_comment_lines_skipped / typed_accessors / garbage_lines_are_skipped + 新 7)

- [ ] **Step 5: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 合成 111 passed

```bash
git add crates/core/src/format/ini.rs
git commit -m "feat(core): IniDocument 普查适配(段头尾注释/atoi 前缀 get_int/孤儿行计数/不变量锁定)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 3: map/ 模块族脚手架 + GBK 有损解码 + 解码策略(mapdoc)

**Files:**
- Create: `crates/core/src/format/map/mod.rs`(脚手架,后续任务逐步追加 `pub mod`)
- Create: `crates/core/src/format/map/gbk_table.rs`(**生成文件**,由 Step 1 的脚本产出并入库)
- Create: `crates/core/src/format/map/gbk.rs`
- Create: `crates/core/src/format/map/mapdoc.rs`
- Create: `tools/gen_gbk_table.py`
- Modify: `crates/core/src/format/mod.rs`(追加 `pub mod map;`)

**Interfaces:**
- Consumes: `IniDocument`(Task 2 后形态)
- Produces(供 Task 7/9 使用):
  - `pub fn gbk_lossy(data: &[u8]) -> String`(gbk.rs)— CP936 有损解码:ASCII 直通、0x80 → €、双字节查生成表、未配对/未定义 → U+FFFD;全路径零 panic。
  - `pub fn decode_map_text(data: &[u8]) -> String`(mapdoc.rs)— UTF-8 严格优先,失败回退 `gbk_lossy`(普查 §8-3:28 张 GBK 图、5 张截断 GBK 图,绝不失败)。
  - `pub const GBK: [u16; 23940]`(gbk_table.rs,生成)— 索引 = `(lead-0x81)*190 + trail偏移`(trail<0x7F → trail-0x40,否则 trail-0x41)。
- 设计说明:spec §4 的模块清单没有单列 gbk —— 表是 190 KB 级生成数据,塞进 mapdoc.rs 会让"解码策略"文件不可读;独立 `gbk.rs`+`gbk_table.rs` 是 spec §4"mapdoc.rs 承担 GBK 容错解码"职责的物理拆分,已在 Self-Review 附录登记。

- [ ] **Step 1: 写生成脚本 `tools/gen_gbk_table.py`(完整文件)并生成表**

```python
#!/usr/bin/env python3
"""生成 CP936(GBK)双字节区 → Unicode 查找表(地图名有损解码用)。

用法:python tools/gen_gbk_table.py crates/core/src/format/map/gbk_table.rs
依赖 Python 3 标准库 gbk 编解码器(全平台内置);表生成后由 gbk.rs 的
钉字节单测验证正确性。布局:lead 0x81..=0xFE(126 个)× trail 0x40..=0xFE
除 0x7F(190 个)= 23940 项,未定义码位填 0xFFFD。
"""
import sys


def main() -> None:
    out = sys.argv[1] if len(sys.argv) > 1 else "crates/core/src/format/map/gbk_table.rs"
    chars = []
    for lead in range(0x81, 0xFF):
        for trail in range(0x40, 0xFF):
            if trail == 0x7F:
                continue
            try:
                chars.append(bytes([lead, trail]).decode("gbk"))
            except UnicodeDecodeError:
                chars.append("\uFFFD")
    assert len(chars) == 23940, len(chars)
    with open(out, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("// SPDX-License-Identifier: GPL-3.0-only\n\n")
        fh.write("//! CP936(GBK)双字节区 → Unicode 查找表。\n//!\n")
        fh.write("//! 本文件由 `tools/gen_gbk_table.py` 生成(Python 标准库 gbk 编解码器),\n")
        fh.write("//! 勿手改;重生成命令见该脚本 docstring。索引 = (lead-0x81)*190 +\n")
        fh.write("//! trail 偏移(0x40..=0x7E → trail-0x40;0x80..=0xFE → trail-0x41);\n")
        fh.write("//! 未定义码位为 0xFFFD。\n\n")
        fh.write("#[rustfmt::skip]\n")
        fh.write("pub const GBK: [u16; 23940] = [\n")
        for i in range(0, len(chars), 16):
            row = ", ".join(f"0x{ord(c):04X}" for c in chars[i:i + 16])
            fh.write("    " + row + ",\n")
        fh.write("];\n")
    print(f"written: {out}")


if __name__ == "__main__":
    main()
```

Run: `python tools/gen_gbk_table.py crates/core/src/format/map/gbk_table.rs && wc -l crates/core/src/format/map/gbk_table.rs`
Expected: `written: ...` 且约 1510 行(生成文件以 GPL 头开头、以 `];` 结尾)

- [ ] **Step 2: 写 `gbk.rs` / `mapdoc.rs` / `map/mod.rs` 失败测试 + 实现**

`crates/core/src/format/mod.rs` 追加:

```rust
pub mod map;
```

`crates/core/src/format/map/mod.rs`(脚手架):

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! 地图/地形解析层(M2)。
//!
//! 模块族:`pack`(四包段容器)、`iso`(IsoMapPack5 瓦片网格)、`overlay`
//! (双 512×512 网格)、`preview`(裸 RGB)、`objects`(对象段)、`meta`
//! (元数据段)、`mapdoc`(解码策略)、`gbk`/`gbk_table`(GBK 有损解码)。
//! 总入口 `MapFile::parse` 在后续任务落地。

pub mod gbk;
pub mod gbk_table;
pub mod mapdoc;
```

`crates/core/src/format/map/gbk.rs` 完整文件:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! GBK(CP936)有损解码——地图 INI 的兜底解码路径(mapdoc 调用)。
//!
//! 单字节区 = ASCII(0x00..=0x7F);0x80 = 欧元符号;0x81..=0xFE 为双字节
//! lead,配 trail 0x40..=0xFE(除 0x7F),查生成表 [`gbk_table::GBK`];
//! 尾部未配对 lead / 非法 trail / 未定义码位 → U+FFFD。全路径零 panic。

use super::gbk_table::GBK;

pub fn gbk_lossy(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len());
    let mut i = 0usize;
    while i < data.len() {
        let b = data[i];
        if b < 0x80 {
            out.push(b as char);
            i += 1;
        } else if b == 0x80 {
            out.push('\u{20AC}');
            i += 1;
        } else if i + 1 < data.len() {
            let trail = data[i + 1];
            if (0x40..=0xFE).contains(&trail) && trail != 0x7F {
                let t = if trail < 0x7F { trail - 0x40 } else { trail - 0x41 };
                let idx = (b as usize - 0x81) * 190 + t as usize;
                out.push(char::from_u32(GBK[idx] as u32).unwrap_or('\u{FFFD}'));
            } else {
                out.push('\u{FFFD}');
            }
            i += 2;
        } else {
            out.push('\u{FFFD}');
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_and_euro_passthrough() {
        assert_eq!(gbk_lossy(b"Name=Test"), "Name=Test");
        assert_eq!(gbk_lossy(&[0x80]), "\u{20AC}");
    }

    #[test]
    fn decodes_gbk_names_from_real_maps() {
        // 规格依据 §10:城市海啸.map Name 原始字节 → "海嘯";
        // 分而治之.map Name → "[4]分而治之"
        assert_eq!(gbk_lossy(&[0xBA, 0xA3, 0x87, 0x5B]), "海嘯");
        assert_eq!(
            gbk_lossy(&[0x5B, 0x34, 0x5D, 0xB7, 0xD6, 0xB6, 0xF8, 0xD6, 0xCE, 0xD6, 0xAE]),
            "[4]分而治之"
        );
    }

    #[test]
    fn truncated_trailing_lead_becomes_replacement() {
        // 规格依据 §10:终极海战防守图.map 的截断 Name 序列(Python gbk/replace 对拍)
        assert_eq!(
            gbk_lossy(&[0x31, 0xCD, 0xE6, 0xBC, 0xD2, 0xD4, 0xAE, 0xBE]),
            "1玩家援\u{FFFD}"
        );
    }

    #[test]
    fn invalid_trail_consumes_pair_and_replaces() {
        // trail 0x7F 非法:成对消费、单个替换,后续字节不受影响
        assert_eq!(gbk_lossy(&[b'A', 0xC7, 0x7F, b'B']), "A\u{FFFD}B");
    }
}
```

`crates/core/src/format/map/mapdoc.rs` 完整文件:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! 地图 INI 解码策略(普查 2026-11-04-map-ini-survey.md §8-3)。
//!
//! 字节 → 文本:严格 UTF-8 优先(774/807 图完美,含真 UTF-8 的 随机地图.map),
//! 失败回退 GBK 有损(28 张 GBK 图 + 5 张截断 GBK 图;绝不 strict、绝不失败)。
//! IniDocument 自身的段头/get_int/孤儿行适配在 M2 的 ini.rs 任务完成。

use super::gbk::gbk_lossy;

pub fn decode_map_text(data: &[u8]) -> String {
    match std::str::from_utf8(data) {
        Ok(s) => s.to_string(),
        Err(_) => gbk_lossy(data),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_takes_priority() {
        // 规格依据 §10:随机地图.map 的 Name 是真 UTF-8;若顺序反了会解出
        // GBK 乱码"闅忔満鍦板浘"
        let bytes = "随机地图".encode_utf8(&mut [0u8; 16]).as_bytes().to_vec();
        assert_eq!(decode_map_text(&bytes), "随机地图");
    }

    #[test]
    fn gbk_fallback_decodes_names() {
        assert_eq!(decode_map_text(&[0xBA, 0xA3, 0x87, 0x5B]), "海嘯");
    }

    #[test]
    fn never_fails_on_hostile_bytes() {
        // 截断 GBK 序列 + 任意二进制垃圾:不 panic、不失败,只出现 U+FFFD
        let out = decode_map_text(&[0x31, 0xCD, 0xE6, 0xBC, 0xD2, 0xD4, 0xAE, 0xBE]);
        assert_eq!(out, "1玩家援\u{FFFD}");
        let out = decode_map_text(&[0xFF, 0xFF, 0x81, 0xFE]);
        assert!(out.contains('\u{FFFD}'));
    }
}
```

注意 `utf8_takes_priority` 里 `encode_utf8` 的用法:`"随机地图".encode_utf8(&mut [0u8; 16])` 返回 `&mut str`,`.as_bytes()` 取其字节——等价于手动拼 `e9 9a 8f e6 9c ba e5 9c b0 e5 9b be`,但由编译器保证与字符串字面量一致。

- [ ] **Step 3: 运行单元测试通过**

Run: `cargo test -p ra2-core map::`
Expected: 7 passed(gbk 4 + mapdoc 3)

- [ ] **Step 4: 真图锁定(ignored 测试,追加到新文件 `crates/core/tests/map_samples.rs` 的第一个测试)**

创建 `crates/core/tests/map_samples.rs`(Task 9 会继续追加):

```rust
// SPDX-License-Identifier: GPL-3.0-only
//! 真实地图集成测试(M2):地图直接是 RA2_ASSETS_DIR 下的散文件(不经 MIX),
//! 中文/GBK 文件名按相对路径直接 join。运行:
//! RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test map_samples -- --ignored

use ra2_core::format::map::mapdoc::decode_map_text;
use ra2_core::format::ini::IniDocument;

fn assets_dir() -> std::path::PathBuf {
    std::env::var("RA2_ASSETS_DIR")
        .expect("需要环境变量 RA2_ASSETS_DIR")
        .into()
}

fn read_map(rel: &str) -> Vec<u8> {
    std::fs::read(assets_dir().join(rel))
        .unwrap_or_else(|e| panic!("读取地图 {rel}: {e}"))
}

/// GBK/UTF-8 解码策略的真图锁定(普查 §1 编码普查 + 规格依据 §10):
/// 随机地图.map 是真 UTF-8;城市海啸.map 的 Name 是 GBK;终极海战防守图.map
/// 的对象段 Name 带截断 GBK 序列([Basic] Name 为 ASCII 拼音,勘误见下)。
/// 三者都必须无损解析出 IniDocument 且 Name 正确。
#[test]
#[ignore]
fn decode_policy_over_real_maps() {
    // UTF-8 优先
    let doc = IniDocument::parse(&decode_map_text(&read_map("Maps/Custom/随机地图.map")));
    assert_eq!(doc.value("Basic", "Name"), Some("随机地图"));
    // GBK 回退
    let doc = IniDocument::parse(&decode_map_text(&read_map("Maps/MadHQ/城市海啸.map")));
    assert_eq!(doc.value("Basic", "Name"), Some("海嘯"));
    // 截断 GBK:整文件有损解码后仍成档
    let doc = IniDocument::parse(&decode_map_text(&read_map("Maps/DDLY/终极海战防守图.map")));
    assert_eq!(doc.value("Basic", "Name"), Some("jiangweidajihaizhanfangshoutu"));
    // 截断序列在该图的对象段(如 [01000014])Name,不在 [Basic]
    let name = doc.value("01000014", "Name").expect("对象段应有 Name");
    assert!(name.contains('\u{FFFD}'), "截断序列应产生替换符: {name}");
    assert!(name.contains("玩家援"));
}
```

> **勘误(2026-11-04,Task 3 实现时实证)**:原文称该图 [Basic] 有"两个 `Name=` 键(首个 ASCII 拼音,第二个 GBK 截断)——last-wins 真实样本"。实测(False):[Basic] 仅一个 `Name=jiangweidajihaizhanfangshoutu`(ASCII 拼音);截断 GBK `31 cd e6 bc d2 d4 ae be` 的 Name 在**对象段**(如 `[01000014]`),全文件任一段内 0 重复键(与普查 §8"全语料 0 重复键"一致)。last-wins 行为仅由 Task 2 合成测试锁定,无真实语料样本。测试已按真实事实改写(见 map_samples.rs 提交 8017297)。

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test map_samples -- --ignored`
Expected: 1 passed(ignored 22/27 进度)

- [ ] **Step 5: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 合成 118 passed;ignored 22 passed

```bash
git add crates/core/src/format/map/ crates/core/src/format/mod.rs crates/core/tests/map_samples.rs tools/gen_gbk_table.py
git commit -m "feat(core): 地图模块族脚手架 + GBK 有损解码与 UTF-8 优先解码策略" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 4: 包段容器解码(pack.rs)+ LCW 消耗量包装

**Files:**
- Create: `crates/core/src/format/map/pack.rs`
- Modify: `crates/core/src/format/map/mod.rs`(追加 `pub mod pack;`)
- Modify: `crates/core/src/format/lcw.rs`(核心循环改名为 `lcw_decode_consumed` 返回消耗量;原 `lcw_decode` 变包装;追加 1 个测试)

**Interfaces:**
- Consumes: `lzo1x_decode`(Task 1)、`lcw_decode`(M1b)、`FormatError`
- Produces(供 Task 7/9 使用):
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum PackCodec { Lzo1x, Lcw }`
  - `#[derive(Debug, Clone, Copy)] pub enum PackBound { Exact(usize), Tiles { max_out: usize }, Preview { max_out: usize } }`
  - `pub struct PackSection { pub data: Vec<u8>, pub chunks: usize }`
  - `pub fn decode_pack_section(entries: &[(String, String)], codec: PackCodec, bound: PackBound) -> Result<PackSection, FormatError>`
  - `pub fn base64_decode(text: &str) -> Result<Vec<u8>, FormatError>`(标准 RFC 4648,跳过 `=`)
  - `pub fn base64_decode_lenient(text: &str) -> Vec<u8>`(预览段:首个非法字符截断 + 丢弃尾部不完整 4 字符组)
  - `pub fn lcw_decode_consumed(src: &[u8], out: &mut Vec<u8>) -> Result<usize, FormatError>`(lcw.rs;返回源内消耗字节;`lcw_decode` 语义不变 = 累计产出长度)
- 语义要点(研究 §1-2、§8,勿改):编号 key 按**数值序**拼接;块头 4 字节 `{u16 size_in, u16 size_out}`;size_in==0/越界 → 拒;size_out==0/>8192 → 拒(**仅 PreviewPack 例外**:停在首个坏块保留前缀——语料实测 7 张 DDLY 图的"坏块"全部是位图完整后的异常尾块);聚合 Σsize_out:Exact 必须 == 262144,Tiles ≤ max 且 %11 ∈ {0,4},Preview 不以 Σ 预拒(异常尾块会抬高 Σ,如城郊之战 57016 > 48600)但逐块累加时超过 max_out 即停;所有分配上界在解码前确定;严格段 consumed/produced 与块头不符 → Invalid。

- [ ] **Step 1: 写 `lcw.rs` 失败测试(追加到其测试模块)**

```rust
    #[test]
    fn consumed_tracks_source_bytes() {
        // 0x83 = 字面量运行 count 3;消耗量含 0x80 终止符
        let mut out = Vec::new();
        let consumed = lcw_decode_consumed(&[0x83, b'A', b'B', b'C', 0x80], &mut out).unwrap();
        assert_eq!(consumed, 5);
        assert_eq!(out, b"ABC");
        // 包装函数语义不变:返回累计产出长度(M1b 契约)
        let mut out2 = Vec::new();
        let produced = lcw_decode(&[0x83, b'A', b'B', b'C', 0x80], &mut out2).unwrap();
        assert_eq!(produced, 3);
        assert_eq!(out2, b"ABC");
    }
```

- [ ] **Step 2: 实现 `lcw_decode_consumed`(lcw.rs)**

把现有 `pub fn lcw_decode(src: &[u8], out: &mut Vec<u8>) -> Result<usize, FormatError> {` 整体改名为 `pub fn lcw_decode_consumed(...)`,循环内唯一的成功出口(0x80 分支)`return Ok(out.len());` 改为 `return Ok(sp);`;然后在原位置之前加回包装:

```rust
/// LCW(Format 80)解压;返回**累计产出长度**(M1b 契约,SHP/TMP 调用方)。
pub fn lcw_decode(src: &[u8], out: &mut Vec<u8>) -> Result<usize, FormatError> {
    lcw_decode_consumed(src, out).map(|_| out.len())
}
```

同时把原函数的文档注释移到 `lcw_decode_consumed` 上并补一句"返回源内消耗的字节数(包段循环核对 consumed == size_in 用);产出用 out 的增量核对"。

- [ ] **Step 3: 运行 lcw 测试**

Run: `cargo test -p ra2-core lcw`
Expected: 8 passed(既有 7 + 新 1;shp/tmp 测试经包装函数零影响)

- [ ] **Step 4: 写 `pack.rs` 失败测试 + 实现(同文件)**

`crates/core/src/format/map/mod.rs` 追加 `pub mod pack;`。

`crates/core/src/format/map/pack.rs` 完整文件:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! 地图包段容器解码(四段共用:`[IsoMapPack5]` / `[OverlayPack]` /
//! `[OverlayDataPack]` / `[PreviewPack]`)。
//!
//! 布局(研究 2026-11-04-map-pack-and-binary-sections.md §1-2、§8;XCC
//! `shp_decode.cpp:1127-1155` decode5 语义 + 540 图语料验证):
//! 1. 编号 key(`1=`、`2=`…)按**数值序**拼接 → 标准 RFC 4648 base64
//!    (`=` 补位跳过);
//! 2. 解码字节 = 背靠背块 `{u16 size_in; u16 size_out; payload[size_in]}`;
//! 3. codec 按段名由调用方分派:IsoMapPack5/PreviewPack = LZO1X,overlay
//!    双段 = LCW("IsoMapPack5" 的 5 即 codec id);
//! 4. 预检(研究 §8,内存尖峰加固——所有大分配之前完成):size_in==0/越界
//!    拒;size_out==0/>8192 拒(仅 PreviewPack 例外:停在首个坏块保留干净
//!    前缀);聚合 Σsize_out 按段核对(overlay 恒 262144;瓦片流 ≤ 上界且
//!    %11 ∈ {0,4};预览不以 Σ 预拒但逐块累加超过 max_out 即停)。

use super::lzo1x::lzo1x_decode;
use crate::format::lcw::lcw_decode_consumed;
use crate::format::FormatError;

/// 包段 codec。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackCodec {
    /// `[IsoMapPack5]` / `[PreviewPack]`(miniLZO)。
    Lzo1x,
    /// `[OverlayPack]` / `[OverlayDataPack]`(Format 80)。
    Lcw,
}

/// 解码前的输出上界形态(研究 §8-3)。
#[derive(Debug, Clone, Copy)]
pub enum PackBound {
    /// 聚合输出必须恰好等于该值(overlay 双段恒 262144)。
    Exact(usize),
    /// 瓦片流:Σ ≤ max_out 且 Σ % 11 ∈ {0, 4}(4 = EA/FinalAlert 的零尾)。
    Tiles { max_out: usize },
    /// 预览:容忍截断/异常尾块(lenient),累加超过 max_out 即停。
    Preview { max_out: usize },
}

/// 解码后的包段(严格段 data 长度 == Σ size_out)。
pub struct PackSection {
    pub data: Vec<u8>,
    pub chunks: usize,
}

fn b64_val(b: u8) -> Option<u8> {
    match b {
        b'A'..=b'Z' => Some(b - b'A'),
        b'a'..=b'z' => Some(b - b'a' + 26),
        b'0'..=b'9' => Some(b - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// 标准 RFC 4648 base64 解码(跳过 `=`;其余字符必须在标准字母表内)。
pub fn base64_decode(text: &str) -> Result<Vec<u8>, FormatError> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3 + 3);
    let mut acc = 0u32;
    let mut nbits = 0u32;
    for &b in text.as_bytes() {
        if b == b'=' {
            continue;
        }
        let v = b64_val(b)
            .ok_or_else(|| FormatError::Invalid(format!("base64 字符 0x{b:02X} 不在标准字母表内")))?;
        acc = (acc << 6) | v as u32;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((acc >> nbits) as u8);
        }
    }
    Ok(out)
}

/// 预览段专用:在首个非法字符处截断(保留前缀),并丢弃尾部不完整的
/// 4 字符组(普查 §4.5:7 张 DDLY 图尾组被截断——"drop the partial tail group")。
pub fn base64_decode_lenient(text: &str) -> Vec<u8> {
    let mut end = text.len();
    for (i, &b) in text.as_bytes().iter().enumerate() {
        if b != b'=' && b64_val(b).is_none() {
            end = i;
            break;
        }
    }
    let kept: Vec<u8> = text.as_bytes()[..end]
        .iter()
        .copied()
        .filter(|&b| b != b'=')
        .collect();
    let keep = kept.len() - kept.len() % 4;
    let mut out = Vec::with_capacity(keep / 4 * 3 + 3);
    let mut acc = 0u32;
    let mut nbits = 0u32;
    for &b in &kept[..keep] {
        let v = b64_val(b).expect("已预过滤") as u32;
        acc = (acc << 6) | v;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((acc >> nbits) as u8);
        }
    }
    out
}

/// 解码一个包段。`entries` 为该段的全部键值对(顺序任意,按 key 数值排序)。
pub fn decode_pack_section(
    entries: &[(String, String)],
    codec: PackCodec,
    bound: PackBound,
) -> Result<PackSection, FormatError> {
    // 1) 编号 key 数值序拼接(普查 §4.5:键域 1..=9568;排序兼容敌意乱序)
    let mut keyed: Vec<(usize, &str)> = Vec::with_capacity(entries.len());
    for (k, v) in entries {
        let n: usize = k
            .parse()
            .map_err(|_| FormatError::Invalid(format!("包段键不是十进制数: {k}")))?;
        keyed.push((n, v.as_str()));
    }
    keyed.sort_by_key(|(n, _)| *n);
    let mut text = String::with_capacity(keyed.iter().map(|(_, v)| v.len()).sum());
    for (_, v) in &keyed {
        text.push_str(v);
    }
    let lenient = matches!(bound, PackBound::Preview { .. });
    let blob = if lenient {
        base64_decode_lenient(&text)
    } else {
        base64_decode(&text)?
    };

    // 2) 块链预检(只读块头,不解码):Σ size_out 与逐块合法性,任何分配之前
    let mut sum_out: u64 = 0;
    let mut sp = 0usize;
    while sp < blob.len() {
        if blob.len() - sp < 4 {
            return Err(FormatError::UnexpectedEof {
                needed: 4,
                remaining: blob.len() - sp,
            });
        }
        let size_in = u16::from_le_bytes([blob[sp], blob[sp + 1]]) as usize;
        let size_out = u16::from_le_bytes([blob[sp + 2], blob[sp + 3]]) as usize;
        if size_in == 0 {
            return Err(FormatError::Invalid(format!("块偏移 {sp} 的 size_in == 0")));
        }
        if size_in > blob.len() - sp - 4 {
            return Err(FormatError::UnexpectedEof {
                needed: size_in,
                remaining: blob.len() - sp - 4,
            });
        }
        if size_out == 0 || size_out > 8192 {
            if lenient {
                break; // 预览:异常末块(城郊之战 (8, 56994))——保留干净前缀
            }
            return Err(FormatError::Invalid(format!(
                "块偏移 {sp} 的 size_out = {size_out},合法 1..=8192"
            )));
        }
        sum_out += size_out as u64;
        sp += 4 + size_in;
    }
    let alloc = match (bound, sum_out) {
        (PackBound::Exact(n), s) => {
            if s != n as u64 {
                return Err(FormatError::Invalid(format!(
                    "聚合输出 {s} != 期望 {n}"
                )));
            }
            n
        }
        (PackBound::Tiles { max_out }, s) => {
            if s > max_out as u64 {
                return Err(FormatError::Invalid(format!(
                    "聚合输出 {s} 超过瓦片流上界 {max_out}"
                )));
            }
            if s % 11 != 0 && s % 11 != 4 {
                return Err(FormatError::Invalid(format!(
                    "聚合输出 {s} 对 11 取模 = {},合法余数 0/4",
                    s % 11
                )));
            }
            s as usize
        }
        (PackBound::Preview { max_out }, s) => max_out.min(s as usize),
    };
    let mut out = Vec::with_capacity(alloc);

    // 3) 逐块解码(与预检同样的停止规则)
    let mut sp = 0usize;
    let mut chunks = 0usize;
    while sp < blob.len() {
        if blob.len() - sp < 4 {
            break; // lenient 尾部残头;严格段已被预检拒绝
        }
        let size_in = u16::from_le_bytes([blob[sp], blob[sp + 1]]) as usize;
        let size_out = u16::from_le_bytes([blob[sp + 2], blob[sp + 3]]) as usize;
        if size_in == 0 || size_in > blob.len() - sp - 4 {
            break; // 同上,防御性
        }
        if size_out == 0 || size_out > 8192 {
            if lenient {
                break;
            }
            return Err(FormatError::Invalid(format!(
                "块偏移 {sp} 的 size_out = {size_out},合法 1..=8192"
            )));
        }
        if lenient {
            if let PackBound::Preview { max_out } = bound {
                if out.len() + size_out > max_out {
                    break; // 声明 Σ 超上界的敌意预览:累加越界即停
                }
            }
        }
        let payload = &blob[sp + 4..sp + 4 + size_in];
        let before = out.len();
        let check = match codec {
            PackCodec::Lzo1x => lzo1x_decode(payload, size_out, &mut out).and_then(|produced| {
                if produced != size_out {
                    Err(FormatError::Invalid(format!(
                        "LZO 产出 {produced} != 块头 size_out {size_out}"
                    )))
                } else {
                    Ok(())
                }
            }),
            PackCodec::Lcw => {
                let consumed = lcw_decode_consumed(payload, &mut out)?;
                if consumed != size_in {
                    Err(FormatError::Invalid(format!(
                        "LCW 消耗 {consumed} != 块头 size_in {size_in}"
                    )))
                } else if out.len() - before != size_out {
                    Err(FormatError::Invalid(format!(
                        "LCW 产出 {} != 块头 size_out {size_out}",
                        out.len() - before
                    )))
                } else {
                    Ok(())
                }
            }
        };
        match check {
            Ok(()) => {
                chunks += 1;
                sp += 4 + size_in;
            }
            Err(e) => {
                if lenient {
                    out.truncate(before);
                    break; // 保留干净前缀(研究 §5 契约)
                }
                return Err(e);
            }
        }
    }
    Ok(PackSection { data: out, chunks })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用最小 LZO 块载荷:全字面量(首 token 17+n,1 ≤ n ≤ 238)+ EOF 标记。
    fn lzo_literal_payload(data: &[u8]) -> Vec<u8> {
        assert!((1..=238).contains(&data.len()));
        let mut p = vec![(17 + data.len()) as u8];
        p.extend_from_slice(data);
        p.extend_from_slice(&[0x11, 0x00, 0x00]);
        p
    }

    fn push_lzo_chunk(blob: &mut Vec<u8>, data: &[u8]) {
        let payload = lzo_literal_payload(data);
        blob.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        blob.extend_from_slice(&(data.len() as u16).to_le_bytes());
        blob.extend_from_slice(&payload);
    }

    /// 测试用 LZO 块流:数据按 ≤238 字节切全字面量块。
    fn lzo_literal_stream(data: &[u8]) -> Vec<u8> {
        let mut blob = Vec::new();
        let (full, rest) = data.as_chunks::<238>();
        for chunk in full {
            push_lzo_chunk(&mut blob, chunk);
        }
        if !rest.is_empty() {
            push_lzo_chunk(&mut blob, rest);
        }
        blob
    }

    /// 测试用 LCW 块流:0xFE 长填充(n ≤ 8192/块)。
    fn lcw_fill_stream(fill: u8, total: usize) -> Vec<u8> {
        let mut blob = Vec::new();
        let mut left = total;
        while left > 0 {
            let n = left.min(8192);
            blob.extend_from_slice(&4u16.to_le_bytes());
            blob.extend_from_slice(&(n as u16).to_le_bytes());
            blob.extend_from_slice(&[0xFE, n as u8, (n >> 8) as u8, fill]);
            left -= n;
        }
        blob
    }

    fn b64(bytes: &[u8]) -> String {
        const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for c in bytes.as_chunks::<3>().0 {
            let n = ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32;
            s.push(A[(n >> 18) as usize & 63] as char);
            s.push(A[(n >> 12) as usize & 63] as char);
            s.push(A[(n >> 6) as usize & 63] as char);
            s.push(A[n as usize & 63] as char);
        }
        match bytes.len() % 3 {
            1 => {
                let n = (bytes[bytes.len() - 1] as u32) << 16;
                s.push(A[(n >> 18) as usize & 63] as char);
                s.push(A[(n >> 12) as usize & 63] as char);
                s.push('=');
                s.push('=');
            }
            2 => {
                let n = ((bytes[bytes.len() - 2] as u32) << 16) | ((bytes[bytes.len() - 1] as u32) << 8);
                s.push(A[(n >> 18) as usize & 63] as char);
                s.push(A[(n >> 12) as usize & 63] as char);
                s.push(A[(n >> 6) as usize & 63] as char);
                s.push('=');
            }
            _ => {}
        }
        s
    }

    fn entries_of(blob: &[u8]) -> Vec<(String, String)> {
        // 按测试需要切成多行 base64(每行 ≤ 70 字符,模拟真实图的换行键)
        let text = b64(blob);
        let mut v = Vec::new();
        let mut i = 0usize;
        let mut k = 1usize;
        while i < text.len() {
            let n = (text.len() - i).min(70);
            v.push((k.to_string(), text[i..i + n].to_string()));
            i += n;
            k += 1;
        }
        v
    }

    #[test]
    fn base64_decodes_standard_alphabet_and_skips_padding() {
        assert_eq!(base64_decode("QUI=").unwrap(), b"AB");
        assert_eq!(base64_decode("QUJD").unwrap(), b"ABC");
        assert_eq!(base64_decode("QUJDQUI=").unwrap(), b"ABCAB");
    }

    #[test]
    fn base64_strict_rejects_bad_char() {
        assert!(matches!(
            base64_decode("QU?D"),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn base64_lenient_cuts_at_bad_char_and_partial_group() {
        // 'p' 前 6 个合法字符 → 保留 4("QUJD")→ "ABC";尾部残组被丢弃
        assert_eq!(base64_decode_lenient("QUJDQU?pQ"), b"ABC");
    }

    #[test]
    fn numbered_keys_concat_in_numeric_order() {
        // 键乱序传入:1→[0x11], 2→[0x22], 10→[0x33];数值序拼接后输出
        // [0x11,0x22,0x33](字典序会得到 1,10,2 → [0x11,0x33,0x22])。
        // 3 字节 %11 = 3 不合法 → 用 Preview 上界绕开模数判定。
        let mk = |d: u8| {
            let payload = lzo_literal_payload(&[d]);
            let mut blob = Vec::new();
            blob.extend_from_slice(&(payload.len() as u16).to_le_bytes());
            blob.extend_from_slice(&1u16.to_le_bytes());
            blob.extend_from_slice(&payload);
            b64(&blob)
        };
        let entries = vec![
            ("10".to_string(), mk(0x33)),
            ("2".to_string(), mk(0x22)),
            ("1".to_string(), mk(0x11)),
        ];
        let sec = decode_pack_section(&entries, PackCodec::Lzo1x, PackBound::Preview { max_out: 3 })
            .unwrap();
        assert_eq!(sec.data, vec![0x11, 0x22, 0x33]);
        assert_eq!(sec.chunks, 3);
    }

    #[test]
    fn rejects_non_numeric_key() {
        let entries = vec![("x".to_string(), "QUJD".to_string())];
        assert!(matches!(
            decode_pack_section(&entries, PackCodec::Lzo1x, PackBound::Preview { max_out: 3 }),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_size_in_zero() {
        let blob = [0u8, 0, 8, 0]; // size_in=0
        let entries = entries_of(&blob);
        assert!(matches!(
            decode_pack_section(&entries, PackCodec::Lzo1x, PackBound::Preview { max_out: 8 }),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_size_in_beyond_stream() {
        let blob = [100, 0, 8, 0, 1, 2]; // size_in=100 但只剩 2 字节载荷
        let entries = entries_of(&blob);
        assert!(matches!(
            decode_pack_section(&entries, PackCodec::Lzo1x, PackBound::Preview { max_out: 8 }),
            Err(FormatError::UnexpectedEof { .. })
        ));
    }

    #[test]
    fn strict_rejects_size_out_zero() {
        // size_out=0 在 Σ 累计之前即被拒(bound 参数不参与该判定);载荷补齐
        // size_in 声明的 9 字节,确保先越过 size_in 越界核对
        let blob = [9, 0, 0, 0, 0x83, b'A', b'B', b'C', 0x80, 0x00, 0x00, 0x00, 0x00];
        let entries = entries_of(&blob);
        assert!(matches!(
            decode_pack_section(&entries, PackCodec::Lcw, PackBound::Exact(0)),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn strict_rejects_size_out_over_8192() {
        // u16 LE 的 8193 = 0x2001 = [0x01, 0x20]
        let blob = [9, 0, 0x01, 0x20, 0x83, b'A', b'B', b'C', 0x80, 0x00, 0x00, 0x00, 0x00];
        let entries = entries_of(&blob);
        assert!(matches!(
            decode_pack_section(&entries, PackCodec::Lcw, PackBound::Exact(8193)),
            Err(FormatError::Invalid(_))
        ));
    }
```

继续追加其余测试:

```rust
    #[test]
    fn exact_bound_rejects_sum_mismatch() {
        // 单个 8191 填充块 ≠ 262144 → Invalid(overlay 的 Σ 恒等判定)
        let blob = lcw_fill_stream(0xFF, 8191);
        let entries = entries_of(&blob);
        assert!(matches!(
            decode_pack_section(&entries, PackCodec::Lcw, PackBound::Exact(262144)),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn tiles_bound_rejects_bad_modulus() {
        // Σ = 8192,%11 = 2 ∉ {0,4} → Invalid
        let blob = lzo_literal_stream(&[0x5Au8; 8192]);
        let entries = entries_of(&blob);
        assert!(matches!(
            decode_pack_section(
                &entries,
                PackCodec::Lzo1x,
                PackBound::Tiles { max_out: 8192 }
            ),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn tiles_bound_rejects_over_max() {
        // Σ = 26(%11 = 4 合法)但超过 max_out = 15 → Invalid
        let mut data = Vec::new();
        data.extend_from_slice(&[0x57, 0x00, 0x01, 0x00, 0x3C, 0x01, 0x00, 0x00, 0x01, 0x02, 0x00]);
        data.extend_from_slice(&[0x56, 0x00, 0x02, 0x00, 0x3C, 0x01, 0x00, 0x00, 0x02, 0x02, 0x00]);
        data.extend_from_slice(&[0, 0, 0, 0]);
        let blob = lzo_literal_stream(&data);
        let entries = entries_of(&blob);
        assert!(matches!(
            decode_pack_section(
                &entries,
                PackCodec::Lzo1x,
                PackBound::Tiles { max_out: 15 }
            ),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn lcw_research_chunk0_decodes_to_8192_ff() {
        // 规格依据 §4:c1a01md OverlayPack 块 0 原始字节(XCC decode5 语义);
        // 9 载荷字节 → 8192 个 FF
        let blob: Vec<u8> = [
            &[0x09u8, 0x00, 0x00, 0x20] as &[u8],
            &[0x81, 0xFF, 0xFE, 0xFE, 0x1F, 0xFF, 0x81, 0xFF, 0x80],
        ]
        .concat();
        let entries = entries_of(&blob);
        let sec = decode_pack_section(&entries, PackCodec::Lcw, PackBound::Exact(8192)).unwrap();
        assert_eq!(sec.chunks, 1);
        assert_eq!(sec.data.len(), 8192);
        assert!(sec.data.iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn tiles_accepts_short_stream_with_zero_tail() {
        // 2 条记录(规格依据 §5 的 rec0/rec1)+ 4 零尾:Σ = 26,%11 = 4,
        // ≤ max_out=26 → Ok(4 张 TS 代短图的上界语义)
        let mut data = Vec::new();
        data.extend_from_slice(&[0x57, 0x00, 0x01, 0x00, 0x3C, 0x01, 0x00, 0x00, 0x01, 0x02, 0x00]);
        data.extend_from_slice(&[0x56, 0x00, 0x02, 0x00, 0x3C, 0x01, 0x00, 0x00, 0x02, 0x02, 0x00]);
        data.extend_from_slice(&[0, 0, 0, 0]);
        let blob = lzo_literal_stream(&data);
        let entries = entries_of(&blob);
        let sec = decode_pack_section(
            &entries,
            PackCodec::Lzo1x,
            PackBound::Tiles { max_out: 26 },
        )
        .unwrap();
        assert_eq!(sec.data.len(), 26);
        assert_eq!(sec.chunks, 1);
        assert_eq!(&sec.data[..11], &data[..11]);
    }

    #[test]
    fn preview_lenient_stops_at_anomalous_final_chunk() {
        // 城郊之战形态:干净块(12+10 字节)+ 异常末块 (8, 56994) → 保留前缀 22
        let mut blob = Vec::new();
        push_lzo_chunk(&mut blob, &[b'a'; 12]);
        push_lzo_chunk(&mut blob, &[b'b'; 10]);
        blob.extend_from_slice(&8u16.to_le_bytes());
        blob.extend_from_slice(&56994u16.to_le_bytes());
        blob.extend_from_slice(&[0x11, 0x00, 0x00, 0x11, 0x00, 0x00, 0x11, 0x00]);
        let entries = entries_of(&blob);
        let sec = decode_pack_section(
            &entries,
            PackCodec::Lzo1x,
            PackBound::Preview { max_out: 22 },
        )
        .unwrap();
        assert_eq!(sec.data.len(), 22);
        assert_eq!(sec.chunks, 2);
        assert!(sec.data.starts_with(b"aaaaaaaaaaaa"));
    }

    #[test]
    fn preview_lenient_stops_when_accumulation_exceeds_max() {
        // 敌意预览:3 块各声明 8192(Σ 24576 > max 20000)→ 累加到 16384 后停
        // (LCW 流配 LCW codec;lenient 分支不核对 Σ,只钳累加)
        let blob = lcw_fill_stream(0x11, 24576);
        let entries = entries_of(&blob);
        let sec = decode_pack_section(
            &entries,
            PackCodec::Lcw,
            PackBound::Preview { max_out: 20000 },
        )
        .unwrap();
        assert_eq!(sec.data.len(), 16384);
    }
```

最后一个测试:

```rust
    #[test]
    fn strict_rejects_produced_mismatch() {
        // 块头声明 size_out=5,载荷只产 3 → Invalid(研究 §8-5 的 produced 核对;
        // 用严格 Exact(5) 绑定——lenient 路径会吞掉产出不符而保留前缀)
        let payload = [0x14u8, b'A', b'B', b'C', 0x11, 0x00, 0x00]; // t = 0x14-17 = 3
        let mut blob = Vec::new();
        blob.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        blob.extend_from_slice(&5u16.to_le_bytes());
        blob.extend_from_slice(&payload);
        let entries = entries_of(&blob);
        assert!(matches!(
            decode_pack_section(&entries, PackCodec::Lzo1x, PackBound::Exact(5)),
            Err(FormatError::Invalid(_))
        ));
    }
}
```

- [ ] **Step 5: 运行单元测试通过**

Run: `cargo test -p ra2-core pack && cargo test -p ra2-core lcw`
Expected: pack 17 passed;lcw 8 passed

- [ ] **Step 6: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 合成 136 passed

```bash
git add crates/core/src/format/map/pack.rs crates/core/src/format/map/mod.rs crates/core/src/format/lcw.rs
git commit -m "feat(core): 包段容器解码(编号键/标准base64/4字节块头/按段codec/内存预检)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 5: IsoMapPack5 瓦片网格(iso.rs)

**Files:**
- Create: `crates/core/src/format/map/iso.rs`
- Modify: `crates/core/src/format/map/mod.rs`(追加 `pub mod iso;`)

**Interfaces:**
- Consumes: `FormatError`;包段解压字节(Task 4 产出,由 Task 9 接线)
- Produces(供 Task 9/10/11 使用):
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct IsoTile { pub x: u16, pub y: u16, pub tile: i16, pub sub_tile: u8, pub z: u8 }`
  - `pub struct IsoGrid` + `IsoGrid::parse(data: &[u8], w: u16, h: u16) -> Result<Self, FormatError>`
  - 只读访问:`len() / is_empty() / walk_capacity() / records() -> &[IsoTile] / cell(x: u16, y: u16) -> Option<&IsoTile> / max_tile_index() -> Option<i16>`
- 语义要点(研究 §3,勿改):11 字节记录 `{u16 x; u16 y; i16 tile(−1=无瓦片); u8 zero×2; u8 sub_tile; u8 z; u8 zero}`;**存储的 (x,y) 是权威键**(§3.6:4 张 TS 代官方图缺格,缺失行走格 = 无瓦片);流长 %11 ∈ {0,4}(4 = 全零尾);记录数 ≤ (2W−1)·H(u128 算上界);恒零垫字节非零 → Invalid;(x,y) ∈ 1..=W+H−1 且唯一(34.4M 语料记录零重复);网格坐标即 [Terrain]/[Structures]/[Waypoints] 的同一菱形坐标系(恒等,无偏移,研究 §3.4)。

- [ ] **Step 1: 写 `iso.rs` 失败测试 + 实现(同文件)**

`crates/core/src/format/map/mod.rs` 追加 `pub mod iso;`。

`crates/core/src/format/map/iso.rs` 完整文件:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! `[IsoMapPack5]` 解压后的等距瓦片记录流(11 字节记录)。
//!
//! 布局(研究 2026-11-04-map-pack-and-binary-sections.md §3;XCC
//! `cc_structures.h:207-216` `t_iso_map_pack_entry` + 34.4M 语料记录验证):
//! `{u16 x; u16 y; i16 tile(−1=无瓦片); u8 zero1; u8 zero2; u8 sub_tile;
//! u8 z; u8 zero3}`。记录数 ≤ (2W−1)·H([Map]Size = 0,0,W,H);
//! 流长 %11 ∈ {0,4}(4 = EA/FinalAlert 的全零尾)。
//! **存储的 (x,y) 是权威键**:4 张 TS 代官方图省略 void 格(riverram/
//! frstbite/hillbtwn/isleofoades,§3.6),缺失的行走格按无瓦片处理;
//! 行走顺序仅作数量上界,不作定位依据。该 (x,y) 坐标与 [Terrain]/
//! [Structures]/[Waypoints] 的菱形坐标恒等(研究 §3.4 的 749/749 等实证)。

use super::FormatError;

/// 一条瓦片记录(三个恒零垫字节已校验,不存储)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsoTile {
    pub x: u16,
    pub y: u16,
    pub tile: i16,
    pub sub_tile: u8,
    pub z: u8,
}

/// 稀疏瓦片网格:按存储 (x,y) 排序索引(排序 Vec + 二分,无 HashMap)。
pub struct IsoGrid {
    records: Vec<IsoTile>,
    walk_capacity: usize,
}

fn cell_key(x: u16, y: u16) -> u64 {
    ((y as u64) << 16) | x as u64
}

impl IsoGrid {
    /// 从解压后的记录流解析。`w`/`h` 来自 [Map]Size(调用方已保证 ≥ 1;
    /// 此处再防御性钳制,敌意值不 panic)。
    pub fn parse(data: &[u8], w: u16, h: u16) -> Result<Self, FormatError> {
        match data.len() % 11 {
            0 => {}
            4 => {
                if data[data.len() - 4..] != [0, 0, 0, 0] {
                    return Err(FormatError::Invalid("瓦片流 4 字节尾不全为零".to_string()));
                }
            }
            _ => {
                return Err(FormatError::Invalid(format!(
                    "瓦片流长度 {} 对 11 取模非法(合法余数 0/4)",
                    data.len()
                )));
            }
        }
        let w = w.max(1);
        let h = h.max(1);
        // 行走容量 (2W−1)·H:u128 计算防敌意尺寸溢出(照 HVA u128 教训)
        let capacity = (2u128 * w as u128 - 1) * h as u128;
        let n = data.len() / 11;
        if n as u128 > capacity {
            return Err(FormatError::Invalid(format!(
                "瓦片记录 {n} 超过行走容量 {capacity} = (2·{w}−1)·{h}"
            )));
        }
        let max_xy = w as u32 + h as u32 - 1;
        let mut records = Vec::with_capacity(n);
        for i in 0..n {
            let b = &data[i * 11..i * 11 + 11];
            if b[6] != 0 || b[7] != 0 || b[10] != 0 {
                return Err(FormatError::Invalid(format!("记录 {i} 的恒零垫字节非零")));
            }
            let x = u16::from_le_bytes([b[0], b[1]]);
            let y = u16::from_le_bytes([b[2], b[3]]);
            if x == 0 || y == 0 || x as u32 > max_xy || y as u32 > max_xy {
                return Err(FormatError::Invalid(format!(
                    "记录 {i} 的坐标 ({x},{y}) 越出菱形界 1..={max_xy}"
                )));
            }
            records.push(IsoTile {
                x,
                y,
                tile: i16::from_le_bytes([b[4], b[5]]),
                sub_tile: b[8],
                z: b[9],
            });
        }
        // (x,y) 唯一(语料 34.4M 记录零重复;重复即畸形)+ 排序建二分索引
        records.sort_by_key(|t| cell_key(t.x, t.y));
        for pair in records.windows(2) {
            if cell_key(pair[0].x, pair[0].y) == cell_key(pair[1].x, pair[1].y) {
                return Err(FormatError::Invalid(format!(
                    "坐标 ({},{}) 重复",
                    pair[0].x, pair[0].y
                )));
            }
        }
        Ok(IsoGrid {
            records,
            walk_capacity: capacity as usize,
        })
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// 行走容量上界 (2W−1)·H(统计/交叉验证用)。
    pub fn walk_capacity(&self) -> usize {
        self.walk_capacity
    }

    pub fn records(&self) -> &[IsoTile] {
        &self.records
    }

    /// 按存储 (x,y) 查格(二分);None = 该格无记录(缺格/无瓦片)。
    pub fn cell(&self, x: u16, y: u16) -> Option<&IsoTile> {
        let key = cell_key(x, y);
        self.records
            .binary_search_by_key(&key, |t| cell_key(t.x, t.y))
            .ok()
            .map(|i| &self.records[i])
    }

    /// 最大非 −1 tile 索引(剧场交叉验证用;全空/全 −1 返回 None)。
    pub fn max_tile_index(&self) -> Option<i16> {
        self.records.iter().map(|t| t.tile).filter(|&t| t >= 0).max()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(x: u16, y: u16, tile: i16, sub: u8, z: u8) -> [u8; 11] {
        let mut b = [0u8; 11];
        b[0..2].copy_from_slice(&x.to_le_bytes());
        b[2..4].copy_from_slice(&y.to_le_bytes());
        b[4..6].copy_from_slice(&tile.to_le_bytes());
        b[8] = sub;
        b[9] = z;
        b
    }

    #[test]
    fn parses_real_c1a01md_records() {
        // 规格依据 §5:c1a01md 块 0 的 rec0/rec1 原始字节 + 4 零尾
        let mut data = Vec::new();
        data.extend_from_slice(&[0x57, 0x00, 0x01, 0x00, 0x3C, 0x01, 0x00, 0x00, 0x01, 0x02, 0x00]);
        data.extend_from_slice(&[0x56, 0x00, 0x02, 0x00, 0x3C, 0x01, 0x00, 0x00, 0x02, 0x02, 0x00]);
        data.extend_from_slice(&[0, 0, 0, 0]);
        let grid = IsoGrid::parse(&data, 87, 71).unwrap();
        assert_eq!(grid.len(), 2);
        assert_eq!(grid.walk_capacity(), (2 * 87 - 1) * 71);
        let r0 = grid.cell(87, 1).unwrap();
        assert_eq!((r0.tile, r0.sub_tile, r0.z), (316, 1, 2));
        let r1 = grid.cell(86, 2).unwrap();
        assert_eq!((r1.tile, r1.sub_tile, r1.z), (316, 2, 2));
        assert!(grid.cell(1, 1).is_none());
        assert_eq!(grid.max_tile_index(), Some(316));
    }

    #[test]
    fn no_tail_stream_is_accepted() {
        // 社区产图器在 11·records 处精确截止(研究 §3.3)
        let data = rec(3, 4, 10, 0, 2);
        let grid = IsoGrid::parse(&data, 10, 10).unwrap();
        assert_eq!(grid.len(), 1);
        assert_eq!(grid.cell(3, 4).unwrap().tile, 10);
    }

    #[test]
    fn rejects_nonzero_pad_bytes() {
        let mut data = rec(3, 4, 10, 0, 2);
        data[6] = 1;
        assert!(matches!(IsoGrid::parse(&data, 10, 10), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_nonzero_four_byte_tail() {
        let mut data = Vec::new();
        data.extend_from_slice(&rec(3, 4, 10, 0, 2));
        data.extend_from_slice(&[0, 0, 0, 1]);
        assert!(matches!(IsoGrid::parse(&data, 10, 10), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_bad_stream_length() {
        let data = [0u8; 24]; // %11 = 2
        assert!(matches!(IsoGrid::parse(&data, 10, 10), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_count_over_walk_capacity() {
        // w=1,h=1 的行走容量 = 1:两条记录即超界(indicator.map 的恰 1 条是上界内)
        let mut data = Vec::new();
        data.extend_from_slice(&rec(1, 1, 0, 0, 0));
        data.extend_from_slice(&rec(1, 1, 1, 0, 0));
        assert!(matches!(IsoGrid::parse(&data, 1, 1), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_out_of_range_xy() {
        // w=10,h=10 → 菱形界 1..=19;x=20 越界;x=0 越界
        assert!(matches!(
            IsoGrid::parse(&rec(20, 1, 0, 0, 0), 10, 10),
            Err(FormatError::Invalid(_))
        ));
        assert!(matches!(
            IsoGrid::parse(&rec(0, 1, 0, 0, 0), 10, 10),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_duplicate_xy() {
        let mut data = Vec::new();
        data.extend_from_slice(&rec(3, 4, 10, 0, 0));
        data.extend_from_slice(&rec(3, 4, 11, 0, 0));
        assert!(matches!(IsoGrid::parse(&data, 10, 10), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn accepts_legacy_subset_of_walk() {
        // §3.6:riverram 等四图的记录数少于行走容量(缺格 = 存储子集)
        let mut data = Vec::new();
        data.extend_from_slice(&rec(10, 1, 5, 0, 0));
        data.extend_from_slice(&rec(9, 2, 6, 1, 3));
        data.extend_from_slice(&rec(8, 3, -1, 0, 1));
        let grid = IsoGrid::parse(&data, 4, 4).unwrap(); // 容量 (2·4−1)·4 = 28
        assert_eq!(grid.len(), 3);
        assert_eq!(grid.walk_capacity(), 28);
        assert!(grid.cell(7, 4).is_none()); // 行走格缺省无瓦片
        assert_eq!(grid.cell(9, 2).unwrap().sub_tile, 1);
        assert_eq!(grid.max_tile_index(), Some(6)); // −1 不参与最大值
    }

    #[test]
    fn max_tile_index_all_minus_one_is_none() {
        let data = rec(2, 2, -1, 0, 0);
        let grid = IsoGrid::parse(&data, 10, 10).unwrap();
        assert_eq!(grid.max_tile_index(), None);
    }
}
```

- [ ] **Step 2: 运行单元测试通过**

Run: `cargo test -p ra2-core iso`
Expected: 10 passed

- [ ] **Step 3: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 合成 146 passed

```bash
git add crates/core/src/format/map/
git commit -m "feat(core): IsoMapPack5 瓦片网格(11 字节记录/存储坐标权威/稀疏二分索引)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 6: 双层覆盖网格(overlay.rs)

**Files:**
- Create: `crates/core/src/format/map/overlay.rs`
- Modify: `crates/core/src/format/map/mod.rs`(追加 `pub mod overlay;`)

**Interfaces:**
- Consumes: `FormatError`
- Produces(供 Task 9/10 使用):
  - `pub const OVERLAY_GRID_LEN: usize`(= 262144)
  - `pub struct OverlayLayers` + `OverlayLayers::parse(types: &[u8], frames: &[u8]) -> Result<Self, FormatError>`
  - 只读访问:`type_at(x: u16, y: u16) -> u8`、`frame_at(x: u16, y: u16) -> Option<u8>`
- 语义要点(研究 §4,勿改):两段各解压到**恰好 262144 B** = 行主序 512×512(`x = i & 511, y = i >> 9`;XCC `map_ts_encoder.cpp:586-593` 写满格后 LCW),**不是**菱形行走序;OverlayPack[i] = 类型 id(0xFF = 空),OverlayDataPack[i] = 帧/状态(0 是合法值);两层独立——46/540 真图在 type==0xFF 的格下有非零 data(陈旧帧字节,含 EA 官图 c3y01md 等),访问语义:**type==0xFF 的格 frame 恒 None**(陈旧字节忽略,XCC `overlay_data_decode4` 同语义);越界格(x/y ≥ 512 或敌意超大坐标)按空格处理,不 panic。

- [ ] **Step 1: 写 `overlay.rs` 失败测试 + 实现(同文件)**

`crates/core/src/format/map/mod.rs` 追加 `pub mod overlay;`。

`crates/core/src/format/map/overlay.rs` 完整文件:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! `[OverlayPack]` / `[OverlayDataPack]` 解压后的双层 512×512 网格。
//!
//! 布局(研究 2026-11-04-map-pack-and-binary-sections.md §4):两段各解压到
//! **恰好 262144 B** = 行主序 512×512(`x = i & 511, y = i >> 9`;XCC
//! `write_map:586-593` 把内部紧凑流展开成满格再 LCW)——不是菱形行走序。
//! OverlayPack[i] = 类型 id(0xFF = 空),OverlayDataPack[i] = 帧/状态
//! (0 是合法值)。两层独立:46/540 真图在 type==0xFF 的格下有非零 data
//! (陈旧帧字节,含 EA 官图)——访问语义:**type==0xFF 的格 frame 恒 None**
//! (XCC `overlay_data_decode4` 同语义,map_ts_encoder.cpp:221-241)。
//! 菱形坐标 1..=W+H−1 直接当格坐标用(真图 max(W+H) = 499 < 512,恒在格内;
//! 越界格按空格处理,零 panic)。

use super::FormatError;

pub const OVERLAY_GRID_LEN: usize = 512 * 512;

/// 双层覆盖网格(类型层 + 帧/状态层)。
pub struct OverlayLayers {
    types: Box<[u8; OVERLAY_GRID_LEN]>,
    frames: Box<[u8; OVERLAY_GRID_LEN]>,
}

impl OverlayLayers {
    /// 两层解压字节各须恰为 262144(不在中途截断/补零)。
    pub fn parse(types: &[u8], frames: &[u8]) -> Result<Self, FormatError> {
        let types: Box<[u8; OVERLAY_GRID_LEN]> = Box::new(types.try_into().map_err(|v: Vec<u8>| {
            FormatError::Invalid(format!("OverlayPack 解压长 {},应为 {OVERLAY_GRID_LEN}", v.len()))
        })?);
        let frames: Box<[u8; OVERLAY_GRID_LEN]> = Box::new(frames.try_into().map_err(|v: Vec<u8>| {
            FormatError::Invalid(format!(
                "OverlayDataPack 解压长 {},应为 {OVERLAY_GRID_LEN}",
                v.len()
            ))
        })?);
        Ok(OverlayLayers { types, frames })
    }

    fn index(x: u16, y: u16) -> Option<usize> {
        if x >= 512 || y >= 512 {
            return None;
        }
        Some((y as usize) << 9 | x as usize)
    }

    /// 类型 id:0xFF = 空;越界格(敌意坐标)返回 0xFF。
    pub fn type_at(&self, x: u16, y: u16) -> u8 {
        match Self::index(x, y) {
            Some(i) => self.types[i],
            None => 0xFF,
        }
    }

    /// 帧/状态:类型 0xFF 的格(含越界格)返回 None——陈旧 data 字节按格式
    /// 语义忽略,不计为损坏。
    pub fn frame_at(&self, x: u16, y: u16) -> Option<u8> {
        match Self::index(x, y) {
            Some(i) if self.types[i] != 0xFF => Some(self.frames[i]),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layers() -> OverlayLayers {
        let mut types = vec![0xFFu8; OVERLAY_GRID_LEN];
        let mut frames = vec![0xEEu8; OVERLAY_GRID_LEN];
        // 格 (3,2):类型 7、帧 3
        types[3 + (2 << 9)] = 7;
        frames[3 + (2 << 9)] = 3;
        // 格 (4,2):类型 0xFF(空)但帧非零——陈旧字节(46/540 真图形态)
        frames[4 + (2 << 9)] = 9;
        OverlayLayers::parse(&types, &frames).unwrap()
    }

    #[test]
    fn row_major_indexing_roundtrip() {
        let layers = layers();
        assert_eq!(layers.type_at(3, 2), 7);
        assert_eq!(layers.type_at(0, 0), 0xFF);
        assert_eq!(layers.frame_at(3, 2), Some(3));
    }

    #[test]
    fn stale_data_under_empty_type_is_ignored() {
        // type==0xFF 且 data≠0:.frame 恒 None(陈旧字节忽略,不报损坏)
        let layers = layers();
        assert_eq!(layers.type_at(4, 2), 0xFF);
        assert_eq!(layers.frame_at(4, 2), None);
    }

    #[test]
    fn out_of_range_cell_behaves_as_empty() {
        let layers = layers();
        assert_eq!(layers.type_at(600, 0), 0xFF);
        assert_eq!(layers.frame_at(600, 0), None);
    }

    #[test]
    fn rejects_short_types() {
        assert!(matches!(
            OverlayLayers::parse(&vec![0u8; OVERLAY_GRID_LEN - 1], &vec![0u8; OVERLAY_GRID_LEN]),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_short_frames() {
        assert!(matches!(
            OverlayLayers::parse(&vec![0u8; OVERLAY_GRID_LEN], &vec![0u8; 5]),
            Err(FormatError::Invalid(_))
        ));
    }
}
```

- [ ] **Step 2: 运行单元测试通过**

Run: `cargo test -p ra2-core overlay`
Expected: 5 passed

- [ ] **Step 3: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 合成 151 passed

```bash
git add crates/core/src/format/map/
git commit -m "feat(core): 双层覆盖网格(512×512 行主序/陈旧帧字节忽略)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 7: 预览位图(preview.rs)+ 元数据段(meta.rs)

**Files:**
- Create: `crates/core/src/format/map/preview.rs`
- Create: `crates/core/src/format/map/meta.rs`
- Modify: `crates/core/src/format/map/mod.rs`(追加 `pub mod preview;` 与 `pub mod meta;`)
- Modify: `crates/core/src/format/ini.rs`(仅可见性:`fn parse_int_prefix` → `pub fn parse_int_prefix`,Task 2 已实现,meta.rs 复用)

**Interfaces:**
- Consumes: `IniDocument`(Task 2 后)、`base64_decode`(Task 4)、`parse_int_prefix`(Task 2,pub 化)
- Produces(供 Task 9/10 使用):
  - `pub const PREVIEW_MAX_PIXELS: u64`(4096²,尺寸谎报防护)
  - `pub struct PreviewImage { pub width: u32, pub height: u32, pub rgb: Vec<u8> }` + `PreviewImage::parse(data: &[u8], pw: u32, ph: u32) -> Option<Self>`(长度恰 3·pw·ph 才 Some;截断/超长/尺寸非法 → None)
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum Theater { Temperate, Snow, Urban, Desert, NewUrban, Lunar }` + `Theater::parse(s: &str) -> Result<Self, FormatError>`(未知 → Unsupported)+ `Theater::iso_palette_file(&self) -> &'static str`
  - `pub struct BasicInfo { pub name/author/next_scenario: Option<String>, pub official/multiplayer_only: bool, pub max_player/min_player: i64 }` + `BasicInfo::from_doc(&IniDocument) -> Option<Self>`
  - `pub struct MapMeta { pub theater: Theater, pub size: [i64; 4], pub local_size: [i64; 4] }` + `MapMeta::from_doc(&IniDocument) -> Result<Self, FormatError>`([Map] 缺失 → Invalid)
  - `pub struct HeaderMeta { pub start_x/start_y/width/height/number_starting_points: i64 }` + `HeaderMeta::from_doc(&IniDocument) -> Self`
  - `pub struct Lighting { pub ambient/red/green/blue/ground/level/ion_ambient/ion_red/ion_green/ion_blue/ion_ground/dominator_ambient/dominator_red/dominator_green: Option<f32> }` + `Lighting::from_doc(&IniDocument) -> Self`
  - `pub fn parse_digest(doc: &IniDocument) -> Option<[u8; 20]>`([Digest] 只读取不校验)
- 语义要点(研究 §5/§7.1-7.2、普查 §4,勿改):预览是**裸 24bpp RGB 无头位图**,长度恒 3·[Preview]Size 的 pw·ph;像素序/通道序 M2 不裁决原样暴露;Theater 六值来自 gamemd.exe 剧场表(精确大写为主、ci 兜底);ISO 调色板 = ra2.mix → id `0x3B5A96DE` 嵌套库内 `isotem/isosno/isourb` 等 768 B 文件(DESERT/NEWURBAN/LUNAR 的在本机缺席,调用方读不到时报错);[Map]Size 用 i64 原样保留(RMG indicator 有负 LocalSize),不设界不解释。

- [ ] **Step 1: ini.rs 的 `parse_int_prefix` 改为 `pub fn`(仅可见性)**

Task 2 已写入该函数;把 `fn parse_int_prefix(v: &str) -> Option<i64> {` 改为 `pub fn parse_int_prefix(v: &str) -> Option<i64> {`(文档注释已注明 pub 用途)。

- [ ] **Step 2: 写 `preview.rs` 失败测试 + 实现(同文件)**

`crates/core/src/format/map/mod.rs` 追加:

```rust
pub mod preview;
pub mod meta;
```

`crates/core/src/format/map/preview.rs` 完整文件:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! `[PreviewPack]` 解压后的小地图位图:裸 24bpp RGB,无头。
//!
//! 研究 §5:EA 产图长度恰为 3·[Preview]Size 的 pw·ph(c1a01md 3·162·59 =
//! 28674 ✓);社区产图器的截断/异常尾块由 pack 层截停(研究 §5 + 2026-11-04
//! 复核:7 张 DDLY 图的干净前缀全部恰为 3·pw·ph,异常块只是位图完整后的
//! 尾垃圾)。前缀长度 ≠ 3·pw·ph 时返回 None("无预览",不报错)。
//! 像素序/通道序(BGR?底行在上?)M2 不裁决,字节原样暴露(渲染时定,
//! 研究 §14-3)。

/// 预览尺寸合法性上界([Preview]Size 谎报防护;语料真图最大 798×100)。
pub const PREVIEW_MAX_PIXELS: u64 = 16_777_216; // 4096²

pub struct PreviewImage {
    pub width: u32,
    pub height: u32,
    /// 24bpp RGB,行主序(字节原样;长度恒 3·width·height)。
    pub rgb: Vec<u8>,
}

impl PreviewImage {
    pub fn parse(data: &[u8], pw: u32, ph: u32) -> Option<Self> {
        if pw == 0 || ph == 0 || pw > 8192 || ph > 8192 {
            return None;
        }
        let pixels = pw as u64 * ph as u64;
        if pixels > PREVIEW_MAX_PIXELS {
            return None;
        }
        if data.len() as u64 != 3 * pixels {
            return None;
        }
        Some(PreviewImage {
            width: pw,
            height: ph,
            rgb: data.to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_length_some_verbatim() {
        let data: Vec<u8> = (0..3 * 4 * 2).map(|i| (i % 251) as u8).collect();
        let img = PreviewImage::parse(&data, 4, 2).unwrap();
        assert_eq!((img.width, img.height), (4, 2));
        assert_eq!(img.rgb, data);
    }

    #[test]
    fn truncated_prefix_is_none() {
        let data = vec![0u8; 3 * 4 * 2 - 3];
        assert!(PreviewImage::parse(&data, 4, 2).is_none());
    }

    #[test]
    fn longer_than_3wh_is_none() {
        let data = vec![0u8; 3 * 4 * 2 + 1];
        assert!(PreviewImage::parse(&data, 4, 2).is_none());
    }

    #[test]
    fn hostile_dimensions_are_none_not_panic() {
        // 尺寸谎报:u64 算术,3·10⁹ 不溢出、超界返回 None(debug 零 panic)
        assert!(PreviewImage::parse(&[], 1_000_000_000, 1_000_000_000).is_none());
        assert!(PreviewImage::parse(&[], 0, 2).is_none());
        assert!(PreviewImage::parse(&[], 8193, 2).is_none());
    }
}
```

- [ ] **Step 3: 写 `meta.rs` 失败测试 + 实现(同文件)**

`crates/core/src/format/map/meta.rs` 完整文件:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! 元数据段结构化读取([Basic] / [Map] / [Header] / [Lighting] / [Digest])。
//!
//! 普查 §4:四段 807/807 全在;[Map]Size 恒 4 int 且前两个恒 0,0(数值用
//! i64 原样保留——RMG indicator 的负 LocalSize 不拒绝);Theater ∈ 六值
//! (研究 §7.1 gamemd.exe 偏移 4070264-4070930 剧场表,精确大写为主、
//! 大小写不敏感兜底);[Lighting] 14 个六位小数 float;[Digest] = 28 字符
//! 标准 base64 → 20 字节,只读取不校验。[Header] 的 StartX/Waypoint1..8
//! 语义未知(研究 §9),按 get_int 原样透传不解释。

use crate::format::ini::IniDocument;
use crate::format::pack::base64_decode;
use crate::format::FormatError;

/// 剧场(gamemd.exe 剧场表六行;INI 名精确大写为主)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theater {
    Temperate,
    Snow,
    Urban,
    Desert,
    NewUrban,
    Lunar,
}

impl Theater {
    pub fn parse(s: &str) -> Result<Self, FormatError> {
        match s.to_ascii_uppercase().as_str() {
            "TEMPERATE" => Ok(Theater::Temperate),
            "SNOW" => Ok(Theater::Snow),
            "URBAN" => Ok(Theater::Urban),
            "DESERT" => Ok(Theater::Desert),
            "NEWURBAN" => Ok(Theater::NewUrban),
            "LUNAR" => Ok(Theater::Lunar),
            _ => Err(FormatError::Unsupported(format!(
                "未知剧场 {s}(剧场表六值之外)"
            ))),
        }
    }

    /// 剧场 ISO 调色板文件名(资源链:ra2.mix → id 0x3B5A96DE 嵌套库,
    /// 研究 §7.2;DESERT/NEWURBAN/LUNAR 的 YR 调色板在本机安装缺席——
    /// 调用方读不到时报错退出)。
    pub fn iso_palette_file(&self) -> &'static str {
        match self {
            Theater::Temperate => "isotem.pal",
            Theater::Snow => "isosno.pal",
            Theater::Urban => "isourb.pal",
            Theater::Desert => "isodes.pal",
            Theater::NewUrban => "isoubn.pal",
            Theater::Lunar => "isolun.pal",
        }
    }
}

/// [Basic] 的结构化子集(CLI info 用;其余键留在 IniDocument 不重复存储)。
pub struct BasicInfo {
    pub name: Option<String>,
    pub author: Option<String>,
    pub next_scenario: Option<String>,
    pub official: bool,
    pub multiplayer_only: bool,
    pub max_player: i64,
    pub min_player: i64,
}

impl BasicInfo {
    /// 无 [Basic] 段时返回 None(fcrash_backup 形态;普查 807/807 有段,
    /// 敌意/极端文件容错)。
    pub fn from_doc(doc: &IniDocument) -> Option<Self> {
        doc.section("Basic")?;
        Some(BasicInfo {
            name: doc.value("Basic", "Name").map(str::to_string),
            author: doc.value("Basic", "Author").map(str::to_string),
            next_scenario: doc.value("Basic", "NextScenario").map(str::to_string),
            official: doc.get_bool("Basic", "Official", false),
            multiplayer_only: doc.get_bool("Basic", "MultiplayerOnly", false),
            max_player: doc.get_int("Basic", "MaxPlayer", 0),
            min_player: doc.get_int("Basic", "MinPlayer", 0),
        })
    }
}

/// [Map] 段(Theater + Size + LocalSize;i64 原样保留不设界)。
pub struct MapMeta {
    pub theater: Theater,
    pub size: [i64; 4],
    pub local_size: [i64; 4],
}

fn parse_csv4(s: &str, what: &str) -> Result<[i64; 4], FormatError> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return Err(FormatError::Invalid(format!(
            "[Map]{what} 应为 4 个整数,实得 {} 项",
            parts.len()
        )));
    }
    let mut out = [0i64; 4];
    for (slot, part) in out.iter_mut().zip(&parts) {
        // atoi 前缀语义与 get_int 一致(Task 2 的 pub parse_int_prefix)
        *slot = crate::format::ini::parse_int_prefix(part).unwrap_or(0);
    }
    Ok(out)
}

impl MapMeta {
    pub fn from_doc(doc: &IniDocument) -> Result<Self, FormatError> {
        let theater = Theater::parse(
            doc.value("Map", "Theater")
                .ok_or_else(|| FormatError::Invalid("缺少 [Map]Theater".to_string()))?,
        )?;
        let size = parse_csv4(
            doc.value("Map", "Size")
                .ok_or_else(|| FormatError::Invalid("缺少 [Map]Size".to_string()))?,
            "Size",
        )?;
        let local_size = parse_csv4(doc.value("Map", "LocalSize").unwrap_or("0,0,0,0"), "LocalSize")?;
        Ok(MapMeta {
            theater,
            size,
            local_size,
        })
    }
}

/// [Header] 段(语义未知字段按 get_int 原样透传;研究 §9 UNKNOWN)。
pub struct HeaderMeta {
    pub start_x: i64,
    pub start_y: i64,
    pub width: i64,
    pub height: i64,
    pub number_starting_points: i64,
}

impl HeaderMeta {
    pub fn from_doc(doc: &IniDocument) -> Self {
        HeaderMeta {
            start_x: doc.get_int("Header", "StartX", -1),
            start_y: doc.get_int("Header", "StartY", -1),
            width: doc.get_int("Header", "Width", -1),
            height: doc.get_int("Header", "Height", -1),
            number_starting_points: doc.get_int("Header", "NumberStartingPoints", 0),
        }
    }
}

/// [Lighting] 的 14 个 float 键(缺失/非数 → None;std f32 解析,零依赖)。
pub struct Lighting {
    pub ambient: Option<f32>,
    pub red: Option<f32>,
    pub green: Option<f32>,
    pub blue: Option<f32>,
    pub ground: Option<f32>,
    pub level: Option<f32>,
    pub ion_ambient: Option<f32>,
    pub ion_red: Option<f32>,
    pub ion_green: Option<f32>,
    pub ion_blue: Option<f32>,
    pub ion_ground: Option<f32>,
    pub dominator_ambient: Option<f32>,
    pub dominator_red: Option<f32>,
    pub dominator_green: Option<f32>,
}

impl Lighting {
    fn f(doc: &IniDocument, key: &str) -> Option<f32> {
        doc.value("Lighting", key)
            .and_then(|v| v.trim().parse::<f32>().ok())
    }

    pub fn from_doc(doc: &IniDocument) -> Self {
        Lighting {
            ambient: Self::f(doc, "Ambient"),
            red: Self::f(doc, "Red"),
            green: Self::f(doc, "Green"),
            blue: Self::f(doc, "Blue"),
            ground: Self::f(doc, "Ground"),
            level: Self::f(doc, "Level"),
            ion_ambient: Self::f(doc, "IonAmbient"),
            ion_red: Self::f(doc, "IonRed"),
            ion_green: Self::f(doc, "IonGreen"),
            ion_blue: Self::f(doc, "IonBlue"),
            ion_ground: Self::f(doc, "IonGround"),
            dominator_ambient: Self::f(doc, "DominatorAmbient"),
            dominator_red: Self::f(doc, "DominatorRed"),
            dominator_green: Self::f(doc, "DominatorGreen"),
        }
    }
}

/// [Digest] 的 20 字节(单键 `1=`,28 字符标准 base64;缺失/畸形 → None,
/// 只读取不校验)。
pub fn parse_digest(doc: &IniDocument) -> Option<[u8; 20]> {
    let v = doc.value("Digest", "1")?;
    base64_decode(v).ok()?.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> IniDocument {
        IniDocument::parse(text)
    }

    #[test]
    fn theater_parses_six_values_case_insensitively() {
        for (s, want) in [
            ("TEMPERATE", Theater::Temperate),
            ("snow", Theater::Snow),
            ("URBAN", Theater::Urban),
            ("Desert", Theater::Desert),
            ("NEWURBAN", Theater::NewUrban),
            ("Lunar", Theater::Lunar),
        ] {
            assert_eq!(Theater::parse(s).unwrap(), want);
        }
        assert!(matches!(
            Theater::parse("BOGUS"),
            Err(FormatError::Unsupported(_))
        ));
        // 调色板文件名(研究 §7.1)
        assert_eq!(Theater::Snow.iso_palette_file(), "isosno.pal");
        assert_eq!(Theater::Temperate.iso_palette_file(), "isotem.pal");
    }

    #[test]
    fn basic_from_ini() {
        let d = doc(
            "[Basic]\nName=海嘯\nAuthor=Handama\nNextScenario=GDI2A.map\nOfficial=yes\nMultiplayerOnly=1\nMaxPlayer=6\nMinPlayer=2\n",
        );
        let b = BasicInfo::from_doc(&d).unwrap();
        assert_eq!(b.name.as_deref(), Some("海嘯"));
        assert_eq!(b.author.as_deref(), Some("Handama"));
        assert_eq!(b.next_scenario.as_deref(), Some("GDI2A.map"));
        assert!(b.official);
        assert!(b.multiplayer_only); // MultiplayerOnly=1 亦为真(get_bool 语义)
        assert_eq!((b.max_player, b.min_player), (6, 2));
        // 无 [Basic] 段 → None(fcrash_backup 形态)
        assert!(BasicInfo::from_doc(&doc("[Map]\nTheater=SNOW\n")).is_none());
    }

    #[test]
    fn map_meta_requires_section_and_validates() {
        // [Map] 缺失 → Invalid(807/807 全有,敌意才缺)
        assert!(matches!(
            MapMeta::from_doc(&doc("[Basic]\nName=X\n")),
            Err(FormatError::Invalid(_))
        ));
        // 剧场未知 → Unsupported
        assert!(matches!(
            MapMeta::from_doc(&doc("[Map]\nTheater=BOGUS\nSize=0,0,10,10\n")),
            Err(FormatError::Unsupported(_))
        ));
        // 正常 + 负值原样(indicator.map 的负 LocalSize)
        let m = MapMeta::from_doc(&doc(
            "[Map]\nTheater=DESERT\nSize=0,0,1,1\nLocalSize=3,5,-5,-10\n",
        ))
        .unwrap();
        assert_eq!(m.theater, Theater::Desert);
        assert_eq!(m.size, [0, 0, 1, 1]);
        assert_eq!(m.local_size, [3, 5, -5, -10]);
        // 项数错 → Invalid
        assert!(matches!(
            MapMeta::from_doc(&doc("[Map]\nTheater=SNOW\nSize=0,0,60\n")),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn header_and_lighting_from_ini() {
        let d = doc(
            "[Header]\nStartX=233\nStartY=30\nWidth=45\nHeight=44\nNumberStartingPoints=0\n\
             [Lighting]\nAmbient=1.000000\nRed=0.550000\nLevel=0.030000\n",
        );
        let h = HeaderMeta::from_doc(&d);
        assert_eq!(
            (h.start_x, h.start_y, h.width, h.height, h.number_starting_points),
            (233, 30, 45, 44, 0)
        );
        let l = Lighting::from_doc(&d);
        assert_eq!(l.ambient, Some(1.0));
        assert_eq!(l.red, Some(0.55));
        assert_eq!(l.green, None);
        assert_eq!(l.level, Some(0.03));
    }

    #[test]
    fn digest_decodes_twenty_bytes() {
        // 规格依据 §9:c1a01md 的 [Digest] 样本
        let d = doc("[Digest]\n1=qSt1w6I0r+9EWkfWl7XFZMt0Vo4=\n");
        let digest = parse_digest(&d).unwrap();
        assert_eq!(
            digest,
            [
                0xA9, 0x2B, 0x75, 0xC3, 0xA2, 0x34, 0xAF, 0xEF, 0x44, 0x5A, 0x47, 0xD6, 0x97,
                0xB5, 0xC5, 0x64, 0xCB, 0x74, 0x56, 0x8E
            ]
        );
        // 缺失 → None;畸形 → None(容错,不报错)
        assert!(parse_digest(&doc("[Basic]\nName=X\n")).is_none());
        assert!(parse_digest(&doc("[Digest]\n1=不合法\n")).is_none());
    }
}
```

- [ ] **Step 4: 运行单元测试通过**

Run: `cargo test -p ra2-core preview && cargo test -p ra2-core meta && cargo test -p ra2-core ini`
Expected: preview 4 passed;meta 5 passed;ini 11 passed(可见性改动零影响)

- [ ] **Step 5: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 合成 160 passed

```bash
git add crates/core/src/format/map/ crates/core/src/format/ini.rs
git commit -m "feat(core): 预览裸 RGB 位图与元数据段结构化(剧场六值/Basic/Map/Header/Lighting/Digest)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 8: 对象段结构化(objects.rs)

**Files:**
- Create: `crates/core/src/format/map/objects.rs`
- Modify: `crates/core/src/format/map/mod.rs`(追加 `pub mod objects;`)

**Interfaces:**
- Consumes: `IniDocument`
- Produces(供 Task 9/10/11 使用):
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum ObjectClass { Structure, Unit, Infantry, Aircraft, Terrain, Smudge }`
  - `#[derive(Debug, Clone)] pub struct MapObject { pub class: ObjectClass, pub key: String, pub fields: Vec<String>, pub malformed: bool }`
  - 访问器:`owner() -> Option<&str>`、`kind() -> Option<&str>`、`cell() -> Option<(u32, u32)>`、`facing() -> Option<u32>`、`sub_cell() -> Option<u8>`
  - `pub struct MapObjects { pub all: Vec<MapObject>, pub malformed: usize }` + `MapObjects::count(&self, class: ObjectClass) -> usize`
  - `pub fn parse_objects(doc: &IniDocument) -> MapObjects`
- 语义要点(普查 §4.7/§4.8,勿改):字段数恒定——Structures 17、Units/Infantry 14、Aircraft 12、Terrain 1 token、Smudge 4;编号键段坐标在 field 3/4,Infantry 的 field 5 是 sub_cell 0..=5(field 6 跨段过载,不做共享布局);[Terrain] 的 key = Y·1000+X 坐标键、value 单 token;[Smudge] 坐标在 field 1/2;键是大 ID(1025970…)时**绝不能当数组下标——逐项枚举**;真图带真实损坏(勇士与巨龙.map 的 16 字段 Structures 行)→ 字段保留、malformed 标记、计数上报、不中断。

- [ ] **Step 1: 写 `objects.rs` 失败测试 + 实现(同文件)**

`crates/core/src/format/map/mod.rs` 追加 `pub mod objects;`。

`crates/core/src/format/map/objects.rs` 完整文件(测试常量为普查 §4.7/§4.8 与规格依据 §9 钉死的真图原始行):

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! 对象段结构化:[Structures] / [Units] / [Infantry] / [Aircraft] /
//! [Terrain] / [Smudge] → 类型化对象列表。
//!
//! 布局(普查 2026-11-04-map-ini-survey.md §4.7/§4.8;字段数精确且恒定):
//! - [Structures] 17 字段:`owner,type,strength,X,Y,facing,tag,7 个标志位,
//!   None×3,末两位`——X/Y(field 3/4)即菱形坐标
//! - [Units] / [Infantry] 14 字段:同族布局,field 5 在 Units 是 facing、
//!   在 Infantry 是 sub_cell(0..=5);field 6 跨段过载(mission 词/None/
//!   hex8 tag),按段各有含义,不做共享布局
//! - [Aircraft] 12 字段,同族
//! - [Terrain]:key = 菱形坐标键 Y·1000+X,value = 单 token(类型名)
//! - [Smudge]:key = 顺序编号,value = `name,X,Y,0`(坐标在字段里)
//! 键形态:编号 0..n-1 或大 ID(1025970…)——**绝不用 key 当下标,逐项枚举**。
//! 真图带真实损坏(普查:勇士与巨龙.map 第 6187 行的 16 字段 Structures 行,
//! owner+type 丢逗号)——字段数与段布局不符时保留原始字段、标 malformed、
//! 计数上报,不中断解析。

use crate::format::ini::IniDocument;

/// 对象段类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectClass {
    Structure,
    Unit,
    Infantry,
    Aircraft,
    Terrain,
    Smudge,
}

/// 一个对象:原始键 + 原始 CSV 字段(trim 后)+ 宽容解析的派生访问器。
#[derive(Debug, Clone)]
pub struct MapObject {
    pub class: ObjectClass,
    /// 段内原始键(编号键或坐标键的十进制文本)。
    pub key: String,
    /// 原始 CSV 字段(trim 后;Terrain 为单元素)。
    pub fields: Vec<String>,
    /// 字段数与段布局不符——对象保留,计入 MapObjects::malformed。
    pub malformed: bool,
}

impl MapObject {
    fn field(&self, i: usize) -> Option<&str> {
        self.fields.get(i).map(String::as_str)
    }

    /// 所属方(编号键段 field 0;Terrain/Smudge 无)。
    pub fn owner(&self) -> Option<&str> {
        match self.class {
            ObjectClass::Terrain | ObjectClass::Smudge => None,
            _ => self.field(0),
        }
    }

    /// 类型名(编号键段 field 1;Terrain/Smudge 为 field 0)。
    pub fn kind(&self) -> Option<&str> {
        match self.class {
            ObjectClass::Terrain | ObjectClass::Smudge => self.field(0),
            _ => self.field(1),
        }
    }

    /// 菱形坐标 (x,y):编号键段取 field 3/4;Terrain 由键 Y·1000+X 解出;
    /// Smudge 取 field 1/2。任一分量非整数 → None(宽容,零 panic)。
    pub fn cell(&self) -> Option<(u32, u32)> {
        match self.class {
            ObjectClass::Terrain => {
                let n: u32 = self.key.parse().ok()?;
                Some((n % 1000, n / 1000))
            }
            ObjectClass::Smudge => Some((
                self.fields.get(1)?.trim().parse().ok()?,
                self.fields.get(2)?.trim().parse().ok()?,
            )),
            _ => Some((
                self.fields.get(3)?.trim().parse().ok()?,
                self.fields.get(4)?.trim().parse().ok()?,
            )),
        }
    }

    /// 朝向(编号键段 field 5;Infantry 该位是 sub_cell → None)。
    pub fn facing(&self) -> Option<u32> {
        match self.class {
            ObjectClass::Infantry | ObjectClass::Terrain | ObjectClass::Smudge => None,
            _ => self.field(5)?.trim().parse().ok(),
        }
    }

    /// 步兵 sub_cell(仅 Infantry field 5,合法域 0..=5,越域 → None)。
    pub fn sub_cell(&self) -> Option<u8> {
        match self.class {
            ObjectClass::Infantry => self.field(5)?.trim().parse().ok().filter(|&s| s <= 5),
            _ => None,
        }
    }
}

/// 全部对象 + 损坏行计数。
pub struct MapObjects {
    pub all: Vec<MapObject>,
    pub malformed: usize,
}

impl MapObjects {
    /// 某类对象数(CLI info 的分档计数)。
    pub fn count(&self, class: ObjectClass) -> usize {
        self.all.iter().filter(|o| o.class == class).count()
    }
}

/// (段名, 类别, 期望字段数)。
const SECTION_LAYOUTS: [(&str, ObjectClass, usize); 6] = [
    ("Structures", ObjectClass::Structure, 17),
    ("Units", ObjectClass::Unit, 14),
    ("Infantry", ObjectClass::Infantry, 14),
    ("Aircraft", ObjectClass::Aircraft, 12),
    ("Terrain", ObjectClass::Terrain, 1),
    ("Smudge", ObjectClass::Smudge, 4),
];

/// 从文档抽取六段全部对象(段缺失 = 空档;枚举顺序 = 文件序)。
pub fn parse_objects(doc: &IniDocument) -> MapObjects {
    let mut all = Vec::new();
    let mut malformed = 0usize;
    for (section, class, expected) in SECTION_LAYOUTS {
        let Some(sec) = doc.section(section) else {
            continue;
        };
        for (key, value) in &sec.entries {
            let fields: Vec<String> = value.split(',').map(str::trim).map(str::to_string).collect();
            let is_malformed = fields.len() != expected;
            if is_malformed {
                malformed += 1;
            }
            all.push(MapObject {
                class,
                key: key.clone(),
                fields,
                malformed: is_malformed,
            });
        }
    }
    MapObjects { all, malformed }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(section: &str, key: &str, value: &str) -> MapObject {
        let doc = IniDocument::parse(&format!("[{section}]\n{key}={value}\n"));
        let objs = parse_objects(&doc);
        assert_eq!(objs.all.len(), 1);
        objs.all[0].clone()
    }

    #[test]
    fn structures_17_fields_real_row() {
        // 普查 §4.7 的真实样例行
        let o = one(
            "Structures",
            "0",
            "Neutral,CABHUT,256,103,64,0,None,0,0,1,0,0,None,None,None,0,0",
        );
        assert!(!o.malformed);
        assert_eq!(o.owner(), Some("Neutral"));
        assert_eq!(o.kind(), Some("CABHUT"));
        assert_eq!(o.cell(), Some((103, 64)));
        assert_eq!(o.facing(), Some(0));
        assert_eq!(o.sub_cell(), None);
    }

    #[test]
    fn units_14_fields_real_row_with_negative_flags() {
        // 普查 §4.7:负数字段(-1)必须宽容存活
        let o = one("Units", "0", "Neutral,CAR,256,66,104,0,Sleep,None,0,-1,0,-1,1,1");
        assert!(!o.malformed);
        assert_eq!(o.kind(), Some("CAR"));
        assert_eq!(o.cell(), Some((66, 104)));
        assert_eq!(o.facing(), Some(0));
        assert_eq!(o.fields.len(), 14);
    }

    #[test]
    fn infantry_subcell_real_row() {
        let o = one("Infantry", "0", "Neutral,COW,256,82,81,3,Guard,125,None,0,-1,0,1,1");
        assert!(!o.malformed);
        assert_eq!(o.cell(), Some((82, 81)));
        assert_eq!(o.sub_cell(), Some(3));
        assert_eq!(o.facing(), None); // Infantry field 5 是 sub_cell
    }

    #[test]
    fn aircraft_12_fields_real_row() {
        let o = one("Aircraft", "0", "Neutral,PDPLANE,256,108,168,64,Guard,None,0,-1,1,1");
        assert!(!o.malformed);
        assert_eq!(o.cell(), Some((108, 168)));
        assert_eq!(o.facing(), Some(64));
    }

    #[test]
    fn terrain_coordinate_key_decodes() {
        // 普查 §4.8:key = Y·1000+X,value = 单 token
        let o = one("Terrain", "101043", "TIBTRE01");
        assert!(!o.malformed);
        assert_eq!(o.cell(), Some((43, 101)));
        assert_eq!(o.kind(), Some("TIBTRE01"));
        assert_eq!(o.owner(), None);
        // 非整数键 → cell None,不 panic(负值是 [Waypoints] 的事,但宽容至上)
        let o = one("Terrain", "junk", "TIBTRE01");
        assert_eq!(o.cell(), None);
    }

    #[test]
    fn smudge_coordinate_in_fields() {
        // 普查 §4.8:4 字段 name,X,Y,0
        let o = one("Smudge", "0", "CRATER07,92,153,0");
        assert!(!o.malformed);
        assert_eq!(o.cell(), Some((92, 153)));
        assert_eq!(o.kind(), Some("CRATER07"));
    }

    #[test]
    fn corrupt_16_field_row_is_malformed_and_kept() {
        // 规格依据 §9:勇士与巨龙.map 第 6187 行(16 字段,owner+type 丢逗号)
        let doc = IniDocument::parse(
            "[Structures]\n69=NeutralGAWEAT,256,138,264,0,None,1,0,1,0,0,None,None,None,1,0\n",
        );
        let objs = parse_objects(&doc);
        assert_eq!(objs.all.len(), 1);
        assert_eq!(objs.malformed, 1);
        let o = &objs.all[0];
        assert!(o.malformed);
        assert_eq!(o.fields.len(), 16);
        // 坐标位(3/4)未受损坏影响,仍可读
        assert_eq!(o.cell(), Some((138, 264)));
    }

    #[test]
    fn counts_rollup_per_class() {
        let doc = IniDocument::parse(
            "[Structures]\n0=Neutral,CABHUT,256,1,1,0,None,0,0,1,0,0,None,None,None,0,0\n\
             [Units]\n0=Neutral,CAR,256,2,2,0,Sleep,None,0,-1,0,-1,1,1\n\
             [Terrain]\n1002=TIBTRE01\n",
        );
        let objs = parse_objects(&doc);
        assert_eq!(objs.count(ObjectClass::Structure), 1);
        assert_eq!(objs.count(ObjectClass::Unit), 1);
        assert_eq!(objs.count(ObjectClass::Infantry), 0);
        assert_eq!(objs.count(ObjectClass::Aircraft), 0);
        assert_eq!(objs.count(ObjectClass::Terrain), 1);
        assert_eq!(objs.count(ObjectClass::Smudge), 0);
        assert_eq!(objs.malformed, 0);
    }

    #[test]
    fn big_id_keys_are_enumerated_not_indexed() {
        // 普查 §4.7:183 张 MadHQ 图的键是排序大 ID——绝不当数组下标
        let doc = IniDocument::parse(
            "[Structures]\n1025970=Neutral,CABHUT,256,1,1,0,None,0,0,1,0,0,None,None,None,0,0\n\
             1025971=Neutral,CABHUT,256,2,2,0,None,0,0,1,0,0,None,None,None,0,0\n",
        );
        let objs = parse_objects(&doc);
        assert_eq!(objs.all.len(), 2);
        assert_eq!(objs.all[0].key, "1025970");
        assert_eq!(objs.all[1].key, "1025971");
        assert_eq!(objs.all[1].cell(), Some((2, 2)));
    }
}
```

- [ ] **Step 2: 运行单元测试通过**

Run: `cargo test -p ra2-core objects`
Expected: 9 passed

- [ ] **Step 3: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 合成 169 passed

```bash
git add crates/core/src/format/map/
git commit -m "feat(core): 对象段结构化(六段分档布局/坐标键解码/损坏行宽容计数)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 9: MapFile 总入口(map/mod.rs)+ 全量语料遍历

**Files:**
- Modify: `crates/core/src/format/map/mod.rs`(追加 MapFile/LogicSection/PackStats 与 parse 总装;追加 11 个合成集成测试)
- Modify: `crates/core/tests/map_samples.rs`(追加 3 个 ignored 测试)

**Interfaces:**
- Consumes: 前 8 个任务的全部产物
- Produces(供 Task 10/11 使用):
  - `pub struct MapFile { pub basic: Option<BasicInfo>, pub map_meta: MapMeta, pub header: HeaderMeta, pub lighting: Lighting, pub digest: Option<[u8; 20]>, pub tiles: IsoGrid, pub overlays: OverlayLayers, pub preview: Option<PreviewImage>, pub objects: MapObjects, pub logic_sections: Vec<LogicSection>, pub orphan_key_values: usize, pub pack_stats: PackStats }`
  - `MapFile::parse(data: &[u8]) -> Result<MapFile, FormatError>`(零 IO、零 panic;API 形状冻结为 parse + 只读字段)
  - `pub struct LogicSection { pub name: String, pub entries: Vec<(String, String)> }`(tokenize-only 段的原始键值行)
  - `pub struct PackStats { pub iso_out: usize, pub overlay_out: usize, pub overlay_data_out: usize, pub preview_out: usize, pub preview_truncated: bool }`
- 装配语义(全部来自研究/普查已裁决项,勿改):decode_map_text → IniDocument;[Map] 缺失/剧场未知 → Invalid/Unsupported;IsoMapPack5 缺失 = 空平地(templateMap 形态);overlay 双段缺失 → Invalid(807/807 全有);PreviewPack/[Preview] 缺失 = 无预览;瓦片流上界 = [Map]Size 合法域 1..=4096 内的 11·(2W−1)·H+4(超 300×300 兜底常量 1,976,704 时钳到兜底——真图最大 943,734),否则兜底 (300,300);logic 段按白名单 tokenize([AITriggerTypes, AITriggerTypesEnable, Actions, CellTags, Events, ScriptTypes, Tags, TaskForces, TeamTypes, Triggers, VariableNames, Waypoints],覆盖 spec §3 清单 + 语料中的同族段)。

- [ ] **Step 1: 写 MapFile 总装(map/mod.rs 追加;`pub mod` 清单补全)**

map/mod.rs 的模块声明区替换为完整清单:

```rust
pub mod gbk;
pub mod gbk_table;
pub mod iso;
pub mod mapdoc;
pub mod meta;
pub mod objects;
pub mod overlay;
pub mod pack;
pub mod preview;
```

模块文档注释区之后追加总装代码:

```rust
use crate::format::ini::IniDocument;
use crate::format::FormatError;
use iso::IsoGrid;
use meta::{BasicInfo, HeaderMeta, Lighting, MapMeta};
use objects::MapObjects;
use overlay::OverlayLayers;
use pack::{decode_pack_section, PackBound, PackCodec};

/// logic 段白名单(tokenize-only:原始键值行保留,不解码语义;spec §3 清单
/// + 语料同族段 [AITriggerTypes]/[AITriggerTypesEnable]/[CellTags])。
const TOKENIZE_SECTIONS: [&str; 12] = [
    "AITriggerTypes",
    "AITriggerTypesEnable",
    "Actions",
    "CellTags",
    "Events",
    "ScriptTypes",
    "Tags",
    "TaskForces",
    "TeamTypes",
    "Triggers",
    "VariableNames",
    "Waypoints",
];

/// 一个 tokenize-only 保留段(原始键值行;键值如文件所示,ci 查找用规范名)。
#[derive(Debug, Clone)]
pub struct LogicSection {
    pub name: String,
    pub entries: Vec<(String, String)>,
}

/// 四包段解压字节数统计(CLI info 用)。
#[derive(Debug, Clone, Copy, Default)]
pub struct PackStats {
    pub iso_out: usize,
    pub overlay_out: usize,
    pub overlay_data_out: usize,
    pub preview_out: usize,
    /// 预览前缀不足 3·pw·ph(截断/全失败;7 张 DDLY 图前缀精确故为 false)。
    pub preview_truncated: bool,
}

/// 解析后的地图(API 形状冻结:parse + 只读字段)。
pub struct MapFile {
    pub basic: Option<BasicInfo>,
    pub map_meta: MapMeta,
    pub header: HeaderMeta,
    pub lighting: Lighting,
    pub digest: Option<[u8; 20]>,
    pub tiles: IsoGrid,
    pub overlays: OverlayLayers,
    pub preview: Option<preview::PreviewImage>,
    pub objects: MapObjects,
    pub logic_sections: Vec<LogicSection>,
    /// 首段前的孤儿键值行数(RMG MapUnit 半图;IniDocument 计数透传)。
    pub orphan_key_values: usize,
    pub pack_stats: PackStats,
}

/// 瓦片流上界的兜底常量(300×300 行走容量字节;研究 §8-3)。真图最大
/// 943,734 B(2+1=3.map,272×158)在此之下。
const TILE_STREAM_FALLBACK_CAP: usize = 11 * (2 * 300 - 1) * 300 + 4;

/// [Map]Size 的 (W,H):合法域 1..=4096;否则 (300,300) 兜底(研究 §8-3:
/// INI 不可用时以兜底上界放行,再由瓦片记录数上界与 %11 判定兜底正确性)。
fn map_size_wh(size: [i64; 4]) -> (u16, u16) {
    let w = size[2];
    let h = size[3];
    if (1..=4096).contains(&w) && (1..=4096).contains(&h) {
        (w as u16, h as u16)
    } else {
        (300, 300)
    }
}

/// 11·(2W−1)·H+4,u128 防溢出,超过兜底常量时钳到兜底。
fn tile_stream_bound(w: u16, h: u16) -> usize {
    let bound = (2u128 * w as u128 - 1) * h as u128 * 11 + 4;
    if bound > TILE_STREAM_FALLBACK_CAP as u128 {
        TILE_STREAM_FALLBACK_CAP
    } else {
        bound as usize
    }
}

/// [Preview]Size → (pw, ph):合法域 1..=8192 且 ≤ PREVIEW_MAX_PIXELS;
/// 缺失/畸形 → None(= 无预览)。
fn preview_dims(doc: &IniDocument) -> Option<(u32, u32)> {
    let s = doc.value("Preview", "Size")?;
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return None;
    }
    let pw = crate::format::ini::parse_int_prefix(parts[2])?;
    let ph = crate::format::ini::parse_int_prefix(parts[3])?;
    if !(1..=8192).contains(&pw) || !(1..=8192).contains(&ph) {
        return None;
    }
    if pw as u64 * ph as u64 > preview::PREVIEW_MAX_PIXELS {
        return None;
    }
    Some((pw as u32, ph as u32))
}

fn tokenize(doc: &IniDocument) -> Vec<LogicSection> {
    let mut out = Vec::new();
    for name in TOKENIZE_SECTIONS {
        if let Some(sec) = doc.section(name) {
            out.push(LogicSection {
                name: name.to_string(),
                entries: sec.entries.clone(),
            });
        }
    }
    out
}

impl MapFile {
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        let doc = IniDocument::parse(&mapdoc::decode_map_text(data));
        let orphan_key_values = doc.orphan_key_values;
        let map_meta = MapMeta::from_doc(&doc)?;
        let basic = BasicInfo::from_doc(&doc);
        let header = HeaderMeta::from_doc(&doc);
        let lighting = Lighting::from_doc(&doc);
        let digest = meta::parse_digest(&doc);
        let (w, h) = map_size_wh(map_meta.size);

        // IsoMapPack5:缺失 = 空平地(templateMap 形态,研究 §1/§8-1)
        let tiles_section = match doc.section("IsoMapPack5") {
            Some(sec) => Some(decode_pack_section(
                &sec.entries,
                PackCodec::Lzo1x,
                PackBound::Tiles {
                    max_out: tile_stream_bound(w, h),
                },
            )?),
            None => None,
        };
        let tiles = match &tiles_section {
            Some(s) => IsoGrid::parse(&s.data, w, h)?,
            None => IsoGrid::parse(&[], w, h)?,
        };
        let iso_out = tiles_section.as_ref().map_or(0, |s| s.data.len());

        // overlay 双段:恒 262144(807/807 全有;缺失即 Invalid)
        let overlay_section = doc
            .section("OverlayPack")
            .ok_or_else(|| FormatError::Invalid("缺少 [OverlayPack] 段".to_string()))?;
        let overlay_data_section = doc
            .section("OverlayDataPack")
            .ok_or_else(|| FormatError::Invalid("缺少 [OverlayDataPack] 段".to_string()))?;
        let overlay_out = decode_pack_section(
            &overlay_section.entries,
            PackCodec::Lcw,
            PackBound::Exact(overlay::OVERLAY_GRID_LEN),
        )?;
        let overlay_data_out = decode_pack_section(
            &overlay_data_section.entries,
            PackCodec::Lcw,
            PackBound::Exact(overlay::OVERLAY_GRID_LEN),
        )?;
        let overlays = OverlayLayers::parse(&overlay_out.data, &overlay_data_out.data)?;

        // PreviewPack:缺失或 [Preview] 缺失/畸形 = 无预览;
        // lenient 前缀 + "恰 3·pw·ph" 判定(研究 §5;7 张 DDLY 图前缀精确 → Some)
        let (preview, preview_out, preview_truncated) = match (doc.section("PreviewPack"), preview_dims(&doc)) {
            (Some(sec), Some((pw, ph))) => {
                let max_out = 3usize * pw as usize * ph as usize;
                let section = decode_pack_section(
                    &sec.entries,
                    PackCodec::Lzo1x,
                    PackBound::Preview { max_out },
                )?;
                let truncated = section.data.len() != max_out;
                (
                    preview::PreviewImage::parse(&section.data, pw, ph),
                    section.data.len(),
                    truncated,
                )
            }
            (Some(sec), None) => {
                // 有包段但无 [Preview] 尺寸:无法判界,按无预览处理(统计清零)
                let _ = sec;
                (None, 0, false)
            }
            (None, _) => (None, 0, false),
        };

        let objects = objects::parse_objects(&doc);
        let logic_sections = tokenize(&doc);

        Ok(MapFile {
            basic,
            map_meta,
            header,
            lighting,
            digest,
            tiles,
            overlays,
            preview,
            objects,
            logic_sections,
            orphan_key_values,
            pack_stats: PackStats {
                iso_out,
                overlay_out: overlay_out.data.len(),
                overlay_data_out: overlay_data_out.data.len(),
                preview_out,
                preview_truncated,
            },
        })
    }
}
```

- [ ] **Step 2: 写 11 个合成集成测试(map/mod.rs 测试模块)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // —— 测试专用微型编码器(与 pack.rs 测试同方案;测试代码不进公开 API)——
    fn lzo_literal_payload(data: &[u8]) -> Vec<u8> {
        assert!((1..=238).contains(&data.len()));
        let mut p = vec![(17 + data.len()) as u8];
        p.extend_from_slice(data);
        p.extend_from_slice(&[0x11, 0x00, 0x00]);
        p
    }

    fn push_lzo_chunk(blob: &mut Vec<u8>, data: &[u8]) {
        let payload = lzo_literal_payload(data);
        blob.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        blob.extend_from_slice(&(data.len() as u16).to_le_bytes());
        blob.extend_from_slice(&payload);
    }

    fn lzo_literal_stream(data: &[u8]) -> Vec<u8> {
        let mut blob = Vec::new();
        let (full, rest) = data.as_chunks::<238>();
        for chunk in full {
            push_lzo_chunk(&mut blob, chunk);
        }
        if !rest.is_empty() {
            push_lzo_chunk(&mut blob, rest);
        }
        blob
    }

    fn lcw_fill_stream(fill: u8, total: usize) -> Vec<u8> {
        let mut blob = Vec::new();
        let mut left = total;
        while left > 0 {
            let n = left.min(8192);
            blob.extend_from_slice(&4u16.to_le_bytes());
            blob.extend_from_slice(&(n as u16).to_le_bytes());
            blob.extend_from_slice(&[0xFE, n as u8, (n >> 8) as u8, fill]);
            left -= n;
        }
        blob
    }

    fn b64(bytes: &[u8]) -> String {
        const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for c in bytes.as_chunks::<3>().0 {
            let n = ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32;
            s.push(A[(n >> 18) as usize & 63] as char);
            s.push(A[(n >> 12) as usize & 63] as char);
            s.push(A[(n >> 6) as usize & 63] as char);
            s.push(A[n as usize & 63] as char);
        }
        match bytes.len() % 3 {
            1 => {
                let n = (bytes[bytes.len() - 1] as u32) << 16;
                s.push(A[(n >> 18) as usize & 63] as char);
                s.push(A[(n >> 12) as usize & 63] as char);
                s.push_str("==");
            }
            2 => {
                let n = ((bytes[bytes.len() - 2] as u32) << 16)
                    | ((bytes[bytes.len() - 1] as u32) << 8);
                s.push(A[(n >> 18) as usize & 63] as char);
                s.push(A[(n >> 12) as usize & 63] as char);
                s.push(A[(n >> 6) as usize & 63] as char);
                s.push('=');
            }
            _ => {}
        }
        s
    }

    fn entries_text(blob: &[u8]) -> Vec<(String, String)> {
        let text = b64(blob);
        let mut v = Vec::new();
        let mut i = 0usize;
        let mut k = 1usize;
        while i < text.len() {
            let n = (text.len() - i).min(70);
            v.push((k.to_string(), text[i..i + n].to_string()));
            i += n;
            k += 1;
        }
        v
    }

    fn push_pack_section(ini: &mut String, name: &str, blob: &[u8]) {
        ini.push_str(&format!("[{name}]\n"));
        for (k, v) in entries_text(blob) {
            ini.push_str(&k);
            ini.push('=');
            ini.push_str(&v);
            ini.push('\n');
        }
    }

    fn rec(x: u16, y: u16, tile: i16, sub: u8, z: u8) -> [u8; 11] {
        let mut b = [0u8; 11];
        b[0..2].copy_from_slice(&x.to_le_bytes());
        b[2..4].copy_from_slice(&y.to_le_bytes());
        b[4..6].copy_from_slice(&tile.to_le_bytes());
        b[8] = sub;
        b[9] = z;
        b
    }

    /// mini 地图:w=4,h=4;2 条瓦片记录 + 4 零尾;预览 2×1(6 B);
    /// overlay 全 FF/全 00;1 结构 + 1 地形(坐标与瓦片记录 2 重合);
    /// Waypoints 一条 + Actions 一条(tokenize 验证);Digest 一条。
    fn mini_map() -> Vec<u8> {
        let mut tiles = Vec::new();
        tiles.extend_from_slice(&rec(1, 1, 5, 0, 0));
        tiles.extend_from_slice(&rec(3, 2, 6, 2, 1));
        tiles.extend_from_slice(&[0, 0, 0, 0]);
        let preview: &[u8] = &[0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00];
        let mut ini = String::new();
        ini.push_str("[Basic]\nName=TestMap\nOfficial=yes\nMaxPlayer=4\n");
        ini.push_str("[Map]\nTheater=SNOW\nSize=0,0,4,4\nLocalSize=0,0,4,4\n");
        ini.push_str("[Header]\nWidth=3\nHeight=3\nNumberStartingPoints=2\n");
        ini.push_str("[Lighting]\nAmbient=1.000000\n");
        ini.push_str("[Preview]\nSize=0,0,2,1\n");
        push_pack_section(&mut ini, "PreviewPack", &lzo_literal_stream(preview));
        push_pack_section(&mut ini, "OverlayPack", &lcw_fill_stream(0xFF, 262144));
        push_pack_section(&mut ini, "OverlayDataPack", &lcw_fill_stream(0x00, 262144));
        push_pack_section(&mut ini, "IsoMapPack5", &lzo_literal_stream(&tiles));
        ini.push_str("[Structures]\n0=Neutral,CABHUT,256,2,3,0,None,0,0,1,0,0,None,None,None,0,0\n");
        ini.push_str("[Terrain]\n2003=TIBTRE01\n");
        ini.push_str("[Waypoints]\n0=2003\n");
        ini.push_str("[Actions]\n0c44c1dc=11,99,7,_Amb_OceanHeavy,0,0,0,0,AA,99,7\n");
        ini.push_str("[Digest]\n1=qSt1w6I0r+9EWkfWl7XFZMt0Vo4=\n");
        ini.into_bytes()
    }

    fn strip_section(data: &[u8], section: &str) -> Vec<u8> {
        // 从 mini 地图文本里删掉一个段(含其后的键值行)——敌意/缺失路径用
        let text = String::from_utf8(data.to_vec()).unwrap();
        let mut out = String::new();
        let mut skipping = false;
        for line in text.lines() {
            if line.starts_with('[') {
                skipping = line == format!("[{section}]");
            }
            if !skipping {
                out.push_str(line);
                out.push('\n');
            }
        }
        out.into_bytes()
    }

    #[test]
    fn parses_full_synthetic_map() {
        let m = MapFile::parse(&mini_map()).unwrap();
        // 元数据
        let basic = m.basic.as_ref().unwrap();
        assert_eq!(basic.name.as_deref(), Some("TestMap"));
        assert!(basic.official);
        assert_eq!(basic.max_player, 4);
        assert_eq!(m.map_meta.theater, meta::Theater::Snow);
        assert_eq!(m.map_meta.size, [0, 0, 4, 4]);
        assert_eq!(m.header.width, 3);
        assert_eq!(m.lighting.ambient, Some(1.0));
        // Digest:规格依据 §9 钉死的 20 字节(c1a01md 样本,与 meta.rs 测试同值)
        assert_eq!(
            m.digest,
            Some([
                0xA9, 0x2B, 0x75, 0xC3, 0xA2, 0x34, 0xAF, 0xEF, 0x44, 0x5A, 0x47, 0xD6, 0x97,
                0xB5, 0xC5, 0x64, 0xCB, 0x74, 0x56, 0x8E
            ])
        );
        // 瓦片:存储坐标权威 + 与对象坐标恒等(同菱形坐标系)
        assert_eq!(m.tiles.len(), 2);
        let t = m.tiles.cell(3, 2).unwrap();
        assert_eq!((t.tile, t.sub_tile, t.z), (6, 2, 1));
        let terrain = m
            .objects
            .all
            .iter()
            .find(|o| o.class == objects::ObjectClass::Terrain)
            .unwrap();
        assert_eq!(terrain.cell(), Some((3, 2)));
        assert!(m.tiles.cell(3, 2).is_some());
        assert!(m.tiles.cell(2, 3).is_none());
        // overlay:全 FF 类型 → frame 恒 None
        assert_eq!(m.overlays.type_at(2, 3), 0xFF);
        assert_eq!(m.overlays.frame_at(2, 3), None);
        // 预览:Some 且字节原样
        let img = m.preview.as_ref().unwrap();
        assert_eq!((img.width, img.height), (2, 1));
        assert_eq!(img.rgb, vec![0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00]);
        // 对象/逻辑/统计
        assert_eq!(m.objects.count(objects::ObjectClass::Structure), 1);
        assert_eq!(m.objects.count(objects::ObjectClass::Terrain), 1);
        assert_eq!(m.objects.malformed, 0);
        assert_eq!(m.logic_sections.len(), 2); // Waypoints + Actions
        let wp = m.logic_sections.iter().find(|s| s.name == "Waypoints").unwrap();
        assert_eq!(wp.entries, vec![("0".to_string(), "2003".to_string())]);
        assert_eq!(m.orphan_key_values, 0);
        assert_eq!(m.pack_stats.iso_out, 26);
        assert_eq!(m.pack_stats.overlay_out, 262144);
        assert_eq!(m.pack_stats.overlay_data_out, 262144);
        assert_eq!(m.pack_stats.preview_out, 6);
        assert!(!m.pack_stats.preview_truncated);
    }

    #[test]
    fn missing_isomapack5_is_empty_flat_map() {
        let m = MapFile::parse(&strip_section(&mini_map(), "IsoMapPack5")).unwrap();
        assert!(m.tiles.is_empty());
        assert_eq!(m.pack_stats.iso_out, 0);
    }

    #[test]
    fn missing_preview_pack_is_none() {
        let m = MapFile::parse(&strip_section(&mini_map(), "PreviewPack")).unwrap();
        assert!(m.preview.is_none());
        assert_eq!(m.pack_stats.preview_out, 0);
        assert!(!m.pack_stats.preview_truncated);
    }

    #[test]
    fn missing_digest_is_ok() {
        let m = MapFile::parse(&strip_section(&mini_map(), "Digest")).unwrap();
        assert!(m.digest.is_none());
    }

    #[test]
    fn missing_basic_is_ok() {
        let m = MapFile::parse(&strip_section(&mini_map(), "Basic")).unwrap();
        assert!(m.basic.is_none());
    }

    #[test]
    fn hostile_truncated_base64_iso_is_err() {
        // IsoMapPack5 的 base64 中段坏字符:严格段必须 Err(截断中段敌意样例)
        let mut data = mini_map();
        let text = String::from_utf8(data.clone()).unwrap();
        let mut out = String::new();
        let mut in_iso = false;
        let mut patched = false;
        for line in text.lines() {
            if line.starts_with('[') {
                in_iso = line == "[IsoMapPack5]";
            } else if in_iso && !patched && line.contains('=') && line.len() > 4 {
                let (k, _) = line.split_once('=').unwrap();
                out.push_str(&format!("{k}=QU?JD\n")); // 坏字符 '?'
                patched = true;
                continue;
            }
            out.push_str(line);
            out.push('\n');
        }
        data = out.into_bytes();
        assert!(matches!(MapFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn hostile_overlay_sum_lie_is_err() {
        // OverlayPack 只有半量块(Σ 131072 ≠ 262144)→ Invalid
        let mut ini = String::new();
        ini.push_str("[Map]\nTheater=SNOW\nSize=0,0,4,4\n");
        ini.push_str("[Preview]\nSize=0,0,2,1\n");
        push_pack_section(&mut ini, "OverlayPack", &lcw_fill_stream(0xFF, 131072));
        push_pack_section(&mut ini, "OverlayDataPack", &lcw_fill_stream(0x00, 262144));
        push_pack_section(&mut ini, "IsoMapPack5", &lzo_literal_stream(&rec(1, 1, 5, 0, 0)));
        assert!(matches!(MapFile::parse(&ini.into_bytes()), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn hostile_overlay_bad_char_is_err() {
        let mut ini = String::from("[Map]\nTheater=SNOW\nSize=0,0,4,4\n[OverlayPack]\n1=QU?JD\n");
        let blob = lcw_fill_stream(0xFF, 262144);
        for (k, v) in entries_text(&blob).iter().skip(1) {
            ini.push_str(&format!("{k}={v}\n"));
        }
        ini.push_str("[OverlayDataPack]\n");
        for (k, v) in entries_text(&lcw_fill_stream(0x00, 262144)) {
            ini.push_str(&format!("{k}={v}\n"));
        }
        ini.push_str("[IsoMapPack5]\n");
        for (k, v) in entries_text(&lzo_literal_stream(&rec(1, 1, 5, 0, 0))) {
            ini.push_str(&format!("{k}={v}\n"));
        }
        assert!(matches!(MapFile::parse(&ini.into_bytes()), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn hostile_truncated_preview_degrades_to_none() {
        // 前缀 3 字节 < 3·2·1 = 6:Ok + preview None + truncated true
        let mut ini = String::new();
        ini.push_str("[Map]\nTheater=SNOW\nSize=0,0,4,4\n");
        ini.push_str("[Preview]\nSize=0,0,2,1\n");
        push_pack_section(&mut ini, "PreviewPack", &lzo_literal_stream(&[1, 2, 3]));
        push_pack_section(&mut ini, "OverlayPack", &lcw_fill_stream(0xFF, 262144));
        push_pack_section(&mut ini, "OverlayDataPack", &lcw_fill_stream(0x00, 262144));
        push_pack_section(&mut ini, "IsoMapPack5", &lzo_literal_stream(&rec(1, 1, 5, 0, 0)));
        let m = MapFile::parse(&ini.into_bytes()).unwrap();
        assert!(m.preview.is_none());
        assert_eq!(m.pack_stats.preview_out, 3);
        assert!(m.pack_stats.preview_truncated);
    }

    #[test]
    fn hostile_iso_over_capacity_is_err() {
        // Size=1,1(容量 1)但 2 条记录 → 瓦片层 Invalid(敌意记录超界)
        let mut tiles = Vec::new();
        tiles.extend_from_slice(&rec(1, 1, 5, 0, 0));
        tiles.extend_from_slice(&rec(1, 1, 6, 0, 0));
        let mut ini = String::new();
        ini.push_str("[Map]\nTheater=SNOW\nSize=0,0,1,1\n");
        ini.push_str("[Preview]\nSize=0,0,2,1\n");
        push_pack_section(&mut ini, "OverlayPack", &lcw_fill_stream(0xFF, 262144));
        push_pack_section(&mut ini, "OverlayDataPack", &lcw_fill_stream(0x00, 262144));
        push_pack_section(&mut ini, "IsoMapPack5", &lzo_literal_stream(&tiles));
        assert!(matches!(MapFile::parse(&ini.into_bytes()), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn logic_sections_tokenized_verbatim() {
        // Actions 的 8 字段链行原样保留(tokenize-only);缺失段不出现
        let m = MapFile::parse(&mini_map()).unwrap();
        let actions = m.logic_sections.iter().find(|s| s.name == "Actions").unwrap();
        assert_eq!(
            actions.entries[0].1,
            "11,99,7,_Amb_OceanHeavy,0,0,0,0,AA,99,7"
        );
        assert!(m.logic_sections.iter().all(|s| s.name != "Tags"));
    }

    #[test]
    fn rmg_orphan_lines_are_counted() {
        // 孤儿键值行(RMG 半图形态):继续丢弃但计数透传
        let mut data = mini_map();
        let mut text = String::from_utf8(data).unwrap();
        text.insert_str(0, "Size=0,0,60,30\n1=AAAA\n2=BBBB\n");
        data = text.into_bytes();
        let m = MapFile::parse(&data).unwrap();
        assert_eq!(m.orphan_key_values, 3);
    }
}
```

- [ ] **Step 3: 运行合成集成测试通过**

Run: `cargo test -p ra2-core map`
Expected: map 模块族全绿 68 passed(gbk 4 + mapdoc 3 + pack 17 + iso 10 + overlay 5 + preview 4 + meta 5 + objects 9 + mod 11;过滤词 "map" 命中全部 map:: 路径)

- [ ] **Step 4: 追加 3 个 ignored 测试(crates/core/tests/map_samples.rs)**

在 Task 3 创建的文件末尾追加(文件头已有 assets_dir/read_map 助手):

```rust
/// 递归收集 RA2_ASSETS_DIR 下全部 .map(807 张;普查 §1 的语料构成)。
fn map_files() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![assets_dir()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("读目录 {}: {e}", dir.display()));
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

/// 剧场交叉验证(spec §6-5):SNOW 图的最大 tile 索引 < snow.mix +
/// SNOWMD.MIX 的 TMP 瓦片总数(2026-11-04 实测 343 文件/6804 瓦片 +
/// 81 文件/3212 瓦片 = 10016 → 上界 10015;语料实测最大 963)。
#[test]
#[ignore]
fn snow_tile_index_within_theater_mix_capacity() {
    use ra2_core::format::mix::MixArchive;
    fn tmp_tile_capacity(mix: &MixArchive) -> usize {
        let mut total = 0usize;
        for e in mix.entries() {
            let b = mix.read_by_id(e.id).unwrap();
            if b.len() >= 8 && b[0] == 0 && b[1] == 0 {
                let tiles = b[6] as usize | ((b[7] as usize) << 8);
                if 0 < tiles && tiles < 4096 {
                    total += tiles;
                }
            }
        }
        total
    }
    let ra2 = MixArchive::parse(std::fs::read(assets_dir().join("ra2.mix")).unwrap()).unwrap();
    let snow = MixArchive::parse(ra2.read("SNOW.MIX").unwrap().to_vec()).unwrap();
    let ra2md = MixArchive::parse(std::fs::read(assets_dir().join("ra2md.mix")).unwrap()).unwrap();
    let snowmd = MixArchive::parse(ra2md.read("SNOWMD.MIX").unwrap().to_vec()).unwrap();
    let capacity = tmp_tile_capacity(&snow) + tmp_tile_capacity(&snowmd);
    assert_eq!(capacity, 10016);

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
    assert!(snow_maps >= 100, "Maps/** 下应有 106 张 SNOW 图,实得 {snow_maps}");
    assert!(max_tile >= 0, "至少一张 SNOW 图应有瓦片");
    assert!(
        (max_tile as i64) < capacity as i64,
        "tile {max_tile} 超过剧场瓦片容量 {capacity}"
    );
    assert_eq!(max_tile, 963, "语料实测最大 SNOW tile(交流意见.map)");
}

/// 截断 PreviewPack 组(规格依据 §8):7 张 DDLY 图在完整位图后带异常尾块,
/// 干净前缀恰为 3·pw·ph → 全部 Ok 且预览 Some(2026-11-04 逐图复核);
/// templateMap.map(无 IsoMapPack5)预览前缀 0 → None。
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
    let m = MapFile::parse(&read_map("Resources/RandomMapGenerator_RA2/templateMap.map")).unwrap();
    assert!(m.tiles.is_empty());
    assert!(m.preview.is_none());
    assert!(m.pack_stats.preview_truncated);
}
```

- [ ] **Step 5: 运行真实资产测试**

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test map_samples -- --ignored`
Expected: 4 passed(1 + 3;ignored 25/27 进度)。若 corpus_walk 失败:按输出首张图名定位——容错规则已覆盖全部已知语料异常,失败必是实现缺陷或语料变动,先复现再修。

- [ ] **Step 6: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 合成 180 passed

```bash
git add crates/core/src/format/map/mod.rs crates/core/tests/map_samples.rs
git commit -m "feat(core): MapFile 总入口(四包段装配/logic tokenize/元数据)+ 807 图全量遍历" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 10: ra2-map CLI(info / extract-tile + 纯 std PNG)

**Files:**
- Modify: `Cargo.toml`(workspace `members` 追加 `"tools/ra2-map"`)
- Create: `tools/ra2-map/Cargo.toml`
- Create: `tools/ra2-map/src/main.rs`
- Create: `tools/ra2-map/src/png.rs`
- Create: `tools/ra2-map/tests/cli.rs`

**Interfaces:**
- Consumes: `MapFile`(Task 9)、`Theater::iso_palette_file`(Task 7)、`Palette`(M1b)、`MixArchive::read_by_id`(M1b)、`crc32`(M1b)
- Produces:
  - 退出码契约(与 ra2-mix 相同):0 成功;1 找不到文件/解析失败/写失败/资源缺失(含 YR 三剧场调色板缺席、无瓦片格、缺 assets);2 用法错误(含非 UTF-8 参数、坐标非整数);usage 裁决先于任何文件 IO;stdout 断管 → 0
  - `ra2-map info <file.map>` 输出(逐行 `key=value`,锁定格式):`name= / theater= / size=WxH / tiles= / structures= units= infantry= aircraft= terrain= smudge= / malformed= / packs: iso= overlay= overlay_data= preview= preview_truncated= / logic_sections= / players= / digest=`
  - `ra2-map extract-tile <file.map> <x> <y> -o <file.png> [--assets <游戏目录>]`:取该格瓦片记录,解析剧场 ISO 调色板(ra2.mix → id 0x3B5A96DE 嵌套库),写 60×30 菱形**占位渲染** PNG(填充色 = 调色板第 `tile%256` 项,菱形外黑;tile→TMP 位图归 M3,`--help` 注明)
  - `png::encode_png(width: u32, height: u32, rgb: &[u8]) -> Vec<u8>`、`png::write_png_file(path, width, height, rgb) -> std::io::Result<()>`(签名 + IHDR(8bit RGB)+ 单 IDAT(zlib store + Adler32)+ IEND;CRC32 复用 core `crc32`)
- 语义要点(spec §7,勿改):PNG 最小合法即可,不用图像库;`--assets` 缺省取 `RA2_ASSETS_DIR`;坐标经 i64 解析(非整数 → 2),负/超界/无记录/tile=-1 → 1。

- [ ] **Step 1: 写失败测试(`tools/ra2-map/tests/cli.rs` 完整文件;此时 crate 尚不存在,TDD 以 Step 3 的编译/运行确认)**

```rust
// SPDX-License-Identifier: GPL-3.0-only

use std::process::Command;

fn bin() -> std::path::PathBuf {
    env!("CARGO_BIN_EXE_ra2-map").into()
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("ra2-map-cli-{tag}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

// —— 微型编码器(与 core map::tests 同方案;测试复制,不导出)——
fn lzo_literal_payload(data: &[u8]) -> Vec<u8> {
    assert!((1..=238).contains(&data.len()));
    let mut p = vec![(17 + data.len()) as u8];
    p.extend_from_slice(data);
    p.extend_from_slice(&[0x11, 0x00, 0x00]);
    p
}

fn push_lzo_chunk(blob: &mut Vec<u8>, data: &[u8]) {
    let payload = lzo_literal_payload(data);
    blob.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    blob.extend_from_slice(&(data.len() as u16).to_le_bytes());
    blob.extend_from_slice(&payload);
}

fn lzo_literal_stream(data: &[u8]) -> Vec<u8> {
    let mut blob = Vec::new();
    let (full, rest) = data.as_chunks::<238>();
    for chunk in full {
        push_lzo_chunk(&mut blob, chunk);
    }
    if !rest.is_empty() {
        push_lzo_chunk(&mut blob, rest);
    }
    blob
}

fn lcw_fill_stream(fill: u8, total: usize) -> Vec<u8> {
    let mut blob = Vec::new();
    let mut left = total;
    while left > 0 {
        let n = left.min(8192);
        blob.extend_from_slice(&4u16.to_le_bytes());
        blob.extend_from_slice(&(n as u16).to_le_bytes());
        blob.extend_from_slice(&[0xFE, n as u8, (n >> 8) as u8, fill]);
        left -= n;
    }
    blob
}

fn b64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in bytes.as_chunks::<3>().0 {
        let n = ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32;
        s.push(A[(n >> 18) as usize & 63] as char);
        s.push(A[(n >> 12) as usize & 63] as char);
        s.push(A[(n >> 6) as usize & 63] as char);
        s.push(A[n as usize & 63] as char);
    }
    match bytes.len() % 3 {
        1 => {
            let n = (bytes[bytes.len() - 1] as u32) << 16;
            s.push(A[(n >> 18) as usize & 63] as char);
            s.push(A[(n >> 12) as usize & 63] as char);
            s.push_str("==");
        }
        2 => {
            let n = ((bytes[bytes.len() - 2] as u32) << 16) | ((bytes[bytes.len() - 1] as u32) << 8);
            s.push(A[(n >> 18) as usize & 63] as char);
            s.push(A[(n >> 12) as usize & 63] as char);
            s.push(A[(n >> 6) as usize & 63] as char);
            s.push('=');
        }
        _ => {}
    }
    s
}

fn push_pack_section(ini: &mut String, name: &str, blob: &[u8]) {
    ini.push_str(&format!("[{name}]\n"));
    let text = b64(blob);
    let mut i = 0usize;
    let mut k = 1usize;
    while i < text.len() {
        let n = (text.len() - i).min(70);
        ini.push_str(&format!("{k}={}\n", &text[i..i + n]));
        i += n;
        k += 1;
    }
}

fn rec(x: u16, y: u16, tile: i16, sub: u8, z: u8) -> [u8; 11] {
    let mut b = [0u8; 11];
    b[0..2].copy_from_slice(&x.to_le_bytes());
    b[2..4].copy_from_slice(&y.to_le_bytes());
    b[4..6].copy_from_slice(&tile.to_le_bytes());
    b[8] = sub;
    b[9] = z;
    b
}

/// 与 core 测试同一张 mini 地图(SNOW 4×4,2 瓦片,1 结构 1 地形,预览 2×1)。
fn mini_map() -> Vec<u8> {
    let mut tiles = Vec::new();
    tiles.extend_from_slice(&rec(1, 1, 5, 0, 0));
    tiles.extend_from_slice(&rec(3, 2, 6, 2, 1));
    tiles.extend_from_slice(&[0, 0, 0, 0]);
    let mut ini = String::new();
    ini.push_str("[Basic]\nName=TestMap\nOfficial=yes\nMaxPlayer=4\n");
    ini.push_str("[Map]\nTheater=SNOW\nSize=0,0,4,4\nLocalSize=0,0,4,4\n");
    ini.push_str("[Header]\nWidth=3\nHeight=3\nNumberStartingPoints=2\n");
    ini.push_str("[Lighting]\nAmbient=1.000000\n");
    ini.push_str("[Preview]\nSize=0,0,2,1\n");
    push_pack_section(&mut ini, "PreviewPack", &lzo_literal_stream(&[0xFF, 0, 0, 0, 0xFF, 0]));
    push_pack_section(&mut ini, "OverlayPack", &lcw_fill_stream(0xFF, 262144));
    push_pack_section(&mut ini, "OverlayDataPack", &lcw_fill_stream(0x00, 262144));
    push_pack_section(&mut ini, "IsoMapPack5", &lzo_literal_stream(&tiles));
    ini.push_str("[Structures]\n0=Neutral,CABHUT,256,2,3,0,None,0,0,1,0,0,None,None,None,0,0\n");
    ini.push_str("[Terrain]\n2003=TIBTRE01\n");
    ini.push_str("[Waypoints]\n0=2003\n");
    ini.push_str("[Digest]\n1=qSt1w6I0r+9EWkfWl7XFZMt0Vo4=\n");
    ini.into_bytes()
}

#[test]
fn usage_errors_exit_2_before_io() {
    // 路径故意不可读:退出码 2 证明 usage 裁决先于文件 IO
    for args in [
        vec![],
        vec!["frobnicate", "Z:/不存在/nope.map"],
        vec!["info"],
        vec!["extract-tile", "Z:/不存在/nope.map", "1", "1"],
        vec!["extract-tile", "Z:/不存在/nope.map", "x", "1", "-o", "-"],
    ] {
        let out = Command::new(bin()).args(&args).output().unwrap();
        assert_eq!(out.status.code(), Some(2), "args={args:?}");
    }
}

#[test]
fn non_utf8_arg_exits_2_not_panic() {
    #[cfg(windows)]
    let bad = {
        use std::os::windows::ffi::OsStringExt;
        std::ffi::OsString::from_wide(&[0xD800, 0x0069])
    };
    #[cfg(unix)]
    let bad = {
        use std::os::unix::ffi::OsStringExt;
        std::ffi::OsString::from_vec(vec![0xFF, b'i'])
    };
    let out = Command::new(bin()).arg(bad).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn info_on_synthetic_map() {
    let dir = temp_dir("info");
    let p = dir.join("t.map");
    std::fs::write(&p, mini_map()).unwrap();
    let out = Command::new(bin())
        .args(["info", p.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("name=TestMap"), "{stdout}");
    assert!(stdout.contains("theater=SNOW"));
    assert!(stdout.contains("size=4x4"));
    assert!(stdout.contains("tiles=2"));
    assert!(stdout.contains("structures=1"));
    assert!(stdout.contains("terrain=1"));
    assert!(stdout.contains("packs: iso=26 overlay=262144 overlay_data=262144 preview=6"));
    assert!(stdout.contains("logic_sections=1"));
    assert!(stdout.contains("players=4"));
    assert!(stdout.contains("digest=yes"));
}

#[test]
fn info_failures_exit_1() {
    let dir = temp_dir("infofail");
    let missing = Command::new(bin())
        .args(["info", dir.join("nope.map").to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(1));
    let bad = dir.join("bad.map");
    std::fs::write(&bad, b"hello world\n"); // 无 [Map] 段 → Invalid
    let out = Command::new(bin())
        .args(["info", bad.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn extract_tile_without_assets_exits_1() {
    // 无 --assets 且进程内移除 RA2_ASSETS_DIR → 缺资源退出 1,不 panic
    let dir = temp_dir("extract");
    let p = dir.join("t.map");
    std::fs::write(&p, mini_map()).unwrap();
    let out_path = dir.join("tile.png");
    let mut cmd = Command::new(bin());
    cmd.args(["extract-tile", p.to_str().unwrap(), "1", "1", "-o", out_path.to_str().unwrap()]);
    cmd.env_remove("RA2_ASSETS_DIR");
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(!out_path.exists());
}

/// 真图活体验收(spec §6-7):TEMPERATE 图 extract-tile 产出可解析的
/// 60×30 PNG;info 对三张不同剧场图全部成功(info 不需要调色板)。
#[test]
#[ignore]
fn real_maps_info_and_extract_tile() {
    let assets = std::env::var("RA2_ASSETS_DIR").expect("需要 RA2_ASSETS_DIR");
    let dir = temp_dir("real");
    // 1) extract-tile:c1a01md (87,1) → PNG(签名/IEND/IHDR 尺寸)
    let map = std::path::Path::new(&assets).join("Maps/Cooperative/c1a01md.map");
    let out_path = dir.join("tile.png");
    let out = Command::new(bin())
        .args(["extract-tile", map.to_str().unwrap(), "87", "1", "-o", out_path.to_str().unwrap()])
        .env("RA2_ASSETS_DIR", &assets)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let png = std::fs::read(&out_path).unwrap();
    assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    assert_eq!(png[16..20], 60u32.to_be_bytes());
    assert_eq!(png[20..24], 30u32.to_be_bytes());
    let n = png.len();
    assert_eq!(&png[n - 8..n - 4], b"IEND");
    // 2) info 三剧场冒烟
    for (rel, theater) in [
        ("Maps/Cooperative/c1a01md.map", "TEMPERATE"),
        ("Maps/Cooperative/c1a03md.map", "SNOW"),
        ("Maps/Cooperative/c2s02md.map", "DESERT"),
    ] {
        let m = std::path::Path::new(&assets).join(rel);
        let out = Command::new(bin())
            .args(["info", m.to_str().unwrap()])
            .env("RA2_ASSETS_DIR", &assets)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "{rel}");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains(&format!("theater={theater}")), "{rel}: {stdout}");
    }
}
```

注:mini_map 的 `logic_sections=1`(仅 Waypoints——本测试版 mini 地图未含 [Actions],与 core 测试的 2 不同,勿混抄)。

- [ ] **Step 2: workspace 与 crate 清单**

根 `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/core", "tools/ra2-mix", "tools/ra2-map"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "GPL-3.0-only"

[profile.release]
lto = true
```

`tools/ra2-map/Cargo.toml`:

```toml
[package]
name = "ra2-map"
description = "Map parsing command-line tool (info/extract-tile)"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "ra2-map"
path = "src/main.rs"

[dependencies]
ra2-core = { path = "../../crates/core" }
```

- [ ] **Step 3: 写实现(`src/png.rs` 与 `src/main.rs` 完整文件)**

`tools/ra2-map/src/png.rs`:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! 最小合法 PNG 编码器(纯 std;spec §7 的工具侧要求)。
//!
//! 结构:8 字节签名 + IHDR(8bit/RGB 彩色类型 2/无隔行)+ 单 IDAT
//! (zlib store:0x78 0x01 + deflate 存储块 + Adler32)+ IEND。
//! CRC32 复用 `ra2_core::format::crc::crc32`(zlib 多项式,PNG 同款)。

use ra2_core::format::crc::crc32;
use std::io::Write;

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

/// adler32(zlib 流校验尾)。
fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// 编码 24bpp RGB 图(width×height,行主序,长度恒 3·w·h)。
/// `rgb` 长度由调用方构造保证(程序不变量,assert 属编程错误而非输入错误)。
pub fn encode_png(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    assert_eq!(rgb.len(), 3 * width as usize * height as usize, "RGB 长度不符");
    let mut out = Vec::with_capacity(rgb.len() + 1024);
    out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    // bitdepth 8 / 彩色类型 2(RGB)/ deflate / 过滤 0 / 无隔行
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    // 原始扫描流:每行前置 1 字节过滤类型 0(None)
    let stride = 3 * width as usize;
    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    for y in 0..height as usize {
        raw.push(0);
        raw.extend_from_slice(&rgb[y * stride..(y + 1) * stride]);
    }
    // zlib store:头 + ≤65535 字节的存储块 + adler32
    let mut idat = vec![0x78, 0x01];
    let mut pos = 0usize;
    while pos < raw.len() {
        let n = (raw.len() - pos).min(65535);
        let last = pos + n >= raw.len();
        idat.push(if last { 1 } else { 0 });
        idat.extend_from_slice(&(n as u16).to_le_bytes());
        idat.extend_from_slice(&(!(n as u16)).to_le_bytes());
        idat.extend_from_slice(&raw[pos..pos + n]);
        pos += n;
    }
    idat.extend_from_slice(&adler32(&raw).to_be_bytes());
    chunk(&mut out, b"IDAT", &idat);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// 编码并落盘(失败即 IO 错误,由 CLI 转退出码 1)。
pub fn write_png_file(
    path: &std::path::Path,
    width: u32,
    height: u32,
    rgb: &[u8],
) -> std::io::Result<()> {
    let bytes = encode_png(width, height, rgb);
    let mut f = std::fs::File::create(path)?;
    f.write_all(&bytes)?;
    f.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_and_ihdr_for_2x1() {
        let png = encode_png(2, 1, &[10, 20, 30, 40, 50, 60]);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(&png[8..12], &13u32.to_be_bytes());
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[16..29], &[0, 0, 0, 2, 0, 0, 0, 1, 8, 2, 0, 0, 0]);
        let crc_input: Vec<u8> = [b"IHDR".as_slice(), &png[16..29]].concat();
        assert_eq!(&png[29..33], &crc32(&crc_input).to_be_bytes());
    }

    #[test]
    fn zlib_store_block_layout_for_2x1() {
        // raw = 2 行 ×(1 过滤字节 + 3 RGB)= 8 字节 → 单个 last 块
        let raw_expected = [0u8, 10, 20, 30, 0, 40, 50, 60];
        let png = encode_png(2, 1, &[10, 20, 30, 40, 50, 60]);
        // IDAT:签名 8 + IHDR 块 25 = 33;len = 2(头)+5(块头)+8+4(adler)= 19
        assert_eq!(&png[33..37], &19u32.to_be_bytes());
        assert_eq!(&png[37..41], b"IDAT");
        let idat = &png[41..41 + 19];
        assert_eq!(&idat[..2], &[0x78, 0x01]);
        assert_eq!(&idat[2..7], &[0x01, 0x08, 0x00, 0xF8, 0xFF]); // last + LEN/NLEN
        assert_eq!(&idat[7..15], &raw_expected);
        assert_eq!(&idat[15..19], &adler32(&raw_expected).to_be_bytes());
    }

    #[test]
    fn iend_tail_and_adler_known_vector() {
        // adler32 公开校验向量("abc")
        assert_eq!(adler32(b"abc"), 0x024D_0127);
        let png = encode_png(1, 1, &[1, 2, 3]);
        let n = png.len();
        assert_eq!(&png[n - 12..n - 8], &0u32.to_be_bytes());
        assert_eq!(&png[n - 8..n - 4], b"IEND");
    }
}
```

`tools/ra2-map/src/main.rs`:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! ra2-map:地图解析命令行工具(info / extract-tile)。
//!
//! 退出码契约与 ra2-mix 相同:0 成功;1 找不到文件/解析失败/写失败/资源
//! 缺失;2 用法错误(含非 UTF-8 参数、坐标非整数)。usage 裁决先于任何
//! 文件 IO;stdout 断管(BrokenPipe)视为成功。

use ra2_core::format::map::meta::Theater;
use ra2_core::format::map::objects::ObjectClass;
use ra2_core::format::map::MapFile;
use ra2_core::format::mix::MixArchive;
use ra2_core::format::pal::Palette;
use std::io::Write;
use std::process::ExitCode;

mod png;

/// ra2.mix 内嵌调色板库的条目 ID(研究 §7.2;库本身按名可读)。
const PALETTE_MIX_ID: u32 = 0x3B5A_96DE;

fn usage() -> ! {
    eprintln!("用法: ra2-map info <file.map>");
    eprintln!("      ra2-map extract-tile <file.map> <x> <y> -o <file.png> [--assets <游戏目录>]");
    eprintln!("说明: extract-tile 输出该格 60×30 菱形占位渲染(填充色 = 剧场 ISO");
    eprintln!("      调色板第 tile%256 项);tile→TMP 真位图解析归 M3。缺 [--assets]");
    eprintln!("      时取环境变量 RA2_ASSETS_DIR。DESERT/NEWURBAN/LUNAR 的 YR 调色板");
    eprintln!("      不在本机安装中,将报缺资源(退出码 1)。");
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

fn theater_name(t: Theater) -> &'static str {
    match t {
        Theater::Temperate => "TEMPERATE",
        Theater::Snow => "SNOW",
        Theater::Urban => "URBAN",
        Theater::Desert => "DESERT",
        Theater::NewUrban => "NEWURBAN",
        Theater::Lunar => "LUNAR",
    }
}

fn run_info(map_path: &str) -> ExitCode {
    let data = match std::fs::read(map_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("读取 {map_path} 失败: {e}");
            return ExitCode::from(1);
        }
    };
    let m = match MapFile::parse(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("解析失败: {e}");
            return ExitCode::from(1);
        }
    };
    let name = m
        .basic
        .as_ref()
        .and_then(|b| b.name.clone())
        .unwrap_or_else(|| "-".to_string());
    let s = m.pack_stats;
    let mut out = String::new();
    out.push_str(&format!("name={name}\n"));
    out.push_str(&format!("theater={}\n", theater_name(m.map_meta.theater)));
    out.push_str(&format!("size={}x{}\n", m.map_meta.size[2], m.map_meta.size[3]));
    out.push_str(&format!("tiles={}\n", m.tiles.len()));
    out.push_str(&format!(
        "structures={} units={} infantry={} aircraft={} terrain={} smudge={}\n",
        m.objects.count(ObjectClass::Structure),
        m.objects.count(ObjectClass::Unit),
        m.objects.count(ObjectClass::Infantry),
        m.objects.count(ObjectClass::Aircraft),
        m.objects.count(ObjectClass::Terrain),
        m.objects.count(ObjectClass::Smudge),
    ));
    out.push_str(&format!("malformed={}\n", m.objects.malformed));
    out.push_str(&format!(
        "packs: iso={} overlay={} overlay_data={} preview={} preview_truncated={}\n",
        s.iso_out, s.overlay_out, s.overlay_data_out, s.preview_out, s.preview_truncated
    ));
    out.push_str(&format!("logic_sections={}\n", m.logic_sections.len()));
    out.push_str(&format!(
        "players={}\n",
        m.basic.as_ref().map(|b| b.max_player).unwrap_or(0)
    ));
    out.push_str(&format!(
        "digest={}\n",
        if m.digest.is_some() { "yes" } else { "no" }
    ));
    write_stdout(out.as_bytes())
}

/// 剧场 ISO 调色板:ra2.mix → id 0x3B5A96DE 嵌套库 → <theater>.pal(768 B)。
fn load_theater_palette(assets: &std::path::Path, theater: Theater) -> Result<Palette, String> {
    let ra2 = MixArchive::parse(
        std::fs::read(assets.join("ra2.mix")).map_err(|e| format!("读 ra2.mix: {e}"))?,
    )
    .map_err(|e| format!("解析 ra2.mix: {e}"))?;
    let blob = ra2
        .read_by_id(PALETTE_MIX_ID)
        .map_err(|_| "ra2.mix 内找不到调色板库(id 3B5A96DE)".to_string())?
        .to_vec();
    let nested = MixArchive::parse(blob).map_err(|e| format!("解析调色板库: {e}"))?;
    let pal_bytes = nested
        .read(theater.iso_palette_file())
        .map_err(|e| {
            format!(
                "调色板库内读 {}: {e}(YR 三剧场调色板不在本机安装)",
                theater.iso_palette_file()
            )
        })?
        .to_vec();
    Palette::parse(&pal_bytes).map_err(|e| format!("调色板 768B 校验失败: {e}"))
}

fn run_extract_tile(
    map_path: &str,
    x: i64,
    y: i64,
    out_path: &str,
    assets: Option<&str>,
) -> ExitCode {
    let data = match std::fs::read(map_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("读取 {map_path} 失败: {e}");
            return ExitCode::from(1);
        }
    };
    let m = match MapFile::parse(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("解析失败: {e}");
            return ExitCode::from(1);
        }
    };
    // 坐标 → u16(负/超界按无记录处理,不 panic)
    let tile = match (usize::try_from(x), usize::try_from(y)) {
        (Ok(x), Ok(y)) if x <= 65535 && y <= 65535 => m.tiles.cell(x as u16, y as u16),
        _ => None,
    };
    let Some(tile) = tile else {
        eprintln!("格 ({x},{y}) 无瓦片记录");
        return ExitCode::from(1);
    };
    if tile.tile < 0 {
        eprintln!("格 ({x},{y}) 是无瓦片格(tile=-1)");
        return ExitCode::from(1);
    }
    let assets_dir = assets
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var("RA2_ASSETS_DIR").ok().map(std::path::PathBuf::from));
    let Some(assets_dir) = assets_dir else {
        eprintln!("需要 --assets 或环境变量 RA2_ASSETS_DIR 指向游戏目录");
        return ExitCode::from(1);
    };
    let palette = match load_theater_palette(&assets_dir, m.map_meta.theater) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("解析剧场调色板失败: {e}");
            return ExitCode::from(1);
        }
    };
    // 60×30 菱形占位渲染:填充 = palette[tile%256];菱形外黑(0,0,0)。
    // 菱形谓词 |2cx-59| + 2·|2cy-29| ≤ 60(中心 (29.5,14.5),确定性、可复现)
    let idx = tile.tile as usize % 256;
    let [r, g, b] = palette.rgb[idx];
    let (w, h) = (60u32, 30u32);
    let mut rgb = vec![0u8; 3 * w as usize * h as usize];
    for cy in 0..h as usize {
        for cx in 0..w as usize {
            let inside = (2 * cx as u32).abs_diff(59) + 2 * (2 * cy as u32).abs_diff(29) <= 60;
            if inside {
                let i = cy * 3 * w as usize + cx * 3;
                rgb[i] = r;
                rgb[i + 1] = g;
                rgb[i + 2] = b;
            }
        }
    }
    match png::write_png_file(std::path::Path::new(out_path), w, h, &rgb) {
        Ok(()) => {
            eprintln!("已写 {out_path}(60×30 占位渲染;tile {} → 调色板项 {idx})", tile.tile);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("写 {out_path} 失败: {e}");
            ExitCode::from(1)
        }
    }
}

fn main() -> ExitCode {
    // std::env::args() 遇非 UTF-8 参数会 panic;args_os + usage(退出 2)
    let mut args = Vec::new();
    for a in std::env::args_os().skip(1) {
        match a.into_string() {
            Ok(s) => args.push(s),
            Err(_) => usage(),
        }
    }
    // 先裁决用法,再触碰文件 IO
    enum Action<'a> {
        Info(&'a str),
        ExtractTile {
            map: &'a str,
            x: i64,
            y: i64,
            out: &'a str,
            assets: Option<&'a str>,
        },
    }
    let action = match args.as_slice() {
        ["info", map] => Action::Info(map),
        ["extract-tile", map, x, y, flag, out, rest @ ..] if flag == "-o" => {
            let Ok(x) = x.parse::<i64>() else {
                usage();
            };
            let Ok(y) = y.parse::<i64>() else {
                usage();
            };
            let assets = match rest {
                [] => None,
                ["--assets", dir] => Some(*dir),
                _ => usage(),
            };
            Action::ExtractTile {
                map,
                x,
                y,
                out,
                assets,
            }
        }
        _ => usage(),
    };
    match action {
        Action::Info(map) => run_info(map),
        Action::ExtractTile {
            map,
            x,
            y,
            out,
            assets,
        } => run_extract_tile(map, x, y, out, assets),
    }
}
```

- [ ] **Step 4: 运行全部测试通过**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p ra2-map && cargo test -p ra2-mix`
Expected: ra2-map 8 passed(4 cli 合成 + 3 png + 忽略 1 未跑);ra2-mix 6 passed(既有零影响)

- [ ] **Step 5: BrokenPipe 手工验收(无法稳定自动化;Git Bash)**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo run -q -p ra2-map -- info "E:\hongjing\Reunion 2023\Maps\Cooperative\c1a01md.map" | head -3; echo "exit=${PIPESTATUS[0]}"`
Expected: 打印 name/theater/size 三行后 `exit=0`

- [ ] **Step 6: 真图活体验收**

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-map -- --ignored`
Expected: 1 passed(ignored 26/27 进度)

- [ ] **Step 7: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 合成 188 passed

```bash
git add Cargo.toml tools/ra2-map/
git commit -m "feat(tools): ra2-map CLI(info/extract-tile,纯 std PNG,退出码契约同 ra2-mix)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 11: M2 里程碑验收(代表性真图甲骨文 + README + 终账)

**Files:**
- Create: `crates/core/tests/m2_acceptance.rs`
- Modify: `README.md`(在"格式层(M1b)"小节后追加"地图层(M2)"小节)

**Interfaces:**
- Consumes: `MapFile` 全量(Task 9)、`ObjectClass`(Task 8)、`Theater`(Task 7)
- Produces: `#[ignore]` 验收测试 `m2_representative_maps_full_oracle`——18 张代表图逐张钉死剧场/尺寸/记录数/六档对象计数/malformed/预览形态/孤儿行/摘要;spec §6 全项核对;README 记录里程碑能力。

- [ ] **Step 1: 写 `crates/core/tests/m2_acceptance.rs`(完整文件)**

数据来源:研究 §13 指纹 + 2026-11-04 计划作者 oracle 扫描(probe_oracle.py,逐图实测;规格依据 §7-§9),**每行常量都已执行期复核**:

```rust
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
    Oracle { rel: "Maps/Cooperative/c1a01md.map", theater: Theater::Temperate, w: 87, h: 71, recs: 12283, structures: 65, units: 10, infantry: 14, aircraft: 0, terrain: 749, smudge: 0, malformed: 0, preview: Some((162, 59, 28674)), name: None, orphans: 0 },
    Oracle { rel: "Maps/Cooperative/c1a03md.map", theater: Theater::Snow, w: 110, h: 80, recs: 17520, structures: 48, units: 0, infantry: 0, aircraft: 0, terrain: 1227, smudge: 0, malformed: 0, preview: Some((208, 69, 43056)), name: None, orphans: 0 },
    Oracle { rel: "Maps/Cooperative/c2s02md.map", theater: Theater::Desert, w: 90, h: 103, recs: 18437, structures: 54, units: 2, infantry: 9, aircraft: 0, terrain: 500, smudge: 0, malformed: 0, preview: Some((170, 92, 46920)), name: None, orphans: 0 },
    Oracle { rel: "Maps/Cooperative/c1a02md.map", theater: Theater::NewUrban, w: 105, h: 110, recs: 22990, structures: 255, units: 56, infantry: 44, aircraft: 3, terrain: 1154, smudge: 0, malformed: 0, preview: Some((200, 97, 58200)), name: None, orphans: 0 },
    Oracle { rel: "Maps/Standard/moonpatr.map", theater: Theater::Lunar, w: 60, h: 62, recs: 7378, structures: 11, units: 0, infantry: 0, aircraft: 0, terrain: 0, smudge: 0, malformed: 0, preview: Some((111, 52, 17316)), name: None, orphans: 0 },
    // TS 代官方:记录数 < (2W−1)·H(缺 void 格,§3.6)
    Oracle { rel: "Maps/Standard/riverram.map", theater: Theater::Temperate, w: 52, h: 61, recs: 6255, structures: 0, units: 0, infantry: 0, aircraft: 0, terrain: 128, smudge: 0, malformed: 0, preview: Some((95, 49, 13965)), name: None, orphans: 0 },
    Oracle { rel: "Maps/Standard/frstbite.map", theater: Theater::Snow, w: 100, h: 100, recs: 19898, structures: 18, units: 0, infantry: 0, aircraft: 0, terrain: 190, smudge: 0, malformed: 0, preview: Some((188, 87, 49068)), name: None, orphans: 0 },
    Oracle { rel: "Maps/Standard/hillbtwn.map", theater: Theater::Urban, w: 60, h: 68, recs: 8081, structures: 129, units: 76, infantry: 0, aircraft: 0, terrain: 266, smudge: 0, malformed: 0, preview: Some((111, 57, 18981)), name: None, orphans: 0 },
    Oracle { rel: "Maps/Standard/isleofoades.map", theater: Theater::Temperate, w: 98, h: 85, recs: 16557, structures: 8, units: 0, infantry: 0, aircraft: 0, terrain: 358, smudge: 0, malformed: 0, preview: Some((186, 75, 41850)), name: None, orphans: 0 },
    // —— 语料极值/社区/中文图名
    Oracle { rel: "Maps/MadHQ/2+1=3.map", theater: Theater::Temperate, w: 272, h: 158, recs: 85794, structures: 35, units: 0, infantry: 0, aircraft: 0, terrain: 836, smudge: 0, malformed: 0, preview: Some((544, 158, 257856)), name: Some("Trinity (2-8)"), orphans: 0 },
    Oracle { rel: "Maps/MadHQ/避暑山庄.map", theater: Theater::Temperate, w: 399, h: 100, recs: 79700, structures: 650, units: 40, infantry: 93, aircraft: 0, terrain: 3883, smudge: 0, malformed: 0, preview: Some((798, 100, 239400)), name: None, orphans: 0 },
    Oracle { rel: "Maps/MadHQ/陋室空堂.map", theater: Theater::Temperate, w: 200, h: 200, recs: 79800, structures: 589, units: 28, infantry: 0, aircraft: 0, terrain: 12562, smudge: 122, malformed: 0, preview: Some((400, 200, 240000)), name: None, orphans: 0 },
    Oracle { rel: "Maps/DDLY/城郊之战.map", theater: Theater::Urban, w: 90, h: 90, recs: 16110, structures: 285, units: 0, infantry: 0, aircraft: 0, terrain: 486, smudge: 0, malformed: 0, preview: Some((180, 90, 48600)), name: Some("No name"), orphans: 0 },
    Oracle { rel: "Maps/DDLY/终极海战防守图.map", theater: Theater::Temperate, w: 130, h: 80, recs: 20720, structures: 201, units: 1, infantry: 0, aircraft: 0, terrain: 0, smudge: 0, malformed: 0, preview: Some((260, 80, 62400)), name: None, orphans: 0 },
    // —— 真实损坏行
    Oracle { rel: "Maps/MadHQ/勇士与巨龙.map", theater: Theater::Temperate, w: 137, h: 140, recs: 38220, structures: 160, units: 0, infantry: 0, aircraft: 0, terrain: 697, smudge: 270, malformed: 1, preview: Some((274, 140, 115080)), name: Some("Weathered Alliance (2-7)"), orphans: 0 },
    // —— RMG 产物
    Oracle { rel: "Resources/RandomMapGenerator_RA2/MapUnits/NEWURBAN/1,1,1,1,01,spawn.map", theater: Theater::NewUrban, w: 30, h: 30, recs: 1770, structures: 1, units: 0, infantry: 0, aircraft: 0, terrain: 9, smudge: 0, malformed: 0, preview: Some((60, 30, 5400)), name: Some("No name"), orphans: 44 },
    // —— 退化/缺段样例
    Oracle { rel: "Resources/RandomMapGenerator_RA2/MapUnits/DESERT/indicator.map", theater: Theater::Desert, w: 1, h: 1, recs: 1, structures: 0, units: 0, infantry: 0, aircraft: 0, terrain: 0, smudge: 0, malformed: 0, preview: Some((2, 1, 6)), name: Some("No name"), orphans: 0 },
    Oracle { rel: "Resources/RandomMapGenerator_RA2/templateMap.map", theater: Theater::Temperate, w: 50, h: 50, recs: 0, structures: 0, units: 0, infantry: 0, aircraft: 0, terrain: 0, smudge: 0, malformed: 0, preview: None, name: Some("Random Map"), orphans: 0 },
];

#[test]
#[ignore]
fn m2_representative_maps_full_oracle() {
    for o in ORACLES {
        let m = MapFile::parse(&read_map(o.rel))
            .unwrap_or_else(|e| panic!("{}: {e:?}", o.rel));
        assert_eq!(m.map_meta.theater, o.theater, "{}", o.rel);
        assert_eq!((m.map_meta.size[2], m.map_meta.size[3]), (o.w, o.h), "{}", o.rel);
        assert_eq!(m.tiles.len(), o.recs, "{}", o.rel);
        assert_eq!(m.objects.count(ObjectClass::Structure), o.structures, "{}", o.rel);
        assert_eq!(m.objects.count(ObjectClass::Unit), o.units, "{}", o.rel);
        assert_eq!(m.objects.count(ObjectClass::Infantry), o.infantry, "{}", o.rel);
        assert_eq!(m.objects.count(ObjectClass::Aircraft), o.aircraft, "{}", o.rel);
        assert_eq!(m.objects.count(ObjectClass::Terrain), o.terrain, "{}", o.rel);
        assert_eq!(m.objects.count(ObjectClass::Smudge), o.smudge, "{}", o.rel);
        assert_eq!(m.objects.malformed, o.malformed, "{}", o.rel);
        match o.preview {
            Some((pw, ph, bytes)) => {
                let img = m.preview.as_ref().unwrap_or_else(|| panic!("{} 应有预览", o.rel));
                assert_eq!((img.width, img.height), (pw, ph), "{}", o.rel);
                assert_eq!(img.rgb.len(), bytes, "{}", o.rel);
            }
            None => assert!(m.preview.is_none(), "{} 应无预览", o.rel),
        }
        if let Some(n) = o.name {
            assert_eq!(m.basic.as_ref().unwrap().name.as_deref(), Some(n), "{}", o.rel);
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
```

- [ ] **Step 2: 运行验收测试**

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test m2_acceptance -- --ignored`
Expected: 1 passed

- [ ] **Step 3: README 追加地图层小节(插在 `## ra2-mix` 之前)**

```markdown
## 地图层(M2)

`ra2-core::format::map` 现可解析任意用户自有 `.map`:四个包段
(IsoMapPack5/PreviewPack = LZO1X,Overlay 双段 = LCW)全解码,
瓦片网格/双层覆盖/预览位图/六类对象段结构化,logic 段 tokenize 保留,
[Digest] 读取不校验;807 张真实地图全量解析零 panic。`tools/ra2-map`
提供 `info` 与 `extract-tile`(60×30 菱形占位渲染;tile→TMP 真位图
解析归 M3)。规格依据见 `docs/superpowers/research/`,实施计划见
`docs/superpowers/plans/`。
```

- [ ] **Step 4: 终账(计划级完成定义核对)**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 全绿;**合成 188 passed**

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored`
Expected: **27 passed**(既有 21 + 本计划 6:map_samples 4 + ra2-map cli 1 + m2_acceptance 1)

Run: `git ls-files | grep -iE "\.(map|mix|pal|shp|tmp|csf|vxl|hva|aud)$" || echo "clean"`
Expected: `clean`(仓库无 EA 资产)

- [ ] **Step 5: Commit**

```bash
git add crates/core/tests/m2_acceptance.rs README.md
git commit -m "test(core): M2 里程碑验收(18 图字节级甲骨文 + 坐标恒等 + README 地图层)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

## 完成定义(计划级验收)

1. `cargo test --workspace` 全绿:**188 个合成测试**(79 既有 core + 95 新 core + 6 既有 cli + 5 新 cli + 3 png);`RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored` 全绿:**27 个真实资产测试**(既有 21 + map_samples 4 + ra2-map cli 1 + m2_acceptance 1)。
2. `cargo fmt --all --check` 与 `cargo clippy --all-targets -- -D warnings` 零告警。
3. spec §6 七项验收全落地:合成边界/敌意覆盖(每模块)、18 图字节级甲骨文、807 图全量零 panic(corpus_walk)、敌意变异集(每段族 ≥1 例:pack 8 例/iso 5 例/MapFile 级 5 例)、剧场交叉验证(snow tile 上界 10016)、fmt/clippy/GPL/零依赖、CLI 活体验收(三剧场 info + extract-tile PNG)。
4. 对任意敌意输入零 panic(含 debug 溢出检查):宽算术全部 u128/checked。
5. 仓库不含任何 EA 资产(`fixtures/` 已忽略;`git ls-files` 审计通过)。

## 附录:Self-Review(计划自查,2026-11-04)

**spec 覆盖检查**(对照设计文档 §3/§4/§6 与两份研究文档):

- §3 做清单:[Basic]/[Map]/[Lighting]/[Header] 结构化 → Task 7 meta ✓;IsoMapPack5 → Task 5 ✓;Overlay 双段 → Task 6 ✓;PreviewPack → Task 7 preview ✓;六对象段([Smudge] 正名按普查)→ Task 8 ✓;logic 段 tokenize(spec 清单 + AITriggerTypes 同族 + CellTags)→ Task 9 ✓;[Digest] 读取不校验 → Task 7/9 ✓;剧场交叉验证 → Task 9 map_samples ✓
- §3 不做清单:渲染/M3 tile→TMP(extract-tile 占位渲染并注明)、地图写入、rules 语义、get_float——全部按 Global Constraints 排除 ✓
- §4 模块结构:lzo1x.rs 平级 lcw ✓;pack/mapdoc/iso/overlay/preview/objects/mod 与 spec §4 一致;gbk.rs/gbk_table.rs 是"mapdoc 承担 GBK 容错"的物理拆分(生成表 190 KB 级,不入 mapdoc),meta.rs 是元数据段承载(allowing mod.rs 只做总装)——两处增量已在此声明,不改变 API 形状 ✓
- §4 决策复核:编号 key 数值序 ✓;4 字节块头 ✓;codec 按段名分派 ✓;Overlay 恒 512×512 ✓;Preview 裸 RGB ✓;存储 (x,y) 权威 ✓;陈旧 data 忽略 ✓;7 张截断 PreviewPack 前缀保留 ✓(2026-11-04 复核升级:前缀全部恰为 3·pw·ph → 全部产出 Some 预览)
- §5 零 panic/内存预检:size_in==0/越界、size_out==0/>8192(preview 例外)、Σ 预检(overlay 恒 262144 / 瓦片 ≤ 11·(2W−1)·H+4 且 %11∈{0,4} / 预览 ≤ 3·pw·ph 且累加钳制)、W+H 语料极值修正为 499(原文档 898 不成立,已在规格依据 §2 勘误)、allocation 上界先于分配 ✓
- §6 验收七项:1→各任务单测;2→Task 11 十八图;3→Task 9 corpus_walk(807,逐张 Err 即失败,打印首张);4→敌意变异集(pack 8 + iso 5 + MapFile 5,覆盖截断/坏 base64/尺寸谎报 ×4 段族);5→Task 9 snow 交叉验证;6→各任务 gate;7→Task 10 活体验收 ✓
- §7 CLI 契约:info 输出项(名称/玩家/剧场/尺寸/瓦片数/对象分档/四包段字节/logic 段数)与 extract-tile(--assets/RA2_ASSETS_DIR、剧场链调色板、最小合法 PNG)→ Task 10 ✓;退出码 0/1/2 与 ra2-mix 相同 ✓

**placeholder 扫描**:全文代码块无 `todo!`/`unimplemented!`/`...` 省略/"同 Task N"引用;每个测试与实现均为完整可编译文件或完整替换段;测试常量全部来自研究文档或本计划规格依据 §1-§12 的取证记录(2026-11-04 对真机资产逐一复核,LZO 移植经 18 手造向量 + 5 图 ×4 段全链指纹 + Python 双实现对拍三重锚定)✓

**跨任务类型一致性检查**:

- `lzo1x_decode(src, out_cap, out) -> Result<usize, FormatError>`:Task 1 定义,Task 4(pack)/测试一致 ✓
- `lcw_decode_consumed(src, out) -> Result<usize, FormatError>`:Task 4 定义并即用;`lcw_decode` 语义不变(M1b shp/tmp 零影响)✓
- `PackCodec/PackBound/PackSection/decode_pack_section`:Task 4 定义,Task 9 全按此调用 ✓
- `IsoGrid::parse(data, w, h)` / `cell(x, y)` / `max_tile_index()`:Task 5 定义,Task 9/10/11 一致 ✓
- `OverlayLayers::parse(types, frames)` / `type_at` / `frame_at`:Task 6 定义,Task 9/10 一致 ✓
- `PreviewImage::parse(data, pw, ph) -> Option`:Task 7 定义,Task 9 一致 ✓
- `Theater`/`BasicInfo`/`MapMeta`/`HeaderMeta`/`Lighting`/`parse_digest`:Task 7 定义,Task 9/10/11 一致 ✓
- `MapObject`/`MapObjects::count`/`parse_objects`:Task 8 定义,Task 9/10/11 一致 ✓
- `MapFile` 字段集:Task 9 定义,Task 10(info 读 basic/map_meta/tiles/objects/pack_stats/digest/logic_sections)与 Task 11(全部字段)一致 ✓
- `Theater::parse` 未知值返回 `Unsupported`、`MapMeta::from_doc` 缺段返回 `Invalid`:Task 7 定义,Task 9 透传、Task 10 转 exit 1 一致 ✓
- 计数核对:合成 188 = 79 既有 core + (19+7+7+18+10+5+9+9+11) 新 core + 6 既有 cli + 8 新 tools;ignored 27 = 21 既有 + map_samples 4 + ra2-map 1 + m2_acceptance 1——与各任务 Run 步骤的期望值逐一相符 ✓

**已知风险与执行提示**:

- LZO1X 是本计划唯一"从规范语义重写"的算法:Task 1 的 19 个测试(18 手造 + 1 真流)必须全绿;若真流向量失败,先用只读脚本对 moonpatr.map 的 PreviewPack 块 2 重提取(规格依据 §3 的提取路径),**不得放宽断言**。
- gbk_table.rs 依赖执行期 Python(标准库 gbk 编解码器):生成后由 Task 3 的钉字节单测验证;若实现环境无 Python,须先解决工具链再继续,不得手写表。
- corpus_walk 断言 `total == 807` 与 `max_tile == 963`:若游戏目录变动导致失配,按 M1b 惯例——只读脚本重测、更新常量并在 commit message 注明,不得静默放宽。
- clippy 新近 deny lints:计划代码已按 `as_chunks`/迭代器风格预写;若个别循环报 `needless_range_loop`,改写为 `zip/enumerate`,不得 `#[allow]`。
- 测试文件间的微型编码器(b64/lzo_literal_stream/lcw_fill_stream)在 core 测试与 cli 测试各有一份拷贝——这是"测试代码不进公开 API"的有意取舍,勿"优化"成共享模块。
- 城郊之战/终极海战防守图等 GBK 文件名在 Git Bash 命令行中直接可用(Rust fs 按 UTF-16 API 访问);若实现者终端显示乱码,不影响测试(常量在源文件内)。


