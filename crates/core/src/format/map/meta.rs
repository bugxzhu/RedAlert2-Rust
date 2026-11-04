// SPDX-License-Identifier: GPL-3.0-only

//! 元数据段结构化读取([Basic] / [Map] / [Header] / [Lighting] / [Digest])。
//!
//! 普查 §4:四段 807/807 全在;[Map]Size 恒 4 int 且前两个恒 0,0(数值用
//! i64 原样保留——RMG indicator 的负 LocalSize 不拒绝);Theater ∈ 六值
//! (研究 §7.1 gamemd.exe 偏移 4070264-4070930 剧场表,精确大写为主、
//! 大小写不敏感兜底);[Lighting] 14 个六位小数 float;[Digest] = 28 字符
//! 标准 base64 → 20 字节,只读取不校验。[Header] 的 StartX/Waypoint1..8
//! 语义未知(研究 §9),按 get_int 原样透传不解释。

use crate::format::ini::IniDocument;
use crate::format::map::pack::base64_decode;
use crate::format::FormatError;

/// 剧场(gamemd.exe 剧场表六行;INI 名精确大写为主)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theater {
    Temperate,
    Snow,
    Urban,
    Desert,
    NewUrban,
    Lunar,
}

impl Theater {
    pub fn parse(s: &str) -> Result<Self, FormatError> {
        match s.to_ascii_uppercase().as_str() {
            "TEMPERATE" => Ok(Theater::Temperate),
            "SNOW" => Ok(Theater::Snow),
            "URBAN" => Ok(Theater::Urban),
            "DESERT" => Ok(Theater::Desert),
            "NEWURBAN" => Ok(Theater::NewUrban),
            "LUNAR" => Ok(Theater::Lunar),
            _ => Err(FormatError::Unsupported(format!(
                "未知剧场 {s}(剧场表六值之外)"
            ))),
        }
    }

    /// 剧场 ISO 调色板文件名(资源链:ra2.mix → id 0x3B5A96DE 嵌套库,
    /// 研究 §7.2;DESERT/NEWURBAN/LUNAR 的 YR 调色板在本机安装缺席——
    /// 调用方读不到时报错退出)。
    pub fn iso_palette_file(&self) -> &'static str {
        match self {
            Theater::Temperate => "isotem.pal",
            Theater::Snow => "isosno.pal",
            Theater::Urban => "isourb.pal",
            Theater::Desert => "isodes.pal",
            Theater::NewUrban => "isoubn.pal",
            Theater::Lunar => "isolun.pal",
        }
    }
}

/// [Basic] 的结构化子集(CLI info 用;其余键留在 IniDocument 不重复存储)。
pub struct BasicInfo {
    pub name: Option<String>,
    pub author: Option<String>,
    pub next_scenario: Option<String>,
    pub official: bool,
    pub multiplayer_only: bool,
    pub max_player: i64,
    pub min_player: i64,
}

impl BasicInfo {
    /// 无 [Basic] 段时返回 None(fcrash_backup 形态;普查 807/807 有段,
    /// 敌意/极端文件容错)。
    pub fn from_doc(doc: &IniDocument) -> Option<Self> {
        doc.section("Basic")?;
        Some(BasicInfo {
            name: doc.value("Basic", "Name").map(str::to_string),
            author: doc.value("Basic", "Author").map(str::to_string),
            next_scenario: doc.value("Basic", "NextScenario").map(str::to_string),
            official: doc.get_bool("Basic", "Official", false),
            multiplayer_only: doc.get_bool("Basic", "MultiplayerOnly", false),
            max_player: doc.get_int("Basic", "MaxPlayer", 0),
            min_player: doc.get_int("Basic", "MinPlayer", 0),
        })
    }
}

/// [Map] 段(Theater + Size + LocalSize;i64 原样保留不设界)。
pub struct MapMeta {
    pub theater: Theater,
    pub size: [i64; 4],
    pub local_size: [i64; 4],
}

fn parse_csv4(s: &str, what: &str) -> Result<[i64; 4], FormatError> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return Err(FormatError::Invalid(format!(
            "[Map]{what} 应为 4 个整数,实得 {} 项",
            parts.len()
        )));
    }
    let mut out = [0i64; 4];
    for (slot, part) in out.iter_mut().zip(&parts) {
        // atoi 前缀语义与 get_int 一致(Task 2 的 pub parse_int_prefix)
        *slot = crate::format::ini::parse_int_prefix(part).unwrap_or(0);
    }
    Ok(out)
}

