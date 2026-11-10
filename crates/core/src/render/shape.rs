// SPDX-License-Identifier: GPL-3.0-only

//! SHP-TS 帧落画布(M4b;研究 §3/§6):索引像素、**值 0 = 透明跳过**、
//! 画布居中锚定(SHAPE_CENTER 语义:文件画布中心 = 锚点,帧按帧头
//! (x,y) 子偏移落位;60×30 overlay 帧恰覆格为自洽证,overtype.cpp:167)。
//! 越界三态:Inside/Clipped(可见部分仍写)/Outside(完全出界不写)。

use crate::format::pal::Palette;
use crate::format::shp_ts::{ShpTsFile, ShpTsFrame};
use crate::format::theater::TheaterAssets;

/// 缓存值:缺档/解析败(负缓存)/文件。
pub(crate) enum CachedShp {
    Missing,
    Fail,
    File(ShpTsFile),
}

pub(crate) enum BlitResult {
    Inside,
    Clipped,
    Outside,
}

/// 帧落画布。`center` = 画布绝对坐标锚点(TL = C − 文件半幅 + 帧头子偏移)。
pub(crate) fn blit_frame(
    rgb: &mut [u8],
    cw: usize,
    ch: usize,
    file: &ShpTsFile,
    frame: &ShpTsFrame,
    center: (i64, i64),
    palette: &Palette,
) -> BlitResult {
    let (fw, fh) = (i64::from(frame.header.cx), i64::from(frame.header.cy));
    if fw == 0 || fh == 0 {
        return BlitResult::Outside; // 空帧约定(shp_ts.rs):无像素
    }
    let dx = center.0 - i64::from(file.cx) / 2 + i64::from(frame.header.x);
    let dy = center.1 - i64::from(file.cy) / 2 + i64::from(frame.header.y);
    let (cw64, ch64) = (cw as i64, ch as i64);
    if dx >= cw64 || dy >= ch64 || dx + fw <= 0 || dy + fh <= 0 {
        return BlitResult::Outside;
    }
    let result = if dx < 0 || dy < 0 || dx + fw > cw64 || dy + fh > ch64 {
        BlitResult::Clipped
    } else {
        BlitResult::Inside
    };
    for row in 0..fh as usize {
        let py = dy + row as i64;
        if py < 0 || py >= ch64 {
            continue;
        }
        let src = &frame.pixels[row * fw as usize..(row + 1) * fw as usize];
        for (col, &idx) in src.iter().enumerate() {
            if idx == 0 {
                continue; // 透明(研究 §6)
            }
            let px = dx + col as i64;
            if px < 0 || px >= cw64 {
                continue;
            }
            let i = 3 * (py as usize * cw + px as usize);
            // 偏差记录(brief 原文 `palette.rgb[idx]` 直写 6 位原值):画布是
            // 8 位视图(M2 裁定 6 位存储/rgb8 视图换算,terrain.rs 同式),
            // 且本任务 overlay 测试的期望色即 rgb8——按 rgb8 落笔。
            rgb[i..i + 3].copy_from_slice(&palette.rgb8(idx as usize));
        }
    }
    result
}

/// 名 → 候选 "{image}.shp" → "{image}.{ext}" 首个命中的解析结果(带缓存;
/// 剧场后缀候选源自 overtype.cpp:179/186 的变体装载序;键小写归一,
/// Missing/Fail 一并缓存防重复查档)。
pub(crate) fn load_shp<'a>(
    assets: &TheaterAssets,
    cache: &'a mut super::ShpCache,
    image: &str,
    ext: &str,
) -> &'a CachedShp {
    let key = image.to_ascii_lowercase();
    if !cache.contains_key(&key) {
        let hit = [format!("{image}.shp"), format!("{image}.{ext}")]
            .iter()
            .find_map(|n| assets.lookup(n).map(|b| b.to_vec()));
        let v = match hit {
            None => CachedShp::Missing,
            Some(b) => match ShpTsFile::parse(&b) {
                Ok(f) => CachedShp::File(f),
                Err(_) => CachedShp::Fail,
            },
        };
        cache.insert(key.clone(), v);
    }
    &cache[&key]
}

