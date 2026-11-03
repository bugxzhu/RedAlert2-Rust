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

## ra2-mix

    ra2-mix list <file.mix>
    ra2-mix extract <file.mix> <文件名> -o <目录|->
    ra2-mix cat <file.mix> <文件名>
