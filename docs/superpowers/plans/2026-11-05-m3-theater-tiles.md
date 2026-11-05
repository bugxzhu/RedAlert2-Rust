# M3 剧场瓦片→TMP 映射(tile_id→INI 表→mix 查找→TS TMP→菱形像素)实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 M2 地图层之上闭合瓦片解析链:`(theater, tile_id, sub_tile) → 剧场 INI 表 → 文件名 → MIX 按名查找(含递归嵌套)→ TS TMP 槽位 → RAW 菱形像素 → 剧场调色板 → PNG`,并清偿三项历史债:mix.rs 幻影 digest 错位(研究 §2)、tmp.rs 的 M1b 误判(实为 TS SHP,研究 §4)、T9 容量测试失真(10016→964/INI 语义)。

**Architecture:** 延续 cargo workspace。`ra2-core` 新增 `format/theater_tmp.rs`(TS TMP 解析器)、`format/theater.rs`(TheaterIni 顺序探查 + 剧场常数表 + TheaterAssets 引擎挂载序装载器);`format/tmp.rs` 更名重定域为 `format/shp_ts.rs`(TS SHP 帧格式);`format/mix.rs` 摘要位引擎语义修复(从不跳 20,REA 勘误)。`tools/ra2-map` 的 extract-tile 从 60×30 占位升级为真瓦片渲染。工作分支 `feat/m3-theater-tiles`(已检出)。

**Tech Stack:** Rust stable(edition 2021),零第三方依赖,Git Bash / Windows。

## 任务总表(SDD 脚本索引)

| Task | 一句话 |
|---|---|
| Task 1 | mix.rs 摘要位引擎语义修复(从不跳 20 + 旧格式守卫;REA 勘误,sha1.rs 作废)+ 46 容器语料 ignored 测试 |
| Task 2 | `tmp.rs` → `shp_ts.rs` 更名重定域(字段重标 + RAW 回退)+ M1b/format_samples 测试迁移 |
| Task 3 | 新建 `theater_tmp.rs`:TS TMP 解析器(16B 头/槽表/52B 头/RAW 菱形/z,敌意守卫零 panic) |
| Task 4 | 新建 `theater.rs` 前半:TheaterIni(顺序探查/atoi/blank 保留槽/LastTilesInSet)+ 剧场常数表 |
| Task 5 | `theater.rs` 后半:TheaterAssets 装载器(引擎挂载序 + 递归嵌套回退层,首中即胜) |
| Task 6 | T9 重钉:map_samples snow 容量测试改 INI 语义(964)并改名 |
| Task 7 | CLI extract-tile 真瓦片渲染(INI 反查 + 槽位 + 菱形铺canvas + 调色板 + PNG) |
| Task 8 | M3 里程碑验收(807 图闭合 + 缺文件槽精确账 + 全量零 panic + 甲骨文锚点 + README) |

## Global Constraints

- 许可证:GPL-3.0-only。XCC(GPL,Olaf van der Spek)派生的结构/算法(TS TMP 52B 头、SHP 帧头)**语义重写并注明出处**,不逐行翻译;已泄漏 TS/RA2 引擎源码是红线,绝不参考(本计划全部事实来自研究文档 + 语料探针 + REA 裁决)。
- 每个文件第一行 `// SPDX-License-Identifier: GPL-3.0-only`。
- 零第三方依赖(原计划的自写 SHA-1 已随 REA 勘误作废——引擎不对 mix 数据做哈希,判别不再需要;见 Task 1 勘误块)。
- 仓库不提交任何 EA 资产;`fixtures/` 已 gitignore;真实资产测试用 `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023"`,`#[ignore]` 标注。
- 零 panic:所有解析器切片/索引用 ByteReader 或显式边界检查;宽算术显式宽类型/checked(debug 溢出检查下也不 panic)。敌意输入(截断/谎报/越界)一律 Err,分配前先用输入长度推导上界。
- 错误三变体语义不变:`UnexpectedEof` = 截断;`Unsupported` = 合法但本版本不支持(含 LastTilesInSet 引擎规则死路);`Invalid` = 畸形。
- 零 IO 于解析器:所有 parse 接受字节切片;IO 收口在 `TheaterAssets::open`(库边界的装载器,spec §4 明文)与测试/CLI。核心解析器无 HashMap/HashSet(装载器索引用排序 Vec + 二分;测试代码可用 BTreeMap)。
- 计数基线(2026-11-05 实测):`cargo test --workspace` = **192 合成**(core 178 + ra2-map 主程序单测 3 + ra2-map cli 5 + ra2-mix cli 6);`-- --ignored` = **27**(format_samples 13 + m1b 1 + m2 1 + map_samples 4 + mix_integration 7 + ra2-map cli 1)。每任务给出"新增 N 个"并全绿;M2 教训:不链式维护逐任务累计值,终账见 Task 8(合成 **226** / ignored **35**;Task 1 勘误后 228→226;原计划 T5 行有 218+6=224 笔误,勘误时一并校正)。
- 每任务收尾:`export PATH="$HOME/.cargo/bin:$PATH" && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace` 全绿后 commit;涉真资产的任务另跑 ignored 全绿。clippy 新 deny lint 用迭代器/`as_chunks` 风格预避免,不得 `#[allow]`。
- commit 用 conventional commits,每条末尾附 `Co-Authored-By: Claude Code <noreply@anthropic.com>`(第二个 `-m` 传入)。
- **REA 并行抽查(非本计划任务)**:gamemd.exe 抽查与实现并行进行;其第一批结论已作为"REA 勘误(2026-11-05)"并入本计划(Task 4 顺序探查/LastTilesInSet、Task 5 挂载序、剧场常数表出处 0x7E1B78)。后续 REA 发现与本计划冲突时,由控制器发勘误,实现者不自行改判定。
- 本计划不做(spec §3 边界):整图渲染/wgpu(M4)、高度 z 着色与 terrain/ramp 语义枚举(字段暴露原始字节)、overlay/smock 渲染、rules 语义、地图写回、TS SHP remap/阴影语义、非 2:1 画布的菱形渲染(语料 100% 为 2:1,见 依据 §6;越界即明确退出 1)。

## 依据(计划编写日 2026-11-05 对真机资产的只读探针复核;实现者不需要再考证)

以下常量均已由计划作者在本机资产(`E:\hongjing\Reunion 2023`,只读 python -I 探针)逐项复核,冲突时以本节为准:

1. **测试计数基线**:合成 192 / ignored 27(上表分解;`cargo test --workspace` 复跑确认)。
2. **mix 幻影 digest(研究 §2 的工程落地依据)**:递归遍历 7 个顶层 mix(ares/expandmd01/langmd/language/ra2/ra2md/thememd,深度 ≤4)共 **46 个可解析容器**;27 个 flags=0x00010000,19 个 flags=0。27 个摘要位容器**全部**满足:`min(entry.offset)==0`、`body_size == len − (10+12·count) − 20`、`max(offset+size) ≤ len − (10+12·count) − 20`——即**在头字段层面与"真摘要"容器结构不可分辨**(三种判别候选——offset 基准、body_size、跳 20 版面可行性——全部证伪)。XCC `mix_file.cpp` 的 `is_valid`(2026-11-05 取自 github.com/OlafvdSpek/xcc,GPL)只做尺寸一致性校验(`4+6+12·count+size+(checksum?20)==filesize`),对这 27 个容器同样误判为"有摘要"。~~结论:唯一可靠的判别 = 内容校验(20 字节窗口 == SHA-1(索引表))~~ **已被 REA 勘误推翻(见 Task 1 勘误块)**:引擎从不跳 20、从不哈希 mix 数据,位为咨询位——上述结构不可分性恰恰因为根本不存在两种形态。46 容器/27+19 口径不变,语义改为位记账。控制器简报中的"47 容器"与本计划遍历定义差 1(verify2 记账口径不同);本计划测试钉 46,遍历定义见 Task 1(顶层 = 目录下全部 `*.mix`,含 langmd/thememd/language/ares)。
3. **剧场 INI(REA 勘误后语义)**:六份 md INI 均在 ra2md.mix→localmd.mix(urbannmd.ini 另有 expandmd01.mix 版):段名 `[TileSet0000]` 起**四位零填充、自 0 连续无缺口**(temperatmd 0000..0081/82 段、urbanmd 0000..0110/111、snowmd 0000..0082/83、desertmd 0000..0081/82、lunarmd 0000..0084/85、urbannmd(localmd) 0000..0121/122、urbannmd(expandmd01) 0000..0125/126)。`TilesInSet` atoi 前缀容量复算:**838 / 1081 / 964 / 726 / 198 / 1175(localmd) / 1189(expandmd01)**;`TilesInSet=o` 脏数据仅在 localmd 版 urbannmd 的 set0105。`LastTilesInSet` 在全部 8 份 INI(含 base 的 snow.ini)**零出现**(与 REA verify6 一致)。base `snow.ini`(ra2.mix→local.mix,22,603B)容量 **798** < 964(md 替换实证)。
4. **甲骨文锚点复核(所在容器)**:`clear01.sno`→ra2.mix/isosnow(1,872B);`burnt01.sno`→ra2.mix/snow.mix(520B,帧 0 offset=56);`lobrdb08.sno`→snow.mix(帧 1 offset=152);`clifxl04.sno`→ra2md.mix/**isosnomd**;`sfbrdg01.ubn`→ra2md.mix/**isoubnmd**;`ariz01.tem`→ra2.mix/**isotemp**;`Crevas06.lun`→ra2md.mix/**isolunmd**。四锚点全部按 mix_name_id 跨容器可达。
5. **TS TMP 画布普查**:全部剧场容器内 13,020 个 TS TMP 形态 blob 的 (cbx,cby,cx,cy) 直方图**全部 cx=60、cy=30**(cbx·cby 网格任意)——CLI 菱形渲染的 `cx == 2·cy` 前置在语料上 100% 成立。
6. **调色板现状**:ra2.mix 内调色板库(id `0x3B5A96DE`,44 项,带 LMD 43 名)只有 `isotem.pal`/`isosno.pal`/`isourb.pal`;`isodes.pal`/`isoubn.pal`/`isolun.pal` **在全语料按名 + 全部 LMD 名单中均不存在**(候选名 19 个逐一探测为零)。因此 spec §6-6 的"snow/newurban/desert 各一图活体验收"按 **TEMPERATE/SNOW/URBAN 三剧场出 PNG + DESERT 负向退出 1** 落地(M2 契约延续;此为对 spec 的偏差登记,控制器复核后可勘误)。
7. ~~SHA-1 测试向量~~(**作废**,Task 1 勘误;留档勿删):`sha1("abc")=a9993e364706816aba3e25717850c26c9cd0d89d`;`sha1("")=da39a3ee5e6b4b0d3255bfef95601890afd80709`;`sha1("a"×56)=c2db330f6083854c99d4b5bfb6e8f29f201be699`;`sha1("a"×64)=0098ba824b5c16427bd7a1122a5a442a25ec644d`;`sha1("a"×119)=ee971065aaa017e0632a8ca6c77bb3bf8b1dfc56`。
8. **expandmd01 匿名嵌套 mix**:id `0xC31B4D61`,1,018 项(内含 323 个 snow 瓦片,研究 §3-5),flags=0x00010000(幻影族)——递归下钻是**必需路径**。

## 规格歧义裁决(计划作者已定;控制器复核可推翻→勘误)

| # | 歧义 | 裁决 |
|---|---|---|
| 1 | spec §6-6 要求 snow/newurban/desert 活体 PNG,但三张 YR 调色板实测缺席(依据 §6) | 活体 PNG 用 TEMPERATE/SNOW/URBAN;DESERT 图 extract-tile 退出 1(stderr 含"调色板"),作为负向活体检查 |
| 2 | mix digest 判别规则(spec §4 留待"XCC 对照定稿") | ~~SHA-1(索引表)校验~~ → **勘误 2026-11-05(REA 二轮)**:引擎从不跳 20(位只存不读,0x5B3C20 全反汇编),规则=镜像引擎:从不跳;旧格式 word@0≠0 → Unsupported。见 Task 1 勘误块 |
| 3 | 研究 §5 只列到 +0x28(40B),52B 头剩 12B 无字段名 | 40..52 暴露为 `tail: [u8; 12]` 原样字节,不校验 |
| 4 | 47 vs 46 容器 | 钉 46,遍历定义写入测试注释(Task 1) |
| 5 | MadHQ 残余界 | 白名单 ⊆ `Maps/MadHQ/`、违规图 ≤4、sub 越界格总数 ≤500(宽裕防增长界;首跑实测值记入测试注释) |
| 6 | 非 2:1 画布渲染 | TsTmpFile 解析器尺寸无关;CLI 渲染要求 `cx == 2·cy`,否则退出 1(依据 §5:语料全 2:1) |
| 7 | z 数据定位(研究 §5 只有 z_ofs 字段) | 按连续布局读:z 在 slot+52+cx·cy/2(clear01.sno 16+4+52+900+900=1872 精确闭合);`z_ofs` 字段原样暴露不校验 |
| 8 | TheaterIni 返回类型 | REA 勘误引入 LastTilesInSet 引擎规则 → `parse` 返回 `Result<TheaterIni, FormatError>`(mismatch → `Unsupported`);spec §5 的"畸形键宽容"对其余键不变 |

---

### Task 1: mix.rs 摘要位引擎语义修复(从不跳 20)+ 旧格式守卫 + 46 容器语料测试

> **勘误 2026-11-05(控制器,REA digest 追问裁决)——本块推翻下文所有 SHA-1 内容,以本块为准:**
>
> REA 二轮对 gamemd.exe MixFileClass ctor(0x5B3C20)全量反汇编裁决:
> ①引擎对 flags 位 0x00010000 **只读一次、存 +0x10、此后从不读**——构造器与
> 全部 mix 子系统(0x5B3E90..0x5B6300 操作数扫描)无任何 +20 跳过;data_start
> **无条件** = 10+12·count(0x5B3DD2 Seek 取当前位 + 零偏移);②引擎**从不**对
> mix 数据跑 SHA-1(变换例程仅用于启动自完整性校验/网络,调用图枚举封闭);
> ③位 0x00020000 置位时头+索引进 0x633xxx 流(非 Blowfish,身份未解;语料零
> 样本);④**旧格式判别**:首个 dword 低 16 位 ≠ 0 → 6 字节头(count u16@0,
> size u32@2,data_start=6+12·count,TS1 时代;语料零样本)。
>
> **结论:SHA-1(索引表)内容校验规则作废**——"幻影 digest"的真相是**引擎根本
> 不认摘要块,位是工具侧咨询位**;本计划依据 §2 的"唯一可靠判别"前提(存在
> 需要判别的两种形态)不成立。替换规则(引擎镜像):
> - 新格式(word@0==0):data_start = 10+12·count,**从不因摘要位跳 20**;
> - 旧格式(word@0≠0):`FormatError::Unsupported`(引擎支持但语料零样本,
>   本里程碑不实现;显式拒绝防静默错解);
> - 0x00020000:维持现行拒绝(Unsupported)。
>
> **交付面修订**:删除 `sha1.rs` 全部内容(Step 1 作废;零依赖原则下不留
> 无用途代码);`MixDigestState`/`digest_state()` 改为 `digest_flag(&self) -> bool`
> (咨询位透传,文档注明引擎从不消费);Step 3 三个测试换为:
> 1. `digest_flag_does_not_skip_twenty`(TDD 红:0x10000 置位 + 索引后 20B 垃圾
>    窗口,read 仍从 10+12·n 读到正确数据;旧实现此处必红)——保留下方
>    `sample_phantom_mix` 样本,断言改为 `data_start()==10+12`、`read` 正确、
>    `digest_flag()==true`;
> 2. `old_format_header_rejected`(首 dword 低 16 位 ≠ 0 → Unsupported;合成
>    6 字节头样本);
> 3. `plain_mix_reports_no_digest_flag`(flags=0:`digest_flag()==false`)。
> Step 5 语料测试语义重钉:46 容器全绿口径不变,但 match 三态改二态
> (27 置位/19 未置位),`panic!("语料不应出现真摘要容器")` 一句删除,
> clear01.sno 1872 内容锚不变。Step 4 TDD 红流程不变。Step 6 计数改:
> 合成 **194**(192 − 删旧 1 + 新 3);ignored 28 不变。计划级终账
> 228→**226**/35。XCC 出处注释改写:mix.rs 文档注明"摘要位为工具侧咨询位,
> 引擎行为经 REA 0x5B3C20 反汇编裁决(gamemd YR 1.001),XCC 的 checksum
> 概念不进入本项目判定路径"。
>
> **下文残留 SHA-1 字样一并作废**(Files 列表的 `Create: sha1.rs` 与
> `pub mod sha1;`、Interfaces 的 `sha1()`/`MixDigestState`/`digest_state()`
> 三行、Step 7 提交信息模板):Step 7 的 commit message 改为
> `fix(core): MIX 摘要位改引擎语义(从不跳 20)+ 旧格式守卫;46 容器语料全绿`,
> Files 列表只保留 mix.rs 与 mix_integration.rs 两行。

**Files:**
- Create: `crates/core/src/format/sha1.rs`
- Modify: `crates/core/src/format/mod.rs`(追加 `pub mod sha1;`,置于 `pub mod shp;` 之前)
- Modify: `crates/core/src/format/mix.rs`(判别规则重写 + `MixDigestState` + `data_start()` 访问器 + 测试)
- Modify: `crates/core/tests/mix_integration.rs`(追加 1 个 ignored 语料测试)

