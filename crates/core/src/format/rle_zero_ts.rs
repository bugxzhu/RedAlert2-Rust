// SPDX-License-Identifier: GPL-3.0-only

//! RLEZeroTS 行链解压(SHP 压缩帧与 RA2 TMP 瓦片共用;语义对齐 XCC
//! `shp_decode.cpp:748-778` 的 `RLEZeroTSDecompress`)。
//!
//! 数据是 `cy` 个背靠背行块:每块 `u16 count`(LE,含自身 2 字节)后跟
//! `count-2` 个数据字节。数据字节非 0 即调色板索引像素;为 0 则下一字节是
//! 零游程(透明)长度。已知编码器怪癖(研究文档 §1.3):
//! 1. 行尾透明像素会多发一个 +1 的零游程——在行宽 `cx` 处截断,块内剩余
//!    token 照常消耗但不输出(XCC 同语义);
//! 2. 行块链之后可能有 2-7 字节 0 填充——`count` 链是唯一权威,自然跳过。
//!
//! 行块下限 = 2(count 含自身 2 字节 → count=2 = 空行、count=3 = 单像素
//! 窄行,CAFNCB f048 行块 `03 00 <px>` 语料实证;引擎行推进只按 count 值、
//! 无下限校验,blowfish.cpp:296-314)。M1b 的 `count≥4` 下限系误推广
//! (研究 2026-11-09 §1.3-B 勘误);count=0/1 块头语义不齐,仍 Err。

use super::FormatError;

/// 解码 `src` 中的 `cy` 个行块追加到 `out`(行主序,恰好追加 `cx*cy` 字节;
/// 行不足处以透明像素 0 补齐)。返回写入的字节数。
pub fn rle_zero_ts_decode(
    src: &[u8],
    cx: usize,
    cy: usize,
    out: &mut Vec<u8>,
) -> Result<usize, FormatError> {
    let start = out.len();
    let mut sp = 0usize;
    for row in 0..cy {
        if src.len() - sp < 2 {
            return Err(FormatError::UnexpectedEof {
                needed: 2,
                remaining: src.len() - sp,
            });
        }
        let count = u16::from_le_bytes([src[sp], src[sp + 1]]) as usize;
        // 行块下限 2:count=2 空行、count=3 单像素窄行(CAFNCB f048 语料
        // 实证;引擎行推进无下限校验,blowfish.cpp:296-314);count=0/1 仍 Err
        if count < 2 {
            return Err(FormatError::Invalid(format!(
                "RLEZeroTS 行 {row} 的 count={count} 过小"
            )));
        }
        let data_end = sp + count;
        if data_end > src.len() {
            return Err(FormatError::UnexpectedEof {
                needed: count,
                remaining: src.len() - sp,
            });
        }
        sp += 2;
        let mut x = 0usize;
        while sp < data_end {
            let v = src[sp];
            sp += 1;
            if v == 0 {
                if sp >= data_end {
                    return Err(FormatError::Invalid(format!(
                        "RLEZeroTS 行 {row} 的零游程长度字节缺失"
                    )));
                }
                let run = src[sp] as usize;
                sp += 1;
                let take = run.min(cx - x);
                out.extend(std::iter::repeat_n(0u8, take));
                x += take;
            } else if x < cx {
                out.push(v);
                x += 1;
            }
        }
        // 行未填满(XCC 语义:缓冲区剩余保持透明)
        out.extend(std::iter::repeat_n(0u8, cx - x));
    }
    Ok(out.len() - start)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(src: &[u8], cx: usize, cy: usize) -> Result<Vec<u8>, FormatError> {
        let mut out = Vec::new();
        rle_zero_ts_decode(src, cx, cy, &mut out).map(|_| out)
    }

    #[test]
    fn literals_and_zero_run() {
        // 行宽 4:字面量 'A',零游程 2,'B'
        let src = [0x06, 0x00, b'A', 0x00, 0x02, b'B'];
        assert_eq!(decode(&src, 4, 1).unwrap(), vec![b'A', 0, 0, b'B']);
    }

    #[test]
    fn zero_run_clamped_at_row_width() {
        // 编码器怪癖:行尾零游程多发 1(应写 3,实发 4)——截断到行宽
        // (块 2 头字节 + 3 数据字节,count=5;brief 原文 0x06 与 5 字节缓冲自相矛盾)
        let src = [0x05, 0x00, b'A', 0x00, 0x04];
        assert_eq!(decode(&src, 4, 1).unwrap(), vec![b'A', 0, 0, 0]);
    }

    #[test]
    fn rows_are_independent_blocks() {
        let src = [0x04, 0x00, 0x11, 0x22, 0x04, 0x00, 0x33, 0x44];
        assert_eq!(decode(&src, 2, 2).unwrap(), vec![0x11, 0x22, 0x33, 0x44]);
    }

    #[test]
    fn tokens_after_row_full_are_consumed() {
        // 零游程 5 → 截断为行宽 2;块内还剩一个字面量 'Z'——消耗但不输出
        // (块 2 头字节 + 3 数据字节,count=5;brief 原文 0x06 与 5 字节缓冲自相矛盾)
        let src = [0x05, 0x00, 0x00, 0x05, b'Z'];
        assert_eq!(decode(&src, 2, 1).unwrap(), vec![0, 0]);
    }

    #[test]
    fn row_underfill_pads_transparent() {
        // 块只有 2 个字面量,行宽 4 → 其余补 0(count 链权威,不足处透明)
        let src = [0x04, 0x00, 0x77, 0x88];
        assert_eq!(decode(&src, 4, 1).unwrap(), vec![0x77, 0x88, 0, 0]);
    }

    #[test]
    fn rejects_count_below_two() {
        // count=0/1:块头语义不齐(含自身 2 字节则数据长为负),恒 Err
        assert!(matches!(
            decode(&[0x00, 0x00], 4, 1),
            Err(FormatError::Invalid(_))
        ));
        assert!(matches!(
            decode(&[0x01, 0x00], 4, 1),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn count_two_is_empty_row() {
        // count=2 = 空行(无数据字节)→ 整行透明补零(M4c 下限 2)
        let src = [0x02, 0x00];
        assert_eq!(decode(&src, 4, 1).unwrap(), vec![0, 0, 0, 0]);
    }

    #[test]
    fn count_three_is_single_pixel_row() {
        // count=3 = 单像素窄行(CAFNCB f048 行块 `03 00 <px>` 语料实证,
        // 研究 §1.3-B;M1b 的 count≥4 下限误拒此形态)
        let src = [0x03, 0x00, 0x2A];
        assert_eq!(decode(&src, 1, 1).unwrap(), vec![0x2A]);
    }

    #[test]
    fn rejects_truncated_block() {
        // count 声明 8 字节块但只剩块头
        let src = [0x08, 0x00, 0x11];
        assert!(matches!(
            decode(&src, 4, 1),
            Err(FormatError::UnexpectedEof { .. })
        ));
    }
}
