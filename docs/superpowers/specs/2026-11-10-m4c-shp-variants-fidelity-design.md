SPDX-License-Identifier: GPL-3.0-only

# M4c 设计:SHP 压缩变体解码 + 保真小批

日期:2026-11-10。分支:`feat/m4c-shp-variants-fidelity`(自 main 149e22e)。
上游研究:`docs/superpowers/research/2026-11-09-m4c-shp-variants-fidelity.md`
(下称"研究",§N 引文均指该文)。既有语义引 M1b 研究
2026-11-03-shp-tmp-layouts.md 与 M4b 研究/spec,本文只记增量。

## 0. 目标与范围

**目标**:把 SHP-TS 帧解码判别式从"先严格 RLE、败且 span==need 才 RAW"
改为引擎对齐的**帧级 flags bit1 门控**,消除 overlay_fail 28946(~11%
人口)与 objects_fail 3155;附带四个保真小件(VEIN 守卫、物件 z 侧账
计数位、阴影帧号规则旗标化)。

**用户裁决(2026-11-10)**:范围 = 研究建议包(全);验收 = 研究 §5.2
照钉(fail==0 主断言 + 总量 oracle + 守恒维持 + Preview 只报告不收紧)。

**入 M4c**:研究 §2.4 四条语义重写、VEIN 前缀守卫、objects_z_fallback
计数位、阴影帧号规则旗标化(零视觉变化)。

**顺延(不在本批,台账记账)**:remap/所属方调色板(LightConvert 构建
链未钉,研究 §4.2)、坡地 ore(0.44% 格面,需 [Tiberiums]
Variety/RampVariety,研究 §4.4)、阴影变暗 DARKEN 表(未钉,研究
§4.1/L7)、−2px fudge/SHAPE_ALPHA/ZWRITE(纯 z/alpha 缓冲面,定性对
2D RGB 零影响,研究 §4.3)、rubble 分支(语料 0)、别名表(无证据,
研究 §3/L6)、voxel 渲染器(另立 M5)。

**明确不做**:missing_art 3539 收敛——研究 §3 已证伪 `;` 注释假设
(artmd 1581 段/art.ini 0 段,剥注可解 0 件),真缺档 95 名(TS 语源
未随 YR 零售出货)唯一路径 = 用户侧资产补丁;M4c 将其钉为**负 oracle**
(3539 恒不变)。Tiberium/Crate bool 零售值干净(ini.rs get_bool 全串
精确匹配 = 引擎 Get_Bool 同判),不修。

## 1. SHP-TS 解码语义重写(研究 §2.4,核心)

改 `crates/core/src/format/shp_ts.rs`:

1. **`ShpTsFrameHeader` 增只读判定 `is_rle()`** = `(self.flags & 0xFF)
   & 2 != 0`。`flags` 字段仍存 u32 原样,字段注释更新:语义只在 +8 单
   字节(msanim.h:49-63 `SHPFrame::Flags`;+9..+0xB 为匿名填充,语料
   观测 `0xCCCC` 高半字即其未初始化残留,研究 §1.2/§2.1);+0xC..+0xF
   = Color 3B+1 pad(现 rgba 命名照旧)。
2. **判别式改引擎对齐**:删"先严格 RLE后 RAW"试探(shp_ts.rs:145-157),
   改 `is_rle()` 门控——bit1=1 → RLE 行链;bit1=0 → RAW。引擎分派
   `HasCompression → RLE_Blit : Bit_Blit`(shapipe.cpp:148-156,
   draw.cpp:88-98;研究 §2.2)。变体是**帧级**(CIV1 476 帧同文件混
   flags=0x03/0x01),任何文件级格式选择不可行。
3. **RAW 语义**:取 `[offset, offset+need)`(need = cx·cy);`offset+
   need > 文件尾` 时**补零截断**(语料 0 出现,防御位,敌意输入不再
   整文件拒;lenient 零 panic 契约),替代现 `span==need` 硬条件——
   RAW 帧普遍带 2-8B 尾 pad(GEM09 2×2 span 8/need 4;DRUM01 11×14
   span 160/need 154;FENCE01 span−need=4;研究 §1.3-A)。
