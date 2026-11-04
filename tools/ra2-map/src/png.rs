// SPDX-License-Identifier: GPL-3.0-only

//! 最小合法 PNG 编码器(纯 std;spec §7 的工具侧要求)。
//!
//! 结构:8 字节签名 + IHDR(8bit/RGB 彩色类型 2/无隔行)+ 单 IDAT
//! (zlib store:0x78 0x01 + deflate 存储块 + Adler32)+ IEND。
//! CRC32 复用 `ra2_core::format::crc::crc32`(zlib 多项式,PNG 同款);
//! zlib/adler/存储块布局语义源自 PNG 规范(RFC 2085/ISO-15948)与 zlib
//! 格式定义,与 XCC 的 LCW/LZO 解包链无关(此处只产出新文件,不复用 EA 位)。

use ra2_core::format::crc::crc32;
use std::io::Write;

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

/// adler32(zlib 流校验尾)。
fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// 编码 24bpp RGB 图(width×height,行主序,长度恒 3·w·h)。
/// `rgb` 长度由调用方构造保证(程序不变量,assert 属编程错误而非输入错误)。
pub fn encode_png(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    assert_eq!(
        rgb.len(),
        3 * width as usize * height as usize,
        "RGB 长度不符"
    );
    let mut out = Vec::with_capacity(rgb.len() + 1024);
    out.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    // bitdepth 8 / 彩色类型 2(RGB)/ deflate / 过滤 0 / 无隔行
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    // 原始扫描流:每行前置 1 字节过滤类型 0(None)
    let stride = 3 * width as usize;
    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    for y in 0..height as usize {
        raw.push(0);
        raw.extend_from_slice(&rgb[y * stride..(y + 1) * stride]);
    }
    // zlib store:头 + ≤65535 字节的存储块 + adler32
    let mut idat = vec![0x78, 0x01];
    let mut pos = 0usize;
    while pos < raw.len() {
        let n = (raw.len() - pos).min(65535);
        let last = pos + n >= raw.len();
        idat.push(if last { 1 } else { 0 });
        idat.extend_from_slice(&(n as u16).to_le_bytes());
        idat.extend_from_slice(&(!(n as u16)).to_le_bytes());
        idat.extend_from_slice(&raw[pos..pos + n]);
        pos += n;
    }
    idat.extend_from_slice(&adler32(&raw).to_be_bytes());
    chunk(&mut out, b"IDAT", &idat);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// 编码并落盘(失败即 IO 错误,由 CLI 转退出码 1)。
pub fn write_png_file(
    path: &std::path::Path,
    width: u32,
    height: u32,
    rgb: &[u8],
) -> std::io::Result<()> {
    let bytes = encode_png(width, height, rgb);
    let mut f = std::fs::File::create(path)?;
    f.write_all(&bytes)?;
    f.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_and_ihdr_for_2x1() {
        let png = encode_png(2, 1, &[10, 20, 30, 40, 50, 60]);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(&png[8..12], &13u32.to_be_bytes());
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[16..29], &[0, 0, 0, 2, 0, 0, 0, 1, 8, 2, 0, 0, 0]);
        let crc_input: Vec<u8> = [b"IHDR".as_slice(), &png[16..29]].concat();
        assert_eq!(&png[29..33], &crc32(&crc_input).to_be_bytes());
    }

    #[test]
    fn zlib_store_block_layout_for_1x2() {
        // 偏差记录:brief 原文用 encode_png(2,1,…) 但期望 2 行 ×(1 过滤字节
        // + 3 RGB)= 8 字节——2 宽 1 高的 raw 实为 1 行 ×(1+6)= 7 字节,断言
        // 全部落空;改为 1 宽 2 高(raw 恰为期望的 8 字节)以保留 brief 全部
        // 字节级断言。raw = 2 行 ×(1 过滤字节 + 3 RGB)= 8 字节 → 单个 last 块
        let raw_expected = [0u8, 10, 20, 30, 0, 40, 50, 60];
        let png = encode_png(1, 2, &[10, 20, 30, 40, 50, 60]);
        // IDAT:签名 8 + IHDR 块 25 = 33;len = 2(头)+5(块头)+8+4(adler)= 19
        assert_eq!(&png[33..37], &19u32.to_be_bytes());
        assert_eq!(&png[37..41], b"IDAT");
        let idat = &png[41..41 + 19];
        assert_eq!(&idat[..2], &[0x78, 0x01]);
        // 偏差记录(续):brief 的 NLEN 期望 [0xF8, 0xFF](= !7)与它自己的
        // LEN [0x08, 0x00](= 8)矛盾——zlib 存储块 NLEN 恒为 LEN 按位取反,
        // LEN=8 → NLEN=0xFFF7,按实现修正。
        assert_eq!(&idat[2..7], &[0x01, 0x08, 0x00, 0xF7, 0xFF]); // last + LEN/NLEN
        assert_eq!(&idat[7..15], &raw_expected);
        assert_eq!(&idat[15..19], &adler32(&raw_expected).to_be_bytes());
    }

    #[test]
    fn iend_tail_and_adler_known_vector() {
        // adler32 公开校验向量("abc")
        assert_eq!(adler32(b"abc"), 0x024D_0127);
        let png = encode_png(1, 1, &[1, 2, 3]);
        let n = png.len();
        assert_eq!(&png[n - 12..n - 8], &0u32.to_be_bytes());
        assert_eq!(&png[n - 8..n - 4], b"IEND");
    }
}
