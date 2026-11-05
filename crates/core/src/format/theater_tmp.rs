// SPDX-License-Identifier: GPL-3.0-only

//! TS 格式剧场瓦片容器(t_tmp,XCC 术语;.tem/.urb/.sno/.des/.lun/.ubn 瓦片)。
//!
//! 出处:裁决后研究 2026-11-05-theater-tile-mapping.md §4/§5 + 同日槽头宽度
//! 勘误(ra2-decomp 匹配源码 + 语料字节双证;XCC GPL 的 t_tmp 结构语义重写,
//! 注明出处;泄漏 TS/RA2 引擎源码未参考)。引擎侧:原版整体读入文件后**直接
//! 内存投射**(`Load_Tile_Data`,gamemd 0x5B3C20 族;ra2-decomp
//! `src/isotype.cpp` @ 0x00547020 + `include/isotype.h` `IsoTileRecord`,
//! `#pragma pack(4)`,GPL-3.0+),磁盘布局 ≡ 内存布局,故 52B 槽头字段全为
//! i32 宽度;语料字节级闭合 2026-11-05 复核。
//!
//! 布局:16B i32 头 `{cbx, cby, cx, cy}` + cbx·cby 个 i32 槽偏移 + 每非空槽
//! 52B 图像头 + RAW 菱形 cx·cy/2 字节 + 可选 z 数据(bitfield bit1 置位时,
//! 恒 cx·cy/2 字节)。**两级偏移语义(勿混)**:槽表项 = 文件基相对
//! (多槽语料首槽值恒 = 16+4n;0 = 空槽);extra_ofs/z_ofs 字段 = 槽(record)
//! 相对(引擎 isotype.cpp:1282 `record + ExtraOffset`)。z 按连续布局读
//! (槽头 → 菱形 → z 紧邻;语料真:clear01 与 10 个多槽文件 `len == 末槽 +
//! 52 + cx·cy/2 + cx·cy/2 [+ extra]` 精确闭合,槽距 1852 = 52+900+900);
//! z_ofs 字段语料恒 = 52+cx·cy/2,原样暴露不校验。子瓦片索引 = x + cbx·y
//! 行主序(引擎 `SubTile_Index`,由 Task 7 按 header.x/y 与 cbx 计算)。
//!
//! 语料锚:clear01.sno 总长 1872 = 16 + 4·1 + 52 + 900 + 900 精确
//! (cbx=cby=1,cx=60,cy=30)。52B 头未用域 0xCD 填充(MSVC 未初始化堆渗入,
//! 西木出厂原样):0xCD/0xCC 域照抄暴露不校验(零 panic 姿势,M2 先例);
//! terrain_type/ramp_type 值表未逐值复核,只透传原始字节(spec §3 边界)。
//! extra 页不解析(spec §3 边界,Task 5/7 自行取用)。
//!
//! bitfield:bit0 = IsHasExtraData、bit1 = IsHasZData、bit2 = IsRandomized
//! (非"damaged";语料 MM 瓦片 0xCE/0xCF 实测置位),bit3+ 原样透传。
//!
//! 敌意预检(谎报一律 `Invalid`,分配前判定):`cbx·cby ≤ 65536`
//! (语料最大 5×5)、`cx·cy/2 ≤ 2 MiB`(60×30 的 900 B 远在其下)、
//! `16+4·cbx·cby+52 ≤ len`;槽偏移负值或落在头/槽表区 → Invalid;槽越界
//! (52 头 + 菱形 + 按 bitfield 精确的一份 z 超文件)→ Invalid。任何残余
//! 截断在读阶段被 ByteReader 拦为 UnexpectedEof(精确预检后不可达,纯防御)。

use super::bytes::ByteReader;
use super::FormatError;

/// 敌意槽网格上界(spec §4;语料最大 5×5)。
pub const MAX_SLOTS: u64 = 65536;

/// 单槽菱形字节数上界(spec §4;60×30 的 900 B 远在其下)。
pub const MAX_DIAMOND_BYTES: usize = 2 * 1024 * 1024;

