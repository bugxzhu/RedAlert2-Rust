# M3 剧场瓦片→TMP 映射 设计规格

日期:2026-11-05
状态:设计已获用户批准(AskUserQuestion 三项:tmp.rs M3 内更名重定域 / 语料闭合+全量
零 panic 验收 / gamemd REA 实现期并行抽查,2026-11-05)
上游:main = 520275a(已推送);研究文档 = docs/superpowers/research/
2026-11-05-theater-tile-mapping.md(裁决后基线,本规格一切格式事实的出处,下称"研究 §n")

## 1. 背景与目标

M2 的 `MapFile` 已给出每格 `(tile_id i16, sub_tile u8)` 与剧场目录名,但 tile_id 仍是
抽象整数。本里程碑交付**解析链闭环**:`tile_id → 剧场 INI 表 → 文件名 → mix 按名查找
→ TS TMP 文件 → 槽位 → RAW 菱形像素 + 剧场调色板`,使"取任一地图任一格的真实瓦片
像素"成为库调用。同时清偿三项裁决出的历史债:mix.rs 幻影 digest 错位、tmp.rs 的
M1b 误判(实为 TS SHP)、T9 容量测试失真。

## 2. 决策记录(用户 2026-11-05 拍板)

| 决策点 | 结论 |
|---|---|
| tmp.rs 误判处置 | **M3 内更名重定域**(TmpRa2File→ShpTsFile 系,字段语义重标);同里程碑新建 TS TMP 解析器 |
| 验收高度 | **语料闭合+全量零 panic**:807 图全量 (theater,id,sub)→文件闭合;现存瓦片全量 TS TMP 零 panic;敌意变异;代表甲骨文 |
| gamemd REA 补证 | **实现期并行抽查,不阻塞主线**:实现假设=expandmd01 覆盖 localmd、缺文件槽=未出货态;REA 冲突则回滚该点 |

## 3. 范围 / 非范围

**做**:
- `mix.rs` 幻影 digest 判别修复(研究 §2:19 个置位无块容器全体错位 +20 的潜伏缺陷)
- `tmp.rs` → `shp_ts.rs` 更名重定域(研究 §4:8B+24B = TS SHP 帧格式;
  RGBA/蓝通道/数据偏移字段重标;RAW 回退)
- 新建 TS TMP 解析器 `theater_tmp.rs`(研究 §4/§5:16B 头/槽表/52B 头/RAW 菱形/z)
- 新建 `theater.rs`:INI 表(atoi 前缀、blank 集、set006 保留槽)→ 索引 → 文件名
  (FileName%02d.ext)→ mix_name_id 跨容器查找 → 容量常数;缺文件槽=显式"未出货"态
