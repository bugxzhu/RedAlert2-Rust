# M4a 地形整图渲染 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 任一 .map + 资产目录 → 整幅等距地形 PNG(headless CPU 合成器 `core::render` + `ra2-map render` 子命令)。

**Architecture:** 纯合成器落 `crates/core/src/render/`(零 I/O);投影/z 公式已由 ra2-decomp 源码钉死(见 Global Constraints 证据链);CLI 只做 I/O 壳;层遍历画布结构为 M4b 预留。

**Tech Stack:** Rust 2021,workspace 内零新依赖;PNG 复用 `tools/ra2-map/src/png.rs`。

## Global Constraints

- **GPL-3.0-only**:每个新建/改动文件第 1 行 SPDX `// SPDX-License-Identifier: GPL-3.0-only`(现有文件已有,不重复动)。
- **零依赖 core**:`crates/core` 不加任何外部 crate;不引入 wgpu/图像库。
- **零 panic 契约**:807 图 + 敌意变异(超界 Size/缺档/敌意 TMP)恒 Err 或计数,不 panic;上界护栏先于分配。
- **退出码契约**:`ra2-map` 0 成功 / 1 失败 / 2 用法错(§7 CLI 契约不变)。
- **引擎证据链**(实现与注释引用,语义重写不抄码;两库只读):
  - 投影 `Rectangular_To_Isometric` tactical.cpp 0x006D1EB0:`iso.X = 30·(x−y), iso.Y = 15·(x+y)`(lepton/256 后);
  - 每级高度 `LEVEL_LEPTON_H = (int)(tan30° · 256√2 / 2) = 104`(geometry.h:25);cell Z = `104·z`(cell.cpp:150);`Z_Lepton_To_Pixel` 0x006D20E0 = `int(z·104·0.143526 + fudge(z≥728) + 0.5)` → **z_px = 15·z 全档 z∈0..14 精确**(含 728 fudge,z=7 得 105);
  - 格常数 ISO_TILE_PIXEL_W=60/H=30(geometry.h:11-12);
  - 绘制序:行对角线 `d=x+y` 升序(近平后画);同对角线互不重叠。
- **画布公式**(由上推出):`W = 30·(w+h)`,`H = 15·(w+h) + 15·z_max`,`ox = 30·(h−1)`,`oy = 15·z_max`,格像素原点 `sx = ox + 30·(x−y)`,`sy = oy + 15·(x+y) − 15·z`;护栏 `MAX_RENDER_PIXELS = 2²⁷ = 134,217,768`(语料最大 272+158=430 → 12,900×6,450+z ≈ 83.2 Mpx,余量 1.6×;4096² 敌意图 30 Gpx 拒)。
- **真资产套件**:fixtures 不入库;运行需 `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023"`;不进 CI。语料 = 资产树**递归** .map 共 807(Maps/ 540 + Resources/RandomMapGenerator_RA2/MapUnits/ 267 RMG 模板;与 M2/M3 口径一致,枚举手法复制 m3_acceptance.rs:20-31)。
- **本机调色板边界(2026-11-06 探针钉死)**:isodes.pal / isoubn.pal / isolun.pal 走完整 M3 跨容器链(全容器按名 + 调色板库 0x3B5A96DE 回退)均缺席;Resources/Skin 下的 *des.pal 等是侧边栏图标调色板,非 iso 瓦片调色板。**本机仅 TEMPERATE/SNOW/URBAN 三剧场可真彩渲染**(剧场分布:temperate 212 / snow 106 / urban 78 / desert 73 / newurban 61 / lunar 1)。spec §6.2"六剧场全覆盖"据此勘误:抽样 PNG = 可解析剧场全覆盖(≥3),palette 缺席剧场(desert/newurban/lunar 共 135 图)以**哑调色板**(768 零字节)跑链路零 panic + 统计,颜色不计;Preview 对拍仅对真彩剧场图有意义。
- **提交**:消息末尾 `Co-Authored-By: Claude Code <noreply@anthropic.com>`;分支 `feat/m4a-terrain-render`(自 main);`cargo` 不在 bash PATH,先 `export PATH="$HOME/.cargo/bin:$PATH"`。
- **注释/文档**:中文,出处用地址锚(`tactical.cpp 0x006D1EB0` 式),不点链接到外部文档。

---

### Task 1: `core::render` 脚手架 + `paint_canvas` 上移

**Files:**
- Create: `crates/core/src/render/mod.rs`
- Create: `crates/core/src/render/terrain.rs`
- Modify: `crates/core/src/lib.rs:5`(加 `pub mod render;`)
- Modify: `tools/ra2-map/src/main.rs`(import 改调 core,删本地 `paint_canvas` 与其 3 个测试)

**Interfaces:**
- Consumes: `Palette::rgb8`(pal.rs:36);M3 的 `paint_canvas` 逻辑(main.rs 现状)。
- Produces(Task 2/3 依赖,签名逐字):
  ```rust
  // render/mod.rs
  pub const MAX_RENDER_PIXELS: u64 = 134_217_728;
  pub const CELL_W: usize = 60;  pub const CELL_H: usize = 30;
  pub const CELL_DX: usize = 30; pub const CELL_DY: usize = 15;
  pub const Z_STEP: usize = 15;
  // render/terrain.rs
  pub fn row_width(r: usize, cy: usize) -> usize;
  pub fn paint_canvas(palette: &Palette, cx: usize, cy: usize, diamond: &[u8]) -> Option<Vec<u8>>;
  ```

- [ ] **Step 1: 建 `render/mod.rs`**(常数 + 结构体占位;RenderStats/TerrainRender 本任务只声明不使用,Task 2 消费)

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! M4a 地形整图渲染:无头合成器(spec docs/superpowers/specs/
//! 2026-11-06-m4a-terrain-render-design.md)。纯计算零 I/O;工具层
//! (`ra2-map render`)负责读资产/写 PNG。

pub mod terrain;

/// 敌意画布上界(spec §5):语料最大 272+158=430 格 → 12,900×6,450
/// ≈ 83.2 Mpx,护栏 2²⁷ 给 1.6× 余量;4096² 敌意图(约 30 Gpx)拒绝。
pub const MAX_RENDER_PIXELS: u64 = 134_217_728;

/// 等距格像素常数(geometry.h:11-12 ISO_TILE_PIXEL_W/H)。
pub const CELL_W: usize = 60;
pub const CELL_H: usize = 30;
/// 半格步长(Rectangular_To_Isometric tactical.cpp 0x006D1EB0:
/// 60/2 与 30/2,lepton/256 后)。
pub const CELL_DX: usize = 30;
pub const CELL_DY: usize = 15;
/// 每级高度像素(LEVEL_LEPTON_H=104 × Z_Lepton_To_Pixel 0x006D20E0,
/// z∈0..14 全档含 728 fudge 精确等于 15·z)。
pub const Z_STEP: usize = 15;

/// 整图渲染统计(spec §5:瓦片链任一环缺不中止,按格计数)。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RenderStats {
    /// 遍历格数(w×h 全量,含无记录格)。
    pub cells: usize,
    /// 无瓦片格(tile=-1)或无记录。
    pub no_tile: usize,
    /// 成功落像素。
    pub painted: usize,
    /// INI 占位槽(SlotFile::Reserved,画背景)。
    pub reserved: usize,
    /// 文件名界内但 mix 未出货。
    pub missing_file: usize,
    /// tile_id 越界 INI 槽位。
    pub out_of_range: usize,
    /// TMP 槽空(sub_tile 越界/空槽)。
    pub missing_slot: usize,
    /// TMP 解析失败或病态画布(非 2:1/阶梯不闭合)。
    pub parse_fail: usize,
}

