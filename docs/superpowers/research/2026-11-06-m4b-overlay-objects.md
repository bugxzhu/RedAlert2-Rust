# M4b overlay 层 + 物件 SHP 层 研究文档(decomp 实钉 + 真图实证)

日期:2026-11-06。上游:main = c87b7b0(M4a 已并)。分支:feat/m4b-overlay-objects。

## 0. 信源与政策

- **E:\hongjing\ra2-decomp**(只读,gamemd 1.001 字节匹配反编译,GPL;18,803/19,362 函数一致):
  实现期引用其 file:line 与短引文作引擎语义依据;ra2-rs 侧一律**语义重写**并注出处,
  绝不逐字转录。仓库本身绝不动。
- **真图实证**:用户资产目录 807 语料直接抽段(austintx.map / c1a02md.map 等),
  字段布局以真数据为准、decomp 加载代码佐证。
- 探针:RA2_ASSETS_DIR 全量跑(807),Preview 四组合对拍(M4a 遗留,走向 B 顺延至此)。

## 1. 层序与物件排序(Q1 实钉)

- 顶层分发器 `Tactical::Render` **未在库内反编译**(DrawPassType 枚举在
  include\tactical.h:20-24);帧级三段:gscreen.cpp:242-245
  `DRAW_PASS_PAN → DRAW_PASS_BACKGROUND → Draw_It(complete) → DRAW_PASS_FOREGROUND`。
- **物件统一通道 = Tactical::Draw_Objects(0x006D8DB0)**,tactical.cpp:1614-1616:
  `for (int layer = LAYER_FIRST; layer < LAYER_COUNT; layer++) { for (int index = 0; …)`
  层内按数组序;仅 LAYER_GROUND 排序(display.cpp:528-531 Submit → layer.cpp:79
  Sorted_Add 在首个 `> object` 前插入 = **Sort_Y 升序稳定插入**)。
- **排序键** object.cpp:1211-1213 `return((Sort_Y() > object.Sort_Y()) …)` +
  object.cpp:1482-1485 `Sort_Y = Render_Coord().Y + Render_Coord().X`(lepton 和,
  即 M4a 的 d 键同构)。Render_Coord:默认 = Center_Coord(object.h:157 /
  object.cpp:1298-1301 = PositionCoord);**建筑例外** building.cpp:7485-7488
  `PositionCoord - Coord(CELL_LEPTON/2, CELL_LEPTON/2, 0)` = **足迹 NW 角**。
  → 排序键换算:树/步兵 = 256(X+Y)+256;建筑 = 256(X+Y)(X,Y 为 0 基锚格)。
- 层归属:object.cpp:297-306 默认不 In_Air → LAYER_GROUND;静态图载具/飞机
  HeightAGL=0 亦 LAYER_GROUND(fly.cpp:1006)。LayerType 枚举数值未在库内
  (M4b 只用 ground 层,无需数值)。
- **裁决**:tactical.cpp:1717 Draw_Terrain **倒序** / :1740 Draw_Buildings 正序
  亦存在,但与 Draw_Objects(正序全层迭代)并存;判定 1717/1740 为
  编辑器/特殊路径,**in-game 以 Draw_Objects 正序为准**(与 M4a d 升序画家序
  连续自洽)。若 Preview 差率系统性异常,此裁决为首要复查点。

## 2. overlay 链(Q2 实钉)

- **索引语义**:OverlayPack 字节 = **rulesmd.ini `[OverlayTypes]` 顺序表位置**
  (rules.cpp:2068-2079 依序 Find_Or_Make;overtype.cpp:97-98
  `HeapID = OverlayTypes.ID(this)` = 追加位)。非 [Overlays] ID 数字——
  控制器预设纠错。0xFF(OVERLAY_NONE)= 空。
- **帧号**:OverlayDataPack 字节 = **SHP 帧号直传**(cell.cpp Draw_Overlay
  0x0047F6A0:1457/1459/1462/1465 墙/矿/藤/通用一律 `Draw_Shape(…, OverlayData, …)`)。
