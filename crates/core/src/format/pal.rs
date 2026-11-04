// SPDX-License-Identifier: GPL-3.0-only

//! VGA 768 字节调色板(RA2 各 theater 通用 .pal)。

use super::FormatError;

/// VGA 调色板:256 个 RGB 条目。
///
/// 西木 .pal 每分量实为 **6 位(0-63,VGA RAMDAC 位宽)**——实测
/// isotem/isosno/isourb 三张剧场调色板 2304 字节全 ≤63(M2 T10 审查,
/// 2026-11-04)。`rgb` 按原样保存原始字节(格式保真);视图侧取色用
/// [`Palette::rgb8`](6 位→8 位换算),勿把原始值直接当 8 位 RGB 写出
/// (否则亮度偏暗约 4 倍)。
pub struct Palette {
    pub rgb: [[u8; 3]; 256],
}

impl Palette {
    /// 从恰好 768 字节的原始数据解析调色板。
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        if data.len() != 768 {
            return Err(FormatError::Invalid(format!(
                "调色板应为 768 字节,实际 {}",
                data.len()
            )));
        }
        let mut rgb = [[0u8; 3]; 256];
        for (i, slot) in rgb.iter_mut().enumerate() {
            *slot = [data[i * 3], data[i * 3 + 1], data[i * 3 + 2]];
        }
        Ok(Palette { rgb })
    }

    /// 指定索引的 8 位 RGB(视图侧取色):6 位→8 位按位复制展开
    /// `(v << 2) | (v >> 4)`(VGA 经典展开:0→0、63→255,整数无舍入)。
    pub fn rgb8(&self, index: usize) -> [u8; 3] {
        let [r, g, b] = self.rgb[index];
        [
            (r << 2) | (r >> 4),
            (g << 2) | (g >> 4),
            (b << 2) | (b >> 4),
        ]
    }

    /// 返回指定索引的 RGBA 颜色(RGB 已 6 位→8 位换算),alpha 恒为 255。
    pub fn rgba(&self, index: usize) -> [u8; 4] {
        let [r, g, b] = self.rgb8(index);
        [r, g, b, 255]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_768_bytes() {
        let mut data = [0u8; 768];
        data[0] = 10;
        data[1] = 20;
        data[2] = 30;
        let pal = Palette::parse(&data).unwrap();
        assert_eq!(pal.rgb[0], [10, 20, 30]);
        assert_eq!(pal.rgb[255], [0, 0, 0]);
        assert_eq!(pal.rgba(0), [40, 81, 121, 255]);
    }

    #[test]
    fn converts_6bit_to_8bit() {
        // 原始字段保 6 位保真;rgb8 按位复制展开 (v<<2)|(v>>4)。
        let mut data = [0u8; 768];
        data[0] = 63;
        data[1] = 51;
        data[2] = 44;
        data[3] = 27;
        let pal = Palette::parse(&data).unwrap();
        assert_eq!(pal.rgb8(0), [255, 207, 178]);
        assert_eq!(pal.rgb8(1)[0], 109); // 27 → (27<<2)|(27>>4) = 109
        assert_eq!(pal.rgb8(255), [0, 0, 0]); // 0 → 0
    }

    #[test]
    fn rejects_wrong_size() {
        assert!(matches!(
            Palette::parse(&[0u8; 767]),
            Err(FormatError::Invalid(_))
        ));
        assert!(matches!(
            Palette::parse(&[0u8; 769]),
            Err(FormatError::Invalid(_))
        ));
    }
}
