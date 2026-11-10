SPDX-License-Identifier: GPL-3.0-only

# M4c:SHP 变体解码 + 保真小批 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 SHP-TS 帧解码判别式从"先严格 RLE、败且 span==need 才 RAW"改为引擎对齐的帧级 flags bit1 门控,消除 overlay_fail 28946 与 objects_fail 3155;附带三个保真小件(VEIN 前缀守卫、objects_z_fallback 计数位、shadow_frame_index 旗标化)+ 真资产验收 m4c_acceptance.rs。

**Architecture:** 纯 Rust workspace(`crates/core` 格式/渲染库 + `tools/ra2-map` CLI)。判别式改 `shp_ts.rs`(帧级分派,删 ends[] 区间数组与 RAW 兜底试探),`rle_zero_ts.rs` 行块下限 4→2;渲染侧三个小件(overlay.rs 守卫 / SceneStats 新字段 / shape.rs 纯函数);验收照 m4b_acceptance.rs 模式新增 m4c_acceptance.rs。每任务收尾 `cargo test --workspace` 全绿。

**Tech Stack:** Rust 2021(零第三方依赖);TDD(每行为任务先红后绿);cargo fmt/clippy -D warnings 门禁;真资产验收需 `RA2_ASSETS_DIR`(只读,绝不入库)。

**上游文档:**
- spec(需求权威,数值逐字):`docs/superpowers/specs/2026-11-10-m4c-shp-variants-fidelity-design.md`
- 研究(证据与出处,§N 引文):`docs/superpowers/research/2026-11-09-m4c-shp-variants-fidelity.md`

## Global Constraints

- 每个改动文件首行 `// SPDX-License-Identifier: GPL-3.0-only`(既有文件已有,勿动)。
- 核心零第三方依赖;**绝不 commit EA 资产字节**,测试夹具一律合成字节。
- 引擎语义引 ra2-decomp file:line 注释随行(只读语义重写,不转录代码)。
- 模拟/视图分层:`crates/core` 内无 I/O;测试可写 `%TEMP%`。
- spec §6.1 断言数值**逐字照抄,不得改写或四舍五入**:`264211` / `514502` / `3539` / `9288` / `8320` / `1` / `507` / `10578` / `0`;Preview 最众组合 `"RGB"` 占比 `≥ 0.85`。
- 门禁(spec §6.4):`cargo fmt --all --check` 净;`cargo clippy --all-targets -- -D warnings` 净;`cargo test --workspace` 全绿 0 fail;真资产 `--ignored` 39 → 40(m4c_acceptance 新增 1)/ 0 fail。
- 每任务收尾 `cargo test --workspace` 必须全绿后才 commit。
- Bash 里跑 cargo 前先 `export PATH="$HOME/.cargo/bin:$PATH"`。
- git 提交信息中文,尾行 `Co-Authored-By: Claude Code <noreply@anthropic.com>`。
- 真资产套件运行环境变量:`RA2_ASSETS_DIR="E:\hongjing\Reunion 2023"`(只读)。
- **明确不做(spec §0/§7)**:rubble 分支(语料 0,只记语义)、remap、坡地 ore、DARKEN 变暗表、−2px/SHAPE_ALPHA/ZWRITE、别名表、voxel 渲染器;missing_art 3539 不收敛(钉为负 oracle)。
- 合成测试总数较 272 增长,精确数由实现上报、任务审查核验后钉台账(spec §6.4)。

## 任务级模型档(供控制器派发)

| 任务 | 内容 | 模型档 |
|---|---|---|
| Task 1 | 夹具先行:RAW 夹具 flags 3→1(4 处) | 低档 |
| Task 2 | rle_zero_ts.rs 行块下限 4→2 | 低档 |
| Task 3 | shp_ts.rs 判别式核心改写(最大任务) | 中档起步 |
| Task 4 | overlay VEIN 前缀守卫 | 低档 |
| Task 5 | SceneStats.objects_z_fallback 侧账 | 低档 |
| Task 6 | shape.rs shadow_frame_index 旗标化 | 低档 |
| Task 7 | m4c_acceptance.rs 真资产验收 | 中档起步 |

**顺序硬约束:Task 1(夹具先行)必须先于 Task 3(判别式改写)**——旧判别式下 flags=1 + span==need 走 RAW 兜底仍过,先行改可保 Task 3 落地时门禁不红。Task 2 先于 Task 3:Task 3 的 count=2/3 oracle 测试依赖新下限。Task 4/5/6 相互独立,但须在 Task 7 之前(验收依赖其行为/字段)。

## 文件结构总览

- 修改 `crates/core/src/format/rle_zero_ts.rs` — RLEZeroTS 行链解码(下限 2)。
- 修改 `crates/core/src/format/shp_ts.rs` — TS SHP 帧容器(帧级 bit1 判别式)。
- 修改 `crates/core/src/render/overlay.rs` — overlay pass(VEIN 守卫 + 夹具 flags)。
- 修改 `crates/core/src/render/objects.rs` — 物件 pass(z 侧账 + 夹具 flags)。
- 修改 `crates/core/src/render/scene.rs` — 场景合成(仅夹具 flags)。
- 修改 `crates/core/src/render/shape.rs` — 帧落画布(新增 shadow_frame_index 纯函数)。
- 修改 `crates/core/src/render/mod.rs` — SceneStats 增 objects_z_fallback。
- 修改 `tools/ra2-map/tests/cli.rs` — CLI 测试夹具 flags。
- 新建 `crates/core/tests/m4c_acceptance.rs` — 真资产验收(807 全量,ignore)。

---

### Task 1: 夹具先行——RAW 夹具 flags 3→1(4 处)

**Files:**
- Modify: `crates/core/src/render/overlay.rs:168-170`(shp_raw 文档注释)与 `:185`(flags 行)
- Modify: `crates/core/src/render/objects.rs:199-202`(shp_frames 文档注释)与 `:217`(flags 行)
- Modify: `crates/core/src/render/scene.rs:131-132`(shp_frames 文档注释)与 `:147`(flags 行)
- Modify: `tools/ra2-map/tests/cli.rs:604-605`(shp_frames 文档注释)与 `:620`(flags 行)

**Interfaces:**
- Consumes: 无(纯夹具改动,无行为变化)。
- Produces: 四个 RAW SHP 夹具构造器(`overlay.rs shp_raw` / `objects.rs shp_frames` / `scene.rs shp_frames` / `cli.rs shp_frames`)帧头 flags = 1(bit1=0)。Task 3 的判别式改写依赖此先行:flags=1 在旧判别式(RLE 失败 + span==need 兜底)与新判别式(bit1 门控直取 RAW)下行为一致,夹具测试全程不红。

**背景(必读):** 现行夹具用 flags=3 + 均匀填充构造 RAW 帧——均匀填充的严格 RLE 链首块 count = 257·fill 必然越界或行链中途耗尽(span=1 时连 2 字节块头都不足),旧判别式下 RLE 必败、span==cx·cy → 走 RAW 兜底。判别式改为 flags bit1 门控后 bit1=1 走 RLE,这些夹具若不改 flags 会全崩;flags=1 则新旧判别式下都按 RAW 处理(旧行为=RLE 失败后兜底,新行为=门控直取),所以本任务只改夹具不改判别式,全套测试应保持全绿。

- [ ] **Step 1: overlay.rs — shp_raw 文档注释与 flags 行**

把 `crates/core/src/render/overlay.rs` 中(约 168-170 行):

```rust
    /// N 帧 60×30 RAW SHP(裸 RAW 判别:span==cx·cy;均匀/近均匀填充的
    /// 严格 RLE 链 `257·v·k = 1800` 无整数解 → RLE 必败走 RAW,shp_ts.rs
    /// 判别式先严格 RLE)。帧体按声明顺序背靠背排列。
```

替换为:

```rust
    /// N 帧 60×30 RAW SHP:flags=1,bit1=0 → 帧级门控直取 RAW(draw.cpp:88-98
    /// Bit_Blit,M4c 判别式)。均匀填充的严格 RLE 链 `257·v·k = 1800` 无整数
    /// 解,任何判别式下均不被误读为 RLE。帧体按声明顺序背靠背排列。
```

并把同函数内(约 185 行):

```rust
            v.extend_from_slice(&3u32.to_le_bytes()); // flags
```

替换为:

```rust
            v.extend_from_slice(&1u32.to_le_bytes()); // flags=1:bit1=0 → RAW
```

- [ ] **Step 2: objects.rs — shp_frames 文档注释与 flags 行**

把 `crates/core/src/render/objects.rs` 中(约 199-202 行):

```rust
    /// N 帧 W×H RAW SHP:帧 i 整帧 = fills[i],帧体按序背靠背。裸 RAW 判别
    /// 同 shp_ts.rs(先严格 RLE,败且 span==cx·cy 才 RAW):均匀填充的严格
    /// RLE 行块必败——首块 count = 257·fill 对 <4 / 越界 / 行链中途耗尽三
    /// 败因全覆盖(span=1 时连 2 字节块头都不足)→ 恒走 RAW 兜底。
```

替换为:

```rust
    /// N 帧 W×H RAW SHP:帧 i 整帧 = fills[i],帧体按序背靠背。flags=1
    /// (bit1=0 → RAW,帧级门控 draw.cpp:88-98 Bit_Blit,M4c 判别式);
    /// 均匀填充的严格 RLE 行块必败——首块 count = 257·fill 越界或行链中途
    /// 耗尽(span=1 时连 2 字节块头都不足),任何判别式下均不被误读为 RLE。
```

并把同函数内(约 217 行)的 `v.extend_from_slice(&3u32.to_le_bytes()); // flags` 替换为:

```rust
            v.extend_from_slice(&1u32.to_le_bytes()); // flags=1:bit1=0 → RAW
```

- [ ] **Step 3: scene.rs — shp_frames 文档注释与 flags 行**