4. **RLE 行块下限 4 → 2**:`crates/core/src/format/rle_zero_ts.rs:34`
   `count < 4` 改 `count < 2`(count=2 = 空行,count=3 = 单像素窄行
   ——CAFNCB f048 行块 `03 00 <px>` 实证;引擎行推进只按 count 无下
   限,blowfish.cpp:296-314;研究 §1.3-B)。count=0/1(块头 2 字节都
   不齐)仍 Err。模块文档注释同步(行块下限语义更新)。
5. **RLE 链区间改 `[offset, 文件尾)`**:现 `ends[]` 数组(下帧更大
   偏移为界,shp_ts.rs:116-127)整体删除;引擎无 region 界(shapipe.
   cpp:103-112 GetPixels 直给指针;研究 §2.2/§2.4-4)。EOF 触碰仍
   Err。语料实证:修复版全集 0 败、rows_over_width=0,文件尾为界足够
   (研究 §2.4-4;原"下帧偏移为界"是 M1b 探针期安全假设,证伪)。
6. **维持不动**:空帧约定(cx==cy==offset==0 判空先于 flags 分派——空
   帧可带非 0 flags)、帧矩形画布界、偏移越界/表前偏移检查、头 zero
   验零(语料 0 违例,研究 §1.2/§9-L2)、MAX_SHP_CANVAS_PIXELS 分配
   前上界、帧数 1..=10000。

## 2. VEIN 前缀守卫(研究 §4.5)

`crates/core/src/render/overlay.rs` paint_overlays:表项类型名以
"VEIN" 开头 → 不画、计 `overlay_drawn`(链路成功、像素面空——与
BlitResult::Outside 计 drawn 同口径;引擎 VEINHOLE/VEINHOLE_DUMMY 直接
return,cell.cpp:1401-1403)。零售 [OverlayTypes] 唯一 VEIN 名 = 表位
178 VEINHOLEDUMMY,**语料命中 0 格**(研究 §4.5)——纯语义对齐,不
改 807 总账。rubble 分支语料 0(零售 Rubble 0 条),只记本文档不实现。

## 3. 物件 z 侧账计数位(研究 §4.7)

`crates/core/src/render/mod.rs` SceneStats 增字段 `objects_z_fallback:
usize`(信息位区,不入守恒恒等式);`crates/core/src/render/objects.rs:81`
`map.tiles.cell(wx, wy).map_or(0u8, |t| t.z)` 的 None 分支计数。
语料量级 9288 件/807 图(研究 §4.7;z 缺席记 0 的诊断面)。overlay 无
此面(其 z 来自记录本体)。

## 4. 阴影帧号规则旗标化(研究 §4.1,零视觉变化)

`crates/core/src/render/shape.rs` 增纯函数:

```rust
/// 阴影半帧号惯例:SHP 后半 = 阴影半,阴影帧 = 本体帧 + 帧数/2
/// (cell.cpp:1393 Draw_Overlay_Shadow `frame + Get_Count()/2`;
/// terrain.cpp:282-283 同式;跨类型惯例 builtype.cpp:771-802)。
/// M4c 只钉语义不接线(变暗 DARKEN 表未钉顺延,L7)。
pub(crate) fn shadow_frame_index(frame: usize, frame_count: usize) -> Option<usize>
```

`frame + frame_count/2 < frame_count` 时 Some,否则 None。配单测(含
奇偶帧数与越界 None)。渲染路径**不调用**(旗标化 = 语义钉定 + 为阴影
批铺接口);Preview 直方图断言不动(差率监督面)。

## 5. 伴随改动(必然波及,实现须同步)

