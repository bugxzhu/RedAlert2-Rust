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

`ra2-core::format` 现可解析:MIX(明文/SHA-1 摘要位索引;加密位明确拒绝)、
INI、PAL、SHP(TS/RA2,RLEZeroTS)、TMP(RA2 剧场布局)、CSF、
AUD(IMA ADPCM,XCC 变体步长表)、VXL、HVA。
规格依据见 `docs/superpowers/research/`,实施计划见 `docs/superpowers/plans/`。

## ra2-mix

    ra2-mix list <file.mix>
    ra2-mix extract <file.mix> <文件名> -o <目录|->
    ra2-mix cat <file.mix> <文件名>