**Interfaces:**
- Consumes: `MixArchive::parse(Vec<u8>)`(入参形状不变)、`FormatError`
- Produces(供 Task 5 装载器与全计划使用):
  - `pub fn sha1(data: &[u8]) -> [u8; 20]`(sha1.rs;FIPS 180-1 语义重写)
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum MixDigestState { None, Phantom, Verified }`
  - `MixArchive::digest_state(&self) -> MixDigestState` — `None`=未声明摘要位;`Phantom`=声明了摘要位但窗口 ≠ SHA-1(索引表),数据区从索引尾开始;`Verified`=窗口通过校验,数据区跳 20
  - `MixArchive::data_start(&self) -> usize`(Task 5 装载器切片用)

- 判别规则(依据 §2;XCC `mix_file.cpp` is_valid 的尺寸一致性规则对语料 27 个幻影容器全部误判,故升一级做内容校验;哈希区域 = 索引表 `data[10..10+12*count]`,为本计划的裁决定义——语料无真摘要正样本,合成测试双侧自洽):
  - flags 无摘要位 → 数据区 = `10+12*count`,state = `None`(现行为);
  - flags 有摘要位 → 计算 `sha1(索引表)`,窗口(索引后 20 字节)相等 → 跳 20,state = `Verified`;不等 → **不跳**,state = `Phantom`(修复点:旧代码无条件跳 20,27 个真库容器读取错位 +20)。

- [ ] **Step 1: 写 `sha1.rs`(`mod.rs` 先追加 `pub mod sha1;`)**

`crates/core/src/format/sha1.rs` 完整文件:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! 最小 SHA-1(FIPS 180-1),仅用于 TS/RA2 MIX 摘要位判别(见 mix.rs)。
//! 出处:算法按 FIPS 180-1 规范语义重写;用途对齐 XCC(GPL,Olaf van der Spek)
//! 对 TS/RA2 MIX checksum 的 SHA-1 语义(xcc misc/mix_file.cpp 的 m_has_checksum;
//! XCC 本体只做尺寸一致性判别,本项目升一级为内容校验,判别规则见 mix.rs 文档)。

pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476, 0xC3D2_E1F0];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for block in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 80];
        for (i, word) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for t in 16..80 {
            w[t] = (w[t - 3] ^ w[t - 8] ^ w[t - 14] ^ w[t - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (t, &wt) in w.iter().enumerate() {
            let (f, k) = match t {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let tmp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(wt);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = tmp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}
```