/// 52B 图像头(`IsoTileRecord` 磁盘投射,`#pragma pack(4)`;0x34 = 52 精确
/// 覆盖,无未文档化尾域)。
pub struct TsTmpSlotHeader {
    /// 槽内绘制原点(引擎 int X/Y)。
    pub x: i32,
    pub y: i32,
    /// extra 数据槽相对偏移(bit0=0 时可为陈旧值,门控以 bit0 为准;
    /// extra 页本解析器不读)。
    pub extra_ofs: i32,
    /// z 数据槽相对偏移;语料恒 = 52+cx·cy/2(连续布局),原样暴露不校验。
    pub z_ofs: i32,
    /// extra-z 数据槽相对偏移(无 extra-z 时 0xCD 填充)。
    pub extra_z_ofs: i32,
    /// extra 页矩形 ExtraX/Y/Width/Height(extra 页不解析,spec §3 边界)。
    pub extra_rect: [i32; 4],
    /// bit0=IsHasExtraData、bit1=IsHasZData、bit2=IsRandomized;bit3+ 原样
    /// 透传(高 3 字节语料恒 0xCD,引擎只消费低 3 位)。
    pub bitfield: u32,
    /// 高度基准(clear01=0)。
    pub height: u8,
    /// 引擎名 TileType(值表未逐值复核,透传)。
    pub terrain_type: i8,
    /// 引擎名 RampType(1..=15 坡向,0=平)。
    pub ramp_type: i8,
    /// LowColor(左半小地图色)。
    pub radar_left: [u8; 3],
    /// HighColor(右半小地图色)。
    pub radar_right: [u8; 3],
    /// pack(4) 对齐垫(语料 0xCD,不校验)。
    pub pad: [u8; 3],
}

impl TsTmpSlotHeader {
    /// bit0:IsHasExtraData(extra 页存在;本解析器不读其内容)。
    pub fn is_has_extra_data(&self) -> bool {
        self.bitfield & 0b1 != 0
    }

    /// bit1:IsHasZData(高度/z 数据存在,恒 cx·cy/2 字节)。
    pub fn is_has_z_data(&self) -> bool {
        self.bitfield & 0b10 != 0
    }

    /// bit2:IsRandomized(语料 MM 瓦片 0xCE/0xCF 实测置位)。
    pub fn is_randomized(&self) -> bool {
        self.bitfield & 0b100 != 0
    }
}

/// 一个非空槽:52B 头 + RAW 菱形 + 可选 z(连续布局)。
pub struct TsTmpSlot {
    pub header: TsTmpSlotHeader,
    diamond: Vec<u8>,
    z: Option<Vec<u8>>,
}

impl TsTmpSlot {
    /// RAW 菱形像素(恒 cx·cy/2 字节;铺 canvas 行序见 Task 7)。
    pub fn diamond_pixels(&self) -> &[u8] {
        &self.diamond
    }

    /// 高度/z 数据(bitfield bit1 置位时存在,恒 cx·cy/2 字节)。
    pub fn z_data(&self) -> Option<&[u8]> {
        self.z.as_deref()
    }
}

/// TS TMP 文件。
pub struct TsTmpFile {
    cbx: u32,
    cby: u32,
    cx: u32,
    cy: u32,
    slots: Vec<Option<TsTmpSlot>>,
}

