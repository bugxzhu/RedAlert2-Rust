// SPDX-License-Identifier: GPL-3.0-only

//! TS SHP 精灵帧容器(t_shp_ts,XCC 术语;.sno/.tem 等剧场装饰精灵即此格式)。
//!
//! 出处:布局与判别按裁决后研究 2026-11-05-theater-tile-mapping.md §4/§6
//! (XCC `t_shp_ts` 语义重写;burnt01/lobrdb08 甲骨文帧偏移精确闭合)。
//! **M1b 误判纠正**:本文件前身 tmp.rs 曾把该 8B+24B 布局当成"RA2 剧场瓦片",
//! 实为 TS SHP 帧格式——帧头 24B = `{x,y i16; cx,cy u16; flags u32; rgba
//! [u8;4]; reserved u32; data_offset u32}`。TS 真瓦片(TS TMP,16B i32 头 +
//! 槽表)是另一种容器,由 theater_tmp.rs 承担,勿混用。
//!
//! **M4c 判别式(引擎对齐,帧级)**:帧头 +8 单字节 bit1 决定压缩形态——
//! bit1=1 → RLEZeroTS 行链(区间 [offset, 文件尾),引擎无 region 界,
//! shapipe.cpp:103-112/148-156 + draw.cpp:88-98);bit1=0 → RAW,取
//! [offset, offset+cx·cy),越文件尾补零截断(防御位;RAW 帧普遍带 2-8B
//! 尾 pad)。变体是帧级(CIV1 476 帧同文件混 0x03/0x01,研究 §1.3-A),
//! 任何文件级格式选择不可行;M1b 的"先严格 RLE、败且 span==cx·cy 才 RAW"
//! 判别式与 ends[] 下帧偏移链界由此废止(研究 §2.4)。空帧约定
//! `cx==cy==data_offset==0` 判空先于 flags 分派(空帧可带非 0 flags)。

use super::bytes::ByteReader;
use super::rle_zero_ts::rle_zero_ts_decode;
use super::FormatError;

/// 语料 SHP 最大画布 253×242(研究 §8);上限给 277 倍余量防炸(spec"分配前上界")。
pub(crate) const MAX_SHP_CANVAS_PIXELS: usize = 4096 * 4096;

/// 单帧帧头(字段语义按研究 §4 重标)。
pub struct ShpTsFrameHeader {
    pub x: i16,
    pub y: i16,
    pub cx: u16,
    pub cy: u16,
    /// +8 单字节语义(引擎 `SHPFrame::Flags`,msanim.h:49-63):bit0 =
    /// HasTransparency、bit1 = HasCompression(shapipe.cpp:137-156);
    /// +9..+0xB 匿名填充(语料观测 `0xCCCC` 高半字 = 未初始化残留,研究
    /// §1.2/§2.1)。u32 原样存储,判定只读低字节([Self::is_rle]);
    /// 观测直方 0x3×20700 / 0xCCCC0003×195 / 0xCCCC0001×31 / 0x4×92 /
    /// 0x1×32 / 0xCCCCCCCC×2(研究 §1.2)。
    pub flags: u32,
    /// +0x0C..0x10 四字节 = 引擎 Color 3B + 1 pad(msanim.h:49-63,ra2-rs
    /// 命名照旧);旧"unknown_d"即其蓝通道字节(研究 §4)。
    pub rgba: [u8; 4],
    /// +0x10..0x14(旧 radar[3]+_pad;lobrdb08 t0 = 0x0012FB00)。
    pub reserved: u32,
    /// 帧数据的绝对文件偏移;空帧为 0。
    pub data_offset: u32,
}

impl ShpTsFrameHeader {
    /// 帧级压缩判定(引擎 `HasCompression = (char)(Flags & 2) >> 1`,
    /// shapipe.cpp:148-156):只读 +8 单字节的 bit1,+9..+0xB 为匿名填充
    /// 不参与(msanim.h:49-63)。变体是帧级(CIV1 476 帧同文件混
    /// 0x03/0x01,研究 §1.3-A),任何文件级格式选择不可行。
    pub fn is_rle(&self) -> bool {
        (self.flags & 0xFF) & 2 != 0
    }
}

