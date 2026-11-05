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
/// 新格式布局:`[u32 flags][u16 count][u32 body_size]` + count×12 字节索引项
/// `{id, offset, size}`,随后是文件体;`data_start = 10 + count*12`,无条件。
/// 摘要位(flags 0x00010000)为工具侧咨询位:引擎(gamemd YR 1.001)行为经
/// REA 0x5B3C20 反汇编裁决——只读一次存 +0x10、此后从不读,构造器与全部
/// mix 子系统(0x5B3E90..0x5B6300)无任何 +20 跳过;语料 46 容器中 27 个
/// 置位,均为工具在 body_size 末尾预留 20 字节的形态(27 例全部满足
/// body_size+10+12·count+20 == 文件长),条目偏移一律相对索引尾。XCC 的
/// checksum 概念不进入本项目判定路径。
/// 旧格式(6 字节头:`count u16@0`、`size u32@2`,data_start=6+12·count,
/// TS1 时代):首个 dword 低 16 位 ≠ 0 判别;引擎支持但语料零样本,本里程碑
/// 不实现,显式拒绝防静默错解。
/// 0x00020000 = 加密索引(拒绝);其余位未知(拒绝)。
pub struct MixArchive {
    data: Vec<u8>,
    entries: Vec<MixEntry>,
    data_start: usize,
    digest_flag: bool,
}

impl MixArchive {
    pub fn parse(data: Vec<u8>) -> Result<Self, FormatError> {
        const FLAG_DIGEST: u32 = 0x0001_0000;
        const FLAG_ENCRYPTED: u32 = 0x0002_0000;
        let mut r = ByteReader::new(&data);
        let flags = r.read_u32_le()?;
        if flags & 0xFFFF != 0 {
            // 旧格式判别(引擎 0x5B3C20 同口径):首个 dword 低 16 位是旧 6 字节头
            // 的 count;新格式 flags 低 16 位恒为 0。判别先于 flags 位检查——旧
            // 格式没有 flags 字段。
            return Err(FormatError::Unsupported(format!(
                "旧格式 MIX(6 字节头)暂不支持 (首字=0x{:04X}, flags=0x{flags:08X})",
                flags & 0xFFFF
            )));
        }
        if flags & FLAG_ENCRYPTED != 0 {
            return Err(FormatError::Unsupported(format!(
                "加密 MIX 索引暂不支持 (flags=0x{flags:08X})"
            )));
        }
        if flags & !(FLAG_DIGEST | FLAG_ENCRYPTED) != 0 {
            return Err(FormatError::Unsupported(format!(
                "未知 MIX 标志位 (flags=0x{flags:08X})"
            )));
        }
        let digest_flag = flags & FLAG_DIGEST != 0;
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
        // 引擎从不因摘要位跳 20(REA 0x5B3C20 裁决):数据区一律紧接索引尾。
        // 置位容器的 body_size 含工具预留的 20 字节尾窗,条目偏移仍从索引尾
        // 起算,引擎从不定位摘要块。
        let data_start = r.position(); // = 10 + 12*count
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
            digest_flag,
        })
    }

    /// 摘要位(0x00010000)是否置位——工具侧咨询位,引擎只读一次存 +0x10、
    /// 此后从不消费(REA 0x5B3C20 裁决),数据区布局与此位无关。
    pub fn digest_flag(&self) -> bool {
        self.digest_flag
    }

    /// 数据区起始绝对偏移(装载器切片用)= 10 + 12·count,与摘要位无关。
    pub fn data_start(&self) -> usize {
        self.data_start
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

    /// 摘要位置位样本(flags=0x00010000),语料 27 个置位容器的形态:
    /// 工具在 body_size 末尾预留 20 字节垃圾(body_size = 体 + 20,且
    /// body_size+10+12·count == 文件长精确成立),条目偏移仍相对索引尾——
    /// 旧实现因摘要位跳 20,会把全部读取错位。
    fn sample_phantom_mix() -> Vec<u8> {
        let id_hello = mix_name_id("HELLO");
        let body: Vec<u8> = b"HELLO".to_vec();
        let mut v = Vec::new();
        v.extend_from_slice(&0x0001_0000u32.to_le_bytes()); // flags: 摘要位
        v.extend_from_slice(&1u16.to_le_bytes()); // count
        v.extend_from_slice(&((body.len() + 20) as u32).to_le_bytes()); // body_size 含预留
        v.extend_from_slice(&id_hello.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes()); // offset: 相对索引尾
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(&body);
        v.extend_from_slice(&[0xABu8; 20]); // 尾部预留窗(引擎从不消费)
        v
    }

    /// 旧格式(6 字节头)样本:`[u16 count][u32 size]` + 索引 + 体;
    /// 首个 dword 低 16 位 = count ≠ 0,即判别位。
    fn sample_old_format_mix() -> Vec<u8> {
        let id_hello = mix_name_id("HELLO");
        let body: Vec<u8> = b"HELLO".to_vec();
        let mut v = Vec::new();
        v.extend_from_slice(&1u16.to_le_bytes()); // count(旧格式判别位)
        v.extend_from_slice(&(body.len() as u32).to_le_bytes()); // size
        v.extend_from_slice(&id_hello.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(&body);
        v
    }

    #[test]
    fn digest_flag_does_not_skip_twenty() {
        // 修复点:旧实现因摘要位无条件跳 20,read("HELLO") 会读到 0xAB 垃圾
        // (语料 27 个置位容器正是此形态);引擎语义从不跳,data_start=10+12·count。
        let mix = MixArchive::parse(sample_phantom_mix()).unwrap();
        assert_eq!(mix.data_start(), 10 + 12);
        assert_eq!(mix.read("HELLO").unwrap(), b"HELLO");
        assert!(mix.digest_flag());
    }

    #[test]
    fn old_format_header_rejected() {
        // 旧格式(6 字节头,TS1 时代):引擎支持但语料零样本,显式拒绝防静默错解;
        // 断言报错指向旧格式守卫(而非误入未知 flags 位分支)。
        // MixArchive 未实现 Debug,无法 unwrap_err,拆臂报告。
        let err = match MixArchive::parse(sample_old_format_mix()) {
            Err(e) => e,
            Ok(_) => panic!("旧格式样本应被拒绝,实际解析成功"),
        };
        assert!(matches!(err, FormatError::Unsupported(_)));
        assert!(err.to_string().contains("旧格式"), "{err}");
    }

    #[test]
    fn plain_mix_reports_no_digest_flag() {
        let mix = MixArchive::parse(sample_mix()).unwrap();
        assert!(!mix.digest_flag());
        assert_eq!(mix.data_start(), 10 + 12 * 2);
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
        data[0..4].copy_from_slice(&0x0002_0000u32.to_le_bytes());
        assert!(matches!(
            MixArchive::parse(data),
            Err(FormatError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_digest_plus_encrypted() {
        // 加密位优先裁决:0x00030000 报 Unsupported(加密分支)
        let mut data = sample_mix();
        data[0..4].copy_from_slice(&0x0003_0000u32.to_le_bytes());
        assert!(matches!(
            MixArchive::parse(data),
            Err(FormatError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_unknown_flag_bits() {
        let mut data = sample_mix();
        data[0..4].copy_from_slice(&0x0004_0000u32.to_le_bytes());
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
