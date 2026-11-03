// SPDX-License-Identifier: GPL-3.0-only

//! Westwood CRC(RA1 移植)、标准 CRC-32 与 TS/RA2 MIX 文件名 ID。
//!
//! `westwood_crc` 移植自 Red Alert 源码 CRC.CPP / CRC.H(原版权 Electronic Arts,GPL-3.0),
//! 仅代表 RA1 语义,不是 TS/RA2 的 MIX 文件名 ID 算法。
//! `mix_name_id` 按 XCC 源码 `misc/mix_file.cpp` 的 `Cmix_file::get_id`(ts/ra2/ra2_yr
//! 分支)实现(XCC 作者 Olaf van der Spek,GPL)。

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

/// 标准 CRC-32(反射,多项式 0xEDB88320,初值与终值异或均为 0xFFFFFFFF;zlib 兼容)。
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// TS/RA2 MIX 文件名 ID(XCC `Cmix_file::get_id` 的 ts/ra2/yr 分支语义)。
///
/// 名字字节级 ASCII 大写化、`/` 视同 `\` 后,若长度非 4 的倍数(余 r):
/// 追加 1 字节(值为 r)与 3-r 个"余数组首字符"拷贝,再对补齐字节串求 [`crc32`]。
/// 实测:ra2md.mix / expandmd01.mix / langmd.mix 三库 LMD 全部 76 个名字 76/76 命中。
pub fn mix_name_id(name: &str) -> u32 {
    let mut buf: Vec<u8> = name.bytes().map(|b| b.to_ascii_uppercase()).collect();
    for b in &mut buf {
        if *b == b'/' {
            *b = b'\\';
        }
    }
    let l = buf.len();
    if l & 3 != 0 {
        let a = l & !3;
        let r = (l - a) as u8;
        buf.push(r);
        let c = buf[a];
        buf.extend(std::iter::repeat_n(c, (3 - r) as usize));
    }
    crc32(&buf)
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
        // 手算:RULESMD.INI -> 0x72033E38(仅 RA1 语义,与 mix_name_id 无关)
        assert_eq!(westwood_crc(b"RULESMD.INI"), 0x7203_3E38);
    }

    #[test]
    fn crc32_known_vectors() {
        // 标准 CRC-32 公开校验向量(zlib 兼容)
        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"a"), 0xE8B7_BE43);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn name_id_no_padding_when_multiple_of_4() {
        // "ABCD" 长度 4,无补齐:退化为 crc32("ABCD")
        assert_eq!(mix_name_id("abcd"), 0xDB17_20A5);
        assert_eq!(mix_name_id(""), 0);
    }

    #[test]
    fn name_id_real_archive_vectors() {
        // 向量直接取自真实 MIX 索引字节(三库 76/76 命中的抽样)
        assert_eq!(mix_name_id("key.ini"), 0x763C_81DD); // r=3, 追加 [0x03]
        assert_eq!(mix_name_id("ra2md.csf"), 0xBD83_5079); // r=1, 追加 [0x01,'F','F']
        assert_eq!(mix_name_id("local mix database.dat"), 0x366E_051F); // r=2, 追加 [0x02,'A']
        assert_eq!(mix_name_id("isodes.mix"), 0x8C8C_CA19); // r=2, 追加 [0x02,'I']
    }

    #[test]
    fn name_id_case_and_slash_normalization() {
        assert_eq!(mix_name_id("Ra2Md.CsF"), 0xBD83_5079);
        assert_eq!(mix_name_id("AB/CD.MIX"), mix_name_id("AB\\CD.MIX"));
    }
}