把 `crates/core/src/render/scene.rs` 中(约 131-132 行):

```rust
    /// N 帧 W×H RAW SHP(objects.rs tests 同款):帧 i 整帧 = fills[i]。
    /// 均匀填充严格 RLE 必败 → 恒走 RAW 兜底(shp_ts.rs 判别式)。
```

替换为:

```rust
    /// N 帧 W×H RAW SHP(objects.rs tests 同款):帧 i 整帧 = fills[i]。
    /// flags=1,bit1=0 → 帧级门控直取 RAW(M4c 判别式);均匀填充的严格
    /// RLE 链无整数解,任何判别式下均不被误读为 RLE。
```

并把同函数内(约 147 行)的 `v.extend_from_slice(&3u32.to_le_bytes()); // flags` 替换为:

```rust
            v.extend_from_slice(&1u32.to_le_bytes()); // flags=1:bit1=0 → RAW
```

- [ ] **Step 4: cli.rs — shp_frames 文档注释与 flags 行**

把 `tools/ra2-map/tests/cli.rs` 中(约 604-605 行):

```rust
/// N 帧 W×H RAW SHP(objects.rs tests 同款):帧 i 整帧 = fills[i]。
/// 均匀填充严格 RLE 必败 → 恒走 RAW 兜底(shp_ts.rs 判别式)。
```

替换为:

```rust
/// N 帧 W×H RAW SHP(objects.rs tests 同款):帧 i 整帧 = fills[i]。
/// flags=1,bit1=0 → 帧级门控直取 RAW(M4c 判别式);均匀填充的严格
/// RLE 链无整数解,任何判别式下均不被误读为 RLE。
```

并把同函数内(约 620 行)的 `v.extend_from_slice(&3u32.to_le_bytes()); // flags` 替换为:

```rust
        v.extend_from_slice(&1u32.to_le_bytes()); // flags=1:bit1=0 → RAW
```

(注意 cli.rs 的缩进是 8 空格,与 core 三处 12 空格不同。)

- [ ] **Step 5: 全量门禁(无行为变化,全程应绿)**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test --workspace
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
```

预期:`cargo test --workspace` 全绿 0 fail(夹具改动在旧判别式下经"RLE 失败 + span==need 兜底"仍走 RAW,逐字节数值不变);fmt/clippy 净。若此处有红,说明对某夹具的 RLE 失败性判断有误——停下带失败输出回设计,不得顺手改判别式。

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/render/overlay.rs crates/core/src/render/objects.rs crates/core/src/render/scene.rs tools/ra2-map/tests/cli.rs
git commit -m "test(m4c): RAW 夹具 flags 3→1 先行——判别式改写前保门禁绿(4 处注释同步)

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 2: rle_zero_ts.rs 行块下限 4→2

**Files:**
- Modify: `crates/core/src/format/rle_zero_ts.rs`(模块文档 3-11 行、下限检查 32-38 行、tests 119-122 行)

**Interfaces:**
- Consumes: 无。
- Produces: `rle_zero_ts_decode(src: &[u8], cx: usize, cy: usize, out: &mut Vec<u8>) -> Result<usize, FormatError>` 行为变化:行块 `count >= 2` 合法(count=2 = 空行整行透明、count=3 = 单像素窄行),`count` 0/1 仍 `Err(FormatError::Invalid)`。签名不变;SHP(shp_ts.rs)与 TMP(theater_tmp.rs)两消费方共用此放宽——语料 TMP 全集 M4a 起全过(旧下限未拒任何零售瓦片),放宽只可能把假 Err 转合法解码,零回归风险(spec §5.4;监督 = M4a/M4b 既有真资产测试保持全绿 + Task 7 验收 terrain 桶对账)。

- [ ] **Step 1: 写失败测试(count=2 空行 / count=3 单像素)**

把 `crates/core/src/format/rle_zero_ts.rs` tests 模块中:

```rust
    #[test]
    fn rejects_count_below_four() {
        let src = [0x02, 0x00];
        assert!(matches!(decode(&src, 4, 1), Err(FormatError::Invalid(_))));
    }