/// 合成结果:行紧排 RGB(3·width·height)+ 统计。
pub struct TerrainRender {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
    pub stats: RenderStats,
}
```

- [ ] **Step 2: 建 `render/terrain.rs`**(`row_width` 提取 + `paint_canvas` 自工具上移,逻辑逐字不变;测试随迁)

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! 地形层合成(M4a 第一层;M4b 的 overlay/物件层在本画布结构上追加)。

use crate::format::pal::Palette;
use super::{CELL_DX, CELL_DY};

/// B 族阶梯行宽:行 r 宽 `4·min(r+1, cy−1−r)`,末行宽 0——引擎 .data
/// 两张 60×30 mask 表(0x3E2B5C/0x3EC4C8)行宽铁证。
pub fn row_width(r: usize, cy: usize) -> usize {
    4 * (r + 1).min(cy - 1 - r)
}

/// 菱形铺入 cx×cy 画布(单瓦片出图路径):行外区 = 调色板 0 号色;
/// 偶 cy 阶梯总耗 `4k² = cx·cy/2` 精确闭合,奇 cy 耗 `cx·cy/2−1` →
/// `None`(病态输入显式失败,不静默漏绘)。
pub fn paint_canvas(palette: &Palette, cx: usize, cy: usize, diamond: &[u8]) -> Option<Vec<u8>> {
    let bg = palette.rgb8(0);
    let mut rgb = Vec::with_capacity(3 * cx * cy);
    let mut off = 0usize;
    for r in 0..cy {
        let w = row_width(r, cy);
        let x0 = (cx - w) / 2;
        for c in 0..cx {
            let color = if c >= x0 && c < x0 + w {
                let idx = *diamond.get(off)? as usize;
                off += 1;
                palette.rgb8(idx)
            } else {
                bg
            };
            rgb.extend_from_slice(&color);
        }
    }
    (off == diamond.len()).then_some(rgb)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 256 色测试调色板:i 号色 6 位原值 = i&63(rgb8 = 展开),0 号 = 0。
    fn test_palette() -> Palette {
        let mut rgb = [[0u8; 3]; 256];
        for (i, slot) in rgb.iter_mut().enumerate() {
            let v = (i & 63) as u8;
            *slot = [v, v, v];
        }
        Palette { rgb }
    }

    /// B 族阶梯行宽序列 60×30 逐行钉死(mask 表 0x3E2B5C/0x3EC4C8 铁证):
    /// [4,8,…,60,56,…,4,0],和 900 = 60·30/2 精确闭合。
    #[test]
    fn ladder_row_widths_60x30() {
        let widths: Vec<usize> = (0..30).map(|r| row_width(r, 30)).collect();
        let expect: Vec<usize> = (1..=15)
            .map(|i| 4 * i)
            .chain((0..15).rev().map(|i| 4 * i))
            .collect();
        assert_eq!(widths, expect);
        assert_eq!(widths.iter().sum::<usize>(), 900);
        let pal = test_palette();
        assert!(paint_canvas(&pal, 60, 30, &vec![1u8; 900]).is_some());
    }

    /// 8×4 全量字节级钉:行宽 [4,8,4,0](末行宽 0 全背景),x0 = [2,0,2,4]。
    #[test]
    fn canvas_8x4_byte_exact() {
        let pal = test_palette();
        let diamond: Vec<u8> = (1..=16).collect();
        let rgb = paint_canvas(&pal, 8, 4, &diamond).unwrap();
        assert_eq!(rgb.len(), 3 * 8 * 4);
        let px = |r: usize, c: usize| -> [u8; 3] {
            let i = 3 * (r * 8 + c);
            [rgb[i], rgb[i + 1], rgb[i + 2]]
        };
        let widths = [4usize, 8, 4, 0];
        let x0s = [2usize, 0, 2, 4];
        let mut it = 1u8;
        for r in 0..4 {
            for c in 0..8 {
                let inside = (x0s[r]..x0s[r] + widths[r]).contains(&c);
                if inside {
                    let v = it;
                    it += 1;
                    assert_eq!(px(r, c), [(v << 2) | (v >> 4); 3], "({r},{c})");
                } else {
                    assert_eq!(px(r, c), [0, 0, 0], "行外 ({r},{c})");
                }
            }
        }
    }

    /// 奇 cy(6×3):阶梯耗 8/9 → None;偶 cy 菱形短缺同 None(敌意饱和)。
    #[test]
    fn odd_canvas_and_short_diamond_return_none() {
        let pal = test_palette();
        assert!(paint_canvas(&pal, 6, 3, &[9u8; 9]).is_none());
        assert!(paint_canvas(&pal, 8, 4, &[7u8; 15]).is_none());
    }
}
```

- [ ] **Step 3: lib.rs 挂模块**

`crates/core/src/lib.rs` 第 5 行 `pub mod format;` 后加一行 `pub mod render;`。

- [ ] **Step 4: 工具改调 + 删本地副本**

`tools/ra2-map/src/main.rs`:
- import 区(现第 9-16 行)加 `use ra2_core::render::terrain::paint_canvas;`
- 删本地 `fn paint_canvas(...)` 定义(现约 130-152 行,含 doc 注释)与文件尾 `#[cfg(test)] mod tests {...}`(3 个测试已随迁);
- `run_extract_tile` 内调用处不变(名字同)。

- [ ] **Step 5: 跑测试**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 合成 235 passed / 0 failed / 35 ignored(数量不变:ra2-map 迁走 3 个、core 落 3 个);clippy 净(`MAX_RENDER_PIXELS/CELL_*/Z_STEP/RenderStats/TerrainRender` 均 pub,lib 公有项无 dead_code 警告)。

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/render/ crates/core/src/lib.rs tools/ra2-map/src/main.rs
git commit -m "refactor(render): core::render 脚手架+paint_canvas 自工具上移(M4a T1)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 2: `render_terrain` 合成器(投影/z/画布/绘制序/统计/护栏)