/// 阴影半帧号惯例:SHP 后半 = 阴影半,阴影帧 = 本体帧 + 帧数/2
/// (cell.cpp:1393 Draw_Overlay_Shadow `frame + Get_Count()/2`;
/// terrain.cpp:282-283 同式;跨类型惯例 builtype.cpp:771-802)。
/// M4c 只钉语义不接线(变暗 DARKEN 表未钉顺延,研究 §9 L7);渲染路径
/// 不调用,为阴影批铺接口。
#[allow(dead_code)] // 旗标化:渲染接线顺延(DARKEN 表未钉,研究 §9 L7)
pub(crate) fn shadow_frame_index(frame: usize, frame_count: usize) -> Option<usize> {
    let idx = frame + frame_count / 2;
    (idx < frame_count).then_some(idx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::pal::Palette;
    use crate::format::shp_ts::{ShpTsFile, ShpTsFrame, ShpTsFrameHeader};

    fn frame(x: i16, y: i16, cx: u16, cy: u16, pixels: &[u8]) -> ShpTsFrame {
        ShpTsFrame {
            header: ShpTsFrameHeader {
                x,
                y,
                cx,
                cy,
                flags: 3,
                rgba: [0; 4],
                reserved: 0,
                data_offset: 28,
            },
            pixels: pixels.to_vec(),
        }
    }

    fn file(cx: u16, cy: u16, frames: Vec<ShpTsFrame>) -> ShpTsFile {
        ShpTsFile {
            zero: 0,
            cx,
            cy,
            frames,
        }
    }

    fn pal() -> Palette {
        let mut rgb = [[0u8; 3]; 256];
        rgb[1] = [10, 20, 30];
        rgb[2] = [40, 50, 60];
        Palette { rgb }
    }

    /// 6→8 位展开(M2 视图换算,(v<<2)|(v>>4);变量入参避免 identity_op)。
    fn c(v: u8) -> u8 {
        (v << 2) | (v >> 4)
    }

    /// 画布居中:4×4 画布 4×4 单帧 → 恰覆画布;透明 0 跳过。
    #[test]
    fn centered_full_frame_and_transparency() {
        let f = file(
            4,
            4,
            vec![frame(
                0,
                0,
                4,
                4,
                &[1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            )],
        );
        // 画布预填哨兵色 200(brief 原稿预填 0 + 断言 (0,0,0):palette 0 号
        // 恰黑,"跳过"与"画 0 号黑"不可区分——突变 0→255 谓词仍全绿,钉不住
        // 透明语义;哨兵使透明像素保持 200 才是真判据)
        let mut rgb = vec![200u8; 3 * 4 * 4];
        let r = blit_frame(&mut rgb, 4, 4, &f, &f.frames[0], (2, 2), &pal());
        assert!(matches!(r, BlitResult::Inside));
        let px = |i: usize| (rgb[3 * i], rgb[3 * i + 1], rgb[3 * i + 2]);
        assert_eq!(px(0), (c(10), c(20), c(30)), "(0,0) idx1 → rgb8 展开");
        assert_eq!(px(1), (200, 200, 200), "(1,0) 透明 0 跳过(哨兵保留)");
        assert_eq!(px(2), (c(40), c(50), c(60)), "(2,0) idx2 → rgb8 展开");
    }

    /// 子矩形偏移:画布 8×8,帧 2×2 @ (3,5) → 帧中心 = 点 + 偏移 − 画布半幅。
    #[test]
    fn subrect_offset_from_canvas_center() {
        let f = file(8, 8, vec![frame(3, 5, 2, 2, &[1, 1, 1, 1])]);
        let mut rgb = vec![0u8; 3 * 8 * 8];
        blit_frame(&mut rgb, 8, 8, &f, &f.frames[0], (4, 4), &pal());
        // 帧左上 = (4,4) − (4,4) + (3,5) = (3,5)
        let at = |x: usize, y: usize| rgb[3 * (y * 8 + x)];
        assert_eq!(at(3, 5), c(10), "帧 (0,0)");
        assert_eq!(at(4, 6), c(10), "帧 (1,1)");
        assert_eq!(at(2, 4), 0, "帧外");
        assert_eq!(at(5, 7), 0, "帧外");
    }

    /// 出界三态:完全出界 Outside;部分越界 Clipped(可见部分仍写)。
    #[test]
    fn clip_and_outside() {
        let f = file(4, 2, vec![frame(0, 0, 4, 2, &[1; 8])]);
        let mut rgb = vec![0u8; 3 * 8 * 4];
        // center (2,1):帧左上 (0,0) Inside
        assert!(matches!(
            blit_frame(&mut rgb, 8, 4, &f, &f.frames[0], (2, 1), &pal()),
            BlitResult::Inside
        ));
        // center (7,1):帧列 5..9 → 右溢 1 列 Clipped((6,1) 会恰收 Inside)
        assert!(matches!(
            blit_frame(&mut rgb, 8, 4, &f, &f.frames[0], (7, 1), &pal()),
            BlitResult::Clipped
        ));
        let (row, col) = (1usize, 7usize); // (6,1) 会恰收 Inside
        assert_eq!(rgb[3 * (row * 8 + col)], c(10), "可见列仍写");
        // center (20,1):完全出界
        assert!(matches!(
            blit_frame(&mut rgb, 8, 4, &f, &f.frames[0], (20, 1), &pal()),
            BlitResult::Outside
        ));
    }

    /// 阴影帧号规则(spec §4,纯函数渲染不接线):偶数帧数 0 → n/2;
    /// 奇数 floor(n/2);越界(含零帧数)None。
    /// 引擎出处:cell.cpp:1393 `frame + Get_Count()/2`;terrain.cpp:282-283
    /// 同式;跨类型惯例 builtype.cpp:771-802。
    #[test]
    fn shadow_frame_index_half_offset() {
        assert_eq!(shadow_frame_index(0, 4), Some(2), "偶数帧数:0 → n/2");
        assert_eq!(shadow_frame_index(1, 4), Some(3));
        assert_eq!(shadow_frame_index(2, 4), None, "2+2 ≥ 4 → 越界 None");
        assert_eq!(shadow_frame_index(3, 4), None);
        assert_eq!(shadow_frame_index(0, 5), Some(2), "奇数帧数:floor(n/2)");
        assert_eq!(shadow_frame_index(2, 5), Some(4));
        assert_eq!(shadow_frame_index(3, 5), None);
        assert_eq!(shadow_frame_index(0, 0), None, "零帧数防御");
    }
}
