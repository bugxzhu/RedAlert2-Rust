// SPDX-License-Identifier: GPL-3.0-only

//! 地形层合成(M4a 第一层;M4b 的 overlay/物件层在本画布结构上追加)。

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

use super::{RenderStats, TerrainRender, CELL_DX, CELL_DY, MAX_RENDER_PIXELS, Z_STEP};
use crate::format::map::iso::IsoTile;
use crate::format::map::MapFile;
use crate::format::theater::{theater_spec, SlotFile, TheaterAssets, TheaterIni};
use crate::format::theater_tmp::TsTmpFile;
use crate::format::FormatError;

/// 整图合成入口(spec §4):剧场表/资产/调色板由调用方装载(CLI 复用
/// extract-tile 的装载链);内部委托 [`compose_terrain`]。
pub fn render_terrain(
    map: &MapFile,
    assets: &TheaterAssets,
    ini: &TheaterIni,
    palette: &Palette,
) -> Result<TerrainRender, FormatError> {
    let (w, h) = map.size_wh();
    let mut cells: Vec<Option<IsoTile>> = vec![None; w as usize * h as usize];
    for y in 0..h {
        for x in 0..w {
            if let Some(t) = map.tiles.cell(x, y) {
                cells[y as usize * w as usize + x as usize] = Some(IsoTile {
                    x,
                    y,
                    tile: t.tile,
                    sub_tile: t.sub_tile,
                    z: t.z,
                });
            }
        }
    }
    let spec = theater_spec(map.map_meta.theater);
    compose_terrain(w, h, &cells, assets, ini, spec.ext, palette)
}

