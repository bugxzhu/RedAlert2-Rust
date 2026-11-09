// SPDX-License-Identifier: GPL-3.0-only

//! M4a 地形整图渲染:无头合成器(spec docs/superpowers/specs/
//! 2026-11-06-m4a-terrain-render-design.md)。纯计算零 I/O;工具层
//! (`ra2-map render`)负责读资产/写 PNG。

pub mod overlay;
pub mod shape;
pub mod terrain;

/// 敌意画布上界(spec §5):走位极值画布口径,语料最大 [Map]Size 对角 430
/// (272×158)→ 16,320×4,965 ≈ 81 Mpx(护栏余量 1.66×);合法方图上限
/// 272×272(16,320×8,205 ≈ 133.4 Mpx,恰过护栏)——60w·(30w+15) ≤ 2²⁷
/// 解得 w ≤ 272.06;4096² 敌意图(约 30 Gpx)拒绝。
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

/// 整图渲染统计(spec §5:瓦片链任一环缺不中止)。语义(M4a 终审重裁定):
/// 记录口径——cells 为 records().len(),每条记录恰落七类计数器之一
/// (恒等式 cells = 其和);走位缺格 = 虚空,不画不计(旧 w×h 矩形域的
/// "界内无记录"计数含义随域改废除)。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RenderStats {
    /// 记录总数(tiles.records().len(),含 tile=−1)。
    pub cells: usize,
    /// 无瓦片记录(tile=−1,斜坡边格)。
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
    /// TMP 解析失败、病态画布(非 2:1/阶梯不闭合)或记录越出走位域。
    pub parse_fail: usize,
}

/// 整图三层合成统计(M4b spec §5;T5/T6 依赖,字段集冻结):地形层
/// RenderStats 原样内嵌;overlay/物件层各计其数、任一环缺不中止;
/// frame_clamp/clip 为链路成功的信息位(非失败)。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SceneStats {
    pub terrain: RenderStats,
    /// overlay 成功落画布(含画布外/裁剪:链路成功,clip 另计)。
    pub overlay_drawn: usize,
    /// 0xFF 之外的越表位/表缺。
    pub overlay_no_type: usize,
    pub overlay_missing_file: usize,
    /// SHP 解析败(跳过)。
    pub overlay_fail: usize,
    pub objects_drawn: usize,
    pub objects_skipped_voxel: usize,
    pub objects_missing_art: usize,
    /// 损坏行/坐标·类型解析失败。
    pub objects_malformed: usize,
    /// SHP 解析败(跳过)。
    pub objects_fail: usize,
    /// 帧号越界钳末帧(overlay+物件,信息位)。
    pub frame_clamp: usize,
    /// 精灵矩形未全入画布(信息位)。
    pub clip: usize,
}

/// SHP 名 → 解析结果缓存(每图一份;Missing/Fail 负缓存防重复查档)。
pub(crate) type ShpCache = std::collections::HashMap<String, shape::CachedShp>;

/// 合成结果:行紧排 RGB(3·width·height)+ 统计。
pub struct TerrainRender {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
    pub stats: RenderStats,
}