```

替换为:

```rust
    #[test]
    fn rejects_count_below_two() {
        // count=0/1:块头语义不齐(含自身 2 字节则数据长为负),恒 Err
        assert!(matches!(
            decode(&[0x00, 0x00], 4, 1),
            Err(FormatError::Invalid(_))
        ));
        assert!(matches!(
            decode(&[0x01, 0x00], 4, 1),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn count_two_is_empty_row() {
        // count=2 = 空行(无数据字节)→ 整行透明补零(M4c 下限 2)
        let src = [0x02, 0x00];
        assert_eq!(decode(&src, 4, 1).unwrap(), vec![0, 0, 0, 0]);
    }

    #[test]
    fn count_three_is_single_pixel_row() {
        // count=3 = 单像素窄行(CAFNCB f048 行块 `03 00 <px>` 语料实证,
        // 研究 §1.3-B;M1b 的 count≥4 下限误拒此形态)
        let src = [0x03, 0x00, 0x2A];
        assert_eq!(decode(&src, 1, 1).unwrap(), vec![0x2A]);
    }
```

- [ ] **Step 2: 跑测试验证红**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p ra2-core --lib rle_zero_ts
```

预期:`count_two_is_empty_row` 与 `count_three_is_single_pixel_row` FAIL(count=2/3 被旧下限 `count < 4` 拒);`rejects_count_below_two` PASS(旧代码 count=0/1 也 Err);其余原有测试 PASS。

- [ ] **Step 3: 实现下限 2 + 文档注释同步**

把模块文档(`crates/core/src/format/rle_zero_ts.rs` 3-11 行)末尾,即:

```rust
//! 2. 行块链之后可能有 2-7 字节 0 填充——`count` 链是唯一权威,自然跳过。
```

替换为(保留前文,在其后追加一段):

```rust
//! 2. 行块链之后可能有 2-7 字节 0 填充——`count` 链是唯一权威,自然跳过。
//!
//! 行块下限 = 2(count 含自身 2 字节 → count=2 = 空行、count=3 = 单像素
//! 窄行,CAFNCB f048 行块 `03 00 <px>` 语料实证;引擎行推进只按 count 值、
//! 无下限校验,blowfish.cpp:296-314)。M1b 的 `count≥4` 下限系误推广
//! (研究 2026-11-09 §1.3-B 勘误);count=0/1 块头语义不齐,仍 Err。
```

并把解码体内(约 33-34 行):

```rust
        // 行块下限 4(块头 2 + 至少 2 个数据字节;研究文档 §3 错误清单)
        if count < 4 {
```

替换为:

```rust
        // 行块下限 2:count=2 空行、count=3 单像素窄行(CAFNCB f048 语料
        // 实证;引擎行推进无下限校验,blowfish.cpp:296-314);count=0/1 仍 Err
        if count < 2 {
```

- [ ] **Step 4: 跑测试验证绿**

```bash
cargo test -p ra2-core --lib rle_zero_ts
```

预期:全部 PASS(含新三测试)。

- [ ] **Step 5: 全量门禁**

```bash
cargo test --workspace
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
```

预期:全绿。注意 shp_ts.rs 既有判别式测试在此下限下仍绿:`raw_fallback_when_rle_fails_and_span_exact` 的 body 首块 count=2 现在被 RLE 半程接受后于行 1 触 EOF,旧判别式兜底 span==need → RAW 不变;`err_when_rle_fails_and_span_mismatch` 首块 count=8 越界 → EOF → span≠need 透传 Err 不变。

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/format/rle_zero_ts.rs
git commit -m "feat(m4c): RLEZeroTS 行块下限 4→2——count=2 空行/3 单像素合法(CAFNCB f048;引擎无下限校验)

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 3: shp_ts.rs 核心改写——帧级 flags bit1 判别式(最大任务)

**Files:**
- Modify: `crates/core/src/format/shp_ts.rs`(模块文档 3-14 行、flags/rgba 字段注释 29-32 行、新增 impl 块、解码块 116-161 行、tests 304-332 行重述 + 新增 oracle 测试)

**Interfaces:**
- Consumes: Task 2 的 `rle_zero_ts_decode`(count≥2 合法;count=0/1 → `FormatError::Invalid`;count 越文件尾 → `FormatError::UnexpectedEof`)。
- Produces:
  - `impl ShpTsFrameHeader { pub fn is_rle(&self) -> bool }` = `(self.flags & 0xFF) & 2 != 0`。
  - `ShpTsFile::parse(data: &[u8]) -> Result<ShpTsFile, FormatError>` 新语义:bit1=1 → RLE 行链(区间 `[offset, 文件尾)`);bit1=0 → RAW 取 `[offset, offset+need)`(need = cx·cy),越文件尾补零截断零 panic。
  - 维持不动:空帧约定(cx==cy==offset==0 判空先于 flags 分派)、帧矩形画布界、偏移越界/表前偏移检查、头 zero 验零、`MAX_SHP_CANVAS_PIXELS` 分配前上界、帧数 1..=10000。
  - 下游(overlay/objects/scene/cli 夹具已 flags=1)零改动。

**背景(必读):** 引擎分派 `HasCompression → RLE_Blit : Bit_Blit`(draw.cpp:88-98;shapipe.cpp:148-156 按 +8 单字节 bit1),变体是**帧级**(CIV1 476 帧同文件混 flags=0x03/0x01,研究 §1.3-A)。现行"先严格 RLE、败且 span==cx·cy 才 RAW、否则透传 Err"判别式 + `ends[]` 下帧偏移链界均为 M1b 探针期假设,被语料证伪(RAW 帧普遍带 2-8B 尾 pad → span≠need 被拒;引擎无 region 界)。修复模拟:判别式 bit1 门控 + 下限 2 → 全集 1045 文件全过、rows_over_width=0、raw 尾截断 0(研究 §1.3)。

- [ ] **Step 1: 重述既有判别式三测试 + 写 oracle 测试(先红)**

把 `crates/core/src/format/shp_ts.rs` tests 模块中以下三个测试(约 304-332 行):

```rust
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
        assert!(matches!(
            ShpTsFile::parse(&data),
            Err(FormatError::UnexpectedEof { .. })
        ));
    }
```

替换为(三测试重述,spec §5.2):

```rust
    #[test]
    fn raw_gated_by_flags_bit1_zero() {
        // M4c 引擎对齐:flags=1 → bit1=0 → 门控直取 RAW(不经"RLE 失败
        // 兜底"路径;draw.cpp:88-98 Bit_Blit,spec §5.2 重述)
        let body = [0x02, 0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let data = shp_bytes((8, 2), &[frame_header(0, 0, 4, 2, 1, [0; 4], 0, 32)], &body);
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!(f.frames[0].pixels, body.to_vec());
    }

    #[test]
    fn rle_gated_by_flags_bit1_one() {
        // flags=3 → bit1=1 → RLE 行链(引擎分派 HasCompression → RLE_Blit,
        // shapipe.cpp:148-156);全透明帧的链 span 也可恰 == cx·cy,门控不看
        // span——语义是"bit1 门控",非旧"RLE 优先试探"(spec §5.2 重述)
        let body = [0x04, 0x00, 0x00, 0x04];
        let data = shp_bytes((4, 1), &[frame_header(0, 0, 4, 1, 3, [0; 4], 0, 32)], &body);
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!(f.frames[0].pixels, vec![0, 0, 0, 0]);
    }

    #[test]
    fn err_when_rle_chain_touches_eof() {
        // RLE 链截断(块 count 声明越过文件尾)→ UnexpectedEof;链区间已改
        // [offset, 文件尾),数据不足语义不变(spec §5.2 重述)
        let body = [0x08, 0x00, 0xAA, 0xBB, 0xCC, 0xDD];
        let data = shp_bytes((8, 2), &[frame_header(0, 0, 4, 2, 3, [0; 4], 0, 32)], &body);
        assert!(matches!(
            ShpTsFile::parse(&data),
            Err(FormatError::UnexpectedEof { .. })
        ));
    }
```

再在同 tests 模块末尾(`rejects_canvas_over_allocation_cap` 之后、模块收尾 `}` 之前)追加 spec §6.2 a-f 合成 oracle 测试(全部合成字节,零 EA 资产):

```rust
    // ===== M4c 合成字节级 oracle(spec §6.2;模式复刻研究 §5.3,零 EA 字节)=====

    /// a. RAW + 尾 pad(DRUM01 模式):11×14 flags=0x01,span=160 > need=154,
    /// 尾部 6B pad 不入帧;同型 flags=0x00(GEM09 2×2 span 8 / need 4)。
    #[test]
    fn oracle_raw_with_tail_pad() {
        // DRUM01 模式:11×14 = 154 像素,帧体 160B(154 数据 + 6 pad)
        let mut body = vec![0x5Au8; 154];
        body.extend_from_slice(&[0xCC; 6]);
        let data = shp_bytes(
            (11, 14),
            &[frame_header(0, 0, 11, 14, 0x01, [0; 4], 0, 32)],
            &body,
        );
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!(f.frames[0].pixels.len(), 154, "pad 不入帧");
        assert!(f.frames[0].pixels.iter().all(|&b| b == 0x5A));
        // GEM09 模式:2×2 = 4 像素,span 8(4 数据 + 4 pad),flags=0x00
        let body2 = [0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00];
        let data2 = shp_bytes(
            (30, 42),
            &[frame_header(21, 30, 2, 2, 0x00, [0; 4], 0, 32)],
            &body2,
        );
        let f2 = ShpTsFile::parse(&data2).unwrap();
        assert_eq!(f2.frames[0].pixels, vec![0x01, 0x01, 0x01, 0x01]);
    }

    /// b. 同文件混变体(CIV1 模式):f0 flags=0x03 RLE 行链、f1 flags=0x01
    /// RAW——帧级门控钉死,任何文件级格式选择不可行(研究 §1.3-A)。
    #[test]
    fn oracle_mixed_variants_in_one_file() {
        // table_end = 8 + 24·2 = 56;f0 RLE 4B @56、f1 RAW 2B @60,背靠背
        let body = [0x04, 0x00, 0x05, 0x06, 0x07, 0x08];
        let data = shp_bytes(
            (4, 1),
            &[
                frame_header(0, 0, 2, 1, 0x03, [0; 4], 0, 56),
                frame_header(0, 0, 2, 1, 0x01, [0; 4], 0, 60),
            ],
            &body,
        );
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!(f.frames[0].pixels, vec![0x05, 0x06], "f0 bit1=1 → RLE");
        assert_eq!(f.frames[1].pixels, vec![0x07, 0x08], "f1 bit1=0 → RAW");
    }

    /// c. 窄行 RLE(CAFNCB f048 模式):1×1 flags=0x02,行块 `03 00 <px>`
    /// → 单像素(依赖 Task 2 下限 2)。
    #[test]
    fn oracle_narrow_row_rle_count_three() {
        let body = [0x03, 0x00, 0x2A];
        let data = shp_bytes(
            (1, 1),
            &[frame_header(0, 0, 1, 1, 0x02, [0; 4], 0, 32)],
            &body,
        );
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!(f.frames[0].pixels, vec![0x2A]);
    }

    /// d. 判定面:flags=0x04 → RAW(bit1=0);flags=0xCCCC0003 → 只读 +8
    /// 字节按 RLE(高半字为匿名填充不参与);flags=0xCCCC0001 → RAW;
    /// flags u32 原样保留。空帧 flags≠0 跳过另见
    /// oracle_empty_frame_with_nonzero_flags 与 parses_frame_and_decodes_rle。
    #[test]
    fn oracle_flag_bitmask_dispatch() {
        let mk = |flags: u32, body: &[u8]| {
            shp_bytes(
                (2, 1),
                &[frame_header(0, 0, 2, 1, flags, [0; 4], 0, 32)],
                body,
            )
        };
        // 0x04:bit1=0 → RAW
        let f = ShpTsFile::parse(&mk(0x04, &[0x11, 0x12])).unwrap();
        assert_eq!(f.frames[0].header.flags, 0x04, "flags u32 原样保留");
        assert_eq!(f.frames[0].pixels, vec![0x11, 0x12]);
        // 0xCCCC0003:低字节 0x03 bit1=1 → RLE(行块 count=4 → 2 字面量)
        let f = ShpTsFile::parse(&mk(0xCCCC_0003, &[0x04, 0x00, 0x21, 0x22])).unwrap();
        assert_eq!(f.frames[0].header.flags, 0xCCCC_0003);
        assert_eq!(f.frames[0].pixels, vec![0x21, 0x22]);
        // 0xCCCC0001:低字节 0x01 bit1=0 → RAW
        let f = ShpTsFile::parse(&mk(0xCCCC_0001, &[0x31, 0x32])).unwrap();
        assert_eq!(f.frames[0].pixels, vec![0x31, 0x32]);
    }

    /// d/e. 空帧约定先于 flags 分派:flags≠0(cx==cy==offset==0)仍按空帧
    /// 跳过(空帧可带非 0 flags,勿以 flags 判空)。
    #[test]
    fn oracle_empty_frame_with_nonzero_flags() {
        let data = shp_bytes(
            (4, 2),
            &[frame_header(0, 0, 0, 0, 0xCCCC_0001, [0; 4], 0, 0)],
            &[],
        );
        let f = ShpTsFile::parse(&data).unwrap();
        assert!(f.frames[0].pixels.is_empty(), "空帧不进 flags 分派");
    }

    /// e. 逐帧像素期望:flags=0x02 RLE 帧体仅 `02 00`(count=2 空行)→
    /// 整行透明;count=0/1 → Err 在 rle_zero_ts.rs 单测钉(rejects_count_below_two)。
    #[test]
    fn oracle_rle_count_two_empty_row_transparent() {
        let body = [0x02, 0x00];
        let data = shp_bytes(
            (4, 1),
            &[frame_header(0, 0, 4, 1, 0x02, [0; 4], 0, 32)],
            &body,
        );
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!(f.frames[0].pixels, vec![0, 0, 0, 0]);
    }

    /// f. 敌意面:RAW 谎报 need 超文件尾 → 补零截断零 panic(新防御位;
    /// 旧判别式整文件拒);表前偏移/画布越界/验零维持由既有测试
    /// rejects_offset_before_table / rejects_rect_out_of_canvas /
    /// rejects_bad_zero_field / rejects_canvas_over_allocation_cap 覆盖。
    #[test]
    fn oracle_raw_need_over_eof_zero_pads() {
        // 100×100 flags=0x01,文件体仅 5B → 前 5B 数据 + 9995 零,零 panic
        let body = [0x11, 0x22, 0x33, 0x44, 0x55];
        let data = shp_bytes(
            (100, 100),
            &[frame_header(0, 0, 100, 100, 0x01, [0; 4], 0, 32)],
            &body,
        );
        let f = ShpTsFile::parse(&data).unwrap();
        let px = &f.frames[0].pixels;
        assert_eq!(px.len(), 10_000);
        assert_eq!(&px[..5], &body);
        assert!(px[5..].iter().all(|&b| b == 0), "越尾部分补零");
    }

    /// f. 敌意面:RLE 行链越文件尾 → UnexpectedEof(链区间 [offset, 文件尾)
    /// 触界仍 Err,零 panic)。
    #[test]
    fn hostile_rle_chain_over_eof_is_eof_error() {
        // 行块 count=0x0100 声明 256B,文件体仅 3B → UnexpectedEof
        let body = [0x00, 0x01, 0xAA];
        let data = shp_bytes(
            (4, 2),
            &[frame_header(0, 0, 4, 2, 0x02, [0; 4], 0, 32)],
            &body,
        );
        assert!(matches!(
            ShpTsFile::parse(&data),
            Err(FormatError::UnexpectedEof { .. })
        ));
    }
```

- [ ] **Step 2: 跑测试验证红**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p ra2-core --lib shp_ts
```

预期 FAIL(旧判别式拒绝这些形态):`oracle_raw_with_tail_pad`(span 160≠154 透传 Err)、`oracle_narrow_row_rle_count_three`(RLE 败 + span≠need → Err)、`oracle_rle_count_two_empty_row_transparent`(RLE 败 + span≠need → Err)、`oracle_raw_need_over_eof_zero_pads`(span≠need → Err)。预期 PASS(钉定语义与旧行为等价):`raw_gated_by_flags_bit1_zero` / `rle_gated_by_flags_bit1_one` / `err_when_rle_chain_touches_eof` / `oracle_mixed_variants_in_one_file` / `oracle_flag_bitmask_dispatch` / `oracle_empty_frame_with_nonzero_flags` / `hostile_rle_chain_over_eof_is_eof_error` 及全部既有测试。

- [ ] **Step 3: 实现——模块文档、字段注释、is_rle、解码块**

(a) 把模块文档(3-14 行)整段:

```rust
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
```

替换为:

```rust
//! TS SHP 精灵帧容器(t_shp_ts,XCC 术语;.sno/.tem 等剧场装饰精灵即此格式)。
//!
//! 出处:布局与判别按裁决后研究 2026-11-05-theater-tile-mapping.md §4/§6
//! (XCC `t_shp_ts` 语义重写;burnt01/lobrdb08 甲骨文帧偏移精确闭合)。
//! **M1b 误判纠正**:本文件前身 tmp.rs 曾把该 8B+24B 布局当成"RA2 剧场瓦片",
//! 实为 TS SHP 帧格式——帧头 24B = `{x,y i16; cx,cy u16; flags u32; rgba
//! [u8;4]; reserved u32; data_offset u32}`。TS 真瓦片(TS TMP,16B i32 头 +
//! 槽表)是另一种容器,由 theater_tmp.rs 承担,勿混用。
//!
//! **M4c 判别式(引擎对齐,帧级)**:帧头 +8 单字节 bit1 决定压缩形态——
//! bit1=1 → RLEZeroTS 行链(区间 [offset, 文件尾),引擎无 region 界,
//! shapipe.cpp:103-112/148-156 + draw.cpp:88-98);bit1=0 → RAW,取
//! [offset, offset+cx·cy),越文件尾补零截断(防御位;RAW 帧普遍带 2-8B
//! 尾 pad)。变体是帧级(CIV1 476 帧同文件混 0x03/0x01,研究 §1.3-A),
//! 任何文件级格式选择不可行;M1b 的"先严格 RLE、败且 span==cx·cy 才 RAW"
//! 判别式与 ends[] 下帧偏移链界由此废止(研究 §2.4)。空帧约定
//! `cx==cy==data_offset==0` 判空先于 flags 分派(空帧可带非 0 flags)。
```

(b) 把 `ShpTsFrameHeader` 的 flags 字段注释(29-30 行):

```rust
    /// 观测多数 3、个别 4(bit0|bit1);语义未裁决,原样暴露。
    pub flags: u32,
```

替换为:

```rust
    /// +8 单字节语义(引擎 `SHPFrame::Flags`,msanim.h:49-63):bit0 =
    /// HasTransparency、bit1 = HasCompression(shapipe.cpp:137-156);
    /// +9..+0xB 匿名填充(语料观测 `0xCCCC` 高半字 = 未初始化残留,研究
    /// §1.2/§2.1)。u32 原样存储,判定只读低字节([Self::is_rle]);
    /// 观测直方 0x3×20700 / 0xCCCC0003×195 / 0xCCCC0001×31 / 0x4×92 /
    /// 0x1×32 / 0xCCCCCCCC×2(研究 §1.2)。
    pub flags: u32,
```

(c) 把 rgba 字段注释(31-32 行):

```rust
    /// +0x0C..0x10 四字节;旧"unknown_d"即其蓝通道字节(研究 §4)。
    pub rgba: [u8; 4],
```

替换为:

```rust
    /// +0x0C..0x10 四字节 = 引擎 Color 3B + 1 pad(msanim.h:49-63,ra2-rs
    /// 命名照旧);旧"unknown_d"即其蓝通道字节(研究 §4)。
    pub rgba: [u8; 4],
```

(d) 在 `ShpTsFrameHeader` 结构体定义结束(`}` 之后、`/// 一帧:帧头 + 解码后的调色板索引...` 之前)插入 impl 块:

```rust
impl ShpTsFrameHeader {
    /// 帧级压缩判定(引擎 `HasCompression = (char)(Flags & 2) >> 1`,
    /// shapipe.cpp:148-156):只读 +8 单字节的 bit1,+9..+0xB 为匿名填充
    /// 不参与(msanim.h:49-63)。变体是帧级(CIV1 476 帧同文件混
    /// 0x03/0x01,研究 §1.3-A),任何文件级格式选择不可行。
    pub fn is_rle(&self) -> bool {
        (self.flags & 0xFF) & 2 != 0
    }
}
```

(e) 把解码块(116-161 行,从 `// 每帧数据区间` 注释起到 `frames.push` 循环结束):

```rust
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
                if need > MAX_SHP_CANVAS_PIXELS {
                    return Err(FormatError::Invalid(format!(
                        "SHP 画布 {}x{} 共 {need} 像素,超分配前上界 {MAX_SHP_CANVAS_PIXELS}",
                        h.cx, h.cy
                    )));
                }
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
```

替换为(ends[] 数组整体删除;is_rle() 门控;RAW 补零截断;RLE 链区间 [offset, 文件尾)):

```rust
        let mut frames = Vec::with_capacity(frame_count);
        for h in headers {
            let empty = h.cx == 0 && h.cy == 0 && h.data_offset == 0;
            let pixels = if empty {
                Vec::new()
            } else {
                let start = h.data_offset as usize;
                let need = h.cx as usize * h.cy as usize;
                if need > MAX_SHP_CANVAS_PIXELS {
                    return Err(FormatError::Invalid(format!(
                        "SHP 画布 {}x{} 共 {need} 像素,超分配前上界 {MAX_SHP_CANVAS_PIXELS}",
                        h.cx, h.cy
                    )));
                }
                let mut pixels = Vec::with_capacity(need);
                if h.is_rle() {
                    // bit1=1 → RLE 行链,区间 [offset, 文件尾):引擎无 region
                    // 界(GetPixels 直给缓冲指针,shapipe.cpp:103-112;行链可
                    // 越过下帧 offset,研究 §2.2),count 链是唯一权威;EOF
                    // 触碰仍 Err。M1b 的 ends[] 下帧偏移链界废止(研究 §2.4-4:
                    // 修复版全集 0 败、rows_over_width=0,文件尾为界足够)。
                    rle_zero_ts_decode(
                        &data[start..],
                        h.cx as usize,
                        h.cy as usize,
                        &mut pixels,
                    )?;
                } else {
                    // bit1=0 → RAW:取 [offset, offset+need);offset+need 越文
                    // 件尾时补零截断(语料 0 出现,防御位——lenient 零 panic,
                    // 敌意输入不再整文件拒;RAW 帧普遍带 2-8B 尾 pad,旧
                    // `span==need` 硬条件废止,draw.cpp:88-98 Bit_Blit)。
                    let take = (start + need).min(file_len);
                    pixels.extend_from_slice(&data[start..take]);
                    pixels.resize(need, 0);
                }
                pixels
            };
            frames.push(ShpTsFrame { header: h, pixels });
        }
```

维持不动的清单(spec §1-6,逐项核对):空帧约定检查(92-100 行)、偏移越界/表前偏移(101-106 行)、画布界(107-114 行)、头 zero 验零(57-59 行)、帧数 1..=10000(63-67 行)、`MAX_SHP_CANVAS_PIXELS` 检查在新解码块内保留(分配前)。

- [ ] **Step 4: 跑测试验证绿**

```bash
cargo test -p ra2-core --lib shp_ts
cargo test -p ra2-core --lib rle_zero_ts
```

预期:两个模块全部 PASS(含 Step 1 全部 oracle 测试)。

- [ ] **Step 5: 全量门禁**

```bash
cargo test --workspace
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
```

预期:全绿。Task 1 的 flags=1 夹具在新判别式下走"bit1=0 → RAW 直取",逐字节结果与旧兜底一致;`burnt01`/`lobrdb08` 等真资产测试(ignore)在无资产环境下不跑,其 RLE 体 flags=3 → is_rle → 与旧行为同链,留 Task 7 门禁复核。

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/format/shp_ts.rs
git commit -m "feat(m4c): SHP-TS 判别式改帧级 flags bit1 门控——RAW 尾 pad 补零截断、RLE 链区间改文件尾(引擎对齐 shapipe/draw)

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 4: overlay VEIN 前缀守卫

**Files:**
- Modify: `crates/core/src/render/overlay.rs`(模块文档 3-8 行、paint_overlays 内 kind(disp) 绑定之后;tests 模块末尾新增测试)

**Interfaces:**
- Consumes: `OverlayKind.image: &str`(`crates/core/src/format/rules.rs:42`,表项名;Image= 缺省 = 类型名,rules.rs:64-69)。
- Produces: `paint_overlays` 行为——表项名以 `"VEIN"` 开头 → 不画、不查档、计 `overlay_drawn`(与 `BlitResult::Outside` 同口径:链路成功、像素面空)。Task 7 验收依赖其语料零命中(表位 178 VEINHOLEDUMMY 0 格)不改 264211 oracle。rubble 分支语料 0,只记语义不实现(spec §2)。

**背景(必读):** 引擎 VEINHOLE/VEINHOLE_DUMMY 直接 return 不画(cell.cpp:1401-1403);零售 [OverlayTypes] 唯一 VEIN 名 = 表位 178 VEINHOLEDUMMY,语料命中 0 格(研究 §4.5)——纯语义对齐,不改 807 总账。守卫放在查档之前:缺档不会被计入 missing_file/fail,这正是"直接 return"的语义。

- [ ] **Step 1: 写失败测试(合成 mini 图,画布零写入断言)**

在 `crates/core/src/render/overlay.rs` tests 模块末尾(`crate_bridge_clamp_counters` 测试之后、模块收尾 `}` 之前)追加:

```rust
    /// VEIN 前缀守卫(cell.cpp:1401-1403):表项名 VEIN* 不画、不计失败,
    /// 计 overlay_drawn(链路成功像素面空,与 Outside 同口径);守卫先于
    /// 查档——资产刻意不提供 VEINHOLEDUMMY.shp,缺档不得入 missing/fail;
    /// 画布零写入(格中心保持地形色)。零售 [OverlayTypes] 唯一 VEIN 名 =
    /// 表位 178 VEINHOLEDUMMY,语料命中 0 格(研究 §4.5),纯语义对齐。
    #[test]
    fn vein_prefixed_overlay_skips_paint() {
        let pal = test_palette();
        let assets = tiny_assets("vein", &[("tst01.sno", tmp60(9))]);
        let map = overlay_map(
            (2, 2),
            &types_layer(&[(2, 2, 0)]),
            &data_layer(&[(2, 2, 0)]),
        );
        let ext = theater_spec(map.map_meta.theater).ext;
        let recs = sorted_records(2, 2);
        let ter = compose_terrain(2, 2, &recs, &assets, &terrain_ini(), ext, &pal).unwrap();
        let (cw, ch) = (ter.width as usize, ter.height as usize);
        let mut rgb = ter.rgb;
        let (d_min, _, xmy_min, _) = walk_extremes(2, 2);
        let mut stats = SceneStats::default();
        let mut cache = ShpCache::new();
        paint_overlays(
            &map,
            &assets,
            &OverlayTable::parse("[OverlayTypes]\n0=VEINHOLEDUMMY\n"),
            &pal,
            &recs,
            d_min,
            xmy_min,
            0,
            ext,
            &mut rgb,
            cw,
            ch,
            &mut stats,
            &mut cache,
        );
        assert_eq!(stats.overlay_drawn, 1, "守卫命中计 drawn(不画)");
        assert_eq!(
            stats.overlay_no_type + stats.overlay_missing_file + stats.overlay_fail,
            0,
            "守卫先于查档:无 no_type/missing/fail"
        );
        assert_conservation(&map, &recs, &stats);
        let px = |x: usize, y: usize| -> [u8; 3] {
            let i = 3 * (y * cw + x);
            [rgb[i], rgb[i + 1], rgb[i + 2]]
        };
        assert_eq!(px(60, 30), c8(9), "画布零写入(格中心保持地形色)");
    }
```

- [ ] **Step 2: 跑测试验证红**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p ra2-core --lib overlay
```

预期:`vein_prefixed_overlay_skips_paint` FAIL——无守卫时 load_shp("VEINHOLEDUMMY") 缺档 → `overlay_missing_file == 1`、`overlay_drawn == 0`,两个断言都挂;`assert_conservation` 也挂(1 ≠ 0)。

- [ ] **Step 3: 实现守卫**

(a) 把模块文档(3-8 行)末句:

```rust
//! 桥 NS 偏移 (−15,7) 与桥高 z 附加
//! (cell.cpp:1408/1372-1376)不感知 → M4c(spec §8 低置信清单)。
```

替换为:

```rust
//! 桥 NS 偏移 (−15,7) 与桥高 z 附加
//! (cell.cpp:1408/1372-1376)不感知 → M4c(spec §8 低置信清单)。
//! VEIN 前缀表项守卫跳画(cell.cpp:1401-1403,M4c spec §2;计 drawn
//! 与 Outside 同口径)。
```

(b) 在 `paint_overlays` 中,把:

```rust
        let Some(kind) = table.kind(disp) else {
            stats.overlay_no_type += 1;
            continue;
        };
        let mut frame = if kind.is_crate {
```

替换为:

```rust
        let Some(kind) = table.kind(disp) else {
            stats.overlay_no_type += 1;
            continue;
        };
        // VEIN 前缀守卫(cell.cpp:1401-1403 VEINHOLE/VEINHOLE_DUMMY 直接
        // return):不画、不计失败,计 overlay_drawn(链路成功、像素面空,
        // 与 BlitResult::Outside 同口径)。守卫先于查档(缺档不入
        // missing/fail)。零售 [OverlayTypes] 唯一 VEIN 名 = 表位 178
        // VEINHOLEDUMMY,语料命中 0 格(研究 §4.5)——纯语义对齐,不改
        // 807 总账;rubble 分支语料 0,只记语义不实现(spec §2)。
        if kind.image.starts_with("VEIN") {
            stats.overlay_drawn += 1;
            continue;
        }
        let mut frame = if kind.is_crate {
```

- [ ] **Step 4: 跑测试验证绿**

```bash
cargo test -p ra2-core --lib overlay
```

预期:全部 PASS(含新测试;既有两测试不受影响——其表项 ORE01/GASAND/CRATE01/BRIDGE1 均无 VEIN 前缀)。

- [ ] **Step 5: 全量门禁**

```bash
cargo test --workspace
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
```

预期:全绿(scene.rs 层序/守恒测试用 ORE01/FULLT,不经守卫分支)。

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/render/overlay.rs
git commit -m "feat(m4c): overlay VEIN 前缀守卫——跳画计 drawn(cell.cpp:1401-1403 语义对齐)

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 5: SceneStats 增 objects_z_fallback 侧账计数位

**Files:**
- Modify: `crates/core/src/render/mod.rs`(SceneStats 结构体,60-81 行,`clip` 字段后追加)
- Modify: `crates/core/src/render/objects.rs`(81 行 z 兜底改 match;tests 模块末尾新增测试)

**Interfaces:**
- Consumes: `map.tiles.cell(wx, wy) -> Option<IsoTile>`(既有,z 缺席 = None)。
- Produces: `SceneStats` 新 pub 字段 `objects_z_fallback: usize`(信息位,**不入**守恒恒等式;`#[derive(Default)]` 自动零初始化,scene.rs 的 `..SceneStats::default()` 零改动)。Task 7 验收依赖 `objects_z_fallback == 9288` oracle。overlay 无此面(其 z 来自记录本体,spec §3)。

**背景(必读):** objects.rs:81 现状 `map.tiles.cell(wx, wy).map_or(0u8, |t| t.z)` 把 z 缺席(物件格在走位域外/虚空)静默记 0;语料量级 9,288 件/807 图(研究 §4.7)。本任务只加诊断计数,不改落笔行为(仍记 0)。

- [ ] **Step 1: 写失败测试(新字段不存在 → 编译红)**

在 `crates/core/src/render/objects.rs` tests 模块末尾(`buckets_and_conservation` 测试之后、模块收尾 `}` 之前)追加:

```rust
    /// objects_z_fallback 侧账(信息位,不入守恒恒等式):物件落格在走位
    /// 域外(tiles.cell 缺席)→ 计数;域内格 → 不计数。Structure 0 基 (0,0)
    /// → cell()=(0,0) → 查档格 (1,1) 在 2×2 走位域 (2,1),(1,2),(2,2),
    /// (3,2),(2,3),(3,3) 外 → fallback +1;树 key 2001 → 0 基 (1,2) → 查档
    /// 格 (2,3) 域内 → 不计。域外件链路成功(Outside 亦计 drawn,clip +1)。
    #[test]
    fn z_fallback_counts_objects_off_walk_domain() {
        let pal = test_palette();
        let assets = tiny_assets(
            "zfall",
            &[
                ("tst01.sno", tmp60(9)),
                ("tstbldg.shp", shp1(1, 1, 50)),
                ("goodtree.shp", shp1(1, 1, 60)),
            ],
        );
        let sections = "[Structures]\n\
                        0=Neutral,TSTBLDG,256,0,0,0,None,0,0,1,0,0,None,None,None,0,0\n\
                        [Terrain]\n2001=GOODTREE\n";
        let (map, rgb, cw, ch, d_min, xmy_min, z_max, ext) =
            stage(2, 2, &[], sections, &assets, &pal);
        let mut rgb = rgb;
        let mut stats = SceneStats::default();
        let mut cache = ShpCache::new();
        paint_objects(
            &map,
            &assets,
            &ArtIndex::parse("", ""),
            &pal,
            d_min,
            xmy_min,
            z_max,
            ext,
            &mut rgb,
            cw,
            ch,
            &mut stats,
            &mut cache,
        );
        assert_eq!(stats.objects_z_fallback, 1, "仅域外建筑计 z 缺席");
        assert_eq!(stats.objects_drawn, 2, "两件链路成功(域外件 Outside 亦计)");
        assert_eq!(stats.clip, 1, "域外件出画布计 clip");
        assert_conservation(&map, &stats);
    }
```

- [ ] **Step 2: 跑测试验证红**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p ra2-core --lib objects
```

预期:**编译失败**(`SceneStats` 无字段 `objects_z_fallback`)——字段先行即红。

- [ ] **Step 3: 实现——字段 + 计数**

(a) 把 `crates/core/src/render/mod.rs` SceneStats 中:

```rust
    /// 精灵矩形未全入画布(信息位)。
    pub clip: usize,
}
```

替换为:

```rust
    /// 精灵矩形未全入画布(信息位)。
    pub clip: usize,
    /// 物件格走位域外/虚空,z 缺席记 0 的侧账(信息位,不入守恒恒等式;
    /// 语料 9,288 件/807 图,研究 §4.7)。overlay 无此面(其 z 来自记录本体)。
    pub objects_z_fallback: usize,
}
```

(b) 把 `crates/core/src/render/objects.rs` 中(81 行):

```rust
        let z = map.tiles.cell(wx, wy).map_or(0u8, |t| t.z);
```

替换为:

```rust
        // z 缺席格(物件格在走位域外/虚空)记 0 兜底并计侧账(信息位,
        // 不入守恒恒等式;语料 9,288 件/807 图,研究 §4.7)。
        let z = match map.tiles.cell(wx, wy) {
            Some(t) => t.z,
            None => {
                stats.objects_z_fallback += 1;
                0
            }
        };
```

- [ ] **Step 4: 跑测试验证绿**

```bash
cargo test -p ra2-core --lib objects
```

预期:全部 PASS(既有测试零 fallback 面——各夹具物件全在走位域内,`objects_z_fallback` 恒 0,不影响其断言)。

- [ ] **Step 5: 全量门禁**

```bash
cargo test --workspace
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
```

预期:全绿。m4b_acceptance.rs 用具名字段构造/读取 SceneStats,新字段走 Default,零改动(其文件原样保留作回归绊线)。

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/render/mod.rs crates/core/src/render/objects.rs
git commit -m "feat(m4c): SceneStats 增 objects_z_fallback 侧账计数位(信息位不入守恒)

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 6: shape.rs shadow_frame_index 纯函数旗标化

**Files:**
- Modify: `crates/core/src/render/shape.rs`(load_shp 之后、`#[cfg(test)]` 之前插入函数;tests 模块末尾新增测试)

**Interfaces:**
- Consumes: 无(纯函数)。
- Produces: `pub(crate) fn shadow_frame_index(frame: usize, frame_count: usize) -> Option<usize>`——`frame + frame_count/2 < frame_count` 时 `Some(frame + frame_count/2)`,否则 `None`。**本批渲染路径不调用**(旗标化 = 语义钉定 + 为阴影批铺接口;变暗 DARKEN 表未钉顺延,研究 §9 L7)——须 `#[allow(dead_code)]` 压 unused 警告(注释带理由)。Preview 直方图断言不动(差率监督面)。

- [ ] **Step 1: 写失败测试(函数不存在 → 编译红)**

在 `crates/core/src/render/shape.rs` tests 模块末尾(`clip_and_outside` 测试之后、模块收尾 `}` 之前)追加:

```rust
    /// 阴影帧号规则(spec §4,纯函数渲染不接线):偶数帧数 0 → n/2;
    /// 奇数 floor(n/2);越界(含零帧数)None。
    /// 引擎出处:cell.cpp:1393 `frame + Get_Count()/2`;terrain.cpp:282-283
    /// 同式;跨类型惯例 builtype.cpp:771-802。
    #[test]
    fn shadow_frame_index_half_offset() {
        assert_eq!(shadow_frame_index(0, 4), Some(2), "偶数帧数:0 → n/2");
        assert_eq!(shadow_frame_index(1, 4), Some(3));
        assert_eq!(shadow_frame_index(2, 4), None, "2+2 ≥ 4 → 越界 None");
        assert_eq!(shadow_frame_index(3, 4), None);
        assert_eq!(shadow_frame_index(0, 5), Some(2), "奇数帧数:floor(n/2)");
        assert_eq!(shadow_frame_index(2, 5), Some(4));
        assert_eq!(shadow_frame_index(3, 5), None);
        assert_eq!(shadow_frame_index(0, 0), None, "零帧数防御");
    }
```

- [ ] **Step 2: 跑测试验证红**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p ra2-core --lib shape
```

预期:**编译失败**(`shadow_frame_index` 未定义)。

- [ ] **Step 3: 实现纯函数**

在 `crates/core/src/render/shape.rs` 的 `load_shp` 函数结束(`}`)之后、`#[cfg(test)]` 之前插入:

```rust
/// 阴影半帧号惯例:SHP 后半 = 阴影半,阴影帧 = 本体帧 + 帧数/2
/// (cell.cpp:1393 Draw_Overlay_Shadow `frame + Get_Count()/2`;
/// terrain.cpp:282-283 同式;跨类型惯例 builtype.cpp:771-802)。
/// M4c 只钉语义不接线(变暗 DARKEN 表未钉顺延,研究 §9 L7);渲染路径
/// 不调用,为阴影批铺接口。
#[allow(dead_code)] // 旗标化:渲染接线顺延(DARKEN 表未钉,研究 §9 L7)
pub(crate) fn shadow_frame_index(frame: usize, frame_count: usize) -> Option<usize> {
    let idx = frame + frame_count / 2;
    (idx < frame_count).then_some(idx)
}
```

- [ ] **Step 4: 跑测试验证绿**

```bash
cargo test -p ra2-core --lib shape
```

预期:全部 PASS。

- [ ] **Step 5: 全量门禁**

```bash
cargo test --workspace
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
```

预期:全绿(无渲染行为变化;`#[allow(dead_code)]` 压住 unused 警告,clippy -D warnings 净)。

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/render/shape.rs
git commit -m "feat(m4c): shadow_frame_index 纯函数旗标化(渲染不接线,DARKEN 表顺延)

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 7: 真资产验收 m4c_acceptance.rs

**Files:**
- Create: `crates/core/tests/m4c_acceptance.rs`(照 `crates/core/tests/m4b_acceptance.rs` 模式;M4b 验收文件**原样保留**作回归绊线)

**Interfaces:**
- Consumes: `render_scene`(scene.rs 签名冻结)、`SceneStats.objects_z_fallback`(Task 5)、Task 3 新判别式、Task 4 VEIN 守卫(其行为面在 264211/0 oracle 内钉死)、`walk_extremes`、`probe_preview_diff` 同款四组合探针。
- Produces: 单一主测试 `m4c_all_maps_scene_render`(`#[ignore]`,需 `RA2_ASSETS_DIR`);真资产 `--ignored` 计数 39 → 40。两个 #[ignore] 以外的测试不需要(spec §6.1:本里程碑一个主测试即可)。

**验收断言全量(spec §6.1,数值逐字):**
1. 主断言:`overlay_fail == 0 && objects_fail == 0`(逐图零容忍,807 全量);
2. 总量 oracle:`overlay_drawn == 264211`、`objects_drawn == 514502`、`objects_missing_art == 3539`(负 oracle,防改名蒙混)、`objects_z_fallback == 9288`、`objects_skipped_voxel == 8320`、`objects_malformed == 1`、`overlay_no_type == 0`、`overlay_missing_file == 0`;
3. 守恒三恒等维持(地形七桶 / 非空 overlay 四桶 / 非 Smudge 物件五桶,逐图独立重数);
4. 预算 `ms < 10578` 维持;
5. Preview 维持口径:`preview_probed == 507`、最众组合 `"RGB"`、占比 `≥ 0.85`(照 M4b 原样,不收紧不放宽;跌破 85% = 真发现,停下带数回设计,禁止悄悄放宽);
6. terrain 层对账(监督):七桶逐桶对账打印 + `missing_file == 0` 硬断言(M4b 基线 `.superpowers/sdd/task-8-assets-run2.log:521` = missing_file 总计 0;逐图七桶恒等已在循环内断言 = 异常即停;解析器改动不得波及地形层)。

- [ ] **Step 1: 创建 `crates/core/tests/m4c_acceptance.rs`(完整文件)**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! M4c 真资产验收(spec §6;研究 §5):807 全量场景渲染,主断言
//! overlay_fail == 0 && objects_fail == 0(判别式改帧级 flags bit1 门控后
//! 205 个失败文件全修复),总量 oracle 照研究 §5.1 照钉(负 oracle
//! missing_art 3539 防改名蒙混);守恒三恒等 / 画布公式 / 预算 / Preview
//! 口径维持 M4b 原样(spec §6.1-5:不收紧不放宽,RGB 占比跌破 85% = 真发现,
//! 停下带数回设计,禁止悄悄放宽);terrain 七桶与 M4b 基线
//! (.superpowers/sdd/task-8-assets-run2.log:521)逐桶对账打印,
//! missing_file == 0 维持(解析器改动不得波及地形层)。
//! 单一主测试,ignore,需 RA2_ASSETS_DIR;M4b 验收(m4b_acceptance.rs)
//! 原样保留作回归绊线。
//!
//! 运行:RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" \
//!   cargo test -p ra2-core --test m4c_acceptance -- --ignored --nocapture

use ra2_core::format::art::ArtIndex;
use ra2_core::format::map::mapdoc::decode_map_text;
use ra2_core::format::map::objects::ObjectClass;
use ra2_core::format::map::MapFile;
use ra2_core::format::pal::Palette;
use ra2_core::format::rules::{OverlayTable, OVERLAY_NONE};
use ra2_core::format::theater::{theater_spec, TheaterAssets, TheaterIni};
use ra2_core::render::scene::render_scene;
use ra2_core::render::terrain::walk_extremes;

fn assets_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(
        std::env::var("RA2_ASSETS_DIR").expect("RA2_ASSETS_DIR 指向用户资产目录"),
    )
}

