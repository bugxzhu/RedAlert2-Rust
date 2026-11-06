// SPDX-License-Identifier: GPL-3.0-only

use std::process::Command;

fn bin() -> std::path::PathBuf {
    env!("CARGO_BIN_EXE_ra2-map").into()
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("ra2-map-cli-{tag}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

// —— 微型编码器(与 core map::tests 同方案;测试复制,不导出)——
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

// 偏差记录:brief 原文的该助手 size_in=4 且无 LCW 0x80 终止符(已知 bug 第 4 次
// 出现)——解码器读完 0xFE 长填充后继续读下一 opcode 会 UnexpectedEof;按
// pack.rs 测试的已验证形态修正(size_in=5,载荷以 0x80 终止)。
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
            let n =
                ((bytes[bytes.len() - 2] as u32) << 16) | ((bytes[bytes.len() - 1] as u32) << 8);
            s.push(A[(n >> 18) as usize & 63] as char);
            s.push(A[(n >> 12) as usize & 63] as char);
            s.push(A[(n >> 6) as usize & 63] as char);
            s.push('=');
        }
        _ => {}
    }
    s
}

fn push_pack_section(ini: &mut String, name: &str, blob: &[u8]) {
    ini.push_str(&format!("[{name}]\n"));
    let text = b64(blob);
    let mut i = 0usize;
    let mut k = 1usize;
    while i < text.len() {
        let n = (text.len() - i).min(70);
        ini.push_str(&format!("{k}={}\n", &text[i..i + n]));
        i += n;
        k += 1;
    }
}

fn rec(x: u16, y: u16, tile: i16, sub: u8, z: u8) -> [u8; 11] {
    let mut b = [0u8; 11];
    b[0..2].copy_from_slice(&x.to_le_bytes());
    b[2..4].copy_from_slice(&y.to_le_bytes());
    b[4..6].copy_from_slice(&tile.to_le_bytes());
    b[8] = sub;
    b[9] = z;
    b
}

/// 与 core 测试同一张 mini 地图(SNOW 4×4,2 瓦片,1 结构 1 地形,预览 2×1);
/// 与 core 版的差异:无 [Actions] 段 → logic_sections=1(仅 Waypoints)。
fn mini_map() -> Vec<u8> {
    let mut tiles = Vec::new();
    tiles.extend_from_slice(&rec(1, 1, 5, 0, 0));
    tiles.extend_from_slice(&rec(3, 2, 6, 2, 1));
    tiles.extend_from_slice(&[0, 0, 0, 0]);
    let mut ini = String::new();
    ini.push_str("[Basic]\nName=TestMap\nOfficial=yes\nMaxPlayer=4\n");
    ini.push_str("[Map]\nTheater=SNOW\nSize=0,0,4,4\nLocalSize=0,0,4,4\n");
    ini.push_str("[Header]\nWidth=3\nHeight=3\nNumberStartingPoints=2\n");
    ini.push_str("[Lighting]\nAmbient=1.000000\n");
    ini.push_str("[Preview]\nSize=0,0,2,1\n");
    push_pack_section(
        &mut ini,
        "PreviewPack",
        &lzo_literal_stream(&[0xFF, 0, 0, 0, 0xFF, 0]),
    );
    push_pack_section(&mut ini, "OverlayPack", &lcw_fill_stream(0xFF, 262144));
    push_pack_section(&mut ini, "OverlayDataPack", &lcw_fill_stream(0x00, 262144));
    push_pack_section(&mut ini, "IsoMapPack5", &lzo_literal_stream(&tiles));
    ini.push_str("[Structures]\n0=Neutral,CABHUT,256,2,3,0,None,0,0,1,0,0,None,None,None,0,0\n");
    ini.push_str("[Terrain]\n2003=TIBTRE01\n");
    ini.push_str("[Waypoints]\n0=2003\n");
    ini.push_str("[Digest]\n1=qSt1w6I0r+9EWkfWl7XFZMt0Vo4=\n");
    ini.into_bytes()
}

