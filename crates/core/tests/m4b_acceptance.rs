// SPDX-License-Identifier: GPL-3.0-only

//! M4b 真资产验收(spec §7):807 场景渲染零 panic + 守恒 + 预算 +
//! Preview 终验四组合探针(裁定:最优组合稳定占比 ≥85% 才钉,差率只报告)。
//! 全部 ignore,需 RA2_ASSETS_DIR。
//!
//! 验收口径:
//! - 逐图 render_scene 计时,预算 `ms < 15000` 为首跑占位(Step 3 控制器
//!   按实测 max×3 收紧,硬帽 15000);
//! - 守恒恒等三组(T4/T6 同式,population 独立重数不信 stats):地形七桶、
//!   非空 overlay(走位域 records 内 type≠0xFF)四桶、非 Smudge 物件五桶;
//! - 画布尺寸 = M4a 公式独立重算(walk_extremes + records z_max 实扫);
//! - 哑调色板剧场(DESERT/NEWURBAN/LUNAR)与真彩同循环全 807 零 panic
//!   (m4a 口径),真彩剧场各抽一图 PPM;
//! - Preview 四组合直方图仅对真彩图统计(哑调色板必 100% 差,无裁定意义)。

use ra2_core::format::art::ArtIndex;
use ra2_core::format::map::mapdoc::decode_map_text;
use ra2_core::format::map::objects::ObjectClass;
use ra2_core::format::map::MapFile;
use ra2_core::format::pal::Palette;
use ra2_core::format::rules::{OverlayTable, OVERLAY_NONE};
use ra2_core::format::theater::{theater_spec, TheaterAssets, TheaterIni};
use ra2_core::render::scene::render_scene;
use ra2_core::render::terrain::walk_extremes;

fn assets_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(
        std::env::var("RA2_ASSETS_DIR").expect("RA2_ASSETS_DIR 指向用户资产目录"),
    )
}

/// 递归收集 .map(m3_acceptance.rs:20-31 同构手法;语料 807 = Maps/ 540
/// + RMG MapUnits 267;扩展名只有 .map,无 .yrm)。
fn maps() -> Vec<std::path::PathBuf> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("read_dir({}) 失败: {e}", dir.display()))
            .flatten()
        {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().map(|x| x == "map").unwrap_or(false) {
                out.push(p);
            }
        }
    }
    let mut v = Vec::new();
    walk(&assets_dir(), &mut v);
    v
}

/// 剧场调色板:可解析 → 真;缺席(DESERT/NEWURBAN/LUNAR)→ 哑(全黑)。
/// 返回 (调色板, 真彩?)。
fn theater_palette(
    assets: &TheaterAssets,
    t: ra2_core::format::map::meta::Theater,
) -> (Palette, bool) {
    let name = t.iso_palette_file();
    if let Some(b) = assets.lookup(name) {
        if let Ok(p) = Palette::parse(b) {
            return (p, true);
        }
    }
    (
        Palette {
            rgb: [[0u8; 3]; 256],
        },
        false,
    )
}

/// PPM 直写(P3 太大用 P6 二进制;目检样本用,core 测试不依赖 tools png)。
fn write_ppm(out: &std::path::Path, w: u32, h: u32, rgb: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut f = std::fs::File::create(out)?;
    write!(f, "P6\n{w} {h}\n255\n")?;
    f.write_all(rgb)
}

