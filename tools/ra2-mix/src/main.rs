// SPDX-License-Identifier: GPL-3.0-only

//! ra2-mix:MIX 档案命令行工具(list / extract / cat)。
//!
//! 退出码契约:0 成功;1 找不到文件/解析失败/写失败;2 用法错误(含非 UTF-8 参数)。

use ra2_core::format::mix::MixArchive;
use std::io::Write;
use std::process::ExitCode;

fn usage() -> ! {
    eprintln!("用法: ra2-mix list <file.mix>");
    eprintln!("      ra2-mix extract <file.mix> <文件名> -o <目录|->");
    eprintln!("      ra2-mix cat <file.mix> <文件名>");
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

enum Action<'a> {
    List,
    Cat(&'a str),
    Extract(&'a str, &'a str),
}

fn main() -> ExitCode {
    // std::env::args() 遇非 UTF-8 参数会 panic;改用 args_os,非 UTF-8 按用法错误处理
    let mut args = Vec::new();
    for a in std::env::args_os().skip(1) {
        match a.into_string() {
            Ok(s) => args.push(s),
            Err(_) => usage(),
        }
    }
    // 先裁决用法,再触碰文件 IO(未知子命令即使 MIX 不可读也必须退出 2)
    let (mix_path, action) = match args.as_slice() {
        [cmd, mix_path, rest @ ..] => match (cmd.as_str(), rest) {
            ("list", []) => (mix_path.as_str(), Action::List),
            ("cat", [name]) => (mix_path.as_str(), Action::Cat(name.as_str())),
            ("extract", [name, flag, dir]) if flag == "-o" => {
                if dir == "-" {
                    (mix_path.as_str(), Action::Cat(name.as_str()))
                } else {
                    (
                        mix_path.as_str(),
                        Action::Extract(name.as_str(), dir.as_str()),
                    )
                }
            }
            _ => usage(),
        },
        _ => usage(),
    };
    run(mix_path, action)
}

fn run(mix_path: &str, action: Action<'_>) -> ExitCode {
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
    match action {
        Action::List => {
            // 先攒齐再一次性写出:list 中途断管不该留下半截 panic
            let mut listing = String::new();
            for e in mix.entries() {
                listing.push_str(&format!("{:08X} {:>10} {:>10}\n", e.id, e.offset, e.size));
            }
            write_stdout(listing.as_bytes())
        }
        Action::Cat(name) => match mix.read(name) {
            Ok(d) => write_stdout(d),
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        },
        Action::Extract(name, dir) => match mix.read(name) {
            Ok(d) => {
                let path = std::path::Path::new(dir).join(name);
                if let Some(parent) = path.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        eprintln!("创建目录 {} 失败: {e}", parent.display());
                        return ExitCode::from(1);
                    }
                }
                match std::fs::write(&path, d) {
                    Ok(()) => {
                        // 用 eprintln:println! 在管道关闭时会 panic,破坏退出码契约
                        eprintln!("已写入 {}", path.display());
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("写 {} 失败: {e}", path.display());
                        ExitCode::from(1)
                    }
                }
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        },
    }
}
