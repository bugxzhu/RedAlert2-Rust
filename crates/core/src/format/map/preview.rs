// SPDX-License-Identifier: GPL-3.0-only

//! `[PreviewPack]` 解压后的小地图位图:裸 24bpp RGB,无头。
//!
//! 研究 §5:EA 产图长度恰为 3·[Preview]Size 的 pw·ph(c1a01md 3·162·59 =
//! 28674 ✓);社区产图器的截断/异常尾块由 pack 层截停(研究 §5 + 2026-11-04
//! 复核:7 张 DDLY 图的干净前缀全部恰为 3·pw·ph,异常块只是位图完整后的
//! 尾垃圾)。前缀长度 ≠ 3·pw·ph 时返回 None("无预览",不报错)。
//! 像素序/通道序(BGR?底行在上?)M2 不裁决,字节原样暴露(渲染时定,
//! 研究 §14-3)。

/// 预览尺寸合法性上界([Preview]Size 谎报防护;语料真图最大 798×100)。
pub const PREVIEW_MAX_PIXELS: u64 = 16_777_216; // 4096²

pub struct PreviewImage {
    pub width: u32,
    pub height: u32,
    /// 24bpp RGB,行主序(字节原样;长度恒 3·width·height)。
    pub rgb: Vec<u8>,
}

impl PreviewImage {
    pub fn parse(data: &[u8], pw: u32, ph: u32) -> Option<Self> {
        if pw == 0 || ph == 0 || pw > 8192 || ph > 8192 {
            return None;
        }
        let pixels = pw as u64 * ph as u64;
        if pixels > PREVIEW_MAX_PIXELS {
            return None;
        }
        if data.len() as u64 != 3 * pixels {
            return None;
        }
        Some(PreviewImage {
            width: pw,
            height: ph,
            rgb: data.to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_length_some_verbatim() {
        let data: Vec<u8> = (0..3 * 4 * 2).map(|i| (i % 251) as u8).collect();
        let img = PreviewImage::parse(&data, 4, 2).unwrap();
        assert_eq!((img.width, img.height), (4, 2));
        assert_eq!(img.rgb, data);
    }

    #[test]
    fn truncated_prefix_is_none() {
        let data = vec![0u8; 3 * 4 * 2 - 3];
        assert!(PreviewImage::parse(&data, 4, 2).is_none());
    }

    #[test]
    fn longer_than_3wh_is_none() {
        let data = vec![0u8; 3 * 4 * 2 + 1];
        assert!(PreviewImage::parse(&data, 4, 2).is_none());
    }

    #[test]
    fn hostile_dimensions_are_none_not_panic() {
        // 尺寸谎报:u64 算术,3·10⁹ 不溢出、超界返回 None(debug 零 panic)
        assert!(PreviewImage::parse(&[], 1_000_000_000, 1_000_000_000).is_none());
        assert!(PreviewImage::parse(&[], 0, 2).is_none());
        assert!(PreviewImage::parse(&[], 8193, 2).is_none());
    }
}
