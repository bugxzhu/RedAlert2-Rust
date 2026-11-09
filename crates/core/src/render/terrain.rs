// SPDX-License-Identifier: GPL-3.0-only

//! 地形层合成(M4a 第一层;M4b 的 overlay/物件层在本画布结构上追加)。
//! 格域 = M2 裁定的菱形走位(存储 (x,y) 权威,研究 2026-11-04 §3.2/§3.6),
//! 非 w×h 矩形;画布尺寸由走位极值决定。

use crate::format::pal::Palette;

/// B 族阶梯行宽:行 r 宽 `4·min(r+1, cy−1−r)`,末行宽 0——引擎 .data
/// 两张 60×30 mask 表(0x3E2B5C/0x3EC4C8)行宽铁证。
pub fn row_width(r: usize, cy: usize) -> usize {
    4 * (r + 1).min(cy - 1 - r)
}

/// 菱形铺入 cx×cy 画布(单瓦片出图路径):行外区 = 调色板 0 号色;
/// 偶 cy 阶梯总耗 `4k² = cx·cy/2` 精确闭合,奇 cy 耗 `cx·cy/2−1` →
/// `None`(病态输入显式失败,不静默漏绘)。
pub fn paint_canvas(palette: &Palette, cx: usize, cy: usize, diamond: &[u8]) -> Option<Vec<u8>> {
    let bg = palette.rgb8(0);
    let mut rgb = Vec::with_capacity(3 * cx * cy);
    let mut off = 0usize;
    for r in 0..cy {
        let w = row_width(r, cy);
        let x0 = (cx - w) / 2;
        for c in 0..cx {
            let color = if c >= x0 && c < x0 + w {
                let idx = *diamond.get(off)? as usize;
                off += 1;
                palette.rgb8(idx)
            } else {
                bg
            };
            rgb.extend_from_slice(&color);
        }
    }
    (off == diamond.len()).then_some(rgb)
}

use super::{
    RenderStats, TerrainRender, CELL_DX, CELL_DY, CELL_H, CELL_W, MAX_RENDER_PIXELS, Z_STEP,
};
use crate::format::map::iso::IsoTile;
use crate::format::map::MapFile;
use crate::format::theater::{theater_spec, SlotFile, TheaterAssets, TheaterIni};
use crate::format::theater_tmp::TsTmpFile;
use crate::format::FormatError;

/// §3.2 菱形走位极值(研究 2026-11-04 的 XCC walk 伪代码逐字回放):只推
/// (x_line, r_line, y) 三变量、不物化格子,O(W+H) 次迭代。返回发射格全体
/// 上 d = x+y 与 xmy = x−y 的 (d_min, d_max, xmy_min, xmy_max)。
/// 画布尺寸由它决定:每 (w,h) 确定(与记录集无关),验收/CLI 可独立重算
/// 对拍。语义注记:发射格总数 = 行走容量 (2W−1)·H,发射行数 = W+H−1,
/// d 跨度 = 2H−1——闭式在无平台段小图((1,1) 等)不齐,故一律回放。
pub fn walk_extremes(w: u16, h: u16) -> (i64, i64, i64, i64) {
    let (cx, cy) = (i64::from(w), i64::from(h));
    let (mut x_line, mut r_line, mut y) = (cx, cx + 1, 1i64);
    let (mut d_min, mut d_max) = (i64::MAX, i64::MIN);
    let (mut xmy_min, mut xmy_max) = (i64::MAX, i64::MIN);
    while x_line < r_line {
        // 行 y 发射闭式区间 [x_line, r_line−1](伪代码 `for x in x_line..r_line-1`)
        d_min = d_min.min(x_line + y);
        d_max = d_max.max(r_line - 1 + y);
        xmy_min = xmy_min.min(x_line - y);
        xmy_max = xmy_max.max(r_line - 1 - y);
        x_line += if y < cx { -1 } else { 1 };
        r_line += if y < cy {
            1
        } else if y > cy {
            -1
        } else {
            0
        };
        y += 1;
    }
    (d_min, d_max, xmy_min, xmy_max)
}