测试(同文件 `mod tests`):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn hex(d: &[u8]) -> String {
        d.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn fips_180_1_known_vectors() {
        // 依据 §7:hashlib 计算的标准向量
        assert_eq!(hex(&sha1(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    #[test]
    fn block_boundary_vectors() {
        // 55/56 字节边界(填充进/不进第二块)与多块向量;0x61 = 字符 a
        assert_eq!(hex(&sha1(&[0x61; 56])), "c2db330f6083854c99d4b5bfb6e8f29f201be699");
        assert_eq!(hex(&sha1(&[0x61; 64])), "0098ba824b5c16427bd7a1122a5a442a25ec644d");
        assert_eq!(hex(&sha1(&[0x61; 119])), "ee971065aaa017e0632a8ca6c77bb3bf8b1dfc56");
    }
}
```

Run: `cargo test -p ra2-core sha1`
Expected: 2 passed

- [ ] **Step 2: mix.rs 判别重写(旧实现下 Step 3 的 phantom 测试必红)**

`crates/core/src/format/mix.rs` 头文档(`//!` 区)追加一段:

```rust
//! 摘要位判别(M3 修复,研究 §2 + 2026-11-05 计划依据 §2 探针):TS/RA2 库中
//! 大量容器声明 SHA-1 摘要位(0x00010000)但索引后并无摘要块(幻影 digest;
//! repack 工具在 body_size 里预留了 20 字节却从未写入)——无条件跳 20 会把
//! 全部读取错位。XCC(GPL)的 is_valid 只做尺寸一致性校验,对这些容器同样
//! 误判(其规则 4+6+12·count+size+(checksum?20)==filesize,幻影容器的
//! body_size 恰好满足)。本实现升一级做内容校验:窗口 == SHA-1(索引表) 才
//! 承认摘要块。哈希区域 = 索引表 data[10..10+12·count](本仓库裁决定义;
//! 语料无真摘要正样本,合成测试两侧用同一函数保证自洽)。
```

文件头 use 区追加 `use super::sha1::sha1;`。enum 与 struct 替换:

```rust
/// 摘要块判别结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixDigestState {
    /// 未声明摘要位。
    None,
    /// 声明了摘要位但窗口未通过 SHA-1 校验(幻影;数据区从索引尾开始)。
    Phantom,
    /// 声明且校验通过(数据区跳 20 字节)。
    Verified,
}

pub struct MixArchive {
    data: Vec<u8>,
    entries: Vec<MixEntry>,
    data_start: usize,
    digest: MixDigestState,
}
```

`parse` 中从 `if has_digest {` 起到 `let data_start = r.position();` 替换为:

```rust
        let index_end = r.position(); // = 10 + 12*count
        let digest = if has_digest {
            // 摘要位声明 ≠ 摘要块存在(幻影 digest,研究 §2)。窗口通过
            // SHA-1(索引表) 校验才跳 20;否则数据区紧接索引尾。
            let index_bytes = &data[10..index_end];
            let expected = sha1(index_bytes);
            match data.get(index_end..index_end + 20) {
                Some(window) if window == expected => {
                    r.skip(20)?;
                    MixDigestState::Verified
                }
                _ => MixDigestState::Phantom,
            }
        } else {
            MixDigestState::None
        };
        let data_start = r.position();
```

构造改为 `Ok(MixArchive { data, entries, data_start, digest })`,impl 内追加访问器:

```rust
    /// 摘要块判别结果(spec §4 的 MixInfo 等价访问器)。
    pub fn digest_state(&self) -> MixDigestState {
        self.digest
    }

    /// 数据区起始绝对偏移(装载器切片用)。
    pub fn data_start(&self) -> usize {
        self.data_start
    }
```

- [ ] **Step 3: mix.rs 测试改写(删旧 `accepts_sha1_digest_flag` 与 `sample_digest_mix`,换三个)**

测试模块追加 use:`use super::sha1::sha1;`

```rust
    /// 构造单条目索引表字节。
    fn index_bytes(id: u32, off: u32, size: u32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&id.to_le_bytes());
        v.extend_from_slice(&off.to_le_bytes());
        v.extend_from_slice(&size.to_le_bytes());
        v
    }

    /// 真摘要样本:窗口 = SHA-1(索引表) → 跳 20 + Verified。
    fn sample_digest_mix() -> Vec<u8> {
        let id_hello = mix_name_id("HELLO");
        let body: Vec<u8> = b"HELLO".to_vec();
        let index = index_bytes(id_hello, 0, body.len() as u32);
        let digest = sha1(&index);
        let mut v = Vec::new();
        v.extend_from_slice(&0x0001_0000u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(&index);
        v.extend_from_slice(&digest);
        v.extend_from_slice(&body);
        v
    }

    /// 幻影摘要样本(M1a 旧 sample_digest_mix 的 0xAB×20 窗口正是此形态):
    /// 声明摘要位、窗口是垃圾 → 不跳 20,数据区紧接索引尾。
    fn sample_phantom_mix() -> Vec<u8> {
        let id_hello = mix_name_id("HELLO");
        let body: Vec<u8> = b"HELLO".to_vec();
        let mut v = Vec::new();
        v.extend_from_slice(&0x0001_0000u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(&index_bytes(id_hello, 0, body.len() as u32));
        v.extend_from_slice(&[0xABu8; 20]);
        v.extend_from_slice(&body);
        v
    }

    #[test]
    fn verified_digest_skips_twenty_and_reports_verified() {
        let mix = MixArchive::parse(sample_digest_mix()).unwrap();
        assert_eq!(mix.digest_state(), MixDigestState::Verified);
        assert_eq!(mix.data_start(), 10 + 12 + 20);
        assert_eq!(mix.read("HELLO").unwrap(), b"HELLO");
    }

    #[test]
    fn phantom_digest_flag_reads_from_index_end() {
        // 修复点:旧实现无条件跳 20,这里会读到 0xAB 垃圾(研究 §2 的 27 个
        // 真库容器正是此形态);新实现不跳,数据区从索引尾开始。
        let mix = MixArchive::parse(sample_phantom_mix()).unwrap();
        assert_eq!(mix.digest_state(), MixDigestState::Phantom);
        assert_eq!(mix.data_start(), 10 + 12);
        assert_eq!(mix.read("HELLO").unwrap(), b"HELLO");
    }

    #[test]
    fn plain_mix_reports_no_digest() {
        let mix = MixArchive::parse(sample_mix()).unwrap();
        assert_eq!(mix.digest_state(), MixDigestState::None);
        assert_eq!(mix.data_start(), 10 + 12 * 2);
    }
```

Run: `cargo test -p ra2-core mix`
Expected: 10 passed(既有 7 + 新 3;其余 mix 用户测试零影响)

- [ ] **Step 4: 先证旧实现必红(TDD 红),再全绿**

在未应用 Step 2 修改前(或临时 `git stash -- crates/core/src/format/mix.rs` 后)运行:

Run: `cargo test -p ra2-core phantom_digest_flag_reads_from_index_end`
Expected: FAIL(旧实现跳 20 后 read("HELLO") 读到 0xAB 垃圾)

恢复新实现后:

Run: `cargo test -p ra2-core sha1 && cargo test -p ra2-core mix`
Expected: sha1 2 passed;mix 10 passed

- [ ] **Step 5: 语料全量测试(ignored,追加到 `crates/core/tests/mix_integration.rs` 末尾)**

```rust
/// 递归枚举容器(与 2026-11-05 计划依据 §2 探针同口径):顶层 = RA2_ASSETS_DIR
/// 下全部 *.mix(含 langmd/thememd/language/ares),每层把"能被 MixArchive::parse
/// 接受"的条目作为嵌套容器下钻,深度 ≤4(顶层为深度 0)。判别修复后语料必须
/// 全绿:46 容器 = 27 幻影(Phantom)+ 19 朴素(None),0 真摘要(Verified)。
#[test]
#[ignore]
fn all_parseable_mix_containers_recurse_green() {
    #[derive(Default)]
    struct Stats {
        total: usize,
        phantom: usize,
        none: usize,
    }
    fn walk(data: &[u8], depth: usize, stats: &Stats) {
        let mix = match MixArchive::parse(data.to_vec()) {
            Ok(m) => m,
            Err(_) => return,
        };
        stats.total += 1;
        match mix.digest_state() {
            MixDigestState::None => stats.none += 1,
            MixDigestState::Phantom => stats.phantom += 1,
            MixDigestState::Verified => panic!("语料不应出现真摘要容器(依据 §2)"),
        }
        if depth >= 4 {
            return;
        }
        for e in mix.entries() {
            let blob = mix.read_by_id(e.id).unwrap().to_vec();
            walk(&blob, depth + 1, stats);
        }
    }
    let stats = Stats::default();
    let mut tops: Vec<std::path::PathBuf> = std::fs::read_dir(assets_dir())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "mix").unwrap_or(false))
        .collect();
    tops.sort();
    assert_eq!(tops.len(), 7, "顶层 mix 文件数(安装变动时同步更新)");
    for p in &tops {
        walk(&std::fs::read(p).unwrap(), 0, &stats);
    }
    assert_eq!(stats.total, 46, "可解析容器总数(依据 §2 遍历口径)");
    assert_eq!(stats.phantom, 27, "幻影 digest 容器数");
    assert_eq!(stats.none, 19, "朴素容器数");
    // 内容锚:判别修复后 isosnow 的 clear01.sno 读取长度精确(错位 +20 时必然失败)
    let ra2 = load("ra2.mix");
    let isosnow = MixArchive::parse(ra2.read("isosnow.mix").unwrap().to_vec()).unwrap();
    assert_eq!(isosnow.read("clear01.sno").unwrap().len(), 1_872);
}
```

文件头追加 use:`use ra2_core::format::mix::MixDigestState;`

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test mix_integration -- --ignored`
Expected: 8 passed(既有 7 + 新 1)

- [ ] **Step 6: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 全绿;合成 **196**(基线 192 + sha1 2 + mix 新 3 - 删旧 1 = +4)

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored`
Expected: 全绿;ignored **28**(27 + 1)

```bash
git add crates/core/src/format/sha1.rs crates/core/src/format/mod.rs crates/core/src/format/mix.rs crates/core/tests/mix_integration.rs
git commit -m "fix(core): MIX 幻影 digest 判别(SHA-1 内容校验)+ 最小 SHA-1;46 容器语料全绿" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 2: `tmp.rs` → `shp_ts.rs` 更名重定域(TS SHP 帧格式 + RAW 回退)+ M1b 测试迁移

**Files:**
- Create: `crates/core/src/format/shp_ts.rs`(由 `tmp.rs` 更名重写)
- Delete: `crates/core/src/format/tmp.rs`
- Modify: `crates/core/src/format/mod.rs`(`pub mod tmp;` 行替换为 `pub mod shp_ts;`,置于 `pub mod shp;` 之后)
- Modify: `crates/core/tests/format_samples.rs`(3 个 tmp 测试迁移重标)
- Modify: `crates/core/tests/m1b_acceptance.rs`(import 与字段重标)

**Interfaces:**
- Consumes: `rle_zero_ts_decode`(M1b,不变)、`FormatError`
- Produces(供 format_samples/m1b 使用;TheaterAssets/CLI 不直接用本模块):
  - `pub struct ShpTsFrameHeader { pub x: i16, pub y: i16, pub cx: u16, pub cy: u16, pub flags: u32, pub rgba: [u8; 4], pub reserved: u32, pub data_offset: u32 }`
  - `pub struct ShpTsFrame { pub header: ShpTsFrameHeader, pub pixels: Vec<u8> }`
  - `pub struct ShpTsFile { pub zero: u16, pub cx: u16, pub cy: u16, pub frames: Vec<ShpTsFrame> }`
  - `ShpTsFile::parse(data: &[u8]) -> Result<Self, FormatError>`

- 语义要点(研究 §4/§6,勿改):
  - 布局 = 8B 文件头 `{u16 zero=0; u16 cx; u16 cy; u16 frames}` + `frames`×24B 帧头 `{x,y i16; cx,cy u16; flags u32@+8; rgba [u8;4]@+12; reserved u32@+16; data_offset u32@+20}`。**重标对照**:旧 `unknown_a`→`flags`;旧 `_b||unknown_d`(字节 12..16)→`rgba`(little 端:低两字节为 R/G 通道,`unknown_d` 的低字节 = 蓝);旧 `radar[3]+_pad`(16..20)→`reserved`;`data_offset` 不变。甲骨文:burnt01 帧 0 data_offset=56=8+24·2、lobrdb08 帧 1 data_offset=152=8+24·6 精确闭合。
  - 帧数据解码判别式(研究 §4/§6):先 **RLEZeroTS 严格解**;失败且**帧数据区间(span)恰 == cx·cy** → RAW 原样 cx·cy 字节;失败且 span ≠ cx·cy → 透传 Err。真 RLE 与 RAW 在语料中无歧义交叉;RLE 可解时恒取 RLE(优先序锁定,含 span==cx·cy 的全透明帧形态)。
  - span 定义 = 该帧 offset 到"其后第一个更大的帧 offset 或文件尾"(与 shp.rs 的 ends 逻辑同构)。
  - 空帧约定 `cx==cy==data_offset==0` 不变;画布越界/表内 offset 校验照旧(全部 Err)。

- [ ] **Step 1: 写 `shp_ts.rs`(完整文件;`mod.rs` 行替换;`git rm crates/core/src/format/tmp.rs`)**

`crates/core/src/format/shp_ts.rs` 完整文件:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! TS SHP 精灵帧容器(t_shp_ts,XCC 术语;.sno/.tem 等剧场装饰精灵即此格式)。
//!
//! 出处:布局与判别按裁决后研究 2026-11-05-theater-tile-mapping.md §4/§6
//! (XCC `t_shp_ts` 语义重写;burnt01/lobrdb08 甲骨文帧偏移精确闭合)。
//! **M1b 误判纠正**:本文件前身 tmp.rs 曾把该 8B+24B 布局当成"RA2 剧场瓦片",
//! 实为 TS SHP 帧格式——帧头 24B = `{x,y i16; cx,cy u16; flags u32; rgba
//! [u8;4]; reserved u32; data_offset u32}`。TS 真瓦片(TS TMP,16B i32 头 +
//! 槽表)是另一种容器,由 theater_tmp.rs 承担,勿混用。
//!
//! 帧数据 = RLEZeroTS 行链(与 shp.rs 同方案)或 RAW cx·cy 字节(研究 §6:
//! 两形态无歧义交叉——先严格 RLE,失败且 span==cx·cy 才取 RAW)。空帧约定
//! `cx==cy==data_offset==0`(此时 flags 仍非 0,勿以 flags 判空)。

use super::bytes::ByteReader;
use super::rle_zero_ts::rle_zero_ts_decode;
use super::FormatError;

/// 单帧帧头(字段语义按研究 §4 重标)。
pub struct ShpTsFrameHeader {
    pub x: i16,
    pub y: i16,
    pub cx: u16,
    pub cy: u16,
    /// 观测多数 3、个别 4(bit0|bit1);语义未裁决,原样暴露。
    pub flags: u32,
    /// +0x0C..0x10 四字节;旧"unknown_d"即其蓝通道字节(研究 §4)。
    pub rgba: [u8; 4],
    /// +0x10..0x14(旧 radar[3]+_pad;lobrdb08 t0 = 0x0012FB00)。
    pub reserved: u32,
    /// 帧数据的绝对文件偏移;空帧为 0。
    pub data_offset: u32,
}

/// 一帧:帧头 + 解码后的调色板索引(行主序,恰好 `cx*cy` 字节;空帧为空)。
pub struct ShpTsFrame {
    pub header: ShpTsFrameHeader,
    pub pixels: Vec<u8>,
}

/// TS SHP 文件。
pub struct ShpTsFile {
    pub zero: u16,
    pub cx: u16,
    pub cy: u16,
    pub frames: Vec<ShpTsFrame>,
}

impl ShpTsFile {
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
            let rgba = [
                r.read_u8()?,
                r.read_u8()?,
                r.read_u8()?,
                r.read_u8()?,
            ];
            let reserved = r.read_u32_le()?;
            let data_offset = r.read_u32_le()?;
            headers.push(ShpTsFrameHeader {
                x,
                y,
                cx: fh_cx,
                cy: fh_cy,
                flags,
                rgba,
                reserved,
                data_offset,
            });
        }
        let table_end = r.position(); // = 8 + 24*frame_count
        let file_len = data.len();
        for h in &headers {
            let empty = h.cx == 0 && h.cy == 0 && h.data_offset == 0;
            if empty {
                continue;
            }
            if h.cx == 0 || h.cy == 0 || h.data_offset == 0 {
                return Err(FormatError::Invalid(
                    "帧矩形与数据偏移必须同时为 0(空)或同时非 0".to_string(),
                ));
            }
            if (h.data_offset as usize) < table_end || (h.data_offset as usize) >= file_len {
                return Err(FormatError::Invalid(format!(
                    "帧数据偏移 {} 越界(表尾 {table_end},文件长 {file_len})",
                    h.data_offset
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
        // 每帧数据区间:此后第一个更大的帧偏移(空帧 offset=0 不参与)或文件尾
        let mut ends = vec![file_len; frame_count];
        for (i, h) in headers.iter().enumerate() {
            let start = h.data_offset as usize;
            if let Some(o) = headers[i + 1..]
                .iter()
                .map(|n| n.data_offset as usize)
                .find(|&o| o > start)
            {
                ends[i] = o;
            }
        }
        let mut frames = Vec::with_capacity(frame_count);
        for (h, end) in headers.into_iter().zip(ends) {
            let empty = h.cx == 0 && h.cy == 0 && h.data_offset == 0;
            let pixels = if empty {
                Vec::new()
            } else {
                let start = h.data_offset as usize;
                let span = end - start;
                let need = h.cx as usize * h.cy as usize;
                let src = &data[start..end];
                let mut pixels = Vec::with_capacity(need);
                // 判别式(研究 §4/§6):先严格 RLE;失败且 span==cx·cy → RAW;
                // 失败且 span≠cx·cy → 透传 Err。RLE 可解恒取 RLE。
                match rle_zero_ts_decode(src, h.cx as usize, h.cy as usize, &mut pixels) {
                    Ok(_) => pixels,
                    Err(e) => {
                        if span == need {
                            pixels.clear();
                            pixels.extend_from_slice(&data[start..start + need]);
                            pixels
                        } else {
                            return Err(e);
                        }
                    }
                }
            };
            frames.push(ShpTsFrame { header: h, pixels });
        }
        Ok(ShpTsFile {
            zero,
            cx,
            cy,
            frames,
        })
    }
}
```

测试(同文件;前 6 个为旧 tmp.rs 测试的重命名迁移,断言字段重标;后 3 个新增 RAW 判别):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn frame_header(x: i16, y: i16, cx: u16, cy: u16, flags: u32, rgba: [u8; 4], reserved: u32, offset: u32) -> [u8; 24] {
        let mut b = [0u8; 24];
        b[0..2].copy_from_slice(&x.to_le_bytes());
        b[2..4].copy_from_slice(&y.to_le_bytes());
        b[4..6].copy_from_slice(&cx.to_le_bytes());
        b[6..8].copy_from_slice(&cy.to_le_bytes());
        b[8..12].copy_from_slice(&flags.to_le_bytes());
        b[12..16].copy_from_slice(&rgba);
        b[16..20].copy_from_slice(&reserved.to_le_bytes());
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
    fn parses_frame_and_decodes_rle() {
        // 旧 tmp.rs parses_tiles_and_decodes_rle 的迁移(burnt01 形态):
        // 帧头 flags=3(旧 unknown_a)、rgba=[0,0,1,0](旧 unknown_d=1 的蓝通道)
        let body: Vec<u8> = vec![
            0x06, 0x00, 0x31, 0x00, 0x01, 0x01, 0x05, 0x00, 0x32, 0x00, 0x04,
        ];
        let data = shp_bytes(
            (60, 60),
            &[frame_header(10, 20, 4, 2, 3, [0, 0, 1, 0], 0, 56)],
            &body,
        );
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!((f.cx, f.cy), (60, 60));
        assert_eq!(f.frames.len(), 1);
        let t = &f.frames[0];
        assert_eq!((t.header.x, t.header.y), (10, 20));
        assert_eq!(t.header.flags, 3);
        assert_eq!(t.header.rgba, [0, 0, 1, 0]);
        assert_eq!(t.header.reserved, 0);
        assert_eq!(t.header.data_offset, 56);
        assert_eq!(t.pixels, vec![0x31, 0, 0x01, 0, 0x32, 0, 0, 0]);
    }

    #[test]
    fn rejects_bad_zero_field() {
        let mut data = shp_bytes((60, 60), &[frame_header(0, 0, 4, 2, 3, [0; 4], 0, 56)], &[0u8; 8]);
        data[0..2].copy_from_slice(&9u16.to_le_bytes());
        assert!(matches!(ShpTsFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_zero_frames() {
        let data = shp_bytes((60, 60), &[], &[]);
        assert!(matches!(ShpTsFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_inconsistent_empty_frame() {
        // cx>0 但 data_offset=0:三字段必须同时为 0 或同时非 0
        let data = shp_bytes((60, 60), &[frame_header(0, 0, 4, 2, 3, [0; 4], 0, 0)], &[]);
        assert!(matches!(ShpTsFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_rect_out_of_canvas() {
        let data = shp_bytes((60, 60), &[frame_header(57, 0, 4, 2, 3, [0; 4], 0, 56)], &[0u8; 8]);
        assert!(matches!(ShpTsFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_offset_before_table() {
        let data = shp_bytes((60, 60), &[frame_header(0, 0, 4, 2, 3, [0; 4], 0, 16)], &[0u8; 8]);
        assert!(matches!(ShpTsFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn raw_fallback_when_rle_fails_and_span_exact() {
        // RAW 形态(研究 §6):span 恰 == cx·cy 且严格 RLE 失败(首行 count=2 < 4)
        let body = [0x02, 0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let data = shp_bytes((8, 2), &[frame_header(0, 0, 4, 2, 1, [0; 4], 0, 32)], &body);
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!(f.frames[0].pixels, body.to_vec());
    }

    #[test]
    fn rle_wins_when_span_equals_need() {
        // 优先序锁定:全透明帧的 RLE 行链(块头 4B + 零游程)span 也可恰 == cx·cy;
        // 此时必须按 RLE 解码为全 0,不得误取 RAW 原始字节
        let body = [0x04, 0x00, 0x00, 0x04];
        let data = shp_bytes((4, 1), &[frame_header(0, 0, 4, 1, 3, [0; 4], 0, 32)], &body);
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!(f.frames[0].pixels, vec![0, 0, 0, 0]);
    }

    #[test]
    fn err_when_rle_fails_and_span_mismatch() {
        // 严格 RLE 失败(截断)且 span(6)≠ cx·cy(8)→ 透传 Err
        let body = [0x08, 0x00, 0xAA, 0xBB, 0xCC, 0xDD];
        let data = shp_bytes((8, 2), &[frame_header(0, 0, 4, 2, 3, [0; 4], 0, 32)], &body);
        assert!(matches!(ShpTsFile::parse(&data), Err(FormatError::UnexpectedEof { .. })));
    }
}
```

Run: `cargo test -p ra2-core shp_ts`
Expected: 9 passed(迁移 6 + 新 3)

- [ ] **Step 2: 真样本测试迁移(`format_samples.rs` 3 个 + `m1b_acceptance.rs`)**

`crates/core/tests/format_samples.rs` 修改(测试名与 import 迁移;哈希/长度/像素断言全部不变,字段重标):

- 3 处 `use ra2_core::format::tmp::TmpRa2File;` → `use ra2_core::format::shp_ts::ShpTsFile;`
- `tmp_burnt01_sno_blob` → `shp_ts_burnt01_sno_sprite`:`TmpRa2File::parse` → `ShpTsFile::parse`;`(tmp.width, tmp.height)` → `(tmp.cx, tmp.cy)`;`tmp.tiles` → `tmp.frames`;`t0.header.unknown_a` → `t0.header.flags`(值 3 不变);`t0.header.unknown_d` → `t0.header.rgba` 改断言 `assert_eq!(t0.header.rgba, [0, 0, 1, 0]);`;`t1.header.radar` 断言删除(并入 reserved,见下);末尾追加 `assert_eq!(t1.header.reserved, 0);`
- `tmp_lobrdb08_sno_diamond` → `shp_ts_lobrdb08_sno_diamond`:同上迁移;`t0.header.radar == [0x00, 0xFB, 0x12]` 改为 `assert_eq!(t0.header.reserved, 0x0012_FB00);`(字节 00 FB 12 00 的小端 u32);`t1.header.unknown_d == 6` 改为 `assert_eq!(t1.header.rgba, [0, 0, 6, 0]);`
- `tmp_tile_reads_by_name_from_snow_mix` → `shp_ts_sprite_reads_by_name_from_snow_mix`:import/类型迁移,断言 `tmp.frames[0].pixels.len() == 29 * 17` 不变

`crates/core/tests/m1b_acceptance.rs` 修改:`use ra2_core::format::tmp::TmpRa2File;` → `use ra2_core::format::shp_ts::ShpTsFile;`;`TmpRa2File::parse(snow.read("burnt01.sno").unwrap())` → `ShpTsFile::parse(...)`;`tmp.tiles.len()` → `tmp.frames.len()`(注释"TMP:ra2.mix → snow.mix → burnt01.sno"改为"SHP(剧场装饰精灵,M1b 误判已纠正,研究 §4)")。

Run: `cargo test -p ra2-core --test format_samples && cargo test -p ra2-core --test m1b_acceptance`
Expected: 编译通过(ignored 13 与 1 个测试仍被过滤;数量不变)

- [ ] **Step 3: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 全绿;合成 **197**(194 + shp_ts 新 3;迁移不改计数)

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored`
Expected: 全绿;ignored 28(burnt01/lobrdb08 甲骨文迁移后依旧通过 = 重标正确性的实证)

```bash
git add crates/core/src/format/shp_ts.rs crates/core/src/format/mod.rs crates/core/tests/format_samples.rs crates/core/tests/m1b_acceptance.rs
git rm crates/core/src/format/tmp.rs
git commit -m "refactor(core): tmp.rs 更名重定域为 shp_ts.rs(TS SHP 帧格式;字段重标 + RAW 回退判别)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 3: TS TMP 解析器(`format/theater_tmp.rs`)

> **勘误 2026-11-05(控制器,ra2-decomp 匹配源码 + 语料字节双证裁决)——
> 本块修正下文 52B 槽头字段宽度与位语义,以本块为准:**
>
> 证据:引擎**整体读入文件直接内存投射**(`Load_Tile_Data` @ 0x00547020,
> `IsoTileSet{MapWidth@0,MapHeight@4,Width@8,Height@12,Tiles[]@16}` +
> `ptr+offset` 修复指针),磁盘布局 ≡ `IsoTileRecord`(#pragma pack(4),
> include/isotype.h)。故下文草图与 Interfaces 中的 i16 宽度全部改 i32:
> `x, y: i32`、`extra_rect: [i32; 4]`;字段位次为 x/y@+0、extra_ofs@+8、
> z_ofs@+0xC、extra_z_ofs@+0x10、extra_rect@+0x14、bitfield@+0x24、
> height:u8@+0x28、terrain_type:i8@+0x29、ramp_type:i8@+0x2A、
> radar_left@+0x2B、radar_right@+0x2E、pad:[u8;3]@+0x31(0x34=52 精确覆盖,
> **原 `tail: [u8; 12]` 字段删除**——歧义裁决 #3 的"未文档化尾域"随宽度
> 修正消解,pad 改 3B)。bit2 语义改 **IsRandomized**(非"damaged";语料
> MM 瓦片 0xCE/0xCF 实测置位),bit0=IsHasExtraData、bit1=IsHasZData 不变。
> **两级偏移语义(勿混)**:槽表项 = 文件基相对(下文原句"槽偏移相对文件头、
> 0=空槽"正确,保留);extra_ofs/z_ofs 字段 = **槽(record)相对**(引擎
> isotype.cpp:1282 `record + ExtraOffset`;z_ofs 语料恒 = 52+cx·cy/2,与
> 歧义裁决 #7 的连续布局读取一致,z_ofs 原样暴露不校验不变)。子瓦片索引
> x + cbx·y 行主序(引擎 SubTile_Index)。语料闭合锚:clear01.sno 1872 全
> 布局 + 10 个多槽文件 `len == 末槽+52+900+900(+extra)` 精确闭合(研究 §5
> 勘误后版本)。合成测试样本按 i32 宽度构造;对真实文件的断言以研究 §5 新表
> 为准。计数不变(合成 210 = 197+13)。

**Files:**
- Create: `crates/core/src/format/theater_tmp.rs`
- Modify: `crates/core/src/format/mod.rs`(追加 `pub mod theater_tmp;`,本任务先置于 `pub mod shp_ts;` 之后;Task 4 建 `theater` 后可按字母序整理,行为无关)

**Interfaces:**
- Consumes: `ByteReader`、`FormatError`(不变)
- Produces(供 Task 5/7/8 使用):
  - `pub const MAX_SLOTS: u64 = 65536;`(敌意槽网格上界,spec §4)
  - `pub const MAX_DIAMOND_BYTES: usize = 2 * 1024 * 1024;`(单槽菱形上界,spec §4)
  - `pub struct TsTmpSlotHeader { pub x: i16, pub y: i16, pub extra_ofs: i32, pub z_ofs: i32, pub extra_z_ofs: i32, pub extra_rect: [i16; 4], pub bitfield: u32, pub height: i8, pub terrain_type: i8, pub ramp_type: i8, pub radar_left: [u8; 3], pub radar_right: [u8; 3], pub pad: [u8; 3], pub tail: [u8; 12] }`(研究 §5 文档化字段止于 +0x28;40..52 为未文档化尾域,原样暴露——歧义裁决 #3)
  - `pub struct TsTmpSlot { pub header: TsTmpSlotHeader }` + `diamond_pixels(&self) -> &[u8]`(恒 cx·cy/2 RAW)+ `z_data(&self) -> Option<&[u8]>`
  - `pub struct TsTmpFile` + `parse(data: &[u8]) -> Result<Self, FormatError>` + `dims(&self) -> (u32, u32, u32, u32)`(cbx,cby,cx,cy)+ `slot_count(&self) -> usize`(= cbx·cby)+ `slot(&self, index: usize) -> Option<&TsTmpSlot>`(None = 空槽/越界)

- 语义要点(研究 §4/§5,勿改):头 16B 四个 i32;槽偏移相对文件头、0=空槽;52B 槽头字段名照研究 §5;守卫 `cbx·cby ≤ 65536`、`cx·cy/2 ≤ 2 MiB`、`16+4·cbx·cby+52 ≤ len`(敌意谎报一律 `Invalid`,分配前判定);`cx·cy` 奇数 → Invalid;槽偏移负值或落在头/槽表区 → Invalid;槽越界(52+菱形+z 超文件)→ Invalid。bitfield bit0=extra(不读内容)、bit1=有 z、bit2=damaged;0xCD 填充照抄不校验。

- [ ] **Step 1: 写 `theater_tmp.rs`(完整文件;TDD 以 Step 2 的编译失败确认红)**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! TS 格式剧场瓦片容器(t_tmp,XCC 术语;.tem/.urb/.sno/.des/.lun/.ubn 瓦片)。
//!
//! 出处:裁决后研究 2026-11-05-theater-tile-mapping.md §4/§5(XCC GPL 的
//! t_tmp 结构语义重写;泄漏 TS/RA2 引擎源码未参考)。布局:16B i32 头
//! `{cbx, cby, cx, cy}` + cbx·cby 个 i32 槽偏移(相对文件头;0 = 空槽)+
//! 每非空槽 52B 图像头 + RAW 菱形 cx·cy/2 字节 + 高度/z 数据(bitfield
//! bit1 置位时)。clear01.sno 精确闭合:len = 16 + 4·1 + 52 + 900 + 900
//! = 1872(cbx=cby=1, cx=60, cy=30)。z 按连续布局读(槽头→菱形→z 紧邻,
//! 见计划歧义裁决 #7;研究 §5 的 z_ofs 字段原样暴露不校验)。0xCC/0xCD
//! 填充域原样暴露不校验(西木出厂态,零 panic 姿势);terrain_type/
//! ramp_type 值表未逐值复核,只透传原始字节(spec §3 边界)。
//!
//! 敌意预检采用保守上界:槽越界预检按"52 头 + 菱形 + 一份 z"的最大需求
//! 判定,实际读取再按 bitfield 精确取量——任何截断都会在读阶段被
//! ByteReader 拦为 UnexpectedEof,预检只负责把离谱偏移提前拦为 Invalid。

use super::bytes::ByteReader;
use super::FormatError;

/// 敌意槽网格上界(spec §4;语料最大 5×5)。
pub const MAX_SLOTS: u64 = 65536;

/// 单槽菱形字节数上界(spec §4;60×30 的 900 B 远在其下)。
pub const MAX_DIAMOND_BYTES: usize = 2 * 1024 * 1024;

/// 52B 图像头(研究 §5 字段表;文档化字段止于 +0x28,tail 为 40..52 原样字节)。
pub struct TsTmpSlotHeader {
    pub x: i16,
    pub y: i16,
    pub extra_ofs: i32,
    pub z_ofs: i32,
    pub extra_z_ofs: i32,
    /// extra_x/y/cx/cy(extra 页不解析,spec §3 边界)。
    pub extra_rect: [i16; 4],
    /// bit0=有 extra,bit1=有 z,bit2=damaged;0xCD 填充原样。
    pub bitfield: u32,
    pub height: i8,
    pub terrain_type: i8,
    pub ramp_type: i8,
    pub radar_left: [u8; 3],
    pub radar_right: [u8; 3],
    pub pad: [u8; 3],
    /// 未文档化尾域(+0x28..0x34),原样暴露不校验(歧义裁决 #3)。
    pub tail: [u8; 12],
}

/// 一个非空槽:52B 头 + RAW 菱形 + 可选 z(连续布局)。
pub struct TsTmpSlot {
    pub header: TsTmpSlotHeader,
    diamond: Vec<u8>,
    z: Option<Vec<u8>>,
}

impl TsTmpSlot {
    /// RAW 菱形像素(恒 cx·cy/2 字节;铺canvas行序见 Task 7)。
    pub fn diamond_pixels(&self) -> &[u8] {
        &self.diamond
    }

    /// 高度/z 数据(bitfield bit1 置位时存在,恒 cx·cy/2 字节)。
    pub fn z_data(&self) -> Option<&[u8]> {
        self.z.as_deref()
    }
}

/// TS TMP 文件。
pub struct TsTmpFile {
    cbx: u32,
    cby: u32,
    cx: u32,
    cy: u32,
    slots: Vec<Option<TsTmpSlot>>,
}

impl TsTmpFile {
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        let mut r = ByteReader::new(data);
        // 敌意负值:i32 原读后再解释(u32 → i32 → i64 保号)
        let cbx = r.read_u32_le()? as i32 as i64;
        let cby = r.read_u32_le()? as i32 as i64;
        let cx = r.read_u32_le()? as i32 as i64;
        let cy = r.read_u32_le()? as i32 as i64;
        if cbx <= 0 || cby <= 0 || cx < 2 || cy < 2 {
            return Err(FormatError::Invalid(format!(
                "TS TMP 头尺寸谎报: cbx={cbx} cby={cby} cx={cx} cy={cy}"
            )));
        }
        if cbx * cby > MAX_SLOTS as i64 {
            return Err(FormatError::Invalid(format!(
                "槽网格 {cbx}×{cby} = {} 超过上界 {MAX_SLOTS}",
                cbx * cby
            )));
        }
        if (cx * cy) % 2 != 0 {
            return Err(FormatError::Invalid(format!(
                "画布 {cx}×{cy} 像素数为奇,无法对半分菱形"
            )));
        }
        let diamond = (cx * cy / 2) as usize;
        if diamond > MAX_DIAMOND_BYTES {
            return Err(FormatError::Invalid(format!(
                "单槽菱形 {diamond} B 超过上界 {MAX_DIAMOND_BYTES}"
            )));
        }
        let table_end = 16 + 4 * (cbx * cby) as usize;
        if table_end + 52 > data.len() {
            return Err(FormatError::Invalid(format!(
                "文件长 {} 不足以容纳头+槽表+最小槽头(需 {table_end}+52)",
                data.len()
            )));
        }
        let mut offsets = Vec::with_capacity((cbx * cby) as usize);
        for _ in 0..cbx * cby {
            offsets.push(r.read_u32_le()? as i32 as i64);
        }
        // 保守上界预检:52 头 + 菱形 + 至多一份 z
        let need_max = 52 + 2 * diamond;
        for (i, &off) in offsets.iter().enumerate() {
            if off == 0 {
                continue; // 空槽
            }
            if off < 0 || (off as usize) < table_end {
                return Err(FormatError::Invalid(format!(
                    "槽 {i} 偏移 {off} 非法(负值或落在头/槽表区 {table_end} 内)"
                )));
            }
            if (off as usize) + need_max > data.len() {
                return Err(FormatError::Invalid(format!(
                    "槽 {i} 偏移 {off} 越界: 需至多 {need_max} B,文件仅剩 {}",
                    data.len() - off as usize
                )));
            }
        }
        let mut slots = Vec::with_capacity(offsets.len());
        for &off in &offsets {
            slots.push(if off == 0 {
                None
            } else {
                Some(parse_slot(data, off as usize, diamond)?)
            });
        }
        Ok(TsTmpFile {
            cbx: cbx as u32,
            cby: cby as u32,
            cx: cx as u32,
            cy: cy as u32,
            slots,
        })
    }

    /// (cbx, cby, cx, cy)。
    pub fn dims(&self) -> (u32, u32, u32, u32) {
        (self.cbx, self.cby, self.cx, self.cy)
    }

    /// 槽位总数 = cbx·cby(空槽计入)。
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// 槽位;`None` = 空槽(偏移 0)或下标越界。
    pub fn slot(&self, index: usize) -> Option<&TsTmpSlot> {
        self.slots.get(index).and_then(|s| s.as_ref())
    }
}

fn parse_slot(data: &[u8], off: usize, diamond_len: usize) -> Result<TsTmpSlot, FormatError> {
    let mut r = ByteReader::new(&data[off..]);
    let x = r.read_u16_le()? as i16;
    let y = r.read_u16_le()? as i16;
    let extra_ofs = r.read_u32_le()? as i32;
    let z_ofs = r.read_u32_le()? as i32;
    let extra_z_ofs = r.read_u32_le()? as i32;
    let extra_x = r.read_u16_le()? as i16;
    let extra_y = r.read_u16_le()? as i16;
    let extra_cx = r.read_u16_le()? as i16;
    let extra_cy = r.read_u16_le()? as i16;
    let bitfield = r.read_u32_le()?;
    let height = r.read_u8()? as i8;
    let terrain_type = r.read_u8()? as i8;
    let ramp_type = r.read_u8()? as i8;
    let radar_left = [r.read_u8()?, r.read_u8()?, r.read_u8()?];
    let radar_right = [r.read_u8()?, r.read_u8()?, r.read_u8()?];
    let pad = [r.read_u8()?, r.read_u8()?, r.read_u8()?];
    let mut tail = [0u8; 12];
    tail.copy_from_slice(r.read_bytes(12)?);
    let header = TsTmpSlotHeader {
        x,
        y,
        extra_ofs,
        z_ofs,
        extra_z_ofs,
        extra_rect: [extra_x, extra_y, extra_cx, extra_cy],
        bitfield,
        height,
        terrain_type,
        ramp_type,
        radar_left,
        radar_right,
        pad,
        tail,
    };
    let mut diamond = Vec::with_capacity(diamond_len);
    diamond.extend_from_slice(r.read_bytes(diamond_len)?);
    let z = if bitfield & 0b10 != 0 {
        let mut z = Vec::with_capacity(diamond_len);
        z.extend_from_slice(r.read_bytes(diamond_len)?);
        Some(z)
    } else {
        None
    };
    Ok(TsTmpSlot { header, diamond, z })
}
```

测试(同文件;13 个):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn slot_bytes(bitfield: u32, diamond: &[u8], z: Option<&[u8]>) -> Vec<u8> {
        let mut b = vec![0u8; 52];
        b[24..28].copy_from_slice(&bitfield.to_le_bytes());
        b.extend_from_slice(diamond);
        if let Some(z) = z {
            b.extend_from_slice(z);
        }
        b
    }

    /// 头 + 槽偏移表 + 任意尾段(槽 blob 由测试自行放置在声明的偏移处)。
    fn tmp_bytes(cbx: i32, cby: i32, cx: i32, cy: i32, offsets: &[i32], tail: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&cbx.to_le_bytes());
        v.extend_from_slice(&cby.to_le_bytes());
        v.extend_from_slice(&cx.to_le_bytes());
        v.extend_from_slice(&cy.to_le_bytes());
        for o in offsets {
            v.extend_from_slice(&o.to_le_bytes());
        }
        v.extend_from_slice(tail);
        v
    }

    #[test]
    fn single_slot_closure_with_z() {
        // cx=8, cy=4 → 菱形 16 B;bit1 置位 → z 16 B;len = 16+4+52+16+16 = 104
        // (clear01.sno 的 1872 = 16+4+52+900+900 同构缩小版)
        let diamond: Vec<u8> = (1..=16).collect();
        let z = vec![0xAAu8; 16];
        let blob = slot_bytes(2, &diamond, Some(&z));
        let data = tmp_bytes(1, 1, 8, 4, &[20], &blob);
        let f = TsTmpFile::parse(&data).unwrap();
        assert_eq!(f.dims(), (1, 1, 8, 4));
        assert_eq!(f.slot_count(), 1);
        let s = f.slot(0).unwrap();
        assert_eq!(s.diamond_pixels(), &diamond);
        assert_eq!(s.z_data(), Some(&z[..]));
        assert_eq!(s.header.bitfield, 2);
        assert_eq!(s.header.x, 0);
        assert!(f.slot(1).is_none());
    }

    #[test]
    fn empty_slot_is_none() {
        let blob = slot_bytes(0, &[7u8; 16], None);
        let data = tmp_bytes(1, 2, 8, 4, &[20, 0], &blob);
        let f = TsTmpFile::parse(&data).unwrap();
        assert_eq!(f.slot_count(), 2);
        assert!(f.slot(0).is_some());
        assert!(f.slot(1).is_none());
    }

    #[test]
    fn multi_slot_grid() {
        // cbx=2, cby=1:槽表 8 B,槽 0 在 24、槽 1 在 108(各 84 B)
        let d0 = vec![1u8; 16];
        let d1 = vec![2u8; 16];
        let mut tail = slot_bytes(0, &d0, None);
        tail.extend_from_slice(&slot_bytes(0, &d1, None));
        let data = tmp_bytes(2, 1, 8, 4, &[24, 108], &tail);
        let f = TsTmpFile::parse(&data).unwrap();
        assert_eq!(f.slot(0).unwrap().diamond_pixels(), &d0);
        assert_eq!(f.slot(1).unwrap().diamond_pixels(), &d1);
    }

    #[test]
    fn z_absent_when_bit1_clear() {
        let blob = slot_bytes(0, &[7u8; 16], None);
        let data = tmp_bytes(1, 1, 8, 4, &[20], &blob);
        let s = TsTmpFile::parse(&data).unwrap().slot(0).unwrap();
        assert!(s.z_data().is_none());
    }

    #[test]
    fn cd_fields_passthrough() {
        // 0xCD 填充槽头(西木出厂态):原样暴露、不校验、不 panic
        let mut blob = vec![0xCDu8; 52];
        blob[24..28].copy_from_slice(&0u32.to_le_bytes()); // bitfield 无 z
        blob.extend_from_slice(&[9u8; 16]);
        let data = tmp_bytes(1, 1, 8, 4, &[20], &blob);
        let s = TsTmpFile::parse(&data).unwrap().slot(0).unwrap();
        assert_eq!(s.header.extra_ofs, -84215045); // 0xCDCDCDCD as i32
        assert_eq!(s.header.tail, [0xCDu8; 12]);
        assert_eq!(s.diamond_pixels(), &[9u8; 16]);
    }

    #[test]
    fn rejects_odd_canvas() {
        let data = tmp_bytes(1, 1, 7, 3, &[], &[]);
        assert!(matches!(TsTmpFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_zero_dims() {
        let data = tmp_bytes(0, 1, 8, 4, &[], &[]);
        assert!(matches!(TsTmpFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_slot_grid_over_max_slots() {
        // 300×300 = 90000 > 65536
        let data = tmp_bytes(300, 300, 8, 4, &[], &[]);
        assert!(matches!(TsTmpFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_canvas_diamond_over_2mib() {
        // 3000×3000/2 = 4.5 MB > 2 MiB
        let data = tmp_bytes(1, 1, 3000, 3000, &[], &[]);
        assert!(matches!(TsTmpFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_truncated_file() {
        // cbx=cby=1 时槽表尾 20,最小槽头到 72;20 字节文件不够
        let data = tmp_bytes(1, 1, 8, 4, &[0u32.to_le_bytes().iter().copied().next().unwrap_or(0) as i32], &[]);
        assert!(matches!(TsTmpFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_slot_offset_into_header_table() {
        // 偏移 4 落在头/槽表区(table_end = 20)内
        let blob = slot_bytes(0, &[7u8; 16], None);
        let data = tmp_bytes(1, 1, 8, 4, &[4], &blob);
        assert!(matches!(TsTmpFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_negative_slot_offset() {
        let blob = slot_bytes(0, &[7u8; 16], None);
        let data = tmp_bytes(1, 1, 8, 4, &[-1], &blob);
        assert!(matches!(TsTmpFile::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_truncated_slot_body() {
        // 槽声明在 20,但 blob 只放了 62 B(52+10),不足 52+16(无 z 下界)
        let blob = slot_bytes(0, &[7u8; 10], None);
        let data = tmp_bytes(1, 1, 8, 4, &[20], &blob);
        assert!(matches!(TsTmpFile::parse(&data), Err(FormatError::Invalid(_))));
    }
}
```

> **偏差记录**:rejects_truncated_file 测试的 offsets 构造行(`0u32.to_le_bytes()...`)写复杂了,实现时直接写 `&[0]`——断言语义不变(文件 20 字节 + 保守预检 72 > 20 → Invalid)。

Run: `cargo test -p ra2-core theater_tmp`
Expected: 13 passed

- [ ] **Step 2: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 全绿;合成 **210**(197 + 13)

```bash
git add crates/core/src/format/theater_tmp.rs crates/core/src/format/mod.rs
git commit -m "feat(core): TS TMP 解析器(16B 头/槽表/52B 头/RAW 菱形 + z;敌意守卫零 panic)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 4: `theater.rs` 前半——TheaterIni(顺序探查/atoi/保留槽/LastTilesInSet)+ 剧场常数表

**Files:**
- Create: `crates/core/src/format/theater.rs`
- Modify: `crates/core/src/format/mod.rs`(追加 `pub mod theater;`,置于 `pub mod shp_ts;` 之后)
- Create: `crates/core/tests/theater_samples.rs`(真 INI 测试;Task 5 继续追加)

**Interfaces:**
- Consumes: `IniDocument`/`parse_int_prefix`(ini.rs,不变)、`decode_map_text`(map/mapdoc.rs)、`Theater`(map/meta.rs,不变)、`FormatError`
- Produces(供 Task 5/6/7/8 使用):
  - `pub struct TheaterSpec { pub ini_name: &'static str, pub ext: &'static str, pub capacity: usize }`
  - `pub fn theater_spec(theater: Theater) -> TheaterSpec`(六剧场常数表,研究 §3/§8;REA Theater::Array VA 0x7E1B78 互证 .ubn 与六份 md INI 名;容量为运行时累加和,非二进制常数)
  - `#[derive(Debug, Clone, PartialEq, Eq)] pub enum SlotFile { File(String), Reserved }`(Reserved = blankFileName 且 TilesInSet>0 的保留槽,set0006 BLACKTILE 形态)
  - `#[derive(Debug, Clone, PartialEq, Eq)] pub struct TheaterSetInfo { pub number: u16, pub name: Option<String>, pub first_index: usize, pub tiles: usize }`
  - `pub struct TheaterIni` + `parse(text: &str) -> Result<Self, FormatError>` + `slot_count(&self) -> usize` + `file_name(&self, index: usize, ext: &str) -> Option<SlotFile>`(None = 界外)+ `set_of(&self, index: usize) -> Option<TheaterSetInfo>`

- 语义要点(研究 §3 + REA 勘误,勿改):
  - **顺序探查**:自 ordinal 0 依次探测段名 `TileSet0000`、`TileSet0001`、…(`format!("TileSet{ordinal:04}")`,IniDocument 段名大小写不敏感),**首个缺失段即停**(引擎逐序探测语义;语料八份 INI 实测 0000 起连续,依据 §3)。
  - `TilesInSet`:atoi 前缀(`get_int`,缺省 0;出厂脏数据 `TilesInSet=o` → 0,研究 §3);`>99` 截到 99;负值钳 0。
  - **LastTilesInSet**(REA 勘误):键存在且其 atoi 值 ≠ TilesInSet → `FormatError::Unsupported`(引擎按 LastTilesInSet 推进全局索引并把 extras 映到表尾——全语料 8 份 INI 零出现,死路不实现,依据 §3);键缺失或值相等 → 正常。
  - blank 集:`FileName` 缺失或空白 → 无文件;TilesInSet=0 → 整集不贡献;>0 → 占保留槽(SlotFile::Reserved),`file_name` 对该槽返回 `Reserved`。
  - 文件名 = `FileName` + `%02d`(集内序数,**1 基**) + `.` + 剧场扩展(锚点:tem[837]=ariz01.tem 为 set0081 唯一文件 → 1 基; TilesInSet 截 99 → 序数恒两位)。
  - 畸形宽容:缺 Name/FileName/TilesInSet 不 Err(spec §5);段序即 IniDocument 的 sections 顺序。

- [ ] **Step 1: 写 `theater.rs` 前半(完整代码)**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! 剧场瓦片索引层(M3):INI 表反查、剧场常数表、资产装载器。
//!
//! 出处:索引空间规则 = 裁决后研究 2026-11-05-theater-tile-mapping.md §3
//! (tile_id = 剧场 INI 枚举位;md 整体替换 base;六剧场容量);REA 勘误
//! (2026-11-05 控制器,Init_Secondary_Mixes 0x5301A0 / MixFile find
//! 0x5B4430 / Theater::Array VA 0x7E1B78):挂载序首中即胜、顺序探查、
//! LastTilesInSet 引擎规则、.ubn 扩展名与六份 md INI 名互证。

use crate::format::ini::{parse_int_prefix, IniDocument};
use crate::format::map::meta::Theater;
use crate::format::FormatError;

/// 剧场常数表项(研究 §3/§8;容量为 md INI 的 TilesInSet 运行时累加和)。
pub struct TheaterSpec {
    pub ini_name: &'static str,
    pub ext: &'static str,
    pub capacity: usize,
}

/// 目录名 → {生效 INI 名, 扩展名, 容量}(urbannmd 采 expandmd01 版,容量 1189)。
pub fn theater_spec(theater: Theater) -> TheaterSpec {
    match theater {
        Theater::Temperate => TheaterSpec { ini_name: "temperatmd.ini", ext: "tem", capacity: 838 },
        Theater::Urban => TheaterSpec { ini_name: "urbanmd.ini", ext: "urb", capacity: 1081 },
        Theater::Snow => TheaterSpec { ini_name: "snowmd.ini", ext: "sno", capacity: 964 },
        Theater::Desert => TheaterSpec { ini_name: "desertmd.ini", ext: "des", capacity: 726 },
        Theater::NewUrban => TheaterSpec { ini_name: "urbannmd.ini", ext: "ubn", capacity: 1189 },
        Theater::Lunar => TheaterSpec { ini_name: "lunarmd.ini", ext: "lun", capacity: 198 },
    }
}

/// 一个槽位的文件形态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotFile {
    /// FileName + %02d(1 基序数)+ 扩展名。
    File(String),
    /// blankFileName 且 TilesInSet>0 的保留槽(set0006 = [General] BLACKTILE
    /// 形态):占索引、无自有文件,语料从不引用(研究 §3)。
    Reserved,
}

/// 集合信息(set_of 返回;所有字段 owned,无借用)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheaterSetInfo {
    pub number: u16,
    /// 集内 [TileSetNNNN] 的 Name= 键(缺失为 None)。
    pub name: Option<String>,
    /// 该集首个全局槽位号。
    pub first_index: usize,
    pub tiles: usize,
}

struct TheaterSet {
    number: u16,
    name: Option<String>,
    /// FileName 原文;None = blank 集。
    file_base: Option<String>,
    tiles: usize,
    first_index: usize,
}

/// 剧场 INI 瓦片索引表。
#[derive(Debug, Clone)]
pub struct TheaterIni {
    sets: Vec<TheaterSet>,
    slot_count: usize,
}

impl TheaterIni {
    /// 解析剧场瓦片 INI(引擎逐序探测语义,见上文语义要点)。
    pub fn parse(text: &str) -> Result<Self, FormatError> {
        let doc = IniDocument::parse(text);
        let mut sets = Vec::new();
        let mut slot_count = 0usize;
        let mut ordinal = 0usize;
        loop {
            let sec = format!("TileSet{ordinal:04}");
            let Some(section) = doc.section(&sec) else {
                break; // 首个缺失段即停(REA 勘误:引擎顺序探查)
            };
            let _ = section;
            let tiles_raw = doc.get_int(&sec, "TilesInSet", 0);
            let tiles = tiles_raw.clamp(0, 99) as usize;
            if let Some(lts_raw) = doc.value(&sec, "LastTilesInSet") {
                // 引擎按 LastTilesInSet 推进索引并映射 extras(REA 勘误);
                // 全语料零出现(依据 §3),死路不实现 → Unsupported
                let lts = parse_int_prefix(lts_raw).unwrap_or(0);
                if lts != tiles_raw {
                    return Err(FormatError::Unsupported(format!(
                        "{sec}: LastTilesInSet={lts_raw} != TilesInSet={tiles_raw}(引擎规则未实现,研究 §3)"
                    )));
                }
            }
            let file_base = doc
                .value(&sec, "FileName")
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(str::to_string);
            if file_base.is_some() || tiles > 0 {
                let name = doc.value(&sec, "Name").map(str::to_string);
                sets.push(TheaterSet {
                    number: ordinal as u16,
                    name,
                    file_base,
                    tiles,
                    first_index: slot_count,
                });
                slot_count += tiles;
            }
            ordinal += 1;
        }
        Ok(TheaterIni { sets, slot_count })
    }

    /// 全局槽位总数(= 六剧场容量常数;Task 8 与 theater_spec 互证)。
    pub fn slot_count(&self) -> usize {
        self.slot_count
    }

    /// 全局槽位 → 文件名;`None` = 界外。
    pub fn file_name(&self, index: usize, ext: &str) -> Option<SlotFile> {
        let set = self.sets.iter().rev().find(|s| s.first_index <= index)?;
        let ordinal = index - set.first_index;
        if ordinal >= set.tiles {
            return None;
        }
        match &set.file_base {
            Some(base) => Some(SlotFile::File(format!("{base}{:02}.{ext}", ordinal + 1))),
            None => Some(SlotFile::Reserved),
        }
    }

    /// 全局槽位 → 所属集合(owned;界外 None,含落入零 tiles 集阴影区的索引)。
    pub fn set_of(&self, index: usize) -> Option<TheaterSetInfo> {
        let set = self.sets.iter().rev().find(|s| s.first_index <= index)?;
        if index - set.first_index >= set.tiles {
            return None;
        }
        Some(TheaterSetInfo {
            number: set.number,
            name: set.name.clone(),
            first_index: set.first_index,
            tiles: set.tiles,
        })
    }
}
```

- [ ] **Step 2: 合成测试(`theater.rs` 追加 `mod tests`,10 个)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn ini(text: &str) -> TheaterIni {
        TheaterIni::parse(text).unwrap()
    }

    #[test]
    fn sequential_probe_and_gap_terminates() {
        // TileSet0000..0002 连续贡献;0003 缺失即停;0004 不参与(REA 勘误)
        let t = ini(
            "[TileSet0000]\nFileName=aaa\nTilesInSet=2\n\
             [TileSet0001]\nFileName=bbb\nTilesInSet=1\n\
             [TileSet0002]\nFileName=ccc\nTilesInSet=3\n\
             [TileSet0004]\nFileName=ddd\nTilesInSet=9\n",
        );
        assert_eq!(t.slot_count(), 6);
        assert_eq!(t.file_name(5, "sno"), Some(SlotFile::File("ccc03.sno".into())));
        assert_eq!(t.file_name(6, "sno"), None);
    }

    #[test]
    fn atoi_prefix_and_clamp() {
        // 出厂脏数据 TilesInSet=o → 0(研究 §3,localmd urbannmd set0105 实测);
        // >99 截 99(语料无,规则照研究);负值钳 0
        let t = ini(
            "[TileSet0000]\nFileName=a\nTilesInSet=o\n\
             [TileSet0001]\nFileName=b\nTilesInSet=12x\n\
             [TileSet0002]\nFileName=c\nTilesInSet=150\n\
             [TileSet0003]\nFileName=d\nTilesInSet=-3\n",
        );
        assert_eq!(t.slot_count(), 111); // 0 + 12 + 99(150 截)+ 0
        assert_eq!(t.set_of(0).unwrap().number, 1);
    }

    #[test]
    fn blank_zero_tiles_contribute_nothing() {
        let t = ini(
            "[TileSet0000]\nFileName=aaa\nTilesInSet=2\n\
             [TileSet0001]\nTilesInSet=0\n\
             [TileSet0002]\nFileName=bbb\nTilesInSet=1\n",
        );
        assert_eq!(t.slot_count(), 3);
        assert_eq!(t.file_name(2, "sno"), Some(SlotFile::File("bbb01.sno".into())));
    }

    #[test]
    fn reserved_blank_set_occupies_slot() {
        // set0006 BLACKTILE 形态:blank + TilesInSet=1 → 占位,Reserved 无文件
        let t = ini(
            "[TileSet0000]\nFileName=aaa\nTilesInSet=2\n\
             [TileSet0001]\nTilesInSet=1\n",
        );
        assert_eq!(t.slot_count(), 3);
        assert_eq!(t.file_name(2, "sno"), Some(SlotFile::Reserved));
        assert!(t.file_name(3, "sno").is_none());
    }

    #[test]
    fn file_name_one_based_ordinals() {
        // 锚点反推:tem[837]=ariz01.tem 为 set0081 唯一文件 → 集内序数 1 基
        let t = ini("[TileSet0000]\nFileName=ariz\nTilesInSet=2\n");
        assert_eq!(t.file_name(0, "tem"), Some(SlotFile::File("ariz01.tem".into())));
        assert_eq!(t.file_name(1, "tem"), Some(SlotFile::File("ariz02.tem".into())));
    }

    #[test]
    fn set_of_returns_info() {
        // 顺序探测语义下 0081 单段不可达(0000 缺即停)——用 81 个无键占位段
        // 铺满 0000..0080(零贡献,同 missing_keys 路径),set0081 first_index=0
        let mut text = String::new();
        for i in 0..81 {
            text.push_str(&format!("[TileSet{i:04}]\n"));
        }
        text.push_str("[TileSet0081]\nName=USS Arizona\nFileName=ariz\nTilesInSet=1\n");
        let t = ini(&text);
        let s = t.set_of(0).unwrap();
        assert_eq!(s.number, 81);
        assert_eq!(s.name.as_deref(), Some("USS Arizona"));
        assert_eq!((s.first_index, s.tiles), (0, 1));
        assert!(t.set_of(1).is_none());
    }

    #[test]
    fn keys_and_sections_case_insensitive() {
        let t = ini("[tileset0000]\nfilename=AAA\ntilesinset=2\nname=Mixed\n");
        assert_eq!(t.slot_count(), 2);
        assert_eq!(t.file_name(0, "sno"), Some(SlotFile::File("AAA01.sno".into())));
    }

    #[test]
    fn missing_keys_lenient() {
        // 缺 FileName/TilesInSet → 空集 0 贡献、探查继续(spec §5 宽容)
        let t = ini(
            "[TileSet0000]\nName=Ghost\n\
             [TileSet0001]\nFileName=bbb\nTilesInSet=2\n",
        );
        assert_eq!(t.slot_count(), 2);
        assert_eq!(t.set_of(0).unwrap().number, 1);
    }

    #[test]
    fn last_tiles_in_set_mismatch_is_unsupported() {
        // REA 勘误引擎规则:LastTilesInSet != TilesInSet → Unsupported(死路)
        let bad = TheaterIni::parse("[TileSet0000]\nFileName=a\nTilesInSet=3\nLastTilesInSet=5\n").unwrap_err();
        assert!(matches!(bad, FormatError::Unsupported(_)));
        // 值相等或缺失 → 正常
        let ok = ini("[TileSet0000]\nFileName=a\nTilesInSet=3\nLastTilesInSet=3\n");
        assert_eq!(ok.slot_count(), 3);
    }

    #[test]
    fn theater_spec_table_six_theaters() {
        // 研究 §3/§8 容量常数 + 扩展名(REA Theater::Array 0x7E1B78 互证)
        let cases = [
            (Theater::Temperate, "temperatmd.ini", "tem", 838),
            (Theater::Urban, "urbanmd.ini", "urb", 1081),
            (Theater::Snow, "snowmd.ini", "sno", 964),
            (Theater::Desert, "desertmd.ini", "des", 726),
            (Theater::NewUrban, "urbannmd.ini", "ubn", 1189),
            (Theater::Lunar, "lunarmd.ini", "lun", 198),
        ];
        for (theater, ini_name, ext, cap) in cases {
            let s = theater_spec(theater);
            assert_eq!(s.ini_name, ini_name);
            assert_eq!(s.ext, ext);
            assert_eq!(s.capacity, cap);
        }
    }
}
```

Run: `cargo test -p ra2-core theater`
Expected: 10 passed

- [ ] **Step 3: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 全绿;合成 **220**(210 + 10)

```bash
git add crates/core/src/format/theater.rs crates/core/src/format/mod.rs
git commit -m "feat(core): TheaterIni 顺序探查(atoi/保留槽/LastTilesInSet 引擎规则)+ 六剧场常数表" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

- [ ] **Step 4: 真 INI 测试(ignored;创建 `crates/core/tests/theater_samples.rs`,Task 5 追加)**

```rust
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
/// 探针复算);LastTilesInSet 全零出现(REA 勘误语料断言);localmd urbannmd
/// 的 TilesInSet=o 脏数据实证(atoi → 0)。
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
        let blob = container.read(name).unwrap_or_else(|e| panic!("读 {name}: {e}"));
        let text = decode_map_text(blob);
        let ini = TheaterIni::parse(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(ini.slot_count(), cap, "{name}");
        // REA 断言的是"活键零出现";七份 INI 均带 ;LastTilesInSet = N 注释模板行
        // (temperatmd 24 行,含 ;; 双分号变体)——注释行不计(2026-11-05 实证)
        assert!(
            !text.lines().any(|l| {
                let t = l.trim_start();
                !t.starts_with(';') && t.to_ascii_lowercase().contains("lasttilesinset")
            }),
            "{name} 出现活键 LastTilesInSet(REA 勘误语料断言;注释模板行不计)"
        );
    }
    // 脏数据实证:localmd urbannmd set0105 行=TilesInSet = o(等号两侧带空格,
    // 2026-11-05 字节级实证;atoi→0,容量 1175 已由上方断言钉)
    let raw = String::from_utf8_lossy(localmd.read("urbannmd.ini").unwrap());
    assert!(raw.contains("TilesInSet = o"), "set0105 脏数据应存在(研究 §3)");
}
```

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test theater_samples -- --ignored`
Expected: 1 passed

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored`
Expected: 全绿;ignored **29**(28 + 1)

```bash
git add crates/core/tests/theater_samples.rs
git commit -m "test(core): 六份 md 剧场 INI 真测(容量/LastTilesInSet 零出现/TilesInSet=o 脏数据)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 5: `theater.rs` 后半——TheaterAssets 资产装载器(引擎挂载序 + 递归嵌套回退)

**Files:**
- Modify: `crates/core/src/format/theater.rs`(追加装载器层与其单测)
- Modify: `crates/core/tests/theater_samples.rs`(追加 2 个 ignored 测试)

**Interfaces:**
- Consumes: `MixArchive`(Task 1 后形态,含 `data_start()`/`digest_flag()`)、`mix_name_id`(crc.rs)、`FormatError`
- Produces(供 Task 6/7/8 使用):
  - `pub const MAX_DESCEND_BYTES: usize = 256 * 1024 * 1024;`(嵌套下钻体量上限,spec"体量上限防炸";语料最大嵌套 35.7 MB 远在其下)
  - `pub struct TheaterAssets` + `open(assets_dir: &std::path::Path) -> Result<Self, FormatError>`
  - `lookup(&self, name: &str) -> Option<&[u8]>`(按名,mix_name_id;借切片)
  - `lookup_id(&self, id: u32) -> Option<&[u8]>`(匿名嵌套库/任意 id 直查)
  - `origin(&self, name: &str) -> Option<&str>`(诊断:命中容器标签)

- 挂载/优先级语义(**REA 勘误后定稿;首中即胜,不是后装覆盖**;勿改):
  - 挂载序 = `expandmd99..expandmd00`(根目录,高位先)> `ra2md.mix` > `ra2.mix`(两者必需,缺失 → Invalid)> `cachemd.mix` > `cache.mix`(可选)> `localmd.mix`(按名在 ra2md 条目中解析)> `local.mix`(在 ra2 中)> 根目录 `ecache*.mix`/`elocal*.mix`(文件名升序)> 每剧场 mix(下方 19 名清单序)。
  - **递归嵌套下钻为 mod 内容回退层**(必需路径:expandmd01 匿名嵌套 `#C31B4D61` 持 323 个 snow 瓦片):深度 ≤3、单容器 ≤ `MAX_DESCEND_BYTES`;**嵌套容器继承其根的挂载位、排在同根直挂条目之后**(排序键 `(pos, depth, seq)` 天然实现)。
  - 挂靠名(localmd/local/每剧场)在下钻时**拦截**——不随父容器层级注册,改挂到自身挂载位(引擎把 cachemd 排在 localmd 之前的次序才有意义)。
  - 注册:全部容器按 `(pos, depth, seq)` 排序后顺序收集 `(id, 容器, start, len)`,按 id **稳定排序**后每组取第一个 = 首中即胜;查询用排序 Vec + 二分(零 HashMap)。

- [ ] **Step 1: 写装载器(`theater.rs` 追加;完整代码)**

```rust
// ============ 装载器层(TheaterAssets;引擎挂载序,首中即胜)============

use crate::format::crc::mix_name_id;
use crate::format::mix::{MixArchive, MixEntry};
use std::collections::BTreeMap;

/// 嵌套下钻体量上限(spec"体量上限防炸";语料最大嵌套容器约 35.7 MB)。
pub const MAX_DESCEND_BYTES: usize = 256 * 1024 * 1024;

/// 嵌套深度上限(spec §4"深度 ≤3";顶层挂载为深度 0)。
const MAX_DEPTH: usize = 3;

/// 每剧场 mix 名单(研究表 1 语料观测;挂载序在 ecache/elocal 之后,
/// 列表内顺序仅为确定性)。localmd.mix/local.mix 的拦截名另有专位。
const THEATER_MIX_NAMES: [&str; 19] = [
    "isogen.mix",
    "isotemp.mix",
    "isosnow.mix",
    "isourb.mix",
    "temperat.mix",
    "urban.mix",
    "snow.mix",
    "isodes.mix",
    "isolun.mix",
    "isoubn.mix",
    "urbann.mix",
    "isogenmd.mix",
    "isotemmd.mix",
    "isourbmd.mix",
    "isosnomd.mix",
    "isodesmd.mix",
    "isolunmd.mix",
    "isoubnmd.mix",
    "snowmd.mix",
];

/// 一个已挂载容器(数据 + 解析后的索引表)。
struct Mounted {
    label: String,
    data: Vec<u8>,
    data_start: usize,
    entries: Vec<MixEntry>,
    pos: usize,
    depth: usize,
    seq: usize,
}

/// id → 数据索引项(start/len 为容器 data 内绝对区间)。
struct IndexEntry {
    id: u32,
    container: usize,
    start: usize,
    len: usize,
}

/// 剧场资产全集(引擎挂载序装载;查询首中即胜)。
pub struct TheaterAssets {
    containers: Vec<Mounted>,
    index: Vec<IndexEntry>,
}

/// 便宜前置判定(避免对每个数据条目做全量 MIX 解析;真正校验仍由
/// MixArchive::parse 完成)。
fn mix_preflight(blob: &[u8]) -> bool {
    if blob.len() < 10 {
        return false;
    }
    let flags = u32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]);
    let count = u16::from_le_bytes([blob[4], blob[5]]);
    if flags & 0x0002_0000 != 0 || flags & !0x0003_0000 != 0 {
        return false;
    }
    10 + 12 * count as usize <= blob.len()
}

/// 先序下钻:容器自身先入册,再递归嵌套(拦截名单命中者截流到
/// holdback_hits,由 open 在其专位挂载)。装载器非解析热路径,克隆为清晰让路。
fn walk(
    data: Vec<u8>,
    label: String,
    pos: usize,
    depth: usize,
    seq: &mut usize,
    out: &mut Vec<Mounted>,
    holdback_hits: &mut BTreeMap<&'static str, Vec<u8>>,
    holdback_ids: &BTreeMap<u32, &'static str>,
) {
    let mix = match MixArchive::parse(data.clone()) {
        Ok(m) => m,
        Err(_) => return,
    };
    let data_start = mix.data_start();
    let entries: Vec<MixEntry> = mix.entries().to_vec();
    let s = *seq;
    *seq += 1;
    out.push(Mounted {
        label,
        data,
        data_start,
        entries: entries.clone(),
        pos,
        depth,
        seq: s,
    });
    if depth >= MAX_DEPTH {
        return;
    }
    let owned = out.last().unwrap().data.clone();
    for e in &entries {
        let start = data_start + e.offset as usize;
        let end = start + e.size as usize;
        if end > owned.len() {
            continue;
        }
        let blob = &owned[start..end];
        if blob.len() > MAX_DESCEND_BYTES || !mix_preflight(blob) {
            continue;
        }
        if let Some(&name) = holdback_ids.get(&e.id) {
            holdback_hits.entry(name).or_insert_with(|| blob.to_vec());
            continue;
        }
        walk(
            blob.to_vec(),
            format!("{label:#{}}", e.id),
            pos,
            depth + 1,
            seq,
            out,
            holdback_hits,
            holdback_ids,
        );
    }
}
```


`open` 与查询访问器(同文件续):

```rust
impl TheaterAssets {
    /// 按引擎挂载序装载(spec §4 + REA 勘误;见本任务语义要点)。
    pub fn open(assets_dir: &std::path::Path) -> Result<Self, FormatError> {
        let mut roots: Vec<(usize, String, Vec<u8>)> = Vec::new();
        // expandmd99..expandmd00(pos = 99−nn,高位先)
        for nn in (0..=99u32).rev() {
            let name = format!("expandmd{nn:02}.mix");
            if let Ok(data) = std::fs::read(assets_dir.join(&name)) {
                roots.push((99 - nn as usize, name, data));
            }
        }
        // ra2md / ra2(必需)
        for (pos, name) in [(100usize, "ra2md.mix"), (101, "ra2.mix")] {
            let data = std::fs::read(assets_dir.join(name)).map_err(|e| {
                FormatError::Invalid(format!("必需资产 {name} 读取失败: {e}"))
            })?;
            roots.push((pos, name.to_string(), data));
        }
        // cachemd / cache(可选,缺席即跳)
        for (pos, name) in [(102usize, "cachemd.mix"), (103, "cache.mix")] {
            if let Ok(data) = std::fs::read(assets_dir.join(name)) {
                roots.push((pos, name.to_string(), data));
            }
        }
        // 根目录 ecache*/elocal*(文件名升序)
        let mut ecaches: Vec<String> = std::fs::read_dir(assets_dir)
            .map_err(|e| FormatError::Invalid(format!("资产目录不可读: {e}")))?
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| {
                n.ends_with(".mix")
                    && (n.starts_with("ecache") || n.starts_with("elocal"))
            })
            .collect();
        ecaches.sort();
        for (i, name) in ecaches.iter().enumerate() {
            if let Ok(data) = std::fs::read(assets_dir.join(name)) {
                roots.push((106 + i, name.clone(), data));
            }
        }
        // 拦截名单(localmd/local/每剧场):id → 名
        let mut holdback_ids: BTreeMap<u32, &'static str> = BTreeMap::new();
        holdback_ids.insert(mix_name_id("localmd.mix"), "localmd.mix");
        holdback_ids.insert(mix_name_id("local.mix"), "local.mix");
        for name in THEATER_MIX_NAMES {
            holdback_ids.insert(mix_name_id(name), name);
        }
        // 先序下钻全部挂载根
        let mut out: Vec<Mounted> = Vec::new();
        let mut holdback_hits: BTreeMap<&'static str, Vec<u8>> = BTreeMap::new();
        let mut seq = 0usize;
        for (pos, name, data) in roots {
            walk(
                data,
                name,
                pos,
                0,
                &mut seq,
                &mut out,
                &mut holdback_hits,
                &holdback_ids,
            );
        }
        // 拦截名按自身挂载位回挂(localmd 104 / local 105 / 每剧场 1000+)
        for (pos, name) in [(104usize, "localmd.mix"), (105, "local.mix")] {
            if let Some(data) = holdback_hits.remove(name) {
                walk(data, name.to_string(), pos, 0, &mut seq, &mut out, &mut holdback_hits, &holdback_ids);
            }
        }
        for (i, name) in THEATER_MIX_NAMES.iter().enumerate() {
            if let Some(data) = holdback_hits.remove(name) {
                walk(data, name.to_string(), 1000 + i, 0, &mut seq, &mut out, &mut holdback_hits, &holdback_ids);
            }
        }
        // (pos, depth, seq) 排序 → 顺序收集 → 按 id 稳定排序 → 每组取首个
        out.sort_by_key(|r| (r.pos, r.depth, r.seq));
        let mut candidates: Vec<(u32, usize, usize, usize)> = Vec::new();
        for (ci, rec) in out.iter().enumerate() {
            for e in &rec.entries {
                candidates.push((e.id, ci, rec.data_start + e.offset as usize, e.size as usize));
            }
        }
        candidates.sort_by_key(|c| c.0); // 稳定:同 id 保装载序首个 = 首中即胜
        let mut index: Vec<IndexEntry> = Vec::new();
        for (id, container, start, len) in candidates {
            if index.last().map(|last| last.id) == Some(id) {
                continue;
            }
            index.push(IndexEntry { id, container, start, len });
        }
        Ok(TheaterAssets { containers: out, index })
    }

    fn slot_of(&self, id: u32) -> Option<(usize, &IndexEntry)> {
        let i = self.index.binary_search_by_key(&id, |e| e.id).ok()?;
        Some((self.index[i].container, &self.index[i]))
    }

    /// 按名查找(文件名大小写/斜杠规则同 mix_name_id)。
    pub fn lookup(&self, name: &str) -> Option<&[u8]> {
        self.lookup_id(mix_name_id(name))
    }

    /// 按 id 直查(匿名嵌套库等)。
    pub fn lookup_id(&self, id: u32) -> Option<&[u8]> {
        let (c, e) = self.slot_of(id)?;
        Some(&self.containers[c].data[e.start..e.start + e.len])
    }

    /// 诊断:命中容器标签(如 ra2.mix / ra2.mix#0F5D1D99)。
    pub fn origin(&self, name: &str) -> Option<&str> {
        let (c, _) = self.slot_of(mix_name_id(name))?;
        Some(self.containers[c].label.as_str())
    }
}
```

> **偏差记录(实现时修正)**:`walk` 里 `format!("{label:#{}}", e.id)` 的格式串有误,实现时写为 `format!("{label}#{:08X}", e.id)`;`owned = out.last()...data.clone()` 的二次克隆可改为 `let owned = data.clone()`(walk 持有的原 Vec 在 push 时已克隆入册)——语义等价,取更直白写法。

- [ ] **Step 2: 装载器单测(追加到 Task 4 建立的 `mod tests` 末尾,6 个)**

```rust
    // —— 装载器单测辅助(临时目录;tag 隔离并行测试)——
    fn temp_assets(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("ra2-theater-{tag}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// 构造 flags=0 的最小合法 MIX(按名条目)。
    fn make_mix(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        let body_len: usize = entries.iter().map(|(_, c)| c.len()).sum();
        v.extend_from_slice(&(body_len as u32).to_le_bytes());
        let mut off = 0u32;
        for (name, c) in entries {
            v.extend_from_slice(&mix_name_id(name).to_le_bytes());
            v.extend_from_slice(&off.to_le_bytes());
            v.extend_from_slice(&(c.len() as u32).to_le_bytes());
            off += c.len() as u32;
        }
        for (_, c) in entries {
            v.extend_from_slice(c);
        }
        v
    }

    /// 同 make_mix,但 id 由调用方给定(匿名嵌套 mix 用)。
    fn make_mix_raw(entries: &[(u32, &[u8])]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        let body_len: usize = entries.iter().map(|(_, c)| c.len()).sum();
        v.extend_from_slice(&(body_len as u32).to_le_bytes());
        let mut off = 0u32;
        for (id, c) in entries {
            v.extend_from_slice(&id.to_le_bytes());
            v.extend_from_slice(&off.to_le_bytes());
            v.extend_from_slice(&(c.len() as u32).to_le_bytes());
            off += c.len() as u32;
        }
        for (_, c) in entries {
            v.extend_from_slice(c);
        }
        v
    }

    fn put(dir: &std::path::Path, name: &str, bytes: &[u8]) {
        std::fs::write(dir.join(name), bytes).unwrap();
    }

    #[test]
    fn first_match_wins_expand_over_ra2md_over_ra2() {
        // REA 勘误:首中即胜,挂载序 expandmd* > ra2md > ra2
        let d = temp_assets("first-match");
        put(&d, "ra2.mix", &make_mix(&[("probe.dat", b"from-ra2")]));
        put(&d, "ra2md.mix", &make_mix(&[("probe.dat", b"from-ra2md")]));
        put(&d, "expandmd01.mix", &make_mix(&[("probe.dat", b"from-expand")]));
        let a = TheaterAssets::open(&d).unwrap();
        assert_eq!(a.lookup("probe.dat").unwrap(), b"from-expand");
        assert_eq!(a.origin("probe.dat"), Some("expandmd01.mix"));
    }

    #[test]
    fn cachemd_beats_localmd() {
        // cachemd(pos 102)> localmd(pos 104,自 ra2md 条目拦截回挂)
        let d = temp_assets("cachemd");
        let localmd_blob = make_mix(&[("probe.dat", b"from-localmd")]);
        put(&d, "ra2.mix", &make_mix(&[]));
        put(&d, "ra2md.mix", &make_mix(&[("localmd.mix", &localmd_blob)]));
        put(&d, "cachemd.mix", &make_mix(&[("probe.dat", b"from-cachemd")]));
        let a = TheaterAssets::open(&d).unwrap();
        assert_eq!(a.lookup("probe.dat").unwrap(), b"from-cachemd");
    }

    #[test]
    fn anonymous_nested_mix_descends() {
        // expandmd01 匿名嵌套 #C31B4D61 的必需路径缩小版
        let d = temp_assets("anon-nested");
        let inner = make_mix(&[("deep.dat", b"nested")]);
        put(&d, "ra2.mix", &make_mix_raw(&[(0xC31B_4D61, &inner)]));
        put(&d, "ra2md.mix", &make_mix(&[]));
        let a = TheaterAssets::open(&d).unwrap();
        assert_eq!(a.lookup("deep.dat").unwrap(), b"nested");
        assert!(a.origin("deep.dat").unwrap().contains("C31B4D61"));
    }

    #[test]
    fn depth_guard_stops_at_three() {
        // ra2(0) → m1(1) → m2(2) → m3(3) → m4(4 超限不下钻):d3 可查,d4 不可查
        let d = temp_assets("depth");
        let m4 = make_mix(&[("d4.dat", b"x4")]);
        let m3 = make_mix_raw(&[(0x1111_1111, &m4), (mix_name_id("d3.dat"), b"x3")]);
        let m2 = make_mix_raw(&[(0x2222_2222, &m3)]);
        let m1 = make_mix_raw(&[(0x3333_3333, &m2)]);
        put(&d, "ra2.mix", &make_mix_raw(&[(0x4444_4444, &m1)]));
        put(&d, "ra2md.mix", &make_mix(&[]));
        let a = TheaterAssets::open(&d).unwrap();
        assert_eq!(a.lookup("d3.dat").unwrap(), b"x3");
        assert!(a.lookup("d4.dat").is_none());
    }

    #[test]
    fn missing_required_root_is_err() {
        let d = temp_assets("missing-root");
        put(&d, "ra2.mix", &make_mix(&[])); // 缺 ra2md.mix → Invalid
        assert!(matches!(
            TheaterAssets::open(&d),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn max_descend_bytes_const_pinned() {
        // 语料最大嵌套容器约 35.7 MB;256 MiB 为防炸护栏(依据 §1)
        assert_eq!(MAX_DESCEND_BYTES, 268_435_456);
    }
```

Run: `cargo test -p ra2-core theater`
Expected: 16 passed(TheaterIni 10 + 装载器 6)

- [ ] **Step 3: 装载器真测(ignored;`theater_samples.rs` 追加 2 个)**

文件头 use 区追加 `use ra2_core::format::theater::TheaterAssets;`,文件末尾追加:

```rust
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
    assert_eq!(ini_blob.len(), 36_725, "应取 expandmd01 版而非 localmd 的 32,614 B");
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
```

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test theater_samples -- --ignored`
Expected: 3 passed(Task 4 的 1 + 新 2)

- [ ] **Step 4: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 全绿;合成 **226**(220 + 6)

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored`
Expected: 全绿;ignored **31**(29 + 2)

```bash
git add crates/core/src/format/theater.rs crates/core/tests/theater_samples.rs
git commit -m "feat(core): TheaterAssets 引擎挂载序装载器(首中即胜 + 递归嵌套回退,匿名嵌套 mix 必需路径)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 6: T9 重钉——map_samples 的 snow 容量测试改 INI 语义

**Files:**
- Modify: `crates/core/tests/map_samples.rs`(整段替换 `snow_tile_index_within_theater_mix_capacity`,删除其 `tmp_tile_capacity` 辅助函数;其余三个测试不动)

**Interfaces:**
- Consumes: `TheaterIni::parse`/`SlotFile`(Task 4)、`decode_map_text`、`MapFile`(M2)、`MixArchive`
- Produces: 无新 API;ignored 计数不变(4 个)

- 语义要点:T9 旧口径"SNOW 容量 = snow.mix + SNOWMD.MIX 的 TMP 瓦片总数 10016"来自被裁决推翻的目录序规则(研究 §3);新口径 = 剧场 INI 枚举容量 **964**(snowmd.ini,2026-11-05 探针复算)。锚点保持:语料 106 张 SNOW 图 max tile = 963 = 容量−1;963 号槽文件 = clifxl04.sno(位于 isosnomd,跨容器按名可达)。

- [ ] **Step 1: 整段替换测试(完整代码)**

删除 `snow_tile_index_within_theater_mix_capacity` 与 `tmp_tile_capacity`,原位置写入:

```rust
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
    assert!(snow_maps >= 100, "Maps/** 下应有 106 张 SNOW 图,实得 {snow_maps}");
    assert!(max_tile >= 0, "至少一张 SNOW 图应有瓦片");
    assert_eq!(max_tile, 963, "语料实测最大 SNOW tile(交流意见.map)");
}
```

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test map_samples -- --ignored`
Expected: 4 passed(计数不变;改名重钉)

- [ ] **Step 2: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 全绿;合成 **226**(不变)

```bash
git add crates/core/tests/map_samples.rs
git commit -m "test(core): T9 重钉——snow 容量测试改 INI 枚举语义(10016→964,研究 §3)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 7: CLI extract-tile 真瓦片渲染(INI 反查 + 槽位 + 菱形铺canvas + 调色板)

**Files:**
- Modify: `tools/ra2-map/src/main.rs`(usage 文本、`load_theater_palette` 重构、`run_extract_tile` 整体替换、use 区)
- Modify: `tools/ra2-map/tests/cli.rs`(ignored 活体测试重写:三剧场真瓦片 + 像素多值断言 + DESERT 负向)

**Interfaces:**
- Consumes: `TheaterIni`/`theater_spec`/`TheaterAssets`/`SlotFile`(Task 4/5)、`TsTmpFile`(Task 3)、`decode_map_text`、`Palette::rgb8`(M2)、`MapFile`/`IsoTile`(M2)
- Produces: CLI 契约(spec §7)——`extract-tile <file.map> <x> <y> -o <file.png> [--assets <目录>]` 签名不变、退出码 0/1/2 不变;语义升级为真瓦片;stderr 增补链路诊断(剧场/INI 容量/tile_id→文件名/槽位/来源容器)。

- 语义要点(spec §4/§7 + 依据 §5/§6 + 歧义裁决 #1/#6,勿改):
  - 渲染链:`(x,y)` → MapFile 瓦片记录 `(tile_id, sub_tile)` → `theater_spec(剧场)` → `TheaterAssets::open(assets)` → `lookup(ini_name)` → `TheaterIni::parse` → `file_name(tile_id, ext)` → `lookup(文件名)` → `TsTmpFile::parse` → `slot(sub_tile)` → 菱形像素。
  - 铺canvas:cx×cy 画布,行 r(0..cy)宽 `4·min(r+1, cy−1−r)`(勘误 2026-11-05:引擎 .data 两张 60×30 mask 表 0x3E2B5C/0x3EC4C8 行宽 [4,8,…,60,56,…,4,0] 逐行铁证;原 `4·min(r, cy−1−r)+2` 系误传作废),起始列 `(cx−w)/2`,行外区 = 调色板 0 号色(spec §4"索引 0 行外区照调色板 0 号色");`cx != 2·cy` → 退出 1(歧义裁决 #6;语料 13,020 个 TS TMP 全部 60×30,依据 §5)。
  - 调色板:先 `TheaterAssets::lookup(iso_palette_file)`(localmd/expandmd/ecache 等任意容器),回退 M2 链(ra2.mix → 匿名库 id 0x3B5A96DE → 按名);TEMPERATE/SNOW/URBAN 在本机可解,DESERT/NEWURBAN/LUNAR 报缺调色板退出 1(歧义裁决 #1)。
  - 退出 1 的全部路径(均有明确 stderr):格无记录 / tile<0 / 资产目录缺 ra2(ra2md) / 缺调色板 / 缺 INI / INI Err(LastTilesInSet 死路) / tile_id 界外 / 保留槽 / 文件未出货(stderr 给集合名,spec §4) / 空槽 / 解析失败 / 非 2:1 画布 / 写 PNG 失败。

- [ ] **Step 1: main.rs 导入、usage 与调色板函数**

use 区替换(原 `use ra2_core::format::map::meta::Theater;` 等保留,新增/删除如下):

```rust
use ra2_core::format::map::mapdoc::decode_map_text;
use ra2_core::format::map::meta::Theater;
use ra2_core::format::map::objects::ObjectClass;
use ra2_core::format::map::MapFile;
use ra2_core::format::mix::MixArchive;
use ra2_core::format::pal::Palette;
use ra2_core::format::theater::{theater_spec, SlotFile, TheaterAssets, TheaterIni};
use ra2_core::format::theater_tmp::TsTmpFile;
use std::io::Write;
use std::process::ExitCode;

mod png;
```

删除原 `const PALETTE_MIX_ID: u32 = 0x3B5A_96DE;` 常量(并入下方函数)。`usage()` 替换为:

```rust
fn usage() -> ! {
    eprintln!("用法: ra2-map info <file.map>");
    eprintln!("      ra2-map extract-tile <file.map> <x> <y> -o <file.png> [--assets <游戏目录>]");
    eprintln!("说明: extract-tile 输出该格真瓦片位图(剧场 INI 反查 → TS TMP 槽位 →");
    eprintln!("      60×30 等距菱形 → 剧场调色板上色;行外区为调色板 0 号色)。");
    eprintln!("      缺 [--assets] 时取环境变量 RA2_ASSETS_DIR。DESERT/NEWURBAN/LUNAR");
    eprintln!("      的 YR 调色板不在本机安装中,将报缺资源(退出码 1)。");
    std::process::exit(2);
}
```

`load_theater_palette` 整体替换(签名改为借用 TheaterAssets):

```rust
/// 剧场 ISO 调色板:先全容器按名查(localmd/expandmd/ecache 等任意位置),
/// 回退 M2 链(ra2.mix → 匿名调色板库 id 0x3B5A96DE → 按名)。
fn load_theater_palette(assets: &TheaterAssets, theater: Theater) -> Result<Palette, String> {
    let name = theater.iso_palette_file();
    if let Some(bytes) = assets.lookup(name) {
        return Palette::parse(bytes)
            .map_err(|e| format!("调色板 {name} 768B 校验失败: {e}"));
    }
    const PALETTE_MIX_ID: u32 = 0x3B5A_96DE;
    let blob = assets
        .lookup_id(PALETTE_MIX_ID)
        .ok_or_else(|| format!("调色板库(id 3B5A96DE)不可达,且 {name} 按名未找到"))?
        .to_vec();
    let lib = MixArchive::parse(blob).map_err(|e| format!("解析调色板库: {e}"))?;
    let pal = lib.read(name).map_err(|_| {
        format!("{name} 在本机安装缺席(DESERT/NEWURBAN/LUNAR 的 YR 调色板;活体验收用 TEMPERATE/SNOW/URBAN)")
    })?;
    Palette::parse(pal).map_err(|e| format!("调色板 768B 校验失败: {e}"))
}
```

- [ ] **Step 2: `run_extract_tile` 整体替换(完整函数)**

```rust
fn run_extract_tile(
    map_path: &str,
    x: i64,
    y: i64,
    out_path: &str,
    assets_opt: Option<&str>,
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
    let assets_dir = assets_opt
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var("RA2_ASSETS_DIR").ok().map(std::path::PathBuf::from));
    let Some(assets_dir) = assets_dir else {
        eprintln!("需要 --assets 或环境变量 RA2_ASSETS_DIR 指向游戏目录");
        return ExitCode::from(1);
    };
    let spec = theater_spec(m.map_meta.theater);
    let assets = match TheaterAssets::open(&assets_dir) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("资产装载失败: {e}");
            return ExitCode::from(1);
        }
    };
    let palette = match load_theater_palette(&assets, m.map_meta.theater) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("剧场调色板解析失败: {e}");
            return ExitCode::from(1);
        }
    };
    let ini_bytes = match assets.lookup(spec.ini_name) {
        Some(b) => b,
        None => {
            eprintln!("资产缺剧场表 {}", spec.ini_name);
            return ExitCode::from(1);
        }
    };
    let ini = match TheaterIni::parse(&decode_map_text(ini_bytes)) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("解析 {}: {e}", spec.ini_name);
            return ExitCode::from(1);
        }
    };
    let set_label = ini
        .set_of(tile.tile as usize)
        .map(|s| match s.name {
            Some(n) => n,
            None => format!("set{:04}", s.number),
        })
        .unwrap_or_else(|| "-".to_string());
    let slot_file = match ini.file_name(tile.tile as usize, spec.ext) {
        Some(sf) => sf,
        None => {
            eprintln!(
                "tile_id {} 越界: {} 界内槽位 {}(spec 容量 {})",
                tile.tile,
                ini.slot_count(),
                ini.slot_count(),
                spec.capacity
            );
            return ExitCode::from(1);
        }
    };
    let file_name = match slot_file {
        SlotFile::Reserved => {
            eprintln!("tile_id {} 是保留槽(集合 {set_label},无自有文件)", tile.tile);
            return ExitCode::from(1);
        }
        SlotFile::File(name) => name,
    };
    let blob = match assets.lookup(&file_name) {
        Some(b) => b,
        None => {
            eprintln!("瓦片文件 {file_name} 未出货(缺文件槽;集合 {set_label})");
            return ExitCode::from(1);
        }
    };
    let origin = assets.origin(&file_name).unwrap_or("?").to_string();
    let tmp = match TsTmpFile::parse(blob) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("解析 {file_name}: {e}");
            return ExitCode::from(1);
        }
    };
    let sub = tile.sub_tile as usize;
    let slot = match tmp.slot(sub) {
        Some(s) => s,
        None => {
            eprintln!("{file_name} 槽 {sub} 为空槽(槽网格 {}×{})", tmp.dims().0, tmp.dims().1);
            return ExitCode::from(1);
        }
    };
    let (cbx, cby, cx, cy) = tmp.dims();
    if cx != 2 * cy {
        eprintln!(
            "{file_name} 画布 {cx}×{cy} 非 2:1 等距菱形,M3 渲染仅支持 2:1(语料全部 60×30)"
        );
        return ExitCode::from(1);
    }
    // 菱形铺入 cx×cy 画布:行 r 宽 4·min(r+1, cy−1−r)(引擎 mask 表铁证,
    // 末行宽 0 全背景),起始列 (cx−w)/2;行外区 = 调色板 0 号色(spec §4)
    let diamond = slot.diamond_pixels();
    let bg = palette.rgb8(0);
    let mut rgb = Vec::with_capacity(3 * cx as usize * cy as usize);
    let mut off = 0usize;
    for r in 0..cy as usize {
        let w = 4 * (r + 1).min(cy as usize - 1 - r);
        let x0 = (cx as usize - w) / 2;
        for c in 0..cx as usize {
            let color = if c >= x0 && c < x0 + w {
                let idx = diamond[off] as usize;
                off += 1;
                palette.rgb8(idx)
            } else {
                bg
            };
            rgb.extend_from_slice(&color);
        }
    }
    debug_assert_eq!(off, diamond.len());
    match png::write_png_file(std::path::Path::new(out_path), cx, cy, &rgb) {
        Ok(()) => {
            eprintln!(
                "已写 {out_path}: {file_name} 槽 {sub}(剧场 {}, INI 容量 {}, 画布 {cx}×{cy}, 槽网格 {cbx}×{cby}, 来源 {origin})",
                format!("{:?}", m.map_meta.theater),
                ini.slot_count(),
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("写 {out_path} 失败: {e}");
            ExitCode::from(1)
        }
    }
}
```

Run: `cargo clippy -p ra2-map --all-targets -- -D warnings`
Expected: 零告警(`run_info`/`main` 不动;`Theater` 的 Debug derive 已有)

- [ ] **Step 3: cli.rs 活体测试重写(删旧 `real_maps_info_and_extract_tile`,换下列;其余 5 个合成测试不动)**

`tools/ra2-map/tests/cli.rs` 追加辅助与替换测试:

```rust
/// 解码 png.rs 的单 IDAT(zlib store)→ 原始扫描行(每行 1 过滤字节 + 3·w)。
fn decode_store_idat(png: &[u8]) -> Vec<u8> {
    let mut pos = 8usize;
    let mut idat: Vec<u8> = Vec::new();
    while pos + 12 <= png.len() {
        let len = u32::from_be_bytes(png[pos..pos + 4].try_into().unwrap()) as usize;
        let kind = &png[pos + 4..pos + 8];
        if kind == b"IDAT" {
            idat.extend_from_slice(&png[pos + 8..pos + 8 + len]);
        }
        if kind == b"IEND" {
            break;
        }
        pos += 12 + len;
    }
    assert_eq!(&idat[..2], &[0x78, 0x01], "zlib store 头");
    let mut out = Vec::new();
    let mut p = 2usize;
    loop {
        let header = idat[p];
        let n = u16::from_le_bytes(idat[p + 1..p + 3].try_into().unwrap()) as usize;
        out.extend_from_slice(&idat[p + 5..p + 5 + n]);
        p += 5 + n;
        if header & 1 == 1 {
            break; // BFINAL
        }
    }
    out
}