/// Preview 差异探针:最近邻缩采样(渲染 → preview 尺寸),通道序
/// (RGB/BGR)× 行序(正常/上下翻)四组合取最小——preview.rs:像素/通道
/// 序 M2 未裁决,取最小即"最优解释下的差率"。逐图打印,控制器裁定。
/// 2026-11-06 裁定=走向 B(预览含 M4a 未渲染层):507 真彩图最优组合
/// 差率 <10% 为 0、中位 91.3%、组合不集中(RGB 207/RGB+flip 242/BGR 28/
/// BGR+flip 30)→ 系统性差异,非通道/行序可解释;对拍顺延 M4b 终验。
/// 量级注记:中位 91% 超出"仅矿石 overlay"可解释范围,M4b 须复核
/// 视口原点/缩放/裁边语义。不设阈值断言。
/// M4b 适配(m4a 原样两处):① 参数类型 TerrainRender → SceneRender
/// (场景口径同字段 width/height/rgb);② 返回最优组合名供直方图统计。
fn probe_preview_diff(
    path: &std::path::Path,
    r: &ra2_core::render::scene::SceneRender,
    pv: &ra2_core::format::map::preview::PreviewImage,
) -> &'static str {
    let (w, h) = (r.width as usize, r.height as usize);
    let (pw, ph) = (pv.width as usize, pv.height as usize);
    let mut best = (usize::MAX, "");
    for (swap, flip) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut diff = 0usize;
        for py in 0..ph {
            for pxx in 0..pw {
                let sx = (pxx * w / pw.max(1)).min(w - 1);
                let sy_raw = py * h / ph.max(1);
                let sy = if flip {
                    (ph - 1 - py) * h / ph.max(1)
                } else {
                    sy_raw
                }
                .min(h - 1);
                let i = 3 * (sy * w + sx);
                let j = 3 * (py * pw + pxx);
                let (a, b) = if swap {
                    (pv.rgb[j + 2], pv.rgb[j])
                } else {
                    (pv.rgb[j], pv.rgb[j + 2])
                };
                let d = (r.rgb[i] as i32 - a as i32).abs()
                    + (r.rgb[i + 1] as i32 - pv.rgb[j + 1] as i32).abs()
                    + (r.rgb[i + 2] as i32 - b as i32).abs();
                if d > 96 {
                    diff += 1; // 通道和差 > 96 记差异(探针阈值,控制器裁定)
                }
            }
        }
        let combo = match (swap, flip) {
            (false, false) => "RGB",
            (true, false) => "BGR",
            (false, true) => "RGB+flip",
            _ => "BGR+flip",
        };
        if diff < best.0 {
            best = (diff, combo);
        }
    }
    let total = pw * ph;
    eprintln!(
        "preview-diff {} 最优 {} {}/{} = {:.1}%",
        path.display(),
        best.1,
        best.0,
        total,
        100.0 * best.0 as f64 / total.max(1) as f64
    );
    best.1
}

/// rulesmd/artmd/art 表装载(任务简报 skeleton 原样):rulesmd 缺席 =
/// 空表(全计 no_type,lenient 契约),art 缺席层 = 空文档(合成缺省)。
fn load_tables(assets: &TheaterAssets) -> (OverlayTable, ArtIndex) {
    let table = assets
        .lookup("rulesmd.ini")
        .map(|b| OverlayTable::parse(&decode_map_text(b)))
        .unwrap_or_else(|| OverlayTable::parse(""));
    let art = ArtIndex::parse(
        &assets
            .lookup("artmd.ini")
            .map(decode_map_text)
            .unwrap_or_default(),
        &assets
            .lookup("art.ini")
            .map(decode_map_text)
            .unwrap_or_default(),
    );
    (table, art)
}

/// 逐剧场分档聚合(overlay 4 桶 + 物件 5 桶 + 信息位;图数/真彩/抽样
/// 由外层 tuple 前三位持有,m4a 同构)。
#[derive(Default)]
struct TheaterAgg {
    overlay: [usize; 4], // drawn, no_type, missing_file, fail
    objects: [usize; 5], // drawn, skipped_voxel, missing_art, malformed, fail
    frame_clamp: usize,
    clip: usize,
}

