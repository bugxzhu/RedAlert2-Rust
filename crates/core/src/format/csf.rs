// SPDX-License-Identifier: GPL-3.0-only

//! CSF 本地化字符串表(文件魔数在盘上为 ASCII `" FSC"`)。
//!
//! 布局(研究文档 2026-11-03-csf-aud-layouts.md §1;XCC `csf_file.{h,cpp}`
//! 双向验证,ra2md.csf 全量走查通过):
//! 24 字节头 `{i32 magic=" FSC"=0x4353_4620; i32 flags1(不透明,勿校验);
//! i32 count1=标签数(唯一循环边界);i32 count2(只写不读,忽略);i32 zero;
//! i32 language(不透明)}`。随后 `count1` 个标签背靠背无填充:
//! `{i32 " LBL"=0x4C42_4C20; i32 flags(bit0=有值); i32 name_len; name 原始
//! 字节(不取反);[i32 " RTS"=0x5354_5220 或 "WRTS"=0x5354_5257;
//! i32 cb_str(UTF-16 码元数,不是字节数);2*cb_str 字节 UTF-16LE 逐码元
//! 取反存储;WRTS 另有 i32 extra_len + extra 原始字节]}`。
//! 字节恒等式(写入器):file_size == 24 + Σ(20 + name_len + 2*cb_str [+4+extra])。
//! 解码:u16 LE 读出后逐码元 XOR 0xFFFF,按 UTF-16 解释(lossy:不配对代理
//! 替换为 U+FFFD——M1b 无重写需求,选择有损以保零 panic)。
//! 重复名:保留文件顺序(查找取首个);ra2md.csf 实测无重复。

use super::bytes::ByteReader;
use super::FormatError;

/// 文件魔数:磁盘字节 `20 46 53 43`(" FSC")按 u32 LE 读出。
pub const CSF_MAGIC: u32 = 0x4353_4620;
const LBL_MAGIC: u32 = 0x4C42_4C20; // " LBL"
const STR_MAGIC: u32 = 0x5354_5220; // " RTS"
const STRW_MAGIC: u32 = 0x5354_5257; // "WRTS"

/// 一个标签。`value` 为 `None` 仅当 flags bit0==0(无值标签);
/// 空值标签是 `Some("")`。`extra` 仅 'WRTS' 标签存在(原始 ANSI 字节,不取反)。
pub struct CsfLabel {
    pub name: String,
    pub value: Option<String>,
    pub extra: Option<Vec<u8>>,
}

/// 解析后的字符串表(保留文件顺序)。
pub struct CsfDocument {
    pub labels: Vec<CsfLabel>,
}