/// 不同 RGB 三元组数(跳过每行过滤字节)。
fn distinct_colors(scanlines: &[u8], w: u32) -> usize {
    use std::collections::BTreeSet;
    let stride = 1 + 3 * w as usize;
    let mut set = BTreeSet::new();
    let mut y = 0usize;
    while y * stride + stride <= scanlines.len() {
        let row = &scanlines[y * stride + 1..y * stride + stride];
        for px in row.as_chunks::<3>().0 {
            set.insert(*px);
        }
        y += 1;
    }
    set.len()
}

/// 真图活体验收(M3;spec §6-6 按歧义裁决 #1 落地):TEMPERATE/SNOW/URBAN
/// 各一图 extract-tile 产出 60×30、像素多值的 PNG(非占位非单色);
/// DESERT 图因 isodes.pal 缺席退出 1(stderr 提调色板);info 三剧场不变。
#[test]
#[ignore]
fn real_maps_extract_tile_three_theaters() {
    use ra2_core::format::map::MapFile;
    let assets = std::env::var("RA2_ASSETS_DIR").expect("需要 RA2_ASSETS_DIR");
    let dir = temp_dir("real-m3");
    for rel in [
        "Maps/Cooperative/c1a01md.map", // TEMPERATE
        "Maps/Cooperative/c1a03md.map", // SNOW
        "Maps/Standard/hillbtwn.map",   // URBAN
    ] {
        let map_path = std::path::Path::new(&assets).join(rel);
        // 取首个有效瓦片格(确定性;不硬编码坐标)
        let data = std::fs::read(&map_path).unwrap();
        let m = MapFile::parse(&data).unwrap();
        let rec = m
            .tiles
            .records()
            .iter()
            .find(|r| r.tile >= 0)
            .expect("至少一条瓦片记录");
        let out_path = dir.join("tile.png");
        let out = Command::new(bin())
            .args([
                "extract-tile",
                map_path.to_str().unwrap(),
                &rec.x.to_string(),
                &rec.y.to_string(),
                "-o",
                out_path.to_str().unwrap(),
            ])
            .env("RA2_ASSETS_DIR", &assets)
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(0),
            "{rel}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let png = std::fs::read(&out_path).unwrap();
        assert_eq!(png[16..20], 60u32.to_be_bytes(), "{rel}");
        assert_eq!(png[20..24], 30u32.to_be_bytes(), "{rel}");
        let scanlines = decode_store_idat(&png);
        assert!(distinct_colors(&scanlines, 60) >= 2, "{rel} 应像素多值(非占位)");
        let _ = std::fs::remove_file(&out_path);
    }
    // DESERT 负向(歧义裁决 #1):isodes.pal 缺席 → 退出 1
    let map_path = std::path::Path::new(&assets).join("Maps/Cooperative/c2s02md.map");
    let data = std::fs::read(&map_path).unwrap();
    let m = MapFile::parse(&data).unwrap();
    let rec = m.tiles.records().iter().find(|r| r.tile >= 0).unwrap();
    let out_path = dir.join("tile-desert.png");
    let out = Command::new(bin())
        .args([
            "extract-tile",
            map_path.to_str().unwrap(),
            &rec.x.to_string(),
            &rec.y.to_string(),
            "-o",
            out_path.to_str().unwrap(),
        ])
        .env("RA2_ASSETS_DIR", &assets)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("调色板"), "{stderr}");
    assert!(!out_path.exists());
    // info 三剧场冒烟(info 不需要调色板,M2 契约)
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
        assert!(
            stdout.contains(&format!("theater={theater}")),
            "{rel}: {stdout}"
        );
    }
}
```

Run: `cargo test -p ra2-map`
Expected: 8 passed(5 既有 + 3 png;ignored 仍过滤)

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-map -- --ignored`
Expected: 1 passed(重写的活体测试)

