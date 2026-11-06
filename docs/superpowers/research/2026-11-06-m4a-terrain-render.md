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
- C1 域改勘误(2026-11-06 终审):格域 = 研究文档 2026-11-04 §3.2 菱形走位,
  画布由走位极值决定——回放 (87,71) → d∈[88,229](跨度 2H−1)、x−y∈±(W−1);
  W_px = 30·跨度+60 = 5220、H_px = 15·(2H−1)+30+15·z_max(c1a01md 实测
  5220×2250,z_max=7)。控制器"span 2H−2"散文系笔误,以回放为准(已录终审
  台账);上行 w×h 矩形画布式为 C1 前旧口径,仅存档。
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
- 结论:真彩抽样 = TEMPERATE/SNOW/URBAN;desert/newurban/lunar(135 图,
  正则行口径;parse 权威口径 299 = 170+128+1)哑调色板零 panic 覆盖,
  Preview 对拍不参与。

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

## 4. Preview 对拍裁定(2026-11-06 重裁,C1 域改后两代数据在案)
- 走向 B 维持:对拍顺延 M4b 终验(spec §6.4 勘误已入 9c4f84f)。修复前
  (T4,矩形域欠绘 ~80% 地形):507 张真彩带预览图(508 张中 1 张无内嵌
  预览)最优组合差率 <10% 为 0、<50% 仅 3、中位 91.3%(p25 89.0 / p75 93.5)、
  最小 39.3%(xbreak.map,RGB+flip,5881/14960)、最大 100%(两张 RMG
  indicator.map);组合不集中(RGB 207 / RGB+flip 242 / BGR 28 / BGR+flip 30)
  ——该轮中位 91.3% 系 C1 域缺陷主导,勘误轨迹保留。
- 修复后(C1 菱形域,同 507 图口径):最小 6.3%、p25 26.1、中位 39.2%、
  p75 58.2、最大 100%(indicator.map ×2);<10% ×5、<50% ×318;最优组合
  显著收敛:RGB 435 / RGB+flip 65 / BGR 4 / BGR+flip 3(435/507 = 86%)。
- 新证据注记:M2 未裁决的预览通道/行序问题首获强证据——倾向 RGB 行主序
  不翻转,M4b 终验时裁定。残差归因候选 = M4b 未渲染层(矿石/物件/overlay)
  + 视口几何(tactical.cpp 449/607 的 (−30,−15) 半格居中是否应进画布公式);
  旧"视口语义"假设经 C1 证实部分成立——域缺陷是主因,视口几何仍是候选。
  中位 39.2% 仍 ≥10% 阈值,故走向 B 维持。

## 5. 性能预算
- 每图断言 `ms < 9000`(C1 菱形域实测 max 2995ms = 被遗忘者.map ×3 ≈ 8985
  取整 9000;计划硬帽 10s 内;m4a_acceptance.rs 已入断言。勘误注记:修复前
  1084ms/3500 预算系矩形域欠绘的假低值,作废)。全量套件实测 ~10.7 分钟
  (640.5s,含抽样与对拍探针)。

## 出处
- E:\hongjing\ra2-decomp(只读):tactical.cpp / geometry.h / cell.cpp /
  display.cpp 地址锚如上;语义重写,无拷码。
- 探针:%TEMP%\ra2-m4a-sample-*.ppm;验收测试 crates/core/tests/m4a_acceptance.rs。