/// 递归收集 .map(m3_acceptance.rs:20-31 同构手法;语料 807 = Maps/ 540
/// + RMG MapUnits 267;扩展名只有 .map,无 .yrm)。
fn maps() -> Vec<std::path::PathBuf> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("read_dir({}) 失败: {e}", dir.display()))
            .flatten()
        {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().map(|x| x == "map").unwrap_or(false) {
                out.push(p);
            }
        }
    }
    let mut v = Vec::new();
    walk(&assets_dir(), &mut v);
    v.sort();
    v
}

/// 剧场调色板:可解析 → 真;缺席(DESERT/NEWURBAN/LUNAR)→ 哑(全黑)。
/// 返回 (调色板, 真彩?)。
fn theater_palette(
    assets: &TheaterAssets,
    t: ra2_core::format::map::meta::Theater,
) -> (Palette, bool) {
    let name = t.iso_palette_file();
    if let Some(b) = assets.lookup(name) {
        if let Ok(p) = Palette::parse(b) {
            return (p, true);
        }
    }
    (
        Palette {
            rgb: [[0u8; 3]; 256],
        },
        false,
    )
}

/// PPM 直写(P6 二进制;目检样本用)。
fn write_ppm(out: &std::path::Path, w: u32, h: u32, rgb: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut f = std::fs::File::create(out)?;
    write!(f, "P6\n{w} {h}\n255\n")?;
    f.write_all(rgb)
}

