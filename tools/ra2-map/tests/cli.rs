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

// 偏差记录(承 lcw_fill_stream,T7 起并入本助手):块载荷必须以 0x80
// 终止——解码器读完 0xFE 长填充后继续读下一 opcode,缺终止符会
// UnexpectedEof(已知 bug 第 4 次出现的教训;size_in=5 同此)。
/// 262144 字节层的 LCW 块流(支持逐格改写;core render/scene.rs tests 同款)。
fn lcw_layer_stream(buf: &[u8]) -> Vec<u8> {
    assert_eq!(buf.len(), 262144);
    let mut blob = Vec::new();
    for chunk in buf.chunks(8192) {
        let mut p = Vec::new();
        let mut rest = chunk;
        while !rest.is_empty() {
            let b = rest[0];
            let run = rest.iter().take_while(|&&x| x == b).count();
            if run >= 3 {
                p.extend_from_slice(&[0xFE, run as u8, (run >> 8) as u8, b]);
                rest = &rest[run..];
            } else {
                p.push(0x80 | run as u8);
                p.extend_from_slice(&rest[..run]);
                rest = &rest[run..];
            }
        }
        p.push(0x80);
        blob.extend_from_slice(&(p.len() as u16).to_le_bytes());
        blob.extend_from_slice(&(chunk.len() as u16).to_le_bytes());
        blob.extend_from_slice(&p);
    }
    blob
}

/// 262144 全 0xFF 类型层(OVERLAY_NONE),逐格改写(行主序 (y<<9)|x)。
fn types_layer(cells: &[(u16, u16, u8)]) -> Vec<u8> {
    let mut v = vec![0xFFu8; 262144];
    for &(x, y, t) in cells {
        v[(y as usize) << 9 | x as usize] = t;
    }
    v
}

/// 262144 全 0 帧层,逐格改写。
fn data_layer(cells: &[(u16, u16, u8)]) -> Vec<u8> {
    let mut v = vec![0u8; 262144];
    for &(x, y, d) in cells {
        v[(y as usize) << 9 | x as usize] = d;
    }
    v
}

/// 与 core 测试同构的 mini 地图(SNOW 4×4,2 瓦片,1 结构 1 地形,预览 2×1);
/// 与 core 版的差异:① 记录 #1 移至 (4,1)(原 (1,1) 在走位域外,见下方
/// 偏差记录);② 无 [Actions] 段 → logic_sections=1(仅 Waypoints)。
/// `ovl` = overlay 双包逐格改写 ((x, y, 类型, 帧));空 = 全 0xFF/全 0
/// 原始层(编码字节与旧 lcw_fill_stream 等价,既有断言不变)。
fn mini_map_with(ovl: &[(u16, u16, u8, u8)]) -> Vec<u8> {
    let mut tiles = Vec::new();
    // 偏差记录(M4b T7):首记录原为 (1,1)——d=2 < walk_extremes(4,4) 的
    // d_min=5,在走位域外,首次被场景渲染遍历时按 M4a N1 计 parse_fail;
    // 移到域内 (4,1) 让两瓦片记录都可落绘(info/extract 断言不含坐标,不受
    // 影响)。
    tiles.extend_from_slice(&rec(4, 1, 5, 0, 0));
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
    push_pack_section(
        &mut ini,
        "OverlayPack",
        &lcw_layer_stream(&types_layer(
            &ovl.iter()
                .map(|&(x, y, t, _)| (x, y, t))
                .collect::<Vec<_>>(),
        )),
    );
    push_pack_section(
        &mut ini,
        "OverlayDataPack",
        &lcw_layer_stream(&data_layer(
            &ovl.iter()
                .map(|&(x, y, _, d)| (x, y, d))
                .collect::<Vec<_>>(),
        )),
    );
    push_pack_section(&mut ini, "IsoMapPack5", &lzo_literal_stream(&tiles));
    ini.push_str("[Structures]\n0=Neutral,CABHUT,256,2,3,0,None,0,0,1,0,0,None,None,None,0,0\n");
    ini.push_str("[Terrain]\n2003=TIBTRE01\n");
    ini.push_str("[Waypoints]\n0=2003\n");
    ini.push_str("[Digest]\n1=qSt1w6I0r+9EWkfWl7XFZMt0Vo4=\n");
    ini.into_bytes()
}

