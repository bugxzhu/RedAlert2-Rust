// SPDX-License-Identifier: GPL-3.0-only

//! 地图/地形解析层(M2)。
//!
//! 模块族:`pack`(四包段容器)、`iso`(IsoMapPack5 瓦片网格)、`overlay`
//! (双 512×512 网格)、`preview`(裸 RGB)、`objects`(对象段)、`meta`
//! (元数据段)、`mapdoc`(解码策略)、`gbk`/`gbk_table`(GBK 有损解码)。
//! 总入口 `MapFile::parse` 在本模块总装。

pub mod gbk;
pub mod gbk_table;
pub mod iso;
pub mod mapdoc;
pub mod meta;
pub mod objects;
pub mod overlay;
pub mod pack;
pub mod preview;

use crate::format::ini::IniDocument;
use crate::format::map::iso::IsoGrid;
use crate::format::map::meta::{BasicInfo, HeaderMeta, Lighting, MapMeta};
use crate::format::map::objects::MapObjects;
use crate::format::map::overlay::OverlayLayers;
use crate::format::map::pack::{decode_pack_section, PackBound, PackCodec};
use crate::format::FormatError;

/// logic 段白名单(tokenize-only:原始键值行保留,不解码语义;spec §3 清单
/// + 语料同族段 [AITriggerTypes]/[AITriggerTypesEnable]/[CellTags])。
const TOKENIZE_SECTIONS: [&str; 12] = [
    "AITriggerTypes",
    "AITriggerTypesEnable",
    "Actions",
    "CellTags",
    "Events",
    "ScriptTypes",
    "Tags",
    "TaskForces",
    "TeamTypes",
    "Triggers",
    "VariableNames",
    "Waypoints",
];

/// 一个 tokenize-only 保留段(原始键值行;键值如文件所示,ci 查找用规范名)。
#[derive(Debug, Clone)]
pub struct LogicSection {
    pub name: String,
    pub entries: Vec<(String, String)>,
}

/// 四包段解压字节数统计(CLI info 用)。
#[derive(Debug, Clone, Copy, Default)]
pub struct PackStats {
    pub iso_out: usize,
    pub overlay_out: usize,
    pub overlay_data_out: usize,
    pub preview_out: usize,
    /// 预览前缀不足 3·pw·ph(截断/全失败;全语料 807 图实测仅 templateMap
    /// 一张为 true——18 张 DDLY 异常尾图前缀均精确,为 false)。
    pub preview_truncated: bool,
}

/// 解析后的地图(API 形状冻结:parse + 只读字段)。
pub struct MapFile {
    pub basic: Option<BasicInfo>,
    pub map_meta: MapMeta,
    pub header: HeaderMeta,
    pub lighting: Lighting,
    pub digest: Option<[u8; 20]>,
    pub tiles: IsoGrid,
    pub overlays: OverlayLayers,
    pub preview: Option<preview::PreviewImage>,
    pub objects: MapObjects,
    pub logic_sections: Vec<LogicSection>,
    /// 首段前的孤儿键值行数(RMG MapUnit 半图;IniDocument 计数透传)。
    pub orphan_key_values: usize,
    pub pack_stats: PackStats,
}

/// 瓦片流上界的兜底常量(300×300 行走容量字节;研究 §8-3)。真图最大
/// 943,734 B(2+1=3.map,272×158)在此之下。
const TILE_STREAM_FALLBACK_CAP: usize = 11 * (2 * 300 - 1) * 300 + 4;

/// [Map]Size 的 (W,H):合法域 1..=4096;否则 (300,300) 兜底(研究 §8-3:
/// INI 不可用时以兜底上界放行,再由瓦片记录数上界与 %11 判定兜底正确性)。
fn map_size_wh(size: [i64; 4]) -> (u16, u16) {
    let w = size[2];
    let h = size[3];
    if (1..=4096).contains(&w) && (1..=4096).contains(&h) {
        (w as u16, h as u16)
    } else {
        (300, 300)
    }
}

/// 11·(2W−1)·H+4,u128 防溢出,超过兜底常量时钳到兜底。
fn tile_stream_bound(w: u16, h: u16) -> usize {
    let bound = (2u128 * w as u128 - 1) * h as u128 * 11 + 4;
    if bound > TILE_STREAM_FALLBACK_CAP as u128 {
        TILE_STREAM_FALLBACK_CAP
    } else {
        bound as usize
    }
}

