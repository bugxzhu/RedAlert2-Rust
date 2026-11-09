# M4b overlay 层 + 物件 SHP 层渲染 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 M4a 整幅地形画布上追加 overlay 全量层与物件 SHP 三类(Terrain/Structures/Infantry;Units/Aircraft=voxel 计数跳过),交付 `render_scene` 场景合成器 + CLI 场景输出 + 807 全量验收与 Preview 通道/行序终验裁定。

**Architecture:** 方案 A 场景 pass 追加——`render::scene` 复用 `compose_terrain`(画布公式不动),之上叠 overlay pass(records 菱形 d 升序)与物件 pass(Sort_Y 升序稳定),共用 `SceneStats` 统计与 SHP 名→解析缓存;rules/art 各一小块 INI 解析器(`format::rules`/`format::art`),物件字段类型化由 M2 `objects.rs` 既有访问器 + 新增 `render_facing` 补齐。

**Tech Stack:** Rust 2021,ra2-rs 零依赖 core(std only),PNG 写盘沿用 tools 现有 png 模块;证据源 ra2-decomp(只读,语义重写注出处)。

## Global Constraints

- 每个**新文件首行** `// SPDX-License-Identifier: GPL-3.0-only`;改动文件保持既有 SPDX。
- 分支 `feat/m4b-overlay-objects`(已建,首提交 b607895);**绝不在 main 上直接提交实现**。
- decomp 引证 = E:\hongjing\ra2-decomp(只读)file:line;实现一律**语义重写**并注释出处,绝不逐字转录。
- 真资产仅 `RA2_ASSETS_DIR` 只读;测试夹具全合成(临时目录 mix),**任何 EA 资产不进 git**。
- lenient 契约:敌意输入计数跳过零 panic(同 M4a 口径);统计守恒断言。
- **画布尺寸公式不动**(M4a 807 尺寸断言保持有效);精灵越界裁剪并计 `clip`。
- 调色板:剧场 iso 表直查(无 remap;所属方色按 iso 表原样渲)。
- 透明语义:SHP-TS 像素值 0 跳过;帧号越界钳末帧计 `frame_clamp`。
- 门禁:`cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace` 全绿(clippy 1.99;`chunks_exact` 常量参用 `as_chunks_mut::<N>().0` 先例)。
- 每任务提交信息尾部:`Co-Authored-By: Claude Code <noreply@anthropic.com>`。
- 性能预算:807 实测 max_ms×3,硬帽 15000ms/图。
- Preview 终验:最优组合稳定占比 ≥85%(真彩口径)才钉断言;差率分布只报告不断言。
- 语义钉死出处(全部已实钉,计划内代码直接使用):研究 `docs/superpowers/research/2026-11-06-m4b-overlay-objects.md` §1-§9 + spec `docs/superpowers/specs/2026-11-06-m4b-overlay-objects-render-design.md`。

---

### Task 0: M4a 终审遗留 N1-N3 + 研究文档勘误

**Files:**
- Modify: `crates/core/src/render/terrain.rs`(N1 上溢门 + 对应测试)
- Modify: `crates/core/src/render/mod.rs:9-12`(N2 守卫注释)
- Modify: `docs/superpowers/research/2026-11-06-m4a-terrain-render.md`(N3 引用行)
- Modify: `docs/superpowers/research/2026-11-06-m4b-overlay-objects.md`(§5 键序勘误)

**Interfaces:** 无代码接口;N1 只改敌意分支与测试期望。

- [ ] **Step 1: N1 terrain.rs 上溢侧门**

`compose_terrain` 中(约 :160)把:

```rust
} else if sx < 0 || sy < 0 {
```

替换为:

```rust
} else if sx < 0
    || sy < 0
    || sx as u64 + CELL_W as u64 > width as u64
    || sy as u64 + CELL_H as u64 > height as u64
{
```

注释扩一行:`// 上溢侧同计(parse_fail):越出走位域的记录可能仅右/下越界(M4a N1)`。需在文件顶部 use 域确认 `CELL_W`/`CELL_H` 可见(同模块 `use super::{...}` 已含则不动)。

- [ ] **Step 2: 找到并翻转既有越域测试的期望**

在 terrain.rs tests 内搜断言"越域记录仍计 painted"的测试(M4a C1 修复时留下的下溢断言组,含 hostile/越域字样)。给该夹具补一条**仅上溢**的记录(如 w=2,h=1 域上,取走位域外但包围盒内的格,`tile(2, 1, 5, 0, 0)` 起按既有夹具坐标类推),断言 `stats.parse_fail` 计入而 `painted` 不增:

```rust
// N1:仅上溢越域(右/下缘越界)→ parse_fail,不再计 painted
assert_eq!(stats.parse_fail, 旧值 + 1);
assert_eq!(stats.painted, 旧值);
```

(若既有测试形态不同,以"新增一条仅上溢记录 + parse_fail +1 + painted 不变"为准则改写;守恒恒等式 cells=七桶和必须保持。)

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p ra2-core render::`
Expected: 全部 pass(含改写后的越域测试)。

- [ ] **Step 3: N2 mod.rs 守卫注释**

`render/mod.rs:9-12` 的常量 doc 替换为:

```rust
/// 敌意画布上界(spec §5):走位极值画布口径,语料最大 [Map]Size 对角 430
/// (272×158)→ 16,320×4,965 ≈ 81 Mpx(护栏余量 1.66×);合法方图上限
/// 272×272(16,320×8,205 ≈ 133.4 Mpx,恰过护栏)——60w·(30w+15) ≤ 2²⁷
/// 解得 w ≤ 272.06;4096² 敌意图(约 30 Gpx)拒绝。
```

- [ ] **Step 4: N3 + 研究文档勘误(两处 docs)**

(a) `2026-11-06-m4a-terrain-render.md` §3(语料实测)末尾补一行:

```markdown
- no_tile 371(c1a01md)= M2 研究 2026-11-04-map-pack §12 样本首值,独立
  交叉验证(C1 终审补引,N3)。
```

(b) `2026-11-06-m4b-overlay-objects.md` §5 中 `键 = X·1000+Y` 一行替换为:

```markdown
- `[Terrain]` `KeyY1000X=TreeName`(**键 = Y·1000+X,M2 裁定,objects.rs:73
  `n % 1000 → x, n / 1000 → y` 已实现**;37084 → x=84,y=37;控制器预写
  X·1000+Y 系笔误,此行即勘误);
```

- [ ] **Step 5: 门禁 + 提交**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 245 passed / 0 failed / 37 ignored(N1 只改计数与注释,数量不变)。

```bash
git add crates/core/src/render/terrain.rs crates/core/src/render/mod.rs docs/superpowers/research/2026-11-06-m4a-terrain-render.md docs/superpowers/research/2026-11-06-m4b-overlay-objects.md
git commit -m "fix(render): 敌意越域记录上溢侧计 parse_fail(N1)+守卫注释方图上限 272²(N2)+M4a 研究 371 交叉引用(N3)+M4b 研究键序勘误(M4a 终审遗留清欠,M4b T0)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 1: `format::rules` OverlayTable([OverlayTypes] 表位 → 图名/旗标/ore 游程)

**Files:**
- Create: `crates/core/src/format/rules.rs`
- Modify: `crates/core/src/format/mod.rs`(加 `pub mod rules;`,紧邻既有 `pub mod pal;` 类似行)

**Interfaces:**
- Consumes: `crate::format::ini::IniDocument`(parse/section/value/get_bool,ci 查找,entries 保文件序)。
- Produces(T4/T6 依赖,签名冻结):

```rust
pub struct OverlayTable { /* 私有 Vec<Entry> */ }
pub struct OverlayKind<'a> { pub image: &'a str, pub tiberium: bool, pub is_crate: bool, pub bridge: bool }
impl OverlayTable {
    pub fn parse(text: &str) -> OverlayTable;
    pub fn kind(&self, byte: u8) -> Option<OverlayKind<'_>>;      // None = 0xFF 或越表
    pub fn display_byte(&self, byte: u8, x: u16, y: u16) -> u8;    // ore 游程变体
    pub fn len(&self) -> usize;
}
```

- 引擎语义(注释里注明出处,语义重写):rules.cpp:2068-2079 按文件序 Find_Or_Make → overtype.cpp:97-98 HeapID=追加位 ⇒ **地图字节=段内条目位置,键数字不参与**;Image= 缺省=类型名(objtype.cpp:121/730-731);Tiberium/Crate 逐型节旗标(overtype.cpp Read_INI:215-217);ore 显示型 = 基+(X·Y)%段长(cell.cpp:1451,游程检测等价 overlay.cpp:599 命中判据);桥名前缀 bridge 系启发(桥面板变体表见 T4)。