fn mini_map() -> Vec<u8> {
    mini_map_with(&[])
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

// ===== M4b T7:render 场景化(合成夹具,不 ignore)=====

/// 最小合法 MIX(flags=0,按名条目;core render/scene.rs tests 同款)。
fn make_mix(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    let body_len: usize = entries.iter().map(|(_, c)| c.len()).sum();
    v.extend_from_slice(&(body_len as u32).to_le_bytes());
    let mut off = 0u32;
    for (name, c) in entries {
        v.extend_from_slice(&ra2_core::format::crc::mix_name_id(name).to_le_bytes());
        v.extend_from_slice(&off.to_le_bytes());
        v.extend_from_slice(&(c.len() as u32).to_le_bytes());
        off += c.len() as u32;
    }
    for (_, c) in entries {
        v.extend_from_slice(c);
    }
    v
}

/// 临时资产目录(open() 必需 ra2.mix + ra2md.mix;scene.rs tiny_assets 同款,
/// 目录名带 tag+纳秒:并行线程各自独占)。返回目录路径供 --assets 传入。
fn mini_assets(tag: &str, files: &[(&str, Vec<u8>)]) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "ra2-map-cli-assets-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let refs: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (*n, b.as_slice())).collect();
    std::fs::write(d.join("ra2.mix"), make_mix(&[])).unwrap();
    std::fs::write(d.join("ra2md.mix"), make_mix(&refs)).unwrap();
    d
}

/// N 槽 60×30 TMP(槽 x 独立纯色;52B 槽头全零 = bitfield 0 无 z 无 extra;
/// 偏移 = 文件基相对,首槽恒 16+4·n,theater_tmp.rs 布局)。
fn tmp_multi(fills: &[u8]) -> Vec<u8> {
    let n = fills.len();
    let mut v = Vec::new();
    v.extend_from_slice(&(n as u32).to_le_bytes()); // cbx
    v.extend_from_slice(&1u32.to_le_bytes()); // cby
    v.extend_from_slice(&60u32.to_le_bytes()); // cx
    v.extend_from_slice(&30u32.to_le_bytes()); // cy
    let mut off = (16 + 4 * n) as u32;
    for _ in fills {
        v.extend_from_slice(&off.to_le_bytes());
        off += 52 + 900;
    }
    for f in fills {
        v.extend_from_slice(&[0u8; 52]); // 槽头
        v.extend_from_slice(&vec![*f; 900]); // RAW 菱形
    }
    v
}

/// 768B 剧场调色板(6 位原值 = i&63;0 号 = 背景)。
fn palette768() -> Vec<u8> {
    let mut v = Vec::with_capacity(768);
    for i in 0..256u16 {
        let c = (i & 63) as u8;
        v.extend_from_slice(&[c, c, c]);
    }
    v
}

/// N 帧 W×H RAW SHP(objects.rs tests 同款):帧 i 整帧 = fills[i]。
/// 均匀填充严格 RLE 必败 → 恒走 RAW 兜底(shp_ts.rs 判别式)。
fn shp_frames(cx: u16, cy: u16, fills: &[u8]) -> Vec<u8> {
    assert!(!fills.is_empty());
    let span = cx as usize * cy as usize;
    let mut v = Vec::new();
    v.extend_from_slice(&0u16.to_le_bytes()); // zero
    v.extend_from_slice(&cx.to_le_bytes()); // cx
    v.extend_from_slice(&cy.to_le_bytes()); // cy
    v.extend_from_slice(&(fills.len() as u16).to_le_bytes()); // count
    let table_end = 8 + 24 * fills.len();
    for (i, _) in fills.iter().enumerate() {
        v.extend_from_slice(&0i16.to_le_bytes()); // x
        v.extend_from_slice(&0i16.to_le_bytes()); // y
        v.extend_from_slice(&cx.to_le_bytes());
        v.extend_from_slice(&cy.to_le_bytes());
        v.extend_from_slice(&3u32.to_le_bytes()); // flags
        v.extend_from_slice(&[0u8; 4]); // rgba
        v.extend_from_slice(&0u32.to_le_bytes()); // reserved
        v.extend_from_slice(&((table_end + span * i) as u32).to_le_bytes());
    }
    for f in fills {
        v.extend_from_slice(&vec![*f; span]);
    }
    v
}

/// 单帧 W×H RAW SHP。
fn shp1(cx: u16, cy: u16, fill: u8) -> Vec<u8> {
    shp_frames(cx, cy, &[fill])
}

/// T7 场景夹具资产:SNOW 剧场最小闭环(snowmd.ini 单集合覆盖瓦片 5/6 +
/// isosno.pal + 两 TMP + 三 SHP);`rules=true` 再补 rulesmd.ini
/// ([OverlayTypes] 0=ORE01)与 artmd.ini(CABHUT→CABHUTIMG)——缺席即
/// CLI 退化路径(规则缺=提示,art 缺=静默)。
fn scene_assets(tag: &str, rules: bool) -> std::path::PathBuf {
    let mut files: Vec<(&str, Vec<u8>)> = vec![
        (
            "snowmd.ini",
            b"[TileSet0000]\nFileName=tst\nTilesInSet=8\n".to_vec(),
        ),
        ("isosno.pal", palette768()),
        ("tst06.sno", tmp_multi(&[9])),
        ("tst07.sno", tmp_multi(&[30, 31, 32])),
        ("ore01.shp", shp1(60, 30, 7)),
        ("cabhutimg.shp", shp1(60, 30, 40)),
        ("tibtre01.shp", shp1(1, 1, 60)),
    ];
    if rules {
        files.push((
            "rulesmd.ini",
            b"[OverlayTypes]\n0=ORE01\n[ORE01]\nTiberium=yes\n".to_vec(),
        ));
        files.push(("artmd.ini", b"[CABHUT]\nImage=CABHUTIMG\n".to_vec()));
    }
    mini_assets(tag, &files)
}