/// 整图合成入口(spec §4):剧场表/资产/调色板由调用方装载(CLI 复用
/// extract-tile 的装载链);内部委托 [`compose_terrain`]。
pub fn render_terrain(
    map: &MapFile,
    assets: &TheaterAssets,
    ini: &TheaterIni,
    palette: &Palette,
) -> Result<TerrainRender, FormatError> {
    let (w, h) = map.size_wh();
    // 存储 (x,y) 权威(§3.6):迭代 records() 而非 w×h 矩形重构——画布域
    // 是菱形走位(W+H−1 行、(2W−1)·H 格),缺格 = 虚空(不画不计)。
    let mut records = map.tiles.records().to_vec();
    // 画序:d = x+y 升序近平后画(records() 原序 cell_key = y<<16|x,稳定
    // 排序后同 d 保持 y 升序;同 d 格横距恰 60px 无列重叠,同 d 内序不敏感)。
    records.sort_by_key(|t| u32::from(t.x) + u32::from(t.y));
    let spec = theater_spec(map.map_meta.theater);
    compose_terrain(w, h, &records, assets, ini, spec.ext, palette)
}

/// 纯合成核(测试直打):`records` 须已按 d = x+y 升序(render_terrain
/// 稳定排序)。画布由走位极值决定(walk_extremes 回放,与记录集无关):
///   W_px = 30·(xmy_max − xmy_min) + 60
///   H_px = 15·(d_max − d_min) + 30 + 15·z_max(z_max = 记录最大 z,空集 0)
/// 投影:sx = 30·(x−y−xmy_min),sy = 15·z_max + 15·(d−d_min) − 15·z
/// (z_max 项给顶部抬升格留白,极值菱形恰贴画布缘,测试钉死零裁切)。
pub(crate) fn compose_terrain(
    w: u16,
    h: u16,
    records: &[IsoTile],
    assets: &TheaterAssets,
    ini: &TheaterIni,
    ext: &str,
    palette: &Palette,
) -> Result<TerrainRender, FormatError> {
    let (w, h) = (w.max(1), h.max(1)); // 防御钳制(iso.rs 同款;size_wh 已保证 ≥1)
    let (d_min, d_max, xmy_min, xmy_max) = walk_extremes(w, h);
    let z_max = records.iter().map(|t| t.z).max().unwrap_or(0) as usize;
    let width = CELL_DX * ((xmy_max - xmy_min) as usize) + 2 * CELL_DX;
    let height = CELL_DY * ((d_max - d_min) as usize) + 2 * CELL_DY + Z_STEP * z_max;
    // 护栏先于分配与遍历(4096² 敌意 → 245,760×122,895 ≈ 30 Gpx 拒)
    if width as u64 * height as u64 > MAX_RENDER_PIXELS {
        return Err(FormatError::Invalid(format!(
            "画布 {width}×{height} = {} px 超 MAX_RENDER_PIXELS {MAX_RENDER_PIXELS}(敌意 [Map]Size?)",
            width as u64 * height as u64
        )));
    }
    let bg = palette.rgb8(0);
    let mut rgb = vec![0u8; 3 * width * height];
    // clippy chunks_exact_to_as_chunks(Rust 1.99 deny-by-default):as_chunks_mut
    // 与 chunks_exact_mut 语义一致(余数切片恒空,3·W·H 整除 3)。
    for chunk in rgb.as_chunks_mut::<3>().0 {
        chunk.copy_from_slice(&bg);
    }
    let mut stats = RenderStats {
        cells: records.len(), // 口径 = 记录数(含 tile=−1;走位缺格不在遍历内)
        ..RenderStats::default()
    };
    // d 升序近平后画;每条记录恰落七类计数器之一(恒等式见 RenderStats)。
    for t in records {
        if t.tile < 0 {
            stats.no_tile += 1; // 无瓦片记录(斜坡边格)
            continue;
        }
        match ini.file_name(t.tile as usize, ext) {
            None => stats.out_of_range += 1,
            Some(SlotFile::Reserved) => stats.reserved += 1,
            Some(SlotFile::File(name)) => {
                let Some(blob) = assets.lookup(&name) else {
                    stats.missing_file += 1;
                    continue;
                };
                match TsTmpFile::parse(blob) {
                    Err(_) => stats.parse_fail += 1,
                    Ok(tmp) => match tmp.slot(t.sub_tile as usize) {
                        None => stats.missing_slot += 1,
                        Some(slot) => {
                            let (_, _, cx, cy) = tmp.dims();
                            let d = i64::from(t.x) + i64::from(t.y);
                            let xmy = i64::from(t.x) - i64::from(t.y);
                            let sx = CELL_DX as i64 * (xmy - xmy_min);
                            let sy = Z_STEP as i64 * z_max as i64 + CELL_DY as i64 * (d - d_min)
                                - Z_STEP as i64 * i64::from(t.z);
                            if cx as usize != 2 * cy as usize {
                                stats.parse_fail += 1; // 非 2:1
                            } else if sx < 0
                                || sy < 0
                                || sx as u64 + CELL_W as u64 > width as u64
                                || sy as u64 + CELL_H as u64 > height as u64
                            {
                                // 存储坐标权威但可病态越出走位域(iso.rs 只验
                                // 1..=w+h−1 包围盒,界内 ≠ 走位格):不落画布
                                // 上溢侧同计(parse_fail):越出走位域的记录可能仅右/下越界(M4a N1)
                                stats.parse_fail += 1;
                            } else if !blit_diamond(
                                &mut rgb,
                                width,
                                sx as usize,
                                sy as usize,
                                cx as usize,
                                cy as usize,
                                palette,
                                slot.diamond_pixels(),
                            ) {
                                stats.parse_fail += 1; // 阶梯不闭合
                            } else {
                                stats.painted += 1;
                            }
                        }
                    },
                }
            }
        }
    }
    Ok(TerrainRender {
        width: width as u32,
        height: height as u32,
        rgb,
        stats,
    })
}

