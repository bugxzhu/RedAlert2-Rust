# M2 地图/地形层 设计规格

日期:2026-11-04
状态:设计已获用户批准(逐节口头确认 + 总体 ok)
上游:M0+M1a(3b827d2)+ M1b(a3423df)已并入 main;CI 已在 https://github.com/bugxzhu/RedAlert2-Rust 真实执行成功

## 1. 背景与目标

RA2/YR 地图(`.map`/`.yrm`)是 INI 文本与二进制包段的混合格式。M1b 已交付全部底层格式件
(IniDocument、LCW、MIX 摘要位、TMP/SHP/PAL/CSF/AUD/VXL/HVA),本里程碑把它们组装成
地图解析层:任意用户自有地图 → 可查询数据模型 `MapFile`,零 panic、零第三方依赖,
807 张真实地图(用户资产目录 `Maps/` 下官方战役 + 社区图,含中文图名)全量验证。

## 2. 决策记录(用户 2026-11-04 拍板)

| 决策点 | 结论 |
|---|---|
| 完成边界 | 解析层完整:四个包段全部解码 + 对象段结构化 + 元数据;触发器等任务逻辑段仅 tokenize 存储 |
| 验收高度 | 代表性样本字节级甲骨文 + 807 张全量解析零 panic + 敌意变异样本 + 内存预检加固(终审顺延项落地) |
| 交付形态 | 解析库 + `tools/ra2-map` CLI(info / extract-tile) |
| 架构 | 方案 A:`format/map/` 模块族,复用 IniDocument/lcw_decode,PNG 编码器纯 std 写在工具侧 |

## 3. 范围 / 非范围

**做**:
- `[Basic]`/`[Map]`/`[Lighting]`/`[Header]` 等元数据段结构化读取
- `[IsoMapPack5]` → 等距瓦片网格;`[OverlayPack]`+`[OverlayDataPack]` → 双层数组;
  `[PreviewPack]` → 预览图数据(内部格式研究定)
- `[Structures]`/`[Units]`/`[Infantry]`/`[Aircraft]`/`[Terrain]`/`[Smudge]` → 类型化对象列表
  (勘误 2026-11-04:真实段名是 `[Smudge]`,全语料 41 张含此段、0 张 `[Smudges]`——INI 普查
  docs/superpowers/research/2026-11-04-map-ini-survey.md 裁决)
- `[Triggers]`/`[Events]`/`[Actions]`/`[Tags]`/`[TeamTypes]`/`[TaskForces]`/`[ScriptTypes]`/
  `[VariableNames]`/`[Waypoints]` 等 → 按段保留原始键值行(tokenize,不解码语义)
- `[Digest]` 读取字节但**不校验**(结构记录进研究文档)
- 剧场交叉验证(测试层):抽 snow 剧场图的 tile_index 对照 snow.mix TMP 瓦片数

**不做**(边界,后续里程碑):
- 渲染/wgpu(M3+);地图写入;触发器/任务逻辑语义化;rules 依赖语义(通行性/高度代价);
  多人/遭遇战参数之外的任何模拟行为

## 4. 模块结构(`crates/core/src/format/map/`)

(2026-11-04 研究后增补:完整规格依据见两份研究文档;与初稿的差异——
新增 `lzo1x.rs` 解码器;codec 按段名分派非按块标志;4 字节块头非 5 字节;
Overlay 是固定 512×512 网格;Preview 是裸 RGB 非 packed SHP。)

- `crates/core/src/format/lzo1x.rs`(map 外层,与 lcw.rs 平级)— LZO1X 安全解压器,
  从规范语义重写(参照 Linux 内核 lzo1x_decompress_safe 的算法族与输入校验要点,
  CVE-2014-4608 家族/2017 加固清单为边界检查核对表),零第三方依赖;
  版本头(17,ver)有无都接受;EOF 由 m_pos==op 判定;上溢/下溢映射 FormatError。
  独立单测覆盖全部操作码族(字面量串/短匹配 M2/M1 扩展/M3/M4/尾部 next 字面量)。

- `pack.rs` — 包段容器层解码,四段共用:编号 key 按序拼接 → **标准 RFC 4648 base64** 解码
  → 4 字节块头循环 {u16 size_in, u16 size_out, payload} → 按段名分派 codec:
  IsoMapPack5/PreviewPack = **LZO1X**,OverlayPack/OverlayDataPack = **LCW**(复用 lcw_decode;
  "IsoMapPack5"的 5 即 codec id)。**解码前按声明尺寸做最小载荷预检**(size_in==0/超界、
  size_out==0/>8192 拒绝;聚合输出上界预检:overlay 恒 262144;瓦片流 ≤ 11·(2W−1)·H+4 且
  %11∈{0,4};W+H 语料极值 898),堵 M1b 终审顺延的内存尖峰类问题。
  7 张图 PreviewPack 尾部截断 base64 组:保留前缀、缺字节按"无预览"处理不报错。
- `mapdoc.rs` — 地图 INI 形态适配:复用 `IniDocument`,按普查裁决落地 7 项适配
  (段头尾随注释、值内 `;` 截断语义(原版 atoi 风格取前缀)、GBK 容错解码、
  RMG 孤儿行策略、有序条目不变量、重复键 last-wins/重复段合并锁定测试、
  缺 [Digest]/[IsoMapPack5] 等容错)——逐项依据与实例文件见
  docs/superpowers/research/2026-11-04-map-ini-survey.md 适配清单。
