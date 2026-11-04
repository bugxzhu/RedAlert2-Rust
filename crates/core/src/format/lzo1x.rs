// SPDX-License-Identifier: GPL-3.0-only

//! LZO1X 解压(miniLZO 兼容;`[IsoMapPack5]` / `[PreviewPack]` 的 codec)。
//!
//! 依据 Linux 内核 `lib/lzo/lzo1x_decompress_safe.c`(GPL-2.0,
//! Markus F.X.J. Oberhumer;CVE-2014-4608 加固后的 "safe" 变体)的规范语义
//! 用 Rust 重写。对应关系:NEED_IP / NEED_OP / TEST_LB 逐一映射为显式边界
//! 检查;输入过读/输出越界 → `UnexpectedEof`;回溯越界(lookbehind)、EOF
//! 标记长度 ≠ 3、EOF 后残留字节、255 计数溢出 → `Invalid`。
//! 流可以 2 字节版本头 `17,<ver>` 开头(内核按 `in_len >= 5 && 首字节 == 17`
//! 识别,带与不带都接受;<ver> 仅启用内核 2020 的零游程扩展)。EOF 由
//! "源==目标"的匹配(m_pos == op,要求 t == 3)判定;成功返回即输入恰好耗尽。

use super::FormatError;

/// M2 短匹配(state==4 路径)的基准偏移(内核 lzodefs.h M2_MAX_OFFSET)。
const M2_MAX_OFFSET: usize = 0x0800;
/// 内核零游程扩展的最小长度(lzodefs.h MIN_ZERO_RUN_LENGTH)。
const MIN_ZERO_RUN_LENGTH: usize = 4;
/// 内核 MAX_255_COUNT:255 游程计数溢出防护上限(usize::MAX / 255 - 2)。
const MAX_255_COUNT: usize = usize::MAX / 255 - 2;

fn eof(needed: usize, remaining: usize) -> FormatError {
    FormatError::UnexpectedEof { needed, remaining }
}

fn invalid(msg: &str) -> FormatError {
    FormatError::Invalid(msg.to_string())
}

/// 拷贝 `count` 个字面量。对应内核所有字面量拷贝点共用的
/// `NEED_IP(t + 3); NEED_OP(t); do *op++ = *ip++; while (--t)`——3 字节余量
/// 保证循环头至少还能读出一个操作码。
fn copy_literals(
    src: &[u8],
    ip: &mut usize,
    out: &mut Vec<u8>,
    op: &mut usize,
    out_cap: usize,
    count: usize,
) -> Result<(), FormatError> {
    if src.len() - *ip < count + 3 {
        return Err(eof(count + 3, src.len() - *ip));
    }
    if out_cap - *op < count {
        return Err(eof(count, out_cap - *op));
    }
    for _ in 0..count {
        out.push(src[*ip]);
        *ip += 1;
        *op += 1;
    }
    Ok(())
}

/// 255 游程扩展(内核三处同构的 `while (*ip == 0) { ip++; NEED_IP(1); }`):
/// 返回(扫过的零字节数, 终止字节)。消费不含终止字节。
fn read_255_run(src: &[u8], ip: &mut usize) -> Result<(usize, u8), FormatError> {
    let mut zeros = 0usize;
    while *ip < src.len() && src[*ip] == 0 {
        *ip += 1;
        zeros += 1;
    }
    if src.len() - *ip < 1 {
        return Err(eof(1, 0));
    }
    if zeros > MAX_255_COUNT {
        return Err(invalid("LZO 255 游程计数溢出"));
    }
    let term = src[*ip];
    *ip += 1;
    Ok((zeros, term))
}

/// 从 `m_pos` 逐字节正拷 `count` 字节(允许源/目的重叠,与内核字节路径语义
/// 一致)。`m_pos < op` 不变式由调用方的 checked_sub 保证。
fn copy_match(
    out: &mut Vec<u8>,
    base: usize,
    m_pos: usize,
    op: &mut usize,
    out_cap: usize,
    count: usize,
) -> Result<(), FormatError> {
    if out_cap - *op < count {
        return Err(eof(count, out_cap - *op));
    }
    for i in 0..count {
        out.push(out[base + m_pos + i]);
        *op += 1;
    }
    Ok(())
}