**Files:**
- Modify: `crates/core/src/render/terrain.rs`(加 `compose_terrain` + `render_terrain` + `blit_diamond` + 测试)
- Modify: `crates/core/src/render/mod.rs`(去掉 T1 临时 `#[allow(dead_code)]` 若有)
- Modify: `crates/core/src/format/map/mod.rs`(加 `MapFile::size_wh`)

**Interfaces:**
- Consumes: T1 全部;`MapFile.tiles.cell(x,y) -> Option<&IsoTile>`(iso.rs:121,字段 `tile: i16 / sub_tile: u8 / z: u8`);`TheaterIni::file_name(index, ext) -> Option<SlotFile>`;`TheaterAssets::lookup(&str)`;`TsTmpFile::parse/slot/dims`;`theater_spec(Theater) -> TheaterSpec{ini_name, ext, capacity}`。
- Produces(Task 3/4 依赖,逐字):
  ```rust
  impl MapFile { pub fn size_wh(&self) -> (u16, u16); }        // map/mod.rs
  pub fn render_terrain(                                      // render/terrain.rs
      map: &MapFile,
      assets: &TheaterAssets,
      ini: &TheaterIni,
      palette: &Palette,
  ) -> Result<TerrainRender, FormatError>;
  pub(crate) fn compose_terrain(                              // 测试直打
      w: u16, h: u16, cells: &[Option<IsoTile>],              // 行主序 y·w+x
      assets: &TheaterAssets, ini: &TheaterIni, ext: &str,
      palette: &Palette,
  ) -> Result<TerrainRender, FormatError>;
  ```

- [ ] **Step 1: `MapFile::size_wh`**(与 parse 期 `map_size_wh` 同规则:研究 §8-3 兜底语义)

`crates/core/src/format/map/mod.rs` 的 `impl MapFile` 块内加:

```rust
    /// [Map]Size 的 (W,H),与解析期同规则:合法域 1..=4096,否则 (300,300)
    /// 兜底(瓦片记录数上界按兜底口径放行,渲染遍历须与之一致)。
    pub fn size_wh(&self) -> (u16, u16) {
        let w = self.map_meta.size[2];
        let h = self.map_meta.size[3];
        if (1..=4096).contains(&w) && (1..=4096).contains(&h) {
            (w as u16, h as u16)
        } else {
            (300, 300)
        }
    }
```

- [ ] **Step 2: 写失败测试**(先于实现;MIX 夹具逐字复制 `format/theater.rs:668` 的 `make_mix`;TMP 夹具按 theater_tmp.rs 模块注释布局手搓:16B i32 头 `{cbx,cby,cx,cy}` + cbx·cby 个 i32 槽偏移(文件基相对,单槽合法值 = 16+4 = 20)+ 52B 槽头 + RAW 菱形)

`render/terrain.rs` tests 模块追加(所需 use:`crate::format::theater::{SlotFile, TheaterAssets, TheaterIni}`、`crate::format::map::iso::IsoTile`、`crate::format::FormatError`;**IsoTile 是全字段 pub 的 Copy 结构,字面量必须含 x/y**):