/// [Preview]Size → (pw, ph):合法域 1..=8192 且 ≤ PREVIEW_MAX_PIXELS;
/// 缺失/畸形 → None(= 无预览)。
fn preview_dims(doc: &IniDocument) -> Option<(u32, u32)> {
    let s = doc.value("Preview", "Size")?;
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return None;
    }
    let pw = crate::format::ini::parse_int_prefix(parts[2])?;
    let ph = crate::format::ini::parse_int_prefix(parts[3])?;
    if !(1..=8192).contains(&pw) || !(1..=8192).contains(&ph) {
        return None;
    }
    if pw as u64 * ph as u64 > preview::PREVIEW_MAX_PIXELS {
        return None;
    }
    Some((pw as u32, ph as u32))
}

fn tokenize(doc: &IniDocument) -> Vec<LogicSection> {
    let mut out = Vec::new();
    for name in TOKENIZE_SECTIONS {
        if let Some(sec) = doc.section(name) {
            out.push(LogicSection {
                name: name.to_string(),
                entries: sec.entries.clone(),
            });
        }
    }
    out
}

impl MapFile {
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        let doc = IniDocument::parse(&mapdoc::decode_map_text(data));
        let orphan_key_values = doc.orphan_key_values;
        let map_meta = MapMeta::from_doc(&doc)?;
        let basic = BasicInfo::from_doc(&doc);
        let header = HeaderMeta::from_doc(&doc);
        let lighting = Lighting::from_doc(&doc);
        let digest = meta::parse_digest(&doc);
        let (w, h) = map_size_wh(map_meta.size);

        // IsoMapPack5:缺失 = 空平地(templateMap 形态,研究 §1/§8-1)
        let tiles_section = match doc.section("IsoMapPack5") {
            Some(sec) => Some(decode_pack_section(
                &sec.entries,
                PackCodec::Lzo1x,
                PackBound::Tiles {
                    max_out: tile_stream_bound(w, h),
                },
            )?),
            None => None,
        };
        let tiles = match &tiles_section {
            Some(s) => IsoGrid::parse(&s.data, w, h)?,
            None => IsoGrid::parse(&[], w, h)?,
        };
        let iso_out = tiles_section.as_ref().map_or(0, |s| s.data.len());

        // overlay 双段:恒 262144(807/807 全有;缺失即 Invalid)
        let overlay_section = doc
            .section("OverlayPack")
            .ok_or_else(|| FormatError::Invalid("缺少 [OverlayPack] 段".to_string()))?;
        let overlay_data_section = doc
            .section("OverlayDataPack")
            .ok_or_else(|| FormatError::Invalid("缺少 [OverlayDataPack] 段".to_string()))?;
        let overlay_out = decode_pack_section(
            &overlay_section.entries,
            PackCodec::Lcw,
            PackBound::Exact(overlay::OVERLAY_GRID_LEN),
        )?;
        let overlay_data_out = decode_pack_section(
            &overlay_data_section.entries,
            PackCodec::Lcw,
            PackBound::Exact(overlay::OVERLAY_GRID_LEN),
        )?;
        let overlays = OverlayLayers::parse(&overlay_out.data, &overlay_data_out.data)?;

        // PreviewPack:缺失或 [Preview] 缺失/畸形 = 无预览;
        // lenient 前缀 + "恰 3·pw·ph" 判定(研究 §5;7 张 DDLY 图前缀精确 → Some)
        let (preview, preview_out, preview_truncated) =
            match (doc.section("PreviewPack"), preview_dims(&doc)) {
                (Some(sec), Some((pw, ph))) => {
                    let max_out = 3usize * pw as usize * ph as usize;
                    let section = decode_pack_section(
                        &sec.entries,
                        PackCodec::Lzo1x,
                        PackBound::Preview { max_out },
                    )?;
                    let truncated = section.data.len() != max_out;
                    (
                        preview::PreviewImage::parse(&section.data, pw, ph),
                        section.data.len(),
                        truncated,
                    )
                }
                (Some(sec), None) => {
                    // 有包段但无 [Preview] 尺寸:无法判界,按无预览处理(统计清零)
                    let _ = sec;
                    (None, 0, false)
                }
                (None, _) => (None, 0, false),
            };

        let objects = objects::parse_objects(&doc);
        let logic_sections = tokenize(&doc);