/// 全量渲染:零 panic + 守恒三恒等 + 尺寸公式 + 真彩抽样 PPM +
/// Preview 四组合直方图 + 耗时/分档总账。哑调色板剧场同循环(全 807
/// 零 panic,m4a 口径)。
#[test]
#[ignore]
fn m4b_all_maps_scene_render() {
    let assets = TheaterAssets::open(&assets_dir()).unwrap();
    let (table, art) = load_tables(&assets);
    let mut total = 0usize;
    let mut total_ms = 0u128;
    let mut max_ms: (u128, String) = (0, String::new());
    let mut all_ms: Vec<u128> = Vec::new();
    let mut total_missing_file = 0usize;
    let mut over = [0usize; 4]; // drawn, no_type, missing_file, fail
    let mut objs = [0usize; 5]; // drawn, voxel, missing_art, malformed, fail
    let mut total_frame_clamp = 0usize;
    let mut total_clip = 0usize;
    // 剧场 → (渲染数, 真彩数, 已抽样?, 分档)
    let mut by_theater: std::collections::BTreeMap<String, (usize, usize, bool, TheaterAgg)> =
        Default::default();
    // Preview 最优组合直方图(真彩口径;裁定输入,不设断言——Step 3)
    let mut preview_hist: std::collections::BTreeMap<&'static str, usize> = Default::default();
    let mut preview_probed = 0usize;
    for path in maps() {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(m) = MapFile::parse(&bytes) else {
            continue;
        };
        let spec = theater_spec(m.map_meta.theater);
        let ini = TheaterIni::parse(&decode_map_text(
            assets.lookup(spec.ini_name).expect("剧场表在"),
        ))
        .expect("剧场 INI 可解析");
        let (pal, real) = theater_palette(&assets, m.map_meta.theater);
        let t0 = std::time::Instant::now();
        let r = render_scene(&m, &assets, &ini, &table, &art, &pal).expect("全量合成零 panic");
        let ms = t0.elapsed().as_millis();
        // 预算:首跑占位 15000(计划硬帽;Step 3 控制器按实测 max×3 收紧)
        assert!(
            ms < 15_000,
            "{} 耗时 {ms}ms 超 M4b 首跑占位预算",
            path.display()
        );
        total += 1;
        total_ms += ms;
        all_ms.push(ms);
        if ms > max_ms.0 {
            max_ms = (ms, path.display().to_string());
        }
        let s = r.stats;
        total_missing_file += s.terrain.missing_file;
        // 守恒 1:地形七桶(每记录恰落七类计数器之一;cells = 记录口径)
        assert_eq!(s.terrain.cells, m.tiles.len(), "{}", path.display());
        assert_eq!(
            s.terrain.painted
                + s.terrain.no_tile
                + s.terrain.missing_file
                + s.terrain.reserved
                + s.terrain.out_of_range
                + s.terrain.missing_slot
                + s.terrain.parse_fail,
            s.terrain.cells,
            "{}: 地形七桶恒等",
            path.display()
        );
        // 守恒 2:非空 overlay(独立重数,overlay pass 同域 = d 升序 records;
        // scene.rs 排序键 u32::from(x)+u32::from(y),此处同式重构不信 stats)
        let mut records = m.tiles.records().to_vec();
        records.sort_by_key(|t| u32::from(t.x) + u32::from(t.y));
        let non_empty = records
            .iter()
            .filter(|t| m.overlays.type_at(t.x, t.y) != OVERLAY_NONE)
            .count();
        assert_eq!(
            non_empty,
            s.overlay_drawn + s.overlay_no_type + s.overlay_missing_file + s.overlay_fail,
            "{}: 非空 overlay 四桶恒等(每记录恰一计数器)",
            path.display()
        );
        // 守恒 3:非 Smudge 物件(objects.rs tests assert_conservation 同式)
        let non_smudge = m
            .objects
            .all
            .iter()
            .filter(|o| o.class != ObjectClass::Smudge)
            .count();
        assert_eq!(
            non_smudge,
            s.objects_drawn
                + s.objects_skipped_voxel
                + s.objects_missing_art
                + s.objects_malformed
                + s.objects_fail,
            "{}: 非 Smudge 物件五桶恒等(每件恰一计数器)",
            path.display()
        );
        // 画布公式独立重算(M1/M4a 口径):W/H 均由 walk_extremes + 实扫 z_max
        let (w, h) = m.size_wh();
        let (d_min, d_max, xmy_min, xmy_max) = walk_extremes(w, h);
        let z_max = m
            .tiles
            .records()
            .iter()
            .map(|t| t.z as usize)
            .max()
            .unwrap_or(0);
        assert_eq!(
            r.width as usize,
            30 * ((xmy_max - xmy_min) as usize) + 60,
            "{}: W = 30·(x−y 跨度)+60",
            path.display()
        );
        assert_eq!(
            r.height as usize,
            15 * ((d_max - d_min) as usize) + 30 + 15 * z_max,
            "{}: H = 15·(d 跨度)+30+15·z_max",
            path.display()
        );
        // 总账累计
        let ov = [
            s.overlay_drawn,
            s.overlay_no_type,
            s.overlay_missing_file,
            s.overlay_fail,
        ];
        let ob = [
            s.objects_drawn,
            s.objects_skipped_voxel,
            s.objects_missing_art,
            s.objects_malformed,
            s.objects_fail,
        ];
        for (a, v) in over.iter_mut().zip(ov) {
            *a += v;
        }
        for (a, v) in objs.iter_mut().zip(ob) {
            *a += v;
        }
        total_frame_clamp += s.frame_clamp;
        total_clip += s.clip;
        let key = format!("{:?}", m.map_meta.theater);
        let entry = by_theater.entry(key).or_default();
        entry.0 += 1;
        for (a, v) in entry.3.overlay.iter_mut().zip(ov) {
            *a += v;
        }
        for (a, v) in entry.3.objects.iter_mut().zip(ob) {
            *a += v;
        }
        entry.3.frame_clamp += s.frame_clamp;
        entry.3.clip += s.clip;
        // 真彩剧场:抽样 PPM + Preview 探针(哑调色板必 100% 差,不参与)
        if real {
            entry.1 += 1;
            if !entry.2 {
                entry.2 = true;
                let out = std::env::temp_dir().join(format!(
                    "ra2-m4b-scene-sample-{}.ppm",
                    path.file_name().unwrap().to_string_lossy()
                ));
                write_ppm(&out, r.width, r.height, &r.rgb).unwrap();
                eprintln!("样本: {}", out.display());
            }
            if let Some(pv) = &m.preview {
                let combo = probe_preview_diff(&path, &r, pv);
                *preview_hist.entry(combo).or_insert(0) += 1;
                preview_probed += 1;
            }
        }
    }
    // 总账行(控制器裁定输入)
    all_ms.sort_unstable();
    let p90 = all_ms
        .get((((all_ms.len() as f64) * 0.9).ceil() as usize).saturating_sub(1))
        .copied()
        .unwrap_or(0);
    let mean = if total > 0 {
        total_ms / total as u128
    } else {
        0
    };
    eprintln!(
        "M4b: 共 {total} 图,总耗时 {total_ms}ms,最慢 {max_ms:?},均值 {mean}ms,p90 {p90}ms,terrain missing_file 总计 {total_missing_file}"
    );
    eprintln!(
        "M4b overlay 总账: drawn {} no_type {} missing_file {} fail {}(frame_clamp {total_frame_clamp}, clip {total_clip})",
        over[0], over[1], over[2], over[3]
    );
    eprintln!(
        "M4b objects 总账: drawn {} voxel {} missing_art {} malformed {} fail {}",
        objs[0], objs[1], objs[2], objs[3], objs[4]
    );
    let sprites = over[0] + objs[0];
    eprintln!(
        "M4b clip 占比: {total_clip}/{sprites} = {:.2}%",
        100.0 * total_clip as f64 / sprites.max(1) as f64
    );
    for (k, (maps, real_n, sampled, agg)) in &by_theater {
        eprintln!(
            "剧场 {k}: {maps} 图(真彩 {real_n},抽样 {sampled})overlay[drawn {} no_type {} missing_file {} fail {}] objects[drawn {} voxel {} missing_art {} malformed {} fail {}] frame_clamp {} clip {}",
            agg.overlay[0], agg.overlay[1], agg.overlay[2], agg.overlay[3],
            agg.objects[0], agg.objects[1], agg.objects[2], agg.objects[3], agg.objects[4],
            agg.frame_clamp, agg.clip
        );
    }
    eprintln!("M4b preview 直方图(真彩含 Preview 共 {preview_probed} 图,Step 3 裁定输入):");
    for (k, v) in &preview_hist {
        eprintln!(
            "  {k}: {v} = {:.1}%",
            100.0 * *v as f64 / preview_probed.max(1) as f64
        );
    }
    assert_eq!(total, 807, "语料 = 807(M2/M3/M4a 口径)");
    // §6.5 缺槽账(m4a 同式):807 图地形引用性缺档 = 0
    assert_eq!(
        total_missing_file, 0,
        "807 图地形引用性缺档=0(M4a 实测口径,场景合成地形层不变)"
    );
    let real_theaters = by_theater.values().filter(|v| v.2).count();
    assert!(
        real_theaters >= 3,
        "真彩抽样至少覆盖 TEMPERATE/SNOW/URBAN,实得 {real_theaters}"
    );
}