```rust
    // ===== compose_terrain 夹具(手法同 format/theater.rs 测试)=====

    /// flags=0 最小合法 MIX(按名条目)。
    fn make_mix(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        let body_len: usize = entries.iter().map(|(_, c)| c.len()).sum();
        v.extend_from_slice(&(body_len as u32).le_bytes());
        let mut off = 0u32;
        for (name, c) in entries {
            v.extend_from_slice(&crate::format::crc::mix_name_id(name).to_le_bytes());
            v.extend_from_slice(&off.to_le_bytes());
            v.extend_from_slice(&(c.len() as u32).to_le_bytes());
            off += c.len() as u32;
        }
        for (_, c) in entries {
            v.extend_from_slice(c);
        }
        v
    }

    /// 单槽 60×30 TMP:头 {1,1,60,30} + 槽表 1 项(偏移 20 = 16+4·1,文件
    /// 基相对)+ 52B 全零槽头(bitfield=0:无 extra 无 z)+ 900B 菱形全 =
    /// fill。总长 20+52+900 = 972,clear01.sno 同构去 z。
    fn tmp60(fill: u8) -> Vec<u8> {
        let mut v = Vec::new();
        for i32v in [1i32, 1, 60, 30, 20] {
            v.extend_from_slice(&i32v.to_le_bytes());
        }
        v.extend_from_slice(&[0u8; 52]);
        v.extend_from_slice(&vec![fill; 900]);
        v
    }

    fn tile(x: u16, y: u16, tile: i16, sub: u8, z: u8) -> IsoTile {
        IsoTile { x, y, tile, sub_tile: sub, z }
    }

    fn tiny_assets(tmps: &[(&str, Vec<u8>)]) -> TheaterAssets {
        let d = std::env::temp_dir().join(format!(
            "ra2-render-t{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let refs: Vec<(&str, &[u8])> = tmps
            .iter()
            .map(|(n, b)| (*n, b.as_slice()))
            .collect();
        // open() 必需 ra2md.mix 与 ra2.mix(theater.rs:369-374);瓦片放
        // ra2md(pos 100,优先于 ra2 的 101),无 expandmd/ecache 遮蔽
        std::fs::write(d.join("ra2.mix"), make_mix(&[])).unwrap();
        std::fs::write(d.join("ra2md.mix"), make_mix(&refs)).unwrap();
        TheaterAssets::open(&d).unwrap()
    }

    fn tiny_ini() -> TheaterIni {
        TheaterIni::parse("[TileSet0000]\nFileName=tst\nTilesInSet=2\n").unwrap()
    }

    /// 画布锚点:2×1 图(ox = 30·(h−1) = 0),(0,0) 格铺在 sx=0/sy=0;
    /// 菱形行 0 宽 4、x0=(60−4)/2=28 → 画布列 28..31 色 = rgb8(9)。
    #[test]
    fn projection_and_canvas_anchors() {
        let pal = test_palette();
        let assets = tiny_assets(&[("tst01.ubn".to_string(), tmp60(9))]);
        let ini = tiny_ini();
        // 行主序 y·w+x:(0,0) 有瓦片,(1,0) 无记录
        let cells = vec![Some(tile(0, 0, 0, 0, 0)), None];
        let r = compose_terrain(2, 1, &cells, &assets, &ini, "ubn", &pal).unwrap();
        // W = 30·(2+1) = 90,H = 15·(2+1) + 15·z_max(0) = 45
        assert_eq!((r.width, r.height), (90, 45));
        assert_eq!(r.stats.cells, 2);
        assert_eq!(r.stats.painted, 1);
        assert_eq!(r.stats.no_tile, 1); // 无记录口径并入 no_tile
        let px = |x: usize, y: usize| -> [u8; 3] {
            let i = 3 * (y * r.width as usize + x);
            [r.rgb[i], r.rgb[i + 1], r.rgb[i + 2]]
        };
        let c9 = [(9u8 << 2) | (9 >> 4); 3];
        assert_eq!(px(28, 0), c9, "菱形行 0 首像素(列 28)");
        assert_eq!(px(31, 0), c9, "菱形行 0 末像素(列 31)");
        assert_eq!(px(27, 0), [0, 0, 0], "行外 = 背景 0 号色");
        assert_eq!(px(0, 44), [0, 0, 0], "画布末行 = 背景");
    }

    /// z 语义 = 相对画布顶的下沉:高格 (0,0)@z 钉在 sy=0(oy−15z 恰抵消),
    /// 地面格 (1,0)@z=0 随 oy = 15·z_max 下沉;H 同涨 15·z_max。
    /// z∈{0,1,7,14}:7 含 728 fudge 档,14 = 语料 z 上界。
    #[test]
    fn z_shifts_15px_per_level() {
        let pal = test_palette();
        let assets = tiny_assets(&[
            ("tst01.ubn".to_string(), tmp60(9)),
            ("tst02.ubn".to_string(), tmp60(20)),
        ]);
        let ini = tiny_ini();
        for z in [0u8, 1, 7, 14] {
            let cells = vec![
                Some(tile(0, 0, 0, 0, z)), // (0,0) d=0 高格
                Some(tile(1, 0, 1, 0, 0)), // (1,0) d=1 地面格
            ];
            let r = compose_terrain(2, 1, &cells, &assets, &ini, "ubn", &pal).unwrap();
            assert_eq!(r.height as usize, 15 * 3 + 15 * z as usize, "z={z}");
            let w = r.width as usize;
            // (0,0):sy = oy + 0 − 15·z = 0 恒;行 0 列 28 = rgb8(9)
            let i = 3 * 28;
            assert_eq!(r.rgb[i], (9 << 2) | (9 >> 4), "z={z} 高格钉顶");
            // (1,0):sx = 30,sy = oy + 15 − 0 = 15·z + 15;行 0 列 58 = rgb8(20)
            let j = 3 * ((15 * z as usize + 15) * w + 58);
            assert_eq!(r.rgb[j], (20 << 2) | (20 >> 4), "z={z} 地面格下沉 15·z");
        }
    }

    /// 缺环全链各计其数、不中止;七类计数恒等(cells = 和)。
    #[test]
    fn missing_chain_counts() {
        let pal = test_palette();
        let mut bad = tmp60(9);
        bad.truncate(30); // 槽声明偏移 20 但体只余 10B → parse Invalid
        let assets = tiny_assets(&[
            ("tst01.ubn".to_string(), tmp60(9)),
            ("tst02.ubn".to_string(), bad),
        ]);
        let ini = TheaterIni::parse(
            "[TileSet0000]\nFileName=tst\nTilesInSet=3\n[TileSet0001]\nTilesInSet=1\n",
        )
        .unwrap(); // set0001 无 FileName → 槽 3 = Reserved;全局槽位共 4
        let cells = vec![
            Some(tile(0, 0, -1, 0, 0)), // no_tile(tile=-1)
            Some(tile(1, 0, 0, 0, 0)),  // painted(tst01)
            Some(tile(0, 1, 1, 0, 0)),  // parse_fail(tst02 敌意)
            Some(tile(1, 1, 2, 0, 0)),  // missing_file(tst03 未出货)
            Some(tile(0, 2, 3, 0, 0)),  // reserved(set0001)
            Some(tile(1, 2, 9, 0, 0)),  // out_of_range(越 4 槽)
            Some(tile(0, 3, 0, 5, 0)),  // missing_slot(cbx·cby=1,槽 5 空)
            None,                       // 无记录 → no_tile
        ];
        let r = compose_terrain(2, 4, &cells, &assets, &ini, "ubn", &pal).unwrap();
        assert_eq!(r.stats.cells, 8);
        assert_eq!(r.stats.painted, 1);
        assert_eq!(r.stats.no_tile, 2);
        assert_eq!(r.stats.parse_fail, 1);
        assert_eq!(r.stats.missing_file, 1);
        assert_eq!(r.stats.reserved, 1);
        assert_eq!(r.stats.out_of_range, 1);
        assert_eq!(r.stats.missing_slot, 1);
        let s = r.stats;
        assert_eq!(
            s.cells,
            s.painted + s.no_tile + s.parse_fail + s.missing_file + s.reserved
                + s.out_of_range + s.missing_slot,
            "七类计数恒等"
        );
    }

    /// 4096×4096 敌意图 → 护栏先于分配与遍历拒绝(不触 cells 索引)。
    #[test]
    fn hostile_canvas_guard() {
        let pal = test_palette();
        let assets = tiny_assets(&[]);
        let ini = tiny_ini();
        let cells = vec![None; 64]; // 刻意短于 w·h:护栏必须先于遍历
        assert!(matches!(
            compose_terrain(4096, 4096, &cells, &assets, &ini, "ubn", &pal),
            Err(FormatError::Invalid(_))
        ));
    }

    /// 行对角线序:d+1 近瓦片覆写 d 远瓦片的重叠带((0,0) 行 15 是满宽行
    /// x0=0..60,(1,0) 行 0 平移到画布行 15 列 30+28=58 → 交叠,后画者胜)。
    #[test]
    fn diagonal_painters_order() {
        let pal = test_palette();
        let assets = tiny_assets(&[
            ("tst01.ubn".to_string(), tmp60(1)),
            ("tst02.ubn".to_string(), tmp60(30)),
        ]);
        let ini = tiny_ini();
        let cells = vec![
            Some(tile(0, 0, 0, 0, 0)), // (0,0) d=0 远
            Some(tile(1, 0, 1, 0, 0)), // (1,0) d=1 近
        ];
        let r = compose_terrain(2, 1, &cells, &assets, &ini, "ubn", &pal).unwrap();
        // (1,0):sx = 30,sy = 15;其行 0(画布行 15)列 58 = rgb8(30)
        let i = 3 * (15 * r.width as usize + 58);
        assert_eq!(r.rgb[i], (30 << 2) | (30 >> 4), "近瓦片后画胜");
    }
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cargo test -p ra2-core render::`
Expected: 编译失败 `cannot find function compose_terrain`(TDD 红)。

- [ ] **Step 4: 实现**

`render/terrain.rs` 追加:

