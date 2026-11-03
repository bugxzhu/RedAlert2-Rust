// SPDX-License-Identifier: GPL-3.0-only

use ra2_core::format::mix::MixArchive;
use std::io::Write;
use std::process::ExitCode;

fn usage() -> ! {
    eprintln!("用法: ra2-mix list <file.mix>");
    eprintln!("      ra2-mix extract <file.mix> <文件名> -o <目录|->");
    eprintln!("      ra2-mix cat <file.mix> <文件名>");
    std::process::exit(2);
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, mix_path, rest @ ..] => run(cmd, mix_path, rest),
        _ => usage(),
    }
}

fn run(cmd: &str, mix_path: &str, rest: &[String]) -> ExitCode {
    let data = match std::fs::read(mix_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("读取 {mix_path} 失败: {e}");
            return ExitCode::from(1);
        }
    };
    let mix = match MixArchive::parse(data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("解析失败: {e}");
            return ExitCode::from(1);
        }
    };
    match (cmd, rest) {
        ("list", []) => {
            for e in mix.entries() {
                println!("{:08X} {:>10} {:>10}", e.id, e.offset, e.size);
            }
            ExitCode::SUCCESS
        }
        ("cat", [name]) => write_out(&mix, name, None),
        ("extract", [name, out_flag, out_target]) if out_flag == "-o" => {
            if out_target == "-" {
                write_out(&mix, name, None)
            } else {
                write_out(&mix, name, Some(out_target))
            }
        }
        _ => usage(),
    }
}

fn write_out(mix: &MixArchive, name: &str, to_dir: Option<&str>) -> ExitCode {
    let data = match mix.read(name) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    match to_dir {
        None => {
            std::io::stdout().write_all(data).unwrap();
        }
        Some(dir) => {
            let path = std::path::Path::new(dir).join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, data).unwrap_or_else(|e| panic!("写 {}: {e}", path.display()));
            println!("已写入 {}", path.display());
        }
    }
    ExitCode::SUCCESS
}
