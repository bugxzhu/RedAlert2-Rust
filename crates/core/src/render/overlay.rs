// SPDX-License-Identifier: GPL-3.0-only

//! overlay pass(M4b spec §5;研究 §2/§3):records 菱形序(d 升序,scene
//! 已排)逐格查双层网格——字节 = [OverlayTypes] 表位(T1),ore 先游程
//! 变体(cell.cpp:1451),crate 帧特判 0(cell.cpp:1382),桥面板帧 0/9
//! 加 16 项变体表(cell.cpp:1422-1427),帧号 = OverlayData 直传;锚 =
//! 格中心 C(overtype.cpp:167)。桥 NS 偏移 (−15,7) 与桥高 z 附加
//! (cell.cpp:1408/1372-1376)不感知 → 后续里程碑待钉。
//! VEIN 前缀表项守卫跳画(cell.cpp:1401-1403,M4c spec §2;计 drawn
//! 与 Outside 同口径)。

use crate::format::map::iso::IsoTile;
use crate::format::map::MapFile;
use crate::format::pal::Palette;
use crate::format::rules::OverlayTable;
use crate::format::theater::TheaterAssets;
use crate::render::shape::{blit_frame, load_shp, BlitResult, CachedShp};
use crate::render::{SceneStats, ShpCache, CELL_DX, CELL_DY, Z_STEP};

/// 桥面板完整帧变体(cell.cpp:1422-1427,索引 (x&3)|(4·(y&3)))。
const BRIDGE_VARIATION: [usize; 16] = [0, 1, 2, 3, 3, 2, 1, 0, 2, 3, 0, 1, 1, 0, 3, 2];