/// 菱形逐像素落画布(只写行内阶梯区;画布背景已统一铺,与单瓦片
/// `paint_canvas` 的填充语义不同)。返回 false = 阶梯消耗与菱形字节量
/// 不符(奇 cy 病态/敌意短缺,调用方计 parse_fail)。
#[allow(clippy::too_many_arguments)] // 投影平移与菱形全参直传(8 参,默认阈值 7)
fn blit_diamond(
    rgb: &mut [u8],
    cw: usize,
    sx: usize,
    sy: usize,
    cx: usize,
    cy: usize,
    palette: &Palette,
    diamond: &[u8],
) -> bool {
    let mut off = 0usize;
    for r in 0..cy {
        let w = row_width(r, cy);
        let x0 = (cx - w) / 2;
        for c in x0..x0 + w {
            let Some(&idx) = diamond.get(off) else {
                return false;
            };
            off += 1;
            // 界内校验防御性(get_mut 饱和):正常路径恒界内(见画布公式)
            let i = 3 * ((sy + r) * cw + sx + c);
            if let Some(dst) = rgb.get_mut(i..i + 3) {
                dst.copy_from_slice(&palette.rgb8(idx as usize));
            }
        }
    }
    off == diamond.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 256 色测试调色板:i 号色 6 位原值 = i&63(rgb8 = 展开),0 号 = 0。
    fn test_palette() -> Palette {
        let mut rgb = [[0u8; 3]; 256];
        for (i, slot) in rgb.iter_mut().enumerate() {
            let v = (i & 63) as u8;
            *slot = [v, v, v];
        }
        Palette { rgb }
    }

    /// B 族阶梯行宽序列 60×30 逐行钉死(mask 表 0x3E2B5C/0x3EC4C8 铁证):
    /// [4,8,…,60,56,…,4,0],和 900 = 60·30/2 精确闭合。
    #[test]
    fn ladder_row_widths_60x30() {
        let widths: Vec<usize> = (0..30).map(|r| row_width(r, 30)).collect();
        let expect: Vec<usize> = (1..=15)
            .map(|i| 4 * i)
            .chain((0..15).rev().map(|i| 4 * i))
            .collect();
        assert_eq!(widths, expect);
        assert_eq!(widths.iter().sum::<usize>(), 900);
        let pal = test_palette();
        assert!(paint_canvas(&pal, 60, 30, &vec![1u8; 900]).is_some());
    }

    /// 8×4 全量字节级钉:行宽 [4,8,4,0](末行宽 0 全背景),x0 = [2,0,2,4]。
    #[test]
    fn canvas_8x4_byte_exact() {
        let pal = test_palette();
        let diamond: Vec<u8> = (1..=16).collect();
        let rgb = paint_canvas(&pal, 8, 4, &diamond).unwrap();
        assert_eq!(rgb.len(), 3 * 8 * 4);
        let px = |r: usize, c: usize| -> [u8; 3] {
            let i = 3 * (r * 8 + c);
            [rgb[i], rgb[i + 1], rgb[i + 2]]
        };
        let widths = [4usize, 8, 4, 0];
        let x0s = [2usize, 0, 2, 4];
        let mut it = 1u8;
        for r in 0..4 {
            for c in 0..8 {
                let inside = (x0s[r]..x0s[r] + widths[r]).contains(&c);
                if inside {
                    let v = it;
                    it += 1;
                    assert_eq!(px(r, c), [(v << 2) | (v >> 4); 3], "({r},{c})");
                } else {
                    assert_eq!(px(r, c), [0, 0, 0], "行外 ({r},{c})");
                }
            }
        }
    }

    /// 奇 cy(6×3):阶梯耗 8/9 → None;偶 cy 菱形短缺同 None(敌意饱和)。
    #[test]
    fn odd_canvas_and_short_diamond_return_none() {
        let pal = test_palette();
        assert!(paint_canvas(&pal, 6, 3, &[9u8; 9]).is_none());
        assert!(paint_canvas(&pal, 8, 4, &[7u8; 15]).is_none());
    }

    // ===== 走位极值与画布公式 =====

    /// §3.2 伪代码物化回放(测试对拍用):按发射序产出全部 (x,y)。
    fn walk_cells(w: u16, h: u16) -> Vec<(u16, u16)> {
        let (cx, cy) = (i64::from(w), i64::from(h));
        let (mut x_line, mut r_line, mut y) = (cx, cx + 1, 1i64);
        let mut out = Vec::new();
        while x_line < r_line {
            for x in x_line..r_line {
                out.push((x as u16, y as u16));
            }
            x_line += if y < cx { -1 } else { 1 };
            r_line += if y < cy {
                1
            } else if y > cy {
                -1
            } else {
                0
            };
            y += 1;
        }
        out
    }

    /// 控制器走位迹钉死:(87,71) d∈[88,229]、x−y∈[−86,86](c1a01md 尺寸);
    /// 无平台段小图 (1,1)/(2,1)/(1,2);两向不对称 (5,3)/(3,5);物化对拍
    /// 逐 (w,h) 验发射格数 = 行走容量 (2W−1)·H 且极值一致;300²/4096²
    /// 不物化直验(i64 数学无溢出)。
    #[test]
    fn walk_extremes_trace_and_edges() {
        assert_eq!(walk_extremes(87, 71), (88, 229, -86, 86));
        assert_eq!(walk_extremes(1, 1), (2, 2, 0, 0));
        assert_eq!(walk_extremes(2, 1), (3, 4, -1, 1));
        assert_eq!(walk_extremes(1, 2), (2, 4, 0, 0));
        assert_eq!(walk_extremes(5, 3), (6, 11, -4, 4));
        assert_eq!(walk_extremes(3, 5), (4, 13, -2, 2));
        for (w, h) in [
            (1u16, 1u16),
            (2, 1),
            (1, 2),
            (3, 2),
            (5, 3),
            (3, 5),
            (87, 71),
        ] {
            let cells = walk_cells(w, h);
            assert_eq!(
                cells.len(),
                (2 * w as usize - 1) * h as usize,
                "({w},{h}) 发射格 = 行走容量"
            );
            let dmin = cells
                .iter()
                .map(|(x, y)| i64::from(*x) + i64::from(*y))
                .min()
                .unwrap();
            let dmax = cells
                .iter()
                .map(|(x, y)| i64::from(*x) + i64::from(*y))
                .max()
                .unwrap();
            let xmn = cells
                .iter()
                .map(|(x, y)| i64::from(*x) - i64::from(*y))
                .min()
                .unwrap();
            let xmx = cells
                .iter()
                .map(|(x, y)| i64::from(*x) - i64::from(*y))
                .max()
                .unwrap();
            assert_eq!(walk_extremes(w, h), (dmin, dmax, xmn, xmx), "({w},{h})");
        }
        assert_eq!(walk_extremes(300, 300), (301, 900, -299, 299));
        assert_eq!(walk_extremes(4096, 4096), (4097, 12288, -4095, 4095));
    }

    /// 画布公式闭合:全走位格在 z ∈ {0, z_max} 的 60×30 菱形包络严格界内,
    /// 且右/下缘必被某极值格恰贴(W/H 无浪费、blit 无裁切)。
    #[test]
    fn canvas_formula_contains_all_walk_cells() {
        for (w, h) in [
            (1u16, 1u16),
            (2, 1),
            (1, 2),
            (5, 3),
            (3, 5),
            (7, 4),
            (4, 7),
            (12, 9),
        ] {
            let (d_min, d_max, xmy_min, xmy_max) = walk_extremes(w, h);
            for z_top in [0i64, 14] {
                let width = (30 * (xmy_max - xmy_min) + 60) as usize;
                let height = (15 * (d_max - d_min) + 30 + 15 * z_top) as usize;
                let mut touched_right = false;
                let mut touched_bottom = false;
                for (x, y) in walk_cells(w, h) {
                    let d = i64::from(x) + i64::from(y);
                    let xmy = i64::from(x) - i64::from(y);
                    for z in [0, z_top] {
                        let sx = (30 * (xmy - xmy_min)) as usize;
                        let sy = (15 * z_top + 15 * (d - d_min) - 15 * z) as usize;
                        assert!(sx + 60 <= width, "({w},{h}) zmax={z_top} ({x},{y}) 右溢");
                        assert!(sy + 30 <= height, "({w},{h}) zmax={z_top} ({x},{y}) 下溢");
                    }
                    let sx = (30 * (xmy - xmy_min)) as usize;
                    let sy = (15 * z_top + 15 * (d - d_min)) as usize; // z=0 贴底
                    touched_right |= sx + 60 == width;
                    touched_bottom |= sy + 30 == height;
                }
                assert!(touched_right, "({w},{h}) zmax={z_top} 应有格恰贴右缘");
                assert!(touched_bottom, "({w},{h}) zmax={z_top} 应有格恰贴下缘");
            }
        }
    }

    // ===== compose_terrain 夹具(手法同 format/theater.rs 测试)=====

    /// flags=0 最小合法 MIX(按名条目)。
    fn make_mix(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        let body_len: usize = entries.iter().map(|(_, c)| c.len()).sum();
        v.extend_from_slice(&(body_len as u32).to_le_bytes());
        let mut off = 0u32;
        for (name, c) in entries {
            v.extend_from_slice(&crate::format::crc::mix_name_id(name).to_le_bytes());
            v.extend_from_slice(&off.to_le_bytes());
            v.extend_from_slice(&(c.len() as u32).to_le_bytes());
            off += c.len() as u32;
        }
        for (_, c) in entries {
            v.extend_from_slice(c);
        }
        v
    }

    /// 单槽 60×30 TMP:头 {1,1,60,30} + 槽表 1 项(偏移 20 = 16+4·1,文件
    /// 基相对)+ 52B 全零槽头(bitfield=0:无 extra 无 z)+ 900B 菱形全 =
    /// fill。总长 20+52+900 = 972,clear01.sno 同构去 z。
    fn tmp60(fill: u8) -> Vec<u8> {
        let mut v = Vec::new();
        for i32v in [1i32, 1, 60, 30, 20] {
            v.extend_from_slice(&i32v.to_le_bytes());
        }
        v.extend_from_slice(&[0u8; 52]);
        v.extend_from_slice(&vec![fill; 900]);
        v
    }

    fn tile(x: u16, y: u16, tile: i16, sub: u8, z: u8) -> IsoTile {
        IsoTile {
            x,
            y,
            tile,
            sub_tile: sub,
            z,
        }
    }

    fn tiny_assets(tmps: &[(String, Vec<u8>)]) -> TheaterAssets {
        let d = std::env::temp_dir().join(format!(
            "ra2-render-t{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let refs: Vec<(&str, &[u8])> = tmps
            .iter()
            .map(|(n, b)| (n.as_str(), b.as_slice()))
            .collect();
        // open() 必需 ra2md.mix 与 ra2.mix(theater.rs:369-374);瓦片放
        // ra2md(pos 100,优先于 ra2 的 101),无 expandmd/ecache 遮蔽
        std::fs::write(d.join("ra2.mix"), make_mix(&[])).unwrap();
        std::fs::write(d.join("ra2md.mix"), make_mix(&refs)).unwrap();
        TheaterAssets::open(&d).unwrap()
    }

    fn tiny_ini() -> TheaterIni {
        TheaterIni::parse("[TileSet0000]\nFileName=tst\nTilesInSet=2\n").unwrap()
    }

    /// 画布锚点:2×1 图只存 (2,1) 一条记录(另两走位格 (1,2)/(2,2) 虚空,
    /// 不画不计)。走位极值 (3,4,−1,1) → W = 30·2+60 = 120,H = 15·1+30 = 45;
    /// sx = 30·(1−(−1)) = 60,sy = 0;菱形行 0 宽 4、x0 = 28 → 列 88..91。
    #[test]
    #[allow(clippy::identity_op)] // 9>>4=0 是 6→8 位展开式的合法常量档(值不变)
    fn projection_and_canvas_anchors() {
        let pal = test_palette();
        let assets = tiny_assets(&[("tst01.ubn".to_string(), tmp60(9))]);
        let ini = tiny_ini();
        let records = vec![tile(2, 1, 0, 0, 0)];
        let r = compose_terrain(2, 1, &records, &assets, &ini, "ubn", &pal).unwrap();
        assert_eq!((r.width, r.height), (120, 45));
        assert_eq!(r.stats.cells, 1, "cells = 记录口径,缺格不计数");
        assert_eq!(r.stats.painted, 1);
        assert_eq!(r.stats.no_tile, 0);
        let px = |x: usize, y: usize| -> [u8; 3] {
            let i = 3 * (y * r.width as usize + x);
            [r.rgb[i], r.rgb[i + 1], r.rgb[i + 2]]
        };
        let c9 = [(9u8 << 2) | (9 >> 4); 3];
        assert_eq!(px(88, 0), c9, "菱形行 0 首像素(列 88)");
        assert_eq!(px(91, 0), c9, "菱形行 0 末像素(列 91)");
        assert_eq!(px(87, 0), [0, 0, 0], "行外 = 背景 0 号色");
        assert_eq!(px(0, 44), [0, 0, 0], "画布末行 = 背景");
    }

    /// z 语义 = 相对画布顶的下沉:高格 (2,1)@z 钉在 sy=0(z_max 项恰抵消),
    /// 地面格 (2,2)@z=0 随 z_max 下沉;H 同涨 15·z_max。
    /// z∈{0,1,7,14}:7 含 728 fudge 档,14 = 语料 z 上界。
    #[test]
    #[allow(clippy::identity_op)] // 9>>4=0 是 6→8 位展开式的合法常量档(值不变)
    fn z_shifts_15px_per_level() {
        let pal = test_palette();
        let assets = tiny_assets(&[
            ("tst01.ubn".to_string(), tmp60(9)),
            ("tst02.ubn".to_string(), tmp60(20)),
        ]);
        let ini = tiny_ini();
        for z in [0u8, 1, 7, 14] {
            // d 升序:(2,1) d=3 远格,(2,2) d=4 近格
            let records = vec![tile(2, 1, 0, 0, z), tile(2, 2, 1, 0, 0)];
            let r = compose_terrain(2, 1, &records, &assets, &ini, "ubn", &pal).unwrap();
            assert_eq!(r.height as usize, 15 * 1 + 30 + 15 * z as usize, "z={z}");
            let w = r.width as usize;
            // (2,1):sx = 30·(1+1) = 60,sy = 15z + 0 − 15z = 0 恒;行 0 列 88
            let i = 3 * 88;
            assert_eq!(r.rgb[i], (9 << 2) | (9 >> 4), "z={z} 高格钉顶");
            // (2,2):sx = 30·(0+1) = 30,sy = 15z + 15 − 0;行首列 58 = rgb8(20)
            let j = 3 * ((15 * z as usize + 15) * w + 58);
            assert_eq!(r.rgb[j], (20 << 2) | (20 >> 4), "z={z} 地面格下沉 15·z");
        }
    }

    /// 缺环全链各计其数、不中止;七类计数恒等(cells = 和,记录口径)。
    /// 含 N1 仅上溢越域一条(走位域外但包围盒内,右缘越界)。
    #[test]
    fn missing_chain_counts() {
        let pal = test_palette();
        let mut bad = tmp60(9);
        bad.truncate(30); // 槽声明偏移 20 但体只余 10B → parse Invalid
        let assets = tiny_assets(&[
            ("tst01.ubn".to_string(), tmp60(9)),
            ("tst02.ubn".to_string(), bad),
        ]);
        let ini = TheaterIni::parse(
            "[TileSet0000]\nFileName=tst\nTilesInSet=3\n[TileSet0001]\nTilesInSet=1\n",
        )
        .unwrap(); // set0001 无 FileName → 槽 3 = Reserved;全局槽位共 4
                   // 3×2 图的 8 条记录(d 升序;走位域含 (3,1),(2,2),(1,3),
                   // (3,2),…;(4,1) 在包围盒内但走位域外,仅右缘越界 = N1)
        let records = vec![
            tile(3, 1, -1, 0, 0), // no_tile(tile=-1)
            tile(2, 2, 0, 0, 0),  // painted(tst01)
            tile(1, 3, 3, 0, 0),  // reserved(set0001 占位)
            tile(4, 1, 0, 0, 0),  // parse_fail(N1 仅上溢越域:右缘出画布)
            tile(3, 2, 1, 0, 0),  // parse_fail(tst02 敌意)
            tile(2, 3, 9, 0, 0),  // out_of_range(越 4 槽)
            tile(4, 2, 2, 0, 0),  // missing_file(tst03 未出货)
            tile(3, 3, 0, 5, 0),  // missing_slot(cbx·cby=1,槽 5 空)
        ];
        let r = compose_terrain(3, 2, &records, &assets, &ini, "ubn", &pal).unwrap();
        assert_eq!((r.width, r.height), (180, 75)); // 极值 (4,7,−2,2) 口径
        assert_eq!(r.stats.cells, 8);
        assert_eq!(r.stats.no_tile, 1);
        // N1:仅上溢越域(右/下缘越界)→ parse_fail,不再计 painted
        assert_eq!(r.stats.painted, 1);
        assert_eq!(r.stats.parse_fail, 2);
        assert_eq!(r.stats.missing_file, 1);
        assert_eq!(r.stats.reserved, 1);
        assert_eq!(r.stats.out_of_range, 1);
        assert_eq!(r.stats.missing_slot, 1);
        let s = r.stats;
        assert_eq!(
            s.cells,
            s.painted
                + s.no_tile
                + s.parse_fail
                + s.missing_file
                + s.reserved
                + s.out_of_range
                + s.missing_slot,
            "七类计数恒等(每记录恰一计数器)"
        );
    }

    /// 4096×4096 敌意图 → 护栏先于分配与遍历拒绝(不触 records 索引):
    /// 走位极值口径 W = 245,760、H = 122,895 ≈ 30 Gpx;同式合法域
    /// (87×71 = c1a01md 尺寸,空记录)不拒 → 5220×2145 ≈ 11.2 Mpx < 2²⁷。
    #[test]
    fn hostile_canvas_guard() {
        let pal = test_palette();
        let assets = tiny_assets(&[]);
        let ini = tiny_ini();
        let records: Vec<IsoTile> = Vec::new();
        assert!(matches!(
            compose_terrain(4096, 4096, &records, &assets, &ini, "ubn", &pal),
            Err(FormatError::Invalid(_))
        ));
        let r = compose_terrain(87, 71, &records, &assets, &ini, "ubn", &pal).unwrap();
        assert_eq!((r.width, r.height), (5220, 2145));
        assert_eq!(r.stats.cells, 0);
    }

    /// 行对角线序:等高同 d 菱形横距 60 无争用,必须用 z 差诱发重叠——远格
    /// (2,1) z=0(d=3)与近格 (2,2) z=1(d=4)的 sy 同为 15(z_max=1 项恰抵),
    /// 行带 r=15 处远格覆盖列 62..117、近格覆盖列 32..87 → 列 62..87 争用,
    /// 后画的近格(d=4)必须胜;列 100(仅远格覆盖)保留远格色。
    #[test]
    fn diagonal_painters_order() {
        let pal = test_palette();
        let assets = tiny_assets(&[
            ("tst01.ubn".to_string(), tmp60(1)),
            ("tst02.ubn".to_string(), tmp60(30)),
        ]);
        let ini = tiny_ini();
        let records = vec![
            tile(2, 1, 0, 0, 0), // (2,1) d=3 远格 z=0(色 1)
            tile(2, 2, 1, 0, 1), // (2,2) d=4 近格 z=1(色 30)
        ];
        let r = compose_terrain(2, 1, &records, &assets, &ini, "ubn", &pal).unwrap();
        // W=120,H = 15·1+30+15·1 = 60;两格 sy 均 = 15,行 15..44
        assert_eq!(r.height as usize, 60);
        let w = r.width as usize;
        // 争用带像素(行 30 = 两菱形 r=15,列 70):近平后画胜 → rgb8(30)
        let i = 3 * (30 * w + 70);
        assert_eq!(r.rgb[i], (30 << 2) | (30 >> 4), "争用带近格后画胜");
        // 远格专属区(行 30 = 远格 r=15 覆盖 62..117,近格 r=15 覆盖 32..87):
        // 列 100 保留远格色 rgb8(1)
        let j = 3 * (30 * w + 100);
        let c = 1u8; // rgb8(1) 经变量计算,字面量 1>>4=0 会触发 clippy identity_op
        assert_eq!(r.rgb[j], (c << 2) | (c >> 4), "远格专属区不被覆写");
    }

    /// 全菱形覆盖(3×2 图 = (2·3−1)·2 = 10 记录全存、全可绘):每格恰落画,
    /// 菱形行 0 采样点两两不同(后画格最低下移 15 行,永不覆写先画格行 0)
    /// 且全为瓦片色;cells = 10 = 记录口径。
    #[test]
    fn full_diamond_paint_3x2() {
        let pal = test_palette();
        let assets = tiny_assets(&[("tst01.ubn".to_string(), tmp60(9))]);
        let ini = tiny_ini();
        let mut records: Vec<IsoTile> = walk_cells(3, 2)
            .into_iter()
            .map(|(x, y)| tile(x, y, 0, 0, 0))
            .collect();
        records.sort_by_key(|t| u32::from(t.x) + u32::from(t.y));
        assert_eq!(records.len(), (2 * 3 - 1) * 2);
        let r = compose_terrain(3, 2, &records, &assets, &ini, "ubn", &pal).unwrap();
        let (d_min, _, xmy_min, _) = walk_extremes(3, 2);
        assert_eq!((r.width, r.height), (180, 75));
        assert_eq!(r.stats.cells, 10);
        assert_eq!(r.stats.painted, 10);
        assert_eq!(r.stats.no_tile + r.stats.parse_fail, 0);
        let v = 9u8; // rgb8(9) 经变量计算,字面量 9>>4=0 会触发 clippy identity_op
        let c9 = [(v << 2) | (v >> 4); 3];
        let mut samples = std::collections::BTreeSet::new();
        for t in &records {
            let sx = (30 * (i64::from(t.x) - i64::from(t.y) - xmy_min)) as usize;
            let sy = (15 * (i64::from(t.x) + i64::from(t.y) - d_min)) as usize;
            let i = 3 * (sy * r.width as usize + sx + 28);
            assert_eq!(
                [r.rgb[i], r.rgb[i + 1], r.rgb[i + 2]],
                c9,
                "({},{}) 菱形行 0 未落瓦片色",
                t.x,
                t.y
            );
            assert!(
                samples.insert((sx + 28, sy)),
                "({},{}) 采样点与他格重合",
                t.x,
                t.y
            );
        }
    }
}
