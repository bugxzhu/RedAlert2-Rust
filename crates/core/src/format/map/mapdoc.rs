// SPDX-License-Identifier: GPL-3.0-only

//! 地图 INI 解码策略(普查 2026-11-04-map-ini-survey.md §8-3)。
//!
//! 字节 → 文本:严格 UTF-8 优先(774/807 图完美,含真 UTF-8 的 随机地图.map),
//! 失败回退 GBK 有损(28 张 GBK 图 + 5 张截断 GBK 图;绝不 strict、绝不失败)。
//! IniDocument 自身的段头/get_int/孤儿行适配在 M2 的 ini.rs 任务完成。

use super::gbk::gbk_lossy;

pub fn decode_map_text(data: &[u8]) -> String {
    match std::str::from_utf8(data) {
        Ok(s) => s.to_string(),
        Err(_) => gbk_lossy(data),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_takes_priority() {
        // 规格依据 §10:随机地图.map 的 Name 是真 UTF-8;若顺序反了会解出
        // GBK 乱码"闅忔満鍦板浘"
        // 编码偏差记录:brief 原文 `"随机地图".encode_utf8(&mut [0u8; 16])` 无法
        // 编译——`encode_utf8` 是 `char` 的方法而非 `str`(`str` 只有
        // `encode_utf16`)。等价最小修复:字符串字面量本身就是 UTF-8,
        // `as_bytes()` 同样由编译器保证字节正确(e9 9a 8f e6 9c ba e5 9c b0 e5 9b be)。
        let bytes = "随机地图".as_bytes().to_vec();
        assert_eq!(decode_map_text(&bytes), "随机地图");
    }

    #[test]
    fn gbk_fallback_decodes_names() {
        assert_eq!(decode_map_text(&[0xBA, 0xA3, 0x87, 0x5B]), "海嘯");
    }

    #[test]
    fn never_fails_on_hostile_bytes() {
        // 截断 GBK 序列 + 任意二进制垃圾:不 panic、不失败,只出现 U+FFFD
        let out = decode_map_text(&[0x31, 0xCD, 0xE6, 0xBC, 0xD2, 0xD4, 0xAE, 0xBE]);
        assert_eq!(out, "1玩家援\u{FFFD}");
        let out = decode_map_text(&[0xFF, 0xFF, 0x81, 0xFE]);
        assert!(out.contains('\u{FFFD}'));
    }
}
