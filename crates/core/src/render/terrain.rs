// SPDX-License-Identifier: GPL-3.0-only

//! 地形层合成(M4a 第一层;M4b 的 overlay/物件层在本画布结构上追加)。

use crate::format::pal::Palette;

/// B 族阶梯行宽:行 r 宽 `4·min(r+1, cy−1−r)`,末行宽 0——引擎 .data
/// 两张 60×30 mask 表(0x3E2B5C/0x3EC4C8)行宽铁证。
pub fn row_width(r: usize, cy: usize) -> usize {
    4 * (r + 1).min(cy - 1 - r)
}

/// 菱形铺入 cx×cy 画布(单瓦片出图路径):行外区 = 调色板 0 号色;
/// 偶 cy 阶梯总耗 `4k² = cx·cy/2` 精确闭合,奇 cy 耗 `cx·cy/2−1` →
/// `None`(病态输入显式失败,不静默漏绘)。
pub fn paint_canvas(palette: &Palette, cx: usize, cy: usize, diamond: &[u8]) -> Option<Vec<u8>> {
    let bg = palette.rgb8(0);
    let mut rgb = Vec::with_capacity(3 * cx * cy);
    let mut off = 0usize;
    for r in 0..cy {
        let w = row_width(r, cy);
        let x0 = (cx - w) / 2;
        for c in 0..cx {
            let color = if c >= x0 && c < x0 + w {
                let idx = *diamond.get(off)? as usize;
                off += 1;
                palette.rgb8(idx)
            } else {
                bg
            };
            rgb.extend_from_slice(&color);
        }
    }
    (off == diamond.len()).then_some(rgb)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 256 色测试调色板:i 号色 6 位原值 = i&63(rgb8 = 展开),0 号 = 0。
    fn test_palette() -> Palette {
        let mut rgb = [[0u8; 3]; 256];
        for (i, slot) in rgb.iter_mut().enumerate() {
            let v = (i & 63) as u8;
            *slot = [v, v, v];
        }
        Palette { rgb }
    }

    /// B 族阶梯行宽序列 60×30 逐行钉死(mask 表 0x3E2B5C/0x3EC4C8 铁证):
    /// [4,8,…,60,56,…,4,0],和 900 = 60·30/2 精确闭合。
    #[test]
    fn ladder_row_widths_60x30() {
        let widths: Vec<usize> = (0..30).map(|r| row_width(r, 30)).collect();
        let expect: Vec<usize> = (1..=15)
            .map(|i| 4 * i)
            .chain((0..15).rev().map(|i| 4 * i))
            .collect();
        assert_eq!(widths, expect);
        assert_eq!(widths.iter().sum::<usize>(), 900);
        let pal = test_palette();
        assert!(paint_canvas(&pal, 60, 30, &vec![1u8; 900]).is_some());
    }

    /// 8×4 全量字节级钉:行宽 [4,8,4,0](末行宽 0 全背景),x0 = [2,0,2,4]。
    #[test]
    fn canvas_8x4_byte_exact() {
        let pal = test_palette();
        let diamond: Vec<u8> = (1..=16).collect();
        let rgb = paint_canvas(&pal, 8, 4, &diamond).unwrap();
        assert_eq!(rgb.len(), 3 * 8 * 4);
        let px = |r: usize, c: usize| -> [u8; 3] {
            let i = 3 * (r * 8 + c);
            [rgb[i], rgb[i + 1], rgb[i + 2]]
        };
        let widths = [4usize, 8, 4, 0];
        let x0s = [2usize, 0, 2, 4];
        let mut it = 1u8;
        for r in 0..4 {
            for c in 0..8 {
                let inside = (x0s[r]..x0s[r] + widths[r]).contains(&c);
                if inside {
                    let v = it;
                    it += 1;
                    assert_eq!(px(r, c), [(v << 2) | (v >> 4); 3], "({r},{c})");
                } else {
                    assert_eq!(px(r, c), [0, 0, 0], "行外 ({r},{c})");
                }
            }
        }
    }

    /// 奇 cy(6×3):阶梯耗 8/9 → None;偶 cy 菱形短缺同 None(敌意饱和)。
    #[test]
    fn odd_canvas_and_short_diamond_return_none() {
        let pal = test_palette();
        assert!(paint_canvas(&pal, 6, 3, &[9u8; 9]).is_none());
        assert!(paint_canvas(&pal, 8, 4, &[7u8; 15]).is_none());
    }
}