- [ ] **Step 1: 写失败测试**(rules.rs 尾部 `#[cfg(test)] mod tests`,先建只含 SPDX+空 struct+tests 的文件使编译失败)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const RULES: &str = "\
[OverlayTypes]
0=GASAND
1=WALL01
2=ORE01
3=ORE02
4=ORE03
5=GEM01
6=BRIDGE1
[ORE01]
Tiberium=yes
[ORE02]
Tiberium=yes
Image=OREXX
[ORE03]
Tiberium=yes
[GEM01]
Crate=yes
";

    #[test]
    fn byte_is_list_position_not_key() {
        // 键数字故意乱序:位置语义(0→GASAND,2→ORE01)
        let text = RULES.replace("0=GASAND", "7=GASAND");
        let t = OverlayTable::parse(&text);
        assert_eq!(t.kind(0).unwrap().image, "GASAND");
        assert_eq!(t.kind(1).unwrap().image, "WALL01");
        assert!(t.kind(7).is_none()); // 越表
        assert!(t.kind(0xFF).is_none()); // OVERLAY_NONE
        assert_eq!(t.len(), 7);
    }

    #[test]
    fn image_override_and_flags() {
        let t = OverlayTable::parse(RULES);
        assert_eq!(t.kind(3).unwrap().image, "OREXX"); // Image= 覆盖
        assert!(t.kind(3).unwrap().tiberium);
        assert!(t.kind(5).unwrap().is_crate); // GEM01 Crate=yes
        assert!(!t.kind(5).unwrap().tiberium);
        assert!(t.kind(6).unwrap().bridge); // 名前缀 bridge(ci)
    }

    #[test]
    fn tiberium_run_display_byte() {
        let t = OverlayTable::parse(RULES);
        // 游程 [2,5):基=2,长=3;变体 = 2 + (X·Y)%3
        assert_eq!(t.display_byte(2, 0, 0), 2);
        assert_eq!(t.display_byte(3, 1, 1), 2 + 1); // 1%3=1
        assert_eq!(t.display_byte(4, 2, 2), 2 + 1); // 4%3=1
        assert_eq!(t.display_byte(4, 1, 0), 2);     // 0%3=0
        // 非 tiberium 原样;0xFF 原样
        assert_eq!(t.display_byte(1, 5, 7), 1);
        assert_eq!(t.display_byte(0xFF, 5, 7), 0xFF);
    }

    #[test]
    fn missing_section_is_empty_table() {
        let t = OverlayTable::parse("[Whatever]\n1=x\n");
        assert_eq!(t.len(), 0);
        assert!(t.kind(0).is_none());
    }
}
```

- [ ] **Step 2: 跑红**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p ra2-core format::rules`
Expected: 编译失败(`kind`/`display_byte` 未定义)。

- [ ] **Step 3: 实现**(完整文件)

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! rulesmd.ini 的 overlay 侧解析(M4b spec §3/§5;研究 §2)。
//!
//! 引擎语义(ra2-decomp 只读佐证,语义重写):
//! - [OverlayTypes] 按文件序逐条 Find_Or_Make(rules.cpp:2068-2079),
//!   HeapID=追加位(overtype.cpp:97-98)→ **地图 OverlayPack 字节 = 段内
//!   条目位置(0 基),条目键数字不参与**。
//! - 每类型节 Image= 缺省=类型名(objtype.cpp:121/730-731);
//!   Tiberium=/Crate= 旗标(overtype.cpp Read_INI:215-217)。
//! - ore(IsTiberium)显示型 = 游程基 + (X·Y)%游程长(cell.cpp:1451;
//!   命中判据=字节∈[基,基+长) 与 overlay.cpp:599 同构;游程长=Variety
//!   的零售等价,Image= 整型选择器分支(tiberium.cpp:171+)不感知)。
//! - 桥类型名前缀 bridge(ci)系启发式锚定(cell.cpp:1417 变体表消费方);
//!   零售名核对在 807 验收(T8)。

use crate::format::ini::IniDocument;

/// 0xFF = OVERLAY_NONE(overlay.cpp:418-419)。
pub const OVERLAY_NONE: u8 = 0xFF;

/// 逐型条目(表位 → 图名/旗标)。
struct Entry {
    image: String,
    tiberium: bool,
    is_crate: bool,
    bridge: bool,
}

/// 渲染期只读视图(T4 消费)。
pub struct OverlayKind<'a> {
    pub image: &'a str,
    pub tiberium: bool,
    pub is_crate: bool,
    pub bridge: bool,
}

/// [OverlayTypes] 顺序表:地图字节 = 表位。
pub struct OverlayTable {
    entries: Vec<Entry>,
}

impl OverlayTable {
    /// rulesmd 文本 → 表;段缺失 = 空表(调用方全计 no_type)。
    pub fn parse(text: &str) -> OverlayTable {
        let doc = IniDocument::parse(text);
        let mut entries = Vec::new();
        if let Some(list) = doc.section("OverlayTypes") {
            for (_, name) in &list.entries {
                let name = name.trim();
                if name.is_empty() {
                    continue;
                }
                let image = doc
                    .value(name, "Image")
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or(name)
                    .to_string();
                entries.push(Entry {
                    image,
                    tiberium: doc.get_bool(name, "Tiberium", false),
                    is_crate: doc.get_bool(name, "Crate", false),
                    bridge: name.to_ascii_lowercase().starts_with("bridge"),
                });
            }
        }
        OverlayTable { entries }
    }

    /// 字节位 → 渲染视图;0xFF/越表 → None(调用方计 overlay_no_type)。
    pub fn kind(&self, byte: u8) -> Option<OverlayKind<'_>> {
        if byte == OVERLAY_NONE {
            return None;
        }
        let e = self.entries.get(byte as usize())?;
        Some(OverlayKind {
            image: e.image.as_str(),
            tiberium: e.tiberium,
            is_crate: e.is_crate,
            bridge: e.bridge,
        })
    }

    /// ore 游程变体(cell.cpp:1451 非坡支):tiberium 字节 → 游程基 +
    /// (x·y)%游程长;其余原样。游程 = 连续 tiberium 条目的极大段。
    pub fn display_byte(&self, byte: u8, x: u16, y: u16) -> u8 {
        if byte == OVERLAY_NONE {
            return byte;
        }
        if !self.entries.get(byte as usize).is_some_and(|e| e.tiberium) {
            return byte;
        }
        let mut base = byte as usize;
        while base > 0 && self.entries[base - 1].tiberium {
            base -= 1;
        }
        let mut len = 1usize;
        while base + len < self.entries.len() && self.entries[base + len].tiberium {
            len += 1;
        }
        let v = (u32::from(x) * u32::from(y)) % len as u32;
        (base as u32 + v).min(0xFE) as u8
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
```

并在 `format/mod.rs` 模块声明区(按字母序)加 `pub mod rules;`。

- [ ] **Step 4: 跑绿 + 全量门禁**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p ra2-core format::rules && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 4 新测试 pass;workspace 全绿。

- [ ] **Step 5: 提交**

```bash
git add crates/core/src/format/rules.rs crates/core/src/format/mod.rs
git commit -m "feat(format): OverlayTable——[OverlayTypes] 表位语义/Image 覆盖/Tiberium·Crate 旗标/ore 游程变体(M4b T1)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 2: `format::art` ArtIndex(artmd→art 分层索引)

**Files:**
- Create: `crates/core/src/format/art.rs`
- Modify: `crates/core/src/format/mod.rs`(加 `pub mod art;`)

**Interfaces:**
- Consumes: `IniDocument`(ci section/value/parse_int_prefix)。
- Produces(T5/T6 依赖,签名冻结):

```rust
pub struct ArtEntry { pub image: String, pub facings: u32, pub stand_frame: u32 }
pub struct ArtIndex { /* 私有双 IniDocument */ }
impl ArtIndex {
    pub fn parse(artmd: &str, art: &str) -> ArtIndex;
    pub fn get_or_default(&self, type_name: &str, default_facings: u32) -> ArtEntry;
}
```

- 语义:分层**按键**(artmd 先中即返,art 兜底——YR artmd 仅覆盖差量,引擎分层合并;研究 §7);无节 → image=类型名、facings=default(引擎缺省 GraphicName=IniName,objtype.cpp:121;facings 缺省:建筑/地形 1、步兵 8,unittype.cpp:97 同源缺省);StandFrame 缺省 0(键不存在 → 0,L5 低置信,验收监督)。

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const MD: &str = "\
[GI]
Cameo=GIICON
Facings=32
";
    const BASE: &str = "\
[GI]
Image=E1
Facings=8
StandFrame=3
[TREE11]
Image=TREEXX
";

    #[test]
    fn per_key_layering_artmd_wins() {
        let a = ArtIndex::parse(MD, BASE);
        let gi = a.get_or_default("GI", 8);
        assert_eq!(gi.image, "E1"); // Image 仅 art 有 → 兜底命中
        assert_eq!(gi.facings, 32); // Facings artmd 优先
        assert_eq!(gi.stand_frame, 3);
    }

    #[test]
    fn no_section_synthesizes_default() {
        let a = ArtIndex::parse(MD, BASE);
        let e = a.get_or_default("CABHUT", 1);
        assert_eq!(e.image, "CABHUT"); // 无节 → 类型名(objtype.cpp:121)
        assert_eq!(e.facings, 1);
        assert_eq!(e.stand_frame, 0);
        let t = a.get_or_default("TREE11", 1);
        assert_eq!(t.image, "TREEXX"); // 有节无 Facings → default
        assert_eq!(t.facings, 1);
    }

    #[test]
    fn hostile_facings_filtered_to_default() {
        let md = "[X]\nFacings=0\n[Y]\nFacings=-4\n[Z]\nFacings=999999\n";
        let a = ArtIndex::parse(md, "");
        assert_eq!(a.get_or_default("X", 8).facings, 8);
        assert_eq!(a.get_or_default("Y", 8).facings, 8);
        assert_eq!(a.get_or_default("Z", 8).facings, 8);
        // StandFrame 敌意同滤
        let a2 = ArtIndex::parse("[W]\nStandFrame=-1\n", "");
        assert_eq!(a2.get_or_default("W", 8).stand_frame, 0);
    }

    #[test]
    fn both_empty_is_all_defaults() {
        let a = ArtIndex::parse("", "");
        let e = a.get_or_default("ANY", 8);
        assert_eq!(e.image, "ANY");
        assert_eq!(e.facings, 8);
    }
}
```

- [ ] **Step 2: 跑红**(同 T1 手法:空壳文件 + tests)

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p ra2-core format::art`
Expected: 编译失败。

