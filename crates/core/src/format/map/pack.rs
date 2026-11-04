// SPDX-License-Identifier: GPL-3.0-only

//! 地图包段容器解码(四段共用:`[IsoMapPack5]` / `[OverlayPack]` /
//! `[OverlayDataPack]` / `[PreviewPack]`)。
//!
//! 布局(研究 2026-11-04-map-pack-and-binary-sections.md §1-2、§8;XCC
//! `shp_decode.cpp:1127-1155` decode5 语义 + 540 图语料验证):
//! 1. 编号 key(`1=`、`2=`…)按**数值序**拼接 → 标准 RFC 4648 base64
//!    (`=` 补位跳过);
//! 2. 解码字节 = 背靠背块 `{u16 size_in; u16 size_out; payload[size_in]}`;
//! 3. codec 按段名由调用方分派:IsoMapPack5/PreviewPack = LZO1X,overlay
//!    双段 = LCW("IsoMapPack5" 的 5 即 codec id);
//! 4. 预检(研究 §8,内存尖峰加固——所有大分配之前完成):size_in==0/越界
//!    拒;size_out==0/>8192 拒(仅 PreviewPack 例外:停在首个坏块保留干净
//!    前缀);聚合 Σsize_out 按段核对(overlay 恒 262144;瓦片流 ≤ 上界且
//!    %11 ∈ {0,4};预览不以 Σ 预拒但逐块累加超过 max_out 即停)。

use crate::format::lcw::lcw_decode_consumed;
use crate::format::lzo1x::lzo1x_decode;
use crate::format::FormatError;

/// 包段 codec。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackCodec {
    /// `[IsoMapPack5]` / `[PreviewPack]`(miniLZO)。
    Lzo1x,
    /// `[OverlayPack]` / `[OverlayDataPack]`(Format 80)。
    Lcw,
}

/// 解码前的输出上界形态(研究 §8-3)。
#[derive(Debug, Clone, Copy)]
pub enum PackBound {
    /// 聚合输出必须恰好等于该值(overlay 双段恒 262144)。
    Exact(usize),
    /// 瓦片流:Σ ≤ max_out 且 Σ % 11 ∈ {0, 4}(4 = EA/FinalAlert 的零尾)。
    Tiles { max_out: usize },
    /// 预览:容忍截断/异常尾块(lenient),累加超过 max_out 即停。
    Preview { max_out: usize },
}

/// 解码后的包段(严格段 data 长度 == Σ size_out)。
pub struct PackSection {
    pub data: Vec<u8>,
    pub chunks: usize,
}

fn b64_val(b: u8) -> Option<u8> {
    match b {
        b'A'..=b'Z' => Some(b - b'A'),
        b'a'..=b'z' => Some(b - b'a' + 26),
        b'0'..=b'9' => Some(b - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// 标准 RFC 4648 base64 解码(跳过 `=`;其余字符必须在标准字母表内)。
pub fn base64_decode(text: &str) -> Result<Vec<u8>, FormatError> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3 + 3);
    let mut acc = 0u32;
    let mut nbits = 0u32;
    for &b in text.as_bytes() {
        if b == b'=' {
            continue;
        }
        let v = b64_val(b).ok_or_else(|| {
            FormatError::Invalid(format!("base64 字符 0x{b:02X} 不在标准字母表内"))
        })?;
        acc = (acc << 6) | v as u32;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((acc >> nbits) as u8);
        }
    }
    Ok(out)
}

/// 预览段专用:在首个非法字符处截断(保留前缀),并丢弃尾部不完整的
/// 4 字符组(普查 §4.5:7 张 DDLY 图尾组被截断——"drop the partial tail group")。
pub fn base64_decode_lenient(text: &str) -> Vec<u8> {
    let mut end = text.len();
    for (i, &b) in text.as_bytes().iter().enumerate() {
        if b != b'=' && b64_val(b).is_none() {
            end = i;
            break;
        }
    }
    let kept: Vec<u8> = text.as_bytes()[..end]
        .iter()
        .copied()
        .filter(|&b| b != b'=')
        .collect();
    let keep = kept.len() - kept.len() % 4;
    let mut out = Vec::with_capacity(keep / 4 * 3 + 3);
    let mut acc = 0u32;
    let mut nbits = 0u32;
    for v in kept[..keep].iter().copied().filter_map(b64_val) {
        acc = (acc << 6) | v as u32;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((acc >> nbits) as u8);
        }
    }
    out
}

