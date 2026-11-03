// SPDX-License-Identifier: GPL-3.0-only

use ra2_core::format::crc::mix_name_id;
use std::process::Command;

fn bin() -> std::path::PathBuf {
    env!("CARGO_BIN_EXE_ra2-mix").into()
}

/// 用 ra2-core 现场构造一个合成 MIX 供 CLI 测试。
fn make_mix(path: &std::path::Path) {
    let body: Vec<u8> = b"HELLO".to_vec();
    let mut v = Vec::new();
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&(body.len() as u32).to_le_bytes());
    let id = mix_name_id("HELLO");
    v.extend_from_slice(&id.to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&5u32.to_le_bytes());
    v.extend_from_slice(&body);
    std::fs::write(path, v).unwrap();
}

#[test]
fn list_and_cat_work() {
    let dir = std::env::temp_dir().join("ra2-mix-cli-test");
    std::fs::create_dir_all(&dir).unwrap();
    let mix_path = dir.join("t.mix");
    make_mix(&mix_path);

    let out = Command::new(bin())
        .args(["list", mix_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(format!("{:08X}", mix_name_id("HELLO")).as_str()));

    let out = Command::new(bin())
        .args(["cat", mix_path.to_str().unwrap(), "hello"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"HELLO");
}

#[test]
fn missing_file_exits_nonzero() {
    let dir = std::env::temp_dir().join("ra2-mix-cli-test");
    std::fs::create_dir_all(&dir).unwrap();
    let mix_path = dir.join("t.mix");
    make_mix(&mix_path);
    let out = Command::new(bin())
        .args(["cat", mix_path.to_str().unwrap(), "nope.ini"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}