- [ ] **Step 3: 实现**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! artmd.ini → art.ini 分层索引(M4b spec §3;研究 §7)。
//!
//! YR 的 artmd 仅含覆盖差量,引擎分层合并——**按键**分层(artmd 先中
//! 即返,art 兜底),非按节整体。无节 → image=类型名、facings=调用方
//! 缺省(建筑/地形 1、步兵 8;objtype.cpp:121 GraphicName=IniName、
//! unittype.cpp:97 Facings(8) 同源缺省)。StandFrame 缺省 0(键在
//! 零售 art 是否存在未钉,L5,807 验收监督)。

use crate::format::ini::{parse_int_prefix, IniDocument};

/// 一个类型的渲染 art 参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtEntry {
    pub image: String,
    pub facings: u32,
    pub stand_frame: u32,
}

/// 双层 art 索引。
pub struct ArtIndex {
    md: IniDocument,
    base: IniDocument,
}

fn layered<'a>(md: &'a IniDocument, base: &'a IniDocument, sec: &str, key: &str) -> Option<&'a str> {
    md.value(sec, key).or_else(|| base.value(sec, key))
}

impl ArtIndex {
    pub fn parse(artmd: &str, art: &str) -> ArtIndex {
        ArtIndex {
            md: IniDocument::parse(artmd),
            base: IniDocument::parse(art),
        }
    }

    /// 类型 → art 参数;无节合成缺省(image=类型名)。facings/stand_frame
    /// 敌意值(0/负/超大)过滤回缺省。
    pub fn get_or_default(&self, type_name: &str, default_facings: u32) -> ArtEntry {
        let has_section = self.md.section(type_name).is_some()
            || self.base.section(type_name).is_some();
        let image = if has_section {
            layered(&self.md, &self.base, type_name, "Image")
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(type_name)
                .to_string()
        } else {
            type_name.to_string()
        };
        let facings = layered(&self.md, &self.base, type_name, "Facings")
            .and_then(parse_int_prefix)
            .filter(|f| (1..=1024).contains(f))
            .map_or(default_facings.max(1), |f| f as u32);
        let stand_frame = layered(&self.md, &self.base, type_name, "StandFrame")
            .and_then(parse_int_prefix)
            .filter(|s| (0..=1024).contains(s))
            .map_or(0, |s| s as u32);
        ArtEntry {
            image,
            facings,
            stand_frame,
        }
    }
}
```

`format/mod.rs` 加 `pub mod art;`。

- [ ] **Step 4: 跑绿 + 门禁**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p ra2-core format::art && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 4 新测试 pass;全绿。

- [ ] **Step 5: 提交**

```bash
git add crates/core/src/format/art.rs crates/core/src/format/mod.rs
git commit -m "feat(format): ArtIndex——artmd→art 按键分层/Image 缺省类型名/Facings·StandFrame 敌意过滤(M4b T2)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 3: `objects.rs` 渲染朝向访问器(Infantry field 7)+ 语料抽验

**Files:**
- Modify: `crates/core/src/format/map/objects.rs`(impl MapObject 内加一个方法;tests 加一组)

**Interfaces:**
- Consumes: 既有 `MapObject::field/class`。
- Produces(T5 依赖):`pub fn render_facing(&self) -> Option<u32>`——Structure/Unit/Aircraft 取 field 5;**Infantry 取 field 7**(其 field 5 是 sub_cell;真图实证研究 §5:austintx COW 行 [7]∈{32,64,158,186});Terrain/Smudge → None;非整数或 >255 → None。**既有 `facing()`(M2 冻结语义)不动。**

- [ ] **Step 1: 语料抽验(先证后码)**

Run: `cd "/e/hongjing/Reunion 2023" && rg -N -m6 '^\d+=[^,]+,[^,]+,256,\d+,\d+,[0-5],Guard,\d+,' "Maps/Standard/austintx.map"`
Expected: 6 行形如 `0=Neutral,COW,256,128,42,4,Guard,32,…`(field 5=sub_cell 0..5,field 7=朝向整数)。
再抽一张:`rg -N -m4 '^\d+=[^,]+,[^,]+,\d+,\d+,\d+,[0-5],[A-Za-z]+,\d+,' "Maps/Cooperative/c1a02md.map"`(同构)。若两图步兵 field 7 恒整数 0..255 → 语义成立;异常 → BLOCKED 上报控制器。

> spec §4 的 `format/map/objects_typed.rs` 独立文件**收敛不建**:M2 `objects.rs` 已有 class/kind/cell/sub_cell/owner 类型化访问器,本任务仅补渲染朝向缺口(spec 勘误记 T9)。

- [ ] **Step 2: 写失败测试**(objects.rs tests 追加;用既有 tests 的行构造助手——名字不同则以等价替换;测试行字段数须按 SECTION_LAYOUTS 期望补齐(Structures 17/Units 14/Infantry 同表),以免误标 `malformed` 干扰断言语义)

```rust
#[test]
fn render_facing_infantry_field7_real_rows() {
    // austintx COW 行:sub=4/facing=32 与 sub=2/facing=64(研究 §5 实证)
    let a = one(
        "Infantry",
        "0",
        "Neutral,COW,256,128,42,4,Guard,32,None,0,-1,0,1,1",
    );
    assert_eq!(a.render_facing(), Some(32));
    let b = one(
        "Infantry",
        "1",
        "Neutral,COW,256,126,52,2,Guard,64,None,0,-1,0,1,1",
    );
    assert_eq!(b.render_facing(), Some(64));
    assert_eq!(b.facing(), None); // M2 冻结语义:Infantry field 5=sub_cell
}

#[test]
fn render_facing_other_classes_and_hostile() {
    let s = one(
        "Structures",
        "0",
        "Neutral,CABHUT,256,103,64,192,None,0,0,1,0,0,None,None,None,0,0",
    );
    assert_eq!(s.render_facing(), Some(192));
    let u = one("Units", "0", "Neutral,CAR,256,66,104,64,Sleep,None,0,-1,0,-1,1,1");
    assert_eq!(u.render_facing(), Some(64));
    // 敌意:field 7 非整数 / >255 → None(不 panic)
    let bad = one(
        "Infantry",
        "0",
        "Neutral,COW,256,1,2,3,Guard,zzz,None,0,-1,0,1,1",
    );
    assert_eq!(bad.render_facing(), None);
    let over = one(
        "Infantry",
        "0",
        "Neutral,COW,256,1,2,3,Guard,300,None,0,-1,0,1,1",
    );
    assert_eq!(over.render_facing(), None);
    let t = one("Terrain", "1002", "TIBTRE01");
    assert_eq!(t.render_facing(), None);
}
```

- [ ] **Step 3: 跑红 → 实现**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p ra2-core objects`
Expected: 编译失败(`render_facing` 未定义)。

`impl MapObject` 内(facing() 之后)加:

```rust
/// 渲染朝向(M4b):Structure/Unit/Aircraft = field 5;Infantry 的
/// field 5 是 sub_cell,朝向在 field 7(真图实证 2026-11-06,研究 §5);
/// Terrain/Smudge 无。非整数或 >255 → None(宽容)。
pub fn render_facing(&self) -> Option<u32> {
    let idx = match self.class {
        ObjectClass::Infantry => 7,
        ObjectClass::Structure | ObjectClass::Unit | ObjectClass::Aircraft => 5,
        ObjectClass::Terrain | ObjectClass::Smudge => return None,
    };
    self.field(idx)?.trim().parse().ok().filter(|&f| f <= 255)
}
```

- [ ] **Step 4: 跑绿 + 门禁**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p ra2-core objects && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 新 2 测试 pass,既有 9 测试不动;全绿。

- [ ] **Step 5: 提交**

```bash
git add crates/core/src/format/map/objects.rs
git commit -m "feat(map): MapObject::render_facing——Infantry 朝向在 field 7(真图实证),其余编号键段 field 5(M4b T3)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 4: SHP 帧落画布 + overlay pass

**Files:**
- Create: `crates/core/src/render/shape.rs`(blit_frame/load_shp/CachedShp)
- Create: `crates/core/src/render/overlay.rs`(paint_overlays + BRIDGE_VARIATION)
- Modify: `crates/core/src/render/mod.rs`(`pub mod shape; pub mod overlay;` + SceneStats + ShpCache)

**Interfaces:**
- Consumes: `ShpTsFile/ShpTsFrame/ShpTsFrameHeader`(字段全 pub);`TheaterAssets::lookup(&str)->Option<&[u8]>`;`OverlayTable/OverlayKind`(T1);`MapFile.overlays.type_at/frame_at`;`IsoTile`;`CELL_DX/CELL_DY/Z_STEP`。
- Produces(T5/T6 依赖,签名冻结):

```rust
// render/mod.rs
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SceneStats {
    pub terrain: RenderStats,
    pub overlay_drawn: usize,
    pub overlay_no_type: usize,      // 0xFF 之外的越表位/表缺
    pub overlay_missing_file: usize,
    pub overlay_fail: usize,         // SHP 解析败(跳过)
    pub objects_drawn: usize,
    pub objects_skipped_voxel: usize,
    pub objects_missing_art: usize,
    pub objects_malformed: usize,    // 损坏行/坐标·类型解析失败
    pub objects_fail: usize,         // SHP 解析败(跳过)
    pub frame_clamp: usize,          // 帧号越界钳末帧(overlay+物件,信息位)
    pub clip: usize,                 // 精灵矩形未全入画布(信息位)
}
pub(crate) type ShpCache = std::collections::HashMap<String, shape::CachedShp>;

// render/shape.rs
pub(crate) enum CachedShp { Missing, Fail, File(crate::format::shp_ts::ShpTsFile) }
pub(crate) enum BlitResult { Inside, Clipped, Outside }
pub(crate) fn blit_frame(rgb: &mut [u8], cw: usize, ch: usize,
    file: &crate::format::shp_ts::ShpTsFile, frame: &crate::format::shp_ts::ShpTsFrame,
    center: (i64, i64), palette: &crate::format::pal::Palette) -> BlitResult;
pub(crate) fn load_shp<'a>(assets: &TheaterAssets, cache: &'a mut ShpCache,
    image: &str, ext: &str) -> &'a CachedShp;   // 候选 "{image}.shp" → "{image}.{ext}"

// render/overlay.rs
pub(crate) fn paint_overlays(map: &MapFile, assets: &TheaterAssets,
    table: &OverlayTable, palette: &Palette, records: &[IsoTile],
    d_min: i64, xmy_min: i64, z_max: usize, ext: &str,
    rgb: &mut [u8], cw: usize, ch: usize, stats: &mut SceneStats, cache: &mut ShpCache);
```