- 资产装载器:根 mix + 递归嵌套 mix(expandmd01 匿名嵌套 #C31B4D61 是必需路径)
  → id→数据视图;剧场调色板链复用 M2 T10
- T9 遗留:map_samples 容量测试重钉(10016→964/INI 语义,改名+注释)
- CLI `extract-tile`:60×30 占位 → 真瓦片渲染(INI 反查+TS TMP 槽+rgb8 调色板)
- 验收:807 图全量闭合 + 六剧场现存瓦片全量零 panic + 敌意变异 + 甲骨文锚点
- REA 并行抽查任务(非代码交付物,见 §9)

**不做**(边界):
- 整图渲染/wgpu/视图层(M4 候选);高度 z 着色与 terrain/ramp 语义枚举
  (字段暴露原始字节 + 仅已裁决语义命名,研究 §5 值表逐值复核属后续);
- overlay/smock 渲染;rules 依赖语义;地图写回;TS SHP 的 remap/阴影语义
  (shp_ts.rs 只到帧数据字节/RAW-RLE 解码,不解释调色板运用)

## 4. 模块结构(`crates/core/src/format/`)

- `mix.rs`(修复)— 声明摘要位(flags&0x00010000)时的判别规则:**实现期对照
  XCC 源定稿语义并语义重写+注明出处**;工程约束:①语料 47 容器全绿;②M1a/M2
  既有测试不回归;③合成"真摘要块"样本覆盖跳 20 分支。判别信息进 `MixInfo`
  (或等价访问器),不改变 parse 入参形状。
- `shp_ts.rs`(原 tmp.rs 更名)— `ShpTsFile { zero, cx, cy, frames }`,
  帧头 24B:`x,y i16; cx,cy u16; flags u32@+8; rgba [u8;4]@+12;
  reserved u32@+16; data_offset u32@+20`(研究 §4:56=8+24·2 / 152=8+24·6
  精确闭合为据)。帧数据解码:RLEZeroTS 严格解,失败且 span==cx·cy → RAW
  (研究 §4 判别式,真 RLE 与 RAW 无歧义交叉)。`FormatError` 语义不变。
- `theater_tmp.rs`(新)— `TsTmpFile::parse`:
  头 16B i32 `{cbx, cby, cx, cy}`(guard:cbx·cby ≤ 65536、cx·cy/2 ≤ 2 MiB、
  4·cbx·cby + 52 ≤ len,敌意谎报一律 Err);槽表 cbx·cby 个 i32 相对偏移
  (0=空槽);每槽 52B 头(字段名照研究 §5:x/y/extra_ofs/z_ofs/extra_z_ofs/
  extra 矩形/bitfield/height/terrain_type/ramp_type/radar 左右色/pad;0xCD/0xCC
  填充域原样暴露不校验);访问器 `slot(i) -> Option<TsTmpSlot>`、
  `diamond_pixels() -> &[u8]`(恒 cx·cy/2 RAW)、`z_data() -> Option<&[u8]>`。
- `theater.rs`(新)— 三层:
  1. `TheaterIni::parse(text)`:段序保留、键大小写不敏感、TilesInSet **atoi 前缀
     语义**(出厂脏数据 'o'→0;研究 §3)、blank/0 不贡献、set006 型保留槽占位;
     `slot_count()`(=容量 838/1081/964/726/1189/198)、`file_name(idx)`、
     `set_of(idx)`(集合号+SetName);
  2. 剧场常量表:目录名→{生效 INI 名, 扩展名, 容量}(研究 §3/§8;urbannmd 采
     expandmd01 版);
  3. `TheaterAssets::open(assets_dir)`:装载 ra2.mix、ra2md.mix、根级 expand*/ecache*
     并**递归下钻嵌套 mix(深度 ≤3,体量上限防炸)**构建 id→(容器,范围)索引;
     同名 id 冲突时按 mod 覆盖惯例取后装载者(expand*>ecache*>ra2md>ra2 基序,
     实现期以语料闭合校准);`lookup(name) -> Option<数据>`(mix_name_id)。
- `tools/ra2-map` extract-tile 升级 — `(map, x, y)` → map 的剧场+tile_id+sub →
  INI 反查文件名 → mix 取文件 → TsTmpFile 槽 sub → 菱形像素(行宽
  4·min(r,cy−1−r)+2)铺入 cx×cy 画布,索引 0 行外区照调色板 0 号色 → rgb8 →
  png.rs(纯 std,复用);缺文件槽/空槽/越界 sub → 退出码 1 + 明确 stderr
  (未出货槽给出集合名)。
- map_samples.rs T9 重钉:`snow_tile_index_within_theater_ini_capacity`
  (常量 964,注释指向研究 §3)。

## 5. 错误与零 panic 策略

延续全局约束:零 IO 于核心(装载器在库边界收口为字节切片)、零 panic、
`FormatError` 三变体。所有槽/画布算术显式宽类型与上界推导(敌意 cbx/cy/cbx·cby);
INI 层对畸形键宽容(缺 FileName/TilesInSet=空→0),段外行忽略(M2 mapdoc 先例);
0xCC/0xCD 填充域不做值校验。

## 6. 验收标准(完成定义)

1. **807 图全量闭合**(ignored,RA2_ASSETS_DIR):每图每瓦片记录
   `(theater, tile_id, sub)` → INI 界内且文件现存且 sub < cbx·cby;
   违例白名单=恰 MadHQ 4 图既知残余(研究 §3,断言其上限不增长);
   缺文件槽命中集合 ∈ {ZMM/Obsolete 系既知清单} 且**语料零命中**(研究 §8)。
2. **现存瓦片全量零 panic**:六剧场全部现存 TS TMP 文件解析 Err 仅限
   既知敌意面,panic=0;抽样字节闭合(clear01.sno len=1872、首槽 RAW 900B)。
3. **敌意变异合成单测**:TS TMP 截断/谎报 cbx·cby/槽偏移越界;INI 坏段/坏整数/
   blank 变体;mix 真摘要/幻影双分支合成样本。
4. **代表甲骨文**:六容量、锚点([0]=Clear01、snow[963]=clifxl04.sno、
   nu[1174]=sfbrdg01.ubn、tem[837]=ariz01.tem、lun[197]=Crevas06.lun)、
   burnt01 帧 0 offset=56、lobrdb08 帧 1 offset=152、md 替换(snow base 798<964)。
5. `cargo fmt` / `clippy -D` 零告警;GPL-3.0-only 头;零第三方依赖;
   EA 资产零入库(samples 测试时自 RA2_ASSETS_DIR 抽取,fixtures gitignored)。
6. CLI 活体验收:extract-tile 对 snow/newurban/desert 各一图产出可打开 PNG
   且非占位网格(像素多值)。

## 7. CLI 契约(`tools/ra2-map`,退出码 0/1/2 契约不变)

- `ra2-map info <file.map>` — 不变(M2 契约)
- `ra2-map extract-tile <file.map> <x> <y> -o <file.png> [--assets <目录>]` —
  签名不变,语义升级为真瓦片(研究 §4 链路);stderr 增补链路诊断
  (剧场/INI 容量/tile_id→文件名/槽位)。

## 8. 测试策略

照 M1b/M2:合成单测(CI)+ 真资产 ignored(RA2_ASSETS_DIR)+ 自愈样本
(扩展现有 samples 模式到 theater/TS TMP;M1b 的 burnt01/lobrdb08 样本复用为
shp_ts.rs 甲骨文)。全量闭合单独一个 ignored 测试,失败输出首个违例图名+记录三元组。

## 9. 并行 REA 抽查(实现期,非阻塞)

单次 REA 代理任务清单:①Theater::Init 0x5349C0 的 base-vs-md INI 选择与
expandmd01 覆盖序;②缺文件槽引擎行为(跳过/占位/报错);③Theater::Array
0x3E1B78(6×112B)字段与常量表互证。发现与实现假设冲突 → 台账登记 → 回滚该点
重做;无冲突 → 结论入研究 §9 收尾。

## 10. 里程碑定位

M3 = 瓦片解析/反查层(sim 侧数据完备);M4 候选 = 渲染 view 层(wgpu,需用户在线)、
rules/INI 扩展。M2 缓期清单(T1 LZO 覆盖等)不属本里程碑。
