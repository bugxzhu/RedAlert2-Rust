// SPDX-License-Identifier: GPL-3.0-only

//! Westwood 风格 INI 解析(rulesmd.ini / 地图 / spawn.ini 共用)。

pub struct IniSection {
    pub name: String,
    pub entries: Vec<(String, String)>,
}

pub struct IniDocument {
    pub sections: Vec<IniSection>,
    /// 首个段之前被丢弃的 key=value 行数(RMG MapUnit 半图;普查 §8-4:
    /// 继续丢弃但计数上报)
    pub orphan_key_values: usize,
}

impl IniDocument {
    pub fn parse(text: &str) -> IniDocument {
        let mut doc = IniDocument {
            sections: Vec::new(),
            orphan_key_values: 0,
        };
        let mut current: Option<usize> = None;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') {
                continue;
            }
            // 段头:行首 '[' 且行内出现 ']'(取最后一个 ']'——普查 §3-5 的
            // 真实样例段头带尾随 tab/注释);空名跳过
            let header_name = if line.starts_with('[') {
                line.rfind(']')
                    .map(|end| line[1..end].trim().to_string())
                    .filter(|n| !n.is_empty())
            } else {
                None
            };
            if let Some(name) = header_name {
                match doc
                    .sections
                    .iter()
                    .position(|s| s.name.eq_ignore_ascii_case(&name))
                {
                    Some(i) => current = Some(i),
                    None => {
                        doc.sections.push(IniSection {
                            name,
                            entries: Vec::new(),
                        });
                        current = Some(doc.sections.len() - 1);
                    }
                }
            } else if let Some((k, v)) = line.split_once('=') {
                let key = k.trim();
                if key.is_empty() {
                    continue;
                }
                if let Some(i) = current {
                    let entries = &mut doc.sections[i].entries;
                    match entries
                        .iter_mut()
                        .find(|(ek, _)| ek.eq_ignore_ascii_case(key))
                    {
                        Some(slot) => slot.1 = v.trim().to_string(),
                        None => entries.push((key.to_string(), v.trim().to_string())),
                    }
                } else {
                    doc.orphan_key_values += 1;
                }
            }
            // 其余行(垃圾)跳过
        }
        doc
    }

    pub fn section(&self, name: &str) -> Option<&IniSection> {
        self.sections
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    pub fn value(&self, section: &str, key: &str) -> Option<&str> {
        self.section(section)?.value(key)
    }

    pub fn get_int(&self, section: &str, key: &str, default: i64) -> i64 {
        self.value(section, key)
            .and_then(parse_int_prefix)
            .unwrap_or(default)
    }

    pub fn get_bool(&self, section: &str, key: &str, default: bool) -> bool {
        match self
            .value(section, key)
            .map(|v| v.trim().to_ascii_lowercase())
        {
            Some(v) => matches!(v.as_str(), "yes" | "true" | "1"),
            None => default,
        }
    }
}

/// 原版 atoi 风格前缀解析(普查 §8-2):跳过前导空白,可选 +/-,十进制数字
/// 前缀;无数字返回 None;超出 i64 饱和(checked 算术,debug 下零 panic)。
/// pub 供 map/meta.rs 的 CSV 字段解析复用。
pub fn parse_int_prefix(v: &str) -> Option<i64> {
    let t = v.trim_start();
    let b = t.as_bytes();
    let mut i = 0usize;
    let neg = match b.first() {
        Some(b'-') => {
            i = 1;
            true
        }
        Some(b'+') => {
            i = 1;
            false
        }
        _ => false,
    };
    let mut acc: i128 = 0;
    let mut digits = 0usize;
    for &c in &b[i..] {
        if !c.is_ascii_digit() {
            break;
        }
        match acc
            .checked_mul(10)
            .and_then(|a| a.checked_add((c - b'0') as i128))
        {
            Some(a) => acc = a,
            None => return Some(if neg { i64::MIN } else { i64::MAX }),
        }
        digits += 1;
    }
    if digits == 0 {
        return None;
    }
    let signed = if neg { -acc } else { acc };
    if signed > i64::MAX as i128 {
        Some(i64::MAX)
    } else if signed < i64::MIN as i128 {
        Some(i64::MIN)
    } else {
        Some(signed as i64)
    }
}

