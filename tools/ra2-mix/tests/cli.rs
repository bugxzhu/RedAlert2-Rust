// SPDX-License-Identifier: GPL-3.0-only

use std::process::Command;

fn bin() -> std::path::PathBuf {
    env!("CARGO_BIN_EXE_ra2-mix").into()
}

/// 每个测试用独立临时目录(并行测试互不踩踏)。
fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("ra2-mix-cli-{tag}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn make_mix(path: &std::path::Path) {
    use ra2_core::format::crc::mix_name_id;
    let body: Vec<u8> = b"HELLO".to_vec();
    let mut v = Vec::new();
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&(body.len() as u32).to_le_bytes());
    v.extend_from_slice(&mix_name_id("HELLO").to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&(body.len() as u32).to_le_bytes());
    v.extend_from_slice(&body);
    std::fs::write(path, v).unwrap();
}

#[test]
fn list_and_cat_work() {
    let dir = temp_dir("list-cat");
    let mix_path = dir.join("t.mix");
    make_mix(&mix_path);

    let out = Command::new(bin())
        .args(["list", mix_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let expect_id = format!("{:08X}", ra2_core::format::crc::mix_name_id("HELLO"));
    assert!(stdout.contains(&expect_id));

    let out = Command::new(bin())
        .args(["cat", mix_path.to_str().unwrap(), "hello"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"HELLO");
}

#[test]
fn missing_entry_exits_1() {
    let dir = temp_dir("missing");
    let mix_path = dir.join("t.mix");
    make_mix(&mix_path);
    let out = Command::new(bin())
        .args(["cat", mix_path.to_str().unwrap(), "nope.ini"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn unknown_subcommand_exits_2_before_io() {
    // MIX 路径故意不可读:退出码 2 证明 usage 裁决先于文件 IO
    let out = Command::new(bin())
        .args(["frobnicate", "Z:/肯定不存在/nope.mix"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn extract_to_dir_writes_file() {
    let dir = temp_dir("extract");
    let mix_path = dir.join("t.mix");
    make_mix(&mix_path);
    let out_dir = dir.join("out");
    let out = Command::new(bin())
        .args([
            "extract",
            mix_path.to_str().unwrap(),
            "HELLO",
            "-o",
            out_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let written = std::fs::read(out_dir.join("HELLO")).expect("extract 应写文件");
    assert_eq!(written, b"HELLO");
}

#[test]
fn extract_write_failure_exits_1() {
    // -o 指向一个已存在的普通文件:create_dir_all 失败 → 退出 1,不 panic
    let dir = temp_dir("writefail");
    let mix_path = dir.join("t.mix");
    make_mix(&mix_path);
    let blocker = dir.join("blocker");
    std::fs::write(&blocker, b"x").unwrap();
    let out = Command::new(bin())
        .args([
            "extract",
            mix_path.to_str().unwrap(),
            "HELLO",
            "-o",
            blocker.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn non_utf8_arg_exits_2_not_panic() {
    // 旧实现 std::env::args() 遇非法 UTF-8 会 panic(退出 101)
    #[cfg(windows)]
    let bad = {
        use std::os::windows::ffi::OsStringExt;
        std::ffi::OsString::from_wide(&[0xD800, 0x006D, 0x0069, 0x0078]) // 未配对代理 + "mix"
    };
    #[cfg(unix)]
    let bad = {
        use std::os::unix::ffi::OsStringExt;
        std::ffi::OsString::from_vec(vec![0xFF, b'm', b'i', b'x'])
    };
    let dir = temp_dir("utf8");
    let mix_path = dir.join("t.mix");
    make_mix(&mix_path);
    let out = Command::new(bin())
        .args([bad, mix_path.into_os_string()])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}
