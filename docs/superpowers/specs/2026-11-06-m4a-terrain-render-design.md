# M4a 地形整图渲染 设计规格

日期:2026-11-06
状态:设计已获用户批准(AskUserQuestion 三项:方向=整图等距渲染出图 headless/
层范围=全量三层但切 M4a+M4b/验收=纯探针锚点(不依赖用户截图);方案 A
(`core::render` 合成器+瘦 CLI)与 §1-§6 分节设计 2026-11-06 复核"ok")
上游:main = 98a0a9f(本地领先 origin/main 2 提交,待用户推送);引擎取证源 =
E:\hongjing\ra2-decomp(只读,gamemd 1.001 字节匹配反编译,GPL)

## 1. 背景与目标

M2 的 `MapFile.tiles`(IsoGrid,每格 `tile: i16 / sub_tile: u8 / z: u8`)+
M3 的瓦片资产链(`TheaterIni` 索引表 → 文件名 → `TheaterAssets` 跨容器查找 →
TS TMP 槽 → 调色板)已经闭环到**单瓦片 PNG**(`extract-tile`)。本里程碑交付
**整图组装**:任一 .map + 资产目录 → 整幅等距地形 PNG(headless CPU,零依赖,
无 GPU),兑现总设计 §6.2"渲染帧快照对拍"的地形半场。

M4 总方向 = 全量三层渲染(用户 2026-11-06 拍板),切分为:
- **M4a(本规格)**:地形层整图 + 高度 z 接缝 + Preview/锚点对拍;
- **M4b(另立 spec)**:overlay 层 + 物件 SHP 层(帧选择/阴影/remap),验收走
  纯探针锚点(用户拍板,不依赖用户供原版截图)。

## 2. 决策记录(用户 2026-11-06 拍板)

| # | 决策 | 备选否决理由 |
|---|---|---|
| 1 | M4 方向 = 整图等距渲染出图(headless) | rules/INI 与 smudge/overlay 支线让位;wgpu 交互视口 M5+ |
| 2 | 层范围 = 全量三层,切 M4a(地形)→M4b(overlay+物件) | 一次做到底超出单 spec 可验证粒度 |
| 3 | M4b 验收 = 纯探针锚点(引擎源码语义+锚点像素) | 不要求用户跑原版截黄金图 |
| 4 | 架构 = 方案 A:`crates/core/src/render/` 纯合成器 + 瘦 CLI | 方案 B(全留工具)使 807 图对拍与真资产夹具基建(core/tests)绕路 |

## 3. 范围 / 非范围

**做**:
- `core::render` 无头合成器:投影、逐格瓦片反查(M3 链复用)、高度 z 绘制
  偏移、缺槽统计、整幅 RGB 缓冲;
- `ra2-map render` 子命令(纯 I/O 壳:读图/资产 → PNG 写盘);
- `paint_canvas` 自 `tools/ra2-map/src/main.rs` 上移 `core::render`(行为
  不变,M3 单瓦片路径与整图共用);
- 敌意画布上界 `MAX_RENDER_PIXELS`。

**不做**(边界):
- overlay/物件层及其素材链(→M4b);wgpu/视口/交互(→M5+);
- z 着色伪彩可视化、terrain/ramp 语义值表复核(M3 边界延续);
- 地图写回;PNG 以外输出格式;多线程渲染(单线程预算内不做)。

## 4. 模块结构(`crates/core/src/render/`)

```
crates/core/src/render/mod.rs     // 门面 + RenderStats + MAX_RENDER_PIXELS
crates/core/src/render/terrain.rs // render_terrain 合成器 + paint_canvas(自工具上移)
tools/ra2-map/src/main.rs         // render 子命令(I/O 壳)+ extract-tile 改调 core
```

- 合成器签名(形状在计划期可微调):
  ```rust
  pub struct TerrainRender {
      pub width: u32, pub height: u32,
      pub rgb: Vec<u8>,          // 行紧排 RGB,3·w·h
      pub stats: RenderStats,    // cells/missing_tiles/canvas_px 等
  }
  pub fn render_terrain(
      map: &MapFile, assets: &TheaterAssets, ini: &TheaterIni, palette: &Palette,
  ) -> Result<TerrainRender, FormatError>;
  ```
- **层遍历共享画布**结构(M4b 钩子):画布 + 逐层绘制函数,`render_terrain`
  是第一层;M4b 加 overlay/SHP 层不改地基。
