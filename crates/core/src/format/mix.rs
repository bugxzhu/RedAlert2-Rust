// SPDX-License-Identifier: GPL-3.0-only

//! TS/RA2 时代 MIX 容器(明文索引)。

use super::bytes::ByteReader;
use super::crc::mix_name_id;
use super::FormatError;

/// MIX 索引项:ID、相对 body 的偏移、大小。
pub struct MixEntry {
    pub id: u32,
    pub offset: u32,
    pub size: u32,
}

/// TS/RA2 时代 MIX 容器(明文索引)。
///
/// 布局:`[u32 flags][u16 count][u32 body_size]` + count×12 字节索引项
/// `{id, offset, size}`,随后是文件体;`data_start = 10 + count*12`。
pub struct MixArchive {
    data: Vec<u8>,
    entries: Vec<MixEntry>,
    data_start: usize,
}

impl MixArchive {
    pub fn parse(data: Vec<u8>) -> Result<Self, FormatError> {
        let mut r = ByteReader::new(&data);
        let flags = r.read_u32_le()?;
        if flags != 0 {
            return Err(FormatError::Unsupported(format!(
                "加密 MIX 索引暂不支持 (flags=0x{flags:08X})"
            )));
        }
        let count = r.read_u16_le()? as usize;
        let _body_size = r.read_u32_le()?;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            entries.push(MixEntry {
                id: r.read_u32_le()?,
                offset: r.read_u32_le()?,
                size: r.read_u32_le()?,
            });
        }
        let data_start = r.position();
        for e in &entries {
            let end = e.offset as u64 + e.size as u64;
            if end > (data.len() - data_start) as u64 {
                return Err(FormatError::Invalid(format!(
                    "条目越界: id=0x{:08X} offset={} size={}",
                    e.id, e.offset, e.size
                )));
            }
        }
        Ok(MixArchive {
            data,
            entries,
            data_start,
        })
    }

    pub fn entries(&self) -> &[MixEntry] {
        &self.entries
    }

    pub fn find_id(&self, id: u32) -> Option<&MixEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn find(&self, name: &str) -> Option<&MixEntry> {
        self.find_id(mix_name_id(name))
    }

    pub fn read(&self, name: &str) -> Result<&[u8], FormatError> {
        let e = self
            .find(name)
            .ok_or_else(|| FormatError::Invalid(format!("MIX 中找不到文件: {name}")))?;
        let s = self.data_start + e.offset as usize;
        Ok(&self.data[s..s + e.size as usize])
    }

    pub fn read_by_id(&self, id: u32) -> Result<&[u8], FormatError> {
        let e = self
            .find_id(id)
            .ok_or_else(|| FormatError::Invalid(format!("MIX 中找不到 ID: 0x{id:08X}")))?;
        let s = self.data_start + e.offset as usize;
        Ok(&self.data[s..s + e.size as usize])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个含 2 个条目的最小合法 MIX:
    /// body = "HELLO"(5字节, 由名 HELLO 的 ID 索引) + "RA2!"
    fn sample_mix() -> Vec<u8> {
        let id_hello = mix_name_id("HELLO");
        let id_ra2 = mix_name_id("RA2");
        let body: Vec<u8> = b"HELLO".iter().chain(b"RA2!").cloned().collect();
        let mut v = Vec::new();
        v.extend_from_slice(&0u32.to_le_bytes()); // flags
        v.extend_from_slice(&2u16.to_le_bytes()); // count
        v.extend_from_slice(&(body.len() as u32).to_le_bytes()); // body_size
        for (id, off, size) in [(id_hello, 0u32, 5u32), (id_ra2, 5u32, 4u32)] {
            v.extend_from_slice(&id.to_le_bytes());
            v.extend_from_slice(&off.to_le_bytes());
            v.extend_from_slice(&size.to_le_bytes());
        }
        v.extend_from_slice(&body);
        v
    }

    #[test]
    fn parses_header_and_index() {
        let mix = MixArchive::parse(sample_mix()).unwrap();
        assert_eq!(mix.entries().len(), 2);
        assert_eq!(mix.entries()[0].id, mix_name_id("HELLO"));
        assert_eq!(mix.entries()[1].offset, 5);
    }

    #[test]
    fn finds_and_reads_by_name() {
        let mix = MixArchive::parse(sample_mix()).unwrap();
        assert_eq!(mix.read("hello").unwrap(), b"HELLO"); // 名字大小写不敏感
        assert_eq!(mix.read("RA2").unwrap(), b"RA2!");
        assert!(mix.find("不存在").is_none());
    }

    #[test]
    fn rejects_encrypted_index() {
        let mut data = sample_mix();
        data[0..4].copy_from_slice(&0x0001_0000u32.to_le_bytes());
        assert!(matches!(
            MixArchive::parse(data),
            Err(FormatError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_entry_out_of_range() {
        let mut data = sample_mix();
        // 把第 2 个条目 size 改成越界值(索引项1 的 size 位于 10+12+8)
        let pos = 10 + 12 + 8;
        data[pos..pos + 4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        assert!(matches!(
            MixArchive::parse(data),
            Err(FormatError::Invalid(_))
        ));
    }
}
