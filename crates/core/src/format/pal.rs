// SPDX-License-Identifier: GPL-3.0-only

//! VGA 768 字节调色板(RA2 各 theater 通用 .pal)。

use super::FormatError;

/// VGA 调色板:256 个 RGB 条目,每个分量 0-255。
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

    /// 返回指定索引的 RGBA 颜色,alpha 恒为 255。
    pub fn rgba(&self, index: usize) -> [u8; 4] {
        let [r, g, b] = self.rgb[index];
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
        assert_eq!(pal.rgba(0), [10, 20, 30, 255]);
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