/// 场景渲染成功路径(brief §Step 1):mini_map 夹具 + 完整场景资产
/// (rulesmd/artmd 在档)→ exit 0、PNG 落盘、签名与 IHDR 尺寸 = M4a 公式
/// (走位极值 + records 实扫 z_max);stderr 三段统计:overlay 落绘 1
/// (rules 装载生效)、物件落绘 2(artmd Image 重定向 CABHUTIMG 生效 +
/// TIBTRE01 缺省名)、地形全落绘。偏差记录:brief 原文允许空 rules/art
/// (全退化)——改走完整装载路径,规则/素材在档与缺席(同图对照
/// render_missing_rules_degrades)两端才有测试覆盖。
#[test]
fn render_scene_outputs_png_and_stats() {
    use ra2_core::format::map::MapFile;
    use ra2_core::render::terrain::walk_extremes;
    let map_bytes = mini_map_with(&[(3, 2, 0, 0)]); // 格 (3,2) ORE01 帧 0
    let dir = temp_dir("render-scene");
    let p = dir.join("t.map");
    std::fs::write(&p, &map_bytes).unwrap();
    let assets_dir = scene_assets("render-scene", true);
    let out_path = dir.join("scene.png");
    let out = Command::new(bin())
        .args([
            "render",
            p.to_str().unwrap(),
            "-o",
            out_path.to_str().unwrap(),
            "--assets",
            assets_dir.to_str().unwrap(),
        ])
        .env_remove("RA2_ASSETS_DIR")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("地形 2 = 落绘 2"), "地形段: {stderr}");
    assert!(
        stderr.contains("overlay 1 = 落绘 1 + 越表 0"),
        "overlay 段(rules 装载): {stderr}"
    );
    assert!(
        stderr.contains("物件 2 = 落绘 2"),
        "物件段(artmd 重定向): {stderr}"
    );
    assert!(
        stderr.contains("钳帧") && stderr.contains("裁剪") && stderr.contains("耗时"),
        "信息位段: {stderr}"
    );
    // IHDR 尺寸 = M4a 公式(独立重算;scene 不改画布)
    let m = MapFile::parse(&map_bytes).unwrap();
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
    let png = std::fs::read(&out_path).expect("PNG 落盘");
    assert_eq!(png[..8], *b"\x89PNG\r\n\x1a\n", "PNG 签名");
    assert_eq!(png[12..16], *b"IHDR");
    assert_eq!(png[16..20], expect_w.to_be_bytes(), "IHDR 宽");
    assert_eq!(png[20..24], expect_h.to_be_bytes(), "IHDR 高");
    assert!(
        distinct_colors(&decode_store_idat(&png), expect_w) >= 2,
        "像素多值(非占位非单色)"
    );
}

/// rulesmd.ini 缺席(ra2md.mix 无该名)→ 退化非失败(brief §Step 1):
/// exit 0、stderr 提示退化、同一非空 overlay 格计 no_type(对照成功测试
/// 的落绘 1);artmd/art.ini 缺席为静默缺省(stderr 无 artmd 字样),
/// CABHUT 缺省名无档计缺素材、TIBTRE01 照落绘。
#[test]
fn render_missing_rules_degrades() {
    let map_bytes = mini_map_with(&[(3, 2, 0, 0)]);
    let dir = temp_dir("render-norules");
    let p = dir.join("t.map");
    std::fs::write(&p, &map_bytes).unwrap();
    let assets_dir = scene_assets("render-norules", false);
    let out_path = dir.join("scene.png");
    let out = Command::new(bin())
        .args([
            "render",
            p.to_str().unwrap(),
            "-o",
            out_path.to_str().unwrap(),
            "--assets",
            assets_dir.to_str().unwrap(),
        ])
        .env_remove("RA2_ASSETS_DIR")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("资产缺 rulesmd.ini"), "退化提示: {stderr}");
    assert!(
        stderr.contains("overlay 1 = 落绘 0 + 越表 1"),
        "非空格全 no_type: {stderr}"
    );
    assert!(
        stderr.contains("物件 2 = 落绘 1 + voxel跳过 0 + 缺素材 1"),
        "物件段(缺省名): {stderr}"
    );
    assert!(!stderr.contains("artmd"), "art 缺席不提示: {stderr}");
    assert!(out_path.exists(), "退化路径仍产出 PNG");
}
