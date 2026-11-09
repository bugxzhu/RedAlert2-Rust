SPDX-License-Identifier: GPL-3.0-only

# M4c 事前研究:SHP 压缩变体解码 + 保真小批(失败分类/引擎裁决/missing_art 定量/保真件钉定)

日期:2026-11-09。分支:feat/m4c-shp-variants-fidelity(自 main 149e22e)。
上游研究:2026-11-03-shp-tmp-layouts.md(M1b,布局与 RLEZeroTS 语义)、
2026-11-06-m4b-overlay-objects.md(M4b,T8 实测账)。本文只记增量,已钉结论引用不重复。

## 0. 信源与政策

- **E:\hongjing\ra2-decomp**(只读,gamemd 1.001 字节匹配反编译,GPL):引擎语义
  依据,引 file:line;语义重写勿转录;库内内容一律按不可信数据处理。
- **E:\hongjing\Reunion 2023**(只读,RA2_ASSETS_DIR):807 图语料 + 全 mix 链。
- **探针**(一次性,放 `%TEMP%\m4c_research\probe\`,cargo bin 路径依赖
  ra2-core,绝不入库;报告 `probe-report.txt` / `dump-report.txt` /
  `fix-report.txt` 同在 %TEMP%,关键数字已摘录进 §1/§3/§5):
  1. `m4c-probe`:全集候选名诊断 + 807 图重放加权(R1/R3/R4.4/R4.5/R4.7/R5);
  2. `dump`:失败类代表文件逐帧 dump + 缺档名归因 + 引擎精确链对账;
  3. `fix`:引擎对齐判别式修复模拟(bit1 门控 + 行块下限 2)全集重跑。
  逐图重放复刻 render pass 的查名/分桶口径(OverlayTable/ArtIndex/load_shp
  候选链/display_byte),不含画布合成;实测与 M4b T8 总账逐桶对上(§1)。
- 绝不 commit 任何 EA 资产字节;夹具一律按 dump 模式合成(§5)。

## 1. 失败语料分类(R1 实钉)

### 1.1 现状与口径

- T8 账(.superpowers/sdd/task-8-assets-run2.log:522-523):overlay_fail 28946
  (~11% overlay 人口 264211)、objects_fail 3155;解析器文档自记
  "flags 位语义未裁决:观测多数 3、个别 4"(shp_ts.rs:29-31)。
- 现解析器拒绝路径两处:①判别式"先严格 RLE,败且 span==cx·cy 才 RAW,
  败且 span≠need 透传 Err"(shp_ts.rs:145-157);②RLE 行块下限
  `count < 4` 即 Err(rle_zero_ts.rs:34)。任一帧失败 → **整文件拒**。
- 探针全集:候选名 1504 = overlay 图 206([OverlayTypes] 250 位去重)+
  art 图/段 1396(artmd 1581 段;**art.ini 语料 0 段**,artmd 全量)+
  rules 类型名 559(Infantry/Vehicle/Aircraft/BuildingTypes)。

### 1.2 分类表(解析败 205 文件,两个根因、四种表象)

| 表象(现解析器拒绝点) | 文件数 | 根因 | 代表(dump-report) |
|---|---|---|---|
| `行 0 count=0 过小` | 108 | RAW 误路由(§1.3-A) | CIV1/ADOG/CIV2/CCOMAND/CIVB |
| `行 0 count=1 过小` | 11 | RAW 误路由 | GEM04/GACNST_A/GAYARD_C/MLTIMISL |
| `行 0 count=3 过小` | 15 | RAW 误路由 + 窄行合法块(§1.3-B) | CAFNCB/DNOA/BRUTDIE/NATBNK_B |
| `数据不足:需 N 剩余 M`(UnexpectedEof) | 71 | RAW 误路由 | GEM09/GEM11/TRACKS01-16/FENCE01-12/DRUM01 |
| 其余(header_zero/越界/空帧矛盾/画布越界) | 0 | —(语料零出现,M1b 检查全维持) | — |

- 帧头 flags u32 直方(全集 OK+败帧):`0x3`×20700、`0xCCCC0003`×195、
  `0xCCCC0001`×31、`0x4`×92、`0x1`×32、`0xCCCCCCCC`×2 —— 高半字 `0xCCCC`
  = 头部未初始化填充指纹,直接证明 "flags" 只在 +8 单字节(§2.1)。
- 偏移非单调文件 0、表前偏移帧 0、画布越界文件 0 —— M1b 的偏移/画布/
  空帧一致性检查在本语料零违例,全部维持。

### 1.3 根因(逐帧 dump 钉证)

- **A. RAW 帧误路由(主根因,约 190/205 文件)**:帧头 +8 字节 bit1=0 →
  引擎按 RAW 画(draw.cpp:88 分派;§2.2),而 ra2-rs 不看 flags、先跑严格
  RLE:把 raw 像素字节当 `u16 count` 读。三例铁证(dump-report):
  - `GEM09.tem` f021:(30,42) 2×2,byte_flags=0x00,off=4216,span=8,
    need=4,数据 `01 01 01 01`+4B pad → 严格 RLE 读 count=0x0101=257 > span 8;
  - `DRUM01.shp` f000:11×14,flags=0x01,span=160,need=154 → count=35840;
  - `FENCE01.tem` f000:57×28,flags=0x00,span=1600,need=1596 → count=5140。
  RAW 帧普遍带 2-8B 尾 pad(span≠need),现 RAW 兜底的 `span==need` 硬条件
  (shp_ts.rs:150)连带把本应 RAW 的帧逼进 Err 透传 → 整文件拒。
  `CIV1.shp` 476 帧 **同文件混 flags=0x03(RLE)与 flags=0x01(RAW)**
  —— 变体是帧级不是文件级,任何"文件级格式选择"都不可行。
- **B. 窄行 RLE 合法块(15 文件)**:`CAFNCB.shp` f048:1×1,flags=0x02,
  行块 `03 00 01` = count 3(2B 头 + 1B 数据,恰好 1 像素)——宽度 1 帧的
  合法行;现 `count<4` 下限(rle_zero_ts.rs:34,M1b 误推广)误拒。引擎侧
  行推进只按 count 值(blowfish.cpp:296-314),无下限校验。
- 修复模拟(fix 探针):判别式改 **byte bit1 门控 + 行块下限 2**(语义重写
  见 §2.4)→ 全集 1045 文件全过(840 旧 OK 零回归 + 205 全修复),帧级统计:
  RLE 帧 29041 / RAW 帧 2763 / 空帧 17782,行块 1,611,782 个,**行越宽 0、
  raw 尾截断 0**(RAW 全部含 pad 且未触文件尾)。
- 加权对账(807 重放):现判别式下 overlay 引擎精确链 = drawn 235265 /
  fail 28946 / missing 0,objects = 511347 / fail 3155 / missing 3539
  —— **与 T8 逐桶精确一致**,证明重放口径忠实;all-ext 候选链与引擎链差
  2595 格 = SROCK01-04/TROCK01-05 八名的跨剧场副本差异(§5.4)。

## 2. 引擎语义裁决(R2,decomp 实钉)

### 2.1 SHP-TS 容器(引擎视角)

- `SHPStruct { short Flags; short Width; short Height; short Count; }`
  (include/msanim.h:27-47)= 文件头 8B;`SHPFrame`(msanim.h:49-63):
  Left/Top/Width/Height 4×short,`unsigned char Flags` @+8,3 匿名字节,
  RGBStruct Color @+0xC,Reserved @+0x10,Offset int @+0x14 —— 24B 步长,
  字段位置与 ra2-rs 24B 帧头一致,唯 **flags 是 +8 单字节**,+9..+0xB
  匿名(+0xC..+0xF = Color 3B + 1 pad;ra2-rs 的 rgba/reserved 命名照旧,
  语义改注)。`0xCCCC` 高半字即这 3 匿名字节的未初始化残留(§1.2)。
- 装载:shapipe.cpp:65-84 `SHPReference` ctor 只读前 8B(**首 short 进
  Flags,引擎不验零**;ra2-rs/XCC 验零维持,语料 0 违例,§9 L2);
  `GetPixels(frame) = file + Offset`(shapipe.cpp:103-112),**无 region 界**。
- 透明/RLE 判定都在单字节:`HasTransparency = Flags & 1`
  (shapipe.cpp:137-145)、`HasCompression = (char)(Flags & 2) >> 1`
  (:148-156)。flags=4(0x4×92 帧)bit1=0 → 引擎按 **RAW** 画。

### 2.2 解码分派与 RLE 行链

- `Draw_Shape`(draw.cpp:25-104):`HasCompression → RLE_Blit : Bit_Blit`
  (:88-98);`remap != NULL → flags |= SHAPE_REMAP`(:84-86)、
  `convert.Set_Remap(remap)`(:41)。
- `RLE_Blit`(blowfish.cpp:211-330):行推进 `sbuffer += *(u16*)sbuffer`
  (count 含自身,:296-314);行展开体在 `RLEBlitter::Blit` 模板
  (include/blitter.h:21;convert.cpp 1073+ 仅记模板地址,**未在库内反编译**)
  → 行内零游程钳制/行补齐语义维持 XCC `RLEZeroTSDecompress`
  (shp_decode.cpp:748-778,M1b 已钉);**引擎无 count 下限校验、无行链
  region 界**(行链可越过下帧 offset 继续读文件,内存里就是同一缓冲)。
- ShapeFlags_Type 枚举数值未在库内(include 无 draw.hh;SHAPE_* 只有用法
  无定义)→ 位语义只能经 convert.cpp 分派链读出(§9 L4)。

### 2.3 与 R1 分类对拍

| 失败类 | 引擎路径 | ra2-rs 偏差 |
|---|---|---|
| RAW 误路由(108+11+71 文件) | bit1=0 → Bit_Blit(读 need 字节,pad 不影响) | 无 flags 门控,先 RLE → 把 raw 字节当 count;RAW 兜底 span==need 硬条件被 pad 破坏 |
| count=3 窄行(15 文件) | 行推进按 count,无下限 | `count<4` Err(rle_zero_ts.rs:34) |
| flags=4/0xCCCC0001 等 | 按 +8 字节判,bit1=0 → RAW | u32 语义未定(原样暴露),判别式不受影响但与引擎无对账 |

### 2.4 语义重写建议(M4c 实现面)

1. `ShpTsFrameHeader` 增只读判定 `is_rle()` = `(flags & 0xFF) & 2 != 0`
   (字段仍存 u32 原样,高半字保持"未名填充"注记);删"先 RLE 后 RAW"
   判别式,改引擎对齐:bit1 → RLE,否则 → RAW。
2. RAW = 取 `[offset, offset+need)`;`offset+need > 文件尾` 时语料 0 出现
   ——建议补零截断 + 防御计数(lenient 零 panic;敌意输入不再整文件拒),
   替代现 `span==need` 硬条件。
3. `rle_zero_ts_decode` 行块下限 4 → **2**(count=2 = 空行,count=3 = 单
   像素窄行;均为语料实证形态)。
4. RLE 链区间改为 `[offset, 文件尾)`(引擎无 region 界;语料实证:修复版
   全集 0 败、行展开在行宽钳制下 rows_over_width=0,以文件尾为界足够安全;
   EOF 触碰仍 Err)。原"下帧偏移为界"(M1b §1.3)是探针期安全假设,
   被本语料证伪为不必要且语义不等价。
5. 空帧约定/画布界/偏移越界检查全维持(§1.2 语料零违例)。

## 3. missing_art 3539 定量(R3 实钉)

- **`;` 注释假设证伪**:artmd.ini 1581 段、art.ini 0 段(语料未出货);
  art Image= 值 0 处含 `;`;重放 3539 件 missing 中带 `;` 名 0 件,
  剥注后可解 **0** 件(probe-report R3 行 + dump-report D 表)。
  (rules 层的 `;` 剥离已由 M4b OverlayTable 落地,覆盖其存在面。)
- **真缺档 95 名 / 3539 件**(dump-report D 全表,重放加权),三类:
  1. **TS 灯树/道具,art 无节**(图名=类型名):ingalite 908 / inblulmp 353 /
     neglamp 291 / inyelwlamp 230 / atesla 111 / snodaylamp 59 / fapost 52 /
     gaspysat 51 / inredlmp 47 / temdaylamp 45 / e1 40 / tesla 26 ……
  2. **TS 树/岩,art 有节但目标 SHP 未出货**:tree31-36(tree32 188)、
     camorr04-10(共 ~350)、gtgcan 47、canewy04/cala06/cawsh12/capars01/11…
  3. **NewUrban 散件 x1-2 件级**:cakane01/cascie01/catent05-06 等。
- 剧场分布:Desert/NewUrban 偏重(自定义图源),Temperate 亦有
  (ingalite 339/temperate)——TS 语源资产未随 YR 零售(Reunion 2023)出货。
- **结论:missing_art 3539 不可由解析器收敛**;唯一路径 = 用户侧资产补丁;
  别名表无证据不做(§9 L6)。

## 4. 保真小批逐项钉定(R4)

1. **阴影帧**:overlay 阴影 = 帧 + `Get_Count()/2`(cell.cpp:1393
   Draw_Overlay_Shadow,字面 flags 0x4601 含 DARKEN 位但枚举数值未在库内,
   §9 L4);地形同式 + SHAPE_DARKEN(terrain.cpp:282-283,全局
   DrawShapeShadows 门控);"SHP 后半 = 阴影半"为跨类型惯例
   (builtype.cpp:771-802 rubble 帧、animtype.cpp:264 中帧)。物件侧调用链
   `Techno_Draw_Object` **未在库内反编译**(foot.h:329-331 仅 inline 转发;
   include/techno.h:667 声明)→ 帧号规则可钉(+Count/2 惯例,infantry 腾空
   阴影另有专档 INFSHDW.SHP f1,infantry.cpp:585-599),**DARKEN 变暗表
   (ConvertClass ShadowTable)未钉 → M4c 建议:阴影帧号可入(旗标化 +
   Preview 差率监督),变暗效果顺延**。
2. **remap(所属方调色板)**:`TechnoClass::GetRemapColour`
   (techno.cpp:4355-4367)= `ColorSchemes[House->Scheme]->Converter`
   (类型 Palette 覆盖/伪装所属方分支在列);Converter 构建时把基础调色板
   **16..=31 号槽替换为所属方 HSV 的 16 档色**(scheme.cpp:52-77,:91-102;
   参与索引集 _indexes scheme.cpp:15-43);SHAPE_REMAP 逐像素
   `RemapTable[src]`(convert.h:38/161,draw.cpp:41/84-86);墙用
   `ColorSchemes[PlayerPtr->Scheme]`(cell.cpp:1462)。LightConvert 表全构建
   链未钉(lightcon.cpp 仅查到插值片段)→ **复杂度超 M4c,顺延**(需每所属
   方 HSV + 16 槽重映射 + 表验证;独立批候选)。
3. **overlay −2px fudge + SHAPE_ALPHA/ZWRITE**:`height_offset` = RLE_Blit
   的 zdepth(blowfish.cpp:211/:255-258,`current_z += zdepth`)→ 只进 z
   缓冲;SHAPE_ALPHA 只累加 alpha 缓冲(cell.cpp:1325-1358),影响后续半
   透明叠绘,不改本层 RGB;ZWRITE 同为 z 缓冲面。**定性:纯 2D 无 z 缓冲
   模拟下对 RGB 零影响,无需模拟**(诊断位可留,不做)。
4. **坡地 ore**:坡地显示型 = `OverlayTypes[tiberium->Overlay->HeapID +
   Variety + RampVariety/4 × (Ramp−1) + (X·Y) % (RampVariety/4)]`
   (cell.cpp:1448-1450),平地 = `HeapID + (X·Y) % Variety`(:1451,M4b
   已钉);`SlopeZShapes[Ramp−1]` 只作 z-shape 传参(draw.cpp:68-82 →
   RLE_Blit zshape,z 缓冲面)。语料:ore 格 213,488,**坡地 946(0.44%)**
   (ramp 1..12 主 + 17/18 尾;fix 探针 ramp 直方)。实现需 [Tiberiums] 的
   Variety/RampVariety → **顺延**(收益 0.44% 格面,记账即可)。
5. **VEINHOLE 不画 + IsRubble 特例**:VEINHOLE/VEINHOLE_DUMMY 直接 return
   (cell.cpp:1401-1403);零售 [OverlayTypes] 唯一 VEIN 名 = 表位 178
   VEINHOLEDUMMY,**语料命中 0 格**(dump-report C);IsRubble 分支
   (cell.cpp:1384-1391 阴影 / :1477-1481 本体)→ builtype.cpp:771-802;
   零售 overlay `Rubble=yes` 0 条、art `Rubble=` 0 键 → 语料 0。**建议:
   3 行 lenient 守卫(表名 VEIN 前缀跳画)入 M4c 零成本;rubble 语料 0,
   只记语义不实现**。
6. **Tiberium/Crate bool 现状**:零售 rules 实测 `Tiberium=` = "yes"×94 +
   [Powerups] 段 1 处 `0,<none>,no ; tiberium patch`(TS 残段,非 overlay
   型节,引擎 Get_Bool 不达;dump B);`Crate=` = "yes"×2。get_bool 全串
   精确匹配(ini.rs:92-100)与引擎 Get_Bool 同判 → **值干净,无需修**。
7. **物件 z 侧账(纯诊断位)**:现状 objects.rs:81
   `map.tiles.cell(wx, wy).map_or(0u8, |t| t.z)` 记录缺席 → z=0;语料量级
   **9,288 件/807 图**(probe R4.7)。接口建议:`SceneStats` 增
   `objects_z_fallback: usize`(render/mod.rs:61-81 信息位区,不入侵三组
   守恒恒等式),在 objects.rs:81 的 map_or 分支计数;overlay 无此面
   (其 z 来自记录本体)。
- 附带佐证:墙连通变体已由地图字节承载(Wall_Update 写 OverlayData,
  cell.cpp:1750-1763)→ 现帧直传语义正确,M4c 无需重算。

## 5. 验收口径预测(R5)

### 5.1 数量预测(修复后 807 全量,fix 探针)

| 桶 | M4b 实测 | 修复后预测 | 依据 |
|---|---|---|---|
| overlay drawn | 235265 | **264211**(+28946) | 人口 264211 恒等,fix 全集 0 败 |
| overlay fail | 28946 | **0** | 205 败文件全修复(§1.3) |
| overlay no_type/missing_file | 0/0 | 0/0 | 不变 |
| objects drawn | 511347 | **514502**(+3155) | fix 重放逐桶一致 |
| objects fail | 3155 | **0** | 同 overlay 根因 |
| objects missing_art | 3539 | **3539(不变)** | §3 真缺档 |

### 5.2 可钉断言(assets ignore 测试,同 m4b 口径)

1. `overlay_fail == 0 && objects_fail == 0`(807 全量,M4c 主断言);
2. 总量 oracle:`overlay_drawn == 264211`、`objects_drawn == 514502`、
   `missing_art == 3539`(语料固定前提;负 oracle 防改名蒙混);
3. 守恒恒等三组维持(地形七桶/overlay 四桶/物件五桶);
4. Preview 四组合直方图只报告不收紧(阴影/remap/voxel 仍未实现,85.6%
   RGB 最众占比预期维持,差率分布自然下降部分由阴影/remap 顺延面解释)。

### 5.3 字节级 oracle 锚(合成夹具;按 dump-report 模式复刻,绝不 commit EA 字节)

- a. RAW+pad 尾:DRUM01 模式(11×14 flags=0x01,154B raw + 6B pad;同型
  TRACKS01 flags=0x00、FENCE01 span−need=4);
- b. 同文件混 RLE/RAW:CIV1 模式(f0 flags=3 RLE 行链,f1 flags=1 RAW);
- c. 窄行 RLE:CAFNCB f048 模式(1×1 flags=2,行块 `03 00 <px>`);
- d. 判定面:flags=4 → RAW;flags=0xCCCC0003 → 只读 +8 字节按 RLE;
  空帧 flags=1(现测试已有,保留);
- e. 逐帧像素期望:RAW 帧输出 = offset 起 need 字节(pad 不入帧);
  count=2 空行 → 整行透明。

### 5.4 口径注记:两候选链差异 2595 格

引擎精确链(.shp → 本剧场 ext)与 all-ext 链(.shp → 六剧场任意 ext)在
SROCK01-04/TROCK01-05 八名上分桶不同(合计 2595 格):这些名存在跨剧场
副本,某剧场坏副本被引擎链首中计 fail、好副本被 all-ext 链找到计 ok。
修复后两链 0 败,差异消失;§5.1 drawn 采用引擎精确链口径。
证据:dump-report D 节 delta 表 `{SROCK01:321, SROCK03:277, SROCK04:280,
TROCK01:439, TROCK02:347, TROCK03:325, TROCK04:291, TROCK05:315}`。

### 5.5 预算影响

严格 RLE 全帧解码实测 53,211,592 B / 56ms ≈ **940.6 MB/s**(probe R5);
修复版全集(49,586 帧)扫描 56ms → 解码增量在每图 µs-ms 级,
**10578ms/图预算与 ~656s 套件总时长均不动**(解析复杂度不变,少一次
RAW 兜底试探反而略省)。

## 9. 低置信清单

| # | 项 | 处置/监督 |
|---|---|---|
| L1 | RLEBlitter::Blit 行展开体未在库内反编译(convert.cpp 1073+ 仅记模板地址) | 零游程钳制/行补齐维持 XCC 语义(M1b 已钉);bit1 门控 + count≥2 为引擎侧新钉(shapipe.cpp:148-156 / blowfish.cpp:296-314);监督:修复版全帧 rows_over_width=0 + 全集 0 败 |
| L2 | 引擎不验头首短为零(shapipe.cpp:76 读入 Flags 即弃)而 ra2-rs/XCC 验零 | 语料 0 违例 → 验零维持;若未来语料出现 header_zero 类再裁 |
| L3 | 建筑/步兵地面阴影的绘制调用链(Techno_Draw_Object 未反编译) | 帧号 +Count/2 惯例实现需旗标化 + Preview 差率监督 |
| L4 | ShapeFlags_Type 枚举数值(draw.hh 未入库;0x4601 等字面量不可分解) | 只用 convert.cpp 分派位判定语义,不猜数值 |
| L5 | SROCK/TROCK 跨剧场副本逐剧场好坏分布未深挖 | 修复后两链皆零败差异消失;Preview 差率异常时首查副本选择序 |
| L6 | missing_art 95 名的别名可能(不同词干出货) | 无证据不做别名表;资产补丁归用户侧 |
| L7 | SHAPE_DARKEN 变暗表(ConvertClass ShadowTable)内容未钉 | 阴影"变暗"效果顺延;帧号规则先入 |

## 出处

- decomp 引文均出自 E:\hongjing\ra2-decomp(只读)上列 file:line;语义重写
  政策同 2026-11-05-theater-tile-mapping.md §0/§9.1。
- 真图/资产实证:E:\hongjing\Reunion 2023(只读),807 图语料;探针与报告
  在 %TEMP%\m4c_research\(probe/dump/fix 三 bin + 三报告,不入库)。
- T8 基线账:.superpowers/sdd/task-8-assets-run2.log:522-530。
- 既有已钉结论(RLEZeroTS 行格式、空帧约定、overlay/物件链)引自
  2026-11-03-shp-tmp-layouts.md 与 2026-11-06-m4b-overlay-objects.md,本文
  仅记增量与勘误(count≥4 下限、span==need RAW 兜底、"flags u32"读法)。