- `iso.rs` — 11 字节记录流 {u16 x, u16 y, i16 tile(−1=无瓦片), 3×零垫, u8 sub_tile, u8 z, 零垫},
  **以存储的 (x,y) 为权威键**(4 张 TS 代官方图省略 void 格;EA/FinalAlert 产图尾部 4 零字节
  终止符可有可无),稀疏存储 + 按 (x,y) 查询;[Map] Size 给出合法界;瓦片与 [Terrain]/
  [Structures]/[Waypoints] 同一菱形坐标系(恒等,无偏移)。
- `overlay.rs` — 解压后恒 262144 B = 行主序 512×512(x=i&511, y=i>>9)双层:
  OverlayPack=类型,OverlayDataPack=帧/状态;**0xFF 类型格下的陈旧 data 字节必须忽略**
  (46/540 真图携带,非对齐缺陷)。
- `preview.rs` — 裸 24bpp RGB 位图,无头,EA 图精确 3·[Preview]W·H;字节原样暴露
  (RGB/BGR 与行序 M2 不裁决,渲染时定)。
- `objects.rs` — 对象段 → `MapObject { 段类别, 原始键, 解析字段(坐标/类型/所属/朝向等) }`;
  通用编号键(0=,1=,…)与坐标键(地形段)两种形态都处理。
- `mod.rs` — `MapFile::parse(data: &[u8]) -> Result<MapFile, FormatError>` 总入口;
  `MapFile { basic, map_meta, tiles, overlays, preview, objects, logic_sections, lighting }`
  (字段名研究后可微调,API 形状冻结为:parse + 只读访问器)。

## 5. 错误与零 panic 策略

延续全局约束:零 IO、零 panic,`FormatError` 三变体
(UnexpectedEof=截断 / Invalid=畸形 / Unsupported=不支持形态)。
敌意输入(截断、坏 base64 字符、尺寸谎报、重复段、超大声明尺寸)一律 Err,不 panic、
不无界分配;所有容量分配前有输入字节数对声明的上界推导。debug 构建溢出检查同样零 panic
(照 M1b HVA u128 教训,宽计数算术用显式宽类型)。

## 6. 验收标准(完成定义)

1. 合成单测:每模块边界/敌意路径覆盖(CI 可跑,延续零依赖);
2. 代表性真图甲骨文:~15 张钉断言(官方协作图、遭遇战图、社区图、中文文件名图、
   随机地图生成器产物;瓦片数/对象数/包段解压字节数/抽样瓦片字段);
3. **807 张全量解析零 panic**(ignored 测试,RA2_ASSETS_DIR 下,含逐张 Err 情形合法);
4. 敌意变异样本集:每类至少一例(截断中段/坏字符/尺寸谎报 ×4 段);
5. 剧场交叉验证:snow 图 tile_index 上界 ≤ snow.mix TMP 瓦片数;
6. `cargo fmt`/`clippy -D` 零告警;GPL-3.0-only 头;零第三方依赖;
7. CLI 活体验收:`ra2-map info` 三张不同剧场图输出正确;`extract-tile` 产出可打开的 PNG。

## 7. CLI 契约(`tools/ra2-map`,退出码照 ra2-mix 的 0/1/2 契约)

- `ra2-map info <file.map>` — 输出:Basic 名称/玩家数、剧场、尺寸(格)、瓦片单元数、
  对象分档计数、四包段解压字节数、logic 段数
- `ra2-map extract-tile <file.map> <x> <y> -o <file.png> [--assets <RA2目录>]` — 取该格瓦片,
  按剧场调色板上色(剧场链/调色板自 `--assets` 指定的游戏目录解析,默认取
  `RA2_ASSETS_DIR` 环境变量),写最小合法 PNG(工具侧纯 std 编码器:PNG 签名 + IHDR +
  IDAT(zlib store 模式,无压缩器,CRC32 复用 core 的 crc32)+ IEND)

## 8. 测试策略

照 M1b:合成样本单测(CI)+ 真资产 ignored 测试(RA2_ASSETS_DIR)+ 自愈样本提取
(扩展现有 format_samples.rs 模式到地图);全量遍历单独一个 ignored 测试,失败时输出
首张问题图名与错误,便于定位。

## 9. 研究问题(已于 2026-11-04 由两个研究代理产出文档回答)

1. pack 编码精确规格:base64 变体字母表/位序、块头字段序、LCW 与原始块的选择位
   (XCC GPL 源 + ModEnc + 真图字节三方互证);
2. IsoMapPack5 记录布局与字段语义(含 5 字节小头;高度/冰冻增长位);
3. OverlayPack/OverlayDataPack 解压后布局与对齐语义;
4. PreviewPack 内部格式(packed SHP?);
5. IniDocument 对 807 张真图的兼容性实测(重复键/段外行/编码);
6. [Digest] 结构记录(不校验);
7. 剧场名 ↔ 剧场库链映射(snow/temperat/urban/desert/luna…)。

## 10. 里程碑定位

M2 = 地图/地形解析层(本规格);M3 候选 = 渲染 view 层(wgpu,独立 crate,用户在线时)、
rules/INI 扩展(get_float/atoi)。M1b 终审顺延项中,内存预检在本里程碑落地;
VXL z+count 界检与 CLI extract 路径消毒不属本里程碑(渲染/rules 时处理)。