/// 零售规则表健全性:rulesmd.ini 解析 len() ≥ 100(零售 [OverlayTypes]
/// ~百级);至少一个 tiberium 游程;至少一个 bridge 前缀名;至少一个
/// Crate=yes。(桥前缀启发与 ore 游程等价性的零售证据,研究 §2 回填。)
#[test]
#[ignore]
fn m4b_overlay_table_retail_sanity() {
    let assets = TheaterAssets::open(&assets_dir()).unwrap();
    assert!(
        assets.lookup("rulesmd.ini").is_some(),
        "rulesmd.ini 应可达(expandmd01.mix,mix_integration.rs 实测口径)"
    );
    let (table, _art) = load_tables(&assets);
    eprintln!("OverlayTypes len = {}", table.len());
    assert!(
        table.len() >= 100,
        "零售 [OverlayTypes] ~百级,实得 {}",
        table.len()
    );
    // 字节语义:地图字节 0..=254 可达(0xFF = OVERLAY_NONE),表位扫描限界
    let n = table.len().min(255);
    let mut runs: Vec<(usize, usize)> = Vec::new(); // (游程基, 长) 连续 tiberium 极大段
    let mut bridge_names: Vec<&str> = Vec::new();
    let mut crates = 0usize;
    let mut i = 0usize;
    while i < n {
        let Some(k) = table.kind(i as u8) else {
            unreachable!("0..255 位 kind 恒 Some");
        };
        if k.tiberium {
            let base = i;
            while i < n && table.kind(i as u8).is_some_and(|k| k.tiberium) {
                i += 1;
            }
            runs.push((base, i - base));
            continue;
        }
        if k.bridge {
            bridge_names.push(k.image);
        }
        if k.is_crate {
            crates += 1;
        }
        i += 1;
    }
    eprintln!("tiberium 游程(基,长): {runs:?}");
    eprintln!("bridge 前缀名({}): {bridge_names:?}", bridge_names.len());
    eprintln!("Crate=yes 计数: {crates}");
    assert!(
        !runs.is_empty(),
        "零售表至少一个 tiberium 游程(display_byte 语义前提)"
    );
    assert!(
        !bridge_names.is_empty(),
        "零售表至少一个 bridge 前缀名(cell.cpp:1417 变体表消费前提)"
    );
    assert!(crates >= 1, "零售表至少一个 Crate=yes(帧 0 特判前提)");
}
