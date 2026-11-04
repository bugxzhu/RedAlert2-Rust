// SPDX-License-Identifier: GPL-3.0-only

//! ra2-map:地图解析命令行工具(info / extract-tile)。
//!
//! 退出码契约与 ra2-mix 相同:0 成功;1 找不到文件/解析失败/写失败/资源
//! 缺失;2 用法错误(含非 UTF-8 参数、坐标非整数)。usage 裁决先于任何
//! 文件 IO;stdout 断管(BrokenPipe)视为成功。

use ra2_core::format::map::meta::Theater;
use ra2_core::format::map::objects::ObjectClass;
use ra2_core::format::map::MapFile;
use ra2_core::format::mix::MixArchive;
use ra2_core::format::pal::Palette;
use std::io::Write;
use std::process::ExitCode;

mod png;

/// ra2.mix 内嵌调色板库的条目 ID(研究 §7.2;库本身按名可读)。
const PALETTE_MIX_ID: u32 = 0x3B5A_96DE;

fn usage() -> ! {
    eprintln!("用法: ra2-map info <file.map>");
    eprintln!("      ra2-map extract-tile <file.map> <x> <y> -o <file.png> [--assets <游戏目录>]");
    eprintln!("说明: extract-tile 输出该格 60×30 菱形占位渲染(填充色 = 剧场 ISO");
    eprintln!("      调色板第 tile%256 项);tile→TMP 真位图解析归 M3。缺 [--assets]");
    eprintln!("      时取环境变量 RA2_ASSETS_DIR。DESERT/NEWURBAN/LUNAR 的 YR 调色板");
    eprintln!("      不在本机安装中,将报缺资源(退出码 1)。");
    std::process::exit(2);
}

/// 把字节写 stdout;下游提前关闭(BrokenPipe,如 `| head`)视为成功。
fn write_stdout(data: &[u8]) -> ExitCode {
    let mut out = std::io::stdout();
    match out.write_all(data).and_then(|()| out.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("写 stdout 失败: {e}");
            ExitCode::from(1)
        }
    }
}

fn theater_name(t: Theater) -> &'static str {
    match t {
        Theater::Temperate => "TEMPERATE",
        Theater::Snow => "SNOW",
        Theater::Urban => "URBAN",
        Theater::Desert => "DESERT",
        Theater::NewUrban => "NEWURBAN",
        Theater::Lunar => "LUNAR",
    }
}

fn run_info(map_path: &str) -> ExitCode {
    let data = match std::fs::read(map_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("读取 {map_path} 失败: {e}");
            return ExitCode::from(1);
        }
    };
    let m = match MapFile::parse(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("解析失败: {e}");
            return ExitCode::from(1);
        }
    };
    let name = m
        .basic
        .as_ref()
        .and_then(|b| b.name.clone())
        .unwrap_or_else(|| "-".to_string());
    let s = m.pack_stats;
    let mut out = String::new();
    out.push_str(&format!("name={name}\n"));
    out.push_str(&format!("theater={}\n", theater_name(m.map_meta.theater)));
    out.push_str(&format!(
        "size={}x{}\n",
        m.map_meta.size[2], m.map_meta.size[3]
    ));
    out.push_str(&format!("tiles={}\n", m.tiles.len()));
    out.push_str(&format!(
        "structures={} units={} infantry={} aircraft={} terrain={} smudge={}\n",
        m.objects.count(ObjectClass::Structure),
        m.objects.count(ObjectClass::Unit),
        m.objects.count(ObjectClass::Infantry),
        m.objects.count(ObjectClass::Aircraft),
        m.objects.count(ObjectClass::Terrain),
        m.objects.count(ObjectClass::Smudge),
    ));
    out.push_str(&format!("malformed={}\n", m.objects.malformed));
    out.push_str(&format!(
        "packs: iso={} overlay={} overlay_data={} preview={} preview_truncated={}\n",
        s.iso_out, s.overlay_out, s.overlay_data_out, s.preview_out, s.preview_truncated
    ));
    out.push_str(&format!("logic_sections={}\n", m.logic_sections.len()));
    out.push_str(&format!(
        "players={}\n",
        m.basic.as_ref().map(|b| b.max_player).unwrap_or(0)
    ));
    out.push_str(&format!(
        "digest={}\n",
        if m.digest.is_some() { "yes" } else { "no" }
    ));
    write_stdout(out.as_bytes())
}

