# M4b 设计:overlay 层 + 物件 SHP 层渲染(场景合成)

状态:设计 2026-11-06 用户逐节"ok"+ voxel/终验两裁决拍板;事前研究已实钉
(research/2026-11-06-m4b-overlay-objects.md,decomp 引文 + 真图实证)。
上游:main = c87b7b0(M4a 已并,本地领先 origin ~10 提交待用户推送)。
分支:feat/m4b-overlay-objects。

## 1. 背景与目标

M4a 交付整幅地形;本里程碑在其画布上追加 **overlay 全量层**(矿石/宝石/墙/
桥,OverlayData 帧号直传)与 **物件 SHP 三类**(Terrain 树木 / Structures
建筑 / Infantry 步兵;载具与飞机系 voxel 资产,计数跳过)。产出
`render_scene` 场景合成器 + `ra2-map render` 场景输出,并以 807 全量重跑
完成 M4a 顺延的 **Preview 通道/行序终验裁定**。

## 2. 决策记录(用户 2026-11-06 拍板)

| # | 决策 | 备选否决理由 |
|---|---|---|
| 1 | M4b = overlay 全量 + 物件基础版(remap/阴影/状态帧失控再切 M4c) | 全量一包超单 spec 可验证粒度 |
| 2 | Preview 终验 = 通道/行序多数裁定钉断言 + 差率只报告 | 缺层尚多,硬阈值钉错基线 |
| 3 | 架构 = 方案 A:render::scene pass 追加,不动 M4a 合成器 | 预建 M5 抽象有回炉风险 |
| 4 | [Units]/[Aircraft] = voxel 非 SHP → 计数跳过,M4c 立项 | decomp 实钉(fly.cpp:840 等);最小光栅使 M4b 增重 1.6× |

## 3. 范围 / 非范围

**做**:
- `format::rules`:[OverlayTypes] 顺序表(字节=表位语义,索引语义实钉见研究 §2);
- `format::art`:artmd→art 分层解析(Image=/Facings/序列帧块);
- `format::map::objects_typed`:五类字段类型化(布局实证研究 §5;Units/Aircraft
  类型化但渲染跳过);
- `render::overlay` / `render::objects` / `render::scene`:三 pass 场景合成,
  SHP 名→解析缓存(每图),统计扩桶 + 守恒;
- CLI render 场景化;807 验收 + Preview 终验;
- M4a 终审遗留 N1-N3(敌意上溢侧计 parse_fail / 守卫注释 269² / 研究文档
  371 交叉引用行)。

**不做**(M4c 候选):voxel 渲染器(VXL/HVA);所属方 remap;阴影帧;
建筑受损/升级/开火状态帧;炮塔分层;smudge 层;多线程。

## 4. 模块结构(`crates/core/src/`)

```
format/rules.rs                // OverlayTable:[OverlayTypes] 表位→类型名
format/art.rs                  // ArtIndex:类型名→ImageSpec{文件名,Facings,帧块}
format/map/objects_typed.rs    // TypedObject{class,key,x,y,facing,subcell,…}
render/overlay.rs              // overlay pass(records 菱形序 d 升序)
render/objects.rs              // objects pass(Sort_Y 升序稳定,锚定/帧选择)
render/scene.rs                // render_scene:compose_terrain 复用+两 pass+统计合并
tools/ra2-map/src/main.rs      // render 子命令:rules/art 装载+场景输出
```

- 合成器形状(计划期可微调):
  `render_scene(&MapFile, &TheaterAssets, &TheaterIni, &OverlayTable, &ArtIndex, &Palette) -> Option<SceneRender>`
  `SceneRender { width, height, rgb, stats: SceneStats }`(terrain 7 桶 +
  overlay/物件扩桶,守恒断言)。
- **画布尺寸公式不动**(M4a 807 尺寸断言保持有效);物件/overlay 精灵越界
  **裁剪并计 clip 桶**——引擎对视口同语义,整图渲染等价。

## 5. 关键技术事实(全部实钉,出处=研究 §1-§7)

- **层序**:瓦片(d 升序)→ overlay(逐格 d 升序)→ 物件单 pass
  (LAYER_GROUND,Sort_Y = coord.X+coord.Y **升序稳定**;建筑键=足迹 NW 角
  =256(X+Y),其余=格中心=256(X+Y)+256)。
- **overlay**:字节=[OverlayTypes] 表位;帧=OverlayData 直传;crate 帧特判 0;
  桥面板帧+逐格变体表;SHP 名=Image 覆盖否则类型名(剧场后缀兜底)。
- **锚定**(0 基 (X,Y)→记录 (X+1,Y+1);TL=(30(X−Y), oy+15(X+Y)+30−15z),
  C=TL+(30,15)):overlay=C 居中;树=C−12;步兵=C−10(+子格偏移);
  建筑=TL(水平居中+垂直对中,低置信 L1 差率监督)。
- **帧选择**:步兵 `HumanShape[dir32]·Jump+StandFrame` → 等价公式
  `(dir32·Facings+16)>>5` + idle 帧块(钉不住帧 0 计数);建筑/树固定帧
  (真资产帧数钉,兜底帧 0);透明=像素 0 跳过;帧号越界钳末帧计数。
- **调色板**:剧场 iso 表直查(无 remap,差率监督)。

## 6. 统计与 lenient 契约

扩桶:`overlay_drawn / overlay_no_type(字节≥表长或表缺)/ overlay_missing_file /
objects_drawn / objects_skipped_voxel / objects_missing_art / objects_malformed /
clip`(守恒:总记录 = 各桶和;对拍 M2 objects 计数)。类型化失败/缺节/缺档
一律计数跳过零 panic(敌意输入同 M4a 口径);rules/art INI 缺档 = 全图
overlay/物件层计 no_type/missing_art 后地形照出。

## 7. 验收(spec §7 → 计划任务展开)

1. **合成**(合成夹具):层序争用(z 差诱发,同 M4a T2 手法)/帧号直传/
   Sort_Y 序(建筑 vs 树)/锚点(树 −12、步兵 −10、建筑 TL)/透明跳过/
   clip 计数/等价公式/[OverlayTypes] 表位语义小夹具。
2. **真资产 807**:零 panic;场景耗时预算=实测 max_ms×3(硬帽 15s,
   M4a 9000ms 基础上重定);统计守恒;剧场口径沿用 M4a(真彩三剧场抽样
   PPM + 哑调色板三剧场零 panic)。
3. **Preview 终验**:四组合探针重跑;最优组合稳定占比 ≥85%(真彩口径)
   → 钉断言;<85% → 记"未稳定"仅报告(不硬钉);差率分布逐图报告。
4. **CLI**:render 场景输出契约 + usage 错误路径;N1-N3 落地核验。
5. fmt/clippy(1.99)净,全量合成绿,ignored 套件全绿。

## 8. 风险与低置信

L1 建筑锚定(in-game Draw_It 缺)→ Editor 路径落地 + 差率监督;
L2 HumanShape 等价公式 → 帧数复核;L3 子格偏移 → decomp/差率择优;
L4 剧场后缀 → overtype.cpp 直读;L5 idle/树静态帧 → 真资产钉;
L6 Draw_Terrain 倒序裁决 → 差率异常首查。全部清单见研究 §9,
实现任务内闭环,结论回填研究文档。

## 9. 出处

研究文档(本日)= 事前实钉全录;M4a spec/research = 画布/投影/预算基线;
ra2-decomp(只读)file:line 引文见研究 §1-§4;真图字段实证见研究 §5。