/// 解码一个包段。`entries` 为该段的全部键值对(顺序任意,按 key 数值排序)。
pub fn decode_pack_section(
    entries: &[(String, String)],
    codec: PackCodec,
    bound: PackBound,
) -> Result<PackSection, FormatError> {
    // 1) 编号 key 数值序拼接(普查 §4.5:键域 1..=9568;排序兼容敌意乱序)
    let mut keyed: Vec<(usize, &str)> = Vec::with_capacity(entries.len());
    for (k, v) in entries {
        let n: usize = k
            .parse()
            .map_err(|_| FormatError::Invalid(format!("包段键不是十进制数: {k}")))?;
        keyed.push((n, v.as_str()));
    }
    keyed.sort_by_key(|(n, _)| *n);
    let mut text = String::with_capacity(keyed.iter().map(|(_, v)| v.len()).sum());
    for (_, v) in &keyed {
        text.push_str(v);
    }
    let lenient = matches!(bound, PackBound::Preview { .. });
    let blob = if lenient {
        base64_decode_lenient(&text)
    } else {
        base64_decode(&text)?
    };

    // 2) 块链预检(只读块头,不解码):Σ size_out 与逐块合法性,任何分配之前
    let mut sum_out: u64 = 0;
    let mut sp = 0usize;
    while sp < blob.len() {
        if blob.len() - sp < 4 {
            return Err(FormatError::UnexpectedEof {
                needed: 4,
                remaining: blob.len() - sp,
            });
        }
        let size_in = u16::from_le_bytes([blob[sp], blob[sp + 1]]) as usize;
        let size_out = u16::from_le_bytes([blob[sp + 2], blob[sp + 3]]) as usize;
        if size_in == 0 {
            return Err(FormatError::Invalid(format!("块偏移 {sp} 的 size_in == 0")));
        }
        if size_in > blob.len() - sp - 4 {
            return Err(FormatError::UnexpectedEof {
                needed: size_in,
                remaining: blob.len() - sp - 4,
            });
        }
        if size_out == 0 || size_out > 8192 {
            if lenient {
                break; // 预览:异常末块(城郊之战 (8, 56994))——保留干净前缀
            }
            return Err(FormatError::Invalid(format!(
                "块偏移 {sp} 的 size_out = {size_out},合法 1..=8192"
            )));
        }
        sum_out += size_out as u64;
        sp += 4 + size_in;
    }
    let alloc = match (bound, sum_out) {
        (PackBound::Exact(n), s) => {
            if s != n as u64 {
                return Err(FormatError::Invalid(format!("聚合输出 {s} != 期望 {n}")));
            }
            n
        }
        (PackBound::Tiles { max_out }, s) => {
            if s > max_out as u64 {
                return Err(FormatError::Invalid(format!(
                    "聚合输出 {s} 超过瓦片流上界 {max_out}"
                )));
            }
            if s % 11 != 0 && s % 11 != 4 {
                return Err(FormatError::Invalid(format!(
                    "聚合输出 {s} 对 11 取模 = {},合法余数 0/4",
                    s % 11
                )));
            }
            s as usize
        }
        (PackBound::Preview { max_out }, s) => max_out.min(s as usize),
    };
    let mut out = Vec::with_capacity(alloc);

    // 3) 逐块解码(与预检同样的停止规则)
    let mut sp = 0usize;
    let mut chunks = 0usize;
    while sp < blob.len() {
        let size_in = u16::from_le_bytes([blob[sp], blob[sp + 1]]) as usize;
        let size_out = u16::from_le_bytes([blob[sp + 2], blob[sp + 3]]) as usize;
        if size_in == 0 || size_in > blob.len() - sp - 4 {
            break; // 防御性
        }
        if size_out == 0 || size_out > 8192 {
            if lenient {
                break;
            }
            return Err(FormatError::Invalid(format!(
                "块偏移 {sp} 的 size_out = {size_out},合法 1..=8192"
            )));
        }
        if lenient {
            if let PackBound::Preview { max_out } = bound {
                if out.len() + size_out > max_out {
                    break; // 声明 Σ 超上界的敌意预览:累加越界即停
                }
            }
        }
        let payload = &blob[sp + 4..sp + 4 + size_in];
        let before = out.len();
        let check = match codec {
            PackCodec::Lzo1x => lzo1x_decode(payload, size_out, &mut out).and_then(|produced| {
                if produced != size_out {
                    Err(FormatError::Invalid(format!(
                        "LZO 产出 {produced} != 块头 size_out {size_out}"
                    )))
                } else {
                    Ok(())
                }
            }),
            PackCodec::Lcw => lcw_decode_consumed(payload, &mut out).and_then(|consumed| {
                if consumed != size_in {
                    Err(FormatError::Invalid(format!(
                        "LCW 消耗 {consumed} != 块头 size_in {size_in}"
                    )))
                } else if out.len() - before != size_out {
                    Err(FormatError::Invalid(format!(
                        "LCW 产出 {} != 块头 size_out {size_out}",
                        out.len() - before
                    )))
                } else {
                    Ok(())
                }
            }),
        };
        match check {
            Ok(()) => {
                chunks += 1;
                sp += 4 + size_in;
            }
            Err(e) => {
                if lenient {
                    out.truncate(before);
                    break; // 保留干净前缀(研究 §5 契约)
                }
                return Err(e);
            }
        }
    }
    Ok(PackSection { data: out, chunks })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用最小 LZO 块载荷:全字面量(首 token 17+n,1 ≤ n ≤ 238)+ EOF 标记。
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

    /// 测试用 LZO 块流:数据按 ≤238 字节切全字面量块。
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

    /// 测试用 LCW 块流:0xFE 长填充(n ≤ 8192/块)+ 0x80 终止符(size_in = 5)。
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
                s.push('=');
                s.push('=');
            }
            2 => {
                let n = ((bytes[bytes.len() - 2] as u32) << 16)
                    | ((bytes[bytes.len() - 1] as u32) << 8);
                s.push(A[(n >> 18) as usize & 63] as char);
                s.push(A[(n >> 12) as usize & 63] as char);
                s.push(A[(n >> 6) as usize & 63] as char);
                s.push('=');
            }
            _ => {}
        }
        s
    }

    fn entries_of(blob: &[u8]) -> Vec<(String, String)> {
        // 按测试需要切成多行 base64(每行 ≤ 70 字符,模拟真实图的换行键)
        let text = b64(blob);
        let mut v = Vec::new();
        let mut i = 0usize;
        let mut k = 1usize;
        while i < text.len() {
            let n = (text.len() - i).min(70);
            v.push((k.to_string(), text[i..i + n].to_string()));
            i += n;
            k += 1;
        }
        v
    }

    #[test]
    fn base64_decodes_standard_alphabet_and_skips_padding() {
        assert_eq!(base64_decode("QUI=").unwrap(), b"AB");
        assert_eq!(base64_decode("QUJD").unwrap(), b"ABC");
        assert_eq!(base64_decode("QUJDQUI=").unwrap(), b"ABCAB");
    }

    #[test]
    fn base64_strict_rejects_bad_char() {
        assert!(matches!(
            base64_decode("QU?D"),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn base64_lenient_cuts_at_bad_char_and_partial_group() {
        // 'p' 前 6 个合法字符 → 保留 4("QUJD")→ "ABC";尾部残组被丢弃
        assert_eq!(base64_decode_lenient("QUJDQU?pQ"), b"ABC");
    }

    #[test]
    fn numbered_keys_concat_in_numeric_order() {
        // 键乱序传入:1→[0x11], 2→[0x22], 10→[0x33];数值序拼接后输出
        // [0x11,0x22,0x33](字典序会得到 1,10,2 → [0x11,0x33,0x22])。
        // 3 字节 %11 = 3 不合法 → 用 Preview 上界绕开模数判定。
        let mk = |d: u8| {
            let payload = lzo_literal_payload(&[d]);
            let mut blob = Vec::new();
            blob.extend_from_slice(&(payload.len() as u16).to_le_bytes());
            blob.extend_from_slice(&1u16.to_le_bytes());
            blob.extend_from_slice(&payload);
            b64(&blob)
        };
        let entries = vec![
            ("10".to_string(), mk(0x33)),
            ("2".to_string(), mk(0x22)),
            ("1".to_string(), mk(0x11)),
        ];
        let sec = decode_pack_section(
            &entries,
            PackCodec::Lzo1x,
            PackBound::Preview { max_out: 3 },
        )
        .unwrap();
        assert_eq!(sec.data, vec![0x11, 0x22, 0x33]);
        assert_eq!(sec.chunks, 3);
    }

    #[test]
    fn rejects_non_numeric_key() {
        let entries = vec![("x".to_string(), "QUJD".to_string())];
        assert!(matches!(
            decode_pack_section(
                &entries,
                PackCodec::Lzo1x,
                PackBound::Preview { max_out: 3 }
            ),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_size_in_zero() {
        // 补 2 字节使 blob 长度为 3 的倍数:Preview 走 lenient base64 会丢弃
        // 尾部残组,4 字节 blob(补位后 %3=1)经 lenient 解码会缩成 3 字节,
        // 触发 UnexpectedEof 而非本测的 size_in==0 → Invalid
        let blob = [0u8, 0, 8, 0, 0x11, 0x00]; // size_in=0
        let entries = entries_of(&blob);
        assert!(matches!(
            decode_pack_section(
                &entries,
                PackCodec::Lzo1x,
                PackBound::Preview { max_out: 8 }
            ),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_size_in_beyond_stream() {
        let blob = [100, 0, 8, 0, 1, 2]; // size_in=100 但只剩 2 字节载荷
        let entries = entries_of(&blob);
        assert!(matches!(
            decode_pack_section(
                &entries,
                PackCodec::Lzo1x,
                PackBound::Preview { max_out: 8 }
            ),
            Err(FormatError::UnexpectedEof { .. })
        ));
    }

    #[test]
    fn strict_rejects_size_out_zero() {
        // size_out=0 在 Σ 累计之前即被拒(bound 参数不参与该判定);载荷补齐
        // size_in 声明的 9 字节,确保先越过 size_in 越界核对
        let blob = [
            9, 0, 0, 0, 0x83, b'A', b'B', b'C', 0x80, 0x00, 0x00, 0x00, 0x00,
        ];
        let entries = entries_of(&blob);
        assert!(matches!(
            decode_pack_section(&entries, PackCodec::Lcw, PackBound::Exact(0)),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn strict_rejects_size_out_over_8192() {
        // u16 LE 的 8193 = 0x2001 = [0x01, 0x20]
        let blob = [
            9, 0, 0x01, 0x20, 0x83, b'A', b'B', b'C', 0x80, 0x00, 0x00, 0x00, 0x00,
        ];
        let entries = entries_of(&blob);
        assert!(matches!(
            decode_pack_section(&entries, PackCodec::Lcw, PackBound::Exact(8193)),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn exact_bound_rejects_sum_mismatch() {
        // 单个 8191 填充块 ≠ 262144 → Invalid(overlay 的 Σ 恒等判定)
        let blob = lcw_fill_stream(0xFF, 8191);
        let entries = entries_of(&blob);
        assert!(matches!(
            decode_pack_section(&entries, PackCodec::Lcw, PackBound::Exact(262144)),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn tiles_bound_rejects_bad_modulus() {
        // Σ = 8192,%11 = 8 ∉ {0,4} → Invalid
        let blob = lzo_literal_stream(&[0x5Au8; 8192]);
        let entries = entries_of(&blob);
        assert!(matches!(
            decode_pack_section(
                &entries,
                PackCodec::Lzo1x,
                PackBound::Tiles { max_out: 8192 }
            ),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn tiles_bound_rejects_over_max() {
        // Σ = 26(%11 = 4 合法)但超过 max_out = 15 → Invalid
        let mut data = Vec::new();
        data.extend_from_slice(&[
            0x57, 0x00, 0x01, 0x00, 0x3C, 0x01, 0x00, 0x00, 0x01, 0x02, 0x00,
        ]);
        data.extend_from_slice(&[
            0x56, 0x00, 0x02, 0x00, 0x3C, 0x01, 0x00, 0x00, 0x02, 0x02, 0x00,
        ]);
        data.extend_from_slice(&[0, 0, 0, 0]);
        let blob = lzo_literal_stream(&data);
        let entries = entries_of(&blob);
        assert!(matches!(
            decode_pack_section(&entries, PackCodec::Lzo1x, PackBound::Tiles { max_out: 15 }),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn lcw_research_chunk0_decodes_to_8192_ff() {
        // 规格依据 §4:c1a01md OverlayPack 块 0 原始字节(XCC decode5 语义);
        // 9 载荷字节 → 8192 个 FF
        let blob: Vec<u8> = [
            &[0x09u8, 0x00, 0x00, 0x20] as &[u8],
            &[0x81, 0xFF, 0xFE, 0xFE, 0x1F, 0xFF, 0x81, 0xFF, 0x80],
        ]
        .concat();
        let entries = entries_of(&blob);
        let sec = decode_pack_section(&entries, PackCodec::Lcw, PackBound::Exact(8192)).unwrap();
        assert_eq!(sec.chunks, 1);
        assert_eq!(sec.data.len(), 8192);
        assert!(sec.data.iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn tiles_accepts_short_stream_with_zero_tail() {
        // 2 条记录(规格依据 §5 的 rec0/rec1)+ 4 零尾:Σ = 26,%11 = 4,
        // ≤ max_out=26 → Ok(4 张 TS 代短图的上界语义)
        let mut data = Vec::new();
        data.extend_from_slice(&[
            0x57, 0x00, 0x01, 0x00, 0x3C, 0x01, 0x00, 0x00, 0x01, 0x02, 0x00,
        ]);
        data.extend_from_slice(&[
            0x56, 0x00, 0x02, 0x00, 0x3C, 0x01, 0x00, 0x00, 0x02, 0x02, 0x00,
        ]);
        data.extend_from_slice(&[0, 0, 0, 0]);
        let blob = lzo_literal_stream(&data);
        let entries = entries_of(&blob);
        let sec = decode_pack_section(&entries, PackCodec::Lzo1x, PackBound::Tiles { max_out: 26 })
            .unwrap();
        assert_eq!(sec.data.len(), 26);
        assert_eq!(sec.chunks, 1);
        assert_eq!(&sec.data[..11], &data[..11]);
    }

    #[test]
    fn preview_lenient_stops_at_anomalous_final_chunk() {
        // 城郊之战形态:干净块(12+10 字节)+ 异常末块 (8, 56994) → 保留前缀 22。
        // 末块载荷补 1 字节共 9 字节使 blob = 51(%3=0):lenient base64 会丢弃
        // 尾部残组,50 字节 blob 会把末块载荷截成 6 字节而触发 size_in 越界
        let mut blob = Vec::new();
        push_lzo_chunk(&mut blob, &[b'a'; 12]);
        push_lzo_chunk(&mut blob, &[b'b'; 10]);
        blob.extend_from_slice(&8u16.to_le_bytes());
        blob.extend_from_slice(&56994u16.to_le_bytes());
        blob.extend_from_slice(&[0x11, 0x00, 0x00, 0x11, 0x00, 0x00, 0x11, 0x00, 0x00]);
        let entries = entries_of(&blob);
        let sec = decode_pack_section(
            &entries,
            PackCodec::Lzo1x,
            PackBound::Preview { max_out: 22 },
        )
        .unwrap();
        assert_eq!(sec.data.len(), 22);
        assert_eq!(sec.chunks, 2);
        assert!(sec.data.starts_with(b"aaaaaaaaaaaa"));
    }

    #[test]
    fn preview_lenient_stops_when_accumulation_exceeds_max() {
        // 敌意预览:3 块各声明 8192(Σ 24576 > max 20000)→ 累加到 16384 后停
        // (LCW 流配 LCW codec;lenient 分支不核对 Σ,只钳累加)
        let blob = lcw_fill_stream(0x11, 24576);
        let entries = entries_of(&blob);
        let sec = decode_pack_section(
            &entries,
            PackCodec::Lcw,
            PackBound::Preview { max_out: 20000 },
        )
        .unwrap();
        assert_eq!(sec.data.len(), 16384);
    }

    #[test]
    fn strict_rejects_produced_mismatch() {
        // 块头声明 size_out=5,载荷只产 3 → Invalid(研究 §8-5 的 produced 核对;
        // 用严格 Exact(5) 绑定——lenient 路径会吞掉产出不符而保留前缀)
        let payload = [0x14u8, b'A', b'B', b'C', 0x11, 0x00, 0x00]; // t = 0x14-17 = 3
        let mut blob = Vec::new();
        blob.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        blob.extend_from_slice(&5u16.to_le_bytes());
        blob.extend_from_slice(&payload);
        let entries = entries_of(&blob);
        assert!(matches!(
            decode_pack_section(&entries, PackCodec::Lzo1x, PackBound::Exact(5)),
            Err(FormatError::Invalid(_))
        ));
    }
}