impl TsTmpFile {
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        let mut r = ByteReader::new(data);
        // 敌意负值:i32 原读后再解释(u32 → i32 → i64 保号)
        let cbx = r.read_u32_le()? as i32 as i64;
        let cby = r.read_u32_le()? as i32 as i64;
        let cx = r.read_u32_le()? as i32 as i64;
        let cy = r.read_u32_le()? as i32 as i64;
        if cbx <= 0 || cby <= 0 || cx < 2 || cy < 2 {
            return Err(FormatError::Invalid(format!(
                "TS TMP 头尺寸谎报: cbx={cbx} cby={cby} cx={cx} cy={cy}"
            )));
        }
        if cbx * cby > MAX_SLOTS as i64 {
            return Err(FormatError::Invalid(format!(
                "槽网格 {cbx}×{cby} = {} 超过上界 {MAX_SLOTS}",
                cbx * cby
            )));
        }
        if (cx * cy) % 2 != 0 {
            return Err(FormatError::Invalid(format!(
                "画布 {cx}×{cy} 像素数为奇,无法对半分菱形"
            )));
        }
        let diamond = cx * cy / 2;
        if diamond > MAX_DIAMOND_BYTES as i64 {
            return Err(FormatError::Invalid(format!(
                "单槽菱形 {diamond} B 超过上界 {MAX_DIAMOND_BYTES}"
            )));
        }
        let diamond = diamond as usize;
        let slot_count = cbx * cby;
        let table_end = 16 + 4 * slot_count as usize;
        if table_end + 52 > data.len() {
            return Err(FormatError::Invalid(format!(
                "文件长 {} 不足以容纳头+槽表+最小槽头(需 {table_end}+52)",
                data.len()
            )));
        }
        let mut offsets = Vec::with_capacity(slot_count as usize);
        for _ in 0..slot_count {
            offsets.push(r.read_u32_le()? as i32 as i64);
        }
        // 第一级预检:偏移合法性(负值或落入头/槽表区)
        for (i, &off) in offsets.iter().enumerate() {
            if off == 0 {
                continue; // 空槽
            }
            if off < 0 || (off as usize) < table_end {
                return Err(FormatError::Invalid(format!(
                    "槽 {i} 偏移 {off} 非法(负值或落在头/槽表区 {table_end} 内)"
                )));
            }
        }
        // 第二级:逐槽精确解析——读槽头后按 bitfield 精确判定槽体范围
        // (52 头 + 菱形 + 一份 z,bit1 未置位则无 z),越界一律 Invalid。
        let mut slots = Vec::with_capacity(offsets.len());
        for &off in &offsets {
            slots.push(if off == 0 {
                None
            } else {
                Some(parse_slot(data, off as usize, diamond)?)
            });
        }
        Ok(TsTmpFile {
            cbx: cbx as u32,
            cby: cby as u32,
            cx: cx as u32,
            cy: cy as u32,
            slots,
        })
    }

    /// (cbx, cby, cx, cy)。
    pub fn dims(&self) -> (u32, u32, u32, u32) {
        (self.cbx, self.cby, self.cx, self.cy)
    }

    /// 槽位总数 = cbx·cby(空槽计入)。
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// 槽位;`None` = 空槽(偏移 0)或下标越界。
    pub fn slot(&self, index: usize) -> Option<&TsTmpSlot> {
        self.slots.get(index).and_then(|s| s.as_ref())
    }
}

