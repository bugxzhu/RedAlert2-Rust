// SPDX-License-Identifier: GPL-3.0-only

//! 场景合成(M4b spec §4/§5):地形(M4a compose_terrain,画布公式
//! 不动)→ overlay pass → 物件 pass,共用画布/统计/SHP 缓存。
//! z_max 由 records 实扫(独立于 H 反推,M4a 观察项);精灵越界裁剪
//! 计 clip(引擎对视口同语义)。

use crate::format::art::ArtIndex;
use crate::format::map::MapFile;
use crate::format::pal::Palette;
use crate::format::rules::OverlayTable;
use crate::format::theater::{theater_spec, TheaterAssets, TheaterIni};
use crate::format::FormatError;
use crate::render::objects::paint_objects;
use crate::render::overlay::paint_overlays;
use crate::render::terrain::{compose_terrain, walk_extremes};
use crate::render::{SceneStats, ShpCache};

/// 场景合成结果。
pub struct SceneRender {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
    pub stats: SceneStats,
}

/// 场景合成入口(T7 CLI/T8 验收依赖,签名冻结):① records d 升序 →
/// compose_terrain(画布公式不动);② walk_extremes + records z_max 实扫
/// (独立 H 钉,M4a 观察项);③ overlay pass;④ 物件 pass。地形 RGB 即
/// 画布底,两 pass 原位叠写,统计/SHP 缓存全场景共用。
pub fn render_scene(
    map: &MapFile,
    assets: &TheaterAssets,
    ini: &TheaterIni,
    table: &OverlayTable,
    art: &ArtIndex,
    palette: &Palette,
) -> Result<SceneRender, FormatError> {
    let (w, h) = map.size_wh();
    // 存储 (x,y) 权威(records() 原序 cell_key = y<<16|x,稳定排序后同 d
    // 保持 y 升序;compose_terrain 画序契约,overlay pass 同域消费)。
    let mut records = map.tiles.records().to_vec();
    records.sort_by_key(|t| u32::from(t.x) + u32::from(t.y));
    let spec = theater_spec(map.map_meta.theater);
    let terrain = compose_terrain(w, h, &records, assets, ini, spec.ext, palette)?;
    let (d_min, _d_max, xmy_min, _xmy_max) = walk_extremes(w, h);
    // z_max 由 records 实扫(与 compose_terrain 内部推导同源同值,独立于 H)
    let z_max = records.iter().map(|t| t.z).max().unwrap_or(0) as usize;
    let mut stats = SceneStats {
        terrain: terrain.stats,
        ..SceneStats::default()
    };
    let (cw, ch) = (terrain.width as usize, terrain.height as usize);
    let mut rgb = terrain.rgb;
    let mut cache = ShpCache::new();
    paint_overlays(
        map, assets, table, palette, &records, d_min, xmy_min, z_max, spec.ext, &mut rgb, cw, ch,
        &mut stats, &mut cache,
    );
    paint_objects(
        map, assets, art, palette, d_min, xmy_min, z_max, spec.ext, &mut rgb, cw, ch, &mut stats,
        &mut cache,
    );
    Ok(SceneRender {
        width: cw as u32,
        height: ch as u32,
        rgb,
        stats,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::map::objects::ObjectClass;
    use crate::render::terrain::walk_extremes;

    // ===== 夹具:合成剧场 mix(手法同 render/overlay.rs・objects.rs tests)=====

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

    /// open() 必需 ra2.mix + ra2md.mix;资产全放 ra2md(优先级 100),文件名
    /// 全小写——lookup 大小写不敏感,覆盖 load_shp 的 "{IMAGE}.shp" 大写候选。
    /// 目录名带测试名:并行线程各自独占(纯时间戳在 Windows 粗粒度时钟下会
    /// 撞目录,互删 mix;objects.rs 同款)。
    fn tiny_assets(tag: &str, files: &[(&str, Vec<u8>)]) -> TheaterAssets {
        let d = std::env::temp_dir().join(format!(
            "ra2-scene-{tag}-{}",
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

    /// N 帧 W×H RAW SHP(objects.rs tests 同款):帧 i 整帧 = fills[i]。
    /// 均匀填充严格 RLE 必败 → 恒走 RAW 兜底(shp_ts.rs 判别式)。
    fn shp_frames(cx: u16, cy: u16, fills: &[u8]) -> Vec<u8> {
        assert!(!fills.is_empty());
        let span = cx as usize * cy as usize;
        let mut v = Vec::new();
        v.extend_from_slice(&0u16.to_le_bytes()); // zero
        v.extend_from_slice(&cx.to_le_bytes()); // cx
        v.extend_from_slice(&cy.to_le_bytes()); // cy
        v.extend_from_slice(&(fills.len() as u16).to_le_bytes()); // count
        let table_end = 8 + 24 * fills.len();
        for (i, _) in fills.iter().enumerate() {
            v.extend_from_slice(&0i16.to_le_bytes()); // x
            v.extend_from_slice(&0i16.to_le_bytes()); // y
            v.extend_from_slice(&cx.to_le_bytes());
            v.extend_from_slice(&cy.to_le_bytes());
            v.extend_from_slice(&3u32.to_le_bytes()); // flags
            v.extend_from_slice(&[0u8; 4]); // rgba
            v.extend_from_slice(&0u32.to_le_bytes()); // reserved
            v.extend_from_slice(&((table_end + span * i) as u32).to_le_bytes());
        }
        for f in fills {
            v.extend_from_slice(&vec![*f; span]);
        }
        v
    }

    /// 单帧 W×H RAW SHP。
    fn shp1(cx: u16, cy: u16, fill: u8) -> Vec<u8> {
        shp_frames(cx, cy, &[fill])
    }

    // ===== 夹具:mini 地图编码器(map/mod.rs・overlay.rs tests 同款)=====

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

    /// LZO 字面量块流(map/mod.rs tests 同款;IsoMapPack5 用)。
    fn lzo_literal_payload(data: &[u8]) -> Vec<u8> {
        assert!((1..=238).contains(&data.len()));
        let mut p = vec![(17 + data.len()) as u8];
        p.extend_from_slice(data);
        p.extend_from_slice(&[0x11, 0x00, 0x00]);
        p
    }

    fn push_lzo_chunk(blob: &mut Vec<u8>, data: &[u8]) {
        let payload = lzo_literal_payload(data);
        blob.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        blob.extend_from_slice(&(data.len() as u16).to_le_bytes());
        blob.extend_from_slice(&payload);
    }

    fn lzo_literal_stream(data: &[u8]) -> Vec<u8> {
        let mut blob = Vec::new();
        let (full, rest) = data.as_chunks::<238>();
        for chunk in full {
            push_lzo_chunk(&mut blob, chunk);
        }
        if !rest.is_empty() {
            push_lzo_chunk(&mut blob, rest);
        }
        blob
    }

    /// 瓦片记录 11 字节(x,y,tile,sub,z;恒零垫字节)。
    fn rec(x: u16, y: u16, tile: i16, sub: u8, z: u8) -> [u8; 11] {
        let mut b = [0u8; 11];
        b[0..2].copy_from_slice(&x.to_le_bytes());
        b[2..4].copy_from_slice(&y.to_le_bytes());
        b[4..6].copy_from_slice(&tile.to_le_bytes());
        b[8] = sub;
        b[9] = z;
        b
    }

    fn z_at(zed: &[(u16, u16, u8)], x: u16, y: u16) -> u8 {
        zed.iter()
            .find(|&&(zx, zy, _)| zx == x && zy == y)
            .map_or(0, |&(_, _, z)| z)
    }

    /// 全走位 IsoMapPack5 记录(zed 逐格覆写 z)。
    fn tile_records(w: u16, h: u16, zed: &[(u16, u16, u8)]) -> Vec<[u8; 11]> {
        walk_cells(w, h)
            .iter()
            .map(|&(x, y)| rec(x, y, 0, 0, z_at(zed, x, y)))
            .collect()
    }

    /// 262144 字节层的 LCW 块流(overlay.rs tests 同款;支持逐格改写)。
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

    /// mini 场景图:[Map] + IsoMapPack5(全走位,zed 覆写 z)+ overlay 双包段
    /// (ovl = (记录 x, 记录 y, 类型字节, 帧字节))+ 对象段原文——合并
    /// objects_map 与 overlay_map 两夹具,三层内容齐备。
    fn scene_map(
        w: u16,
        h: u16,
        zed: &[(u16, u16, u8)],
        ovl: &[(u16, u16, u8, u8)],
        sections: &str,
    ) -> MapFile {
        let mut ini = String::new();
        ini.push_str(&format!("[Map]\nTheater=SNOW\nSize=0,0,{w},{h}\n"));
        let mut stream = Vec::new();
        for r in tile_records(w, h, zed) {
            stream.extend_from_slice(&r);
        }
        stream.extend_from_slice(&[0, 0, 0, 0]);
        push_pack_section(&mut ini, "IsoMapPack5", &lzo_literal_stream(&stream));
        let types = types_layer(
            &ovl.iter()
                .map(|&(x, y, t, _)| (x, y, t))
                .collect::<Vec<_>>(),
        );
        let data = data_layer(
            &ovl.iter()
                .map(|&(x, y, _, d)| (x, y, d))
                .collect::<Vec<_>>(),
        );
        push_pack_section(&mut ini, "OverlayPack", &lcw_layer_stream(&types));
        push_pack_section(&mut ini, "OverlayDataPack", &lcw_layer_stream(&data));
        ini.push_str(sections);
        MapFile::parse(&ini.into_bytes()).unwrap()
    }

    // ===== 夹具:走位域 + 通用 =====

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

    /// 像素取样。
    fn px(rgb: &[u8], cw: usize, x: usize, y: usize) -> [u8; 3] {
        let i = 3 * (y * cw + x);
        [rgb[i], rgb[i + 1], rgb[i + 2]]
    }

    /// 场景入口直跑(terrain_ini 固定;表/art 文本入参)。
    fn run_scene(
        map: &MapFile,
        assets: &TheaterAssets,
        table: &str,
        art_md: &str,
        pal: &Palette,
    ) -> SceneRender {
        render_scene(
            map,
            assets,
            &terrain_ini(),
            &OverlayTable::parse(table),
            &ArtIndex::parse(art_md, ""),
            pal,
        )
        .unwrap()
    }

    /// ORE01 单条表(孤立格 display_byte = 表位 0:游程长 1,v = (x·y)%1 = 0)。
    const ORE_TABLE: &str = "[OverlayTypes]\n0=ORE01\n[ORE01]\nTiberium=yes\n";

    /// 层序(brief 步骤 1):同格 (2,2)(记录坐标)d=4、xmy=0 → C=(60,30)。
    /// 三层同格异色:瓦片 tst01.sno 菱形色 9、overlay ORE01 60×30 满帧 7
    /// (TL=(30,15) 覆格带)、树 FULLT 60×30 满帧 40(树锚 = C−12 → 帧 TL=
    /// (30,3),仍覆 (60,30))。三态断言:全链 = 物件色;空 [Terrain] =
    /// overlay 色;再空 overlay = 瓦片色。
    #[test]
    fn layer_order_tile_overlay_object() {
        let pal = test_palette();
        let assets = tiny_assets(
            "layer",
            &[
                ("tst01.sno", tmp60(9)),
                ("ore01.shp", shp_frames(60, 30, &[7])),
                ("fullt.shp", shp_frames(60, 30, &[40])),
            ],
        );
        // (a) 全链:瓦片 9 → overlay 7 → 物件 40 同格叠写(树 0 基 (1,1)
        // = 记录 (2,2),与 overlay 同格)
        let map = scene_map(2, 2, &[], &[(2, 2, 0, 0)], "[Terrain]\n1001=FULLT\n");
        let s = run_scene(&map, &assets, ORE_TABLE, "", &pal);
        assert_eq!((s.width, s.height), (120, 75), "2×2 走位极值画布");
        assert_eq!(s.stats.overlay_drawn, 1);
        assert_eq!(s.stats.objects_drawn, 1);
        let cw = s.width as usize;
        assert_eq!(
            px(&s.rgb, cw, 60, 30),
            c8(40),
            "物件 pass 最后画,争用像素 = 物件色"
        );
        assert_eq!(
            px(&s.rgb, cw, 60, 5),
            c8(40),
            "树专属带(帧带行 3..15,背景之上)"
        );
        assert_eq!(
            px(&s.rgb, cw, 60, 40),
            c8(7),
            "overlay 专属带(树带 3..32 之下、overlay 带 15..44 内)"
        );
        // (b) 物件缺失(空 [Terrain])→ overlay 色
        let map = scene_map(2, 2, &[], &[(2, 2, 0, 0)], "");
        let s = run_scene(&map, &assets, ORE_TABLE, "", &pal);
        assert_eq!(s.stats.objects_drawn, 0);
        assert_eq!(
            px(&s.rgb, s.width as usize, 60, 30),
            c8(7),
            "无物件 → overlay 色"
        );
        // (c) 再剥离 overlay → 瓦片色
        let map = scene_map(2, 2, &[], &[], "");
        let s = run_scene(&map, &assets, ORE_TABLE, "", &pal);
        assert_eq!(s.stats.overlay_drawn + s.stats.objects_drawn, 0);
        assert_eq!(
            px(&s.rgb, s.width as usize, 60, 30),
            c8(9),
            "无 overlay → 瓦片色"
        );
    }

    /// 守恒总账(brief 步骤 1):terrain 七桶 + overlay 四桶 + 物件五桶恒等式
    /// 全验;夹具含 Unit(voxel 口径:计 skip 不计 missing)、Smudge(不入桶
    /// 不入式)、16 字段损坏 Structures 行(malformed),逐桶落位钉死。
    /// GOODTREE 锚点像素兼作物件覆 overlay 的层序证据。
    #[test]
    fn scene_stats_conservation() {
        let pal = test_palette();
        let assets = tiny_assets(
            "ledger",
            &[
                ("tst01.sno", tmp60(9)),
                ("ore01.shp", shp_frames(60, 30, &[7])),
                ("goodtree.shp", shp1(1, 1, 60)),
                ("broketree.shp", vec![1, 2, 3]), // 头 zero≠0 → 解析败
            ],
        );
        let sections = "[Units]\n\
                        0=Neutral,CAR,256,1,1,0,Sleep,None,0,-1,0,-1,1,1\n\
                        [Structures]\n\
                        69=NeutralGAWEAT,256,138,264,0,None,1,0,1,0,0,None,None,None,1,0\n\
                        [Terrain]\n1002=GHOSTTREE\n2002=BROKETREE\n2001=GOODTREE\n\
                        [Smudge]\n0=CRATER07,2,2,0\n";
        // overlay:格 (2,2) ORE01(表位 0,帧 0);格 (3,2) 0xAA(越表位)
        let map = scene_map(2, 2, &[], &[(2, 2, 0, 0), (3, 2, 0xAA, 0)], sections);
        assert_eq!(map.objects.all.len(), 6, "Unit+损坏建筑+三树+Smudge");
        assert_eq!(map.objects.malformed, 1, "损坏行解析期已标记");
        let s = run_scene(&map, &assets, ORE_TABLE, "", &pal);
        // (t) terrain 七桶恒等:6 记录全 painted
        let t = &s.stats.terrain;
        assert_eq!(t.cells, 6);
        assert_eq!(
            t.cells,
            t.no_tile
                + t.painted
                + t.reserved
                + t.missing_file
                + t.out_of_range
                + t.missing_slot
                + t.parse_fail,
            "terrain 七桶恒等(cells = 其和)"
        );
        assert_eq!(t.painted, 6, "全走位域铺满 tst01.sno");
        // (o) overlay 四桶恒等:非空 2 格 = drawn 1(ORE01)+ no_type 1(0xAA)
        let non_empty = map
            .tiles
            .records()
            .iter()
            .filter(|t| map.overlays.type_at(t.x, t.y) != crate::format::rules::OVERLAY_NONE)
            .count();
        assert_eq!(non_empty, 2);
        assert_eq!(
            non_empty,
            s.stats.overlay_drawn
                + s.stats.overlay_no_type
                + s.stats.overlay_missing_file
                + s.stats.overlay_fail,
            "每条非空 overlay 记录恰落一个计数器"
        );
        assert_eq!(s.stats.overlay_drawn, 1);
        assert_eq!(s.stats.overlay_no_type, 1, "0xAA 越表位 → no_type");
        // (j) 物件五桶恒等:非 Smudge 5 件恰落五桶
        let non_smudge = map
            .objects
            .all
            .iter()
            .filter(|o| o.class != ObjectClass::Smudge)
            .count();
        assert_eq!(non_smudge, 5);
        assert_eq!(
            non_smudge,
            s.stats.objects_drawn
                + s.stats.objects_skipped_voxel
                + s.stats.objects_missing_art
                + s.stats.objects_malformed
                + s.stats.objects_fail,
            "每件非 Smudge 物件恰落一个计数器"
        );
        assert_eq!(
            s.stats.objects_skipped_voxel, 1,
            "Unit → voxel 计数跳过(非 missing)"
        );
        assert_eq!(s.stats.objects_malformed, 1);
        assert_eq!(s.stats.objects_missing_art, 1, "GHOSTTREE 双候选皆无");
        assert_eq!(s.stats.objects_fail, 1, "BROKETREE 坏字节");
        assert_eq!(s.stats.objects_drawn, 1);
        assert_eq!(
            map.objects.count(ObjectClass::Smudge),
            1,
            "Smudge 1 件不入任何桶(恒等式用 5 已钉排除口径)"
        );
        assert_eq!(s.stats.frame_clamp + s.stats.clip, 0, "全夹具零钳帧零裁剪");
        // 像素:GOODTREE 0 基 (1,2) → 记录 (2,3) C=(30,45),树锚 (30,33);
        // overlay (2,2) 帧带(行 15..44,列 30..89)覆该点(填 7),物件 pass
        // 后画 → 锚点 60、锚下 1px 保持 overlay 填充 7。
        let cw = s.width as usize;
        assert_eq!(
            px(&s.rgb, cw, 30, 33),
            c8(60),
            "物件锚点覆 overlay(层序证据)"
        );
        assert_eq!(px(&s.rgb, cw, 30, 34), c8(7), "锚下 1px = overlay 填充");
    }

    /// 画布尺寸 = M4a 公式(scene 不改画布):walk_extremes 重算 W/H 逐图
    /// 对拍(同 m4a 验收口径,合成小图);(3,3) 图含一格 z=1,z_max 由
    /// records 实扫抬升一档。
    #[test]
    fn canvas_dims_follow_m4a_formula() {
        let pal = test_palette();
        let assets = tiny_assets("dims", &[("tst01.sno", tmp60(9))]);
        let check = |w: u16, h: u16, zed: &[(u16, u16, u8)], ew: u32, eh: u32| {
            let map = scene_map(w, h, zed, &[], "");
            let s = run_scene(&map, &assets, "", "", &pal);
            let (d_min, d_max, xmy_min, xmy_max) = walk_extremes(w, h);
            let z_max = map.tiles.records().iter().map(|t| t.z).max().unwrap_or(0);
            assert_eq!(
                s.width,
                (30 * (xmy_max - xmy_min) + 60) as u32,
                "W = 30·(xmy 跨)+60(M4a 公式重算)"
            );
            assert_eq!(
                s.height,
                (15 * (d_max - d_min) + 30 + 15 * i64::from(z_max)) as u32,
                "H = 15·(d 跨)+30+15·z_max(z 实扫)"
            );
            assert_eq!((s.width, s.height), (ew, eh), "字面锚");
            assert_eq!(
                s.rgb.len(),
                3 * s.width as usize * s.height as usize,
                "行紧排 RGB"
            );
            assert_eq!(
                s.stats.terrain.painted, s.stats.terrain.cells,
                "全走位域铺满"
            );
        };
        check(2, 2, &[], 120, 75);
        check(3, 3, &[(4, 4, 1)], 180, 120);
    }

    /// 敌意(brief 步骤 1):rules/art 双空文本 → 全 no_type/missing_art,
    /// 地形照出零 panic(run_scene unwrap 即 panic 探针)。
    #[test]
    fn empty_tables_degrade_to_terrain_only() {
        let pal = test_palette();
        let assets = tiny_assets("degrade", &[("tst01.sno", tmp60(9))]);
        let sections = "[Units]\n\
                        0=Neutral,CAR,256,1,1,0,Sleep,None,0,-1,0,-1,1,1\n\
                        [Terrain]\n2002=TSTTREE\n1002=GHOST\n\
                        [Smudge]\n0=CRATER07,2,2,0\n";
        let map = scene_map(2, 2, &[], &[(2, 2, 0, 0), (2, 3, 5, 1)], sections);
        let s = run_scene(&map, &assets, "", "", &pal); // 空表 + 空 art
        assert_eq!((s.width, s.height), (120, 75));
        assert_eq!(s.stats.terrain.painted, 6, "地形照出");
        assert_eq!(
            px(&s.rgb, s.width as usize, 60, 30),
            c8(9),
            "画布 = 纯地形色"
        );
        assert_eq!(
            s.stats.overlay_drawn + s.stats.overlay_missing_file + s.stats.overlay_fail,
            0,
            "空表:零落笔零查档"
        );
        assert_eq!(s.stats.overlay_no_type, 2, "双非空格全 no_type");
        assert_eq!(
            s.stats.objects_drawn + s.stats.objects_fail + s.stats.objects_malformed,
            0
        );
        assert_eq!(
            s.stats.objects_missing_art, 2,
            "TSTTREE/GHOST 缺省图名(类型名)皆无档"
        );
        assert_eq!(s.stats.objects_skipped_voxel, 1, "voxel 口径不受空表影响");
        assert_eq!(s.stats.frame_clamp + s.stats.clip, 0);
    }
}
