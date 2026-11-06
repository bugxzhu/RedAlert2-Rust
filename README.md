# ra2-rs

红色警戒2(RA2/YR)引擎的 Rust 开源重实现。加载玩家自备的原版资产(MIX/INI/地图)。

- 设计文档:`docs/superpowers/specs/`
- 实施计划:`docs/superpowers/plans/`
- 许可证:GPL-3.0-only。本仓库不分发任何 EA 版权资产。

## 快速开始

    cargo build --release
    cargo test --workspace                 # 合成样本测试(CI 可跑)
    RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored
                                           # 真实资产集成测试(本地)

## 格式层(M1b)

`ra2-core::format` 现可解析:MIX(明文/摘要位索引(咨询位);加密位明确拒绝)、
INI、PAL、SHP(TS/RA2,RLEZeroTS)、TMP(RA2 剧场布局)、CSF、
AUD(IMA ADPCM,XCC 变体步长表)、VXL、HVA。
规格依据见 `docs/superpowers/research/`,实施计划见 `docs/superpowers/plans/`。

## 地图层(M2)

`ra2-core::format::map` 现可解析任意用户自有 `.map`:四个包段
(IsoMapPack5/PreviewPack = LZO1X,Overlay 双段 = LCW)全解码,
瓦片网格/双层覆盖/预览位图/六类对象段结构化,logic 段 tokenize 保留,
[Digest] 读取不校验;807 张真实地图全量解析零 panic。`tools/ra2-map`
提供 `info` 与 `extract-tile`(60×30 菱形占位渲染;tile→TMP 真位图
解析归 M3)。规格依据见 `docs/superpowers/research/`,实施计划见
`docs/superpowers/plans/`。

## 剧场瓦片层(M3)

`ra2-core::format` 闭合瓦片解析链:MIX 摘要位引擎语义修复(位=工具侧咨询
位,从不跳 20;旧格式显式拒绝)、`shp_ts`(TS SHP 帧,RAW 回退)、`theater_tmp`
(TS TMP:16B 头/槽表/52B 头/RAW 菱形 + z)、`theater`(剧场 INI 顺序
探查→全局索引→文件名反查;引擎挂载序资产装载器,递归嵌套 mix 下钻;
六剧场常数表)。807 图全量 (theater, tile_id, sub)→文件闭合,缺文件槽
精确账(SNOW 78/NEWURBAN 383/LUNAR 44,语料零引用),六剧场现存瓦片
全量零 panic。`ra2-map extract-tile` 输出真瓦片位图(2:1 等距菱形、
调色板上色;YR 三剧场调色板缺席时退出 1)。规格依据见
`docs/superpowers/research/`,实施计划见 `docs/superpowers/plans/`。

## ra2-mix

    ra2-mix list <file.mix>
    ra2-mix extract <file.mix> <文件名> -o <目录|->
    ra2-mix cat <file.mix> <文件名>

## ra2-map

    ra2-map render <map.map> -o <out.png> [--assets <dir>]   # M4a 整幅地形渲染 PNG(等距投影/z 接缝/缺环统计;本机真彩限 TEMPERATE/SNOW/URBAN)