- **特判**:板条箱(IsCrate)帧 0(cell.cpp:1381-1393);桥面板帧 =
  OverlayData + 逐格变体表(cell.cpp:1419-1428,实现期直读);阴影帧 =
  帧 + Get_Count()/2(cell.cpp:1381-1393/terrain.cpp:283)→ **M4c 顺延,不做**。
- **SHP 名**:objtype.cpp:121 GraphicName 默认 IniName;`Image=` 覆盖
  (objtype.cpp:730-731);装载 overtype.cpp:236 `_makepath(…, GraphicName, ".SHP")`,
  剧场后缀变体(overtype.cpp:179/186,后缀规则实现期钉:通用名优先,
  缺档再试剧场后缀)。
- OverlayLayers 双层网格语义(M2 已裁决)不变:type_at 0xFF=空,
  frame_at 空档→None。菱形坐标直用(x,y ∈ 1..=W+H−1 < 512 恒在网格内)。

## 3. 锚定(Q4 实钉;画布绝对坐标)

统一:0 基物件坐标 (X,Y) ↔ M4a 记录坐标 (x,y)=(X+1,Y+1);z = 该格瓦片 z
(记录缺席 → z=0 并计 parse_fail 侧账);oy = 15·z_max(M4a 画布纵偏移)。

- 格左上 TL = (30·(X−Y), oy + 15·(X+Y) + 30 − 15z);格中心 C = TL + (30,15)。
- **overlay**:C 居中(overtype.cpp:167 `Top_Left() + point + Point2D(30,15)` +
  SHAPE_CENTER;60×30 帧恰好覆格 = 居中语义自洽证)。
- **树**:C 居中再 **−12px**(terrain.cpp:281 `zadjust - 12`;Height=0 静态)。
- **步兵**:C 居中,zadj **−10**(infantry.cpp:495);**子格 0..4 像素偏移待钉**
  (候选社区值 (0,0)/(−15,−8)/(15,−8)/(−15,7)/(15,7),实现期以 decomp
  InfantryClass 坐标偏移或 Preview 差率择优钉)。
- **建筑**:锚 = **TL**(NW 角,building.cpp:7485-7488 + Editor_Draw_It
  building.cpp:653-654 `height = drawpoint.Y - shapeheight/2` 垂直对中);
  in-game Draw_It 未反编译 → 低置信项,Preview 差率监督(见 §9)。
- SHAPE_CENTER = 水平居中(+由 overlay 60×30 闭包佐证的垂直居中语义)。

## 4. 朝向与帧(Q3 实钉)

- **步兵** infantry.cpp:441-471 Shape_Number:
  `stage % Count + HumanShape[dir32] * Jump + DoControls.Frame`;
  HumanShape 32 项表值**未在库内**(include\infantry.h:171 仅声明)→
  以等价公式 `facing_idx = (dir32·Facings + 16) >> 5` 语义重写(四舍五入),
  实现期以真 SHP 帧数/Preview 差率复核。idle 站立帧块(StandFrame 等)
  从 artmd/art 序列表钉;钉不住 → 帧 0 计数回退(lenient)。
- **建筑**:无朝向项(building.cpp:839-841 仅动画/门/占用逻辑,静态帧 0;
  炮塔帧 building.cpp:805-813 → M4c)。
- **树**:terrain.cpp:243-253 帧取 Strength/.Fetch_Stage(),静态图固定档;
  实现期以真 SHP 帧数钉(缺据 → 帧 0 计数回退)。
- **载具/飞机 = voxel 非 SHP**:fly.cpp:840 `mtx.Rotate_Z(…As_Radian32())`、
  unit.cpp:1921 `key = (key << 5) | (frame & 0x1f)`、aircraft.cpp:293/341 同型
  (5 位朝向缓存键)→ 朝向是 3D 旋转非帧序。**用户裁决 2026-11-06:
  M4b 跳过 [Units]/[Aircraft] 计数,voxel 渲染器另立 M4c**。

## 5. 真图五类字段实证(austintx.map / c1a02md.map,2026-11-06)