- 锚定(研究 §3,画布绝对坐标):TL = (30·(xmy−xmy_min), 15·z_max + 15·(d−d_min) − 15·z),C = TL+(30,15);overlay 帧 SHAPE_CENTER 于 C(overtype.cpp:167 `+(30,15)` 自洽证);**画布居中规则**:帧左上 = C − (file.cx/2, file.cy/2) + (frame.x, frame.y)。
- 帧选择(研究 §2):frame = frame_at(cell)(crate 特判 0;cell.cpp:1382);桥且 frame∈{0,9} → += BRIDGE_VARIATION[(x&3)|(4·(y&3))](cell.cpp:1422-1427 常量 `{0,1,2,3,3,2,1,0,2,3,0,1,1,0,3,2}`);ore 先 display_byte(cell.cpp:1451);帧 ≥ frames.len() → 钳末帧 + frame_clamp。
- 守恒:记录域内非空 overlay = drawn + no_type + missing_file + fail(测试钉)。

- [ ] **Step 1: 写失败测试**

shape.rs tests(合成 ShpTsFile 结构体直造,免字节编码):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::pal::Palette;
    use crate::format::shp_ts::{ShpTsFile, ShpTsFrame, ShpTsFrameHeader};

    fn frame(x: i16, y: i16, cx: u16, cy: u16, pixels: &[u8]) -> ShpTsFrame {
        ShpTsFrame {
            header: ShpTsFrameHeader { x, y, cx, cy, flags: 3, rgba: [0; 4], reserved: 0, data_offset: 28 },
            pixels: pixels.to_vec(),
        }
    }

    fn file(cx: u16, cy: u16, frames: Vec<ShpTsFrame>) -> ShpTsFile {
        ShpTsFile { zero: 0, cx, cy, frames }
    }

    fn pal() -> Palette {
        let mut rgb = [[0u8; 3]; 256];
        rgb[1] = [10, 20, 30];
        rgb[2] = [40, 50, 60];
        Palette { rgb }
    }

    /// 画布居中:4×4 画布 4×4 单帧 → 恰覆画布;透明 0 跳过。
    #[test]
    fn centered_full_frame_and_transparency() {
        let f = file(4, 4, vec![frame(0, 0, 4, 4, &[1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])]);
        let mut rgb = vec![0u8; 3 * 4 * 4];
        let r = blit_frame(&mut rgb, 4, 4, &f, &f.frames[0], (2, 2), &pal());
        assert!(matches!(r, BlitResult::Inside));
        let px = |i: usize| (rgb[3 * i], rgb[3 * i + 1], rgb[3 * i + 2]);
        assert_eq!(px(0), (10, 20, 30));  // (0,0) idx1
        assert_eq!(px(1), (0, 0, 0));     // (1,0) 透明
        assert_eq!(px(2), (40, 50, 60));  // (2,0) idx2
    }

    /// 子矩形偏移:画布 8×8,帧 2×2 @ (3,5) → 帧中心 = 点 + 偏移 − 画布半幅。
    #[test]
    fn subrect_offset_from_canvas_center() {
        let f = file(8, 8, vec![frame(3, 5, 2, 2, &[1, 1, 1, 1])]);
        let mut rgb = vec![0u8; 3 * 8 * 8];
        blit_frame(&mut rgb, 8, 8, &f, &f.frames[0], (4, 4), &pal());
        // 帧左上 = (4,4) − (4,4) + (3,5) = (3,5)
        let at = |x: usize, y: usize| rgb[3 * (y * 8 + x)];
        assert_eq!(at(3, 5), 10);
        assert_eq!(at(4, 6), 10);
        assert_eq!(at(2, 4), 0); // 帧外
        assert_eq!(at(5, 7), 0);
    }

    /// 出界三态:完全出界 Outside;部分越界 Clipped(可见部分仍写)。
    #[test]
    fn clip_and_outside() {
        let f = file(4, 2, vec![frame(0, 0, 4, 2, &[1; 8])]);
        let mut rgb = vec![0u8; 3 * 8 * 4];
        // center (2,1):帧左上 (0,0) Inside
        assert!(matches!(blit_frame(&mut rgb, 8, 4, &f, &f.frames[0], (2, 1), &pal()), BlitResult::Inside));
        // center (7,1):帧列 5..9 → 右溢 1 列 Clipped((6,1) 会恰收 Inside)
        assert!(matches!(blit_frame(&mut rgb, 8, 4, &f, &f.frames[0], (7, 1), &pal()), BlitResult::Clipped));
        assert_eq!(rgb[3 * (1 * 8 + 7)], 10); // 可见列仍写
        // center (20,1):完全出界
        assert!(matches!(blit_frame(&mut rgb, 8, 4, &f, &f.frames[0], (20, 1), &pal()), BlitResult::Outside));
    }
}
```

overlay.rs tests(夹具:mini 地图 + 合成 mix——**先读** `render/terrain.rs:407-500` 的 make_mix/tmp60/tiny_assets/tiny_ini 夹具与 map/mod.rs tests 的 mini_map/lcw/lzo 编码器,复制手法):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::map::MapFile;
    use crate::format::pal::Palette;
    use crate::format::rules::OverlayTable;
    use crate::format::shp_ts::{ShpTsFile, ShpTsFrame, ShpTsFrameHeader};
    use crate::format::theater::{theater_spec, TheaterAssets, TheaterIni};
    use crate::render::terrain::walk_extremes;

    // —— 合成 60×30 单帧 SHP(裸 RAW 判别:span==cx·cy)—
    fn shp60(fill: u8) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&0u16.to_le_bytes());          // zero
        v.extend_from_slice(&60u16.to_le_bytes());         // cx
        v.extend_from_slice(&30u16.to_le_bytes());         // cy
        v.extend_from_slice(&1u16.to_le_bytes());          // count
        // 24B 帧头:x,y,cx,cy,flags,rgba,reserved,data_offset
        v.extend_from_slice(&[0, 0, 0, 0, 60, 0, 30, 0]);
        v.extend_from_slice(&3u32.to_le_bytes());
        v.extend_from_slice(&[0u8; 4]);
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&32u32.to_le_bytes());         // 8+24 = 表尾
        v.extend_from_slice(&vec![fill; 60 * 30]);         // RAW
        v
    }

    // —— 60×30 全 fill 帧:帧中心两像素 0(透明)供穿透断言 —
    // (frame 方案同 shp60,构造 pixels 时 [30*15+29]=0 与 [30*15+30]=0)

    /// 端到端:2×2 地图,格 (1,1) 有 overlay(表位 0,帧 3),SHP 恰覆格菱形域。
    #[test]
    fn overlay_paints_cell_and_conservation() {
        // (a) 迷你地图:复用 map/mod.rs tests 手法(lcw_fill_stream 等),
        //     [OverlayPack] 解码后仅格 (1,1) = 表位 0、[OverlayDataPack] = 3
        //     (构造:262144 全 0xFF 的 buffer,grid[(1<<9)|1]=0x00,data 同法=3)
        // (b) rules 文本:"[OverlayTypes]\n0=ORE01\n[ORE01]\nTiberium=yes\n"
        //     (display_byte(0,1,1)=0+0 → 同型)
        // (c) assets:ra2md.mix 含 "ORE01.shp"=shp60(7)
        // (d) 断言:
        //     - stats:overlay_drawn==1,no_type/missing_file/fail==0
        //     - 画布:格 (1,1) C = TL+(30,15) 处 60×30 帧居中——取帧中心
        //       像素 (C.0, C.1) 色 == palette.rgb8(7);帧中心对称两点为
        //       透明 → 保留地形色(未覆写)
        //     - 守恒:Σ记录域内非空 overlay == drawn+no_type+missing_file+fail
    }

    /// 帧号越界钳末帧 + frame_clamp;crate 帧 0;桥变体表 16 项常量。
    #[test]
    fn crate_bridge_clamp_counters() {
        // (a) 表位 1 type Crate=yes,OverlayData=5 → 实际帧 0,断言色=帧 0 色
        // (b) 表位 2 名 "BRIDGE1",frame=0,(x&3)|(4·(y&3))=1 → 帧变体 +1
        //     (SHP 两帧:帧 0 色 A 帧 1 色 B,断言画到 B)
        // (c) SHP 单帧但 frame=9 → 钳帧 0,frame_clamp==1
    }
}
```

(上两测试按 (a)-(d) 展开为可跑代码。夹具事实已实钉:overlay 包构造 = map/mod.rs tests 的 `push_pack_section(&mut ini, "OverlayPack", &lcw_fill_stream(0xFF, 262144))` 同型——先整层 fill 再按格改写 `buf[(y << 9) | x]`(行主序索引,overlay.rs:52;其 tests `layers()` 夹具同式,格 (3,2) 即 `3 + (2 << 9)`);剧场/TMP 夹具复制 `render/terrain.rs:407-500`;不新发明格式。)