1. **测试夹具 flags=3 → 1(4 处,否则既有测试全崩)**:判别式改后
   bit1=1 走 RLE,均匀填充帧的严格 RLE 链必败(首块 count=257·fill
   越界)→ 现行以 flags=3 构造 RAW 帧的夹具全部改为 flags=1(bit1=0
   → RAW 直取),注释同步:
   - `crates/core/src/render/overlay.rs` shp_raw(~:185);
   - `crates/core/src/render/objects.rs` shp_frames(~:217);
   - `crates/core/src/render/scene.rs` shp_frames(~:147);
   - `tools/ra2-map/tests/cli.rs` shp_frames(~:620)。
2. **shp_ts.rs 既有判别式测试三处重述**:
   - `raw_fallback_when_rle_fails_and_span_exact` → 改名/改注:flags=1
     → bit1 门控直取 RAW(不再经"RLE 失败"路径);
   - `rle_wins_when_span_equals_need` → flags=3 → RLE 语义维持(注释
     更新:bit1 门控,非"RLE 优先试探");
   - `err_when_rle_fails_and_span_mismatch` → RLE 截断 EOF 仍 Err
     (链界文件尾,数据不足语义不变)。
3. **rle_zero_ts.rs 测试**:`rejects_count_below_four` 改边界(count=2
   合法空行/count=0 仍 Err);新增 count=2 空行、count=3 单像素行。
4. **theater_tmp.rs 面**:RLEZeroTS 为 SHP/TMP 共用,下限放宽对 TMP
   同生效——语料 TMP 全集 M4a 起全过(旧下限未拒任何零售瓦片),放宽
   只可能把假 Err 转合法解码,零回归风险;监督 = M4a/M4b 既有真资产
   测试保持全绿 + m4c 验收 terrain 桶与 M4b 基线对账(§6.5)。
5. 模块文档注释(shp_ts.rs 头部判别式描述、rle_zero_ts.rs 行块下限)
   与研究勘误口径一致(研究 §出处"勘误"清单)。

## 6. 验收(研究 §5)

### 6.1 新增 `crates/core/tests/m4c_acceptance.rs`(照 m4b 模式,全部 ignore,需 RA2_ASSETS_DIR;M4b 验收文件原样保留作回归绊线)

主测试 m4c_all_maps_scene_render:807 全量循环(m4b 同构:零 panic +
守恒三恒等逐图 + 画布公式独立重算 + 预算 + 真彩抽样 PPM + Preview 直
方图),断言:

1. **主断言**:`overlay_fail == 0 && objects_fail == 0`(807 全量);
2. **总量 oracle**(语料冻结前提,研究 §5.1):`overlay_drawn ==
   264211`、`objects_drawn == 514502`、`objects_missing_art == 3539`
   (负 oracle,防改名蒙混)、`objects_z_fallback == 9288`、
   `objects_skipped_voxel == 8320`、`objects_malformed == 1`、
   `overlay_no_type == 0 && overlay_missing_file == 0`;
3. **守恒三恒等维持**(地形七桶/非空 overlay 四桶/非 Smudge 物件五桶,
   m4b 同式逐图独立重数);
4. **预算** `ms < 10578` 维持(研究 §5.5:解码增量 µs-ms 级,少一次
   RAW 兜底试探反而略省);
5. **Preview 维持口径**:`preview_probed == 507`、最众组合 == "RGB"、
   占比 ≥ 0.85 ——**断言照 M4b 原样维持不收紧不放宽**;研究预测 85.6%
   基本不动(§5.2-4)。若实测跌破 85% = 真发现,停下带数回设计,禁
   止悄悄放宽;
6. **terrain 层对账**(监督,不新增硬断言):terrain 七桶/missing_file
   == 0 与 M4b 基线(task-8-assets-run2.log)逐桶对账打印,异常即停
   ——解析器改动不得波及地形层。

### 6.2 合成字节级 oracle(shp_ts.rs/rle_zero_ts.rs 单元测试;模式复刻研究 §5.3,绝不 commit EA 字节)