#[test]
fn usage_errors_exit_2_before_io() {
    // 路径故意不可读:退出码 2 证明 usage 裁决先于文件 IO
    for args in [
        vec![],
        vec!["frobnicate", "Z:/不存在/nope.map"],
        vec!["info"],
        vec!["extract-tile", "Z:/不存在/nope.map", "1", "1"],
        vec!["extract-tile", "Z:/不存在/nope.map", "x", "1", "-o", "-"],
    ] {
        let out = Command::new(bin()).args(&args).output().unwrap();
        assert_eq!(out.status.code(), Some(2), "args={args:?}");
    }
}

#[test]
fn non_utf8_arg_exits_2_not_panic() {
    #[cfg(windows)]
    let bad = {
        use std::os::windows::ffi::OsStringExt;
        std::ffi::OsString::from_wide(&[0xD800, 0x0069])
    };
    #[cfg(unix)]
    let bad = {
        use std::os::unix::ffi::OsStringExt;
        std::ffi::OsString::from_vec(vec![0xFF, b'i'])
    };
    let out = Command::new(bin()).arg(bad).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn info_on_synthetic_map() {
    let dir = temp_dir("info");
    let p = dir.join("t.map");
    std::fs::write(&p, mini_map()).unwrap();
    let out = Command::new(bin())
        .args(["info", p.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("name=TestMap"), "{stdout}");
    assert!(stdout.contains("theater=SNOW"));
    assert!(stdout.contains("size=4x4"));
    assert!(stdout.contains("tiles=2"));
    assert!(stdout.contains("structures=1"));
    assert!(stdout.contains("terrain=1"));
    assert!(stdout.contains("packs: iso=26 overlay=262144 overlay_data=262144 preview=6"));
    assert!(stdout.contains("logic_sections=1"));
    assert!(stdout.contains("players=4"));
    assert!(stdout.contains("digest=yes"));
}

#[test]
fn info_failures_exit_1() {
    let dir = temp_dir("infofail");
    let missing = Command::new(bin())
        .args(["info", dir.join("nope.map").to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(1));
    let bad = dir.join("bad.map");
    std::fs::write(&bad, b"hello world\n").unwrap(); // 无 [Map] 段 → Invalid
    let out = Command::new(bin())
        .args(["info", bad.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn extract_tile_without_assets_exits_1() {
    // 无 --assets 且进程内移除 RA2_ASSETS_DIR → 缺资源退出 1,不 panic
    let dir = temp_dir("extract");
    let p = dir.join("t.map");
    std::fs::write(&p, mini_map()).unwrap();
    let out_path = dir.join("tile.png");
    let mut cmd = Command::new(bin());
    cmd.args([
        "extract-tile",
        p.to_str().unwrap(),
        "1",
        "1",
        "-o",
        out_path.to_str().unwrap(),
    ]);
    cmd.env_remove("RA2_ASSETS_DIR");
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(!out_path.exists());
}

// 用法错:render 缺 -o → 退出 2(usage 裁决先于文件 IO)
#[test]
fn render_usage_errors_exit_2() {
    let out = Command::new(bin())
        .args(["render", "whatever.map"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

// 缺资产目录(无 --assets 且 RA2_ASSETS_DIR 清空)→ 退出 1 且 stderr 提示。
// 偏差记录:brief 原文用不存在的 "whatever.map"——run_render 先读文件后查
// 资产,读失败同样退出 1 但 stderr 不含 RA2_ASSETS_DIR,第二断言必挂;改用
// 合成 mini_map 走到资产分支(与 extract_tile_without_assets_exits_1 同手法)。
#[test]
fn render_needs_assets_exit_1() {
    let dir = temp_dir("render-noassets");
    let p = dir.join("t.map");
    std::fs::write(&p, mini_map()).unwrap();
    let out_path = dir.join("x.png"); // 独立临时路径:断言失败后不落盘(残余 5)
    let out = Command::new(bin())
        .args([
            "render",
            p.to_str().unwrap(),
            "-o",
            out_path.to_str().unwrap(),
        ])
        .env_remove("RA2_ASSETS_DIR")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("RA2_ASSETS_DIR"), "{stderr}");
    assert!(!out_path.exists(), "失败路径不得产出文件");
}

/// 解码 png.rs 的单 IDAT(zlib store)→ 原始扫描行(每行 1 过滤字节 + 3·w)。
fn decode_store_idat(png: &[u8]) -> Vec<u8> {
    let mut pos = 8usize;
    let mut idat: Vec<u8> = Vec::new();
    while pos + 12 <= png.len() {
        let len = u32::from_be_bytes(png[pos..pos + 4].try_into().unwrap()) as usize;
        let kind = &png[pos + 4..pos + 8];
        if kind == b"IDAT" {
            idat.extend_from_slice(&png[pos + 8..pos + 8 + len]);
        }
        if kind == b"IEND" {
            break;
        }
        pos += 12 + len;
    }
    assert_eq!(&idat[..2], &[0x78, 0x01], "zlib store 头");
    let mut out = Vec::new();
    let mut p = 2usize;
    loop {
        let header = idat[p];
        let n = u16::from_le_bytes(idat[p + 1..p + 3].try_into().unwrap()) as usize;
        out.extend_from_slice(&idat[p + 5..p + 5 + n]);
        p += 5 + n;
        if header & 1 == 1 {
            break; // BFINAL
        }
    }
    out
}

/// 不同 RGB 三元组数(跳过每行过滤字节)。
fn distinct_colors(scanlines: &[u8], w: u32) -> usize {
    use std::collections::BTreeSet;
    let stride = 1 + 3 * w as usize;
    let mut set = BTreeSet::new();
    let mut y = 0usize;
    while y * stride + stride <= scanlines.len() {
        let row = &scanlines[y * stride + 1..y * stride + stride];
        for px in row.as_chunks::<3>().0 {
            set.insert(*px);
        }
        y += 1;
    }
    set.len()
}

/// 真图活体验收(M3;spec §6-6 按歧义裁决 #1 落地):TEMPERATE/SNOW/URBAN
/// 各一图 extract-tile 产出 60×30、像素多值的 PNG(非占位非单色);
/// DESERT 图因 isodes.pal 缺席退出 1(stderr 提调色板);info 三剧场不变。
#[test]
#[ignore]
fn real_maps_extract_tile_three_theaters() {
    use ra2_core::format::map::MapFile;
    let assets = std::env::var("RA2_ASSETS_DIR").expect("需要 RA2_ASSETS_DIR");
    let dir = temp_dir("real-m3");
    for rel in [
        "Maps/Cooperative/c1a01md.map", // TEMPERATE
        "Maps/Cooperative/c1a03md.map", // SNOW
        "Maps/Standard/hillbtwn.map",   // URBAN
    ] {
        let map_path = std::path::Path::new(&assets).join(rel);
        // 取首个有效瓦片格(确定性;不硬编码坐标)
        let data = std::fs::read(&map_path).unwrap();
        let m = MapFile::parse(&data).unwrap();
        let rec = m
            .tiles
            .records()
            .iter()
            .find(|r| r.tile >= 0)
            .expect("至少一条瓦片记录");
        let out_path = dir.join("tile.png");
        let out = Command::new(bin())
            .args([
                "extract-tile",
                map_path.to_str().unwrap(),
                &rec.x.to_string(),
                &rec.y.to_string(),
                "-o",
                out_path.to_str().unwrap(),
            ])
            .env("RA2_ASSETS_DIR", &assets)
            .output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(0),
            "{rel}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let png = std::fs::read(&out_path).unwrap();
        assert_eq!(png[16..20], 60u32.to_be_bytes(), "{rel}");
        assert_eq!(png[20..24], 30u32.to_be_bytes(), "{rel}");
        let scanlines = decode_store_idat(&png);
        assert!(
            distinct_colors(&scanlines, 60) >= 2,
            "{rel} 应像素多值(非占位)"
        );
        let _ = std::fs::remove_file(&out_path);
    }
    // DESERT 负向(歧义裁决 #1):isodes.pal 缺席 → 退出 1
    let map_path = std::path::Path::new(&assets).join("Maps/Cooperative/c2s02md.map");
    let data = std::fs::read(&map_path).unwrap();
    let m = MapFile::parse(&data).unwrap();
    let rec = m.tiles.records().iter().find(|r| r.tile >= 0).unwrap();
    let out_path = dir.join("tile-desert.png");
    let out = Command::new(bin())
        .args([
            "extract-tile",
            map_path.to_str().unwrap(),
            &rec.x.to_string(),
            &rec.y.to_string(),
            "-o",
            out_path.to_str().unwrap(),
        ])
        .env("RA2_ASSETS_DIR", &assets)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("调色板"), "{stderr}");
    assert!(!out_path.exists());
    // info 三剧场冒烟(info 不需要调色板,M2 契约)
    for (rel, theater) in [
        ("Maps/Cooperative/c1a01md.map", "TEMPERATE"),
        ("Maps/Cooperative/c1a03md.map", "SNOW"),
        ("Maps/Cooperative/c2s02md.map", "DESERT"),
    ] {
        let m = std::path::Path::new(&assets).join(rel);
        let out = Command::new(bin())
            .args(["info", m.to_str().unwrap()])
            .env("RA2_ASSETS_DIR", &assets)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "{rel}");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains(&format!("theater={theater}")),
            "{rel}: {stdout}"
        );
    }
}

/// render 成功路径活体(需 RA2_ASSETS_DIR;补 I2 成功路径缺口):TEMPERATE
/// 一图渲染到临时 PNG,退出 0、文件非空、IHDR 尺寸 == 走位极值画布公式
/// (30·(x−y 跨度)+60 × 15·(d 跨度)+30+15·z_max,walk_extremes 独立重算)。
#[test]
#[ignore]
fn real_map_render_outputs_png() {
    use ra2_core::format::map::MapFile;
    use ra2_core::render::terrain::walk_extremes;
    let assets = std::env::var("RA2_ASSETS_DIR").expect("需要 RA2_ASSETS_DIR");
    let map_path = std::path::Path::new(&assets).join("Maps/Cooperative/c1a01md.map");
    let data = std::fs::read(&map_path).unwrap();
    let m = MapFile::parse(&data).unwrap();
    let (w, h) = m.size_wh();
    let (d_min, d_max, xmy_min, xmy_max) = walk_extremes(w, h);
    let z_max = m
        .tiles
        .records()
        .iter()
        .map(|t| t.z as usize)
        .max()
        .unwrap_or(0);
    let expect_w = (30 * (xmy_max - xmy_min) + 60) as u32;
    let expect_h = (15 * (d_max - d_min) + 30 + 15 * z_max as i64) as u32;
    let dir = temp_dir("real-render");
    let out_path = dir.join("terrain.png");
    let out = Command::new(bin())
        .args([
            "render",
            map_path.to_str().unwrap(),
            "-o",
            out_path.to_str().unwrap(),
        ])
        .env("RA2_ASSETS_DIR", &assets)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let png = std::fs::read(&out_path).expect("PNG 落盘");
    assert!(png.len() > 24, "PNG 非空");
    assert_eq!(png[..8], *b"\x89PNG\r\n\x1a\n", "PNG 签名");
    assert_eq!(png[12..16], *b"IHDR");
    assert_eq!(
        png[16..20],
        expect_w.to_be_bytes(),
        "IHDR 宽 = 走位极值口径"
    );
    assert_eq!(
        png[20..24],
        expect_h.to_be_bytes(),
        "IHDR 高 = 走位极值 + 实扫 z_max"
    );
}
