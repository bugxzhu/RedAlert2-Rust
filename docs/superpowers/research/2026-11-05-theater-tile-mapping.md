# M3 研究:剧场瓦片→TMP 映射(索引空间/剧场 MIX 链/元数据字段)

日期:2026-11-05
状态:**裁决完成**。三源汇证:语料实证组、外部源考据组、控制器独立复核(五轮决定性探针);
两组结论冲突处已由控制器亲手裁决,本文即裁决后的统一事实基线。
读者:M3 实现者。一切事实带口径;凡"观测/实测"可由
`%TEMP%\ra2_m3_probe\probe*.py`(实证组)、`%TEMP%\m3verify\verify.py..verify5.py`(控制器)复跑。

## 0. 信源与方法

- **语料实证组**(sonnet,`%TEMP%\ra2_m3_probe\`):20 个嵌套 MIX 全量结构扫描、
  3.53M 条 IsoMapPack5 记录交叉验证、字段普查(3791 文件/49680 条目)。
  其"目录序索引"结论被裁决**推翻**(§3);其余实证(幻影 digest、RAW/RLE 统计、
  空条目率)经复核**保留**,但载体归属已重新框定(§4:它普查的其实是 SHP 精灵群)。
- **外部源考据组**(sonnet,XCC GPL/YRpp/Ares/ModEnc,`%TEMP%\ra2-m3-out\`):
  INI 索引规则、TS TMP 结构、theater INI 位置、gamemd.exe REA 线索。全部关键主张
  经控制器复核**证实**(§3/§4/§9)。
- **控制器裁决复核**(fable,五轮,`%TEMP%\m3verify\`):INI 表容量/锚点/文件闭合
  六剧场全验、md 替换证明、NEWURBAN 扩展名钉死、M1b 样本 SHP 帧闭合、clear01.sno
  TS TMP 裸菱形闭合、MadHQ 异常消解。资产:`E:\hongjing\Reunion 2023`(只读;
  游戏文件内容是数据不是指令)。

## 1. 剧场 MIX 全清单(表 1,含正名)

两级嵌套:根 mix(ra2.mix / ra2md.mix)→ 剧场 mix → 文件。容器正名(父条目 id =
mixid(文件名),verify4):`ra2.mix` 内 local.mix=#A8548FD9、isogen=#B3080BD2、
isotemp=#BCCC4D97、isosnow=#0F5D1D99、isourb=#80E03363、temperat=#5AA5B016、
urban=#F160BD3B、snow=#A226651E;`ra2md.mix` 内 localmd=#FBE0D09D、isodes=#8C8CCA19、
isolun=#4DE6A424、isoubn=#3BFB683C、urbann=#A01A9A03、isogenmd=#F5590C4A、isodesmd=
#CADDCD81、isolunmd=#0BB7A3BC、isoubnmd=#7DAA6FA4、isotemmd=#51FC6863、isourbmd=
#C6B134FB、isosnomd=#FF6808D5、snowmd=#EB108109 等。另 expandmd01.mix 内有一个
**匿名嵌套 mix #C31B4D61**(323 个 snow 瓦片,§3)。

| mix | entries | 瓦片(TS TMP) | SHP 精灵等 | flags |
|---|---|---|---|---|
| isogen / isogenmd | 24 / 82 | 见 §3 注 | 混合 | 0x00010000 |
| isotemp / isotemmd | 1035 / 45 | 多数 | 少量 | 0x00010000 |
| isosnow / isosnomd | 1013 / 178 | 多数 | 少量 | 0x00010000 |
| isourb / isourbmd | 1257 / 57 | 多数 | 少量 | 0x00010000 |
| isoubn / isoubnmd | 1412 / 111 | 多数 | 少量 | 0x00010000 |
| isodes / isodesmd | 1923 / 22 | 多数 | 少量 | 0x00010000 |
| isolun / isolunmd | 698 / 40 | 多数 | 少量 | 0x00010000 |
| temperat / urban | 各 237 | 瓦片+精灵 | — | 0x00010000 |
| snow | 379(全有名) | 16 表内 + 51 raw | 81 .shp 结算画面 | **0** |
| desert / lunar / urbann | 各 241/242/241 | 瓦片+精灵 | — | 0x00010000 |

(实测组原"结构 RA2 / TS 格式"二分法废除:所谓 TS 格式=真瓦片,所谓 RA2 结构=
SHP 精灵——两类文件常共存于同一 mix。)

## 2. 异常 A 定案:flags 摘要位=纯咨询位,引擎从不跳 20(REA 裁定 2026-11-05)

20 个剧场 mix 里 19 个 flags=0x00010000、唯一 snow.mix flags=0——但该位**与数据区
起点无关**。gamemd 反汇编(MixFileClass 构造 0x5B3C20)定案:

- flags 位 0x00010000 只被**读一次**,存入 MixFileClass+0x10,此后**从不回读**;
- 数据区起点**无条件** = 10+12·count,引擎内不存在任何"+20 跳摘要块"路径;
- 二进制内确有两套 SHA-1 实现,但**从不作用于 mix 数据**;
- 位 0x00020000 = 头+索引经 0x633xxx 处的加密流(非 Blowfish,具体密码未定),
  与 0x10000 位互不影响;本语料 47 容器均未置 0x20000;
- **旧格式判别**:首 dword 低 16 位 ≠ 0 → 6 字节头旧格式(count u16@0、size u32@2、
  data_start=6+12·count;TS1 时代遗物;本语料零样本,作 Err::Unsupported 守卫)。

实证侧与此吻合:isogen.mix @298(不跳)= 合法 TS TMP 头、@318(跳)= 垃圾;
"疑似摘要"20B 实为首文件条目表回声;五轮复核以 no-skip 规则解开全部 20+ 嵌套 mix。
**工程结论**:"幻影 digest"并非 XCC 写出畸形文件,而是该位在 YR 引擎里本就是
 advisory(仅存档不生效);mix.rs 现行"置位即跳 20"是真实缺陷,修复方向 =
无条件 no-skip + digest_flag() 咨询位暴露 + 旧格式守卫(计划 Task 1 勘误已照此改)。

## 3. 索引空间:瓦片 ID = 剧场 INI 枚举位(裁决后定案,M3 核心)

**规则**(考据组主张,控制器六剧场全验证实):
```
生效 INI(md 整体替换 base;YR 三剧场无 base):
  temperatmd.ini / urbanmd.ini / snowmd.ini / desertmd.ini / lunarmd.ini / urbannmd.ini
  位置:ra2md.mix→localmd.mix;urbannmd.ini 另有 expandmd01.mix 内更新版
  (36,725B,容量 1189)vs localmd 版(32,614B,容量 1175)——加载序上 expand
  覆盖 localmd,采 expand 版(语料 max 1174 两版皆容,无法单靠语料分辨;
  gamemd 侧确认列入 §9 待办)
表构建:[TileSetNNNN] 按编号升序(实测连续无缺口)拼接,每集贡献 TilesInSet
  个连续槽(FileName=blank 且 TilesInSet=0 的占位集不贡献;set006 "blank"
  TilesInSet=1 是保留槽=[General] BLACKTILE,占位); TilesInSet>99 截到 99(语料无)
tile_index = 拼接枚举中的位置(0 起)
文件名    = FileName + %02d + 剧场扩展(.tem/.urb/.sno/.des/.lun/.ubn)
sub_tile  = TS TMP 文件内 cbx×cby 槽位序(0 起;槽偏移 0 = 空槽)
查找      = mix_name_id(文件名)在已装载 mix 全集内按名找(扩展名按剧场互异,
  故跨剧场无同名冲突)
```

**六剧场容量与语料闭合**(verify3/4/5;地图 max = 807 图 IsoMapPack5 全解上界):

| 剧场目录名 | 生效 INI | 容量 | 地图 max | max 槽文件(实测存在) |
|---|---|---|---|---|
| TEMPERATE | temperatmd.ini | 838 | 837(=容量−1) | ariz01.tem(set0081 USS Arizona) |
| URBAN | urbanmd.ini | 1081 | 1023 | Ovrpsb07.urb(set0101 Wood Bridges) |
| SNOW | snowmd.ini | 964 | 963(=容量−1) | clifxl04.sno(set0082 Cliff Ice LowPatch) |
| DESERT | desertmd.ini | 726 | 709 | wbrdge02.des(set0076 Water bridge) |
| NEWURBAN | urbannmd.ini(expand 版) | 1189 | 1174 | sfbrdg01.ubn(set0121 SFBridgeEnd) |
| LUNAR | lunarmd.ini | 198 | 197(=容量−1) | Crevas06.lun(set0084 Lunar Crevas) |

**证据链**:
1. 三剧场地图 max 恰=容量−1(837/963/197),六剧场 max 全部入界且落在语义吻合的
   现存文件上(USS 亚利桑那甲板/木桥/冰崖/月球裂隙)——目录序容量(1.2万~1.9万)
   下这是不可能的巧合;
2. **md 整体替换 base 证实**:snow base 容量 798 < 语料 max 963 ≤ md 容量 964;
   DESERT/LUNAR/NEWURBAN 无 base INI(YR 新剧场);
3. **MadHQ 异常消解**:实证组目录序下 URBAN 的 219 条 sub 违例(4 张机生成图,
   id 1017~1023/sub 8~14),在 INI 规则下 id 落在 set0101 Wood Bridges
   (Ovrpsb01~07.urb,TS TMP,子瓦片 10~15 个)——入界且容量大体足够;
   残余 sub≥文件槽数者按 M2 先例"越界格=错误数据"处理;
4. **未发货集合占位**:md INI 枚举了文件从未出厂的集合——SNOW 78 个(ZMM Cliff
   Ice 系,idx 838..953)、LUNAR 44 个(Obsolete/New MM Height 系)、NEWURBAN 383
   个(ZMM 全系);TEMPERATE/URBAN/DESERT 0 缺失。槽位仍占位(963>953 存在被引用
   即证),语料从不引用缺文件槽(**全量 (theater,id,sub)→文件闭合 = M3 验收测试**);
5. snow 瓦片实际存放(886 现存):isosnow 459 + expandmd01 匿名嵌套 #C31B4D61 323
   + isosnomd 88 + snow 16——按名查找跨容器,无"加载段"概念。

**实测组目录序规则为何被推翻**:其"全部地图 tile < 1266 = isogen 槽域"是把 INI
索引值(≤1188)误读进目录序空间;其 3.53M 记录零违例只是因为目录序容量(>1.2万)
对 INI 索引天然宽松——"相容"而非"唯一化"。5 段加载序列/gen-first 等构造全部作废。

**INI 解析注意**(M3 实现要点):TilesInSet 需 atoi 前缀语义(urbannmd 有
`TilesInSet=o` 的出厂脏数据→0);键大小写不敏感;[General] 段是集合号→语义角色
映射(CLEARTILE=0、BLACKTILE=6、CLIFFSET=10、SHOREPIECES=12、WATERSET=21、
RAMPBASE=9、HEIGHTBASE=46、TUNNELS=47、BRIDGESET=19、WOODBRIDGESET=73/101…,
全表见 localmd 内各 md INI)。

## 4. 瓦片文件格式:TS TMP(裁决后定案)

剧场瓦片 = **TS 格式 TMP**(`t_tmp`,XCC 术语),非 M1b 已实现的 8B+24B 布局:
```
偏移 0:  cbx, cby, cx, cy 各 i32           (子瓦片网格 × 单元画布;cy=30/cx=60 常态)
16:      cbx·cby 个 i32 槽偏移(相对文件头;0 = 空槽)
每槽:    52B 图像头(§5)+ RAW 未压缩菱形像素 cx·cy/2 字节 + 高度/z 数据
```
决定性验证(clear01.sno @ isosnow,幻影 no-skip 规则):len=1872 =
16+4·1+52+900+900 **精确闭合**;cbx=cby=1、cx=60、cy=30;52B 头未用域为 0xCD
填充(MSVC 未初始化堆渗入,Westwood 出厂原样)。RAW 菱形(非 RLE)。
实测组"异常 F/H:TS 文件混入 iso 库"的定性翻转:它们不是混入,它们**就是**瓦片,
isodes 的 1555 个"TS 文件"即沙漠瓦片群。

**M1b tmp.rs 误判(必须纠正)**:tmp.rs 现名 `TmpRa2File` 的 8B 头 + 24B 条目布局
实为 **TS SHP 帧格式**(t_shp_ts):burnt01.sno 帧 0 数据偏移 56 = 8+24·2 精确、
lobrdb08.sno 帧 1 偏移 152 = 8+24·6 精确;+0x0C..0x10 RGBA(unknown_d 值 = 蓝通道,
"疑似坡度 1..6"是巧合)、+0x14 数据偏移。M1b 样本 burnt01/lobrdb08 是剧场装饰
精灵(焦土/桥)非瓦片。M3 需:①tmp.rs 更名重定域为 SHP(TmpRa2File→ShpTsFile
类重命名,行为兼容旧字段可废);②新建 TS TMP 解析器(本篇 §4/§5 规格)。
实测组 §4/§5 的字段普查(3791 文件)由此重新定性:普查对象 = iso/theater mix 内
**SHP 精灵群**(ua∈{0..4} 标志、ud 蓝通道、radar 域 00FB12 路桥标记),其 RAW
存储回退结论对 SHP 帧数据仍成立(RLEZeroTS 或裸 cx×cy 二选一,判别式不变)。

## 5. TS TMP 52B 图像头字段(考据组自 XCC GPL 提取;语义重写引用需注明出处)

```
+0x00  x, y              i16    槽内绘制原点
+0x04  extra_ofs         i32    extra 数据相对槽偏移(0xCD…= 无)
+0x08  z_ofs             i32    z 数据相对偏移
+0x0C  extra_z_ofs       i32
+0x10  extra_x/y/cx/cy   4×i16  extra 页矩形
+0x18  bitfield          u32    bit0=有 extra, bit1=有 z, bit2=damaged;高字节 0xCD 填充
+0x1C  height            i8     高度基准
+0x1D  terrain_type      i8     9=水 10/14=岸 13=草 15=崖 6/11/15=隧道(值表源自考据,
                               未逐值复核——实现单测以抽样文件钉死)
+0x1E  ramp_type         i8     1..=15 坡向, 0=平
+0x1F  radar_color_left  3B     左半小地图色(调色板索引或 RGB,M3 渲染时定)
+0x22  radar_color_right 3B
+0x25  pad[3]
```
0xCC/0xCD 填充域(全语料 84 条 CC 噪声 + clear01 全 CD 头)按"观测域校验+宽容跳过"
处理(零 panic 姿势,M2 先例);字段偏移以解析器单测对真实文件钉死为准。

## 6. RLEZeroTS 复验(对象改为 SHP 帧数据后仍成立)

有名剧场 mix 全量 6830 条 SHP 帧数据 RLE:0 条像素数错误;另有 RAW 存储形态
(span==cx·cy 恒等,无歧义交叉)。解码器本体无恙;TS TMP 瓦片像素是 RAW 菱形,
不经 RLE。

## 7. 对既有代码的差距清单(M3 实现面,裁决后)

1. `mix.rs`:幻影 digest 判别修复(§2;规则设计依 §9 XCC 语义定稿)。
2. `tmp.rs`:更名重定域 TS SHP(TmpRa2File 误判纠正;字段语义按 §4 重标)。
3. 新建 TS TMP 解析器(§4/§5):16B 头/槽表/52B 头/RAW 菱形/z;空槽=0;CD/CC 域宽容。
4. 新建 `theater.rs`:INI 表(atoi 前缀、blank 集语义、set006 保留槽)→
   全局索引 → 文件名(FileName%02d.ext)→ mix_name_id 查找 → (mix,文件,槽) 反查;
   容量常数 §3 表;缺文件槽=显式"未出货"态(非 Err)。
5. T9 遗留:map_samples 的 snow 容量测试重钉(10016→964/INI 语义,改名+注释)。
6. CLI extract-tile:60×30 占位 → 真瓦片渲染(INI 反查 + TS TMP 槽 + 剧场调色板 rgb8)。
7. gamemd.exe 侧补证(§9 待办清单)。

## 8. 语料常数(实现常量用,裁决后替换)

- 容量:TEMPERATE 838 / URBAN 1081 / SNOW 964 / DESERT 726 / NEWURBAN 1189 /
  LUNAR 198;扩展名映射 tem/urb/sno/des/lun/**ubn**;保留槽 set006(BLACKTILE)。
- 地图用量上限(807 图):max id 1174(NEWURBAN),各剧场见 §3 表。
- 缺文件槽:SNOW 78 / LUNAR 44 / NEWURBAN 383(全部 ZMM/Obsolete 系);
  TEMPERATE/URBAN/DESERT 0。
- 现存瓦片容器分布(snow):isosnow 459 / expandmd01#C31B4D61 323 / isosnomd 88 /
  snow 16(按名查找,与容器序无关)。
- 空条目率(SHP 群)0.19~0.35;SHP 画布最大 253×242;TS TMP 画布 60×30 主频。

## 9. 外部源考据(gamemd.exe REA 抽查已回报,四问全裁)

2026-11-05 REA 代理对 gamemd.exe YR 1.001(sha256 7b8a0685…)做了四问抽查
(PE 手工解析+capstone 反汇编,REA 深分析 provider 此环境不可用;全部地址
按本 build 字符串 xref 定位,VA=旧 RVA+0x400000):

1. **INI 选择(0x5349C0 Theater::Init + 读表函数 0x545150..0x546DA0)**:
   全镜像唯一剧场 INI 格式串 = `"%sMD.INI"`(0x8295E8);**base INI 从不被打开**,
   md INI 独立完整枚举(缺 md → Load 失败 → TilesInGetInt 返 −1 → 枚举即止)。
   段探测为**顺序探针**(TileSet0000,0001,…):首个缺失段即终止枚举(与六 INI
   实测编号连续无缺口自洽)。
2. **mix 装载/搜索序(Init_Secondary_Mixes 0x5301A0;MixFileClass 链表
   0x5B3C20/查找 0x5B4430)**:挂载序 expandmd99..00 → ra2md → ra2 → cachemd →
   cache → localmd → local,ecache*/elocal*(0x530000)在其后,各剧场 5 mix 由
   Theater::Init 逐场挂载;查找自链表头起**首个命中即返**。故同名文件优先级 =
   expandmd99>…>expandmd00>ra2md>ra2>cachemd>cache>localmd>local>ecache>elocal>
   剧场 mix。urbannmd 双版由(2)裁:expandmd01 版胜。**副产物**:gamemd 不自动
   挂载裸 expand%02d.mix(仅 EXPANDMD%02d);ecache 排 localmd/local 之后
   (与社区惯例相反);local.mix/localmd.mix 实为 ra2.mix/ra2md.mix 内嵌文件,
   由 CCFileClass 按名在已挂载 mix 内寻得(嵌套 mix 按名挂载即此机制)。
3. **缺文件槽行为**:索引分配先于任何文件 IO——0x546092 起,运行计数按
   `TilesInSet`(或 `LastTilesInSet`,见下)前进,无回滚/无移位;文件全 miss
   时槽 0 仍建 0x30C 字节对象并注册进全局向量(0xB0F670/0xA8ED28),槽 1..N−1
   链到前一瓦片(+0x2F0 计数),继续枚举,**不报错不弹窗**。⇒ 未出货集合照常
   占满 N 个全局槽(§3 假设证实)。`LastTilesInSet` 机制:存在且≠TilesInSet
   时计数按 Last 前进,{base+Last, Tiles−Last} 对入 0xAA1078 向量,循环后经
   0xAA1140 表重映射 TOTEMPERATETHEATER/TOSNOWTHEATER 目标。**六 md INI 普查
   (verify6):0 集携带该键**——本语料 ΣTilesInSet 即引擎真容量,重映射路径死
   代码;INI 键全集=FILENAME/SETNAME/TILESINSET/MARBLEMADNESS/NONMARBLEMADNESS/
   MORPHABLE/LOWRADARCOLOR/HIGHRADARCOLOR/ALLOWBURROWING/ALLOWTIBERIUM/
   ALLOWTOPLACE/REQUIREDFORRMG/TOTEMPERATETHEATER/TOSNOWTHEATER。
4. **Theater::Array = VA 0x7E1B78(6×112B)**,记录布局:+0x00 名/​+0x10 UI 标/
   +0x30 文件 stem/​+0x3A ISO stem/​+0x44 第二 ISO stem/​+0x4E 3 字符扩展/
   +0x52 MM 后缀/​+0x56 字母码/​+0x58..0x64 环境光 4 float(M4 渲染金料:SNOW
   绿 0.8/蓝 1.8125 等)/​+0x68、+0x6C int。六剧场 stem/扩展/md 名与本篇 §3/§8
   表**逐项吻合**(NEWURBAN=URBANN/.UBN 独立铁证);**容量非二进制常量**,
   纯运行期 ΣTilesInSet(与 3 自洽,数值只能语料定)。

仍开放(不阻塞实现):
1. 幻影 digest 的 XCC 判别语义(非加密 TS mix 摘要位含义);
2. terrain_type/ramp_type 值表逐值复核;
3. warub* 族文件身份(ovrpsb* 已由 MadHQ 消解为 Wood Bridges 瓦片);
4. Theater::Init 五 mix 挂载中 rec+0x4E stem("%s.MIX"→"SNO.MIX"型)与
   snow.mix(#A226651E)命名关系(对按名查找无影响,记录待考)。

## 10. 出处

- 控制器裁决探针(五轮,可复跑):`%TEMP%\m3verify\verify.py`(INI 容量/锚点/
  SHP 帧闭合/clear01 闭合)、verify2.py(47 容器全扫/扩展名探测)、verify3.py
  (占位口径六剧场容量/MadHQ/缺失细分)、verify4.py(容器正名/md 替换证明/
  [General] 表)、verify5.py(NEWURBAN 扩展名钉死)。python -I。
- 实证组:`%TEMP%\ra2_m3_probe\probe2/9/16/17/21/22/23`;考据组:`%TEMP%\ra2-m3-out\`、
  `%TEMP%\ra2-m3-scripts\`。
- M1b 研究 §2(tmp.rs 布局——本篇 §4 已纠正其定性)、M2 T9/T10 锚点。
- XCC 为 GPL 源码:源自其的结构/算法须语义重写并注明出处;已泄漏 TS/RA2 源码
  绝不上碰(红线,本篇未用)。
