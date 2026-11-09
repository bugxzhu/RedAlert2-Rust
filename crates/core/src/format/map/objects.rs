// SPDX-License-Identifier: GPL-3.0-only

//! 对象段结构化:[Structures] / [Units] / [Infantry] / [Aircraft] /
//! [Terrain] / [Smudge] → 类型化对象列表。
//!
//! 布局(普查 2026-11-04-map-ini-survey.md §4.7/§4.8;字段数精确且恒定):
//! - [Structures] 17 字段:`owner,type,strength,X,Y,facing,tag,7 个标志位,
//!   None×3,末两位`——X/Y(field 3/4)即菱形坐标
//! - [Units] / [Infantry] 14 字段:同族布局,field 5 在 Units 是 facing、
//!   在 Infantry 是 sub_cell(0..=5);field 6 跨段过载(mission 词/None/
//!   hex8 tag),按段各有含义,不做共享布局
//! - [Aircraft] 12 字段,同族
//! - [Terrain]:key = 菱形坐标键 Y·1000+X,value = 单 token(类型名)
//! - [Smudge]:key = 顺序编号,value = `name,X,Y,0`(坐标在字段里)
//!
//! 键形态:编号 0..n-1 或大 ID(1025970…)——**绝不用 key 当下标,逐项枚举**。
//! 真图带真实损坏(普查:勇士与巨龙.map 第 6187 行的 16 字段 Structures 行,
//! owner+type 丢逗号)——字段数与段布局不符时保留原始字段、标 malformed、
//! 计数上报,不中断解析。

use crate::format::ini::IniDocument;

/// 对象段类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectClass {
    Structure,
    Unit,
    Infantry,
    Aircraft,
    Terrain,
    Smudge,
}

/// 一个对象:原始键 + 原始 CSV 字段(trim 后)+ 宽容解析的派生访问器。
#[derive(Debug, Clone)]
pub struct MapObject {
    pub class: ObjectClass,
    /// 段内原始键(编号键或坐标键的十进制文本)。
    pub key: String,
    /// 原始 CSV 字段(trim 后;Terrain 为单元素)。
    pub fields: Vec<String>,
    /// 字段数与段布局不符——对象保留,计入 MapObjects::malformed。
    pub malformed: bool,
}

impl MapObject {
    fn field(&self, i: usize) -> Option<&str> {
        self.fields.get(i).map(String::as_str)
    }

    /// 所属方(编号键段 field 0;Terrain/Smudge 无)。
    pub fn owner(&self) -> Option<&str> {
        match self.class {
            ObjectClass::Terrain | ObjectClass::Smudge => None,
            _ => self.field(0),
        }
    }

    /// 类型名(编号键段 field 1;Terrain/Smudge 为 field 0)。
    pub fn kind(&self) -> Option<&str> {
        match self.class {
            ObjectClass::Terrain | ObjectClass::Smudge => self.field(0),
            _ => self.field(1),
        }
    }

    /// 菱形坐标 (x,y):编号键段取 field 3/4;Terrain 由键 Y·1000+X 解出;
    /// Smudge 取 field 1/2。任一分量非整数 → None(宽容,零 panic)。
    pub fn cell(&self) -> Option<(u32, u32)> {
        match self.class {
            ObjectClass::Terrain => {
                let n: u32 = self.key.parse().ok()?;
                Some((n % 1000, n / 1000))
            }
            ObjectClass::Smudge => Some((
                self.fields.get(1)?.trim().parse().ok()?,
                self.fields.get(2)?.trim().parse().ok()?,
            )),
            _ => Some((
                self.fields.get(3)?.trim().parse().ok()?,
                self.fields.get(4)?.trim().parse().ok()?,
            )),
        }
    }

    /// 朝向(编号键段 field 5;Infantry 该位是 sub_cell → None)。
    pub fn facing(&self) -> Option<u32> {
        match self.class {
            ObjectClass::Infantry | ObjectClass::Terrain | ObjectClass::Smudge => None,
            _ => self.field(5)?.trim().parse().ok(),
        }
    }

    /// 渲染朝向(M4b):Structure/Unit/Aircraft = field 5;Infantry 的
    /// field 5 是 sub_cell,朝向在 field 7(真图实证 2026-11-06,研究 §5);
    /// Terrain/Smudge 无。非整数或 >255 → None(宽容)。
    pub fn render_facing(&self) -> Option<u32> {
        let idx = match self.class {
            ObjectClass::Infantry => 7,
            ObjectClass::Structure | ObjectClass::Unit | ObjectClass::Aircraft => 5,
            ObjectClass::Terrain | ObjectClass::Smudge => return None,
        };
        self.field(idx)?.trim().parse().ok().filter(|&f| f <= 255)
    }