/// Preview 差异探针(m4b_acceptance probe_preview_diff 同款):最近邻缩采样
/// (渲染 → preview 尺寸),通道序(RGB/BGR)× 行序(正常/上下翻)四组合
/// 取最小差率——preview.rs 像素/通道序 M2 裁定走向 A(RGB 85.6%)。逐图
/// 打印,返回最优组合名供直方图统计。
fn probe_preview_diff(
    path: &std::path::Path,
    r: &ra2_core::render::scene::SceneRender,
    pv: &ra2_core::format::map::preview::PreviewImage,
) -> &'static str {
    let (w, h) = (r.width as usize, r.height as usize);
    let (pw, ph) = (pv.width as usize, pv.height as usize);
    let mut best = (usize::MAX, "");
    for (swap, flip) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut diff = 0usize;
        for py in 0..ph {
            for pxx in 0..pw {
                let sx = (pxx * w / pw.max(1)).min(w - 1);
                let sy_raw = py * h / ph.max(1);
                let sy = if flip {
                    (ph - 1 - py) * h / ph.max(1)
                } else {
                    sy_raw
                }
                .min(h - 1);
                let i = 3 * (sy * w + sx);
                let j = 3 * (py * pw + pxx);
                let (a, b) = if swap {
                    (pv.rgb[j + 2], pv.rgb[j])
                } else {
                    (pv.rgb[j], pv.rgb[j + 2])
                };
                let d = (r.rgb[i] as i32 - a as i32).abs()
                    + (r.rgb[i + 1] as i32 - pv.rgb[j + 1] as i32).abs()
                    + (r.rgb[i + 2] as i32 - b as i32).abs();
                if d > 96 {
                    diff += 1; // 通道和差 > 96 记差异(探针阈值,M4b 裁定)
                }
            }
        }
        let combo = match (swap, flip) {
            (false, false) => "RGB",
            (true, false) => "BGR",
            (false, true) => "RGB+flip",
            _ => "BGR+flip",
        };
        if diff < best.0 {
            best = (diff, combo);
        }
    }
    let total = pw * ph;
    eprintln!(
        "preview-diff {} 最优 {} {}/{} = {:.1}%",
        path.display(),
        best.1,
        best.0,
        total,
        100.0 * best.0 as f64 / total.max(1) as f64
    );
    best.1
}

