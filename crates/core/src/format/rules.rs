// SPDX-License-Identifier: GPL-3.0-only

//! rulesmd.ini 的 overlay 侧解析(M4b spec §3/§5;研究 §2)。
//!
//! 引擎语义(ra2-decomp 只读佐证,语义重写):
//! - [OverlayTypes] 按文件序逐条 Find_Or_Make(rules.cpp:2068-2079),
//!   HeapID=追加位(overtype.cpp:97-98)→ **地图 OverlayPack 字节 = 段内
//!   条目位置(0 基),条目键数字不参与**。
//! - 每类型节 Image= 缺省=类型名(objtype.cpp:121/730-731);
//!   Tiberium=/Crate= 旗标(overtype.cpp Read_INI:215-217)。
//! - ore(IsTiberium)显示型 = 游程基 + (X·Y)%游程长(cell.cpp:1451;
//!   命中判据=字节∈[基,基+长) 与 overlay.cpp:599 同构;游程长=Variety
//!   的零售等价,Image= 整型选择器分支(tiberium.cpp:171+)不感知)。
//! - 桥类型名前缀 bridge(ci)系启发式锚定(cell.cpp:1417 变体表消费方);
//!   零售名核对在 807 验收(T8)。

use crate::format::ini::IniDocument;

/// 0xFF = OVERLAY_NONE(overlay.cpp:418-419)。
pub const OVERLAY_NONE: u8 = 0xFF;

/// 逐型条目(表位 → 图名/旗标)。
struct Entry {
    image: String,
    tiberium: bool,
    is_crate: bool,
    bridge: bool,
}

/// 渲染期只读视图(T4 消费)。
pub struct OverlayKind<'a> {
    pub image: &'a str,
    pub tiberium: bool,
    pub is_crate: bool,
    pub bridge: bool,
}

/// [OverlayTypes] 顺序表:地图字节 = 表位。
pub struct OverlayTable {
    entries: Vec<Entry>,
}

impl OverlayTable {
    /// rulesmd 文本 → 表;段缺失 = 空表(调用方全计 no_type)。
    pub fn parse(text: &str) -> OverlayTable {
        let doc = IniDocument::parse(text);
        let mut entries = Vec::new();
        if let Some(list) = doc.section("OverlayTypes") {
            for (_, name) in &list.entries {
                let name = name.trim();
                if name.is_empty() {
                    continue;
                }
                let image = doc
                    .value(name, "Image")
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or(name)
                    .to_string();
                entries.push(Entry {
                    image,
                    tiberium: doc.get_bool(name, "Tiberium", false),
                    is_crate: doc.get_bool(name, "Crate", false),
                    bridge: name.to_ascii_lowercase().starts_with("bridge"),
                });
            }
        }
        OverlayTable { entries }
    }

    /// 字节位 → 渲染视图;0xFF/越表 → None(调用方计 overlay_no_type)。
    pub fn kind(&self, byte: u8) -> Option<OverlayKind<'_>> {
        if byte == OVERLAY_NONE {
            return None;
        }
        let e = self.entries.get(byte as usize)?;
        Some(OverlayKind {
            image: e.image.as_str(),
            tiberium: e.tiberium,
            is_crate: e.is_crate,
            bridge: e.bridge,
        })
    }

    /// ore 游程变体(cell.cpp:1451 非坡支):tiberium 字节 → 游程基 +
    /// (x·y)%游程长;其余原样。游程 = 连续 tiberium 条目的极大段。
    pub fn display_byte(&self, byte: u8, x: u16, y: u16) -> u8 {
        if byte == OVERLAY_NONE {
            return byte;
        }
        if !self.entries.get(byte as usize).is_some_and(|e| e.tiberium) {
            return byte;
        }
        let mut base = byte as usize;
        while base > 0 && self.entries[base - 1].tiberium {
            base -= 1;
        }
        let mut len = 1usize;
        while base + len < self.entries.len() && self.entries[base + len].tiberium {
            len += 1;
        }
        let v = (u32::from(x) * u32::from(y)) % len as u32;
        (base as u32 + v).min(0xFE) as u8
    }

    /// 表长(表位上界;T4/T6 消费)。签名冻结,不配 is_empty(T1 brief)。
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULES: &str = "\
[OverlayTypes]
0=GASAND
1=WALL01
2=ORE01
3=ORE02
4=ORE03
5=GEM01
6=BRIDGE1
[ORE01]
Tiberium=yes
[ORE02]
Tiberium=yes
Image=OREXX
[ORE03]
Tiberium=yes
[GEM01]
Crate=yes
";

    #[test]
    fn byte_is_list_position_not_key() {
        // 键数字故意乱序:位置语义(0→GASAND,2→ORE01)
        let text = RULES.replace("0=GASAND", "7=GASAND");
        let t = OverlayTable::parse(&text);
        assert_eq!(t.kind(0).unwrap().image, "GASAND");
        assert_eq!(t.kind(1).unwrap().image, "WALL01");
        assert!(t.kind(7).is_none()); // 越表
        assert!(t.kind(0xFF).is_none()); // OVERLAY_NONE
        assert_eq!(t.len(), 7);
    }

    #[test]
    fn image_override_and_flags() {
        let t = OverlayTable::parse(RULES);
        assert_eq!(t.kind(3).unwrap().image, "OREXX"); // Image= 覆盖
        assert!(t.kind(3).unwrap().tiberium);
        assert!(t.kind(5).unwrap().is_crate); // GEM01 Crate=yes
        assert!(!t.kind(5).unwrap().tiberium);
        assert!(t.kind(6).unwrap().bridge); // 名前缀 bridge(ci)
    }

    #[test]
    fn tiberium_run_display_byte() {
        let t = OverlayTable::parse(RULES);
        // 游程 [2,5):基=2,长=3;变体 = 2 + (X·Y)%3
        assert_eq!(t.display_byte(2, 0, 0), 2);
        assert_eq!(t.display_byte(3, 1, 1), 2 + 1); // 1%3=1
        assert_eq!(t.display_byte(4, 2, 2), 2 + 1); // 4%3=1
        assert_eq!(t.display_byte(4, 1, 0), 2); // 0%3=0
                                                // 非 tiberium 原样;0xFF 原样
        assert_eq!(t.display_byte(1, 5, 7), 1);
        assert_eq!(t.display_byte(0xFF, 5, 7), 0xFF);
    }

    #[test]
    fn missing_section_is_empty_table() {
        let t = OverlayTable::parse("[Whatever]\n1=x\n");
        assert_eq!(t.len(), 0);
        assert!(t.kind(0).is_none());
    }
}
