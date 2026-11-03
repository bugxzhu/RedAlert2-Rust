// SPDX-License-Identifier: GPL-3.0-only

//! LCW(Format 80)解压。
//!
//! 移植自 Red Alert 源码 LCW.CPP(原版权 Electronic Arts,GPL-3.0)。

use super::FormatError;

pub fn lcw_decode(src: &[u8], out: &mut Vec<u8>) -> Result<usize, FormatError> {
    let mut sp = 0usize;
    // 读一个字节并前进(显式递增,避免宏/闭包重复读取同一位置)
    fn next(src: &[u8], sp: &mut usize) -> Result<u8, FormatError> {
        let b = *src.get(*sp).ok_or(FormatError::UnexpectedEof {
            needed: 1,
            remaining: 0,
        })?;
        *sp += 1;
        Ok(b)
    }
    fn next_u16(src: &[u8], sp: &mut usize) -> Result<u16, FormatError> {
        let lo = next(src, sp)? as u16;
        let hi = next(src, sp)? as u16;
        Ok(lo | (hi << 8))
    }
    // 从 out 的绝对位置 p 起逐字节复制 count 个(允许重叠,等价 RA1 的 copy_ptr++ 循环)
    fn copy_from_out(out: &mut Vec<u8>, p: usize, count: usize) {
        for i in (p..).take(count) {
            let byte = out[i];
            out.push(byte);
        }
    }
    loop {
        let op = next(src, &mut sp)?;
        if op & 0x80 == 0 {
            // 短回拷(相对)
            let b = next(src, &mut sp)?;
            let count = ((op >> 4) as usize) + 3;
            let back = (b as usize) + (((op & 0x0F) as usize) << 8);
            if back == 0 || back > out.len() {
                return Err(FormatError::Invalid(format!("LCW 短回拷 back={back} 无效")));
            }
            let start = out.len() - back;
            copy_from_out(out, start, count);
        } else if op & 0x40 == 0 {
            if op == 0x80 {
                return Ok(out.len());
            }
            // 源字面量
            let count = (op & 0x3F) as usize;
            let end = sp + count;
            let bytes = src.get(sp..end).ok_or(FormatError::UnexpectedEof {
                needed: count,
                remaining: src.len() - sp,
            })?;
            sp = end;
            out.extend_from_slice(bytes);
        } else if op == 0xFE {
            // 长填充
            let count = next_u16(src, &mut sp)? as usize;
            let byte = next(src, &mut sp)?;
            out.extend(std::iter::repeat_n(byte, count));
        } else if op == 0xFF {
            // 长回拷(绝对)
            let count = next_u16(src, &mut sp)? as usize;
            let off = next_u16(src, &mut sp)? as usize;
            if off >= out.len() {
                return Err(FormatError::Invalid(format!("LCW 长回拷 off={off} 越界")));
            }
            copy_from_out(out, off, count);
        } else {
            // 中回拷(绝对)
            let count = ((op & 0x3F) as usize) + 3;
            let off = next_u16(src, &mut sp)? as usize;
            if off >= out.len() {
                return Err(FormatError::Invalid(format!("LCW 中回拷 off={off} 越界")));
            }
            copy_from_out(out, off, count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_marker_only() {
        let mut out = Vec::new();
        assert_eq!(lcw_decode(&[0x80], &mut out).unwrap(), 0);
        assert!(out.is_empty());
    }

    #[test]
    fn literals_short() {
        // 0x83: 复制 3 个字面量
        let mut out = Vec::new();
        lcw_decode(&[0x83, b'A', b'B', b'C', 0x80], &mut out).unwrap();
        assert_eq!(out, b"ABC");
    }

    #[test]
    fn short_back_copy_replicates() {
        // 先输出 "AB"(0x82 + 2 字面量),再 op=0x00: count=(0>>4)+3=3, back=0x02
        // 重叠回拷:AB -> 追加 A,B,A -> "ABABA"
        let mut out = Vec::new();
        lcw_decode(&[0x82, b'A', b'B', 0x00, 0x02, 0x80], &mut out).unwrap();
        assert_eq!(out, b"ABABA");
    }

    #[test]
    fn long_run() {
        // 0xFE, count=4 LE, byte=0x77 -> 4 字节 0x77
        let mut out = Vec::new();
        lcw_decode(&[0xFE, 0x04, 0x00, 0x77, 0x80], &mut out).unwrap();
        assert_eq!(out, vec![0x77u8; 4]);
    }

    #[test]
    fn medium_back_copy_absolute() {
        // 字面量 "XY",然后中回拷 op=0xC5: count=(5)+3=8, off=0x0000 -> "XY" 重复到 10 字节
        let mut out = Vec::new();
        lcw_decode(&[0x82, b'X', b'Y', 0xC5, 0x00, 0x00, 0x80], &mut out).unwrap();
        assert_eq!(out, b"XYXYXYXYXY");
    }

    #[test]
    fn long_back_copy_absolute() {
        // 字面量 "XY",然后长回拷 op=0xFF: count=3, off=0 -> 追加 "XYX"
        let mut out = Vec::new();
        lcw_decode(
            &[0x82, b'X', b'Y', 0xFF, 0x03, 0x00, 0x00, 0x00, 0x80],
            &mut out,
        )
        .unwrap();
        assert_eq!(out, b"XYXYX");
    }

    #[test]
    fn bad_back_offset_is_rejected() {
        let mut out = Vec::new();
        assert!(matches!(
            lcw_decode(&[0x01, 0x01, 0x80], &mut out), // back=1 但 out 为空
            Err(FormatError::Invalid(_))
        ));
    }
}
