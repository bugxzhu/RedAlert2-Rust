// SPDX-License-Identifier: GPL-3.0-only

//! M4c 真资产验收(spec §6;研究 §5):807 全量场景渲染,主断言
//! overlay_fail == 0 && objects_fail == 0(判别式改帧级 flags bit1 门控后
//! 205 个失败文件全修复),总量 oracle 照研究 §5.1 照钉(负 oracle
//! missing_art 3539 防改名蒙混);守恒三恒等 / 画布公式 / 预算 / Preview
//! 口径维持 M4b 原样(spec §6.1-5:不收紧不放宽,RGB 占比跌破 85% = 真发现,
//! 停下带数回设计,禁止悄悄放宽);terrain 七桶与 M4b 基线
//! (.superpowers/sdd/task-8-assets-run2.log:521)逐桶对账打印,
//! missing_file == 0 维持(解析器改动不得波及地形层)。
//! 单一主测试,ignore,需 RA2_ASSETS_DIR;M4b 验收(m4b_acceptance.rs)
//! 原样保留作回归绊线。
//!
//! 运行:RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" \
//!   cargo test -p ra2-core --test m4c_acceptance -- --ignored --nocapture

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
    v.sort();
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

/// PPM 直写(P6 二进制;目检样本用)。
fn write_ppm(out: &std::path::Path, w: u32, h: u32, rgb: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut f = std::fs::File::create(out)?;
    write!(f, "P6\n{w} {h}\n255\n")?;
    f.write_all(rgb)
}

/// Preview 差异探针(m4b_acceptance probe_preview_diff 同款):最近邻缩采样
/// (渲染 → preview 尺寸),通道序(RGB/BGR)× 行序(正常/上下翻)四组合
/// 取最小差率——preview.rs 像素/通道序 M2 裁定走向 A(RGB 85.6%)。逐图
/// 打印,返回最优组合名供直方图统计。
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
                    diff += 1; // 通道和差 > 96 记差异(探针阈值,M4b 裁定)
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

/// rulesmd/artmd/art 表装载(m4b 同款):rulesmd 缺席 = 空表(lenient
/// 契约,全计 no_type),art 缺席层 = 空文档(合成缺省)。
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

/// 逐剧场分档聚合(overlay 4 桶 + 物件 5 桶 + 信息位;m4b 同构)。
#[derive(Default)]
struct TheaterAgg {
    overlay: [usize; 4], // drawn, no_type, missing_file, fail
    objects: [usize; 5], // drawn, skipped_voxel, missing_art, malformed, fail
    frame_clamp: usize,
    clip: usize,
}