impl CsfDocument {
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        let mut r = ByteReader::new(data);
        let magic = r.read_u32_le()?;
        if magic != CSF_MAGIC {
            return Err(FormatError::Invalid(format!("CSF 魔数不符: 0x{magic:08X}")));
        }
        let _flags1 = r.read_u32_le()?;
        let count1 = r.read_u32_le()? as usize;
        let _count2 = r.read_u32_le()?;
        let _zero = r.read_u32_le()?;
        let _language = r.read_u32_le()?;
        let mut labels = Vec::new();
        for i in 0..count1 {
            let lbl = r.read_u32_le()?;
            if lbl != LBL_MAGIC {
                return Err(FormatError::Invalid(format!(
                    "标签 {i} 魔数不符: 0x{lbl:08X}"
                )));
            }
            let flags = r.read_u32_le()?;
            let name_len = r.read_u32_le()? as usize;
            let name = String::from_utf8_lossy(r.read_bytes(name_len)?).into_owned();
            let (value, extra) = if flags & 1 != 0 {
                let str_magic = r.read_u32_le()?;
                let cb_str = r.read_u32_le()? as usize;
                let need = cb_str
                    .checked_mul(2)
                    .ok_or_else(|| FormatError::Invalid("字符串长度溢出".to_string()))?;
                let raw = r.read_bytes(need)?;
                // clippy chunks_exact_to_as_chunks(Rust 1.99.1 deny-by-default):
                // as_chunks 与 chunks_exact 语义一致(忽略余数切片;此处
                // need=2*cb_str 恒为偶数,余数恒空),行为不变。
                let units: Vec<u16> = raw
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_le_bytes(*c) ^ 0xFFFF)
                    .collect();
                let value = String::from_utf16_lossy(&units);
                let extra = if str_magic == STRW_MAGIC {
                    let extra_len = r.read_u32_le()? as usize;
                    Some(r.read_bytes(extra_len)?.to_vec())
                } else if str_magic == STR_MAGIC {
                    None
                } else {
                    return Err(FormatError::Invalid(format!(
                        "字符串魔数不符: 0x{str_magic:08X}"
                    )));
                };
                (Some(value), extra)
            } else {
                (None, None)
            };
            labels.push(CsfLabel { name, value, extra });
        }
        if r.remaining() != 0 {
            return Err(FormatError::Invalid(format!(
                "标签流结束后仍有 {} 字节",
                r.remaining()
            )));
        }
        Ok(CsfDocument { labels })
    }

    /// 按名精确查找(大小写敏感;保留文件顺序,取首个)。
    pub fn find(&self, name: &str) -> Option<&CsfLabel> {
        self.labels.iter().find(|l| l.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 值编码:逐 UTF-16 码元取反后按 LE 拼字节(XCC encode 的逆)。
    fn inv_bytes(s: &str) -> Vec<u8> {
        s.encode_utf16()
            .flat_map(|u| (u ^ 0xFFFF).to_le_bytes())
            .collect()
    }

    fn label_blob(name: &str, value: Option<&str>, extra: Option<&[u8]>) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&LBL_MAGIC.to_le_bytes());
        match value {
            Some(s) => {
                v.extend_from_slice(&1u32.to_le_bytes()); // flags bit0=1
                v.extend_from_slice(&(name.len() as u32).to_le_bytes());
                v.extend_from_slice(name.as_bytes());
                let magic = if extra.is_some() {
                    STRW_MAGIC
                } else {
                    STR_MAGIC
                };
                v.extend_from_slice(&magic.to_le_bytes());
                v.extend_from_slice(&(s.encode_utf16().count() as u32).to_le_bytes());
                v.extend_from_slice(&inv_bytes(s));
                if let Some(e) = extra {
                    v.extend_from_slice(&(e.len() as u32).to_le_bytes());
                    v.extend_from_slice(e);
                }
            }
            None => {
                v.extend_from_slice(&0u32.to_le_bytes()); // flags bit0=0:无值
                v.extend_from_slice(&(name.len() as u32).to_le_bytes());
                v.extend_from_slice(name.as_bytes());
            }
        }
        v
    }

    fn csf_bytes(labels: &[Vec<u8>]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&CSF_MAGIC.to_le_bytes());
        v.extend_from_slice(&3u32.to_le_bytes()); // flags1(不透明)
        v.extend_from_slice(&(labels.len() as u32).to_le_bytes()); // count1
        v.extend_from_slice(&(labels.len() as u32).to_le_bytes()); // count2
        v.extend_from_slice(&0u32.to_le_bytes()); // zero
        v.extend_from_slice(&0u32.to_le_bytes()); // language
        for l in labels {
            v.extend_from_slice(l);
        }
        v
    }

    #[test]
    fn decodes_inverted_utf16_value() {
        let data = csf_bytes(&[label_blob("GUI:OK", Some("确定"), None)]);
        let doc = CsfDocument::parse(&data).unwrap();
        assert_eq!(doc.labels.len(), 1);
        assert_eq!(doc.labels[0].name, "GUI:OK");
        assert_eq!(doc.labels[0].value.as_deref(), Some("确定"));
        assert!(doc.labels[0].extra.is_none());
        assert!(doc.find("GUI:OK").is_some());
        assert!(doc.find("GUI:不存在的键").is_none());
    }

    #[test]
    fn label_without_value_maps_to_none() {
        let data = csf_bytes(&[label_blob("GUI:None", None, None)]);
        let doc = CsfDocument::parse(&data).unwrap();
        assert_eq!(doc.labels[0].value, None);
        assert!(doc.labels[0].extra.is_none());
    }

    #[test]
    fn empty_value_label_maps_to_empty_string() {
        // 空值仍花 8 字节('STR ' 魔数 + i32 0),是 Some("") 而非 None
        let data = csf_bytes(&[label_blob("GUI:Blank", Some(""), None)]);
        let doc = CsfDocument::parse(&data).unwrap();
        assert_eq!(doc.labels[0].value.as_deref(), Some(""));
    }

    #[test]
    fn strw_label_carries_extra() {
        let data = csf_bytes(&[label_blob("VOX:ceva001", Some("值"), Some(b"ceva001c"))]);
        let doc = CsfDocument::parse(&data).unwrap();
        assert_eq!(doc.labels[0].value.as_deref(), Some("值"));
        assert_eq!(doc.labels[0].extra.as_deref(), Some(b"ceva001c".as_slice()));
    }

    #[test]
    fn rejects_bad_file_magic() {
        let mut data = csf_bytes(&[label_blob("A", Some("b"), None)]);
        data[0..4].copy_from_slice(&0u32.to_le_bytes());
        assert!(matches!(
            CsfDocument::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_bad_label_magic_midstream() {
        let mut data = csf_bytes(&[label_blob("A", Some("b"), None)]);
        let pos = 24 + 4; // 首标签的 " LBL" 位置
        data[pos..pos + 4].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        assert!(matches!(
            CsfDocument::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_trailing_bytes() {
        let mut data = csf_bytes(&[label_blob("A", Some("b"), None)]);
        data.push(0);
        assert!(matches!(
            CsfDocument::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }
}