/// 剧场 ISO 调色板:ra2.mix → id 0x3B5A96DE 嵌套库 → <theater>.pal(768 B)。
fn load_theater_palette(assets: &std::path::Path, theater: Theater) -> Result<Palette, String> {
    let ra2 = MixArchive::parse(
        std::fs::read(assets.join("ra2.mix")).map_err(|e| format!("读 ra2.mix: {e}"))?,
    )
    .map_err(|e| format!("解析 ra2.mix: {e}"))?;
    let blob = ra2
        .read_by_id(PALETTE_MIX_ID)
        .map_err(|_| "ra2.mix 内找不到调色板库(id 3B5A96DE)".to_string())?
        .to_vec();
    let nested = MixArchive::parse(blob).map_err(|e| format!("解析调色板库: {e}"))?;
    let pal_bytes = nested
        .read(theater.iso_palette_file())
        .map_err(|e| {
            format!(
                "调色板库内读 {}: {e}(YR 三剧场调色板不在本机安装)",
                theater.iso_palette_file()
            )
        })?
        .to_vec();
    Palette::parse(&pal_bytes).map_err(|e| format!("调色板 768B 校验失败: {e}"))
}

fn run_extract_tile(
    map_path: &str,
    x: i64,
    y: i64,
    out_path: &str,
    assets: Option<&str>,
) -> ExitCode {
    let data = match std::fs::read(map_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("读取 {map_path} 失败: {e}");
            return ExitCode::from(1);
        }
    };
    let m = match MapFile::parse(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("解析失败: {e}");
            return ExitCode::from(1);
        }
    };
    // 坐标 → u16(负/超界按无记录处理,不 panic)
    let tile = match (usize::try_from(x), usize::try_from(y)) {
        (Ok(x), Ok(y)) if x <= 65535 && y <= 65535 => m.tiles.cell(x as u16, y as u16),
        _ => None,
    };
    let Some(tile) = tile else {
        eprintln!("格 ({x},{y}) 无瓦片记录");
        return ExitCode::from(1);
    };
    if tile.tile < 0 {
        eprintln!("格 ({x},{y}) 是无瓦片格(tile=-1)");
        return ExitCode::from(1);
    }
    let assets_dir = assets.map(std::path::PathBuf::from).or_else(|| {
        std::env::var("RA2_ASSETS_DIR")
            .ok()
            .map(std::path::PathBuf::from)
    });
    let Some(assets_dir) = assets_dir else {
        eprintln!("需要 --assets 或环境变量 RA2_ASSETS_DIR 指向游戏目录");
        return ExitCode::from(1);
    };
    let palette = match load_theater_palette(&assets_dir, m.map_meta.theater) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("解析剧场调色板失败: {e}");
            return ExitCode::from(1);
        }
    };
    // 60×30 菱形占位渲染:填充 = palette[tile%256](6 位→8 位换算);菱形外
    // 黑(0,0,0)。菱形谓词 |2cx-59| + 2·|2cy-29| ≤ 60(中心 (29.5,14.5),确定性、可复现)
    let idx = tile.tile as usize % 256;
    let [r, g, b] = palette.rgb8(idx);
    let (w, h) = (60u32, 30u32);
    let mut rgb = vec![0u8; 3 * w as usize * h as usize];
    for cy in 0..h as usize {
        for cx in 0..w as usize {
            let inside = (2 * cx as u32).abs_diff(59) + 2 * (2 * cy as u32).abs_diff(29) <= 60;
            if inside {
                let i = cy * 3 * w as usize + cx * 3;
                rgb[i] = r;
                rgb[i + 1] = g;
                rgb[i + 2] = b;
            }
        }
    }
    match png::write_png_file(std::path::Path::new(out_path), w, h, &rgb) {
        Ok(()) => {
            eprintln!(
                "已写 {out_path}(60×30 占位渲染;tile {} → 调色板项 {idx})",
                tile.tile
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("写 {out_path} 失败: {e}");
            ExitCode::from(1)
        }
    }
}

fn main() -> ExitCode {
    // std::env::args() 遇非 UTF-8 参数会 panic;args_os + usage(退出 2)
    let mut args = Vec::new();
    for a in std::env::args_os().skip(1) {
        match a.into_string() {
            Ok(s) => args.push(s),
            Err(_) => usage(),
        }
    }
    // 偏差记录:brief 骨架直接 match args.as_slice()(&[String] 对 &str 字面量
    // 模式类型不符,原文不可编译)——借一层 &str 视图后再做切片模式匹配。
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    // 先裁决用法,再触碰文件 IO
    enum Action<'a> {
        Info(&'a str),
        ExtractTile {
            map: &'a str,
            x: i64,
            y: i64,
            out: &'a str,
            assets: Option<&'a str>,
        },
    }
    let action = match argv.as_slice() {
        ["info", map] => Action::Info(map),
        ["extract-tile", map, x, y, flag, out, rest @ ..] if *flag == "-o" => {
            let Ok(x) = x.parse::<i64>() else {
                usage();
            };
            let Ok(y) = y.parse::<i64>() else {
                usage();
            };
            let assets = match rest {
                [] => None,
                ["--assets", dir] => Some(*dir),
                _ => usage(),
            };
            Action::ExtractTile {
                map,
                x,
                y,
                out,
                assets,
            }
        }
        _ => usage(),
    };
    match action {
        Action::Info(map) => run_info(map),
        Action::ExtractTile {
            map,
            x,
            y,
            out,
            assets,
        } => run_extract_tile(map, x, y, out, assets),
    }
}