/// rulesmd/artmd/art 表装载(m4b 同款):rulesmd 缺席 = 空表(lenient
/// 契约,全计 no_type),art 缺席层 = 空文档(合成缺省)。
fn load_tables(assets: &TheaterAssets) -> (OverlayTable, ArtIndex) {
    let table = assets
        .lookup("rulesmd.ini")
        .map(|b| OverlayTable::parse(&decode_map_text(b)))
        .unwrap_or_else(|| OverlayTable::parse(""));
    let art = ArtIndex::parse(
        &assets
            .lookup("artmd.ini")
            .map(decode_map_text)
            .unwrap_or_default(),
        &assets
            .lookup("art.ini")
            .map(decode_map_text)
            .unwrap_or_default(),
    );
    (table, art)
}

/// 逐剧场分档聚合(overlay 4 桶 + 物件 5 桶 + 信息位;m4b 同构)。
#[derive(Default)]
struct TheaterAgg {
    overlay: [usize; 4], // drawn, no_type, missing_file, fail
    objects: [usize; 5], // drawn, skipped_voxel, missing_art, malformed, fail
    frame_clamp: usize,
    clip: usize,
}

/// 全量渲染 + M4c 断言(spec §6.1 全量)。
#[test]
#[ignore]
fn m4c_all_maps_scene_render() {
    let assets = TheaterAssets::open(&assets_dir()).unwrap();
    let (table, art) = load_tables(&assets);
    let mut total = 0usize;
    let mut total_ms = 0u128;
    let mut max_ms: (u128, String) = (0, String::new());
    let mut all_ms: Vec<u128> = Vec::new();
    let mut over = [0usize; 4]; // drawn, no_type, missing_file, fail
    let mut objs = [0usize; 5]; // drawn, voxel, missing_art, malformed, fail
    let mut total_z_fallback = 0usize;
    // terrain 七桶对账累计(spec §6.1-6;桶序 = m4b 恒等式顺序:painted,
    // no_tile, missing_file, reserved, out_of_range, missing_slot, parse_fail)
    let mut terrain_cells = 0usize;
    let mut terrain_totals = [0usize; 7];
    let mut total_frame_clamp = 0usize;
    let mut total_clip = 0usize;
    // 剧场 → (渲染数, 真彩数, 已抽样?, 分档)
    let mut by_theater: std::collections::BTreeMap<String, (usize, usize, bool, TheaterAgg)> =
        Default::default();
    // Preview 最优组合直方图(真彩口径;断言照 M4b 原样,spec §6.1-5)
    let mut preview_hist: std::collections::BTreeMap<&'static str, usize> = Default::default();
    let mut preview_probed = 0usize;
    for path in maps() {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(m) = MapFile::parse(&bytes) else {
            continue;
        };
        let spec = theater_spec(m.map_meta.theater);
        let ini = TheaterIni::parse(&decode_map_text(
            assets.lookup(spec.ini_name).expect("剧场表在"),
        ))
        .expect("剧场 INI 可解析");
        let (pal, real) = theater_palette(&assets, m.map_meta.theater);
        let t0 = std::time::Instant::now();
        let r = render_scene(&m, &assets, &ini, &table, &art, &pal).expect("全量合成零 panic");
        let ms = t0.elapsed().as_millis();
        // 预算(spec §6.1-4):M4b 实测最慢 3526ms ×3 = 10578;研究 §5.5:
        // 解码增量 µs-ms 级,少一次 RAW 兜底试探反而略省
        assert!(
            ms < 10_578,
            "{} 耗时 {ms}ms 超预算(实测 max×3,硬帽 15s)",
            path.display()
        );
        let s = r.stats;
        // M4c 主断言(spec §6.1-1,逐图零容忍):判别式改帧级 flags bit1
        // 门控后 205 个失败文件全修复,任一非零即停(带图名停下回设计)
        assert_eq!(
            s.overlay_fail, 0,
            "{}: overlay_fail 非零——判别式遗漏面,停下带数回设计",
            path.display()
        );
        assert_eq!(
            s.objects_fail, 0,
            "{}: objects_fail 非零——判别式遗漏面,停下带数回设计",
            path.display()
        );
        total += 1;
        total_ms += ms;
        all_ms.push(ms);
        if ms > max_ms.0 {
            max_ms = (ms, path.display().to_string());
        }
        // terrain 七桶累计 + 守恒 1:地形七桶恒等(cells = 其和)
        terrain_cells += s.terrain.cells;
        let tb = [
            s.terrain.painted,
            s.terrain.no_tile,
            s.terrain.missing_file,
            s.terrain.reserved,
            s.terrain.out_of_range,
            s.terrain.missing_slot,
            s.terrain.parse_fail,
        ];
        for (a, v) in terrain_totals.iter_mut().zip(tb) {
            *a += v;
        }
        assert_eq!(s.terrain.cells, m.tiles.len(), "{}", path.display());
        assert_eq!(
            s.terrain.painted
                + s.terrain.no_tile
                + s.terrain.missing_file
                + s.terrain.reserved
                + s.terrain.out_of_range
                + s.terrain.missing_slot
                + s.terrain.parse_fail,
            s.terrain.cells,
            "{}: 地形七桶恒等",
            path.display()
        );
        // 守恒 2:非空 overlay 四桶(独立重数,overlay pass 同域 = d 升序
        // records;scene.rs 排序键 u32::from(x)+u32::from(y),此处同式重构
        // 不信 stats)
        let mut records = m.tiles.records().to_vec();
        records.sort_by_key(|t| u32::from(t.x) + u32::from(t.y));
        let non_empty = records
            .iter()
            .filter(|t| m.overlays.type_at(t.x, t.y) != OVERLAY_NONE)
            .count();
        assert_eq!(
            non_empty,
            s.overlay_drawn + s.overlay_no_type + s.overlay_missing_file + s.overlay_fail,
            "{}: 非空 overlay 四桶恒等(每记录恰一计数器)",
            path.display()
        );
        // 守恒 3:非 Smudge 物件五桶(objects.rs tests assert_conservation 同式)
        let non_smudge = m
            .objects
            .all
            .iter()
            .filter(|o| o.class != ObjectClass::Smudge)
            .count();
        assert_eq!(
            non_smudge,
            s.objects_drawn
                + s.objects_skipped_voxel
                + s.objects_missing_art
                + s.objects_malformed
                + s.objects_fail,
            "{}: 非 Smudge 物件五桶恒等(每件恰一计数器)",
            path.display()
        );
        // 画布公式独立重算(M4a 口径):W/H 均由 walk_extremes + 实扫 z_max
        let (w, h) = m.size_wh();
        let (d_min, d_max, xmy_min, xmy_max) = walk_extremes(w, h);
        let z_max = m
            .tiles
            .records()
            .iter()
            .map(|t| t.z as usize)
            .max()
            .unwrap_or(0);
        assert_eq!(
            r.width as usize,
            30 * ((xmy_max - xmy_min) as usize) + 60,
            "{}: W = 30·(x−y 跨度)+60",
            path.display()
        );
        assert_eq!(
            r.height as usize,
            15 * ((d_max - d_min) as usize) + 30 + 15 * z_max,
            "{}: H = 15·(d 跨度)+30+15·z_max",
            path.display()
        );
        // 总账累计
        let ov = [
            s.overlay_drawn,
            s.overlay_no_type,
            s.overlay_missing_file,
            s.overlay_fail,
        ];
        let ob = [
            s.objects_drawn,
            s.objects_skipped_voxel,
            s.objects_missing_art,
            s.objects_malformed,
            s.objects_fail,
        ];
        for (a, v) in over.iter_mut().zip(ov) {
            *a += v;
        }
        for (a, v) in objs.iter_mut().zip(ob) {
            *a += v;
        }
        total_z_fallback += s.objects_z_fallback;
        total_frame_clamp += s.frame_clamp;
        total_clip += s.clip;
        let key = format!("{:?}", m.map_meta.theater);
        let entry = by_theater.entry(key).or_default();
        entry.0 += 1;
        for (a, v) in entry.3.overlay.iter_mut().zip(ov) {
            *a += v;
        }
        for (a, v) in entry.3.objects.iter_mut().zip(ob) {
            *a += v;
        }
        entry.3.frame_clamp += s.frame_clamp;
        entry.3.clip += s.clip;
        // 真彩剧场:抽样 PPM + Preview 探针(哑调色板必 100% 差,不参与)
        if real {
            entry.1 += 1;
            if !entry.2 {
                entry.2 = true;
                let out = std::env::temp_dir().join(format!(
                    "ra2-m4c-scene-sample-{}.ppm",
                    path.file_name().unwrap().to_string_lossy()
                ));
                write_ppm(&out, r.width, r.height, &r.rgb).unwrap();
                eprintln!("样本: {}", out.display());
            }
            if let Some(pv) = &m.preview {
                let combo = probe_preview_diff(&path, &r, pv);
                *preview_hist.entry(combo).or_insert(0) += 1;
                preview_probed += 1;
            }
        }
    }
    // 总账行(控制器对账输入;M4b 基线 = task-8-assets-run2.log:521-524)
    all_ms.sort_unstable();
    let p90 = all_ms
        .get((((all_ms.len() as f64) * 0.9).ceil() as usize).saturating_sub(1))
        .copied()
        .unwrap_or(0);
    let mean = if total > 0 {
        total_ms / total as u128
    } else {
        0
    };
    eprintln!(
        "M4c: 共 {total} 图,总耗时 {total_ms}ms,最慢 {max_ms:?},均值 {mean}ms,p90 {p90}ms"
    );
    eprintln!(
        "M4c overlay 总账: drawn {} no_type {} missing_file {} fail {}(frame_clamp {total_frame_clamp}, clip {total_clip})",
        over[0], over[1], over[2], over[3]
    );
    eprintln!(
        "M4c objects 总账: drawn {} voxel {} missing_art {} malformed {} fail {} z_fallback {total_z_fallback}",
        objs[0], objs[1], objs[2], objs[3], objs[4]
    );
    // terrain 七桶对账打印(M4b 基线 log:521 = missing_file 总计 0;硬断言
    // 在下方,其余桶以逐图七桶恒等 + 打印对账)
    eprintln!(
        "M4c terrain 对账(M4b 基线 missing_file 总计 0): cells {terrain_cells} painted {} no_tile {} missing_file {} reserved {} out_of_range {} missing_slot {} parse_fail {}",
        terrain_totals[0], terrain_totals[1], terrain_totals[2],
        terrain_totals[3], terrain_totals[4], terrain_totals[5], terrain_totals[6]
    );
    let sprites = over[0] + objs[0];
    eprintln!(
        "M4c clip 占比: {total_clip}/{sprites} = {:.2}%",
        100.0 * total_clip as f64 / sprites.max(1) as f64
    );
    for (k, (maps_n, real_n, sampled, agg)) in &by_theater {
        eprintln!(
            "剧场 {k}: {maps_n} 图(真彩 {real_n},抽样 {sampled})overlay[drawn {} no_type {} missing_file {} fail {}] objects[drawn {} voxel {} missing_art {} malformed {} fail {}] frame_clamp {} clip {}",
            agg.overlay[0], agg.overlay[1], agg.overlay[2], agg.overlay[3],
            agg.objects[0], agg.objects[1], agg.objects[2], agg.objects[3], agg.objects[4],
            agg.frame_clamp, agg.clip
        );
    }
    eprintln!("M4c preview 直方图(真彩含 Preview 共 {preview_probed} 图):");
    for (k, v) in &preview_hist {
        eprintln!(
            "  {k}: {v} = {:.1}%",
            100.0 * *v as f64 / preview_probed.max(1) as f64
        );
    }
    // === M4c 总量 oracle(spec §6.1-2;研究 §5.1 修复后预测,语料冻结前提;
    // 数值逐字照钉,不得改写或四舍五入)===
    assert_eq!(total, 807, "语料 = 807(M2/M3/M4a/M4b 口径)");
    assert_eq!(
        over[0], 264211,
        "overlay_drawn == 264211(人口恒等:235265 drawn + 28946 fail)"
    );
    assert_eq!(over[1], 0, "overlay_no_type == 0");
    assert_eq!(over[2], 0, "overlay_missing_file == 0");
    assert_eq!(over[3], 0, "overlay_fail == 0(主断言总量口径)");
    assert_eq!(
        objs[0], 514502,
        "objects_drawn == 514502(511347 drawn + 3155 fail)"
    );
    assert_eq!(objs[1], 8320, "objects_skipped_voxel == 8320");
    assert_eq!(
        objs[2], 3539,
        "objects_missing_art == 3539(负 oracle:真缺档 95 名,TS 语源未随 YR 零售出货;防改名蒙混)"
    );
    assert_eq!(objs[3], 1, "objects_malformed == 1");
    assert_eq!(objs[4], 0, "objects_fail == 0(主断言总量口径)");
    assert_eq!(
        total_z_fallback, 9288,
        "objects_z_fallback == 9288(研究 §4.7 语料量级)"
    );
    // === Preview 口径照 M4b 原样维持(spec §6.1-5:不收紧不放宽)===
    assert_eq!(preview_probed, 507, "真彩含 Preview 图数钉死(T8 实测)");
    let (best_combo, best_count) = preview_hist
        .iter()
        .max_by_key(|(_, v)| **v)
        .map(|(k, v)| (*k, *v))
        .unwrap_or(("", 0));
    assert_eq!(
        best_combo, "RGB",
        "最众组合钉 RGB(M4b T8 实测 85.6%;跌破 85% = 真发现,停下带数回设计,禁止悄悄放宽)"
    );
    assert!(
        best_count as f64 / preview_probed.max(1) as f64 >= 0.85,
        "RGB 稳定占比 ≥85%(实得 {best_combo} {best_count}/{preview_probed})"
    );
    // terrain 对账硬面(spec §6.1-6:M4b 基线 missing_file 总计 0;逐图七桶
    // 恒等已在循环内断言 = 异常即停;解析器改动不得波及地形层)
    assert_eq!(
        terrain_totals[2], 0,
        "terrain missing_file == 0(M4b 基线 task-8-assets-run2.log:521)"
    );
    let real_theaters = by_theater.values().filter(|v| v.2).count();
    assert!(
        real_theaters >= 3,
        "真彩抽样至少覆盖 TEMPERATE/SNOW/URBAN,实得 {real_theaters}"
    );
}
```

- [ ] **Step 2: 编译与非 ignore 运行(合成环境门禁)**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p ra2-core --test m4c_acceptance
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
```