fn parse_slot(data: &[u8], off: usize, diamond_len: usize) -> Result<TsTmpSlot, FormatError> {
    if off.checked_add(52).is_none_or(|end| end > data.len()) {
        return Err(FormatError::Invalid(format!(
            "槽偏移 {off} 处 52B 槽头越界(文件长 {})",
            data.len()
        )));
    }
    let mut r = ByteReader::new(&data[off..]);
    let x = r.read_u32_le()? as i32;
    let y = r.read_u32_le()? as i32;
    let extra_ofs = r.read_u32_le()? as i32;
    let z_ofs = r.read_u32_le()? as i32;
    let extra_z_ofs = r.read_u32_le()? as i32;
    let extra_x = r.read_u32_le()? as i32;
    let extra_y = r.read_u32_le()? as i32;
    let extra_cx = r.read_u32_le()? as i32;
    let extra_cy = r.read_u32_le()? as i32;
    let bitfield = r.read_u32_le()?;
    let height = r.read_u8()?;
    let terrain_type = r.read_u8()? as i8;
    let ramp_type = r.read_u8()? as i8;
    let radar_left = [r.read_u8()?, r.read_u8()?, r.read_u8()?];
    let radar_right = [r.read_u8()?, r.read_u8()?, r.read_u8()?];
    let pad = [r.read_u8()?, r.read_u8()?, r.read_u8()?];
    let header = TsTmpSlotHeader {
        x,
        y,
        extra_ofs,
        z_ofs,
        extra_z_ofs,
        extra_rect: [extra_x, extra_y, extra_cx, extra_cy],
        bitfield,
        height,
        terrain_type,
        ramp_type,
        radar_left,
        radar_right,
        pad,
    };
    // 槽体精确范围:52 头 + 菱形 + 一份 z(bit1 置位时);超文件 → Invalid。
    let z_len = if header.is_has_z_data() {
        diamond_len
    } else {
        0
    };
    let need = 52 + diamond_len + z_len;
    if off.checked_add(need).is_none_or(|end| end > data.len()) {
        return Err(FormatError::Invalid(format!(
            "槽偏移 {off} 越界: 槽体需 {need} B(52 头+{diamond_len} 菱形+{z_len} z),文件仅剩 {}",
            data.len() - off
        )));
    }
    let mut diamond = Vec::with_capacity(diamond_len);
    diamond.extend_from_slice(r.read_bytes(diamond_len)?);
    let z = if header.is_has_z_data() {
        let mut z = Vec::with_capacity(diamond_len);
        z.extend_from_slice(r.read_bytes(diamond_len)?);
        Some(z)
    } else {
        None
    };
    Ok(TsTmpSlot { header, diamond, z })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 52B 槽头(blob 前 52 字节除 bitfield 全零)+ 菱形 + 可选 z。
    /// bitfield 在 +0x24 = 36(i32 宽度布局)。
    fn slot_bytes(bitfield: u32, diamond: &[u8], z: Option<&[u8]>) -> Vec<u8> {
        let mut b = vec![0u8; 52];
        b[36..40].copy_from_slice(&bitfield.to_le_bytes());
        b.extend_from_slice(diamond);
        if let Some(z) = z {
            b.extend_from_slice(z);
        }
        b
    }

    /// 头 + 槽偏移表 + 任意尾段(槽 blob 由测试自行放置在声明的偏移处)。
    fn tmp_bytes(cbx: i32, cby: i32, cx: i32, cy: i32, offsets: &[i32], tail: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&cbx.to_le_bytes());
        v.extend_from_slice(&cby.to_le_bytes());
        v.extend_from_slice(&cx.to_le_bytes());
        v.extend_from_slice(&cy.to_le_bytes());
        for o in offsets {
            v.extend_from_slice(&o.to_le_bytes());
        }
        v.extend_from_slice(tail);
        v
    }

    #[test]
    fn single_slot_closure_with_z() {
        // cx=8, cy=4 → 菱形 16 B;bit1 置位 → z 16 B;len = 16+4+52+16+16 = 104
        // (clear01.sno 的 1872 = 16+4+52+900+900 同构缩小版)
        let diamond: Vec<u8> = (1..=16).collect();
        let z = vec![0xAAu8; 16];
        let blob = slot_bytes(2, &diamond, Some(&z));
        let data = tmp_bytes(1, 1, 8, 4, &[20], &blob);
        let f = TsTmpFile::parse(&data).unwrap();
        assert_eq!(f.dims(), (1, 1, 8, 4));
        assert_eq!(f.slot_count(), 1);
        let s = f.slot(0).unwrap();
        assert_eq!(s.diamond_pixels(), &diamond);
        assert_eq!(s.z_data(), Some(&z[..]));
        assert_eq!(s.header.bitfield, 2);
        assert!(s.header.is_has_z_data());
        assert!(!s.header.is_has_extra_data());
        assert!(!s.header.is_randomized());
        assert_eq!(s.header.x, 0);
        assert!(f.slot(1).is_none());
    }

    #[test]
    fn empty_slot_is_none() {
        // cbx=1, cby=2 → 槽表 8 B,table_end = 24;槽 0 在 24,槽 1 偏移 0 = 空槽
        let blob = slot_bytes(0, &[7u8; 16], None);
        let data = tmp_bytes(1, 2, 8, 4, &[24, 0], &blob);
        let f = TsTmpFile::parse(&data).unwrap();
        assert_eq!(f.slot_count(), 2);
        assert!(f.slot(0).is_some());
        assert!(f.slot(1).is_none());
    }

    #[test]
    fn multi_slot_grid() {
        // cbx=2, cby=1:槽表 8 B(table_end = 24);无 z 槽体 68 B,
        // 槽 0 在 24、槽 1 在 92;len = 24 + 68·2 = 160
        let d0 = vec![1u8; 16];
        let d1 = vec![2u8; 16];
        let mut tail = slot_bytes(0, &d0, None);
        tail.extend_from_slice(&slot_bytes(0, &d1, None));
        let data = tmp_bytes(2, 1, 8, 4, &[24, 92], &tail);
        let f = TsTmpFile::parse(&data).unwrap();
        assert_eq!(f.dims(), (2, 1, 8, 4));
        assert_eq!(f.slot(0).unwrap().diamond_pixels(), &d0);
        assert_eq!(f.slot(1).unwrap().diamond_pixels(), &d1);
    }

    #[test]
    fn z_absent_when_bit1_clear() {
        let blob = slot_bytes(0, &[7u8; 16], None);
        let data = tmp_bytes(1, 1, 8, 4, &[20], &blob);
        let f = TsTmpFile::parse(&data).unwrap();
        let s = f.slot(0).unwrap();
        assert!(s.z_data().is_none());
    }

    #[test]
    fn cd_fields_passthrough() {
        // 0xCD 填充槽头(西木出厂态):原样暴露、不校验、不 panic
        let mut blob = vec![0xCDu8; 52];
        blob[36..40].copy_from_slice(&0u32.to_le_bytes()); // bitfield 无 z
        blob.extend_from_slice(&[9u8; 16]);
        let data = tmp_bytes(1, 1, 8, 4, &[20], &blob);
        let f = TsTmpFile::parse(&data).unwrap();
        let s = f.slot(0).unwrap();
        assert_eq!(s.header.extra_ofs, -842150451); // 0xCDCDCDCD as i32
        assert_eq!(s.header.x, -842150451); // i32 宽度(勘误):原样保号
        assert_eq!(s.header.extra_rect, [-842150451; 4]);
        assert_eq!(s.header.pad, [0xCDu8; 3]); // +0x31 pack(4) 垫,52 精确覆盖
        assert_eq!(s.header.terrain_type, -51); // 0xCD as i8
        assert_eq!(s.diamond_pixels(), &[9u8; 16]);
    }

    #[test]
    fn rejects_odd_canvas() {
        let data = tmp_bytes(1, 1, 7, 3, &[], &[]);
        assert!(matches!(
            TsTmpFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_zero_dims() {
        let data = tmp_bytes(0, 1, 8, 4, &[], &[]);
        assert!(matches!(
            TsTmpFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_slot_grid_over_max_slots() {
        // 300×300 = 90000 > 65536
        let data = tmp_bytes(300, 300, 8, 4, &[], &[]);
        assert!(matches!(
            TsTmpFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_canvas_diamond_over_2mib() {
        // 3000×3000/2 = 4.5 MB > 2 MiB
        let data = tmp_bytes(1, 1, 3000, 3000, &[], &[]);
        assert!(matches!(
            TsTmpFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_truncated_file() {
        // cbx=cby=1 时槽表尾 20,最小槽头到 72;20 字节文件不够
        let data = tmp_bytes(1, 1, 8, 4, &[0], &[]);
        assert!(matches!(
            TsTmpFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_slot_offset_into_header_table() {
        // 偏移 4 落在头/槽表区(table_end = 20)内
        let blob = slot_bytes(0, &[7u8; 16], None);
        let data = tmp_bytes(1, 1, 8, 4, &[4], &blob);
        assert!(matches!(
            TsTmpFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_negative_slot_offset() {
        let blob = slot_bytes(0, &[7u8; 16], None);
        let data = tmp_bytes(1, 1, 8, 4, &[-1], &blob);
        assert!(matches!(
            TsTmpFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_truncated_slot_body() {
        // 槽声明在 20,但 blob 只放了 62 B(52+10):无 z 时精确槽体 52+16 = 68,
        // 20+68 = 88 > 82 → Invalid(敌意谎报恒 Err,不 panic、不 EOF)
        let blob = slot_bytes(0, &[7u8; 10], None);
        let data = tmp_bytes(1, 1, 8, 4, &[20], &blob);
        assert!(matches!(
            TsTmpFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }
}