- a. **RAW+尾 pad(DRUM01 模式)**:11×14 flags=0x01,span=160 >
  need=154,尾部 6B pad → RAW 解出 154B,pad 不入帧;同型 flags=0x00
  (GEM09 2×2 span 8/need 4);
- b. **同文件混变体(CIV1 模式)**:f0 flags=0x03 RLE 行链,f1
  flags=0x01 RAW——帧级门控钉死;
- c. **窄行 RLE(CAFNCB f048 模式)**:1×1 flags=0x02,行块
  `03 00 <px>` → 单像素;
- d. **判定面**:flags=0x04 → RAW;flags=0xCCCC0003 → 只读 +8 字节按
  RLE;flags=0xCCCC0001 → RAW;空帧 flags≠0 仍按空帧约定跳过;
- e. **逐帧像素期望**:RAW 帧输出 = offset 起 need 字节(pad 不入帧);
  count=2 空行 → 整行透明;count=0/1 → Err;
- f. **敌意面**:RAW 谎报 need 超文件尾 → 补零截断零 panic(新防御);
  RLE 链越文件尾 → UnexpectedEof;表前偏移/画布越界/验零维持。

### 6.3 VEIN/z 侧账/阴影旗标单测

- VEIN 前缀表项 → overlay_drawn+1 且画布零写入(合成 mini 图);
- objects_z_fallback:z 缺席格物件计数、z 在格不计数(合成 mini 图);
- shadow_frame_index:偶数帧数(0→n/2)、奇数帧数(floor)、越界 None。

### 6.4 门禁

`cargo fmt --all --check` 净;`cargo clippy --all-targets -- -D
warnings` 净;`cargo test --workspace` 全绿 0 fail(合成计数较 272 增
长,精确数由实现上报、任务审查核验后钉台账);真资产 `--ignored`
39 → 40(m4c_acceptance 新增 1)/0 fail。

## 7. 顺延清单(台账记账,本批不做)

remap(LightConvert 16 槽链未钉)/坡地 ore(0.44%)/SHAPE_DARKEN 变暗
表/−2px+SHAPE_ALPHA/ZWRITE(定性 RGB 零影响)/rubble(语料 0)/别名
表(无证据)/voxel 渲染器(M5)。

## 8. 低置信与监督(研究 §9)

- **L1** RLEBlitter::Blit 行展开体未反编译 → 零游程钳制/行补齐维持
  XCC 语义(M1b 钉);监督 = 修复版全帧 rows_over_width=0 + 全集 0 败
  (研究已实证,验收断言固化);
- **L4** ShapeFlags 枚举数值不在库 → 只用 convert.cpp 分派位语义
  (bit1),不猜数值;字面量 0x4601 等不可分解,注释如引用须带此注;
- **L5** SROCK/TROCK 跨剧场副本 2595 格差异:修复后两链皆零败、差异
  消失(研究 §5.4);Preview 差率异常时首查副本选择序;
- **L6** missing_art 95 名别名:无证据不做;
- **L2** 头验零维持(引擎不验、语料 0 违例;若未来语料出现
  header_zero 类再裁)。

## 9. 出处

- 引擎语义:E:\hongjing\ra2-decomp(只读)file:line 见研究 §2/§4
  (msanim.h/shapipe.cpp/draw.cpp/blowfish.cpp/cell.cpp/terrain.cpp/
  techno.cpp/scheme.cpp);语义重写不转录,出处随行注释。
- 语料实证:E:\hongjing\Reunion 2023(只读)807 图 + 全 mix 链;探针
  报告在 %TEMP%\m4c_research\(不入库)。
- 既有钉定:M1b 2026-11-03-shp-tmp-layouts.md(RLEZeroTS 行格式/空帧
  约定)、M4b spec/研究(overlay/物件链、SceneStats、验收口径);
  本文对其的勘误(count≥4 下限、span==need RAW 兜底、flags u32 读法)
  以研究 §出处 勘误清单为准。
