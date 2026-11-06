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
- 807 图全量:两轮全跑 269.08s / 266.33s(纯渲染累计 202,285ms);最慢单图
  1084ms = Maps\MadHQ\被遗忘者.map;missing_file 总计 0(引用性)。
- 真彩三剧场样本 PPM(%TEMP%\ra2-m4a-sample-*.ppm):c1a01md(TEMPERATE)、
  c1a03md(SNOW)、城郊之战(URBAN)。
- 画布尺寸公式断言:807/807 全过,零异常(注:H 公式的 z_max 由 H 反推,
  该断言近同义反复——W 为独立钉;M4b 改由 tiles 的 max z 独立钉,已录
  终审台账)。
- 剧场权威分布(MapFile::parse 口径,807):Temperate 324 / Snow 106 /
  Urban 78 / Desert 170 / NewUrban 128 / Lunar 1;正则 ^Theater= 行口径
  仅 531(212/106/78/73/61/1)——RMG 模板(MapUnits 267)无 Theater 头,
  以 parse 为准。

## 4. Preview 对拍裁定(T4 Step 3 结论回填)
- 走向 B:对拍顺延 M4b 终验(spec §6.4 勘误已入 9c4f84f)。数据:507 张
  真彩带预览图(508 张中 1 张无内嵌预览),最优组合差率 <10% 为 0、
  <50% 仅 3、中位 91.3%(p25 89.0 / p75 93.5)、最小 39.3%(xbreak.map,
  RGB+flip,5881/14960)、最大 100%(两张 RMG indicator.map);最优组合
  分布不集中:RGB 207 / RGB+flip 242 / BGR 28 / BGR+flip 30。
- 量级注记:中位 91% 超出"预览仅多矿石/物件 overlay"所能解释的范围,
  M4b 须复核视口原点/缩放/裁边语义(tactical.cpp 449/607 的 (−30,−15)
  半格居中是否应进画布公式)。

## 5. 性能预算
- 每图断言 `ms < 3500`(实测 max 1084ms ×3 ≈ 3252 取整 3500;spec 硬帽
  10s 内;m4a_acceptance.rs 已入断言)。全量套件本地实测 ~4.5 分钟。

## 出处
- E:\hongjing\ra2-decomp(只读):tactical.cpp / geometry.h / cell.cpp /
  display.cpp 地址锚如上;语义重写,无拷码。
- 探针:%TEMP%\ra2-m4a-sample-*.ppm;验收测试 crates/core/tests/m4a_acceptance.rs。