/// 全量渲染 + M4c 断言(spec §6.1 全量)。
#[test]
#[ignore]
fn m4c_all_maps_scene_render() {
    let assets = TheaterAssets::open(&assets_dir()).unwrap();
    let (table, art) = load_tables(&assets);
    let mut total = 0usize;
    let mut total_ms = 0u128;
    let mut max_ms: (u128, String) = (0, String::new());
    let mut all_ms: Vec<u128> = Vec::new();
    let mut over = [0usize; 4]; // drawn, no_type, missing_file, fail
    let mut objs = [0usize; 5]; // drawn, voxel, missing_art, malformed, fail
    let mut total_z_fallback = 0usize;
    // terrain 七桶对账累计(spec §6.1-6;桶序 = m4b 恒等式顺序:painted,
    // no_tile, missing_file, reserved, out_of_range, missing_slot, parse_fail)
    let mut terrain_cells = 0usize;
    let mut terrain_totals = [0usize; 7];
    let mut total_frame_clamp = 0usize;
    let mut total_clip = 0usize;
    // 剧场 → (渲染数, 真彩数, 已抽样?, 分档)
    let mut by_theater: std::collections::BTreeMap<String, (usize, usize, bool, TheaterAgg)> =
        Default::default();
    // Preview 最优组合直方图(真彩口径;断言照 M4b 原样,spec §6.1-5)
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
        // 预算(spec §6.1-4):M4b 实测最慢 3526ms ×3 = 10578;研究 §5.5:
        // 解码增量 µs-ms 级,少一次 RAW 兜底试探反而略省
        assert!(
            ms < 10_578,
            "{} 耗时 {ms}ms 超预算(实测 max×3,硬帽 15s)",
            path.display()
        );
        let s = r.stats;
        // M4c 主断言(spec §6.1-1,逐图零容忍):判别式改帧级 flags bit1
        // 门控后 205 个失败文件全修复,任一非零即停(带图名停下回设计)
        assert_eq!(
            s.overlay_fail,
            0,
            "{}: overlay_fail 非零——判别式遗漏面,停下带数回设计",
            path.display()
        );
        assert_eq!(
            s.objects_fail,
            0,
            "{}: objects_fail 非零——判别式遗漏面,停下带数回设计",
            path.display()
        );
        total += 1;
        total_ms += ms;
        all_ms.push(ms);
        if ms > max_ms.0 {
            max_ms = (ms, path.display().to_string());
        }
        // terrain 七桶累计 + 守恒 1:地形七桶恒等(cells = 其和)
        terrain_cells += s.terrain.cells;
        let tb = [
            s.terrain.painted,
            s.terrain.no_tile,
            s.terrain.missing_file,
            s.terrain.reserved,
            s.terrain.out_of_range,
            s.terrain.missing_slot,
            s.terrain.parse_fail,
        ];
        for (a, v) in terrain_totals.iter_mut().zip(tb) {
            *a += v;
        }
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
        // 守恒 2:非空 overlay 四桶(独立重数,overlay pass 同域 = d 升序
        // records;scene.rs 排序键 u32::from(x)+u32::from(y),此处同式重构
        // 不信 stats)
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
        // 守恒 3:非 Smudge 物件五桶(objects.rs tests assert_conservation 同式)
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
        // 画布公式独立重算(M4a 口径):W/H 均由 walk_extremes + 实扫 z_max
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
        total_z_fallback += s.objects_z_fallback;
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
                    "ra2-m4c-scene-sample-{}.ppm",
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
    // 总账行(控制器对账输入;M4b 基线 = task-8-assets-run2.log:521-524)
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
    eprintln!("M4c: 共 {total} 图,总耗时 {total_ms}ms,最慢 {max_ms:?},均值 {mean}ms,p90 {p90}ms");
    eprintln!(
        "M4c overlay 总账: drawn {} no_type {} missing_file {} fail {}(frame_clamp {total_frame_clamp}, clip {total_clip})",
        over[0], over[1], over[2], over[3]
    );
    eprintln!(
        "M4c objects 总账: drawn {} voxel {} missing_art {} malformed {} fail {} z_fallback {total_z_fallback}",
        objs[0], objs[1], objs[2], objs[3], objs[4]
    );
    // terrain 七桶对账打印(M4b 基线 log:521 = missing_file 总计 0;硬断言
    // 在下方,其余桶以逐图七桶恒等 + 打印对账)
    eprintln!(
        "M4c terrain 对账(M4b 基线 missing_file 总计 0): cells {terrain_cells} painted {} no_tile {} missing_file {} reserved {} out_of_range {} missing_slot {} parse_fail {}",
        terrain_totals[0], terrain_totals[1], terrain_totals[2],
        terrain_totals[3], terrain_totals[4], terrain_totals[5], terrain_totals[6]
    );
    let sprites = over[0] + objs[0];
    eprintln!(
        "M4c clip 占比: {total_clip}/{sprites} = {:.2}%",
        100.0 * total_clip as f64 / sprites.max(1) as f64
    );
    for (k, (maps_n, real_n, sampled, agg)) in &by_theater {
        eprintln!(
            "剧场 {k}: {maps_n} 图(真彩 {real_n},抽样 {sampled})overlay[drawn {} no_type {} missing_file {} fail {}] objects[drawn {} voxel {} missing_art {} malformed {} fail {}] frame_clamp {} clip {}",
            agg.overlay[0], agg.overlay[1], agg.overlay[2], agg.overlay[3],
            agg.objects[0], agg.objects[1], agg.objects[2], agg.objects[3], agg.objects[4],
            agg.frame_clamp, agg.clip
        );
    }
    eprintln!("M4c preview 直方图(真彩含 Preview 共 {preview_probed} 图):");
    for (k, v) in &preview_hist {
        eprintln!(
            "  {k}: {v} = {:.1}%",
            100.0 * *v as f64 / preview_probed.max(1) as f64
        );
    }
    // === M4c 总量 oracle(spec §6.1-2;研究 §5.1 修复后预测,语料冻结前提;
    // 数值逐字照钉,不得改写或四舍五入)===
    assert_eq!(total, 807, "语料 = 807(M2/M3/M4a/M4b 口径)");
    assert_eq!(
        over[0], 264211,
        "overlay_drawn == 264211(人口恒等:235265 drawn + 28946 fail)"
    );
    assert_eq!(over[1], 0, "overlay_no_type == 0");
    assert_eq!(over[2], 0, "overlay_missing_file == 0");
    assert_eq!(over[3], 0, "overlay_fail == 0(主断言总量口径)");
    assert_eq!(
        objs[0], 514502,
        "objects_drawn == 514502(511347 drawn + 3155 fail)"
    );
    assert_eq!(objs[1], 8320, "objects_skipped_voxel == 8320");
    assert_eq!(
        objs[2], 3539,
        "objects_missing_art == 3539(负 oracle:真缺档 95 名,TS 语源未随 YR 零售出货;防改名蒙混)"
    );
    assert_eq!(objs[3], 1, "objects_malformed == 1");
    assert_eq!(objs[4], 0, "objects_fail == 0(主断言总量口径)");
    assert_eq!(
        total_z_fallback, 9288,
        "objects_z_fallback == 9288(研究 §4.7 语料量级)"
    );
    // === Preview 口径照 M4b 原样维持(spec §6.1-5:不收紧不放宽)===
    assert_eq!(preview_probed, 507, "真彩含 Preview 图数钉死(T8 实测)");
    let (best_combo, best_count) = preview_hist
        .iter()
        .max_by_key(|(_, v)| **v)
        .map(|(k, v)| (*k, *v))
        .unwrap_or(("", 0));
    assert_eq!(
        best_combo, "RGB",
        "最众组合钉 RGB(M4b T8 实测 85.6%;跌破 85% = 真发现,停下带数回设计,禁止悄悄放宽)"
    );
    assert!(
        best_count as f64 / preview_probed.max(1) as f64 >= 0.85,
        "RGB 稳定占比 ≥85%(实得 {best_combo} {best_count}/{preview_probed})"
    );
    // terrain 对账硬面(spec §6.1-6:M4b 基线 missing_file 总计 0;逐图七桶
    // 恒等已在循环内断言 = 异常即停;解析器改动不得波及地形层)
    assert_eq!(
        terrain_totals[2], 0,
        "terrain missing_file == 0(M4b 基线 task-8-assets-run2.log:521)"
    );
    let real_theaters = by_theater.values().filter(|v| v.2).count();
    assert!(
        real_theaters >= 3,
        "真彩抽样至少覆盖 TEMPERATE/SNOW/URBAN,实得 {real_theaters}"
    );
}
