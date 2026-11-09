// SPDX-License-Identifier: GPL-3.0-only

//! artmd.ini → art.ini 分层索引(M4b spec §3;研究 §7)。
//!
//! YR 的 artmd 仅含覆盖差量,引擎分层合并——**按键**分层(artmd 先中
//! 即返,art 兜底),非按节整体。无节 → image=类型名、facings=调用方
//! 缺省(建筑/地形 1、步兵 8;objtype.cpp:121 GraphicName=IniName、
//! unittype.cpp:97 Facings(8) 同源缺省)。StandFrame 缺省 0(键在
//! 零售 art 是否存在未钉,L5,807 验收监督)。

use crate::format::ini::{parse_int_prefix, IniDocument};

/// 一个类型的渲染 art 参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtEntry {
    pub image: String,
    pub facings: u32,
    pub stand_frame: u32,
}

/// 双层 art 索引。
pub struct ArtIndex {
    md: IniDocument,
    base: IniDocument,
}

fn layered<'a>(
    md: &'a IniDocument,
    base: &'a IniDocument,
    sec: &str,
    key: &str,
) -> Option<&'a str> {
    md.value(sec, key).or_else(|| base.value(sec, key))
}

impl ArtIndex {
    pub fn parse(artmd: &str, art: &str) -> ArtIndex {
        ArtIndex {
            md: IniDocument::parse(artmd),
            base: IniDocument::parse(art),
        }
    }

    /// 类型 → art 参数;无节合成缺省(image=类型名)。facings/stand_frame
    /// 敌意值(0/负/超大)过滤回缺省。
    pub fn get_or_default(&self, type_name: &str, default_facings: u32) -> ArtEntry {
        let has_section =
            self.md.section(type_name).is_some() || self.base.section(type_name).is_some();
        let image = if has_section {
            layered(&self.md, &self.base, type_name, "Image")
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(type_name)
                .to_string()
        } else {
            type_name.to_string()
        };
        let facings = layered(&self.md, &self.base, type_name, "Facings")
            .and_then(parse_int_prefix)
            .filter(|f| (1..=1024).contains(f))
            .map_or(default_facings.max(1), |f| f as u32);
        let stand_frame = layered(&self.md, &self.base, type_name, "StandFrame")
            .and_then(parse_int_prefix)
            .filter(|s| (0..=1024).contains(s))
            .map_or(0, |s| s as u32);
        ArtEntry {
            image,
            facings,
            stand_frame,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MD: &str = "\
[GI]
Cameo=GIICON
Facings=32
";
    const BASE: &str = "\
[GI]
Image=E1
Facings=8
StandFrame=3
[TREE11]
Image=TREEXX
";

    #[test]
    fn per_key_layering_artmd_wins() {
        let a = ArtIndex::parse(MD, BASE);
        let gi = a.get_or_default("GI", 8);
        assert_eq!(gi.image, "E1"); // Image 仅 art 有 → 兜底命中
        assert_eq!(gi.facings, 32); // Facings artmd 优先
        assert_eq!(gi.stand_frame, 3);
    }

    #[test]
    fn no_section_synthesizes_default() {
        let a = ArtIndex::parse(MD, BASE);
        let e = a.get_or_default("CABHUT", 1);
        assert_eq!(e.image, "CABHUT"); // 无节 → 类型名(objtype.cpp:121)
        assert_eq!(e.facings, 1);
        assert_eq!(e.stand_frame, 0);
        let t = a.get_or_default("TREE11", 1);
        assert_eq!(t.image, "TREEXX"); // 有节无 Facings → default
        assert_eq!(t.facings, 1);
    }

    #[test]
    fn hostile_facings_filtered_to_default() {
        let md = "[X]\nFacings=0\n[Y]\nFacings=-4\n[Z]\nFacings=999999\n";
        let a = ArtIndex::parse(md, "");
        assert_eq!(a.get_or_default("X", 8).facings, 8);
        assert_eq!(a.get_or_default("Y", 8).facings, 8);
        assert_eq!(a.get_or_default("Z", 8).facings, 8);
        // StandFrame 敌意同滤
        let a2 = ArtIndex::parse("[W]\nStandFrame=-1\n", "");
        assert_eq!(a2.get_or_default("W", 8).stand_frame, 0);
    }

    #[test]
    fn both_empty_is_all_defaults() {
        let a = ArtIndex::parse("", "");
        let e = a.get_or_default("ANY", 8);
        assert_eq!(e.image, "ANY");
        assert_eq!(e.facings, 8);
    }
}