```rust
use crate::format::map::iso::IsoTile;
use crate::format::map::MapFile;
use crate::format::theater::{theater_spec, SlotFile, TheaterAssets, TheaterIni};
use crate::format::theater_tmp::TsTmpFile;
use crate::format::FormatError;
use super::{RenderStats, TerrainRender, CELL_DX, CELL_DY, MAX_RENDER_PIXELS, Z_STEP};

/// 整图合成入口(spec §4):剧场表/资产/调色板由调用方装载(CLI 复用
/// extract-tile 的装载链);内部委托 [`compose_terrain`]。
pub fn render_terrain(
    map: &MapFile,
    assets: &TheaterAssets,
    ini: &TheaterIni,
    palette: &Palette,
) -> Result<TerrainRender, FormatError> {
    let (w, h) = map.size_wh();
    let mut cells: Vec<Option<IsoTile>> = vec![None; w as usize * h as usize];
    for y in 0..h {
        for x in 0..w {
            if let Some(t) = map.tiles.cell(x, y) {
                cells[y as usize * w as usize + x as usize] = Some(IsoTile {
                    x,
                    y,
                    tile: t.tile,
                    sub_tile: t.sub_tile,
                    z: t.z,
                });
            }
        }
    }
    let spec = theater_spec(map.map_meta.theater);
    compose_terrain(w, h, &cells, assets, ini, spec.ext, palette)
}

/// 纯合成核(测试直打):行主序 cells,行对角线 d=x+y 升序近平后画。
/// 画布 W=30·(w+h),H=15·(w+h)+15·z_max,原点 ox=30·(h−1)/oy=15·z_max。
pub(crate) fn compose_terrain(
    w: u16,
    h: u16,
    cells: &[Option<IsoTile>],
    assets: &TheaterAssets,
    ini: &TheaterIni,
    ext: &str,
    palette: &Palette,
) -> Result<TerrainRender, FormatError> {
    let (w, h) = (w as usize, h as usize);
    let z_max = cells
        .iter()
        .flatten()
        .map(|t| t.z)
        .max()
        .unwrap_or(0) as usize;
    let width = CELL_DX * (w + h);
    let height = CELL_DY * (w + h) + Z_STEP * z_max;
    if width as u64 * height as u64 > MAX_RENDER_PIXELS {
        return Err(FormatError::Invalid(format!(
            "画布 {width}×{height} = {} px 超 MAX_RENDER_PIXELS {MAX_RENDER_PIXELS}(敌意 [Map]Size?)",
            width as u64 * height as u64
        )));
    }
    let oy = Z_STEP * z_max; // x 向原点已并入 sx 公式(h−1+2x−d)
    let bg = palette.rgb8(0);
    let mut rgb = vec![0u8; 3 * width * height];
    for chunk in rgb.chunks_exact_mut(3) {
        chunk.copy_from_slice(&bg);
    }
    let mut stats = RenderStats {
        cells: cells.len(),
        ..RenderStats::default()
    };
    // 行对角线序:d = x+y 升序(近平后画;同对角线互不重叠)。
    for d in 0..(w + h).saturating_sub(1) {
        let x_lo = d.saturating_sub(h - 1);
        let x_hi = d.min(w - 1);
        for x in x_lo..=x_hi {
            let Some(t) = cells[(d - x) * w + x] else {
                stats.no_tile += 1; // 界内无记录(口径含无瓦片格)
                continue;
            };
            if t.tile < 0 {
                stats.no_tile += 1;
                continue;
            }
            match ini.file_name(t.tile as usize, ext) {
                None => stats.out_of_range += 1,
                Some(SlotFile::Reserved) => stats.reserved += 1,
                Some(SlotFile::File(name)) => {
                    let Some(blob) = assets.lookup(&name) else {
                        stats.missing_file += 1;
                        continue;
                    };
                    match TsTmpFile::parse(blob) {
                        Err(_) => stats.parse_fail += 1,
                        Ok(tmp) => match tmp.slot(t.sub_tile as usize) {
                            None => stats.missing_slot += 1,
                            Some(slot) => {
                                let (_, _, cx, cy) = tmp.dims();
                                // sx = ox + 30·(x−y),y = d−x → 2x−d;并入 ox:
                                // h−1+2x−d ∈ [0, w+h−2] 恒非负,免 isize
                                let sx = CELL_DX * (h - 1 + 2 * x - d);
                                // sy = oy + 15·d − 15·z;oy ≥ 15·z 恒成立,
                                // 左结合求值无下溢
                                let sy = oy + CELL_DY * d - Z_STEP * t.z as usize;
                                if cx as usize != 2 * cy as usize
                                    || !blit_diamond(
                                        &mut rgb,
                                        width,
                                        sx,
                                        sy,
                                        cx as usize,
                                        cy as usize,
                                        palette,
                                        slot.diamond_pixels(),
                                    )
                                {
                                    stats.parse_fail += 1; // 非 2:1/阶梯不闭合
                                } else {
                                    stats.painted += 1;
                                }
                            }
                        },
                    }
                }
            }
        }
    }
    Ok(TerrainRender {
        width: width as u32,
        height: height as u32,
        rgb,
        stats,
    })
}

/// 菱形逐像素落画布(只写行内阶梯区;画布背景已统一铺,与单瓦片
/// `paint_canvas` 的填充语义不同)。返回 false = 阶梯消耗与菱形字节量
/// 不符(奇 cy 病态/敌意短缺,调用方计 parse_fail)。
fn blit_diamond(
    rgb: &mut [u8],
    cw: usize,
    sx: usize,
    sy: usize,
    cx: usize,
    cy: usize,
    palette: &Palette,
    diamond: &[u8],
) -> bool {
    let mut off = 0usize;
    for r in 0..cy {
        let w = row_width(r, cy);
        let x0 = (cx - w) / 2;
        for c in x0..x0 + w {
            let Some(&idx) = diamond.get(off) else {
                return false;
            };
            off += 1;
            // 界内校验防御性(get_mut 饱和):正常路径恒界内(见画布公式)
            let i = 3 * ((sy + r) * cw + sx + c);
            if let Some(dst) = rgb.get_mut(i..i + 3) {
                dst.copy_from_slice(&palette.rgb8(idx as usize));
            }
        }
    }
    off == diamond.len()
}
```

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p ra2-core render::`
Expected: 8 passed(3 迁移 + 5 新:anchors/z_shifts/missing_chain/guard/diagonal)。

- [ ] **Step 6: 全量门禁**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: 合成 240 passed / 0 failed / 35 ignored(235+5)。

- [ ] **Step 7: Commit**

```bash
git add crates/core/src/render/ crates/core/src/format/map/mod.rs
git commit -m "feat(render): render_terrain 合成器——投影 30(x−y)/15(x+y)、z=15px/级、行对角线序、缺环统计、128Mpx 护栏(M4a T2)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 3: `ra2-map render` CLI 子命令