        Ok(MapFile {
            basic,
            map_meta,
            header,
            lighting,
            digest,
            tiles,
            overlays,
            preview,
            objects,
            logic_sections,
            orphan_key_values,
            pack_stats: PackStats {
                iso_out,
                overlay_out: overlay_out.data.len(),
                overlay_data_out: overlay_data_out.data.len(),
                preview_out,
                preview_truncated,
            },
        })
    }

    /// [Map]Size 的 (W,H),与解析期同规则:合法域 1..=4096,否则 (300,300)
    /// 兜底(瓦片记录数上界按兜底口径放行,渲染遍历须与之一致)。
    pub fn size_wh(&self) -> (u16, u16) {
        let w = self.map_meta.size[2];
        let h = self.map_meta.size[3];
        if (1..=4096).contains(&w) && (1..=4096).contains(&h) {
            (w as u16, h as u16)
        } else {
            (300, 300)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // —— 测试专用微型编码器(与 pack.rs 测试同方案;测试代码不进公开 API)——
    fn lzo_literal_payload(data: &[u8]) -> Vec<u8> {
        assert!((1..=238).contains(&data.len()));
        let mut p = vec![(17 + data.len()) as u8];
        p.extend_from_slice(data);
        p.extend_from_slice(&[0x11, 0x00, 0x00]);
        p
    }

    fn push_lzo_chunk(blob: &mut Vec<u8>, data: &[u8]) {
        let payload = lzo_literal_payload(data);
        blob.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        blob.extend_from_slice(&(data.len() as u16).to_le_bytes());
        blob.extend_from_slice(&payload);
    }

    fn lzo_literal_stream(data: &[u8]) -> Vec<u8> {
        let mut blob = Vec::new();
        let (full, rest) = data.as_chunks::<238>();
        for chunk in full {
            push_lzo_chunk(&mut blob, chunk);
        }
        if !rest.is_empty() {
            push_lzo_chunk(&mut blob, rest);
        }
        blob
    }

    // 偏差记录:brief 原文的该助手 size_in=4 且无 LCW 0x80 终止符——解码器
    // 读完 0xFE 长填充后继续读下一 opcode 会 UnexpectedEof;按 pack.rs 测试
    // 的已验证形态修正(size_in=5,载荷以 0x80 终止)。
    fn lcw_fill_stream(fill: u8, total: usize) -> Vec<u8> {
        let mut blob = Vec::new();
        let mut left = total;
        while left > 0 {
            let n = left.min(8192);
            blob.extend_from_slice(&5u16.to_le_bytes());
            blob.extend_from_slice(&(n as u16).to_le_bytes());
            blob.extend_from_slice(&[0xFE, n as u8, (n >> 8) as u8, fill, 0x80]);
            left -= n;
        }
        blob
    }

    fn b64(bytes: &[u8]) -> String {
        const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for c in bytes.as_chunks::<3>().0 {
            let n = ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32;
            s.push(A[(n >> 18) as usize & 63] as char);
            s.push(A[(n >> 12) as usize & 63] as char);
            s.push(A[(n >> 6) as usize & 63] as char);
            s.push(A[n as usize & 63] as char);
        }
        match bytes.len() % 3 {
            1 => {
                let n = (bytes[bytes.len() - 1] as u32) << 16;
                s.push(A[(n >> 18) as usize & 63] as char);
                s.push(A[(n >> 12) as usize & 63] as char);
                s.push_str("==");
            }
            2 => {
                let n = ((bytes[bytes.len() - 2] as u32) << 16)
                    | ((bytes[bytes.len() - 1] as u32) << 8);
                s.push(A[(n >> 18) as usize & 63] as char);
                s.push(A[(n >> 12) as usize & 63] as char);
                s.push(A[(n >> 6) as usize & 63] as char);
                s.push('=');
            }
            _ => {}
        }
        s
    }

    fn entries_text(blob: &[u8]) -> Vec<(String, String)> {
        let text = b64(blob);
        let mut v = Vec::new();
        let mut i = 0usize;
        let mut k = 1usize;
        while i < text.len() {
            let n = (text.len() - i).min(70);
            v.push((k.to_string(), text[i..i + n].to_string()));
            i += n;
            k += 1;
        }
        v
    }

    fn push_pack_section(ini: &mut String, name: &str, blob: &[u8]) {
        ini.push_str(&format!("[{name}]\n"));
        for (k, v) in entries_text(blob) {
            ini.push_str(&k);
            ini.push('=');
            ini.push_str(&v);
            ini.push('\n');
        }
    }

    fn rec(x: u16, y: u16, tile: i16, sub: u8, z: u8) -> [u8; 11] {
        let mut b = [0u8; 11];
        b[0..2].copy_from_slice(&x.to_le_bytes());
        b[2..4].copy_from_slice(&y.to_le_bytes());
        b[4..6].copy_from_slice(&tile.to_le_bytes());
        b[8] = sub;
        b[9] = z;
        b
    }

    /// mini 地图:w=4,h=4;2 条瓦片记录 + 4 零尾;预览 2×1(6 B);
    /// overlay 全 FF/全 00;1 结构 + 1 地形(坐标与瓦片记录 2 重合);
    /// Waypoints 一条 + Actions 一条(tokenize 验证);Digest 一条。
    fn mini_map() -> Vec<u8> {
        let mut tiles = Vec::new();
        tiles.extend_from_slice(&rec(1, 1, 5, 0, 0));
        tiles.extend_from_slice(&rec(3, 2, 6, 2, 1));
        tiles.extend_from_slice(&[0, 0, 0, 0]);
        let preview: &[u8] = &[0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00];
        let mut ini = String::new();
        ini.push_str("[Basic]\nName=TestMap\nOfficial=yes\nMaxPlayer=4\n");
        ini.push_str("[Map]\nTheater=SNOW\nSize=0,0,4,4\nLocalSize=0,0,4,4\n");
        ini.push_str("[Header]\nWidth=3\nHeight=3\nNumberStartingPoints=2\n");
        ini.push_str("[Lighting]\nAmbient=1.000000\n");
        ini.push_str("[Preview]\nSize=0,0,2,1\n");
        push_pack_section(&mut ini, "PreviewPack", &lzo_literal_stream(preview));
        push_pack_section(&mut ini, "OverlayPack", &lcw_fill_stream(0xFF, 262144));
        push_pack_section(&mut ini, "OverlayDataPack", &lcw_fill_stream(0x00, 262144));
        push_pack_section(&mut ini, "IsoMapPack5", &lzo_literal_stream(&tiles));
        ini.push_str(
            "[Structures]\n0=Neutral,CABHUT,256,2,3,0,None,0,0,1,0,0,None,None,None,0,0\n",
        );
        ini.push_str("[Terrain]\n2003=TIBTRE01\n");
        ini.push_str("[Waypoints]\n0=2003\n");
        ini.push_str("[Actions]\n0c44c1dc=11,99,7,_Amb_OceanHeavy,0,0,0,0,AA,99,7\n");
        ini.push_str("[Digest]\n1=qSt1w6I0r+9EWkfWl7XFZMt0Vo4=\n");
        ini.into_bytes()
    }

    fn strip_section(data: &[u8], section: &str) -> Vec<u8> {
        // 从 mini 地图文本里删掉一个段(含其后的键值行)——敌意/缺失路径用
        let text = String::from_utf8(data.to_vec()).unwrap();
        let mut out = String::new();
        let mut skipping = false;
        for line in text.lines() {
            if line.starts_with('[') {
                skipping = line == format!("[{section}]");
            }
            if !skipping {
                out.push_str(line);
                out.push('\n');
            }
        }
        out.into_bytes()
    }

    #[test]
    fn parses_full_synthetic_map() {
        let m = MapFile::parse(&mini_map()).unwrap();
        // 元数据
        let basic = m.basic.as_ref().unwrap();
        assert_eq!(basic.name.as_deref(), Some("TestMap"));
        assert!(basic.official);
        assert_eq!(basic.max_player, 4);
        assert_eq!(m.map_meta.theater, meta::Theater::Snow);
        assert_eq!(m.map_meta.size, [0, 0, 4, 4]);
        assert_eq!(m.header.width, 3);
        assert_eq!(m.lighting.ambient, Some(1.0));
        // Digest:规格依据 §9 钉死的 20 字节(c1a01md 样本,与 meta.rs 测试同值)
        assert_eq!(
            m.digest,
            Some([
                0xA9, 0x2B, 0x75, 0xC3, 0xA2, 0x34, 0xAF, 0xEF, 0x44, 0x5A, 0x47, 0xD6, 0x97, 0xB5,
                0xC5, 0x64, 0xCB, 0x74, 0x56, 0x8E
            ])
        );
        // 瓦片:存储坐标权威 + 与对象坐标恒等(同菱形坐标系)
        assert_eq!(m.tiles.len(), 2);
        let t = m.tiles.cell(3, 2).unwrap();
        assert_eq!((t.tile, t.sub_tile, t.z), (6, 2, 1));
        let terrain = m
            .objects
            .all
            .iter()
            .find(|o| o.class == objects::ObjectClass::Terrain)
            .unwrap();
        assert_eq!(terrain.cell(), Some((3, 2)));
        assert!(m.tiles.cell(3, 2).is_some());
        assert!(m.tiles.cell(2, 3).is_none());
        // overlay:全 FF 类型 → frame 恒 None
        assert_eq!(m.overlays.type_at(2, 3), 0xFF);
        assert_eq!(m.overlays.frame_at(2, 3), None);
        // 预览:Some 且字节原样
        let img = m.preview.as_ref().unwrap();
        assert_eq!((img.width, img.height), (2, 1));
        assert_eq!(img.rgb, vec![0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00]);
        // 对象/逻辑/统计
        assert_eq!(m.objects.count(objects::ObjectClass::Structure), 1);
        assert_eq!(m.objects.count(objects::ObjectClass::Terrain), 1);
        assert_eq!(m.objects.malformed, 0);
        assert_eq!(m.logic_sections.len(), 2); // Waypoints + Actions
        let wp = m
            .logic_sections
            .iter()
            .find(|s| s.name == "Waypoints")
            .unwrap();
        assert_eq!(wp.entries, vec![("0".to_string(), "2003".to_string())]);
        assert_eq!(m.orphan_key_values, 0);
        assert_eq!(m.pack_stats.iso_out, 26);
        assert_eq!(m.pack_stats.overlay_out, 262144);
        assert_eq!(m.pack_stats.overlay_data_out, 262144);
        assert_eq!(m.pack_stats.preview_out, 6);
        assert!(!m.pack_stats.preview_truncated);
    }

    #[test]
    fn missing_isomapack5_is_empty_flat_map() {
        let m = MapFile::parse(&strip_section(&mini_map(), "IsoMapPack5")).unwrap();
        assert!(m.tiles.is_empty());
        assert_eq!(m.pack_stats.iso_out, 0);
    }

    #[test]
    fn missing_preview_pack_is_none() {
        let m = MapFile::parse(&strip_section(&mini_map(), "PreviewPack")).unwrap();
        assert!(m.preview.is_none());
        assert_eq!(m.pack_stats.preview_out, 0);
        assert!(!m.pack_stats.preview_truncated);
    }

    #[test]
    fn missing_digest_is_ok() {
        let m = MapFile::parse(&strip_section(&mini_map(), "Digest")).unwrap();
        assert!(m.digest.is_none());
    }

    #[test]
    fn missing_basic_is_ok() {
        let m = MapFile::parse(&strip_section(&mini_map(), "Basic")).unwrap();
        assert!(m.basic.is_none());
    }

    #[test]
    fn hostile_truncated_base64_iso_is_err() {
        // IsoMapPack5 的 base64 中段坏字符:严格段必须 Err(截断中段敌意样例)
        let mut data = mini_map();
        let text = String::from_utf8(data.clone()).unwrap();
        let mut out = String::new();
        let mut in_iso = false;
        let mut patched = false;
        for line in text.lines() {
            if line.starts_with('[') {
                in_iso = line == "[IsoMapPack5]";
            } else if in_iso && !patched && line.contains('=') && line.len() > 4 {
                let (k, _) = line.split_once('=').unwrap();
                out.push_str(&format!("{k}=QU?JD\n")); // 坏字符 '?'
                patched = true;
                continue;
            }
            out.push_str(line);
            out.push('\n');
        }
        data = out.into_bytes();
        assert!(matches!(
            MapFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn hostile_overlay_sum_lie_is_err() {
        // OverlayPack 只有半量块(Σ 131072 ≠ 262144)→ Invalid
        let mut ini = String::new();
        ini.push_str("[Map]\nTheater=SNOW\nSize=0,0,4,4\n");
        ini.push_str("[Preview]\nSize=0,0,2,1\n");
        push_pack_section(&mut ini, "OverlayPack", &lcw_fill_stream(0xFF, 131072));
        push_pack_section(&mut ini, "OverlayDataPack", &lcw_fill_stream(0x00, 262144));
        push_pack_section(
            &mut ini,
            "IsoMapPack5",
            &lzo_literal_stream(&rec(1, 1, 5, 0, 0)),
        );
        assert!(matches!(
            MapFile::parse(&ini.into_bytes()),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn hostile_overlay_bad_char_is_err() {
        let mut ini = String::from("[Map]\nTheater=SNOW\nSize=0,0,4,4\n[OverlayPack]\n1=QU?JD\n");
        let blob = lcw_fill_stream(0xFF, 262144);
        for (k, v) in entries_text(&blob).iter().skip(1) {
            ini.push_str(&format!("{k}={v}\n"));
        }
        ini.push_str("[OverlayDataPack]\n");
        for (k, v) in entries_text(&lcw_fill_stream(0x00, 262144)) {
            ini.push_str(&format!("{k}={v}\n"));
        }
        ini.push_str("[IsoMapPack5]\n");
        for (k, v) in entries_text(&lzo_literal_stream(&rec(1, 1, 5, 0, 0))) {
            ini.push_str(&format!("{k}={v}\n"));
        }
        assert!(matches!(
            MapFile::parse(&ini.into_bytes()),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn hostile_truncated_preview_degrades_to_none() {
        // 前缀 3 字节 < 3·2·1 = 6:Ok + preview None + truncated true
        let mut ini = String::new();
        ini.push_str("[Map]\nTheater=SNOW\nSize=0,0,4,4\n");
        ini.push_str("[Preview]\nSize=0,0,2,1\n");
        push_pack_section(&mut ini, "PreviewPack", &lzo_literal_stream(&[1, 2, 3]));
        push_pack_section(&mut ini, "OverlayPack", &lcw_fill_stream(0xFF, 262144));
        push_pack_section(&mut ini, "OverlayDataPack", &lcw_fill_stream(0x00, 262144));
        push_pack_section(
            &mut ini,
            "IsoMapPack5",
            &lzo_literal_stream(&rec(1, 1, 5, 0, 0)),
        );
        let m = MapFile::parse(&ini.into_bytes()).unwrap();
        assert!(m.preview.is_none());
        assert_eq!(m.pack_stats.preview_out, 3);
        assert!(m.pack_stats.preview_truncated);
    }

    #[test]
    fn hostile_iso_over_capacity_is_err() {
        // Size=1,1(容量 1)但 2 条记录 → 瓦片层 Invalid(敌意记录超界)
        let mut tiles = Vec::new();
        tiles.extend_from_slice(&rec(1, 1, 5, 0, 0));
        tiles.extend_from_slice(&rec(1, 1, 6, 0, 0));
        let mut ini = String::new();
        ini.push_str("[Map]\nTheater=SNOW\nSize=0,0,1,1\n");
        ini.push_str("[Preview]\nSize=0,0,2,1\n");
        push_pack_section(&mut ini, "OverlayPack", &lcw_fill_stream(0xFF, 262144));
        push_pack_section(&mut ini, "OverlayDataPack", &lcw_fill_stream(0x00, 262144));
        push_pack_section(&mut ini, "IsoMapPack5", &lzo_literal_stream(&tiles));
        assert!(matches!(
            MapFile::parse(&ini.into_bytes()),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn logic_sections_tokenized_verbatim() {
        // Actions 的 8 字段链行原样保留(tokenize-only);缺失段不出现
        let m = MapFile::parse(&mini_map()).unwrap();
        let actions = m
            .logic_sections
            .iter()
            .find(|s| s.name == "Actions")
            .unwrap();
        assert_eq!(
            actions.entries[0].1,
            "11,99,7,_Amb_OceanHeavy,0,0,0,0,AA,99,7"
        );
        assert!(m.logic_sections.iter().all(|s| s.name != "Tags"));
    }

    #[test]
    fn rmg_orphan_lines_are_counted() {
        // 孤儿键值行(RMG 半图形态):继续丢弃但计数透传
        let mut data = mini_map();
        let mut text = String::from_utf8(data).unwrap();
        text.insert_str(0, "Size=0,0,60,30\n1=AAAA\n2=BBBB\n");
        data = text.into_bytes();
        let m = MapFile::parse(&data).unwrap();
        assert_eq!(m.orphan_key_values, 3);
    }
}