- [ ] **Step 2: 跑红**(空壳三文件 + mod 声明 + SceneStats 先行,函数未实现)

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p ra2-core render::`
Expected: 编译失败(blit_frame/paint_overlays 未定义)。

- [ ] **Step 3: 实现**

`render/mod.rs` 追加(常量区之后):

```rust
pub mod overlay;
pub mod shape;
```

(按字母序插入既有 `pub mod terrain;` 前;等 T5/T6 再加 objects/scene。)`RenderStats` 定义后追加 `SceneStats`(接口块原文)与:

```rust
/// SHP 名 → 解析结果缓存(每图一份;Missing/Fail 负缓存防重复查档)。
pub(crate) type ShpCache = std::collections::HashMap<String, shape::CachedShp>;
```

`render/shape.rs`:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! SHP-TS 帧落画布(M4b;研究 §3/§6):索引像素、**值 0 = 透明跳过**、
//! 画布居中锚定(SHAPE_CENTER 语义:文件画布中心 = 锚点,帧按帧头
//! (x,y) 子偏移落位;60×30 overlay 帧恰覆格为自洽证,overtype.cpp:167)。
//! 越界三态:Inside/Clipped(可见部分仍写)/Outside(完全出界不写)。

use crate::format::pal::Palette;
use crate::format::shp_ts::{ShpTsFile, ShpTsFrame};
use crate::format::theater::TheaterAssets;

/// 缓存值:缺档/解析败(负缓存)/文件。
pub(crate) enum CachedShp {
    Missing,
    Fail,
    File(ShpTsFile),
}

pub(crate) enum BlitResult {
    Inside,
    Clipped,
    Outside,
}

/// 帧落画布。`center` = 画布绝对坐标锚点。
pub(crate) fn blit_frame(
    rgb: &mut [u8],
    cw: usize,
    ch: usize,
    file: &ShpTsFile,
    frame: &ShpTsFrame,
    center: (i64, i64),
    palette: &Palette,
) -> BlitResult {
    let (fw, fh) = (i64::from(frame.header.cx), i64::from(frame.header.cy));
    if fw == 0 || fh == 0 {
        return BlitResult::Outside; // 空帧约定(shp_ts.rs):无像素
    }
    let dx = center.0 - i64::from(file.cx) / 2 + i64::from(frame.header.x);
    let dy = center.1 - i64::from(file.cy) / 2 + i64::from(frame.header.y);
    let (cw64, ch64) = (cw as i64, ch as i64);
    if dx >= cw64 || dy >= ch64 || dx + fw <= 0 || dy + fh <= 0 {
        return BlitResult::Outside;
    }
    let result = if dx < 0 || dy < 0 || dx + fw > cw64 || dy + fh > ch64 {
        BlitResult::Clipped
    } else {
        BlitResult::Inside
    };
    for row in 0..fh as usize {
        let py = dy + row as i64;
        if py < 0 || py >= ch64 {
            continue;
        }
        let src = &frame.pixels[row * fw as usize..(row + 1) * fw as usize];
        for (col, &idx) in src.iter().enumerate() {
            if idx == 0 {
                continue; // 透明(研究 §6)
            }
            let px = dx + col as i64;
            if px < 0 || px >= cw64 {
                continue;
            }
            let i = 3 * (py as usize * cw + px as usize);
            rgb[i..i + 3].copy_from_slice(&palette.rgb[idx as usize]);
        }
    }
    result
}

/// 名 → 候选 "{image}.shp" → "{image}.{ext}" 首个命中的解析结果(带缓存;
/// 剧场后缀候选源自 overtype.cpp:179/186 的变体装载序)。
pub(crate) fn load_shp<'a>(
    assets: &TheaterAssets,
    cache: &'a mut super::ShpCache,
    image: &str,
    ext: &str,
) -> &'a CachedShp {
    let key = image.to_ascii_lowercase();
    if !cache.contains_key(&key) {
        let hit = [format!("{image}.shp"), format!("{image}.{ext}")]
            .iter()
            .find_map(|n| assets.lookup(n).map(|b| b.to_vec()));
        let v = match hit {
            None => CachedShp::Missing,
            Some(b) => match ShpTsFile::parse(&b) {
                Ok(f) => CachedShp::File(f),
                Err(_) => CachedShp::Fail,
            },
        };
        cache.insert(key.clone(), v);
    }
    &cache[&key]
}
```