/// 纯合成核(测试直打):行主序 cells,行对角线 d=x+y 升序近平后画。
/// 画布 W=30·(w+h),H=15·(w+h)+15·z_max,原点 ox=30·(h−1)/oy=15·z_max。
pub(crate) fn compose_terrain(
    w: u16,
    h: u16,
    cells: &[Option<IsoTile>],
    assets: &TheaterAssets,
    ini: &TheaterIni,
    ext: &str,
    palette: &Palette,
) -> Result<TerrainRender, FormatError> {
    let (w, h) = (w as usize, h as usize);
    let z_max = cells.iter().flatten().map(|t| t.z).max().unwrap_or(0) as usize;
    let width = CELL_DX * (w + h);
    let height = CELL_DY * (w + h) + Z_STEP * z_max;
    if width as u64 * height as u64 > MAX_RENDER_PIXELS {
        return Err(FormatError::Invalid(format!(
            "画布 {width}×{height} = {} px 超 MAX_RENDER_PIXELS {MAX_RENDER_PIXELS}(敌意 [Map]Size?)",
            width as u64 * height as u64
        )));
    }
    let oy = Z_STEP * z_max; // x 向原点已并入 sx 公式(h−1+2x−d)
    let bg = palette.rgb8(0);
    let mut rgb = vec![0u8; 3 * width * height];
    // clippy chunks_exact_to_as_chunks(Rust 1.99 deny-by-default):as_chunks_mut
    // 与 chunks_exact_mut 语义一致(余数切片恒空,3·W·H 整除 3)。
    for chunk in rgb.as_chunks_mut::<3>().0 {
        chunk.copy_from_slice(&bg);
    }
    let mut stats = RenderStats {
        cells: cells.len(),
        ..RenderStats::default()
    };
    // 行对角线序:d = x+y 升序(近平后画;同对角线互不重叠)。
    for d in 0..(w + h).saturating_sub(1) {
        let x_lo = d.saturating_sub(h - 1);
        let x_hi = d.min(w - 1);
        for x in x_lo..=x_hi {
            let Some(t) = cells[(d - x) * w + x] else {
                stats.no_tile += 1; // 界内无记录(口径含无瓦片格)
                continue;
            };
            if t.tile < 0 {
                stats.no_tile += 1;
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
                                // sx = ox + 30·(x−y),y = d−x → 2x−d;并入 ox:
                                // h−1+2x−d ∈ [0, w+h−2] 恒非负,免 isize
                                let sx = CELL_DX * (h - 1 + 2 * x - d);
                                // sy = oy + 15·d − 15·z;oy ≥ 15·z 恒成立,
                                // 左结合求值无下溢
                                let sy = oy + CELL_DY * d - Z_STEP * t.z as usize;
                                if cx as usize != 2 * cy as usize
                                    || !blit_diamond(
                                        &mut rgb,
                                        width,
                                        sx,
                                        sy,
                                        cx as usize,
                                        cy as usize,
                                        palette,
                                        slot.diamond_pixels(),
                                    )
                                {
                                    stats.parse_fail += 1; // 非 2:1/阶梯不闭合
                                } else {
                                    stats.painted += 1;
                                }
                            }
                        },
                    }
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

    /// 画布锚点:2×1 图(ox = 30·(h−1) = 0),(0,0) 格铺在 sx=0/sy=0;
    /// 菱形行 0 宽 4、x0=(60−4)/2=28 → 画布列 28..31 色 = rgb8(9)。
    #[test]
    #[allow(clippy::identity_op)] // 9>>4=0 是 6→8 位展开式的合法常量档(值不变)
    fn projection_and_canvas_anchors() {
        let pal = test_palette();
        let assets = tiny_assets(&[("tst01.ubn".to_string(), tmp60(9))]);
        let ini = tiny_ini();
        // 行主序 y·w+x:(0,0) 有瓦片,(1,0) 无记录
        let cells = vec![Some(tile(0, 0, 0, 0, 0)), None];
        let r = compose_terrain(2, 1, &cells, &assets, &ini, "ubn", &pal).unwrap();
        // W = 30·(2+1) = 90,H = 15·(2+1) + 15·z_max(0) = 45
        assert_eq!((r.width, r.height), (90, 45));
        assert_eq!(r.stats.cells, 2);
        assert_eq!(r.stats.painted, 1);
        assert_eq!(r.stats.no_tile, 1); // 无记录口径并入 no_tile
        let px = |x: usize, y: usize| -> [u8; 3] {
            let i = 3 * (y * r.width as usize + x);
            [r.rgb[i], r.rgb[i + 1], r.rgb[i + 2]]
        };
        let c9 = [(9u8 << 2) | (9 >> 4); 3];
        assert_eq!(px(28, 0), c9, "菱形行 0 首像素(列 28)");
        assert_eq!(px(31, 0), c9, "菱形行 0 末像素(列 31)");
        assert_eq!(px(27, 0), [0, 0, 0], "行外 = 背景 0 号色");
        assert_eq!(px(0, 44), [0, 0, 0], "画布末行 = 背景");
    }

    /// z 语义 = 相对画布顶的下沉:高格 (0,0)@z 钉在 sy=0(oy−15z 恰抵消),
    /// 地面格 (1,0)@z=0 随 oy = 15·z_max 下沉;H 同涨 15·z_max。
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
            let cells = vec![
                Some(tile(0, 0, 0, 0, z)), // (0,0) d=0 高格
                Some(tile(1, 0, 1, 0, 0)), // (1,0) d=1 地面格
            ];
            let r = compose_terrain(2, 1, &cells, &assets, &ini, "ubn", &pal).unwrap();
            assert_eq!(r.height as usize, 15 * 3 + 15 * z as usize, "z={z}");
            let w = r.width as usize;
            // (0,0):sy = oy + 0 − 15·z = 0 恒;行 0 列 28 = rgb8(9)
            let i = 3 * 28;
            assert_eq!(r.rgb[i], (9 << 2) | (9 >> 4), "z={z} 高格钉顶");
            // (1,0):sx = 30,sy = oy + 15 − 0 = 15·z + 15;行 0 列 58 = rgb8(20)
            let j = 3 * ((15 * z as usize + 15) * w + 58);
            assert_eq!(r.rgb[j], (20 << 2) | (20 >> 4), "z={z} 地面格下沉 15·z");
        }
    }

    /// 缺环全链各计其数、不中止;七类计数恒等(cells = 和)。
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
        let cells = vec![
            Some(tile(0, 0, -1, 0, 0)), // no_tile(tile=-1)
            Some(tile(1, 0, 0, 0, 0)),  // painted(tst01)
            Some(tile(0, 1, 1, 0, 0)),  // parse_fail(tst02 敌意)
            Some(tile(1, 1, 2, 0, 0)),  // missing_file(tst03 未出货)
            Some(tile(0, 2, 3, 0, 0)),  // reserved(set0001)
            Some(tile(1, 2, 9, 0, 0)),  // out_of_range(越 4 槽)
            Some(tile(0, 3, 0, 5, 0)),  // missing_slot(cbx·cby=1,槽 5 空)
            None,                       // 无记录 → no_tile
        ];
        let r = compose_terrain(2, 4, &cells, &assets, &ini, "ubn", &pal).unwrap();
        assert_eq!(r.stats.cells, 8);
        assert_eq!(r.stats.painted, 1);
        assert_eq!(r.stats.no_tile, 2);
        assert_eq!(r.stats.parse_fail, 1);
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
            "七类计数恒等"
        );
    }

    /// 4096×4096 敌意图 → 护栏先于分配与遍历拒绝(不触 cells 索引)。
    #[test]
    fn hostile_canvas_guard() {
        let pal = test_palette();
        let assets = tiny_assets(&[]);
        let ini = tiny_ini();
        let cells = vec![None; 64]; // 刻意短于 w·h:护栏必须先于遍历
        assert!(matches!(
            compose_terrain(4096, 4096, &cells, &assets, &ini, "ubn", &pal),
            Err(FormatError::Invalid(_))
        ));
    }

    /// 行对角线序:等高菱形完美镶嵌无争用,必须用 z 差诱发重叠——远格
    /// (0,0) z=0 与近格 (1,0) z=1 的 sy 同为 15(oy=15 近格抬升恰抵),
    /// 行带 r=15 处远格覆盖列 2..57、近格覆盖列 32..87 → 列 32..57 争用,
    /// 后画的近格(d=1)必须胜;列 20(仅远格覆盖)保留远格色。
    #[test]
    fn diagonal_painters_order() {
        let pal = test_palette();
        let assets = tiny_assets(&[
            ("tst01.ubn".to_string(), tmp60(1)),
            ("tst02.ubn".to_string(), tmp60(30)),
        ]);
        let ini = tiny_ini();
        let cells = vec![
            Some(tile(0, 0, 0, 0, 0)), // (0,0) d=0 远格 z=0(色 1)
            Some(tile(1, 0, 1, 0, 1)), // (1,0) d=1 近格 z=1(色 30)
        ];
        let r = compose_terrain(2, 1, &cells, &assets, &ini, "ubn", &pal).unwrap();
        // W=90,H = 15·3+15·1 = 60;两格 sy 均 = 15,行 15..44
        assert_eq!(r.height as usize, 60);
        let w = r.width as usize;
        // 争用带像素(行 30 = 两菱形 r=15,列 40):近平后画胜 → rgb8(30)
        let i = 3 * (30 * w + 40);
        assert_eq!(r.rgb[i], (30 << 2) | (30 >> 4), "争用带近格后画胜");
        // 远格专属区(行 20 = 远格 r=5 覆盖 18..41,近格 r=5 覆盖 48..71):
        // 列 20 保留远格色 rgb8(1)
        let j = 3 * (20 * w + 20);
        let c = 1u8; // rgb8(1) 经变量计算,字面量 1>>4=0 会触发 clippy identity_op
        assert_eq!(r.rgb[j], (c << 2) | (c >> 4), "远格专属区不被覆写");
    }
}