**Files:**
- Modify: `tools/ra2-map/src/main.rs`(Action 枚举 + parse arm + `run_render` + usage)

**Interfaces:**
- Consumes: T2 `render_terrain`;现有 `load_theater_palette`(main.rs:113)、`decode_map_text`、`theater_spec`、`TheaterAssets::open`、`TheaterIni::parse`、`png::write_png_file(path,w,h,rgb)`。
- Produces: CLI 契约 `ra2-map render <map> -o <out.png> [--assets <dir>]`(0/1/2)。

- [ ] **Step 1: 写失败测试**(`tools/ra2-map/tests/cli.rs` 追加;手法同现有 usage/info 测试)

```rust
// 用法错:render 缺 -o → 退出 2
#[test]
fn render_usage_errors_exit_2() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_ra2-map"))
        .args(["render", "whatever.map"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}
```

Run: `cargo test -p ra2-map --test cli render_usage`
Expected: FAIL(现在 "render" 落到 `_ => usage()` 前的匹配分支外——实际会 usage 退出 2?**注意**:现有 parse 若把未知子命令落 usage 也返回 2,此测试可能直接绿——改为先测**成功路径**红:见 Step 1b)。

- [ ] **Step 1b: 成功路径失败测试**(真红;用 `RA2_ASSETS_DIR` 缺失的确定性失败)

```rust
// 缺资产目录(无 --assets 且 RA2_ASSETS_DIR 清空)→ 退出 1 且 stderr 提示
#[test]
fn render_needs_assets_exit_1() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_ra2-map"))
        .args(["render", "whatever.map", "-o", "x.png"])
        .env_remove("RA2_ASSETS_DIR")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("RA2_ASSETS_DIR"));
}
```

Run: `cargo test -p ra2-map --test cli render_needs`
Expected: FAIL(未知子命令现走 usage 退出 2)。

- [ ] **Step 2: 实现**(`main.rs`)

Action 枚举加变体(与 ExtractTile 相邻):

```rust
    Render {
        map: String,
        out: String,
        assets: Option<String>,
    },
```

parse arm(放在 extract-tile arm 之前):

```rust
        ["render", map, flag, out, rest @ ..] if *flag == "-o" => {
            let assets = match rest {
                [] => None,
                ["--assets", dir] => Some(*dir),
                _ => usage(),
            };
            Action::Render {
                map,
                out,
                assets,
            }
        }
```

`run_render`(放 `run_extract_tile` 前;装载链逐字复用 extract-tile 的 191-228 行段):

```rust
fn run_render(map_path: &str, out_path: &str, assets_opt: Option<&str>) -> ExitCode {
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
    let assets_dir = assets_opt.map(std::path::PathBuf::from).or_else(|| {
        std::env::var("RA2_ASSETS_DIR")
            .ok()
            .map(std::path::PathBuf::from)
    });
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
    let t0 = std::time::Instant::now();
    let r = match ra2_core::render::terrain::render_terrain(&m, &assets, &ini, &palette) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("合成失败: {e}");
            return ExitCode::from(1);
        }
    };
    let dt = t0.elapsed();
    if let Err(e) = png::write_png_file(
        std::path::Path::new(out_path),
        r.width,
        r.height,
        &r.rgb,
    ) {
        eprintln!("写 {out_path} 失败: {e}");
        return ExitCode::from(1);
    }
    let s = r.stats;
    eprintln!(
        "已写 {out_path}: {}×{}px | 格 {} = 落绘 {} + 无瓦片 {} + 缺文件 {} + 保留槽 {} + 越界 {} + 空槽 {} + 解析败 {} | 剧场 {:?} | 耗时 {dt:?}",
        r.width,
        r.height,
        s.cells,
        s.painted,
        s.no_tile,
        s.missing_file,
        s.reserved,
        s.out_of_range,
        s.missing_slot,
        s.parse_fail,
        m.map_meta.theater,
    );
    ExitCode::SUCCESS
}
```

(计划注:stderr 统计格式实现者可微调措辞,五类缺环计数必须全数出现。)

`usage()` 加一行(与 extract-tile 行并列,保持现有文案风格):

```
  ra2-map render <map.map> -o <out.png> [--assets <dir>]   整幅地形渲染 PNG(spec M4a)
```

`main()` match 加 arm:`Action::Render { map, out, assets } => run_render(map, out, assets),`

- [ ] **Step 3: 跑测试**

Run: `cargo test -p ra2-map && cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: cli 全绿(含 2 个新测试);workspace 合成 242(240+2)。

- [ ] **Step 4: 真资产冒烟(手跑一次)**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo run -p ra2-map -- render "E:\hongjing\Reunion 2023\Maps\Standard\<任选 TEMPERATE/SNOW/URBAN 图>.map" -o "$TEMP/m4a-smoke.png"`
Expected: stderr 概要 + 退出 0;`$TEMP/m4a-smoke.png` 可打开目检(等距菱形拼图;**勿选 DESERT/NEWURBAN/LUNAR 图——本机缺其调色板,CLI 退出 1 属预期**)。
**目检不通过(几何错乱/全黑)即 BLOCKED 上报,不进入 Task 4。**

- [ ] **Step 5: Commit**

```bash
git add tools/ra2-map/src/main.rs tools/ra2-map/tests/cli.rs
git commit -m "feat(ra2-map): render 子命令——整幅地形 PNG,五类缺环统计入 stderr(M4a T3)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 4: 真资产验收套件(807 零 panic + 真彩剧场抽样 + 尺寸钉 + Preview 差异探针 + 性能预算)

**Files:**
- Create: `crates/core/tests/m4a_acceptance.rs`(全部 `#[ignore]`;地图**递归**枚举手法复制 m3_acceptance.rs:20-31,资产装载复制其头部夹具——实现者先读该文件)

**Interfaces:**
- Consumes: T2 全部;`MapFile::preview -> Option<PreviewImage{width: u32, height: u32, rgb: Vec<u8>>}`(map/preview.rs:15;24bpp 行紧排,通道序/行序 M2 未裁决 → 探针四组合取最小);`Theater::iso_palette_file()`(meta.rs:45)。
- Produces: 验收常量(预算/抽样钉值)与 Preview 对拍裁定数据(控制器消费,见 Step 3);spec §6.2 勘误记录。

