// SPDX-License-Identifier: GPL-3.0-only

//! M4a 地形整图渲染:无头合成器(spec docs/superpowers/specs/
//! 2026-11-06-m4a-terrain-render-design.md)。纯计算零 I/O;工具层
//! (`ra2-map render`)负责读资产/写 PNG。

pub mod terrain;

/// 敌意画布上界(spec §5):语料最大 272+158=430 格 → 12,900×6,450
/// ≈ 83.2 Mpx,护栏 2²⁷ 给 1.6× 余量;4096² 敌意图(约 30 Gpx)拒绝。
pub const MAX_RENDER_PIXELS: u64 = 134_217_728;

/// 等距格像素常数(geometry.h:11-12 ISO_TILE_PIXEL_W/H)。
pub const CELL_W: usize = 60;
pub const CELL_H: usize = 30;
/// 半格步长(Rectangular_To_Isometric tactical.cpp 0x006D1EB0:
/// 60/2 与 30/2,lepton/256 后)。
pub const CELL_DX: usize = 30;
pub const CELL_DY: usize = 15;
/// 每级高度像素(LEVEL_LEPTON_H=104 × Z_Lepton_To_Pixel 0x006D20E0,
/// z∈0..14 全档含 728 fudge 精确等于 15·z)。
pub const Z_STEP: usize = 15;

/// 整图渲染统计(spec §5:瓦片链任一环缺不中止,按格计数)。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RenderStats {
    /// 遍历格数(w×h 全量,含无记录格)。
    pub cells: usize,
    /// 无瓦片格(tile=-1)或无记录。
    pub no_tile: usize,
    /// 成功落像素。
    pub painted: usize,
    /// INI 占位槽(SlotFile::Reserved,画背景)。
    pub reserved: usize,
    /// 文件名界内但 mix 未出货。
    pub missing_file: usize,
    /// tile_id 越界 INI 槽位。
    pub out_of_range: usize,
    /// TMP 槽空(sub_tile 越界/空槽)。
    pub missing_slot: usize,
    /// TMP 解析失败或病态画布(非 2:1/阶梯不闭合)。
    pub parse_fail: usize,
}

/// 合成结果:行紧排 RGB(3·width·height)+ 统计。
pub struct TerrainRender {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
    pub stats: RenderStats,
}