- [ ] **Step 4: 全套回归后 Commit**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 全绿;合成 **226**(不变)

```bash
git add tools/ra2-map/src/main.rs tools/ra2-map/tests/cli.rs
git commit -m "feat(cli): extract-tile 真瓦片渲染(INI 反查→TS TMP 槽→菱形铺canvas→调色板;YR 三剧场负向退出 1)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 8: M3 里程碑验收(807 图闭合 + 缺文件槽精确账 + 全量零 panic + 甲骨文)

**Files:**
- Create: `crates/core/tests/m3_acceptance.rs`
- Modify: `README.md`(追加"剧场瓦片层(M3)"小节)

**Interfaces:**
- Consumes: 全前序(`MapFile`、`TheaterIni`、`TheaterAssets`、`TsTmpFile`、`theater_spec`、`SlotFile`)
- Produces: 3 个 ignored 验收测试(spec §6 全项核对表见任务末);README 记录里程碑能力。

- 验收口径(spec §6,勿改):
  1. **闭合测试**:807 图全量,每瓦片记录 `(theater, tile_id, sub)` → tile_id 在 INI 界内(类 A)、文件现存(类 B,= 缺文件槽**语料零引用**)、非保留槽(类 C)、现存文件可解析(类 E)、`sub < cbx·cby`(类 D,**仅** MadHQ 机生成图白名单,界 ≤4 图/≤500 槽——宽裕防增长界,首跑实测值记入注释)。
  2. **缺文件槽精确账**:按 INI 全枚举(非仅引用),`File(name)` 而 lookup 失败的计数 = TEMPERATE 0 / URBAN 0 / DESERT 0 / SNOW 78 / NEWURBAN 383 / LUNAR 44(研究 §8);SNOW 缺失槽全部 ≥838。
  3. **全量零 panic**:六剧场现存 TS TMP 全量解析,snow 分布钉 459/323/88/16(研究 §3-5:isosnow / expandmd01#C31B4D61 / isosnomd / snow.mix);clear01.sno 字节闭合(1872 = 16+4+52+900+900,cbx=cby=1,cx=60,cy=30,首槽 RAW 900B + z 900B)。
  4. **甲骨文**:六容量(经装载器)、四锚点(snow[963]=clifxl04.sno / nu[1174]=sfbrdg01.ubn / tem[837]=ariz01.tem / lun[197]=Crevas06.lun)、[0]=clear01.sno、md 替换(base snow.ini 798 < 964)、urbannmd 取 expandmd01 版(36,725 B)。burnt01 帧 0 offset=56 与 lobrdb08 帧 1 offset=152 已由 Task 2 迁移后的 format_samples 断言覆盖,不重复。

- [ ] **Step 1: 创建 `crates/core/tests/m3_acceptance.rs`(头部 + 辅助函数)**

```rust
// SPDX-License-Identifier: GPL-3.0-only
//! M3 里程碑验收:807 图全量 (theater, tile_id, sub) → 文件闭合 + 缺文件槽
//! 精确账 + 六剧场现存瓦片全量零 panic + 甲骨文锚点。
//! 运行:RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test m3_acceptance -- --ignored

