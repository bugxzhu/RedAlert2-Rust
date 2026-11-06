// SPDX-License-Identifier: GPL-3.0-only

//! M4a 真资产验收(spec §6 + 勘误):807 图全量合成零 panic + 真彩剧场
//! 抽样 PPM + 画布尺寸公式钉死 + 内嵌 Preview 差异探针(四组合最小,
//! 输出供控制器裁定对拍走向)+ 性能预算。
//! 调色板边界(2026-11-06 探针):isodes/isoubn/isolun.pal 本机缺席
//! (全容器按名 + 0x3B5A96DE 库回退均无)→ desert/newurban/lunar 用
//! 哑调色板跑链路零 panic,不参与抽样/对拍。全部 ignore,需 RA2_ASSETS_DIR。

use ra2_core::format::map::mapdoc::decode_map_text;
use ra2_core::format::map::MapFile;
use ra2_core::format::pal::Palette;
use ra2_core::format::theater::{theater_spec, TheaterAssets, TheaterIni};
use ra2_core::render::terrain::render_terrain;

fn assets_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(
        std::env::var("RA2_ASSETS_DIR").expect("RA2_ASSETS_DIR 指向用户资产目录"),
    )
}

/// 递归收集 .map(m3_acceptance.rs:20-31 同构手法;语料 807 = Maps/ 540
/// + RMG MapUnits 267;扩展名只有 .map,无 .yrm)。
fn maps() -> Vec<std::path::PathBuf> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
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

/// PPM 直写(P3 太大用 P6 二进制;目检样本用,core 测试不依赖 tools png)。
fn write_ppm(out: &std::path::Path, w: u32, h: u32, rgb: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut f = std::fs::File::create(out)?;
    write!(f, "P6\n{w} {h}\n255\n")?;
    f.write_all(rgb)
}

/// 全量渲染:零 panic + 尺寸公式 + 真彩剧场首图抽样 PPM + 耗时/缺档总账。
#[test]
#[ignore]
fn m4a_all_maps_render_and_samples() {
    let assets = TheaterAssets::open(&assets_dir()).unwrap();
    let mut total = 0usize;
    let mut total_ms = 0u128;
    let mut max_ms: (u128, String) = (0, String::new());
    let mut total_missing_file = 0usize;
    // 剧场 → (渲染数, 真彩数, 已抽样?)
    let mut by_theater: std::collections::BTreeMap<String, (usize, usize, bool)> =
        Default::default();
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
        .unwrap();
        let (pal, real) = theater_palette(&assets, m.map_meta.theater);
        let t0 = std::time::Instant::now();
        let r = render_terrain(&m, &assets, &ini, &pal).expect("全量合成零 panic");
        let ms = t0.elapsed().as_millis();
        // 预算=实测 max 1084ms(被遗忘者.map)×3≈3252 取 3500,硬帽 10s 内(spec §9 任务③)。
        assert!(ms < 3500, "{} 耗时 {ms}ms 超 M4a 预算", path.display());
        total += 1;
        total_ms += ms;
        if ms > max_ms.0 {
            max_ms = (ms, path.display().to_string());
        }
        total_missing_file += r.stats.missing_file;
        let (w, h) = m.size_wh();
        // H 反推 z_max 兼作一致性校验(H 公式钉死)
        let z_max = (r.height as usize - 15 * (w as usize + h as usize)) / 15;
        assert_eq!(
            r.width as usize,
            30 * (w as usize + h as usize),
            "{}",
            path.display()
        );
        assert_eq!(
            r.height as usize,
            15 * (w as usize + h as usize) + 15 * z_max
        );
        let key = format!("{:?}", m.map_meta.theater);
        let entry = by_theater.entry(key).or_insert((0, 0, false));
        entry.0 += 1;
        if real {
            entry.1 += 1;
            if !entry.2 {
                entry.2 = true;
                let out = std::env::temp_dir().join(format!(
                    "ra2-m4a-sample-{}.ppm",
                    path.file_name().unwrap().to_string_lossy()
                ));
                write_ppm(&out, r.width, r.height, &r.rgb).unwrap();
                eprintln!("样本: {}", out.display());
            }
            // Preview 对拍仅对真彩图有意义(哑调色板必 100% 差)
            if let Some(pv) = &m.preview {
                probe_preview_diff(&path, &r, pv);
            }
        }
    }
    eprintln!(
        "M4a: 共 {total} 图,总耗时 {total_ms}ms,最慢 {max_ms:?},missing_file 总计 {total_missing_file}"
    );
    for (k, (n, real_n, sampled)) in &by_theater {
        eprintln!("剧场 {k}: {n} 图(真彩 {real_n},抽样 {sampled})");
    }
    assert_eq!(total, 807, "语料 = 807(M2/M3 口径)");
    // §6.5 缺槽账:807 图引用性缺档 = 0(M3 账 0/78/0/0/383/44 是 INI
    // 结构性未出货占位,真图不引用;2026-11-06 实测总计 0)。
    assert_eq!(
        total_missing_file, 0,
        "807 图引用性缺档=0(M3 账 0/78/0/0/383/44 是 INI 结构性未出货占位,真图不引用)"
    );
    let real_theaters = by_theater.values().filter(|v| v.2).count();
    assert!(
        real_theaters >= 3,
        "真彩抽样至少覆盖 TEMPERATE/SNOW/URBAN,实得 {real_theaters}"
    );
    // 剧场分布钉死(2026-11-06 实测权威口径,合计 807)。
    let pinned: std::collections::BTreeMap<&str, usize> = [
        ("Temperate", 324),
        ("Snow", 106),
        ("Urban", 78),
        ("Desert", 170),
        ("NewUrban", 128),
        ("Lunar", 1),
    ]
    .into_iter()
    .collect();
    for (name, n) in &pinned {
        assert_eq!(
            by_theater.get(*name).map(|v| v.0).unwrap_or(0),
            *n,
            "剧场 {name} 图数钉死(2026-11-06 实测口径)"
        );
    }
}

/// Preview 差异探针:最近邻缩采样(渲染 → preview 尺寸),通道序
/// (RGB/BGR)× 行序(正常/上下翻)四组合取最小——preview.rs:像素/通道
/// 序 M2 未裁决,取最小即"最优解释下的差率"。逐图打印,控制器裁定。
/// 2026-11-06 裁定=走向 B(预览含 M4a 未渲染层):507 真彩图最优组合
/// 差率 <10% 为 0、中位 91.3%、组合不集中(RGB 207/RGB+flip 242/BGR 28/
/// BGR+flip 30)→ 系统性差异,非通道/行序可解释;对拍顺延 M4b 终验。
/// 量级注记:中位 91% 超出"仅矿石 overlay"可解释范围,M4b 须复核
/// 视口原点/缩放/裁边语义。不设阈值断言。
fn probe_preview_diff(
    path: &std::path::Path,
    r: &ra2_core::render::TerrainRender,
    pv: &ra2_core::format::map::preview::PreviewImage,
) {
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
}
