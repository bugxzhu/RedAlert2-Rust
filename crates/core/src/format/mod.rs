// SPDX-License-Identifier: GPL-3.0-only

//! 原版资产容器与数据格式解析。

pub mod aud;
pub mod bytes;
pub mod crc;
pub mod csf;
pub mod ini;
pub mod lcw;
pub mod mix;
pub mod pal;
pub mod rle_zero_ts;
pub mod shp;
pub mod tmp;
pub mod vxl;

use std::fmt;

/// 格式解析错误。
#[derive(Debug)]
pub enum FormatError {
    /// 输入数据不足。
    UnexpectedEof { needed: usize, remaining: usize },
    /// 格式合法但本版本不支持(如加密 MIX 索引)。
    Unsupported(String),
    /// 数据不符合格式。
    Invalid(String),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::UnexpectedEof { needed, remaining } => {
                write!(f, "数据不足:需要 {needed} 字节,剩余 {remaining}")
            }
            FormatError::Unsupported(s) => write!(f, "不支持:{}", s),
            FormatError::Invalid(s) => write!(f, "无效数据:{}", s),
        }
    }
}

impl std::error::Error for FormatError {}