- [ ] **Step 1: 写验收测试**(断言先只含确定性项;哑调色板 = 768 零字节全黑——链路/统计真实,颜色不计)

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! M4a 真资产验收(spec §6 + 勘误):807 图全量合成零 panic + 真彩剧场
//! 抽样 PPM + 画布尺寸公式钉死 + 内嵌 Preview 差异探针(四组合最小,
//! 输出供控制器裁定对拍走向)+ 性能预算。
//! 调色板边界(2026-11-06 探针):isodes/isoubn/isolun.pal 本机缺席
//! (全容器按名 + 0x3B5A96DE 库回退均无)→ desert/newurban/lunar 用
//! 哑调色板跑链路零 panic,不参与抽样/对拍。全部 ignore,需 RA2_ASSETS_DIR。

use ra2_core::format::map::mapdoc::decode_map_text;
use ra2_core::format::map::MapFile;
use ra2_core::format::pal::Palette;
use ra2_core::format::theater::{theater_spec, TheaterAssets, TheaterIni};
use ra2_core::render::terrain::render_terrain;

fn assets_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(
        std::env::var("RA2_ASSETS_DIR").expect("RA2_ASSETS_DIR 指向用户资产目录"),
    )
}

/// 递归收集 .map(m3_acceptance.rs:20-31 同构手法;语料 807 = Maps/ 540
/// + RMG MapUnits 267;扩展名只有 .map,无 .yrm)。
fn maps() -> Vec<std::path::PathBuf> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
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
    (Palette { rgb: [[0u8; 3]; 256] }, false)
}

/// PPM 直写(P3 太大用 P6 二进制;目检样本用,core 测试不依赖 tools png)。
fn write_ppm(out: &std::path::Path, w: u32, h: u32, rgb: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut f = std::fs::File::create(out)?;
    write!(f, "P6\n{w} {h}\n255\n")?;
    f.write_all(rgb)
}

/// 全量渲染:零 panic + 尺寸公式 + 真彩剧场首图抽样 PPM + 耗时/缺档总账。
#[test]
#[ignore]
fn m4a_all_maps_render_and_samples() {
    let assets = TheaterAssets::open(&assets_dir()).unwrap();
    let mut total = 0usize;
    let mut total_ms = 0u128;
    let mut max_ms: (u128, String) = (0, String::new());
    let mut total_missing_file = 0usize;
    // 剧场 → (渲染数, 真彩数, 已抽样?)
    let mut by_theater: std::collections::BTreeMap<String, (usize, usize, bool)> =
        Default::default();
    for path in maps() {
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(m) = MapFile::parse(&bytes) else { continue };
        let spec = theater_spec(m.map_meta.theater);
        let ini = TheaterIni::parse(
            &decode_map_text(assets.lookup(spec.ini_name).expect("剧场表在")),
        )
        .unwrap();
        let (pal, real) = theater_palette(&assets, m.map_meta.theater);
        let t0 = std::time::Instant::now();
        let r = render_terrain(&m, &assets, &ini, &pal).expect("全量合成零 panic");
        let ms = t0.elapsed().as_millis();
        total += 1;
        total_ms += ms;
        if ms > max_ms.0 {
            max_ms = (ms, path.display().to_string());
        }
        total_missing_file += r.stats.missing_file;
        let (w, h) = m.size_wh();
        // H 反推 z_max 兼作一致性校验(H 公式钉死)
        let z_max = (r.height as usize - 15 * (w as usize + h as usize)) / 15;
        assert_eq!(
            r.width as usize,
            30 * (w as usize + h as usize),
            "{}",
            path.display()
        );
        assert_eq!(r.height as usize, 15 * (w as usize + h as usize) + 15 * z_max);
        let key = format!("{:?}", m.map_meta.theater);
        let entry = by_theater.entry(key).or_insert((0, 0, false));
        entry.0 += 1;
        if real {
            entry.1 += 1;
            if !entry.2 {
                entry.2 = true;
                let out = std::env::temp_dir().join(format!(
                    "ra2-m4a-sample-{}.ppm",
                    path.file_name().unwrap().to_string_lossy()
                ));
                write_ppm(&out, r.width, r.height, &r.rgb).unwrap();
                eprintln!("样本: {}", out.display());
            }
            // Preview 对拍仅对真彩图有意义(哑调色板必 100% 差)
            if let Some(pv) = &m.preview {
                probe_preview_diff(&path, &r, pv);
            }
        }
    }
    eprintln!(
        "M4a: 共 {total} 图,总耗时 {total_ms}ms,最慢 {max_ms:?},missing_file 总计 {total_missing_file}"
    );
    for (k, (n, real_n, sampled)) in &by_theater {
        eprintln!("剧场 {k}: {n} 图(真彩 {real_n},抽样 {sampled})");
    }
    assert_eq!(total, 807, "语料 = 807(M2/M3 口径)");
    let real_theaters = by_theater.values().filter(|v| v.2).count();
    assert!(
        real_theaters >= 3,
        "真彩抽样至少覆盖 TEMPERATE/SNOW/URBAN,实得 {real_theaters}"
    );
}