use ra2_core::format::map::mapdoc::decode_map_text;
use ra2_core::format::map::meta::Theater;
use ra2_core::format::map::MapFile;
use ra2_core::format::mix::MixArchive;
use ra2_core::format::theater::{theater_spec, SlotFile, TheaterAssets, TheaterIni};
use ra2_core::format::theater_tmp::TsTmpFile;
use std::collections::BTreeMap;

fn assets_dir() -> std::path::PathBuf {
    std::env::var("RA2_ASSETS_DIR")
        .expect("需要环境变量 RA2_ASSETS_DIR")
        .into()
}

/// 递归收集 RA2_ASSETS_DIR 下全部 .map(与 map_samples 同构;测试代码复制,不共享)。
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

fn theater_index(t: Theater) -> usize {
    match t {
        Theater::Temperate => 0,
        Theater::Snow => 1,
        Theater::Urban => 2,
        Theater::Desert => 3,
        Theater::NewUrban => 4,
        Theater::Lunar => 5,
    }
}

/// 剧场上下文:全槽位文件状态(INI 全枚举)。
enum SlotState {
    Reserved,
    File(String),
}

fn theater_slots(assets: &TheaterAssets, theater: Theater) -> Vec<SlotState> {
    let spec = theater_spec(theater);
    let ini_bytes = assets.lookup(spec.ini_name).expect("剧场 INI 可达");
    let ini = TheaterIni::parse(&decode_map_text(ini_bytes)).expect("剧场 INI 可解析");
    assert_eq!(ini.slot_count(), spec.capacity, "{}", spec.ini_name);
    let mut files = Vec::with_capacity(ini.slot_count());
    for idx in 0..ini.slot_count() {
        files.push(match ini.file_name(idx, spec.ext) {
            Some(SlotFile::File(name)) => SlotState::File(name),
            Some(SlotFile::Reserved) => SlotState::Reserved,
            None => panic!("界内槽位 {idx} 返回 None(不可能)"),
        });
    }
    files
}
```

- [ ] **Step 2: 闭合测试(追加)**

```rust
/// 807 图全量闭合(spec §6-1)。四类违例必须为零:
/// A 界外 / B 引用未出货槽 / C 引用保留槽 / E 现存文件解析失败;
/// 类 D(sub ≥ cbx·cby)仅允许 MadHQ 机生成图白名单(研究 §3),
/// 界 ≤4 图 / ≤500 槽为防增长宽裕界;【首跑实测值在此注释记录】。
#[test]
#[ignore]
fn m3_full_closure_807_maps() {
    let assets = TheaterAssets::open(&assets_dir()).unwrap();
    let theaters = [
        Theater::Temperate,
        Theater::Snow,
        Theater::Urban,
        Theater::Desert,
        Theater::NewUrban,
        Theater::Lunar,
    ];
    // 每剧场:槽位文件状态 + 现存文件 → cbx·cby 解析缓存
    let mut slots: Vec<Vec<SlotState>> = Vec::with_capacity(6);
    let mut slot_counts: Vec<BTreeMap<String, u32>> = Vec::with_capacity(6);
    for t in theaters {
        slots.push(theater_slots(&assets, t));
        slot_counts.push(BTreeMap::new());
    }
    let mut d_maps: BTreeMap<String, usize> = BTreeMap::new();
    let mut d_total = 0usize;
    let mut total_maps = 0usize;
    for p in map_files() {
        total_maps += 1;
        let data = std::fs::read(&p).unwrap();
        let m = MapFile::parse(&data).unwrap_or_else(|e| panic!("{}: {e:?}", p.display()));
        let ti = theater_index(m.map_meta.theater);
        for r in m.tiles.records() {
            if r.tile < 0 {
                continue;
            }
            let idx = r.tile as usize;
            if idx >= slots[ti].len() {
                panic!("类 A: {} tile_id {idx} 越界(槽位 {})", p.display(), slots[ti].len());
            }
            let name = match &slots[ti][idx] {
                SlotState::Reserved => {
                    panic!("类 C: {} 引用保留槽 {idx}", p.display());
                }
                SlotState::File(name) => name,
            };
            let blob = match assets.lookup(name) {
                Some(b) => b,
                None => panic!("类 B: {} 引用未出货槽 {idx}({name})", p.display()),
            };
            let counts = slot_counts[ti]
                .entry(name.clone())
                .or_insert_with(|| match TsTmpFile::parse(blob) {
                    Ok(t) => t.slot_count() as u32,
                    Err(e) => panic!("类 E: {} 解析 {name}: {e:?}", p.display()),
                });
            if (r.sub_tile as u32) >= *counts {
                let key = p.display().to_string();
                assert!(
                    key.contains("MadHQ"),
                    "类 D 非 MadHQ 图 sub 违例: {key} tile {idx} sub {}",
                    r.sub_tile
                );
                *d_maps.entry(key).or_insert(0) += 1;
                d_total += 1;
            }
        }
    }
    assert_eq!(total_maps, 807, "语料 807 图(变动时同步更新)");
    assert!(
        d_maps.len() <= 4,
        "MadHQ 残余白名单 ≤4 图,实得 {} 张: {d_maps:?}",
        d_maps.len()
    );
    assert!(
        d_total <= 500,
        "sub 残余违例总数 ≤500(防增长界),实得 {d_total}"
    );
}
```

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test m3_acceptance m3_full_closure -- --ignored`
Expected: 1 passed(首跑后把 d_maps/d_total 实测值写进测试注释,替换【首跑实测值】占位注释)

