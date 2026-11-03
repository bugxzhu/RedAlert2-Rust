// SPDX-License-Identifier: GPL-3.0-only

//! Westwood 风格 INI 解析(rulesmd.ini / 地图 / spawn.ini 共用)。

pub struct IniSection {
    pub name: String,
    pub entries: Vec<(String, String)>,
}

pub struct IniDocument {
    pub sections: Vec<IniSection>,
}

impl IniDocument {
    pub fn parse(text: &str) -> IniDocument {
        let mut doc = IniDocument {
            sections: Vec::new(),
        };
        let mut current: Option<usize> = None;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                let name = name.trim().to_string();
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
                }
            }
            // 其余行(垃圾/无节键值)跳过
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
            .and_then(|v| v.trim().parse::<i64>().ok())
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
}