/// Preview 差异探针:最近邻缩采样(渲染 → preview 尺寸),通道序
/// (RGB/BGR)× 行序(正常/上下翻)四组合取最小——preview.rs:像素/通道
/// 序 M2 未裁决,取最小即"最优解释下的差率"。逐图打印,控制器裁定。
fn probe_preview_diff(
    path: &std::path::Path,
    r: &ra2_core::render::TerrainRender,
    pv: &ra2_core::format::map::preview::PreviewImage,
) {
    let (w, h) = (r.width as usize, r.height as usize);
    let (pw, ph) = (pv.width as usize, pv.height as usize);
    let mut best = (usize::MAX, "");
    for (swap, flip) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut diff = 0usize;
        for py in 0..ph {
            for pxx in 0..pw {
                let sx = (pxx * w / pw.max(1)).min(w - 1);
                let sy_raw = py * h / ph.max(1);
                let sy = if flip { (ph - 1 - py) * h / ph.max(1) } else { sy_raw }.min(h - 1);
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
                    diff += 1; // 通道和差 > 96 记差异(探针阈值,控制器裁定)
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
}
```

- [ ] **Step 2: 跑真资产套件**

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test m4a_acceptance -- --ignored --nocapture 2>&1 | tail -60`
Expected: 1 passed;stderr:真彩图逐条 preview-diff(最优组合)+ 样本 PPM 路径 + 总账行(共 807 图/最慢/missing_file)+ 逐剧场行;断言(807/尺寸公式/真彩 ≥3 剧场)全过。
**任何 panic/assert 失败 = BLOCKED 上报附输出。**

- [ ] **Step 3: 控制器裁定步(非子代理;结论记 `.superpowers/sdd/progress.md` + 上报用户)**

依据 Step 2 输出:
- **Preview 对拍走向**(spec §6.4 双枝):看真彩图最优组合差率分布——若中位差率 < 10% 且最优组合稳定集中(通道/行序可裁定)→ 走向 A:钉通道序结论 + 中位差率×2 回归阈值入断言;若差异系统性(预期:预览含矿石/物件 overlay,M4a 只渲染地形)→ 走向 B:spec 勘误"Preview 对拍顺延 M4b 终验",探针输出保留不设断言。
- **性能预算**:取 max_ms×3 为 ignored 套件预算常数(硬帽 10s),入断言 `assert!(ms < 预算, "{path}")` 或文档化(按实测量级定)。
- **spec §6.2 勘误定稿**:"六剧场全覆盖" → "真彩三剧场(TEMPERATE/SNOW/URBAN)抽样全覆盖 + 缺调色板三剧场哑调色板零 panic 全覆盖"(本机安装边界,证据链 Global Constraints;向用户报告)。
- **缺槽账对照**(spec §6.5):total_missing_file 对照 M3 六剧场缺档账(0/78/0/0/383/44)——同量级即一致,显著偏离则逐剧场排查出报告。

- [ ] **Step 4: 裁定落地**(按 Step 3 走向补断言/勘误,重跑至绿)

Run: 同 Step 2。Expected: passed,断言含裁定阈值(或走向 B 时 spec 勘误段落成文)。

- [ ] **Step 5: Commit**

```bash
git add crates/core/tests/m4a_acceptance.rs docs/superpowers/specs/2026-11-06-m4a-terrain-render-design.md
git commit -m "test(m4a): 807 图整图渲染验收+尺寸公式钉死+真彩三剧场抽样+Preview 对拍裁定+§6.2 调色板勘误(M4a T4)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 5: 研究文档 + README + 收尾

**Files:**
- Create: `docs/superpowers/research/2026-11-06-m4a-terrain-render.md`
- Modify: `README.md`(用法区加 render 行)

**Interfaces:** 无代码;文档事实须与 T2-T4 实测一致。

- [ ] **Step 1: 研究文档**(内容骨架,证据链全录)

```markdown
# M4a 地形整图渲染 研究文档

## 1. 投影与高度(decomp 源码级)
- Rectangular_To_Isometric 0x006D1EB0:iso.X = X·60/2 − Y·60/2,iso.Y =
  X·30/2 + Y·30/2;Coord_To_Pixel_Absolute 0x006D1F10 再 /256(lepton/格)
  → sx = 30·(x−y),sy = 15·(x+y) − z_px。
- 高度:LEVEL_LEPTON_H = (int)(tan30° · 256√2/2) = 104(geometry.h:25);
  cell Z = 104·Height(cell.cpp:150,display.cpp:1327);
  Z_Lepton_To_Pixel 0x006D20E0 = int(lepton·sin60°·60/(256√2) + fudge + 0.5),
  fudge = lepton≥728 ? 1 : 0(7·104 = 728 恰触)。
  → z_px = 15·z 对 z∈0..14 逐档验证精确(0,15,…,105,120,135,150,165,
  180,195,210)。
- 画布:W = 30·(w+h),H = 15·(w+h)+15·z_max,ox = 30·(h−1),oy = 15·z_max
  (x−y ∈ [−(h−1), w−1];z_max 取全格实扫)。
- 绘制序:d = x+y 升序(近平后画);(x+1,y)/(x,y+1) 相对 +30/−30, +15 半格
  重叠,同对角线互不重叠 → 对角线内序不拘。
- (−30,−15) 半格居中(tactical.cpp:449/607,鼠标↔格逆变换侧)不进合成
  公式:我们的画布以 (0,0) 格画布左上为原点,与引擎视口原点平移等价。

## 2. 本机调色板边界(2026-11-06 探针,决定 spec §6.2 勘误)
- isodes.pal / isoubn.pal / isolun.pal 走 TheaterAssets 全容器按名 +
  调色板库 0x3B5A96DE 回退均缺席;Resources/Skin 下的 *des.pal 等是
  侧边栏图标调色板非 iso 瓦片调色板;根目录无散装 iso*.pal。
- 剧场分布(807 = Maps/ 540 + RMG MapUnits 267):temperate 212 /
  snow 106 / urban 78 / desert 73 / newurban 61 / lunar 1(^Theater=
  行命中 531,余为 RMG 模板无此行头;逐图剧场以 MapFile::parse 为准)。
- 结论:真彩抽样 = TEMPERATE/SNOW/URBAN;desert/newurban/lunar(135 图)
  哑调色板零 panic 覆盖,Preview 对拍不参与。

## 3. 语料实测(T4 输出回填)
- 807 图全量:耗时分布/最慢图/missing_file 总计:(回填)
- 真彩三剧场抽样 PPM:(路径)
- 画布尺寸公式断言:全过/异常:(回填)

## 4. Preview 对拍裁定(T4 Step 3 结论回填)
- 走向 A/B + 最优组合(通道序/行序)+ 差率分布 + 阈值或顺延理由:(回填)

## 5. 性能预算
- (回填 max_ms×3,硬帽 10s)

## 出处
- E:\hongjing\ra2-decomp(只读):tactical.cpp / geometry.h / cell.cpp /
  display.cpp 地址锚如上;语义重写,无拷码。
- 探针:%TEMP%\ra2-m4a-sample-*.ppm;验收测试 crates/core/tests/m4a_acceptance.rs。
```

- [ ] **Step 2: README** 用法区(extract-tile 行后)加:

```
  ra2-map render <map.map> -o <out.png> [--assets <dir>]   # M4a 整幅地形渲染 PNG(等距投影/z 接缝/缺环统计;本机真彩限 TEMPERATE/SNOW/URBAN)
```

- [ ] **Step 3: 全量门禁 + 真资产**

Run: `cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace && RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored`
Expected: 合成全绿(242);ignored 35+1(m2/m3 既有 35 + m4a_acceptance 1)。

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/research/2026-11-06-m4a-terrain-render.md README.md
git commit -m "docs(m4a): 研究文档(投影/z/画布/绘制序证据链+实测回填)+README render 行(M4a T5)" -m "Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

## 验收对照(spec §6)

| spec 条款 | 落点 |
|---|---|
| §6.1 807 零 panic | T4(真彩+哑调色板全覆盖) |
| §6.2 六剧场抽样+尺寸钉(勘误:真彩三剧场抽样 + 哑调色板三剧场零 panic) | T4 + Step 3 勘误定稿 |
| §6.3 投影/接缝/z 锚点合成测试 | T2(anchors/z_shifts/diagonal) |
| §6.4 Preview 双走向 | T4 Step 3 裁定(四组合探针) |
| §6.5 缺槽账一致性 | T4(missing_file 总计,对照 M3 0/78/0/0/383/44) |
| §7 CLI 契约 | T3 |
| §8 paint_canvas 随迁 | T1 |