- [ ] **Step 3: 缺文件槽精确账 + 全量零 panic(追加两个测试)**

```rust
/// 缺文件槽精确账(spec §6-1,研究 §8):按 INI 全枚举(非仅引用),
/// File(name) 而资产查无的槽位计数 = TEMPERATE 0 / SNOW 78 / URBAN 0 /
/// DESERT 0 / NEWURBAN 383 / LUNAR 44;SNOW 缺失槽全部 ≥838
/// (ZMM Cliff Ice 系,研究 §3)。语料零引用已由闭合测试类 B 保证。
#[test]
#[ignore]
fn m3_unshipped_slot_census_exact() {
    let assets = TheaterAssets::open(&assets_dir()).unwrap();
    for (theater, missing_want) in [
        (Theater::Temperate, 0usize),
        (Theater::Snow, 78),
        (Theater::Urban, 0),
        (Theater::Desert, 0),
        (Theater::NewUrban, 383),
        (Theater::Lunar, 44),
    ] {
        let spec = theater_spec(theater);
        let slots = theater_slots(&assets, theater);
        let mut missing = 0usize;
        let mut missing_min = usize::MAX;
        for (idx, s) in slots.iter().enumerate() {
            if let SlotState::File(name) = s {
                if assets.lookup(name).is_none() {
                    missing += 1;
                    missing_min = missing_min.min(idx);
                }
            }
        }
        assert_eq!(missing, missing_want, "{} 缺文件槽", spec.ini_name);
        if theater == Theater::Snow && missing > 0 {
            assert!(missing_min >= 838, "SNOW 缺失槽应 ≥838,首例 {missing_min}");
        }
    }
}

/// 六剧场现存 TS TMP 全量零 panic(spec §6-2)+ snow 分布精确账
/// (研究 §3-5;INI 槽位反查口径:isosnow 459 + expandmd01#C31B4D61 323 +
/// isosnomd 88 + snow.mix 16 = 886 现存 = 964 − 78 缺失)。
/// 注意:容器结构判别(parse Ok 计数)口径**不可用**——isosnow 共 1013 条
/// 含非瓦片精灵(2026-11-05 探针裁定)。
/// 分布失配时:只读探针重测、更新常数、commit 注明,不得静默放宽(M2 惯例)。
#[test]
#[ignore]
fn m3_existing_tiles_parse_zero_panic_and_snow_distribution() {
    fn load(name: &str) -> MixArchive {
        let p = assets_dir().join(name);
        let data = std::fs::read(&p).unwrap_or_else(|e| panic!("读取 {}: {e}", p.display()));
        MixArchive::parse(data).expect("解析 MIX")
    }
    let ra2 = load("ra2.mix");
    let ra2md = load("ra2md.mix");
    let expand = load("expandmd01.mix");
    let isosnow = MixArchive::parse(ra2.read("isosnow.mix").unwrap().to_vec()).unwrap();
    let snow_mix = MixArchive::parse(ra2.read("snow.mix").unwrap().to_vec()).unwrap();
    let isosnomd = MixArchive::parse(ra2md.read("isosnomd.mix").unwrap().to_vec()).unwrap();
    let anon = MixArchive::parse(expand.read_by_id(0xC31B_4D61).unwrap().to_vec()).unwrap();
    // snow 分布精确账(研究 §3-5;**INI 槽位反查口径**,2026-11-05 探针裁定:
    // 964−78 缺失 = 886 现存 = isosnow 459 + expandmd01#C31B4D61 323 +
    // isosnomd 88 + snow.mix 16。容器结构判别口径会数错——isosnow 共 1013 条
    // 含非瓦片精灵——禁止用"容器内 parse Ok 计数"替代)
    let assets = TheaterAssets::open(&assets_dir()).unwrap();
    let snow_ini =
        TheaterIni::parse(&decode_map_text(assets.lookup("snowmd.ini").unwrap())).unwrap();
    let mut by_origin: BTreeMap<String, usize> = BTreeMap::new();
    let mut missing = 0usize;
    for idx in 0..snow_ini.slot_count() {
        if let Some(SlotFile::File(name)) = snow_ini.file_name(idx, "sno") {
            if assets.lookup(&name).is_some() {
                let o = assets.origin(&name).unwrap().to_string();
                *by_origin.entry(o).or_insert(0) += 1;
            } else {
                missing += 1;
            }
        }
    }
    assert_eq!(missing, 78, "snow 缺文件槽(与缺槽精确账测试互证)");
    assert_eq!(by_origin.get("isosnow.mix"), Some(&459), "分布键: {by_origin:?}");
    assert_eq!(by_origin.get("isosnomd.mix"), Some(&88));
    assert_eq!(by_origin.get("snow.mix"), Some(&16));
    let anon_n = by_origin
        .iter()
        .find(|(k, _)| k.starts_with("expandmd01.mix#"))
        .map(|(_, &v)| v)
        .unwrap_or(0);
    assert_eq!(anon_n, 323, "匿名嵌套 mix 分布键: {by_origin:?}");
    assert_eq!(by_origin.values().sum::<usize>(), 886);
    // snow 四容器全量零 panic(分布口径之外的结构层兜底)
    for mix in [&isosnow, &snow_mix, &isosnomd, &anon] {
        for e in mix.entries() {
            let _ = TsTmpFile::parse(mix.read_by_id(e.id).unwrap());
        }
    }
    // clear01.sno 字节闭合(spec §6-2 抽样):1872 = 16+4·1+52+900+900
    let clear = TsTmpFile::parse(isosnow.read("clear01.sno").unwrap()).unwrap();
    assert_eq!(clear.dims(), (1, 1, 60, 30));
    assert_eq!(clear.slot_count(), 1);
    let s = clear.slot(0).unwrap();
    assert_eq!(s.diamond_pixels().len(), 900);
    assert_eq!(s.z_data().map(|z| z.len()), Some(900));
    // 其余剧场容器全量尝试解析:只求零 panic(Err = SHP 精灵等非瓦片,合法)
    for (mix, names) in [
        (
            &ra2,
            [
                "isogen.mix",
                "isotemp.mix",
                "isourb.mix",
                "temperat.mix",
                "urban.mix",
            ],
        ),
        (
            &ra2md,
            [
                "isodes.mix",
                "isolun.mix",
                "isoubn.mix",
                "urbann.mix",
                "isogenmd.mix",
                "isotemmd.mix",
                "isourbmd.mix",
                "isodesmd.mix",
                "isolunmd.mix",
                "isoubnmd.mix",
            ],
        ),
    ] {
        for name in names {
            let m = MixArchive::parse(mix.read(name).unwrap().to_vec()).unwrap();
            for e in m.entries() {
                let _ = TsTmpFile::parse(m.read_by_id(e.id).unwrap());
            }
        }
    }
}
```