/// 一帧:帧头 + 解码后的调色板索引(行主序,恰好 `cx*cy` 字节;空帧为空)。
pub struct ShpTsFrame {
    pub header: ShpTsFrameHeader,
    pub pixels: Vec<u8>,
}

/// TS SHP 文件。
pub struct ShpTsFile {
    pub zero: u16,
    pub cx: u16,
    pub cy: u16,
    pub frames: Vec<ShpTsFrame>,
}

impl ShpTsFile {
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        let mut r = ByteReader::new(data);
        let zero = r.read_u16_le()?;
        if zero != 0 {
            return Err(FormatError::Invalid(format!("SHP 头 zero={zero},应为 0")));
        }
        let cx = r.read_u16_le()?;
        let cy = r.read_u16_le()?;
        let frame_count = r.read_u16_le()? as usize;
        if frame_count == 0 || frame_count > 10000 {
            return Err(FormatError::Invalid(format!(
                "SHP 帧数 {frame_count} 超出 1..=10000"
            )));
        }
        let mut headers = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            let x = r.read_u16_le()? as i16;
            let y = r.read_u16_le()? as i16;
            let fh_cx = r.read_u16_le()?;
            let fh_cy = r.read_u16_le()?;
            let flags = r.read_u32_le()?;
            let rgba = [r.read_u8()?, r.read_u8()?, r.read_u8()?, r.read_u8()?];
            let reserved = r.read_u32_le()?;
            let data_offset = r.read_u32_le()?;
            headers.push(ShpTsFrameHeader {
                x,
                y,
                cx: fh_cx,
                cy: fh_cy,
                flags,
                rgba,
                reserved,
                data_offset,
            });
        }
        let table_end = r.position(); // = 8 + 24*frame_count
        let file_len = data.len();
        for h in &headers {
            let empty = h.cx == 0 && h.cy == 0 && h.data_offset == 0;
            if empty {
                continue;
            }
            if h.cx == 0 || h.cy == 0 || h.data_offset == 0 {
                return Err(FormatError::Invalid(
                    "帧矩形与数据偏移必须同时为 0(空)或同时非 0".to_string(),
                ));
            }
            if (h.data_offset as usize) < table_end || (h.data_offset as usize) >= file_len {
                return Err(FormatError::Invalid(format!(
                    "帧数据偏移 {} 越界(表尾 {table_end},文件长 {file_len})",
                    h.data_offset
                )));
            }
            if i32::from(h.x) + i32::from(h.cx) > i32::from(cx)
                || i32::from(h.y) + i32::from(h.cy) > i32::from(cy)
            {
                return Err(FormatError::Invalid(format!(
                    "帧矩形 ({},{},{},{}) 超出画布 {cx}x{cy}",
                    h.x, h.y, h.cx, h.cy
                )));
            }
        }
        let mut frames = Vec::with_capacity(frame_count);
        for h in headers {
            let empty = h.cx == 0 && h.cy == 0 && h.data_offset == 0;
            let pixels = if empty {
                Vec::new()
            } else {
                let start = h.data_offset as usize;
                let need = h.cx as usize * h.cy as usize;
                if need > MAX_SHP_CANVAS_PIXELS {
                    return Err(FormatError::Invalid(format!(
                        "SHP 画布 {}x{} 共 {need} 像素,超分配前上界 {MAX_SHP_CANVAS_PIXELS}",
                        h.cx, h.cy
                    )));
                }
                let mut pixels = Vec::with_capacity(need);
                if h.is_rle() {
                    // bit1=1 → RLE 行链,区间 [offset, 文件尾):引擎无 region
                    // 界(GetPixels 直给缓冲指针,shapipe.cpp:103-112;行链可
                    // 越过下帧 offset,研究 §2.2),count 链是唯一权威;EOF
                    // 触碰仍 Err。M1b 的 ends[] 下帧偏移链界废止(研究 §2.4-4:
                    // 修复版全集 0 败、rows_over_width=0,文件尾为界足够)。
                    rle_zero_ts_decode(&data[start..], h.cx as usize, h.cy as usize, &mut pixels)?;
                } else {
                    // bit1=0 → RAW:取 [offset, offset+need);offset+need 越文
                    // 件尾时补零截断(语料 0 出现,防御位——lenient 零 panic,
                    // 敌意输入不再整文件拒;RAW 帧普遍带 2-8B 尾 pad,旧
                    // `span==need` 硬条件废止,draw.cpp:88-98 Bit_Blit)。
                    let take = (start + need).min(file_len);
                    pixels.extend_from_slice(&data[start..take]);
                    pixels.resize(need, 0);
                }
                pixels
            };
            frames.push(ShpTsFrame { header: h, pixels });
        }
        Ok(ShpTsFile {
            zero,
            cx,
            cy,
            frames,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn frame_header(
        x: i16,
        y: i16,
        cx: u16,
        cy: u16,
        flags: u32,
        rgba: [u8; 4],
        reserved: u32,
        offset: u32,
    ) -> [u8; 24] {
        let mut b = [0u8; 24];
        b[0..2].copy_from_slice(&x.to_le_bytes());
        b[2..4].copy_from_slice(&y.to_le_bytes());
        b[4..6].copy_from_slice(&cx.to_le_bytes());
        b[6..8].copy_from_slice(&cy.to_le_bytes());
        b[8..12].copy_from_slice(&flags.to_le_bytes());
        b[12..16].copy_from_slice(&rgba);
        b[16..20].copy_from_slice(&reserved.to_le_bytes());
        b[20..24].copy_from_slice(&offset.to_le_bytes());
        b
    }

    fn shp_bytes(canvas: (u16, u16), headers: &[[u8; 24]], body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&0u16.to_le_bytes()); // zero
        v.extend_from_slice(&canvas.0.to_le_bytes());
        v.extend_from_slice(&canvas.1.to_le_bytes());
        v.extend_from_slice(&(headers.len() as u16).to_le_bytes());
        for h in headers {
            v.extend_from_slice(h);
        }
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn parses_frame_and_decodes_rle() {
        // 旧 tmp.rs parses_tiles_and_decodes_rle 的迁移(burnt01 形态):
        // 帧头 flags=3(旧 unknown_a)、rgba=[0,0,1,0](旧 unknown_d=1 的蓝通道)。
        // 勘误(实现时实测):brief 原稿只列 1 个帧头却断言 data_offset=56
        // ——甲骨文 56=8+24·2 需要两个帧头才闭合,单帧形态表尾 32、文件长 43,
        // offset 检查直接 Err("帧数据偏移 56 越界")。按迁移源(旧测试
        // 图形项 + 空项两项)与 burnt01 甲骨文恢复两帧形态,断言随之取 2。
        let body: Vec<u8> = vec![
            0x06, 0x00, 0x31, 0x00, 0x01, 0x01, 0x05, 0x00, 0x32, 0x00, 0x04,
        ];
        let data = shp_bytes(
            (60, 60),
            &[
                frame_header(10, 20, 4, 2, 3, [0, 0, 1, 0], 0, 56),
                frame_header(0, 0, 0, 0, 4, [0; 4], 0, 0),
            ],
            &body,
        );
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!((f.cx, f.cy), (60, 60));
        assert_eq!(f.frames.len(), 2);
        let t = &f.frames[0];
        assert_eq!((t.header.x, t.header.y), (10, 20));
        assert_eq!(t.header.flags, 3);
        assert_eq!(t.header.rgba, [0, 0, 1, 0]);
        assert_eq!(t.header.reserved, 0);
        assert_eq!(t.header.data_offset, 56);
        assert_eq!(t.pixels, vec![0x31, 0, 0x01, 0, 0x32, 0, 0, 0]);
        // 空帧:像素为空,flags 仍非 0(勿以 flags 判空)
        assert_eq!(f.frames[1].header.flags, 4);
        assert!(f.frames[1].pixels.is_empty());
    }

    #[test]
    fn rejects_bad_zero_field() {
        let mut data = shp_bytes(
            (60, 60),
            &[frame_header(0, 0, 4, 2, 3, [0; 4], 0, 56)],
            &[0u8; 8],
        );
        data[0..2].copy_from_slice(&9u16.to_le_bytes());
        assert!(matches!(
            ShpTsFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_zero_frames() {
        let data = shp_bytes((60, 60), &[], &[]);
        assert!(matches!(
            ShpTsFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_inconsistent_empty_frame() {
        // cx>0 但 data_offset=0:三字段必须同时为 0 或同时非 0
        let data = shp_bytes((60, 60), &[frame_header(0, 0, 4, 2, 3, [0; 4], 0, 0)], &[]);
        assert!(matches!(
            ShpTsFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_rect_out_of_canvas() {
        let data = shp_bytes(
            (60, 60),
            &[frame_header(57, 0, 4, 2, 3, [0; 4], 0, 56)],
            &[0u8; 8],
        );
        assert!(matches!(
            ShpTsFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_offset_before_table() {
        let data = shp_bytes(
            (60, 60),
            &[frame_header(0, 0, 4, 2, 3, [0; 4], 0, 16)],
            &[0u8; 8],
        );
        assert!(matches!(
            ShpTsFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    #[test]
    fn raw_gated_by_flags_bit1_zero() {
        // M4c 引擎对齐:flags=1 → bit1=0 → 门控直取 RAW(不经"RLE 失败
        // 兜底"路径;draw.cpp:88-98 Bit_Blit,spec §5.2 重述)
        let body = [0x02, 0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let data = shp_bytes((8, 2), &[frame_header(0, 0, 4, 2, 1, [0; 4], 0, 32)], &body);
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!(f.frames[0].pixels, body.to_vec());
    }

    #[test]
    fn rle_gated_by_flags_bit1_one() {
        // flags=3 → bit1=1 → RLE 行链(引擎分派 HasCompression → RLE_Blit,
        // shapipe.cpp:148-156);全透明帧的链 span 也可恰 == cx·cy,门控不看
        // span——语义是"bit1 门控",非旧"RLE 优先试探"(spec §5.2 重述)
        let body = [0x04, 0x00, 0x00, 0x04];
        let data = shp_bytes((4, 1), &[frame_header(0, 0, 4, 1, 3, [0; 4], 0, 32)], &body);
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!(f.frames[0].pixels, vec![0, 0, 0, 0]);
    }

    #[test]
    fn err_when_rle_chain_touches_eof() {
        // RLE 链截断(块 count 声明越过文件尾)→ UnexpectedEof;链区间已改
        // [offset, 文件尾),数据不足语义不变(spec §5.2 重述)
        let body = [0x08, 0x00, 0xAA, 0xBB, 0xCC, 0xDD];
        let data = shp_bytes((8, 2), &[frame_header(0, 0, 4, 2, 3, [0; 4], 0, 32)], &body);
        assert!(matches!(
            ShpTsFile::parse(&data),
            Err(FormatError::UnexpectedEof { .. })
        ));
    }

    #[test]
    fn rejects_canvas_over_allocation_cap() {
        // 敌意 ~40 B blob 谎报画布 65535×65535(u16² ≈ 4.29e9 像素):
        // 分配前上界拦截,恒 Err,不得先做巨型 Vec 预留
        let data = shp_bytes(
            (65535, 65535),
            &[frame_header(0, 0, 65535, 65535, 3, [0; 4], 0, 32)],
            &[0u8; 8],
        );
        assert!(matches!(
            ShpTsFile::parse(&data),
            Err(FormatError::Invalid(_))
        ));
    }

    // ===== M4c 合成字节级 oracle(spec §6.2;模式复刻研究 §5.3,零 EA 字节)=====

    /// a. RAW + 尾 pad(DRUM01 模式):11×14 flags=0x01,span=160 > need=154,
    /// 尾部 6B pad 不入帧;同型 flags=0x00(GEM09 2×2 span 8 / need 4)。
    #[test]
    fn oracle_raw_with_tail_pad() {
        // DRUM01 模式:11×14 = 154 像素,帧体 160B(154 数据 + 6 pad)
        let mut body = vec![0x5Au8; 154];
        body.extend_from_slice(&[0xCC; 6]);
        let data = shp_bytes(
            (11, 14),
            &[frame_header(0, 0, 11, 14, 0x01, [0; 4], 0, 32)],
            &body,
        );
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!(f.frames[0].pixels.len(), 154, "pad 不入帧");
        assert!(f.frames[0].pixels.iter().all(|&b| b == 0x5A));
        // GEM09 模式:2×2 = 4 像素,span 8(4 数据 + 4 pad),flags=0x00
        let body2 = [0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00];
        let data2 = shp_bytes(
            (30, 42),
            &[frame_header(21, 30, 2, 2, 0x00, [0; 4], 0, 32)],
            &body2,
        );
        let f2 = ShpTsFile::parse(&data2).unwrap();
        assert_eq!(f2.frames[0].pixels, vec![0x01, 0x01, 0x01, 0x01]);
    }

    /// b. 同文件混变体(CIV1 模式):f0 flags=0x03 RLE 行链、f1 flags=0x01
    /// RAW——帧级门控钉死,任何文件级格式选择不可行(研究 §1.3-A)。
    #[test]
    fn oracle_mixed_variants_in_one_file() {
        // table_end = 8 + 24·2 = 56;f0 RLE 4B @56、f1 RAW 2B @60,背靠背
        let body = [0x04, 0x00, 0x05, 0x06, 0x07, 0x08];
        let data = shp_bytes(
            (4, 1),
            &[
                frame_header(0, 0, 2, 1, 0x03, [0; 4], 0, 56),
                frame_header(0, 0, 2, 1, 0x01, [0; 4], 0, 60),
            ],
            &body,
        );
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!(f.frames[0].pixels, vec![0x05, 0x06], "f0 bit1=1 → RLE");
        assert_eq!(f.frames[1].pixels, vec![0x07, 0x08], "f1 bit1=0 → RAW");
    }

    /// c. 窄行 RLE(CAFNCB f048 模式):1×1 flags=0x02,行块 `03 00 <px>`
    /// → 单像素(依赖 Task 2 下限 2)。
    #[test]
    fn oracle_narrow_row_rle_count_three() {
        let body = [0x03, 0x00, 0x2A];
        let data = shp_bytes(
            (1, 1),
            &[frame_header(0, 0, 1, 1, 0x02, [0; 4], 0, 32)],
            &body,
        );
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!(f.frames[0].pixels, vec![0x2A]);
    }

    /// d. 判定面:flags=0x04 → RAW(bit1=0);flags=0xCCCC0003 → 只读 +8
    /// 字节按 RLE(高半字为匿名填充不参与);flags=0xCCCC0001 → RAW;
    /// flags u32 原样保留。空帧 flags≠0 跳过另见
    /// oracle_empty_frame_with_nonzero_flags 与 parses_frame_and_decodes_rle。
    #[test]
    fn oracle_flag_bitmask_dispatch() {
        let mk = |flags: u32, body: &[u8]| {
            shp_bytes(
                (2, 1),
                &[frame_header(0, 0, 2, 1, flags, [0; 4], 0, 32)],
                body,
            )
        };
        // 0x04:bit1=0 → RAW
        let f = ShpTsFile::parse(&mk(0x04, &[0x11, 0x12])).unwrap();
        assert_eq!(f.frames[0].header.flags, 0x04, "flags u32 原样保留");
        assert_eq!(f.frames[0].pixels, vec![0x11, 0x12]);
        // 0xCCCC0003:低字节 0x03 bit1=1 → RLE(行块 count=4 → 2 字面量)
        let f = ShpTsFile::parse(&mk(0xCCCC_0003, &[0x04, 0x00, 0x21, 0x22])).unwrap();
        assert_eq!(f.frames[0].header.flags, 0xCCCC_0003);
        assert_eq!(f.frames[0].pixels, vec![0x21, 0x22]);
        // 0xCCCC0001:低字节 0x01 bit1=0 → RAW
        let f = ShpTsFile::parse(&mk(0xCCCC_0001, &[0x31, 0x32])).unwrap();
        assert_eq!(f.frames[0].pixels, vec![0x31, 0x32]);
    }

    /// d/e. 空帧约定先于 flags 分派:flags≠0(cx==cy==offset==0)仍按空帧
    /// 跳过(空帧可带非 0 flags,勿以 flags 判空)。
    #[test]
    fn oracle_empty_frame_with_nonzero_flags() {
        let data = shp_bytes(
            (4, 2),
            &[frame_header(0, 0, 0, 0, 0xCCCC_0001, [0; 4], 0, 0)],
            &[],
        );
        let f = ShpTsFile::parse(&data).unwrap();
        assert!(f.frames[0].pixels.is_empty(), "空帧不进 flags 分派");
    }

    /// e. 逐帧像素期望:flags=0x02 RLE 帧体仅 `02 00`(count=2 空行)→
    /// 整行透明;count=0/1 → Err 在 rle_zero_ts.rs 单测钉(rejects_count_below_two)。
    #[test]
    fn oracle_rle_count_two_empty_row_transparent() {
        let body = [0x02, 0x00];
        let data = shp_bytes(
            (4, 1),
            &[frame_header(0, 0, 4, 1, 0x02, [0; 4], 0, 32)],
            &body,
        );
        let f = ShpTsFile::parse(&data).unwrap();
        assert_eq!(f.frames[0].pixels, vec![0, 0, 0, 0]);
    }

    /// f. 敌意面:RAW 谎报 need 超文件尾 → 补零截断零 panic(新防御位;
    /// 旧判别式整文件拒);表前偏移/画布越界/验零维持由既有测试
    /// rejects_offset_before_table / rejects_rect_out_of_canvas /
    /// rejects_bad_zero_field / rejects_canvas_over_allocation_cap 覆盖。
    #[test]
    fn oracle_raw_need_over_eof_zero_pads() {
        // 100×100 flags=0x01,文件体仅 5B → 前 5B 数据 + 9995 零,零 panic
        let body = [0x11, 0x22, 0x33, 0x44, 0x55];
        let data = shp_bytes(
            (100, 100),
            &[frame_header(0, 0, 100, 100, 0x01, [0; 4], 0, 32)],
            &body,
        );
        let f = ShpTsFile::parse(&data).unwrap();
        let px = &f.frames[0].pixels;
        assert_eq!(px.len(), 10_000);
        assert_eq!(&px[..5], &body);
        assert!(px[5..].iter().all(|&b| b == 0), "越尾部分补零");
    }

    /// f. 敌意面:RLE 行链越文件尾 → UnexpectedEof(链区间 [offset, 文件尾)
    /// 触界仍 Err,零 panic)。
    #[test]
    fn hostile_rle_chain_over_eof_is_eof_error() {
        // 行块 count=0x0100 声明 256B,文件体仅 3B → UnexpectedEof
        let body = [0x00, 0x01, 0xAA];
        let data = shp_bytes(
            (4, 2),
            &[frame_header(0, 0, 4, 2, 0x02, [0; 4], 0, 32)],
            &body,
        );
        assert!(matches!(
            ShpTsFile::parse(&data),
            Err(FormatError::UnexpectedEof { .. })
        ));
    }
}
