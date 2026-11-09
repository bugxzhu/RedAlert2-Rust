// SPDX-License-Identifier: GPL-3.0-only

//! 物件 pass(M4b spec §5;研究 §1/§3/§4):三类 SHP 物件(树/建筑/
//! 步兵)按引擎 Sort_Y 升序稳定绘制;Unit/Aircraft = voxel 计数跳过
//! (用户裁决 2026-11-06,M4c);Smudge 顺延不计。锚定/朝向公式出处见
//! 研究文档;建筑 in-game Draw_IT 未反编译 → Editor 路径锚定(L1)。

use crate::format::art::ArtIndex;
use crate::format::map::objects::ObjectClass;
use crate::format::map::MapFile;
use crate::format::pal::Palette;
use crate::format::theater::TheaterAssets;
use crate::render::shape::{blit_frame, load_shp, BlitResult, CachedShp};
use crate::render::{SceneStats, ShpCache, CELL_DX, CELL_DY, Z_STEP};

/// 步兵子格像素偏移(0..=5;5 = 兜底中心;L3 待钉,研究 §9)。
const SUBCELL_OFFSETS: [(i64, i64); 6] = [(0, 0), (-15, -8), (15, -8), (-15, 7), (15, 7), (0, 0)];

/// HumanShape[dir32] 四舍五入等价(research §4):dir256·F/256 就近取整,
/// 边界钳 F−1。
fn facing_idx(facing: u32, facings: u32) -> u32 {
    let f = u64::from(facings.max(1));
    (((u64::from(facing) * f + 128) >> 8) as u32).min(facings.max(1) - 1)
}