`render/overlay.rs`:

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! overlay pass(M4b spec §5;研究 §2/§3):records 菱形序(d 升序,scene
//! 已排)逐格查双层网格——字节 = [OverlayTypes] 表位(T1),ore 先游程
//! 变体(cell.cpp:1451),crate 帧特判 0(cell.cpp:1382),桥面板帧 0/9
//! 加 16 项变体表(cell.cpp:1422-1427),帧号 = OverlayData 直传;锚 =
//! 格中心 C(overtype.cpp:167)。桥 NS 偏移 (−15,7) 与桥高 z 附加
//! (cell.cpp:1408/1372-1376)不感知 → M4c(spec §8 低置信清单)。

use crate::format::map::iso::IsoTile;
use crate::format::map::MapFile;
use crate::format::pal::Palette;
use crate::format::rules::OverlayTable;
use crate::format::theater::TheaterAssets;
use crate::render::shape::{blit_frame, load_shp, BlitResult, CachedShp};
use crate::render::{SceneStats, ShpCache, CELL_DX, CELL_DY, Z_STEP};

/// 桥面板完整帧变体(cell.cpp:1422-1427,索引 (x&3)|(4·(y&3)))。
const BRIDGE_VARIATION: [usize; 16] = [0, 1, 2, 3, 3, 2, 1, 0, 2, 3, 0, 1, 1, 0, 3, 2];

pub(crate) fn paint_overlays(
    map: &MapFile,
    assets: &TheaterAssets,
    table: &OverlayTable,
    palette: &Palette,
    records: &[IsoTile],
    d_min: i64,
    xmy_min: i64,
    z_max: usize,
    ext: &str,
    rgb: &mut [u8],
    cw: usize,
    ch: usize,
    stats: &mut SceneStats,
    cache: &mut ShpCache,
) {
    for t in records {
        let ty = map.overlays.type_at(t.x, t.y);
        if ty == crate::format::rules::OVERLAY_NONE {
            continue;
        }
        let Some(kind) = table.kind(ty) else {
            stats.overlay_no_type += 1;
            continue;
        };
        let disp = table.display_byte(ty, t.x, t.y);
        let Some(kind) = table.kind(disp) else {
            stats.overlay_no_type += 1;
            continue;
        };
        let mut frame = if kind.is_crate {
            0
        } else {
            usize::from(map.overlays.frame_at(t.x, t.y).unwrap_or(0))
        };
        if kind.bridge && (frame == 0 || frame == 9) {
            let i = ((t.x & 3) | (4 * (t.y & 3))) as usize;
            frame += BRIDGE_VARIATION[i];
        }
        let loaded = load_shp(assets, cache, kind.image, ext);
        let CachedShp::File(shp) = loaded else {
            if matches!(loaded, CachedShp::Missing) {
                stats.overlay_missing_file += 1;
            } else {
                stats.overlay_fail += 1;
            }
            continue;
        };
        if frame >= shp.frames.len() {
            frame = shp.frames.len() - 1;
            stats.frame_clamp += 1;
        }
        let d = i64::from(t.x) + i64::from(t.y);
        let xmy = i64::from(t.x) - i64::from(t.y);
        let center = (
            CELL_DX as i64 * (xmy - xmy_min) + CELL_DX as i64,
            Z_STEP as i64 * z_max as i64 + CELL_DY as i64 * (d - d_min)
                - Z_STEP as i64 * i64::from(t.z) + CELL_DY as i64,
        );
        match blit_frame(rgb, cw, ch, shp, &shp.frames[frame], center, palette) {
            BlitResult::Outside => {
                stats.clip += 1;
                stats.overlay_drawn += 1; // 链路成功,画布外
            }
            BlitResult::Clipped => {
                stats.clip += 1;
                stats.overlay_drawn += 1;
            }
            BlitResult::Inside => stats.overlay_drawn += 1,
        }
    }
}
```

- [ ] **Step 4: 跑绿 + 门禁**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p ra2-core render:: && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: shape 3 + overlay 2 新测试 pass;全绿。

- [ ] **Step 5: 提交**

```bash
git add crates/core/src/render/shape.rs crates/core/src/render/overlay.rs crates/core/src/render/mod.rs
git commit -m "feat(render): SHP 帧居中落画布(透明/裁剪三态/双候选名缓存)+ overlay pass(表位/ore 游程/crate 帧 0/桥变体表/帧号直传)+ SceneStats(M4b T4)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 5: 物件 pass(Sort_Y 序/分类锚定/朝向帧)

**Files:**
- Create: `crates/core/src/render/objects.rs`
- Modify: `crates/core/src/render/mod.rs`(加 `pub mod objects;`)

**Interfaces:**
- Consumes: T2 `ArtIndex/ArtEntry`;T3 `render_facing`;M2 `MapObject/ObjectClass/{cell,kind,sub_cell,malformed}`;T4 `blit_frame/load_shp/CachedShp/BlitResult`;`map.tiles.cell(x,y)->Option<&IsoTile>`。
- Produces(T6 依赖):

```rust
pub(crate) fn paint_objects(map: &MapFile, assets: &TheaterAssets,
    art: &crate::format::art::ArtIndex, palette: &Palette,
    d_min: i64, xmy_min: i64, z_max: usize, ext: &str,
    rgb: &mut [u8], cw: usize, ch: usize, stats: &mut SceneStats, cache: &mut ShpCache);
```

- 语义(研究 §1/§3/§4):
  - 分类处置:Unit/Aircraft → `objects_skipped_voxel`(voxel 资产,M4c);Smudge → 不计(守恒排除,spec §6);Terrain/Structure/Infantry 渲染。
  - 可用性:`o.malformed || o.cell().is_none() || o.kind().is_none()` → `objects_malformed`。
  - 排序键(Sort_Y,object.cpp:1482-1485):Structure = 256·(X+Y)(Render_Coord=足迹 NW 角,building.cpp:7485-7488);Terrain/Infantry = 256·(X+Y)+256(格中心);**稳定排序**(Sorted_Add 等插入序,map 文件序即提交序)。
  - 锚定:0 基 (X,Y) ↔ 记录 (X+1,Y+1);C = TL+(30,15);树 = (C.0, C.1−12)(terrain.cpp:281);步兵 = C + 子格偏移 − (0,10)(infantry.cpp:495;子格表 L3 待钉候选);建筑 = (C.0, C.1−15)(NW 角 = TL 顶点;Editor 路径佐证,L1)。
  - 帧:树/建筑 = 0;步兵 = stand_frame + facing_idx,facing_idx = min(F−1, (facing·F+128)>>8)(HumanShape[dir32] 四舍五入等价,研究 §4);F = art facings(缺省 8)。
  - z = tiles.cell(x,y) 的 z(缺记录 → 0)。
  - SHP 解析败 → `objects_fail`;缺档(双候选皆无)→ `objects_missing_art`;钳帧计 `frame_clamp`;守恒:非 Smudge 物件 = drawn+skipped_voxel+missing_art+malformed+fail。
  - 子格偏移表:`const SUBCELL_OFFSETS: [(i64,i64); 6] = [(0,0),(-15,-8),(15,-8),(-15,7),(15,7),(0,0)];`(0..=5,5=兜底中心;L3)。

- [ ] **Step 1: 写失败测试**(objects.rs tests;夹具手法同 T4——合成地图+mix+SHP+art 文本)

```rust
#[cfg(test)]
mod tests {
    // 夹具(复制 T4 的 shp60/make_mix/tiny_assets 手法 + map/mod.rs 编码器)

    /// 排序:同列树 (X,Y+1) 与建筑 (X,Y)(同 d 邻格)——建筑键 256·d、树
    /// 键 256·d+256(半格差):构造重叠(z 差诱发争用带,同 M4a T2 手法),
    /// 断言近者(树)后画覆写;再交换 z 使建筑近 → 建筑覆写。
    #[test]
    fn sort_y_orders_building_vs_tree() { /* 构造+双色断言 */ }

    /// 锚定:格中心 C;树 = C−12;步兵 = C−10;建筑 = C−15。
    /// 单物件单帧 1×1 SHP,断言像素落点逐类钉死。
    #[test]
    fn anchors_tree_infantry_building() { /* 四断言 */ }

    /// 朝向帧:Facings=8,facing=64 → idx=(64·8+128)>>8=2;StandFrame=3
    /// → 帧 5;SHP 8+ 帧逐帧异色,断言命中帧 5 色。facing=255 → idx=7(钳)。
    #[test]
    fn infantry_facing_frame_formula() { /* … */ }

    /// 子格:sub_cell=1 → C+(−15,−8)−(0,10)。
    #[test]
    fn subcell_offset_applied() { /* … */ }

    /// 分类桶与守恒:Unit/Aircraft → skipped_voxel;Smudge 不计;
    /// 损坏行(16 字段 Structures)→ malformed;缺 SHP → missing_art;
    /// 坏 SHP 字节 → fail;守恒恒等式断言。
    #[test]
    fn buckets_and_conservation() { /* … */ }
}
```

(各测试体按注释展开为可跑代码;所有断言值先手推再写死。)

- [ ] **Step 2: 跑红**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p ra2-core render::objects`
Expected: 编译失败(paint_objects 未定义)。

- [ ] **Step 3: 实现**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! 物件 pass(M4b spec §5;研究 §1/§3/§4):三类 SHP 物件(树/建筑/
//! 步兵)按引擎 Sort_Y 升序稳定绘制;Unit/Aircraft = voxel 计数跳过
//! (用户裁决 2026-11-06,M4c);Smudge 顺延不计。锚定/朝向公式出处见
//! 研究文档;建筑 in-game Draw_IT 未反编译 → Editor 路径锚定(L1)。

use crate::format::art::ArtIndex;
use crate::format::map::objects::ObjectClass;
use crate::format::map::MapFile;
use crate::format::pal::Palette;
use crate::format::theater::TheaterAssets;
use crate::render::shape::{blit_frame, load_shp, BlitResult, CachedShp};
use crate::render::{SceneStats, ShpCache, CELL_DX, CELL_DY, Z_STEP};

/// 步兵子格像素偏移(0..=5;5 = 兜底中心;L3 待钉,研究 §9)。
const SUBCELL_OFFSETS: [(i64, i64); 6] = [(0, 0), (-15, -8), (15, -8), (-15, 7), (15, 7), (0, 0)];

/// HumanShape[dir32] 四舍五入等价(research §4):dir256·F/256 就近取整,
/// 边界钳 F−1。
fn facing_idx(facing: u32, facings: u32) -> u32 {
    let f = u64::from(facings.max(1));
    (((u64::from(facing) * f + 128) >> 8) as u32).min(facings.max(1) - 1)
}

pub(crate) fn paint_objects(
    map: &MapFile,
    assets: &TheaterAssets,
    art: &ArtIndex,
    palette: &Palette,
    d_min: i64,
    xmy_min: i64,
    z_max: usize,
    ext: &str,
    rgb: &mut [u8],
    cw: usize,
    ch: usize,
    stats: &mut SceneStats,
    cache: &mut ShpCache,
) {
    // (sort_key, 对象)——稳定排序保持 map 文件序(Sorted_Add 等插入序)
    let mut drawable: Vec<(i64, &crate::format::map::objects::MapObject)> = Vec::new();
    for o in &map.objects.all {
        match o.class {
            ObjectClass::Unit | ObjectClass::Aircraft => {
                stats.objects_skipped_voxel += 1;
            }
            ObjectClass::Smudge => {} // M4c;不入桶不入守恒
            ObjectClass::Terrain | ObjectClass::Structure | ObjectClass::Infantry => {
                if o.malformed || o.cell().is_none() || o.kind().is_none() {
                    stats.objects_malformed += 1;
                } else {
                    let (x, y) = o.cell().unwrap();
                    let key = i64::from(x) + i64::from(y);
                    // Sort_Y:建筑 = NW 角(building.cpp:7485),其余 = 格中心
                    let key = if matches!(o.class, ObjectClass::Structure) {
                        256 * key
                    } else {
                        256 * key + 256
                    };
                    drawable.push((key, o));
                }
            }
        }
    }
    drawable.sort_by_key(|(k, _)| *k);
    for (_, o) in drawable {
        let (x, y) = o.cell().unwrap();
        let Some(kind) = o.kind() else { continue };
        // 0 基 → 记录坐标;敌意大值钳到域外(u16::MAX 远超走位域 → 自然
        // Outside 计 clip,绝不截断落错位置)
        let (wx, wy) = (
            u16::try_from(x + 1).unwrap_or(u16::MAX),
            u16::try_from(y + 1).unwrap_or(u16::MAX),
        );
        let z = map.tiles.cell(wx, wy).map_or(0u8, |t| t.z);
        let d = i64::from(wx) + i64::from(wy);
        let xmy = i64::from(wx) - i64::from(wy);
        let cx = CELL_DX as i64 * (xmy - xmy_min) + CELL_DX as i64;
        let cy = Z_STEP as i64 * z_max as i64 + CELL_DY as i64 * (d - d_min)
            - Z_STEP as i64 * i64::from(z) + CELL_DY as i64;
        // 锚定/帧选择(研究 §3/§4);get_or_default 返回 owned ArtEntry,
        // image 字段直接移出
        let (image, center, frame) = match o.class {
            ObjectClass::Terrain => {
                let e = art.get_or_default(kind, 1);
                (e.image, (cx, cy - 12), 0usize) // terrain.cpp:281 zadjust −12
            }
            ObjectClass::Infantry => {
                let e = art.get_or_default(kind, 8);
                let f = o.render_facing().unwrap_or(0);
                let idx = facing_idx(f, e.facings) as usize;
                let (ox, oy) = SUBCELL_OFFSETS[(o.sub_cell().unwrap_or(0) as usize).min(5)];
                (e.image, (cx + ox, cy + oy - 10), e.stand_frame as usize + idx) // infantry.cpp:495
            }
            _ => {
                let e = art.get_or_default(kind, 1);
                (e.image, (cx, cy - 15), 0usize) // 建筑锚 = NW 角投影 = C+(0,−15)(L1)
            }
        };
        let loaded = load_shp(assets, cache, &image, ext);
        let CachedShp::File(shp) = loaded else {
            if matches!(loaded, CachedShp::Missing) {
                stats.objects_missing_art += 1;
            } else {
                stats.objects_fail += 1;
            }
            continue;
        };
        let mut frame = frame;
        if frame >= shp.frames.len() {
            frame = shp.frames.len() - 1;
            stats.frame_clamp += 1;
        }
        match blit_frame(rgb, cw, ch, shp, &shp.frames[frame], center, palette) {
            BlitResult::Inside => stats.objects_drawn += 1,
            BlitResult::Clipped | BlitResult::Outside => {
                stats.clip += 1;
                stats.objects_drawn += 1;
            }
        }
    }
}
```

(引用 `...::MapObject` 的 use 行按编译器提示并入既有 use 域;`map.objects.all` 字段名以 objects.rs 实际容器为准——若为别的名字或迭代器方法,以等价迭代替换,语义不变。建筑锚 C+(0,−15) 即 NW 角投影:−½(30,15)−½(−30,15) = (0,−15)。)

- [ ] **Step 4: 跑绿 + 门禁**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p ra2-core render:: && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 新 5 测试 pass;全绿。

- [ ] **Step 5: 提交**

```bash
git add crates/core/src/render/objects.rs crates/core/src/render/mod.rs
git commit -m "feat(render): 物件 pass——Sort_Y 升序稳定序(建筑 NW 角键)/树 −12·步兵 −10+子格·建筑顶点锚定/步兵朝向帧公式/voxel 计数跳过(M4b T5)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 6: 场景合成 `render_scene` + 层序集成测试

**Files:**
- Create: `crates/core/src/render/scene.rs`
- Modify: `crates/core/src/render/mod.rs`(加 `pub mod scene;`)

**Interfaces:**
- Consumes: `render_terrain` 内部链(`map.size_wh`/records d 排序/`compose_terrain`(pub(crate))/`walk_extremes`/`theater_spec`);T4/T5 两 pass;T1/T2 表。
- Produces(T7 CLI 与 T8 验收依赖,签名冻结):

```rust
pub struct SceneRender { pub width: u32, pub height: u32, pub rgb: Vec<u8>, pub stats: SceneStats }
pub fn render_scene(map: &MapFile, assets: &TheaterAssets, ini: &TheaterIni,
    table: &OverlayTable, art: &ArtIndex, palette: &Palette) -> Result<SceneRender, crate::format::FormatError>;
```

- 编排:① records d 升序 → `compose_terrain`(画布公式不动);② `walk_extremes` + records z_max 实扫(独立 H 钉,M4a 观察项);③ overlay pass;④ objects pass。

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    // 夹具链:T4/T5 测试已备的合成地图 + mix(瓦片 TMP/SHP/rules/art 文本)
    // + TheaterIni(tiny_ini 手法,terrain.rs tests)+ 调色板。

    /// 层序:同格 瓦片→overlay→物件——三色各异,断言最终像素 = 物件色;
    /// 物件缺失(空 [Terrain])时 = overlay 色;再剥离 overlay = 瓦片色。
    #[test]
    fn layer_order_tile_overlay_object() { /* 三态断言 */ }

    /// 守恒总账:terrain 七桶 + overlay 四桶 + 物件五桶齐验(夹具含
    /// Unit/Smudge/malformed 行,证明 voxel 与 smudge 的排除口径)。
    #[test]
    fn scene_stats_conservation() { /* … */ }

    /// 画布尺寸 = M4a 公式(scene 不改画布):断言与 walk_extremes
    /// 重算一致(同 m4a 验收口径,合成小图)。
    #[test]
    fn canvas_dims_follow_m4a_formula() { /* … */ }

    /// 敌意:rules/art 双空文本 → 全 no_type/missing_art,地形照出零 panic。
    #[test]
    fn empty_tables_degrade_to_terrain_only() { /* … */ }
}
```

- [ ] **Step 2: 跑红**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p ra2-core render::scene`
Expected: 编译失败(render_scene 未定义)。

- [ ] **Step 3: 实现**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! 场景合成(M4b spec §4/§5):地形(M4a compose_terrain,画布公式
//! 不动)→ overlay pass → 物件 pass,共用画布/统计/SHP 缓存。
//! z_max 由 records 实扫(独立于 H 反推,M4a 观察项);精灵越界裁剪
//! 计 clip(引擎对视口同语义)。

use crate::format::art::ArtIndex;
use crate::format::map::MapFile;
use crate::format::pal::Palette;
use crate::format::rules::OverlayTable;
use crate::format::theater::{theater_spec, TheaterAssets, TheaterIni};
use crate::format::FormatError;
use crate::render::objects::paint_objects;
use crate::render::overlay::paint_overlays;
use crate::render::terrain::{compose_terrain, walk_extremes};
use crate::render::{SceneStats, ShpCache};

/// 场景合成结果。
pub struct SceneRender {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
    pub stats: SceneStats,
}

pub fn render_scene(
    map: &MapFile,
    assets: &TheaterAssets,
    ini: &TheaterIni,
    table: &OverlayTable,
    art: &ArtIndex,
    palette: &Palette,
) -> Result<SceneRender, FormatError> {
    let (w, h) = map.size_wh();
    let mut records = map.tiles.records().to_vec();
    records.sort_by_key(|t| u32::from(t.x) + u32::from(t.y));
    let spec = theater_spec(map.map_meta.theater);
    let terrain = compose_terrain(w, h, &records, assets, ini, spec.ext, palette)?;
    let (d_min, _d_max, xmy_min, _xmy_max) = walk_extremes(w, h);
    let z_max = records.iter().map(|t| t.z).max().unwrap_or(0) as usize;
    let mut stats = SceneStats {
        terrain: terrain.stats,
        ..SceneStats::default()
    };
    let (cw, ch) = (terrain.width as usize, terrain.height as usize);
    let mut rgb = terrain.rgb;
    let mut cache = ShpCache::new();
    paint_overlays(
        map, assets, table, palette, &records, d_min, xmy_min, z_max, spec.ext,
        &mut rgb, cw, ch, &mut stats, &mut cache,
    );
    paint_objects(
        map, assets, art, palette, d_min, xmy_min, z_max, spec.ext,
        &mut rgb, cw, ch, &mut stats, &mut cache,
    );
    Ok(SceneRender {
        width: cw as u32,
        height: ch as u32,
        rgb,
        stats,
    })
}
```

`render/mod.rs` 加 `pub mod scene;`。

- [ ] **Step 4: 跑绿 + 门禁**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p ra2-core render:: && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 新 4 测试 pass;全绿。

- [ ] **Step 5: 提交**

```bash
git add crates/core/src/render/scene.rs crates/core/src/render/mod.rs
git commit -m "feat(render): render_scene 场景合成——地形复用+z_max 独立实扫+overlay/物件双 pass 编排+层序与守恒集成测试(M4b T6)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 7: CLI render 场景化

**Files:**
- Modify: `tools/ra2-map/src/main.rs`(run_render:137-219 + use 域 + usage() render 行)
- Test: `tools/ra2-map/tests/cli.rs`(沿用既有夹具手法)

**Interfaces:**
- Consumes: T6 `render_scene/SceneRender`;T1 `OverlayTable::parse`;T2 `ArtIndex::parse`;既有 `decode_map_text/theater_spec/TheaterAssets/TheaterIni/load_theater_palette/png::write_png_file`。
- Produces: `ra2-map render <map> -o <out.png> [--assets <dir>]` 输出**场景** PNG(地形+overlay+物件);stderr 三段统计。

- [ ] **Step 1: 写失败测试**(cli.rs 追加;夹具 = 既有 mini_map()/tmp 目录手法)

```rust
/// 场景渲染成功路径:mini_map 夹具 + 空 rules/art(全 no_type/missing_art
/// 退化)→ PNG 落盘、签名与 IHDR 尺寸 = M4a 公式;stderr 含 "overlay" 与
/// "物件" 统计段。#[ignore](需真资产?否——合成 mix 即可:本测试建临时
/// 资产目录(ra2.mix/ra2md.mix + 剧场表/pal/TMP,手法复制既有
/// render_needs_assets_exit_1 的 mini 夹具),不 ignore。)
#[test]
fn render_scene_outputs_png_and_stats() { /* … */ }

/// rulesmd.ini 缺席(ra2md.mix 无该名)→ 退化非失败:exit 0,stderr 提示
/// overlay 全 no_type。
#[test]
fn render_missing_rules_degrades() { /* … */ }
```

- [ ] **Step 2: 跑红**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p ra2-map`
Expected: 两新测试失败(实现仍走 render_terrain,stderr 无 overlay 段)。

- [ ] **Step 3: 实现**

use 域补:`use ra2_core::format::art::ArtIndex; use ra2_core::format::rules::OverlayTable; use ra2_core::render::scene::render_scene;`(按既有 use 风格)。

`run_render` 在 `let ini = …;`(:183-189)之后、`let t0` 之前插入:

```rust
    let table = match assets.lookup("rulesmd.ini") {
        Some(b) => OverlayTable::parse(&decode_map_text(b)),
        None => {
            eprintln!("资产缺 rulesmd.ini:overlay 层全计 no_type(退化)");
            OverlayTable::parse("")
        }
    };
    let art = ArtIndex::parse(
        &assets
            .lookup("artmd.ini")
            .map(|b| decode_map_text(b))
            .unwrap_or_default(),
        &assets
            .lookup("art.ini")
            .map(|b| decode_map_text(b))
            .unwrap_or_default(),
    );
```

(若 `decode_map_text` 返回 `String`,`&decode_map_text(b)` 直传;若 `Cow<str>` 加 `.into_owned()`——以现行签名为准,两处一致。)

`let r = match ra2_core::render::terrain::render_terrain(&m, &assets, &ini, &palette)`(:190-197)整段替换为:

```rust
    let r = match render_scene(&m, &assets, &ini, &table, &art, &palette) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("合成失败: {e}");
            return ExitCode::from(1);
        }
    };
```

统计段(:203-217)替换为:

```rust
    let s = &r.stats;
    let ov_total = s.overlay_drawn + s.overlay_no_type + s.overlay_missing_file + s.overlay_fail;
    let ob_total = s.objects_drawn
        + s.objects_skipped_voxel
        + s.objects_missing_art
        + s.objects_malformed
        + s.objects_fail;
    eprintln!(
        "已写 {out_path}: {}×{}px | 地形 {} = 落绘 {} + 无瓦片 {} + 缺文件 {} + 保留槽 {} + 越界 {} + 空槽 {} + 解析败 {} | overlay {ov_total} = 落绘 {} + 越表 {} + 缺档 {} + 失败 {} | 物件 {ob_total} = 落绘 {} + voxel跳过 {} + 缺素材 {} + 损坏 {} + 失败 {} | 钳帧 {} 裁剪 {} | 剧场 {:?} | 耗时 {dt:?}",
        r.width,
        r.height,
        s.terrain.cells,
        s.terrain.painted,
        s.terrain.no_tile,
        s.terrain.missing_file,
        s.terrain.reserved,
        s.terrain.out_of_range,
        s.terrain.missing_slot,
        s.terrain.parse_fail,
        s.overlay_drawn,
        s.overlay_no_type,
        s.overlay_missing_file,
        s.overlay_fail,
        s.objects_drawn,
        s.objects_skipped_voxel,
        s.objects_missing_art,
        s.objects_malformed,
        s.objects_fail,
        s.frame_clamp,
        s.clip,
        m.map_meta.theater,
    );
```

usage() 的 render 行文案改为:`# 场景合成 PNG(地形+overlay+物件;voxel 计数跳过)`(定位 usage() 内既有 render 行原样替换注释部分,参数面不变)。

- [ ] **Step 4: 跑绿 + 门禁(含既有 ignored CLI 测试复核)**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p ra2-map && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 新 2 + 既有 CLI 测试全绿(real_map_render_outputs_png 仍 ignored)。

- [ ] **Step 5: 提交**

```bash
git add tools/ra2-map/src/main.rs tools/ra2-map/tests/cli.rs
git commit -m "feat(cli): render 场景化——rules/art 装载(缺席退化)+render_scene+三段统计输出(M4b T7)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 8: 807 真资产验收 + Preview 终验裁定

**Files:**
- Create: `crates/core/tests/m4b_acceptance.rs`(全部 `#[ignore]`;**实现者先读 `crates/core/tests/m4a_acceptance.rs` 全文**,maps()/theater_palette()/probe_preview_diff() 三者原样复制)

**Interfaces:**
- Consumes: T6 `render_scene/SceneStats`;T1/T2;M4a 验收手法(递归枚举/哑调色板/预算/PPM 抽样)。
- Produces: 验收常量(预算/守恒钉值)+ Preview 通道/行序裁定(控制器消费裁定数据)。

- [ ] **Step 1: 写验收测试**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! M4b 真资产验收(spec §7):807 场景渲染零 panic + 守恒 + 预算 +
//! Preview 终验四组合探针(裁定:最优组合稳定占比 ≥85% 才钉,差率只报告)。
//! 全部 ignore,需 RA2_ASSETS_DIR。

// 头部:assets_dir()/maps()/theater_palette()/write_ppm()/probe_preview_diff()
// 五个助手从 m4a_acceptance.rs 原样复制(含注释);新增:

use ra2_core::format::art::ArtIndex;
use ra2_core::format::rules::OverlayTable;
use ra2_core::render::scene::render_scene;

fn load_tables(assets: &TheaterAssets) -> (OverlayTable, ArtIndex) {
    let table = assets
        .lookup("rulesmd.ini")
        .map(|b| OverlayTable::parse(&decode_map_text(b)))
        .unwrap_or_else(|| OverlayTable::parse(""));
    let art = ArtIndex::parse(
        &assets.lookup("artmd.ini").map(|b| decode_map_text(b)).unwrap_or_default(),
        &assets.lookup("art.ini").map(|b| decode_map_text(b)).unwrap_or_default(),
    );
    (table, art)
}

#[test]
#[ignore]
fn m4b_all_maps_scene_render() {
    // 主循环同 m4a(逐图 parse→theater ini→palette→render_scene 计时);
    // 断言与统计:
    // - assert_eq!(total, 807);零 panic(expect 同 m4a);
    // - 守恒(每图):terrain.cells == 七桶和;非空 overlay == drawn+no_type
    //   +missing_file+fail;非 Smudge 物件 == drawn+voxel+missing_art
    //   +malformed+fail(恒等式直接断言);
    // - 画布尺寸 == M4a 公式(walk_extremes 重算,同 m4a 口径);
    // - 预算:assert!(ms < 15000)(首跑;Step 3 控制器按实测 ×3 收紧);
    // - 逐剧场分档 eprintln(overlay/物件桶计);真彩可用剧场(TEMPERATE/
    //   SNOW/URBAN)各一图 PPM 抽样 + 哑调色板全 807 零 panic(同 m4a 口径);
    // - Preview 探针:真彩图调 probe_preview_diff(路径, SceneStats 口径
    //   换 SceneRender——签名同 m4a probe),逐图 eprintln;同时统计
    //   最优组合直方图 HashMap<&str, usize>,循环后 eprintln 占比。
}

#[test]
#[ignore]
fn m4b_overlay_table_retail_sanity() {
    // 零售规则表健全性:rulesmd.ini 解析 len() ≥ 100(零售 [OverlayTypes]
    // ~百级);至少一个 tiberium 游程;至少一个 bridge 前缀名;至少一个
    // Crate=yes。(桥前缀启发与 ore 游程等价性的零售证据,研究 §2 回填。)
}
```

- [ ] **Step 2: 跑真资产套件**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test m4b_acceptance -- --ignored --nocapture 2>&1 | tail -80`
Expected: 2 passed;stderr:总账行(807/耗时/最慢)+ 逐剧场行 + preview-diff 逐图 + **最优组合直方图与占比**;断言全过。**任何 panic/失败 = BLOCKED 附输出上报。**

- [ ] **Step 3: 控制器裁定(非子代理;结论记 `.superpowers/sdd/progress.md` + 上报用户)**

- **Preview 通道/行序**(spec §7.3):看最优组合直方图——稳定占比 ≥85%(真彩口径)→ 走向 A:`assert!(stable_frac >= 0.85)` + `assert_eq!(最众组合, "RGB")` 两断言入测试(常量按实测写死)重跑至绿;<85% → 记"未稳定",不设断言,直方图与差率分布写研究文档。
- **预算收紧**:max_ms×3(硬帽 15000)替换首跑占位预算。
- **桶健全性复核**:overlay_drawn 总量级(预期 807 图数万+;接近 0 = ore 游程/表位语义有误,回查 T1);objects_drawn 与 M2 对象计数同量级;clip 占比 <5%(异常 = 锚定/坐标 +1 映射疑点,回查 T5/L 列表)。
- **voxel 跳过量级**:skipped_voxel 总量记录(用户报告项,M4c 体量预估输入)。

- [ ] **Step 4: 裁定落地**(按 Step 3 补断言/预算,重跑 Step 2 至绿)

- [ ] **Step 5: 提交**

```bash
git add crates/core/tests/m4b_acceptance.rs
git commit -m "test(m4b): 807 场景渲染验收——守恒/预算/剧场分档/Preview 终验四组合直方图与通道序裁定落地(M4b T8)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 9: 文档收尾(README + 研究回填 + spec 勘误)

**Files:**
- Modify: `README.md`(render 行文案)
- Modify: `docs/superpowers/research/2026-11-06-m4b-overlay-objects.md`(§9 回填 + 勘误)
- Modify: `docs/superpowers/specs/2026-11-06-m4b-overlay-objects-render-design.md`(勘误附录)

**Interfaces:** 无代码;文档事实须与 T4-T8 实测一致。

- [ ] **Step 1: README** render 行替换为:

```
  ra2-map render <map.map> -o <out.png> [--assets <dir>]   # M4b 场景合成 PNG(地形+overlay+物件;voxel 载具/飞机计数跳过;本机真彩限 TEMPERATE/SNOW/URBAN)
```

- [ ] **Step 2: 研究文档回填**(§9 表逐项:已钉/仍低置信+实测依据;新增小节"§10 实测回填":807 总账、Preview 终验裁定结论、voxel 跳过量级、clip/钳帧占比;§2 桥前缀启发与 ore 游程等价性的零售证据(T8 m4b_overlay_table_retail_sanity 输出);§5 键序勘误已在 T0,若 T3 语料抽验有新发现补记)。

- [ ] **Step 3: spec 勘误附录**(文末加"## 10. 实施期勘误与细化"):① Terrain 键序笔误(T0 勘误);② SceneStats 细化:overlay_fail/objects_fail/frame_clamp 独立桶、Smudge 排除守恒口径;③ spec §4 `objects_typed.rs` 收敛——M2 `objects.rs` 类型化访问器已覆盖,仅补 `render_facing`(T3);④ spec §3 N2 "269²" 系笔误:方图上限 = 272² ≈ 133.4 Mpx(60w·(30w+15) ≤ 2²⁷ 解得 w ≤ 272.06,T0 算术);⑤ `render_scene` 返回 `Result`(非 §4 草案的 Option,计划期微调落定);⑥ 桥 NS 偏移与桥高 z 附加顺延 M4c;⑦ Preview 终验裁定结果(走向 A 钉断言或"未稳定"记录);⑧ 预算定值。

- [ ] **Step 4: 全量门禁(含真资产)**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored`
Expected: 合成全绿;ignored 39(37 既有 + m4b 2)。

- [ ] **Step 5: 提交**

```bash
git add README.md docs/superpowers/research/2026-11-06-m4b-overlay-objects.md docs/superpowers/specs/2026-11-06-m4b-overlay-objects-render-design.md
git commit -m "docs(m4b): README 场景行+研究 §9/§10 实测回填+spec 勘误附录(T0 键序/统计桶细化/桥偏移顺延/终验裁定/预算)(M4b T9)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

## 任务后:整分支终审 + 收尾

- 终审(superpowers:requesting-code-review,fable 模型):`scripts/review-package main HEAD` 全分支包;重点面:跨任务缝隙(层序/守恒/锚定一致性)、T1 表位语义、T5 排序键、敌意路径。
- 终审发现 → 单 fixer 波次 → 复审 → finishing-a-development-branch(用户裁决合并方式)。