impl IniSection {
    pub fn value(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
; 顶层注释
[General]
BuildSpeed=0.7
GameSpeedBufferFrames = 0  ; 行尾分号保留在值里
YesKey=yes

[general]
BuildSpeed=1.5

[Units]
List=HTNK,APOC
";

    #[test]
    fn sections_case_insensitive_and_merged() {
        let doc = IniDocument::parse(SAMPLE);
        // [General] 与 [general] 合并,同名键后者覆盖
        assert_eq!(doc.value("GENERAL", "BuildSpeed"), Some("1.5"));
        assert_eq!(doc.sections.len(), 2); // General + Units
    }

    #[test]
    fn values_trimmed_and_comment_lines_skipped() {
        let doc = IniDocument::parse(SAMPLE);
        assert_eq!(
            doc.value("General", "GameSpeedBufferFrames"),
            Some("0  ; 行尾分号保留在值里")
        );
        assert_eq!(doc.value("General", "YesKey"), Some("yes"));
        assert_eq!(doc.value("Units", "List"), Some("HTNK,APOC"));
        assert_eq!(doc.value("General", "不存在"), None);
    }

    #[test]
    fn typed_accessors() {
        let doc = IniDocument::parse(SAMPLE);
        assert_eq!(doc.get_int("Units", "List", -1), -1); // 非整数 -> 默认
        assert!(doc.get_bool("General", "YesKey", false));
        // 非布尔字符串按 Westwood 语义视为 no(不回落默认值)
        assert!(!doc.get_bool("General", "BuildSpeed", true));
        assert!(doc.get_bool("General", "键不存在", true)); // 键缺失 -> 默认
    }

    #[test]
    fn garbage_lines_are_skipped() {
        let doc = IniDocument::parse("垃圾行\n[OK]\nK=V\n=无键\n[未闭合\n");
        assert_eq!(doc.value("OK", "K"), Some("V"));
        assert_eq!(doc.sections.len(), 1);
    }

    #[test]
    fn headers_accept_trailing_comments_and_whitespace() {
        // 普查 §3-5 真实样例:Maps\DDLY\全新油桶一块地.map:156。旧行为会把
        // 8 个键值错挂到前一个段 [NAFLAK] 上
        let doc = IniDocument::parse(
            "[NAFLAK]\nDamage=55\n[FlakWeaponE6]\t\t; This belongs to Flak Cannon\nROF=15\n",
        );
        assert_eq!(doc.value("FlakWeaponE6", "ROF"), Some("15"));
        assert_eq!(doc.value("NAFLAK", "Damage"), Some("55"));
        assert_eq!(doc.value("NAFLAK", "ROF"), None);
        // 行尾空白变体
        let doc = IniDocument::parse("[SomeSection]   \nK=V\n");
        assert_eq!(doc.value("SomeSection", "K"), Some("V"));
        // 未闭合的 `[` 仍按垃圾行跳过(既有行为回归)
        let doc = IniDocument::parse("[未闭合\nK=V\n");
        assert_eq!(doc.sections.len(), 0);
    }

    #[test]
    fn get_int_parses_atoi_prefix() {
        // 普查 §3-10 的游戏内证据:Soylent=900;2000 读作 900
        let doc = IniDocument::parse(
            "[S]\nA=900;2000\nB=  42ab\nC=-13x\nD=+7\nE=12.5\nF=abc\nG=\nH=0  ; 注释\n",
        );
        assert_eq!(doc.get_int("S", "A", -1), 900);
        assert_eq!(doc.get_int("S", "B", -1), 42);
        assert_eq!(doc.get_int("S", "C", -1), -13);
        assert_eq!(doc.get_int("S", "D", -1), 7);
        assert_eq!(doc.get_int("S", "E", -1), 12); // atoi 在 '.' 截断
        assert_eq!(doc.get_int("S", "F", -1), -1); // 无数字 → default
        assert_eq!(doc.get_int("S", "G", -1), -1); // 空值 → default
        assert_eq!(doc.get_int("S", "H", -1), 0);
    }

    #[test]
    fn get_int_huge_number_saturates() {
        // 敌意长数字:饱和不 panic(debug 溢出检查下也不得 panic)
        let doc = IniDocument::parse(
            "[S]\nA=99999999999999999999999999\nB=-99999999999999999999999999\n",
        );
        assert_eq!(doc.get_int("S", "A", 0), i64::MAX);
        assert_eq!(doc.get_int("S", "B", 0), i64::MIN);
    }

    #[test]
    fn orphan_lines_before_first_section_are_counted() {
        // 普查 §3-4:12 张 RMG NEWURBAN 半图以孤儿键值行开头(如 1,1,1,1,01,spawn.map
        // 首段前 44 行)——继续丢弃但计数上报
        let doc = IniDocument::parse("Size=0,0,60,30\n1=AAAA\n2=BBBB\n[Map]\nTheater=SNOW\n");
        assert_eq!(doc.orphan_key_values, 3);
        assert_eq!(doc.sections.len(), 1);
    }

    #[test]
    fn entries_preserve_file_order() {
        // 普查 §8-5 载荷不变量锁定:entries 必须保持文件顺序(包段重组按 key 序
        // 重排,对象段按下标枚举——绝不允许换 HashMap/排序)
        let doc = IniDocument::parse("[IsoMapPack5]\n3=C\n1=A\n2=B\n");
        let sec = doc.section("IsoMapPack5").unwrap();
        let keys: Vec<&str> = sec.entries.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["3", "1", "2"]);
    }

    #[test]
    fn duplicate_key_last_wins_within_section() {
        // 普查 §8-6:语料 0 重复键,但行为必须冻结(与游戏一致)
        let doc = IniDocument::parse("[S]\nA=1\nA=2\n");
        assert_eq!(doc.value("S", "A"), Some("2"));
        assert_eq!(doc.section("S").unwrap().entries.len(), 1);
    }

    #[test]
    fn empty_brackets_section_is_skipped() {
        // 旧行为会创建空名段;新判定(空名跳过)更严,语料零 `[]`,行为冻结
        let doc = IniDocument::parse("[]\nA=1\n[Real]\nB=2\n");
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(doc.value("Real", "B"), Some("2"));
    }
}