预期:`cargo test -p ra2-core --test m4c_acceptance` = `0 passed`(测试被 ignore,编译通过即可);fmt/clippy 净。

- [ ] **Step 3: 全量门禁(合成面)**

```bash
cargo test --workspace
```

预期:全绿(真资产测试全部 ignore,不阻塞)。

- [ ] **Step 4: 真资产验收(需 RA2_ASSETS_DIR)**

```bash
RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test m4c_acceptance -- --ignored --nocapture
```

预期:`m4c_all_maps_scene_render ... ok`;stderr 总账行 `M4c overlay 总账: drawn 264211 no_type 0 missing_file 0 fail ... 0`、`M4c objects 总账: drawn 514502 voxel 8320 missing_art 3539 malformed 1 fail 0 z_fallback 9288`、`M4c terrain 对账 ... missing_file 0`、preview 直方图 RGB 最众 ≥85%、`preview_probed` 计 507、共 807 图。**任何一条不中 = 真发现**:停下带完整总账输出回设计(禁止放宽断言;overlay/objects fail 非零时按研究 §5.4 首查 SROCK/TROCK 跨剧场副本选择序)。

回归绊线复核(同一环境):

```bash
RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored
```

预期:全绿 0 fail,ignored 计数 40(39 + m4c_acceptance 新增 1)——含 M1b/M3/M4a/M4b 既有真资产套件(Task 2 下限放宽对 TMP 的零回归监督面,spec §5.4)。

- [ ] **Step 5: Commit**

```bash
git add crates/core/tests/m4c_acceptance.rs
git commit -m "test(m4c): 真资产验收 m4c_acceptance——fail==0 主断言 + 总量 oracle 照钉 + terrain 对账

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

## 收尾清单(全部任务完成后)

- [ ] `cargo test --workspace` 全绿;`cargo fmt --all --check` 净;`cargo clippy --all-targets -- -D warnings` 净。
- [ ] `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored` 全绿,ignored 计数 40。
- [ ] 合成测试总数上报(较 272 增长的精确数,钉台账)。
- [ ] 顺延台账复核(spec §7):remap / 坡地 ore / DARKEN 表 / −2px+SHAPE_ALPHA+ZWRITE / rubble(语料 0) / 别名表 / voxel(M5)——均未实现,仅注释/文档记账。