- [ ] **Step 4: 甲骨文锚点(追加)**

```rust
/// 甲骨文(spec §6-4):六容量经装载器互证(含 urbannmd 取 expandmd01 版)、
/// 四锚点反查 + 文件现存、[0]=clear01、md 替换(base snow.ini 798 < md 964)。
/// burnt01/lobrdb08 帧偏移锚点在 format_samples(Task 2 迁移)已钉,不重复。
#[test]
#[ignore]
fn m3_oracle_anchors() {
    let assets = TheaterAssets::open(&assets_dir()).unwrap();
    for (theater, cap) in [
        (Theater::Temperate, 838usize),
        (Theater::Snow, 964),
        (Theater::Urban, 1081),
        (Theater::Desert, 726),
        (Theater::NewUrban, 1189),
        (Theater::Lunar, 198),
    ] {
        let spec = theater_spec(theater);
        let ini = TheaterIni::parse(&decode_map_text(assets.lookup(spec.ini_name).unwrap())).unwrap();
        assert_eq!(ini.slot_count(), cap, "{}", spec.ini_name);
    }
    // urbannmd 双版本:装载器首中即胜取 expandmd01(36,725 B > localmd 32,614 B)
    assert_eq!(assets.lookup("urbannmd.ini").unwrap().len(), 36_725);
    // 四锚点 + [0]=Clear01
    let snow = TheaterIni::parse(&decode_map_text(assets.lookup("snowmd.ini").unwrap())).unwrap();
    assert_eq!(snow.file_name(963, "sno"), Some(SlotFile::File("clifxl04.sno".into())));
    assert!(assets.lookup("clifxl04.sno").is_some());
    assert_eq!(snow.set_of(963).unwrap().number, 82);
    // [0] 锚:INI FileName 原文保留大小写(探针:snow[0]=Clear01.sno)
    assert_eq!(
        snow.file_name(0, "sno"),
        Some(SlotFile::File("Clear01.sno".into()))
    );
    let nu = TheaterIni::parse(&decode_map_text(assets.lookup("urbannmd.ini").unwrap())).unwrap();
    assert_eq!(nu.file_name(1174, "ubn"), Some(SlotFile::File("sfbrdg01.ubn".into())));
    assert!(assets.lookup("sfbrdg01.ubn").is_some());
    let tem = TheaterIni::parse(&decode_map_text(assets.lookup("temperatmd.ini").unwrap())).unwrap();
    assert_eq!(tem.file_name(837, "tem"), Some(SlotFile::File("ariz01.tem".into())));
    assert!(assets.lookup("ariz01.tem").is_some());
    assert_eq!(tem.set_of(837).unwrap().number, 81);
    let lun = TheaterIni::parse(&decode_map_text(assets.lookup("lunarmd.ini").unwrap())).unwrap();
    assert_eq!(lun.file_name(197, "lun"), Some(SlotFile::File("Crevas06.lun".into())));
    assert!(assets.lookup("Crevas06.lun").is_some());
    assert_eq!(lun.set_of(197).unwrap().number, 84);
    // md 整体替换 base:base snow.ini(ra2 → local)容量 798 < md 964(研究 §3-2)
    let base = TheaterIni::parse(&decode_map_text(assets.lookup("snow.ini").unwrap())).unwrap();
    assert_eq!(base.slot_count(), 798);
    assert!(base.slot_count() < snow.slot_count());
}
```

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test m3_acceptance -- --ignored`
Expected: 3 passed(闭合 + 缺槽账 + 零 panic)再 `--test m3_acceptance m3_oracle -- --ignored` 1 passed(共 4 个 ignored 测试)

- [ ] **Step 5: README 追加(插在 `## ra2-mix` 之前)**

```markdown
## 剧场瓦片层(M3)

`ra2-core::format` 闭合瓦片解析链:MIX 摘要位 SHA-1 判别(修复幻影 digest
错位;46 容器语料全绿)、`shp_ts`(TS SHP 帧,RAW 回退)、`theater_tmp`
(TS TMP:16B 头/槽表/52B 头/RAW 菱形 + z)、`theater`(剧场 INI 顺序
探查→全局索引→文件名反查;引擎挂载序资产装载器,递归嵌套 mix 下钻;
六剧场常数表)。807 图全量 (theater, tile_id, sub)→文件闭合,缺文件槽
精确账(SNOW 78/NEWURBAN 383/LUNAR 44,语料零引用),六剧场现存瓦片
全量零 panic。`ra2-map extract-tile` 输出真瓦片位图(2:1 等距菱形、
调色板上色;YR 三剧场调色板缺席时退出 1)。规格依据见
`docs/superpowers/research/`,实施计划见 `docs/superpowers/plans/`。
```

- [ ] **Step 6: 终账(计划级完成定义核对)**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 全绿;**合成 226 passed**

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored`
Expected: 全绿;**ignored 35 passed**(既有 27 + mix_integration 1 + theater_samples 3 + m3_acceptance 4;map_samples 4 与 ra2-map cli 1 为改写,计数不变)

Run: `git ls-files | grep -iE "\.(map|mix|pal|shp|tmp|csf|vxl|hva|aud|sno|tem|urb|des|lun|ubn)$" || echo "clean"`
Expected: `clean`(仓库无 EA 资产)

```bash
git add crates/core/tests/m3_acceptance.rs README.md
git commit -m "test(core): M3 里程碑验收(807 图闭合 + 缺文件槽精确账 + 全量零 panic + 甲骨文)+ README" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

## 完成定义(计划级验收)

1. `cargo test --workspace` 全绿:**226 个合成测试**(基线 192 + Task 1 的 2 + Task 2 的 3 + Task 3 的 13 + Task 4 的 10 + Task 5 的 6;Task 6/7/8 只改写不新增;Task 1 勘误后由 4 改 2)。
   `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored` 全绿:**35 个真实资产测试**(基线 27 + Task 1 语料 1 + Task 4 真测 1 + Task 5 真测 2 + Task 8 验收 4;Task 2/6/7 为改写,计数不变)。
2. `cargo fmt --all --check` 与 `cargo clippy --all-targets -- -D warnings` 零告警;debug 溢出检查下零 panic。
3. spec §6 六项验收映射:1→Task 8 闭合测试(白名单界 + 类 A/B/C/E 零);2→Task 8 零 panic 扫描 + clear01 字节闭合;3→Task 1/3/4 敌意合成集(mix 双分支、TS TMP 守卫、INI 坏键);4→Task 8 甲骨文(Task 2 帧偏移、Task 4/8 六容量四锚点、Task 6 的 963=容量−1、md 替换 798<964);5→fmt/clippy/GPL/零依赖/资产零入库;6→Task 7 三剧场活体 PNG + DESERT 负向(歧义裁决 #1)。
4. 仓库不含任何 EA 资产(`fixtures/` 已忽略;`git ls-files` 审计通过)。
5. REA 勘误三项全部落地并有测试:挂载序首中即胜(Task 5 单测)、顺序探查 + LastTilesInSet(Task 4 单测 + 真测语料断言)、每剧场 mix 拦截回挂(Task 5 单测 + 真测)。

## 附录:Self-Review(计划自查,2026-11-05)

**spec 覆盖检查**(对照设计 spec §3/§4/§6 与研究 §0-§10):

- §3 做清单:mix 幻影 digest 修复 → Task 1 ✓;tmp.rs 更名重定域 + RAW 回退 → Task 2 ✓;TS TMP 解析器 → Task 3 ✓;theater.rs 三层(INI 表/常数表/装载器)→ Task 4/5 ✓;T9 重钉 → Task 6 ✓;CLI extract-tile → Task 7 ✓;验收 → Task 8 ✓;REA 并行抽查 → 非代码交付物,勘误已并入(Global Constraints + Task 4/5 语义)✓
- §3 不做清单:整图渲染/wgpu、z 着色与 terrain/ramp 语义枚举、overlay/smock、rules、写回、TS SHP remap/阴影、非 2:1 渲染(歧义裁决 #6)——全部排除且 CLI 对越界给出明确 stderr ✓
- §4 模块结构:mix.rs(修复,判别信息进 digest_flag 访问器——REA 勘误后为咨询位透传,parse 入参形状不变)✓;shp_ts.rs(`ShpTsFile { zero, cx, cy, frames }`,帧头 24B 重标)✓;theater_tmp.rs(`TsTmpFile::parse`/`slot(i)`/`diamond_pixels()`/`z_data()`,守卫三件套)✓;theater.rs(TheaterIni 顺序探查、`slot_count()`/`file_name(idx)`/`set_of(idx)`、常数表、`TheaterAssets::open`)✓;map_samples T9 重钉 ✓;CLI ✓
- §6 验收:六项全落 Task 8(映射见完成定义)✓;§6-6 偏差(三剧场改 TEMPERATE/SNOW/URBAN + DESERT 负向)已在歧义裁决 #1 登记待控制器复核 ✓
- §7 CLI 契约:签名与退出码 0/1/2 不变;stderr 链路诊断(剧场/容量/tile_id→文件名/槽位/来源容器)✓
- §8 测试策略:合成单测(CI)+ 真资产 ignored + 自愈样本沿用;闭合失败输出首违例(panic 消息含图名+三元组)✓

**placeholder 扫描**:全文无 `todo!`/`unimplemented!`/"同 Task N"引用;每个测试与实现均为完整代码段;常量全部来自研究文档、REA 勘误或本计划依据 §1-§8 的 2026-11-05 只读探针实测 ✓。两处显式"偏差记录"(walk 的 format 串、闭合测试首跑实测值占位)均为实现期机械修正项,非待决内容 ✓

**跨任务类型一致性**:

- `sha1(&[u8]) -> [u8; 20]`:Task 1 定义,Task 1 mix.rs 测试使用 ✓
- `digest_flag()`/`data_start()`:Task 1 定义(REA 勘误后形态;原 MixDigestState 三态作废),Task 1 语料测试与 Task 5 walk 使用 ✓
- `ShpTsFile::parse`:Task 2 定义,format_samples/m1b 迁移一致 ✓
- `TsTmpFile::parse`/`dims()`/`slot_count()`/`slot(i)`/`diamond_pixels()`/`z_data()`:Task 3 定义,Task 7(CLI)/Task 8(闭合缓存、clear01 锚)一致 ✓
- `TheaterIni::parse -> Result`/`slot_count()`/`file_name(idx, ext) -> Option<SlotFile>`/`set_of(idx) -> Option<TheaterSetInfo>`(owned,无借用):Task 4 定义,Task 5 真测/Task 6/Task 7/Task 8 一致 ✓
- `theater_spec(Theater) -> TheaterSpec`:Task 4 定义,Task 7/8 一致 ✓
- `TheaterAssets::open/lookup/lookup_id/origin`:Task 5 定义,Task 6 不用(显式链)、Task 7/8 一致 ✓
- `SlotFile::{File, Reserved}`:Task 4 定义,Task 6/7/8 一致 ✓
- 计数核对:合成 226 = 192 + 2 + 3 + 13 + 10 + 6(Task 1 勘误后;原 228 系 SHA-1 方案;原 T5 行 218+6=224 为计划笔误,真值 222+6=228);ignored 35 = 27 + 1 + 1 + 2 + 4——与各任务 Run 步骤期望值逐一相符 ✓

**已知风险与执行提示**:

- **语料计数失配协议**(M2 惯例):46/27/19 容器、807 图、SNOW 78/383/44 缺槽、886 现存瓦片分布、963 max tile——任一失配 = 只读探针重测、更新常数、commit message 注明,**不得静默放宽断言**。分布失配优先排查 TsTmpFile 对 SHP 的拒绝是否失效(cbx=0 判据)。
- **闭合测试 MadHQ 界**(≤4 图/≤500 槽)是宽裕防增长界:首跑把实测值写进测试注释;若实测超界,如实上报控制器裁决,不得放宽。
- **mix digest 规则**是本仓库裁决定义(哈希区域 = 索引表;XCC 尺寸规则不足,依据 §2):实现期可再对照 XCC(GPL,github.com/OlafvdSpek/xcc misc/mix_file.cpp)复核注释措辞,规则本身以语料 46 绿 + 合成双分支为准,不因 XCC 行为回改。
- **任务依赖**:Task 5 依赖 Task 1 的 `data_start()`;Task 6/7/8 依赖 Task 4/5;Task 2 与 Task 3 无相互依赖可并行;Task 7 与 Task 8 的 ignored 测试都要 Task 5。
- **内存提示**:`TheaterAssets::open` 持有 ra2/ra2md/expandmd01 及嵌套容器副本(峰值约 1 GB)——仅测试与 CLI 单发进程使用,勿进常驻服务(M4 装载器再议借用化)。
- **clippy 新近 deny lints**:计划代码已按迭代器/`as_chunks` 风格预写;个别报 `needless_range_loop` 时改 `zip/enumerate`,不得 `#[allow]`。
- **GBK 文件名**:中文 .map 在 Git Bash 命令行可直接传参(Rust fs 走 UTF-16 API);测试常量都在源文件内,终端显示乱码无影响。