#[allow(dead_code, clippy::too_many_arguments)] // T6 scene 接线后移除 dead_code;冻结签名 13 参直传
pub(crate) fn paint_objects(
    map: &MapFile,
    assets: &TheaterAssets,
    art: &ArtIndex,
    palette: &Palette,
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
    // (sort_key, 对象)——稳定排序保持 map 文件序(Sorted_Add 等插入序)
    let mut drawable: Vec<(i64, &crate::format::map::objects::MapObject)> = Vec::new();
    for o in &map.objects.all {
        match o.class {
            ObjectClass::Unit | ObjectClass::Aircraft => {
                stats.objects_skipped_voxel += 1;
            }
            ObjectClass::Smudge => {} // M4c;不入桶不入守恒
            ObjectClass::Terrain | ObjectClass::Structure | ObjectClass::Infantry => {
                if o.malformed || o.cell().is_none() || o.kind().is_none() {
                    stats.objects_malformed += 1;
                } else {
                    let (x, y) = o.cell().unwrap();
                    let key = i64::from(x) + i64::from(y);
                    // Sort_Y:建筑 = NW 角(building.cpp:7485),其余 = 格中心
                    let key = if matches!(o.class, ObjectClass::Structure) {
                        256 * key
                    } else {
                        256 * key + 256
                    };
                    drawable.push((key, o));
                }
            }
        }
    }
    drawable.sort_by_key(|(k, _)| *k);
    for (_, o) in drawable {
        let (x, y) = o.cell().unwrap();
        let Some(kind) = o.kind() else { continue };
        // 0 基 → 记录坐标;敌意大值钳到域外(u16::MAX 远超走位域 → 自然
        // Outside 计 clip,绝不截断落错位置)。saturating_add:u32::MAX+1
        // 敌意行零 panic,同样钳到域外(u32 加法回绕会违零 panic 契约)。
        let (wx, wy) = (
            u16::try_from(x.saturating_add(1)).unwrap_or(u16::MAX),
            u16::try_from(y.saturating_add(1)).unwrap_or(u16::MAX),
        );
        let z = map.tiles.cell(wx, wy).map_or(0u8, |t| t.z);
        let d = i64::from(wx) + i64::from(wy);
        let xmy = i64::from(wx) - i64::from(wy);
        let cx = CELL_DX as i64 * (xmy - xmy_min) + CELL_DX as i64;
        let cy = Z_STEP as i64 * z_max as i64 + CELL_DY as i64 * (d - d_min)
            - Z_STEP as i64 * i64::from(z)
            + CELL_DY as i64;
        // 锚定/帧选择(研究 §3/§4);get_or_default 返回 owned ArtEntry,
        // image 字段直接移出
        let (image, center, frame) = match o.class {
            ObjectClass::Terrain => {
                let e = art.get_or_default(kind, 1);
                (e.image, (cx, cy - 12), 0usize) // terrain.cpp:281 zadjust −12
            }
            ObjectClass::Infantry => {
                let e = art.get_or_default(kind, 8);
                let f = o.render_facing().unwrap_or(0);
                let idx = facing_idx(f, e.facings) as usize;
                let (ox, oy) = SUBCELL_OFFSETS[(o.sub_cell().unwrap_or(0) as usize).min(5)];
                (
                    e.image,
                    (cx + ox, cy + oy - 10),
                    e.stand_frame as usize + idx,
                ) // infantry.cpp:495
            }
            _ => {
                let e = art.get_or_default(kind, 1);
                (e.image, (cx, cy - 15), 0usize) // 建筑锚 = NW 角投影 = C+(0,−15)(L1)
            }
        };
        let loaded = load_shp(assets, cache, &image, ext);
        let CachedShp::File(shp) = loaded else {
            if matches!(loaded, CachedShp::Missing) {
                stats.objects_missing_art += 1;
            } else {
                stats.objects_fail += 1;
            }
            continue;
        };
        let mut frame = frame;
        if frame >= shp.frames.len() {
            frame = shp.frames.len() - 1;
            stats.frame_clamp += 1;
        }
        match blit_frame(rgb, cw, ch, shp, &shp.frames[frame], center, palette) {
            BlitResult::Inside => stats.objects_drawn += 1,
            BlitResult::Clipped | BlitResult::Outside => {
                stats.clip += 1;
                stats.objects_drawn += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::map::iso::IsoTile;
    use crate::format::theater::{theater_spec, TheaterIni};
    use crate::render::terrain::{compose_terrain, walk_extremes};

    // ===== 夹具:合成剧场 mix(手法同 render/overlay.rs tests)=====

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
            "ra2-objects-{tag}-{}",
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

    // ===== 夹具:合成 RAW 形态 SHP-TS(overlay.rs shp_raw 的 W×H 广义化)=====

    /// N 帧 W×H RAW SHP:帧 i 整帧 = fills[i],帧体按序背靠背。裸 RAW 判别
    /// 同 shp_ts.rs(先严格 RLE,败且 span==cx·cy 才 RAW):均匀填充的严格
    /// RLE 行块必败——首块 count = 257·fill 对 <4 / 越界 / 行链中途耗尽三
    /// 败因全覆盖(span=1 时连 2 字节块头都不足)→ 恒走 RAW 兜底。
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

    // ===== 夹具:mini 地图(map/mod.rs tests 同款编码器)=====

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
                s.push(A[n as usize & 63] as char);
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

    /// 262144 字节层的 LCW 块流(map/mod.rs tests 修正款:size_in=5 + 0x80
    /// 终止符;overlay 双包段为 parse 必需,本 pass 不读其内容)。
    fn lcw_fill_stream(fill: u8, total: usize) -> Vec<u8> {
        let mut blob = Vec::new();
        let mut left = total;
        while left > 0 {
            let n = left.min(8192);
            blob.extend_from_slice(&5u16.to_le_bytes());
            blob.extend_from_slice(&(n as u16).to_le_bytes());
            blob.extend_from_slice(&[0xFE, n as u8, (n >> 8) as u8, fill, 0x80]);
            left -= n;
        }
        blob
    }

    /// mini 地图:[Map] + IsoMapPack5(tiles.cell 的 z 源)+ overlay 双包段
    /// (parse 必需)+ 对象段原文。
    fn objects_map(w: u16, h: u16, tiles: &[[u8; 11]], sections: &str) -> MapFile {
        let mut ini = String::new();
        ini.push_str(&format!("[Map]\nTheater=SNOW\nSize=0,0,{w},{h}\n"));
        let mut stream = Vec::new();
        for r in tiles {
            stream.extend_from_slice(r);
        }
        stream.extend_from_slice(&[0, 0, 0, 0]);
        push_pack_section(&mut ini, "IsoMapPack5", &lzo_literal_stream(&stream));
        push_pack_section(&mut ini, "OverlayPack", &lcw_fill_stream(0xFF, 262144));
        push_pack_section(&mut ini, "OverlayDataPack", &lcw_fill_stream(0x00, 262144));
        ini.push_str(sections);
        MapFile::parse(&ini.into_bytes()).unwrap()
    }

    // ===== 夹具:走位域 + 地形画布 =====

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

    /// d 升序全走位画序记录(compose_terrain 输入;与 tile_records 同 z)。
    fn sorted_records(w: u16, h: u16, zed: &[(u16, u16, u8)]) -> Vec<IsoTile> {
        let mut recs: Vec<IsoTile> = walk_cells(w, h)
            .iter()
            .map(|&(x, y)| IsoTile {
                x,
                y,
                tile: 0,
                sub_tile: 0,
                z: z_at(zed, x, y),
            })
            .collect();
        recs.sort_by_key(|t| u32::from(t.x) + u32::from(t.y));
        recs
    }

    /// §3.2 伪代码物化回放(terrain.rs/overlay.rs tests 同款):按发射序
    /// 产出全部 (x,y)。
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

    /// 场景装配:全走位 IsoMapPack5(zed 覆写 z)→ 地图解析 → 全走位地形
    /// 画布(tst01.sno fill 9)→ (map, rgb, cw, ch, d_min, xmy_min, z_max, ext)。
    fn stage(
        w: u16,
        h: u16,
        zed: &[(u16, u16, u8)],
        sections: &str,
        assets: &TheaterAssets,
        pal: &Palette,
    ) -> (
        MapFile,
        Vec<u8>,
        usize,
        usize,
        i64,
        i64,
        usize,
        &'static str,
    ) {
        let map = objects_map(w, h, &tile_records(w, h, zed), sections);
        let ext = theater_spec(map.map_meta.theater).ext;
        let records = sorted_records(w, h, zed);
        let ter = compose_terrain(w, h, &records, assets, &terrain_ini(), ext, pal).unwrap();
        let (d_min, _, xmy_min, _) = walk_extremes(w, h);
        let z_max = records.iter().map(|t| t.z).max().unwrap_or(0) as usize;
        (
            map,
            ter.rgb,
            ter.width as usize,
            ter.height as usize,
            d_min,
            xmy_min,
            z_max,
            ext,
        )
    }

    /// 守恒恒等(brief §):非 Smudge 物件 = drawn+skipped_voxel+missing_art
    /// +malformed+fail(每件恰落一个计数器;Smudge 不入式)。
    fn assert_conservation(map: &MapFile, stats: &SceneStats) {
        let non_smudge = map
            .objects
            .all
            .iter()
            .filter(|o| o.class != ObjectClass::Smudge)
            .count();
        assert_eq!(
            non_smudge,
            stats.objects_drawn
                + stats.objects_skipped_voxel
                + stats.objects_missing_art
                + stats.objects_malformed
                + stats.objects_fail,
            "每件非 Smudge 物件恰落一个计数器"
        );
    }

    /// 排序(Sort_Y,object.cpp:1482-1485):同列建筑/树,键差半格(建筑
    /// = 256·(X+Y) 无中心项、树 = 256·(X+Y)+256)。Sort_Y 键与 z 无关(冻
    /// 结实现),z 只经投影搬运帧带(M4a T2 手法诱发 60×30 帧重叠),故
    /// "近者后画覆写"由键定:配置 A 树键大 → 树后画覆写争用像素;配置 B
    /// 建筑落近格(键大)→ 建筑后画覆写,且文件序([Structures] 恒先于
    /// [Terrain])与画序相反——文件序实现会得树色,钉死排序序。
    #[test]
    fn sort_y_orders_building_vs_tree() {
        let pal = test_palette();
        let assets = tiny_assets(
            "sort",
            &[
                ("tst01.sno", tmp60(9)),
                ("tstbldg.shp", shp_frames(60, 30, &[2])),
                ("tsttree.shp", shp_frames(60, 30, &[40])),
            ],
        );
        let art = ArtIndex::parse("", "");
        // 配置 A:建筑 0 基 (2,1)[键 768]、树 (2,2)[键 1280];树格 z=1 使两
        // 格投影 C 同为 (120,45):建筑帧行 15..44、树帧行 18..47。
        let sections_a = "[Structures]\n\
                          0=Neutral,TSTBLDG,256,2,1,0,None,0,0,1,0,0,None,None,None,0,0\n\
                          [Terrain]\n2002=TSTTREE\n";
        let (map, rgb, cw, ch, d_min, xmy_min, z_max, ext) =
            stage(3, 3, &[(3, 3, 1)], sections_a, &assets, &pal);
        assert_eq!((cw, ch), (180, 120), "z_max=1 抬升画布");
        let mut rgb = rgb;
        let mut stats = SceneStats::default();
        let mut cache = ShpCache::new();
        paint_objects(
            &map, &assets, &art, &pal, d_min, xmy_min, z_max, ext, &mut rgb, cw, ch, &mut stats,
            &mut cache,
        );
        assert_eq!(stats.objects_drawn, 2);
        assert_eq!(stats.clip + stats.frame_clamp, 0);
        assert_eq!(
            px(&rgb, cw, 100, 30),
            c8(40),
            "争用带树后画覆写(键 1280 > 768)"
        );
        assert_eq!(px(&rgb, cw, 100, 16), c8(2), "建筑专属带(行 15..17)");
        assert_eq!(px(&rgb, cw, 100, 46), c8(40), "树专属带(行 45..47)");
        assert_conservation(&map, &stats);
        // 配置 B:建筑 0 基 (3,2)[键 1280]、树 (2,1)[键 1024],建筑格 z=2 抬
        // 举对齐帧带:两格 xmy 同 1 → 同列,C 均 (120,60);建筑帧行 30..59、
        // 树帧行 33..62,争用 33..59。文件序建筑恒先,排序把建筑排后 → 建
        // 筑覆写(文件序实现会得树色)。
        let sections_b = "[Structures]\n\
                          0=Neutral,TSTBLDG,256,3,2,0,None,0,0,1,0,0,None,None,None,0,0\n\
                          [Terrain]\n1002=TSTTREE\n";
        let (map, rgb, cw, ch, d_min, xmy_min, z_max, ext) =
            stage(3, 3, &[(4, 3, 2)], sections_b, &assets, &pal);
        assert_eq!((cw, ch), (180, 135), "z_max=2 抬升画布");
        let mut rgb = rgb;
        let mut stats = SceneStats::default();
        let mut cache = ShpCache::new();
        paint_objects(
            &map, &assets, &art, &pal, d_min, xmy_min, z_max, ext, &mut rgb, cw, ch, &mut stats,
            &mut cache,
        );
        assert_eq!(stats.objects_drawn, 2);
        assert_eq!(stats.clip + stats.frame_clamp, 0);
        assert_eq!(
            px(&rgb, cw, 100, 40),
            c8(2),
            "争用带建筑后画覆写(键 1280 > 1024)"
        );
        assert_eq!(px(&rgb, cw, 100, 31), c8(2), "建筑专属带(行 30..32)");
        assert_eq!(px(&rgb, cw, 100, 60), c8(40), "树专属带(行 60..62)");
        assert_conservation(&map, &stats);
    }

    /// 锚定(研究 §3):格中心 C;树 = C−12;步兵 = C−10;建筑 = C−15
    /// (NW 角顶点)。三物件同格 0 基 (2,2)(记录 (3,3):xmy=0、d=6、z=0,
    /// C = (90,45)),单帧 1×1 SHP 逐类异色 → 帧单像素恰落锚点,落点逐类
    /// 钉死;格中心点无物件落笔(锚全部在 C 上方)。
    #[test]
    fn anchors_tree_infantry_building() {
        let pal = test_palette();
        let assets = tiny_assets(
            "anchors",
            &[
                ("tst01.sno", tmp60(9)),
                ("cabanc1.shp", shp1(1, 1, 50)),
                ("tsttree2.shp", shp1(1, 1, 20)),
                ("tstinf.shp", shp1(1, 1, 30)),
            ],
        );
        let sections = "[Structures]\n\
                        0=Neutral,CABANC1,256,2,2,0,None,0,0,1,0,0,None,None,None,0,0\n\
                        [Infantry]\n\
                        0=Neutral,TSTINF,256,2,2,0,Guard,0,None,0,-1,0,1,1\n\
                        [Terrain]\n2002=TSTTREE2\n";
        let (map, rgb, cw, ch, d_min, xmy_min, z_max, ext) =
            stage(3, 3, &[], sections, &assets, &pal);
        assert_eq!((cw, ch), (180, 105));
        let mut rgb = rgb;
        let mut stats = SceneStats::default();
        let mut cache = ShpCache::new();
        paint_objects(
            &map,
            &assets,
            &ArtIndex::parse("", ""),
            &pal,
            d_min,
            xmy_min,
            z_max,
            ext,
            &mut rgb,
            cw,
            ch,
            &mut stats,
            &mut cache,
        );
        assert_eq!(stats.objects_drawn, 3);
        assert_eq!(stats.clip + stats.frame_clamp, 0);
        assert_eq!(px(&rgb, cw, 90, 30), c8(50), "建筑锚 = C+(0,−15)");
        assert_eq!(px(&rgb, cw, 90, 33), c8(20), "树锚 = C−12");
        assert_eq!(px(&rgb, cw, 90, 35), c8(30), "步兵锚 = C+sub(0,0)−(0,10)");
        assert_eq!(px(&rgb, cw, 90, 45), c8(9), "格中心无落笔(锚全在 C 上方)");
        assert_conservation(&map, &stats);
    }

    /// 朝向帧(研究 §4,HumanShape[dir32] 四舍五入等价):Facings=8、
    /// StandFrame=3。facing=64 → idx = (64·8+128)>>8 = 2 → 帧 5;facing=255
    /// → idx = (255·8+128)>>8 = 8 → 钳 F−1 = 7 → 帧 10。SHP 11 帧逐帧异色
    /// (fill = 10+帧号),像素色即帧号。
    #[test]
    fn infantry_facing_frame_formula() {
        let pal = test_palette();
        let fills: Vec<u8> = (10..=20).collect();
        let assets = tiny_assets(
            "facing",
            &[
                ("tst01.sno", tmp60(9)),
                ("e1.shp", shp_frames(1, 1, &fills)),
            ],
        );
        let sections = "[Infantry]\n\
                        0=Neutral,GI,256,1,1,0,Guard,64,None,0,-1,0,1,1\n\
                        1=Neutral,GI,256,3,1,0,Guard,255,None,0,-1,0,1,1\n";
        let (map, rgb, cw, ch, d_min, xmy_min, z_max, ext) =
            stage(3, 3, &[], sections, &assets, &pal);
        assert_eq!((cw, ch), (180, 105));
        let art = ArtIndex::parse("[GI]\nFacings=8\nStandFrame=3\nImage=E1\n", "");
        let mut rgb = rgb;
        let mut stats = SceneStats::default();
        let mut cache = ShpCache::new();
        paint_objects(
            &map, &assets, &art, &pal, d_min, xmy_min, z_max, ext, &mut rgb, cw, ch, &mut stats,
            &mut cache,
        );
        assert_eq!(stats.objects_drawn, 2);
        assert_eq!(stats.frame_clamp, 0, "11 帧覆盖帧 5/10,公式命中非钳末帧");
        assert_eq!(stats.clip, 0);
        // 步兵 A 0 基 (1,1) → 记录 (2,2),C = (90,15),锚 = (90,5)
        assert_eq!(
            px(&rgb, cw, 90, 5),
            c8(15),
            "facing 64 → 帧 3+2 = 5(fill 15)"
        );
        // 步兵 B 0 基 (3,1) → 记录 (4,2),C = (150,45),锚 = (150,35)
        assert_eq!(
            px(&rgb, cw, 150, 35),
            c8(20),
            "facing 255 → idx 8 钳 7 → 帧 10(fill 20)"
        );
        assert_conservation(&map, &stats);
    }

    /// 子格(研究 §3,infantry.cpp:495):sub_cell=1 → 偏移 (−15,−8),锚 =
    /// C+(−15,−8)−(0,10) = (75,27)(0 基 (2,2) → 记录 (3,3),C = (90,45));
    /// 负证:漏子格偏移的实现会落格中心列 (90,27)——该点地形色 9,断言
    /// 未被覆写。
    #[test]
    fn subcell_offset_applied() {
        let pal = test_palette();
        let assets = tiny_assets(
            "subcell",
            &[("tst01.sno", tmp60(9)), ("tstinf2.shp", shp1(1, 1, 44))],
        );
        let sections = "[Infantry]\n0=Neutral,TSTINF2,256,2,2,1,Guard,0,None,0,-1,0,1,1\n";
        let (map, rgb, cw, ch, d_min, xmy_min, z_max, ext) =
            stage(3, 3, &[], sections, &assets, &pal);
        assert_eq!((cw, ch), (180, 105));
        let mut rgb = rgb;
        let mut stats = SceneStats::default();
        let mut cache = ShpCache::new();
        paint_objects(
            &map,
            &assets,
            &ArtIndex::parse("", ""),
            &pal,
            d_min,
            xmy_min,
            z_max,
            ext,
            &mut rgb,
            cw,
            ch,
            &mut stats,
            &mut cache,
        );
        assert_eq!(stats.objects_drawn, 1);
        assert_eq!(
            px(&rgb, cw, 75, 27),
            c8(44),
            "sub_cell=1 → C+(−15,−8)−(0,10)"
        );
        assert_eq!(px(&rgb, cw, 90, 27), c8(9), "格中心列未被覆写(偏移真生效)");
        assert_conservation(&map, &stats);
    }

    /// 分类桶与守恒(brief §):Unit/Aircraft → objects_skipped_voxel;
    /// Smudge 不入桶不入守恒;16 字段 Structures 损坏行 → malformed;缺档
    /// (双候选皆无)→ missing_art;坏 SHP 字节 → fail;健康树 → drawn 且
    /// 像素恰落树锚(格中心 C−12,非格中心)。守恒:6 件非 Smudge = 1+2+1+1+1。
    #[test]
    fn buckets_and_conservation() {
        let pal = test_palette();
        let assets = tiny_assets(
            "buckets",
            &[
                ("tst01.sno", tmp60(9)),
                ("goodtree.shp", shp1(1, 1, 60)),
                ("broketree.shp", vec![1, 2, 3]), // 头 zero≠0 → ShpTsFile::parse Err
            ],
        );
        let sections = "[Units]\n\
                        0=Neutral,CAR,256,1,1,0,Sleep,None,0,-1,0,-1,1,1\n\
                        [Aircraft]\n\
                        0=Neutral,PDPLANE,256,2,1,64,Guard,None,0,-1,1,1\n\
                        [Structures]\n\
                        69=NeutralGAWEAT,256,138,264,0,None,1,0,1,0,0,None,None,None,1,0\n\
                        [Terrain]\n1002=GHOSTTREE\n2002=BROKETREE\n2001=GOODTREE\n\
                        [Smudge]\n0=CRATER07,2,2,0\n";
        let (map, rgb, cw, ch, d_min, xmy_min, z_max, ext) =
            stage(2, 2, &[], sections, &assets, &pal);
        assert_eq!((cw, ch), (120, 75));
        assert_eq!(map.objects.all.len(), 7);
        assert_eq!(map.objects.count(ObjectClass::Smudge), 1);
        assert_eq!(map.objects.malformed, 1, "损坏行解析期已标记");
        let mut rgb = rgb;
        let mut stats = SceneStats::default();
        let mut cache = ShpCache::new();
        paint_objects(
            &map,
            &assets,
            &ArtIndex::parse("", ""),
            &pal,
            d_min,
            xmy_min,
            z_max,
            ext,
            &mut rgb,
            cw,
            ch,
            &mut stats,
            &mut cache,
        );
        assert_eq!(stats.objects_skipped_voxel, 2, "Unit+Aircraft → voxel(M4c)");
        assert_eq!(stats.objects_malformed, 1);
        assert_eq!(stats.objects_missing_art, 1, "GHOSTTREE 双候选皆无");
        assert_eq!(stats.objects_fail, 1, "BROKETREE 坏字节");
        assert_eq!(stats.objects_drawn, 1, "GOODTREE 恰落锚点");
        assert_eq!(stats.clip + stats.frame_clamp, 0);
        // GOODTREE 0 基 (1,2) → 记录 (2,3):xmy=−1(恰 xmy_min)、d=5 →
        // C = (30,45),树锚 = C−12 = (30,33)
        assert_eq!(
            px(&rgb, cw, 30, 33),
            c8(60),
            "GOODTREE 1×1 恰覆树锚 (30,33)"
        );
        assert_eq!(px(&rgb, cw, 30, 45), c8(9), "格中心无落笔(树锚在 C−12)");
        assert_conservation(&map, &stats);
    }
}
