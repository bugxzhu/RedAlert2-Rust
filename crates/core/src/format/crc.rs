// SPDX-License-Identifier: GPL-3.0-only

//! Westwood CRC 引擎与 MIX 文件名 ID。
//!
//! 移植自 Red Alert 源码 CRC.CPP / CRC.H / MIXFILE.CPP(原版权 Electronic Arts,GPL-3.0)。

/// Westwood CRC:每 4 字节按小端组成 u32,折叠进循环左移 1 位后的累加值;
/// 结尾不足 4 字节的余数按小端部分组入暂存值后一并折叠。空输入返回 0。
pub fn westwood_crc(data: &[u8]) -> u32 {
    let mut crc: u32 = 0;
    let mut staging: u32 = 0;
    let mut index: usize = 0;
    for &b in data {
        staging |= (b as u32) << (8 * index);
        index += 1;
        if index == 4 {
            crc = crc.rotate_left(1).wrapping_add(staging);
            staging = 0;
            index = 0;
        }
    }
    if index != 0 {
        crc = crc.rotate_left(1).wrapping_add(staging);
    }
    crc
}

/// MIX 文件名 ID:ASCII 大写化后求 Westwood CRC。
pub fn mix_name_id(name: &str) -> u32 {
    let upper: Vec<u8> = name.to_ascii_uppercase().into_bytes();
    westwood_crc(&upper)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_zero() {
        assert_eq!(westwood_crc(b""), 0);
    }

    #[test]
    fn single_byte() {
        // staged=0x54, crc = rol(0,1)+0x54
        assert_eq!(westwood_crc(b"T"), 0x54);
    }

    #[test]
    fn one_full_chunk() {
        // "TEST" -> LE u32 0x54534554, crc = 0 + 0x54534554
        assert_eq!(westwood_crc(b"TEST"), 0x5453_4554);
    }

    #[test]
    fn two_full_chunks() {
        // rol(0x54534554,1)=0xA8A68AA8, +0x54534554 = 0xFCF9CFFC
        assert_eq!(westwood_crc(b"TESTTEST"), 0xFCF9_CFFC);
    }

    #[test]
    fn remainder_bytes_fold_on_finish() {
        // 手算:RULESMD.INI -> 0x72033E38
        // 该向量由算法手推;最终裁决是集成测试(ra2md.mix 中必须找到 rulesmd.ini)
        assert_eq!(westwood_crc(b"RULESMD.INI"), 0x7203_3E38);
    }

    #[test]
    fn name_id_uppercases() {
        assert_eq!(mix_name_id("rulesmd.ini"), mix_name_id("RulesMD.ini"));
        assert_eq!(mix_name_id("rulesmd.ini"), westwood_crc(b"RULESMD.INI"));
    }
}