    /// 步兵 sub_cell(仅 Infantry field 5,合法域 0..=5,越域 → None)。
    pub fn sub_cell(&self) -> Option<u8> {
        match self.class {
            ObjectClass::Infantry => self.field(5)?.trim().parse().ok().filter(|&s| s <= 5),
            _ => None,
        }
    }
}

/// 全部对象 + 损坏行计数。
pub struct MapObjects {
    pub all: Vec<MapObject>,
    pub malformed: usize,
}

impl MapObjects {
    /// 某类对象数(CLI info 的分档计数)。
    pub fn count(&self, class: ObjectClass) -> usize {
        self.all.iter().filter(|o| o.class == class).count()
    }
}

/// (段名, 类别, 期望字段数)。
const SECTION_LAYOUTS: [(&str, ObjectClass, usize); 6] = [
    ("Structures", ObjectClass::Structure, 17),
    ("Units", ObjectClass::Unit, 14),
    ("Infantry", ObjectClass::Infantry, 14),
    ("Aircraft", ObjectClass::Aircraft, 12),
    ("Terrain", ObjectClass::Terrain, 1),
    ("Smudge", ObjectClass::Smudge, 4),
];

/// 从文档抽取六段全部对象(段缺失 = 空档;枚举顺序 = 文件序)。
pub fn parse_objects(doc: &IniDocument) -> MapObjects {
    let mut all = Vec::new();
    let mut malformed = 0usize;
    for (section, class, expected) in SECTION_LAYOUTS {
        let Some(sec) = doc.section(section) else {
            continue;
        };
        for (key, value) in &sec.entries {
            let fields: Vec<String> = value
                .split(',')
                .map(str::trim)
                .map(str::to_string)
                .collect();
            let is_malformed = fields.len() != expected;
            if is_malformed {
                malformed += 1;
            }
            all.push(MapObject {
                class,
                key: key.clone(),
                fields,
                malformed: is_malformed,
            });
        }
    }
    MapObjects { all, malformed }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(section: &str, key: &str, value: &str) -> MapObject {
        let doc = IniDocument::parse(&format!("[{section}]\n{key}={value}\n"));
        let objs = parse_objects(&doc);
        assert_eq!(objs.all.len(), 1);
        objs.all[0].clone()
    }

    #[test]
    fn structures_17_fields_real_row() {
        // 普查 §4.7 的真实样例行
        let o = one(
            "Structures",
            "0",
            "Neutral,CABHUT,256,103,64,0,None,0,0,1,0,0,None,None,None,0,0",
        );
        assert!(!o.malformed);
        assert_eq!(o.owner(), Some("Neutral"));
        assert_eq!(o.kind(), Some("CABHUT"));
        assert_eq!(o.cell(), Some((103, 64)));
        assert_eq!(o.facing(), Some(0));
        assert_eq!(o.sub_cell(), None);
    }

    #[test]
    fn units_14_fields_real_row_with_negative_flags() {
        // 普查 §4.7:负数字段(-1)必须宽容存活
        let o = one(
            "Units",
            "0",
            "Neutral,CAR,256,66,104,0,Sleep,None,0,-1,0,-1,1,1",
        );
        assert!(!o.malformed);
        assert_eq!(o.kind(), Some("CAR"));
        assert_eq!(o.cell(), Some((66, 104)));
        assert_eq!(o.facing(), Some(0));
        assert_eq!(o.fields.len(), 14);
    }

    #[test]
    fn infantry_subcell_real_row() {
        let o = one(
            "Infantry",
            "0",
            "Neutral,COW,256,82,81,3,Guard,125,None,0,-1,0,1,1",
        );
        assert!(!o.malformed);
        assert_eq!(o.cell(), Some((82, 81)));
        assert_eq!(o.sub_cell(), Some(3));
        assert_eq!(o.facing(), None); // Infantry field 5 是 sub_cell
    }

    #[test]
    fn aircraft_12_fields_real_row() {
        let o = one(
            "Aircraft",
            "0",
            "Neutral,PDPLANE,256,108,168,64,Guard,None,0,-1,1,1",
        );
        assert!(!o.malformed);
        assert_eq!(o.cell(), Some((108, 168)));
        assert_eq!(o.facing(), Some(64));
    }

    #[test]
    fn terrain_coordinate_key_decodes() {
        // 普查 §4.8:key = Y·1000+X,value = 单 token
        let o = one("Terrain", "101043", "TIBTRE01");
        assert!(!o.malformed);
        assert_eq!(o.cell(), Some((43, 101)));
        assert_eq!(o.kind(), Some("TIBTRE01"));
        assert_eq!(o.owner(), None);
        // 非整数键 → cell None,不 panic(负值是 [Waypoints] 的事,但宽容至上)
        let o = one("Terrain", "junk", "TIBTRE01");
        assert_eq!(o.cell(), None);
    }

    #[test]
    fn smudge_coordinate_in_fields() {
        // 普查 §4.8:4 字段 name,X,Y,0
        let o = one("Smudge", "0", "CRATER07,92,153,0");
        assert!(!o.malformed);
        assert_eq!(o.cell(), Some((92, 153)));
        assert_eq!(o.kind(), Some("CRATER07"));
    }

    #[test]
    fn corrupt_16_field_row_is_malformed_and_kept() {
        // 规格依据 §9:勇士与巨龙.map 第 6187 行(16 字段,owner+type 丢逗号)
        let doc = IniDocument::parse(
            "[Structures]\n69=NeutralGAWEAT,256,138,264,0,None,1,0,1,0,0,None,None,None,1,0\n",
        );
        let objs = parse_objects(&doc);
        assert_eq!(objs.all.len(), 1);
        assert_eq!(objs.malformed, 1);
        let o = &objs.all[0];
        assert!(o.malformed);
        assert_eq!(o.fields.len(), 16);
        // owner+type 并成一词 → 后续字段左移一位:field 3/4 实为 Y/朝向,
        // 逐位读取得 (264,0) 而非真坐标 (138,264)——这正是 malformed
        // 标记存在的意义:损坏行的位置访问器不可信,调用方须以标记为准
        assert_eq!(o.cell(), Some((264, 0)));
    }

    #[test]
    fn counts_rollup_per_class() {
        let doc = IniDocument::parse(
            "[Structures]\n0=Neutral,CABHUT,256,1,1,0,None,0,0,1,0,0,None,None,None,0,0\n\
             [Units]\n0=Neutral,CAR,256,2,2,0,Sleep,None,0,-1,0,-1,1,1\n\
             [Terrain]\n1002=TIBTRE01\n",
        );
        let objs = parse_objects(&doc);
        assert_eq!(objs.count(ObjectClass::Structure), 1);
        assert_eq!(objs.count(ObjectClass::Unit), 1);
        assert_eq!(objs.count(ObjectClass::Infantry), 0);
        assert_eq!(objs.count(ObjectClass::Aircraft), 0);
        assert_eq!(objs.count(ObjectClass::Terrain), 1);
        assert_eq!(objs.count(ObjectClass::Smudge), 0);
        assert_eq!(objs.malformed, 0);
    }

    #[test]
    fn render_facing_infantry_field7_real_rows() {
        // austintx COW 行:sub=4/facing=32 与 sub=2/facing=64(研究 §5 实证)
        let a = one(
            "Infantry",
            "0",
            "Neutral,COW,256,128,42,4,Guard,32,None,0,-1,0,1,1",
        );
        assert_eq!(a.render_facing(), Some(32));
        let b = one(
            "Infantry",
            "1",
            "Neutral,COW,256,126,52,2,Guard,64,None,0,-1,0,1,1",
        );
        assert_eq!(b.render_facing(), Some(64));
        assert_eq!(b.facing(), None); // M2 冻结语义:Infantry field 5=sub_cell
    }

    #[test]
    fn render_facing_other_classes_and_hostile() {
        let s = one(
            "Structures",
            "0",
            "Neutral,CABHUT,256,103,64,192,None,0,0,1,0,0,None,None,None,0,0",
        );
        assert_eq!(s.render_facing(), Some(192));
        let u = one(
            "Units",
            "0",
            "Neutral,CAR,256,66,104,64,Sleep,None,0,-1,0,-1,1,1",
        );
        assert_eq!(u.render_facing(), Some(64));
        // 敌意:field 7 非整数 / >255 → None(不 panic)
        let bad = one(
            "Infantry",
            "0",
            "Neutral,COW,256,1,2,3,Guard,zzz,None,0,-1,0,1,1",
        );
        assert_eq!(bad.render_facing(), None);
        let over = one(
            "Infantry",
            "0",
            "Neutral,COW,256,1,2,3,Guard,300,None,0,-1,0,1,1",
        );
        assert_eq!(over.render_facing(), None);
        let t = one("Terrain", "1002", "TIBTRE01");
        assert_eq!(t.render_facing(), None);
    }

    #[test]
    fn big_id_keys_are_enumerated_not_indexed() {
        // 普查 §4.7:183 张 MadHQ 图的键是排序大 ID——绝不当数组下标
        let doc = IniDocument::parse(
            "[Structures]\n1025970=Neutral,CABHUT,256,1,1,0,None,0,0,1,0,0,None,None,None,0,0\n\
             1025971=Neutral,CABHUT,256,2,2,0,None,0,0,1,0,0,None,None,None,0,0\n",
        );
        let objs = parse_objects(&doc);
        assert_eq!(objs.all.len(), 2);
        assert_eq!(objs.all[0].key, "1025970");
        assert_eq!(objs.all[1].key, "1025971");
        assert_eq!(objs.all[1].cell(), Some((2, 2)));
    }
}
