// SPDX-License-Identifier: GPL-3.0-only

//! ra2-map:地图解析命令行工具(info / extract-tile)。
//!
//! 退出码契约与 ra2-mix 相同:0 成功;1 找不到文件/解析失败/写失败/资源
//! 缺失;2 用法错误(含非 UTF-8 参数、坐标非整数)。usage 裁决先于任何
//! 文件 IO;stdout 断管(BrokenPipe)视为成功。

use ra2_core::format::map::mapdoc::decode_map_text;
use ra2_core::format::map::meta::Theater;
use ra2_core::format::map::objects::ObjectClass;
use ra2_core::format::map::MapFile;
use ra2_core::format::mix::MixArchive;
use ra2_core::format::pal::Palette;
use ra2_core::format::theater::{theater_spec, SlotFile, TheaterAssets, TheaterIni};
use ra2_core::format::theater_tmp::TsTmpFile;
use ra2_core::render::terrain::paint_canvas;
use std::io::Write;
use std::process::ExitCode;

mod png;

fn usage() -> ! {
    eprintln!("用法: ra2-map info <file.map>");
    eprintln!("      ra2-map extract-tile <file.map> <x> <y> -o <file.png> [--assets <游戏目录>]");
    eprintln!("说明: extract-tile 输出该格真瓦片位图(剧场 INI 反查 → TS TMP 槽位 →");
    eprintln!("      60×30 等距菱形 → 剧场调色板上色;行外区为调色板 0 号色)。");
    eprintln!("      缺 [--assets] 时取环境变量 RA2_ASSETS_DIR。DESERT/NEWURBAN/LUNAR");
    eprintln!("      的 YR 调色板不在本机安装中,将报缺资源(退出码 1)。");
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

/// 剧场 ISO 调色板:先全容器按名查(localmd/expandmd/ecache 等任意位置),
/// 回退 M2 链(ra2.mix → 匿名调色板库 id 0x3B5A96DE → 按名)。
fn load_theater_palette(assets: &TheaterAssets, theater: Theater) -> Result<Palette, String> {
    let name = theater.iso_palette_file();
    if let Some(bytes) = assets.lookup(name) {
        return Palette::parse(bytes).map_err(|e| format!("调色板 {name} 768B 校验失败: {e}"));
    }
    const PALETTE_MIX_ID: u32 = 0x3B5A_96DE;
    let blob = assets
        .lookup_id(PALETTE_MIX_ID)
        .ok_or_else(|| format!("调色板库(id 3B5A96DE)不可达,且 {name} 按名未找到"))?
        .to_vec();
    let lib = MixArchive::parse(blob).map_err(|e| format!("解析调色板库: {e}"))?;
    let pal = lib.read(name).map_err(|_| {
        format!("{name} 在本机安装缺席(DESERT/NEWURBAN/LUNAR 的 YR 调色板;活体验收用 TEMPERATE/SNOW/URBAN)")
    })?;
    Palette::parse(pal).map_err(|e| format!("调色板 768B 校验失败: {e}"))
}

fn run_extract_tile(
    map_path: &str,
    x: i64,
    y: i64,
    out_path: &str,
    assets_opt: Option<&str>,
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
    let assets_dir = assets_opt.map(std::path::PathBuf::from).or_else(|| {
        std::env::var("RA2_ASSETS_DIR")
            .ok()
            .map(std::path::PathBuf::from)
    });
    let Some(assets_dir) = assets_dir else {
        eprintln!("需要 --assets 或环境变量 RA2_ASSETS_DIR 指向游戏目录");
        return ExitCode::from(1);
    };
    let spec = theater_spec(m.map_meta.theater);
    let assets = match TheaterAssets::open(&assets_dir) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("资产装载失败: {e}");
            return ExitCode::from(1);
        }
    };
    let palette = match load_theater_palette(&assets, m.map_meta.theater) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("剧场调色板解析失败: {e}");
            return ExitCode::from(1);
        }
    };
    let ini_bytes = match assets.lookup(spec.ini_name) {
        Some(b) => b,
        None => {
            eprintln!("资产缺剧场表 {}", spec.ini_name);
            return ExitCode::from(1);
        }
    };
    let ini = match TheaterIni::parse(&decode_map_text(ini_bytes)) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("解析 {}: {e}", spec.ini_name);
            return ExitCode::from(1);
        }
    };
    let set_label = ini
        .set_of(tile.tile as usize)
        .map(|s| match s.name {
            Some(n) => n,
            None => format!("set{:04}", s.number),
        })
        .unwrap_or_else(|| "-".to_string());
    let slot_file = match ini.file_name(tile.tile as usize, spec.ext) {
        Some(sf) => sf,
        None => {
            eprintln!(
                "tile_id {} 越界: {} 界内槽位 {}(spec 容量 {})",
                tile.tile,
                ini.slot_count(),
                ini.slot_count(),
                spec.capacity
            );
            return ExitCode::from(1);
        }
    };
    let file_name = match slot_file {
        SlotFile::Reserved => {
            eprintln!(
                "tile_id {} 是保留槽(集合 {set_label},无自有文件)",
                tile.tile
            );
            return ExitCode::from(1);
        }
        SlotFile::File(name) => name,
    };
    let blob = match assets.lookup(&file_name) {
        Some(b) => b,
        None => {
            eprintln!("瓦片文件 {file_name} 未出货(缺文件槽;集合 {set_label})");
            return ExitCode::from(1);
        }
    };
    let origin = assets.origin(&file_name).unwrap_or("?").to_string();
    let tmp = match TsTmpFile::parse(blob) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("解析 {file_name}: {e}");
            return ExitCode::from(1);
        }
    };
    let sub = tile.sub_tile as usize;
    let slot = match tmp.slot(sub) {
        Some(s) => s,
        None => {
            eprintln!(
                "{file_name} 槽 {sub} 为空槽(槽网格 {}×{})",
                tmp.dims().0,
                tmp.dims().1
            );
            return ExitCode::from(1);
        }
    };
    let (cbx, cby, cx, cy) = tmp.dims();
    if cx != 2 * cy {
        eprintln!("{file_name} 画布 {cx}×{cy} 非 2:1 等距菱形,M3 渲染仅支持 2:1(语料全部 60×30)");
        return ExitCode::from(1);
    }
    let diamond = slot.diamond_pixels();
    let rgb = match paint_canvas(&palette, cx as usize, cy as usize, diamond) {
        Some(v) => v,
        None => {
            // 奇 cy 阶梯消耗 cx·cy/2−1 字节(如 6×3 用 8/9):debug_assert 升硬校验
            // (终审 RECORD),病态输入显式退出而非静默漏绘
            eprintln!(
                "{file_name} 画布 {cx}×{cy}:B 族阶梯消耗与菱形字节量不符(奇 cy 病态;语料 13,020 TMP 全 60×30)"
            );
            return ExitCode::from(1);
        }
    };
    match png::write_png_file(std::path::Path::new(out_path), cx, cy, &rgb) {
        Ok(()) => {
            eprintln!(
                "已写 {out_path}: {file_name} 槽 {sub}(剧场 {}, INI 容量 {}, 画布 {cx}×{cy}, 槽网格 {cbx}×{cby}, 来源 {origin})",
                format_args!("{:?}", m.map_meta.theater),
                ini.slot_count(),
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
