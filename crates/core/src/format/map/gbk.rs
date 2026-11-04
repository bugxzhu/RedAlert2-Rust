// SPDX-License-Identifier: GPL-3.0-only

//! GBK(CP936)有损解码——地图 INI 的兜底解码路径(mapdoc 调用)。
//!
//! 单字节区 = ASCII(0x00..=0x7F);0x80 = 欧元符号;0x81..=0xFE 为双字节
//! lead,配 trail 0x40..=0xFE(除 0x7F),查生成表 [`gbk_table::GBK`];
//! 尾部未配对 lead / 非法 trail / 未定义码位 → U+FFFD。全路径零 panic。

use super::gbk_table::GBK;

pub fn gbk_lossy(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len());
    let mut i = 0usize;
    while i < data.len() {
        let b = data[i];
        if b < 0x80 {
            out.push(b as char);
            i += 1;
        } else if b == 0x80 {
            out.push('\u{20AC}');
            i += 1;
        } else if i + 1 < data.len() {
            let trail = data[i + 1];
            if (0x40..=0xFE).contains(&trail) && trail != 0x7F {
                let t = if trail < 0x7F {
                    trail - 0x40
                } else {
                    trail - 0x41
                };
                let idx = (b as usize - 0x81) * 190 + t as usize;
                out.push(char::from_u32(GBK[idx] as u32).unwrap_or('\u{FFFD}'));
            } else {
                out.push('\u{FFFD}');
            }
            i += 2;
        } else {
            out.push('\u{FFFD}');
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_and_euro_passthrough() {
        assert_eq!(gbk_lossy(b"Name=Test"), "Name=Test");
        assert_eq!(gbk_lossy(&[0x80]), "\u{20AC}");
    }

    #[test]
    fn decodes_gbk_names_from_real_maps() {
        // 规格依据 §10:城市海啸.map Name 原始字节 → "海嘯";
        // 分而治之.map Name → "[4]分而治之"
        assert_eq!(gbk_lossy(&[0xBA, 0xA3, 0x87, 0x5B]), "海嘯");
        assert_eq!(
            gbk_lossy(&[0x5B, 0x34, 0x5D, 0xB7, 0xD6, 0xB6, 0xF8, 0xD6, 0xCE, 0xD6, 0xAE]),
            "[4]分而治之"
        );
    }

    #[test]
    fn truncated_trailing_lead_becomes_replacement() {
        // 规格依据 §10:终极海战防守图.map 的截断 Name 序列(Python gbk/replace 对拍)
        assert_eq!(
            gbk_lossy(&[0x31, 0xCD, 0xE6, 0xBC, 0xD2, 0xD4, 0xAE, 0xBE]),
            "1玩家援\u{FFFD}"
        );
    }

    #[test]
    fn invalid_trail_consumes_pair_and_replaces() {
        // trail 0x7F 非法:成对消费、单个替换,后续字节不受影响
        assert_eq!(gbk_lossy(&[b'A', 0xC7, 0x7F, b'B']), "A\u{FFFD}B");
    }
}