- `[Structures]` `ID=Owner,Type,Health,X,Y,Facing,Tag,…`(取 [0..5]);
- `[Units]`/`[Aircraft]` `ID=Owner,Type,Health,X,Y,Facing,Mission,Tag,…`
  (c1a02md Aircraft 同构实证);
- `[Infantry]` `ID=Owner,Type,Health,X,Y,`**`Subcell`**`,Facing,Mission,Tag,…`
  (austintx COW 行 [5]=0..4、[6]=32/64/158/186 → 子格在前、朝向 0..255);
- `[Terrain]` `KeyY1000X=TreeName`(**键 = Y·1000+X,M2 裁定,objects.rs:73
  `n % 1000 → x, n / 1000 → y` 已实现**;37084 → x=84,y=37;控制器预写
  X·1000+Y 系笔误,此行即勘误);
- `[Smudge]` → M4c(样本多空,austintx 无)。
- decomp 加载侧佐证:overlay.cpp:418-425([OverlayPack] 逐字节→classid)、
  overlay.cpp:457-461([OverlayDataPack] 逐字节→OverlayData)。
- 坐标基:0 基矩形;(X,Y) → 记录 (X+1,Y+1)(M2 §3.6 存储 (x,y) 权威 +
  M4a 投影闭合)。+1 映射以验收(clip 率 + Preview 差率)监督。

## 6. SHP-TS blit 面(M1b 既有)

- 帧 = 索引像素 + 子矩形头 {x,y,cx,cy}(shp_ts.rs 24B 夗头);
  **像素值 0 = 透明跳过**(RLEZeroTS 零链语义);帧号 ≥ frames.len() →
  钳末帧计数(lenient);空帧约定 cx==cy==data_offset==0。
- 调色板:剧场 iso 调色板直查(无 remap;建筑/步兵 SHP 的所属方色槽
  按 iso 表原样渲 → 差率报告监督,M4c remap 修正)。

## 7. rules/art INI 可达性

TheaterAssets::open 根链含 ra2md.mix(pos 100)/ra2.mix(101)+ expandmd/
cache 顺序链(theater.rs:360-375)→ `lookup("rulesmd.ini")/("artmd.ini")/
("art.ini")` 直达。art 分层:artmd 优先、art.ini 兜底(YR artmd 仅覆盖差量,
引擎同为分层合并)。

## 8. Preview 终验设计(用户裁决 2026-11-06)

重跑 807 四组合探针(M4a probe_preview_diff):**通道/行序以多数裁定钉断言**
——最优组合稳定占比 ≥85%(真彩图口径)才钉;否则记"未稳定"仅报告。
差率分布**只报告不设阈值**(remap/阴影/voxel 缺席,阈值会钉错基线)。

## 9. 低置信/待钉清单(实现期任务内闭环)

| # | 项 | 处置 |
|---|---|---|
| L1 | 建筑 in-game Draw_It 未反编译 | Editor_Draw_It 锚定落地,差率监督 |
| L2 | HumanShape 表值缺 | 等价公式 + 帧数复核 |
| L3 | 步兵子格偏移 | decomp 钉或差率择优 |
| L4 | 剧场后缀变体规则 | overtype.cpp:179/186 直读 |
| L5 | 步兵 idle 帧块 / 树静态帧 | 真资产帧数钉,兜底帧 0 计数 |
| L6 | Draw_Terrain 倒序路径(§1 裁决) | 差率异常时首查 |

## 出处

- decomp 引文均出自 E:\hongjing\ra2-decomp(只读)上列 file:line;语义重写政策
  见研究 2026-11-05-theater-tile-mapping.md §0/§9.1。
- 真图实证:E:\hongjing\Reunion 2023(只读)Maps/Standard/austintx.map、
  Maps/Cooperative/c1a02md.map。
- M4a 基线:docs/superpowers/specs/2026-11-06-m4a-terrain-render-design.md +
  research/2026-11-06-m4a-terrain-render.md(C1 修复后版)。