/// 解码一个完整 LZO1X 流,把不超过 `out_cap` 字节追加到 `out`(自 `out.len()`
/// 起计),返回产出字节数。成功 ⇒ 输入恰好耗尽;"产出 ≠ 块头 size_out"
/// 由调用方核对。
pub fn lzo1x_decode(src: &[u8], out_cap: usize, out: &mut Vec<u8>) -> Result<usize, FormatError> {
    let in_len = src.len();
    if in_len < 3 {
        return Err(eof(3, in_len));
    }
    let base = out.len();
    let mut ip = 0usize;
    let mut bitstream_version = 0u8;
    // 内核 :58-63 的版本头识别
    if in_len >= 5 && src[0] == 17 {
        bitstream_version = src[1];
        ip = 2;
    }
    let mut op = 0usize; // 相对 base 的输出位置
    let mut state = 0usize;

    // 首字节特例(内核 :65-72):t = 首 - 17,<4 转 match_next,否则整段字面量
    if src[ip] > 17 {
        let t = (src[ip] - 17) as usize;
        ip += 1;
        state = if t < 4 { t } else { 4 };
        copy_literals(src, &mut ip, out, &mut op, out_cap, t)?;
    }
    loop {
        if src.len() - ip < 1 {
            return Err(eof(1, 0));
        }
        let tok = src[ip];
        ip += 1;
        let mut t = tok as usize;
        let mut next;
        let mut m_pos;
        if t < 16 {
            if state == 0 {
                if t == 0 {
                    let (zeros, term) = read_255_run(src, &mut ip)?;
                    t += zeros.wrapping_mul(255) + 15 + term as usize;
                }
                t += 3;
                copy_literals(src, &mut ip, out, &mut op, out_cap, t)?;
                state = 4;
                continue;
            } else if state != 4 {
                // M1:2 字节匹配,长度恒 2,偏移 ≤ 1+63+255·4
                next = t & 3;
                if src.len() - ip < 1 {
                    return Err(eof(1, 0));
                }
                let back = 1 + (t >> 2) + ((src[ip] as usize) << 2);
                ip += 1;
                m_pos = op
                    .checked_sub(back)
                    .ok_or_else(|| invalid("LZO 回溯越界"))?;
                if out_cap - op < 2 {
                    return Err(eof(2, out_cap - op));
                }
                out.push(out[base + m_pos]);
                out.push(out[base + m_pos + 1]);
                op += 2;
                state = next;
                copy_literals(src, &mut ip, out, &mut op, out_cap, next)?;
                continue;
            } else {
                // M2 短匹配(state==4),偏移 > M2_MAX_OFFSET
                next = t & 3;
                if src.len() - ip < 1 {
                    return Err(eof(1, 0));
                }
                let back = 1 + M2_MAX_OFFSET + (t >> 2) + ((src[ip] as usize) << 2);
                ip += 1;
                m_pos = op
                    .checked_sub(back)
                    .ok_or_else(|| invalid("LZO 回溯越界"))?;
                t = 3;
            }
        } else if t >= 64 {
            // M2:2 字节常规短匹配,长度 (t>>5)+1,偏移 1..=2048(= M2_MAX_OFFSET,
            // 1 + (t>>2)&7 最大 7 + 扩展字节最大 255<<3 = 2040)
            next = t & 3;
            if src.len() - ip < 1 {
                return Err(eof(1, 0));
            }
            let back = 1 + ((t >> 2) & 7) + ((src[ip] as usize) << 3);
            ip += 1;
            m_pos = op
                .checked_sub(back)
                .ok_or_else(|| invalid("LZO 回溯越界"))?;
            t = (t >> 5) + 1;
        } else if t >= 32 {
            // M3:2 字节 + u16 偏移,长度 (t&31)+2(=2 时 255 扩展)
            t = (t & 31) + 2;
            if t == 2 {
                let (zeros, term) = read_255_run(src, &mut ip)?;
                t += zeros.wrapping_mul(255) + 31 + term as usize;
                if src.len() - ip < 2 {
                    return Err(eof(2, src.len() - ip));
                }
            }
            m_pos = op.checked_sub(1).ok_or_else(|| invalid("LZO 回溯越界"))?;
            next = u16::from_le_bytes([src[ip], src[ip + 1]]) as usize;
            ip += 2;
            m_pos = m_pos
                .checked_sub(next >> 2)
                .ok_or_else(|| invalid("LZO 回溯越界"))?;
            next &= 3;
        } else {
            // M4(16..=31):总偏移 = ((tok&8)<<11) + (next>>2) + 0x4000
            if src.len() - ip < 2 {
                return Err(eof(2, src.len() - ip));
            }
            next = u16::from_le_bytes([src[ip], src[ip + 1]]) as usize;
            if (next & 0xFFFC) == 0xFFFC && (tok & 0xF8) == 0x18 && bitstream_version != 0 {
                // 内核 2020 零游程扩展(仅版本头流):4 字节 [tok][lo][hi][count]
                if src.len() - ip < 3 {
                    return Err(eof(3, src.len() - ip));
                }
                t = ((tok & 7) as usize) | ((src[ip + 2] as usize) << 3);
                t += MIN_ZERO_RUN_LENGTH;
                if out_cap - op < t {
                    return Err(eof(t, out_cap - op));
                }
                for _ in 0..t {
                    out.push(0);
                    op += 1;
                }
                next &= 3;
                ip += 3;
                state = next;
                copy_literals(src, &mut ip, out, &mut op, out_cap, next)?;
                continue;
            }
            m_pos = op
                .checked_sub(((tok & 8) as usize) << 11)
                .ok_or_else(|| invalid("LZO 回溯越界"))?;
            t = ((tok & 7) as usize) + 2;
            if t == 2 {
                // 255 扩展:首 le16 与重读 le16 共享字节——扫描从 next 的首字节
                // 开始(kernel :187-203),扩展后 next 整体重读
                let (zeros, term) = read_255_run(src, &mut ip)?;
                t += zeros.wrapping_mul(255) + 7 + term as usize;
                if src.len() - ip < 2 {
                    return Err(eof(2, src.len() - ip));
                }
                next = u16::from_le_bytes([src[ip], src[ip + 1]]) as usize;
            }
            ip += 2;
            m_pos = m_pos
                .checked_sub(next >> 2)
                .ok_or_else(|| invalid("LZO 回溯越界"))?;
            next &= 3;
            if m_pos == op {
                // EOF 标记:源==目标的匹配(内核 eof_found)
                if t != 3 {
                    return Err(invalid(&format!("LZO EOF 标记匹配长度 {t} != 3")));
                }
                if ip != src.len() {
                    return Err(invalid(&format!("LZO EOF 后仍剩 {} 字节", src.len() - ip)));
                }
                return Ok(op);
            }
            m_pos = m_pos
                .checked_sub(0x4000)
                .ok_or_else(|| invalid("LZO 回溯越界"))?;
        }
        copy_match(out, base, m_pos, &mut op, out_cap, t)?;
        state = next;
        copy_literals(src, &mut ip, out, &mut op, out_cap, next)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 解码便捷封装(与正式 API 相同,只是替测试攒 out)。
    fn dec(src: &[u8], cap: usize) -> Result<Vec<u8>, FormatError> {
        let mut out = Vec::new();
        lzo1x_decode(src, cap, &mut out).map(|_| out)
    }

    fn eq_ok(v: &Result<Vec<u8>, FormatError>, want: &[u8]) -> bool {
        matches!(v, Ok(o) if o == want)
    }

    /// 最小 SHA-256(FIPS 180-4),仅测试用(Vector A 的产出哈希断言)。
    fn sha256(data: &[u8]) -> [u8; 32] {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut h: [u32; 8] = [
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
            0x5be0cd19,
        ];
        let bit_len = (data.len() as u64) * 8;
        let mut msg = data.to_vec();
        msg.push(0x80);
        while msg.len() % 64 != 56 {
            msg.push(0);
        }
        msg.extend_from_slice(&bit_len.to_be_bytes());
        for block in msg.as_chunks::<64>().0 {
            let mut w = [0u32; 64];
            for i in 0..16 {
                w[i] = u32::from_be_bytes([
                    block[i * 4],
                    block[i * 4 + 1],
                    block[i * 4 + 2],
                    block[i * 4 + 3],
                ]);
            }
            for i in 16..64 {
                let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
                let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
                w[i] = w[i - 16]
                    .wrapping_add(s0)
                    .wrapping_add(w[i - 7])
                    .wrapping_add(s1);
            }
            let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
                (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
            for i in 0..64 {
                let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
                let ch = (e & f) ^ ((!e) & g);
                let t1 = hh
                    .wrapping_add(s1)
                    .wrapping_add(ch)
                    .wrapping_add(K[i])
                    .wrapping_add(w[i]);
                let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
                let maj = (a & b) ^ (a & c) ^ (b & c);
                let t2 = s0.wrapping_add(maj);
                hh = g;
                g = f;
                f = e;
                e = d.wrapping_add(t1);
                d = c;
                c = b;
                b = a;
                a = t1.wrapping_add(t2);
            }
            h[0] = h[0].wrapping_add(a);
            h[1] = h[1].wrapping_add(b);
            h[2] = h[2].wrapping_add(c);
            h[3] = h[3].wrapping_add(d);
            h[4] = h[4].wrapping_add(e);
            h[5] = h[5].wrapping_add(f);
            h[6] = h[6].wrapping_add(g);
            h[7] = h[7].wrapping_add(hh);
        }
        let mut o = [0u8; 32];
        for i in 0..8 {
            o[i * 4..i * 4 + 4].copy_from_slice(&h[i].to_be_bytes());
        }
        o
    }

    fn sha16(data: &[u8]) -> String {
        let d = sha256(data);
        (0..8).map(|i| format!("{:02x}", d[i])).collect()
    }

    fn hex_bytes(s: &str) -> Vec<u8> {
        s.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
            .collect()
    }

    #[test]
    fn decodes_first_token_literal_run() {
        // 0x15 = 17+4:首字节特例的整段字面量路径(t=4,NEED_IP(4+3) 恰含 EOF 标记)
        let v = dec(&[0x15, b'A', b'B', b'C', b'D', 0x11, 0, 0], 100);
        assert!(eq_ok(&v, b"ABCD"));
    }

    #[test]
    fn decodes_in_loop_literal_run() {
        // token 12 < 16 且 state==0:t = 12+3 = 15 字面量
        let mut src = vec![0x0Cu8];
        src.extend_from_slice(b"0123456789abcde");
        src.extend_from_slice(&[0x11, 0, 0]);
        let v = dec(&src, 100);
        assert!(eq_ok(&v, b"0123456789abcde"));
    }

    #[test]
    fn decodes_255_extended_literal_run() {
        // token 0,无零扩展(zeros=0,term=0x1A=26):t = 15+26+3 = 44
        let mut src = vec![0x00u8, 0x1A];
        src.extend_from_slice(b"X".repeat(44).as_slice());
        src.extend_from_slice(&[0x11, 0, 0]);
        let v = dec(&src, 200);
        assert!(eq_ok(&v, &[b'X'; 44]));
    }

    #[test]
    fn decodes_255_extended_literal_run_with_zeros() {
        // token 0,1 个零字节(zeros=1,term=4):t = 255+15+4+3 = 277
        let mut src = vec![0x00u8, 0x00, 0x04];
        src.extend_from_slice(b"Y".repeat(277).as_slice());
        src.extend_from_slice(&[0x11, 0, 0]);
        let v = dec(&src, 400);
        assert!(eq_ok(&v, &[b'Y'; 277]));
    }

    #[test]
    fn decodes_m2_short_match() {
        // 0xC0:back = 1+((0xC0>>2)&7)+(0<<3) = 1,len = (0xC0>>5)+1 = 7 → ABCD + 7×'D'
        let v = dec(&[0x15, b'A', b'B', b'C', b'D', 0xC0, 0x00, 0x11, 0, 0], 100);
        assert!(eq_ok(&v, b"ABCDDDDDDDD"));
    }

    #[test]
    fn decodes_m2_with_next_literal_then_m1() {
        // 0xC1(next=1)+字面量 'E' → state=1;token 0x04 走 M1:back = 1+(4>>2)+0 = 2,
        // 拷 2 字节;锁定 match_next 的尾部字面量与 M1 的 state 记账
        let v = dec(
            &[
                0x15, b'A', b'B', b'C', b'D', 0xC1, 0x00, b'E', 0x04, 0x00, 0x11, 0, 0,
            ],
            100,
        );
        assert!(eq_ok(&v, b"ABCDDDDDDDDEDE"));
    }

    #[test]
    fn decodes_m3_match() {
        // 0x21:len = (0x21&31)+2 = 3,next=4 → 偏移 = 1+4>>2 = 2,重叠拷出 CDC
        let v = dec(
            &[0x15, b'A', b'B', b'C', b'D', 0x21, 0x04, 0x00, 0x11, 0, 0],
            100,
        );
        assert!(eq_ok(&v, b"ABCDCDC"));
    }

    #[test]
    fn decodes_m3_255_extended() {
        // 0x20:len = 2 → 255 扩展(zeros=0,term=24):len = 2+31+24 = 57,next=4 → 偏移 2;
        // 重叠拷出 C/D 交替序列,末字节(下标 60)为 'C'
        let src = [
            0x15u8, b'A', b'B', b'C', b'D', 0x20, 0x18, 0x04, 0x00, 0x11, 0, 0,
        ];
        let v = dec(&src, 200);
        assert!(
            matches!(&v, Ok(o) if o.len() == 4 + 57 && o[..4] == *b"ABCD" && o[4] == b'C' && o[60] == b'C')
        );
    }

    #[test]
    fn decodes_m4_long_match_and_255_extension() {
        // M4 总偏移 = ((tok&8)<<11)+(next>>2)+0x4000 ≥ 16385:v8(tok=0x11,bit3=0,
        // next=4 → 偏移 16385)、v9(tok=0x18:bit3 置位,255 扩展 len 13,重读 next=4
        // → 偏移 32769;注意首 le16 读 (b1,b2)、扩展重读 (b2,b3),操作码 4 字节
        // [18 04 04 00])。先用 M3(len 33,偏移 2)增长过 0x8000。
        let mut src = vec![0x15u8, b'A', b'B', b'C', b'D'];
        let k = (32_900 - 4 + 32) / 33;
        for _ in 0..k {
            src.extend_from_slice(&[0x3F, 0x04, 0x00]);
        }
        let mut expected = b"ABCD".to_vec();
        for _ in 0..k {
            let n = expected.len();
            for i in 0..33 {
                expected.push(expected[n - 2 + i]);
            }
        }
        src.extend_from_slice(&[0x11, 0x04, 0x00]);
        let n = expected.len();
        for i in 0..3 {
            expected.push(expected[n - 0x4000 - 1 + i]);
        }
        src.extend_from_slice(&[0x18, 0x04, 0x04, 0x00]);
        let n = expected.len();
        for i in 0..13 {
            expected.push(expected[n - 0x8000 - 1 + i]);
        }
        src.extend_from_slice(&[0x11, 0x00, 0x00]);
        let mut out = Vec::new();
        let r = lzo1x_decode(&src, 100_000, &mut out);
        assert!(matches!(r, Ok(len) if len == expected.len()));
        assert_eq!(out, expected);
    }

    #[test]
    fn decodes_zero_run_with_version_header() {
        // 版本头 17,02;零游程操作码 0x18-0x1F 不能作首字节(>17 会走字面量特例),
        // 故先放一段字面量。next 两字节必须满足 next&0xFFFC==0xFFFC(FC FF)。
        // len = (0x18&7)|((0x05)<<3) + 4 = 44 个零
        let v = dec(
            &[
                0x11, 0x02, 0x15, b'A', b'B', b'C', b'D', 0x18, 0xFC, 0xFF, 0x05, 0x11, 0x00, 0x00,
            ],
            100,
        );
        let mut want = b"ABCD".to_vec();
        want.extend_from_slice(&[0u8; 44]);
        assert!(eq_ok(&v, &want));
    }

    #[test]
    fn decodes_m2_short_large_offset() {
        // state==4 路径(t<16):先字面量(state=4)→ M2(back 1)→ 字面量(state=0
        // 再转字面量分支)→ M3×55 增长 → 字面量 → M2 短匹配(back = 1+2048+1 = 2050)
        let mut src = vec![0xEFu8]; // 239-17 = 222 字面量
        src.extend_from_slice(&(0u8..222).collect::<Vec<u8>>());
        src.extend_from_slice(&[0xC0, 0x00]); // back 1,len 7,next 0
        src.extend_from_slice(&[0x0F]); // state 0 → 字面量 18
        src.extend_from_slice(&(200u8..218).collect::<Vec<u8>>());
        for _ in 0..55 {
            src.extend_from_slice(&[0x3F, 0x04, 0x00]); // len 33,偏移 2
        }
        src.extend_from_slice(&[0x0F]); // state 0 → 字面量 18
        src.extend_from_slice(&(0u8..18).collect::<Vec<u8>>());
        // op = 222+7+18+55*33+18 = 2080;M2 短:back = 1+2048+(0x04>>2)+0 = 2050
        src.extend_from_slice(&[0x04, 0x00]);
        src.extend_from_slice(&[0x11, 0x00, 0x00]);
        let mut out = Vec::new();
        let r = lzo1x_decode(&src, 10_000, &mut out);
        let n = out.len();
        assert!(matches!(r, Ok(m) if m == n));
        assert_eq!(n, 222 + 7 + 18 + 55 * 33 + 18 + 3);
        assert_eq!(out[n - 3..], out[n - 3 - 2050..n - 3 - 2050 + 3]);
    }

    #[test]
    fn rejects_eof_marker_with_length_not_3() {
        // tok 0x12:m_pos==op 满足但 t = (2)+2 = 4 ≠ 3 → Invalid(内核 LZO_E_ERROR)
        let v = dec(&[0x15, b'A', b'B', b'C', b'D', 0x12, 0x00, 0x00], 100);
        assert!(matches!(v, Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_trailing_bytes_after_eof() {
        // EOF 标记后残留字节 → Invalid(内核 LZO_E_INPUT_NOT_CONSUMED)
        let v = dec(&[0x15, b'A', b'B', b'C', b'D', 0x11, 0x00, 0x00, 0x01], 100);
        assert!(matches!(v, Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_truncated_literal_run() {
        // NEED_IP(4+3):只剩 4 字节 → UnexpectedEof
        let v = dec(&[0x15, b'A', b'B', b'C', b'D'], 100);
        assert!(matches!(v, Err(FormatError::UnexpectedEof { .. })));
    }

    #[test]
    fn rejects_output_overrun() {
        // 产出超过 out_cap → UnexpectedEof(内核 LZO_E_OUTPUT_OVERRUN)
        let v = dec(&[0x15, b'A', b'B', b'C', b'D', 0x11, 0, 0], 2);
        assert!(matches!(v, Err(FormatError::UnexpectedEof { .. })));
    }

    #[test]
    fn rejects_lookbehind_via_m3_offset() {
        // M3 偏移 (0x14>>2)=5 > op-1=4 → 回溯越界 → Invalid(内核 TEST_LB)
        let v = dec(&[0x15, b'A', b'B', b'C', b'D', 0x21, 0x14, 0x00], 100);
        assert!(matches!(v, Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_tiny_input() {
        // in_len < 3 → UnexpectedEof(内核 :55-56)
        let v = dec(&[0x11], 100);
        assert!(matches!(v, Err(FormatError::UnexpectedEof { .. })));
    }

    #[test]
    fn decodes_version_header_empty_stream() {
        // [17,00] 版本头 + [11 00 00] EOF → Ok(空产出);首字节 0x11 ≤ 17 进主循环
        let v = dec(&[0x11, 0x00, 0x11, 0x00, 0x00], 100);
        assert!(eq_ok(&v, b""));
    }

    /// 真实流向量(规格依据 §3):moonpatr.map [PreviewPack] 块 2(末块),
    /// 块头 (940, 932)。载荷以 255 游程扩展字面量开头(00 00 00 95 → 3 零 +
    /// 终止 0x95 → 3·255+15+149+3 = 932 个字面量),`11 00 00` EOF 结尾。
    /// 2026-11-04 从真图逐字节提取,Rust/Python 双实现产出哈希一致。
    const REAL_PREVIEW_CHUNK_HEX: &str = "000000009594927E908E7B7A897E61887A68887D858E7C868F7F8592777C897277857E829373768B71738D767896777A9C7A7DA3777AA46B6E9B61649365689A64679C5B5E935A5D946F72B17779C07577C07678C47274C26C6FBC7376CA797CD47476CD7274CD7A7DD77779D17678CC6F72C16F71BD7173BE6D6FB56F72B57D7FC48A8BD07B7EBC777AB37275AA7578A961648F6A6D9564678B5659787173917A7D97787A906B6D805F62745D61726A6E7E7E83907D838F7F86917F8C917F7C8D7F48867F3B847E5887806589815F88856B8D805F8880688984668C846B8B7F3D837D227F7E22818038857C5D877E818D8090917E8A908087927E85917C818F4E54644B50615D6173747689797B8E7274876265784B4F62595C6E747787787D8B7D82907B828D7F8890828C93747C856F7780808A90838E928795968A9797798488687278758084899496757F84798388828B90848D93839093819092808D928088927D838F7E8491757A89767A8A75788A6063795D5F795F617E747698797CA38B8DBB8D8FC17577A95D60916D70A96F72AE5F639E6669A96164A67376C3696CB76C6EBF5B5FAA575AA76E70CB787ADB7376D2797BDB7476D46C6FC47375CB6B6DBD686BB66E71BC686BAF7174B76669A663669E616499686B9E7376A76E719E6D7099696C91595C7C60638073769177799074778A7074856C7080797E8D7C818F7C828E7F8791838A94828E937F8A8F858D94889B978A9D98889C96899E968A9F968AA0958EA4998DA4998A999583778E7F6D8A7D858C7C8D8B808D90818A927F87908087928187927B808D696E7D727686737587828497797B8E5053666264775C5F72575A6D6E71827B7F8E7F849373798679808C787F8A5D6470606873707982838C93848D93869095717A8151596459626B656E76575F69646C75838B928189917F87907E85907F86917C828E7C828F7D82917B7F8F797D8E777A8C76788E6C6F89737593797B9E7679A07A7DA88385B69192CA8A8CC67A7DB77679B67274B57A7DC46F72B97477C36265AE6C6FC07A7DD8686BC05C5FB26A6DCB787BE57073D5696CC5696CC27477D07174C87578C9797CCB7578C2787AC16F71B07C7FBE787BB57275AA777AAC797BAB7A7DA87A7DA46669896B6E8B767893787B91787B8E797D8F7B7F907E82927D82907E85917F8691757C867179836F7981747F85778287838E918995968794948390908693938B9997889595889695889A96859694828E90828B91808A908089917E86907D848F7D838F7B818E7C81907D80907E80927C7E91787A8D110000";

    #[test]
    fn decodes_real_preview_chunk_from_moonpatr() {
        let payload = hex_bytes(REAL_PREVIEW_CHUNK_HEX);
        assert_eq!(payload.len(), 940);
        let mut out = Vec::new();
        let produced = lzo1x_decode(&payload, 932, &mut out).expect("真块应解码成功");
        assert_eq!(produced, 932);
        assert_eq!(out.len(), 932);
        assert_eq!(sha16(&out), "2805014e9aa0295d");
        assert_eq!(
            &out[..16],
            &[
                0x94, 0x92, 0x7e, 0x90, 0x8e, 0x7b, 0x7a, 0x89, 0x7e, 0x61, 0x88, 0x7a, 0x68, 0x88,
                0x7d, 0x85
            ]
        );
    }
}