impl MapMeta {
    pub fn from_doc(doc: &IniDocument) -> Result<Self, FormatError> {
        let theater = Theater::parse(
            doc.value("Map", "Theater")
                .ok_or_else(|| FormatError::Invalid("缺少 [Map]Theater".to_string()))?,
        )?;
        let size = parse_csv4(
            doc.value("Map", "Size")
                .ok_or_else(|| FormatError::Invalid("缺少 [Map]Size".to_string()))?,
            "Size",
        )?;
        let local_size = parse_csv4(
            doc.value("Map", "LocalSize").unwrap_or("0,0,0,0"),
            "LocalSize",
        )?;
        Ok(MapMeta {
            theater,
            size,
            local_size,
        })
    }
}

/// [Header] 段(语义未知字段按 get_int 原样透传;研究 §9 UNKNOWN)。
pub struct HeaderMeta {
    pub start_x: i64,
    pub start_y: i64,
    pub width: i64,
    pub height: i64,
    pub number_starting_points: i64,
}

impl HeaderMeta {
    pub fn from_doc(doc: &IniDocument) -> Self {
        HeaderMeta {
            start_x: doc.get_int("Header", "StartX", -1),
            start_y: doc.get_int("Header", "StartY", -1),
            width: doc.get_int("Header", "Width", -1),
            height: doc.get_int("Header", "Height", -1),
            number_starting_points: doc.get_int("Header", "NumberStartingPoints", 0),
        }
    }
}

/// [Lighting] 的 14 个 float 键(缺失/非数 → None;std f32 解析,零依赖)。
pub struct Lighting {
    pub ambient: Option<f32>,
    pub red: Option<f32>,
    pub green: Option<f32>,
    pub blue: Option<f32>,
    pub ground: Option<f32>,
    pub level: Option<f32>,
    pub ion_ambient: Option<f32>,
    pub ion_red: Option<f32>,
    pub ion_green: Option<f32>,
    pub ion_blue: Option<f32>,
    pub ion_ground: Option<f32>,
    pub dominator_ambient: Option<f32>,
    pub dominator_red: Option<f32>,
    pub dominator_green: Option<f32>,
}

impl Lighting {
    fn f(doc: &IniDocument, key: &str) -> Option<f32> {
        doc.value("Lighting", key)
            .and_then(|v| v.trim().parse::<f32>().ok())
    }

    pub fn from_doc(doc: &IniDocument) -> Self {
        Lighting {
            ambient: Self::f(doc, "Ambient"),
            red: Self::f(doc, "Red"),
            green: Self::f(doc, "Green"),
            blue: Self::f(doc, "Blue"),
            ground: Self::f(doc, "Ground"),
            level: Self::f(doc, "Level"),
            ion_ambient: Self::f(doc, "IonAmbient"),
            ion_red: Self::f(doc, "IonRed"),
            ion_green: Self::f(doc, "IonGreen"),
            ion_blue: Self::f(doc, "IonBlue"),
            ion_ground: Self::f(doc, "IonGround"),
            dominator_ambient: Self::f(doc, "DominatorAmbient"),
            dominator_red: Self::f(doc, "DominatorRed"),
            dominator_green: Self::f(doc, "DominatorGreen"),
        }
    }
}