#[allow(clippy::too_many_arguments)] // 冻结签名 15 参直传
pub(crate) fn paint_overlays(
    map: &MapFile,
    assets: &TheaterAssets,
    table: &OverlayTable,
    palette: &Palette,
    records: &[IsoTile],
    d_min: i64,
    xmy_min: i64,
    z_max: usize,
    ext: &str,
    rgb: &mut [u8],
    cw: usize,
    ch: usize,
    stats: &mut SceneStats,
    cache: &mut ShpCache,
) {
    for t in records {
        let ty = map.overlays.type_at(t.x, t.y);
        if ty == crate::format::rules::OVERLAY_NONE {
            continue;
        }
        let Some(_) = table.kind(ty) else {
            stats.overlay_no_type += 1;
            continue;
        };
        let disp = table.display_byte(ty, t.x, t.y);
        let Some(kind) = table.kind(disp) else {
            stats.overlay_no_type += 1;
            continue;
        };
        // VEIN 前缀守卫(cell.cpp:1401-1403 VEINHOLE/VEINHOLE_DUMMY 直接
        // return):不画、不计失败,计 overlay_drawn(链路成功、像素面空,
        // 与 BlitResult::Outside 同口径)。守卫先于查档(缺档不入
        // missing/fail)。零售 [OverlayTypes] 唯一 VEIN 名 = 表位 178
        // VEINHOLEDUMMY,语料命中 0 格(研究 §4.5)——纯语义对齐,不改
        // 807 总账;rubble 分支语料 0,只记语义不实现(spec §2)。
        if kind.image.starts_with("VEIN") {
            stats.overlay_drawn += 1;
            continue;
        }
        let mut frame = if kind.is_crate {
            0
        } else {
            usize::from(map.overlays.frame_at(t.x, t.y).unwrap_or(0))
        };
        if kind.bridge && (frame == 0 || frame == 9) {
            let i = ((t.x & 3) | (4 * (t.y & 3))) as usize;
            frame += BRIDGE_VARIATION[i];
        }
        let loaded = load_shp(assets, cache, kind.image, ext);
        let CachedShp::File(shp) = loaded else {
            if matches!(loaded, CachedShp::Missing) {
                stats.overlay_missing_file += 1;
            } else {
                stats.overlay_fail += 1;
            }
            continue;
        };
        if frame >= shp.frames.len() {
            frame = shp.frames.len() - 1;
            stats.frame_clamp += 1;
        }
        let d = i64::from(t.x) + i64::from(t.y);
        let xmy = i64::from(t.x) - i64::from(t.y);
        let center = (
            CELL_DX as i64 * (xmy - xmy_min) + CELL_DX as i64,
            Z_STEP as i64 * z_max as i64 + CELL_DY as i64 * (d - d_min)
                - Z_STEP as i64 * i64::from(t.z)
                + CELL_DY as i64,
        );
        match blit_frame(rgb, cw, ch, shp, &shp.frames[frame], center, palette) {
            BlitResult::Outside => {
                stats.clip += 1;
                stats.overlay_drawn += 1; // 链路成功,画布外
            }
            BlitResult::Clipped => {
                stats.clip += 1;
                stats.overlay_drawn += 1;
            }
            BlitResult::Inside => stats.overlay_drawn += 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::theater::{theater_spec, TheaterIni};
    use crate::render::terrain::{compose_terrain, walk_extremes};

    // ===== 夹具:合成剧场 mix(手法同 render/terrain.rs tests)=====

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

    /// open() 必需 ra2.mix + ra2md.mix(terrain.rs tests 同款);资产全放
    /// ra2md(优先级 100),文件名全小写——lookup 大小写不敏感,覆盖
    /// load_shp 的 "{IMAGE}.shp" 大写候选。目录名带测试名:并行线程各自
    /// 独占(纯时间戳在 Windows 粗粒度时钟下会撞目录,互删 mix)。
    fn tiny_assets(tag: &str, files: &[(&str, Vec<u8>)]) -> TheaterAssets {
        let d = std::env::temp_dir().join(format!(
            "ra2-overlay-{tag}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let refs: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (*n, b.as_slice())).collect();
        std::fs::write(d.join("ra2.mix"), make_mix(&[])).unwrap();
        std::fs::write(d.join("ra2md.mix"), make_mix(&refs)).unwrap();
        TheaterAssets::open(&d).unwrap()
    }

    /// 单槽 60×30 TMP(terrain.rs tests 同款):全零槽头 + 900B 菱形 = fill。
    fn tmp60(fill: u8) -> Vec<u8> {
        let mut v = Vec::new();
        for i32v in [1i32, 1, 60, 30, 20] {
            v.extend_from_slice(&i32v.to_le_bytes());
        }
        v.extend_from_slice(&[0u8; 52]);
        v.extend_from_slice(&vec![fill; 900]);
        v
    }

    // ===== 夹具:合成 RAW 形态 SHP-TS =====

    /// 60×30 全 fill 帧体;hollow = 中心两像素(行 15 列 29/30 = 索引
    /// 929/930)置 0(透明)供穿透断言。
    fn body60(fill: u8, hollow: bool) -> Vec<u8> {
        let mut b = vec![fill; 60 * 30];
        if hollow {
            b[15 * 60 + 29] = 0;
            b[15 * 60 + 30] = 0;
        }
        b
    }

    /// N 帧 60×30 RAW SHP:flags=1,bit1=0 → 帧级门控直取 RAW(draw.cpp:88-98
    /// Bit_Blit,M4c 判别式)。均匀填充的严格 RLE 链 `257·v·k = 1800` 无整数
    /// 解,任何判别式下均不被误读为 RLE。帧体按声明顺序背靠背排列。
    fn shp_raw(bodies: &[Vec<u8>]) -> Vec<u8> {
        assert!(!bodies.is_empty());
        let mut v = Vec::new();
        v.extend_from_slice(&0u16.to_le_bytes()); // zero
        v.extend_from_slice(&60u16.to_le_bytes()); // cx
        v.extend_from_slice(&30u16.to_le_bytes()); // cy
        v.extend_from_slice(&(bodies.len() as u16).to_le_bytes()); // count
        let table_end = 8 + 24 * bodies.len();
        for (i, b) in bodies.iter().enumerate() {
            assert_eq!(b.len(), 60 * 30);
            v.extend_from_slice(&0i16.to_le_bytes()); // x
            v.extend_from_slice(&0i16.to_le_bytes()); // y
            v.extend_from_slice(&60u16.to_le_bytes()); // cx
            v.extend_from_slice(&30u16.to_le_bytes()); // cy
            v.extend_from_slice(&1u32.to_le_bytes()); // flags=1:bit1=0 → RAW
            v.extend_from_slice(&[0u8; 4]); // rgba
            v.extend_from_slice(&0u32.to_le_bytes()); // reserved
            v.extend_from_slice(&((table_end + 1800 * i) as u32).to_le_bytes());
        }
        for b in bodies {
            v.extend_from_slice(b);
        }
        v
    }

    // ===== 夹具:mini 地图 + 任意字节 overlay 双层 =====

    fn b64(bytes: &[u8]) -> String {
        const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for c in bytes.as_chunks::<3>().0 {
            let n = ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32;
            s.push(A[(n >> 18) as usize & 63] as char);
            s.push(A[(n >> 12) as usize & 63] as char);
            s.push(A[(n >> 6) as usize & 63] as char);
            s.push(A[n as usize & 63] as char);
        }
        match bytes.len() % 3 {
            1 => {
                let n = (bytes[bytes.len() - 1] as u32) << 16;
                s.push(A[(n >> 18) as usize & 63] as char);
                s.push(A[(n >> 12) as usize & 63] as char);
                s.push_str("==");
            }
            2 => {
                let n = ((bytes[bytes.len() - 2] as u32) << 16)
                    | ((bytes[bytes.len() - 1] as u32) << 8);
                s.push(A[(n >> 18) as usize & 63] as char);
                s.push(A[(n >> 12) as usize & 63] as char);
                s.push(A[(n >> 6) as usize & 63] as char);
                s.push('=');
            }
            _ => {}
        }
        s
    }

    fn entries_text(blob: &[u8]) -> Vec<(String, String)> {
        let text = b64(blob);
        let mut v = Vec::new();
        let mut i = 0usize;
        let mut k = 1usize;
        while i < text.len() {
            let n = (text.len() - i).min(70);
            v.push((k.to_string(), text[i..i + n].to_string()));
            i += n;
            k += 1;
        }
        v
    }

    fn push_pack_section(ini: &mut String, name: &str, blob: &[u8]) {
        ini.push_str(&format!("[{name}]\n"));
        for (k, v) in entries_text(blob) {
            ini.push_str(&k);
            ini.push('=');
            ini.push_str(&v);
            ini.push('\n');
        }
    }

    /// 任意 262144 字节层的 LCW 块流(≤8192/块):0xFE 长填充 +
    /// 0x81..=0xBF 短字面量 + 0x80 终止符(lcw.rs 解码语义的手工对偶;
    /// map/mod.rs tests 的 lcw_fill_stream 只会整层单值填充,本助手支持
    /// 逐格改写的层)。
    fn lcw_layer_stream(buf: &[u8]) -> Vec<u8> {
        assert_eq!(buf.len(), 262144);
        let mut blob = Vec::new();
        for chunk in buf.chunks(8192) {
            let mut p = Vec::new();
            let mut rest = chunk;
            while !rest.is_empty() {
                let b = rest[0];
                let run = rest.iter().take_while(|&&x| x == b).count();
                if run >= 3 {
                    p.extend_from_slice(&[0xFE, run as u8, (run >> 8) as u8, b]);
                    rest = &rest[run..];
                } else {
                    p.push(0x80 | run as u8);
                    p.extend_from_slice(&rest[..run]);
                    rest = &rest[run..];
                }
            }
            p.push(0x80);
            blob.extend_from_slice(&(p.len() as u16).to_le_bytes());
            blob.extend_from_slice(&(chunk.len() as u16).to_le_bytes());
            blob.extend_from_slice(&p);
        }
        blob
    }

    /// 262144 全 0xFF 类型层,逐格改写(行主序 (y<<9)|x)。
    fn types_layer(cells: &[(u16, u16, u8)]) -> Vec<u8> {
        let mut v = vec![0xFFu8; 262144];
        for &(x, y, t) in cells {
            v[(y as usize) << 9 | x as usize] = t;
        }
        v
    }

    /// 262144 全 0 帧层,逐格改写。
    fn data_layer(cells: &[(u16, u16, u8)]) -> Vec<u8> {
        let mut v = vec![0u8; 262144];
        for &(x, y, d) in cells {
            v[(y as usize) << 9 | x as usize] = d;
        }
        v
    }

    /// mini 地图:仅 [Map] + overlay 双包段(MapFile::parse 其余段皆可选)。
    fn overlay_map(size: (u16, u16), types: &[u8], data: &[u8]) -> MapFile {
        let mut ini = String::new();
        ini.push_str(&format!(
            "[Map]\nTheater=SNOW\nSize=0,0,{},{}\n",
            size.0, size.1
        ));
        push_pack_section(&mut ini, "OverlayPack", &lcw_layer_stream(types));
        push_pack_section(&mut ini, "OverlayDataPack", &lcw_layer_stream(data));
        MapFile::parse(&ini.into_bytes()).unwrap()
    }

    // ===== 夹具:走位域 + 地形画布 =====

    fn tile(x: u16, y: u16) -> IsoTile {
        IsoTile {
            x,
            y,
            tile: 0,
            sub_tile: 0,
            z: 0,
        }
    }

    /// §3.2 伪代码物化回放(terrain.rs tests 同款):按发射序产出全部 (x,y)。
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

    /// d 升序全走位记录(渲染画序,compose_terrain/paint_overlays 同域)。
    fn sorted_records(w: u16, h: u16) -> Vec<IsoTile> {
        let mut recs: Vec<IsoTile> = walk_cells(w, h)
            .into_iter()
            .map(|(x, y)| tile(x, y))
            .collect();
        recs.sort_by_key(|t| u32::from(t.x) + u32::from(t.y));
        recs
    }

    fn terrain_ini() -> TheaterIni {
        TheaterIni::parse("[TileSet0000]\nFileName=tst\nTilesInSet=2\n").unwrap()
    }

    /// 256 色调色板(6 位原值 = i&63;0 号 = 背景)。
    fn test_palette() -> Palette {
        let mut rgb = [[0u8; 3]; 256];
        for (i, slot) in rgb.iter_mut().enumerate() {
            let v = (i & 63) as u8;
            *slot = [v, v, v];
        }
        Palette { rgb }
    }

    /// 6→8 位展开期望值(与 Palette::rgb8 同式)。
    fn c8(v: u8) -> [u8; 3] {
        [(v << 2) | (v >> 4); 3]
    }

    /// 守恒恒等(brief §49):记录域内非空 overlay = drawn+no_type+missing_file+fail。
    fn assert_conservation(map: &MapFile, recs: &[IsoTile], stats: &SceneStats) {
        let non_empty = recs
            .iter()
            .filter(|t| map.overlays.type_at(t.x, t.y) != crate::format::rules::OVERLAY_NONE)
            .count();
        assert_eq!(
            non_empty,
            stats.overlay_drawn
                + stats.overlay_no_type
                + stats.overlay_missing_file
                + stats.overlay_fail,
            "每条非空 overlay 记录恰落一个计数器"
        );
    }

    /// 端到端:2×2 地图,走位域 6 格全存;格 (2,2) 有 overlay(表位 0 =
    /// ORE01,帧字节 3),SHP 恰覆格菱形域;透明中心对穿透保留地形色;
    /// 守恒恒等钉死。格取 (2,2) 而非 (1,1):2×2 走位域为 (2,1),(1,2),
    /// (2,2),(3,2),(2,3),(3,3)——(1,1) 在域外,d−d_min = −1 会使帧半幅
    /// 裁出画布,与"SHP 恰覆格菱形域"矛盾;(2,2) 的菱形包围盒与帧矩形
    /// 恰重合(sx = 30,sy = 15)。
    #[test]
    fn overlay_paints_cell_and_conservation() {
        let pal = test_palette();
        let assets = tiny_assets(
            "paints",
            &[
                ("tst01.sno", tmp60(9)),
                ("ore01.shp", shp_raw(&[body60(7, true)])),
            ],
        );
        let map = overlay_map(
            (2, 2),
            &types_layer(&[(2, 2, 0)]),
            &data_layer(&[(2, 2, 3)]),
        );
        let ext = theater_spec(map.map_meta.theater).ext; // "sno"
        let recs = sorted_records(2, 2);
        let ter = compose_terrain(2, 2, &recs, &assets, &terrain_ini(), ext, &pal).unwrap();
        assert_eq!((ter.width, ter.height), (120, 75), "2×2 走位极值画布");
        assert_eq!(ter.stats.painted, 6, "6 格全铺(tst01.sno 色 9)");
        let (cw, ch) = (ter.width as usize, ter.height as usize);
        let mut rgb = ter.rgb;
        let (d_min, _, xmy_min, _) = walk_extremes(2, 2);
        let mut stats = SceneStats::default();
        let mut cache = ShpCache::new();
        paint_overlays(
            &map,
            &assets,
            &OverlayTable::parse("[OverlayTypes]\n0=ORE01\n[ORE01]\nTiberium=yes\n"),
            &pal,
            &recs,
            d_min,
            xmy_min,
            0,
            ext,
            &mut rgb,
            cw,
            ch,
            &mut stats,
            &mut cache,
        );
        // (d) 统计:data=3 > 单帧 SHP → 钳帧 0(frame_clamp 信息位)
        assert_eq!(stats.overlay_drawn, 1);
        assert_eq!(stats.overlay_no_type, 0);
        assert_eq!(stats.overlay_missing_file, 0);
        assert_eq!(stats.overlay_fail, 0);
        assert_eq!(stats.frame_clamp, 1);
        assert_eq!(stats.clip, 0, "60×30 帧恰覆格菱形域 → Inside");
        assert_conservation(&map, &recs, &stats);
        // 像素:格 (2,2) d=4,xmy=0 → C = (60,30),帧 TL = (30,15);
        // 帧中心对称两点 = 帧列 29/30 行 15(构造为透明)→ 保留地形色 9。
        let px = |x: usize, y: usize| -> [u8; 3] {
            let i = 3 * (y * cw + x);
            [rgb[i], rgb[i + 1], rgb[i + 2]]
        };
        assert_eq!(px(61, 30), c8(7), "帧中心右邻 = ORE01 fill 7");
        assert_eq!(px(58, 30), c8(7), "帧中心左邻 = ORE01 fill 7");
        assert_eq!(px(60, 30), c8(9), "帧中心右点透明 → 地形色未覆写");
        assert_eq!(px(59, 30), c8(9), "帧中心左点透明 → 地形色未覆写");
        assert_eq!(px(29, 15), c8(9), "帧左缘外((1,2) 菱形内)无渗色");
    }

    /// 帧号越界钳末帧 + frame_clamp;crate 帧 0;桥变体表 16 项常量。
    /// 3×2 地图:crate (2,2) 表位 1 data 5 → 特判帧 0;bridge (3,2) 表位 2
    /// data 0,i = (3&3)|(4·(2&3)) = 11 → 变体 +1 → 帧 1;gasand (4,2)
    /// 表位 0 data 9 单帧 → 钳 0(frame_clamp 恰 1)。
    #[test]
    fn crate_bridge_clamp_counters() {
        let pal = test_palette();
        let assets = tiny_assets(
            "bridge",
            &[
                ("tst01.sno", tmp60(9)),
                ("gasand.shp", shp_raw(&[body60(7, false)])),
                (
                    "crate01.shp",
                    shp_raw(&[body60(3, false), body60(4, false)]),
                ),
                (
                    "bridge1.shp",
                    shp_raw(&[body60(5, false), body60(6, false)]),
                ),
            ],
        );
        let map = overlay_map(
            (3, 2),
            &types_layer(&[(2, 2, 1), (3, 2, 2), (4, 2, 0)]),
            &data_layer(&[(2, 2, 5), (3, 2, 0), (4, 2, 9)]),
        );
        let ext = theater_spec(map.map_meta.theater).ext;
        let recs = sorted_records(3, 2);
        let ter = compose_terrain(3, 2, &recs, &assets, &terrain_ini(), ext, &pal).unwrap();
        assert_eq!((ter.width, ter.height), (180, 75), "3×2 走位极值画布");
        let (cw, ch) = (ter.width as usize, ter.height as usize);
        let mut rgb = ter.rgb;
        let (d_min, _, xmy_min, _) = walk_extremes(3, 2);
        let table = OverlayTable::parse(
            "[OverlayTypes]\n0=GASAND\n1=CRATE01\n2=BRIDGE1\n[CRATE01]\nCrate=yes\n",
        );
        let mut stats = SceneStats::default();
        let mut cache = ShpCache::new();
        paint_overlays(
            &map, &assets, &table, &pal, &recs, d_min, xmy_min, 0, ext, &mut rgb, cw, ch,
            &mut stats, &mut cache,
        );
        assert_eq!(BRIDGE_VARIATION.len(), 16, "桥变体表 16 项常量");
        assert_eq!(stats.overlay_drawn, 3);
        assert_eq!(
            stats.overlay_no_type + stats.overlay_missing_file + stats.overlay_fail,
            0
        );
        assert_eq!(stats.clip, 0);
        // (c) 钳计数:仅 gasand(data 9 → 单帧钳 0);crate data 5 特判帧 0
        // 不经钳;bridge 0+1 = 1 < 2 帧不钳
        assert_eq!(stats.frame_clamp, 1);
        assert_conservation(&map, &recs, &stats);
        // 格几何(画序 d 升序:crate(2,2) → bridge(3,2) → gasand(4,2)):
        //   crate  TL=(60,0)  矩形行 0..29  列 60..119
        //   bridge TL=(90,15) 矩形行 15..44 列 90..149(争用带后画胜)
        //   gasand TL=(120,30) 矩形行 30..59 列 120..179(最后画)
        let px = |x: usize, y: usize| -> [u8; 3] {
            let i = 3 * (y * cw + x);
            [rgb[i], rgb[i + 1], rgb[i + 2]]
        };
        // (a) crate:data 5 → 特判帧 0(色 3;若 data 直传则钳帧 1 色 4)
        assert_eq!(px(61, 1), c8(3), "crate data=5 → 帧 0 色 3");
        // (b) bridge:i = VARIATION[11] = 1 → 帧 1(色 6;无变体则帧 0 色 5)
        assert_eq!(px(91, 31), c8(6), "桥变体表 +1 → 帧 1 色 6");
        // (c) gasand:data 9 越单帧界 → 钳帧 0(色 7;不钳则帧 9 越索引)
        assert_eq!(px(151, 45), c8(7), "data=9 钳末帧(唯一帧)色 7");
    }

    /// VEIN 前缀守卫(cell.cpp:1401-1403):表项名 VEIN* 不画、不计失败,
    /// 计 overlay_drawn(链路成功像素面空,与 Outside 同口径);守卫先于
    /// 查档——资产刻意不提供 VEINHOLEDUMMY.shp,缺档不得入 missing/fail;
    /// 画布零写入(格中心保持地形色)。零售 [OverlayTypes] 唯一 VEIN 名 =
    /// 表位 178 VEINHOLEDUMMY,语料命中 0 格(研究 §4.5),纯语义对齐。
    #[test]
    fn vein_prefixed_overlay_skips_paint() {
        let pal = test_palette();
        let assets = tiny_assets("vein", &[("tst01.sno", tmp60(9))]);
        let map = overlay_map(
            (2, 2),
            &types_layer(&[(2, 2, 0)]),
            &data_layer(&[(2, 2, 0)]),
        );
        let ext = theater_spec(map.map_meta.theater).ext;
        let recs = sorted_records(2, 2);
        let ter = compose_terrain(2, 2, &recs, &assets, &terrain_ini(), ext, &pal).unwrap();
        let (cw, ch) = (ter.width as usize, ter.height as usize);
        let mut rgb = ter.rgb;
        let (d_min, _, xmy_min, _) = walk_extremes(2, 2);
        let mut stats = SceneStats::default();
        let mut cache = ShpCache::new();
        paint_overlays(
            &map,
            &assets,
            &OverlayTable::parse("[OverlayTypes]\n0=VEINHOLEDUMMY\n"),
            &pal,
            &recs,
            d_min,
            xmy_min,
            0,
            ext,
            &mut rgb,
            cw,
            ch,
            &mut stats,
            &mut cache,
        );
        assert_eq!(stats.overlay_drawn, 1, "守卫命中计 drawn(不画)");
        assert_eq!(
            stats.overlay_no_type + stats.overlay_missing_file + stats.overlay_fail,
            0,
            "守卫先于查档:无 no_type/missing/fail"
        );
        assert_conservation(&map, &recs, &stats);
        let px = |x: usize, y: usize| -> [u8; 3] {
            let i = 3 * (y * cw + x);
            [rgb[i], rgb[i + 1], rgb[i + 2]]
        };
        assert_eq!(px(60, 30), c8(9), "画布零写入(格中心保持地形色)");
    }
}