- 投影整数式(研究任务①钉死前为预期式):格 (x,y) 左上角像素
  `sx=(x−y)·30, sy=(x+y)·15`,画布 `W=30·(w+h), H=15·(w+h)+z_max 偏移余量`;
  半格居中偏移 (−30,−15) 语义按 decomp 锚点定。
- **高度 z 参与本里程碑**(悬崖接缝正确性必需):`z → 绘制 y 偏移` 公式由
  研究任务①b 自 decomp `Draw_Tile`/`LogicCell::Draw` 取证(预期每级 −15px
  量级,以源码为准)。
- 绘制序:`x+y` 升序行对角线序(研究任务①确认悬崖/遮挡序)。

## 5. 错误与零 panic 策略

- 敌意 `[Map]Size`(上界 4096×4096)推得画布超 `MAX_RENDER_PIXELS`
  (拟 2²⁷ = 134,217,728 px;语料最大 272+158=430 → 约 83.6 Mpx,余量 1.6×;
  4096² 敌意图约 30 Gpx 拒绝)→ `FormatError::Invalid`,不分配;
- 瓦片链任一环缺(INI 无集/文件未出货/槽空)→ 该格画调色板 0 号色 +
  `RenderStats` 计数,**不中止整图**(M3"未出货"语义延续);
- TMP 解析失败(敌意字节)→ 同上,按格吞错计数;零 panic 契约(807 图
  含敌意变异)不变。

## 6. 验收标准(完成定义)

1. 807 图全量 `render_terrain` 零 panic(ignored 真资产套件,性能预算见
   研究任务③);
2. 逐剧场抽样图各 ≥1 张出 PNG(六剧场全覆盖)+ 尺寸公式钉死断言;
3. 投影/接缝/z 偏移锚点:角格/中心格/已知悬崖格像素坐标合成测试钉死
   (对 decomp 取证式);
4. Preview 对拍按研究任务②裁定:内嵌预览与纯地形差异可忽略 → 采样图
   像素差率阈值入回归;差异系统性(预览含矿石等 overlay)→ M4a 以锚点
   验收,Preview 对拍顺延 M4b 终验(spec 勘误记录);
5. 缺槽统计与 M3 缺档账(NEWURBAN 383 等)一致或给出差异解释。

## 7. CLI 契约(`tools/ra2-map`,退出码 0/1/2 契约不变)

```
ra2-map render <map> -o <out.png> [--assets <dir>]   # 0 成功 / 1 失败 / 2 用法错
```
- stderr 概要:画布尺寸、格数、缺槽计数、耗时;缺槽非零不置败(诊断态)。
- `extract-tile` 改调 `core::render::paint_canvas`(输出字节不变)。

## 8. 测试策略

- 合成(快):投影锚点、交错接缝(相邻格共享半菱形边)、z 偏移、缺槽统计、
  tiny 夹具 mix(复用 theater.rs 测试的 `make_mix` 手法);
- 真资产 ignored:807 全量零 panic + 抽样 PNG(六剧场)+ 尺寸/统计钉死;
- 上移回归:`paint_canvas` 8×4 字节级钉与 60×30 阶梯钉原样随迁。

## 9. 前置研究任务(计划期 Task 1,产出研究文档后实现落地)

1. **投影与绘制序**:tactical.cpp:188-195 `CoordToPixelMatrix`
   (Rotate_X 60°·Rotate_Z 45°·Scale 60/CELL_LEPTON_DIAG)与 227+
   `Coord_To_Pixel_Absolute` 整数化;449/607 (−30,−15) 半格语义;
   ①b:`z → y 偏移`(Draw_Tile/LogicCell::Draw)与行对角线绘制序;
2. **Preview 差异面**:抽样各剧场 ≥3 图,内嵌 [Preview] vs 纯地形渲染
   差异(裁边/量化/是否含 overlay)→ 裁定 §6.4 走向;
3. **性能预算**:最大图(2+1=3.map,272×158)与 807 全量耗时实测,
   定 ignored 套件预算与护栏值校准。

## 10. 里程碑定位

M4a = M2(地图数据)+M3(瓦片链)的组装闭环,view 侧第一层;M4b =
overlay/物件 SHP 语义(探针锚点验收);M5+ 候选 = wgpu 交互视口。
M2/M3 缓期清单不动。