/// [Digest] 的 20 字节(单键 `1=`,28 字符标准 base64;缺失/畸形 → None,
/// 只读取不校验)。
pub fn parse_digest(doc: &IniDocument) -> Option<[u8; 20]> {
    let v = doc.value("Digest", "1")?;
    base64_decode(v).ok()?.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> IniDocument {
        IniDocument::parse(text)
    }

    #[test]
    fn theater_parses_six_values_case_insensitively() {
        for (s, want) in [
            ("TEMPERATE", Theater::Temperate),
            ("snow", Theater::Snow),
            ("URBAN", Theater::Urban),
            ("Desert", Theater::Desert),
            ("NEWURBAN", Theater::NewUrban),
            ("Lunar", Theater::Lunar),
        ] {
            assert_eq!(Theater::parse(s).unwrap(), want);
        }
        assert!(matches!(
            Theater::parse("BOGUS"),
            Err(FormatError::Unsupported(_))
        ));
        // 调色板文件名(研究 §7.1)
        assert_eq!(Theater::Snow.iso_palette_file(), "isosno.pal");
        assert_eq!(Theater::Temperate.iso_palette_file(), "isotem.pal");
    }

    #[test]
    fn basic_from_ini() {
        let d = doc(
            "[Basic]\nName=海嘯\nAuthor=Handama\nNextScenario=GDI2A.map\nOfficial=yes\nMultiplayerOnly=1\nMaxPlayer=6\nMinPlayer=2\n",
        );
        let b = BasicInfo::from_doc(&d).unwrap();
        assert_eq!(b.name.as_deref(), Some("海嘯"));
        assert_eq!(b.author.as_deref(), Some("Handama"));
        assert_eq!(b.next_scenario.as_deref(), Some("GDI2A.map"));
        assert!(b.official);
        assert!(b.multiplayer_only); // MultiplayerOnly=1 亦为真(get_bool 语义)
        assert_eq!((b.max_player, b.min_player), (6, 2));
        // 无 [Basic] 段 → None(fcrash_backup 形态)
        assert!(BasicInfo::from_doc(&doc("[Map]\nTheater=SNOW\n")).is_none());
    }

    #[test]
    fn map_meta_requires_section_and_validates() {
        // [Map] 缺失 → Invalid(807/807 全有,敌意才缺)
        assert!(matches!(
            MapMeta::from_doc(&doc("[Basic]\nName=X\n")),
            Err(FormatError::Invalid(_))
        ));
        // 剧场未知 → Unsupported
        assert!(matches!(
            MapMeta::from_doc(&doc("[Map]\nTheater=BOGUS\nSize=0,0,10,10\n")),
            Err(FormatError::Unsupported(_))
        ));
        // 正常 + 负值原样(indicator.map 的负 LocalSize)
        let m = MapMeta::from_doc(&doc(
            "[Map]\nTheater=DESERT\nSize=0,0,1,1\nLocalSize=3,5,-5,-10\n",
        ))
        .unwrap();
        assert_eq!(m.theater, Theater::Desert);
        assert_eq!(m.size, [0, 0, 1, 1]);
        assert_eq!(m.local_size, [3, 5, -5, -10]);
        // 项数错 → Invalid
        assert!(matches!(
            MapMeta::from_doc(&doc("[Map]\nTheater=SNOW\nSize=0,0,60\n")),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn header_and_lighting_from_ini() {
        let d = doc(
            "[Header]\nStartX=233\nStartY=30\nWidth=45\nHeight=44\nNumberStartingPoints=0\n\
             [Lighting]\nAmbient=1.000000\nRed=0.550000\nLevel=0.030000\n",
        );
        let h = HeaderMeta::from_doc(&d);
        assert_eq!(
            (
                h.start_x,
                h.start_y,
                h.width,
                h.height,
                h.number_starting_points
            ),
            (233, 30, 45, 44, 0)
        );
        let l = Lighting::from_doc(&d);
        assert_eq!(l.ambient, Some(1.0));
        assert_eq!(l.red, Some(0.55));
        assert_eq!(l.green, None);
        assert_eq!(l.level, Some(0.03));
    }

    #[test]
    fn digest_decodes_twenty_bytes() {
        // 规格依据 §9:c1a01md 的 [Digest] 样本
        let d = doc("[Digest]\n1=qSt1w6I0r+9EWkfWl7XFZMt0Vo4=\n");
        let digest = parse_digest(&d).unwrap();
        assert_eq!(
            digest,
            [
                0xA9, 0x2B, 0x75, 0xC3, 0xA2, 0x34, 0xAF, 0xEF, 0x44, 0x5A, 0x47, 0xD6, 0x97, 0xB5,
                0xC5, 0x64, 0xCB, 0x74, 0x56, 0x8E
            ]
        );
        // 缺失 → None;畸形 → None(容错,不报错)
        assert!(parse_digest(&doc("[Basic]\nName=X\n")).is_none());
        assert!(parse_digest(&doc("[Digest]\n1=不合法\n")).is_none());
    }
}
