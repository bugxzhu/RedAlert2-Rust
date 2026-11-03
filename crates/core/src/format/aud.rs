// SPDX-License-Identifier: GPL-3.0-only

//! AUD 音频(IMA ADPCM,compression=0x63)。
//!
//! 布局(研究文档 2026-11-03-csf-aud-layouts.md §2;XCC `aud_file.cpp` /
//! `aud_decode.cpp` 源码逐行核对,text1.aud 全样本验证):
//! 12 字节头 `{u16 samplerate(8000..=48000); u32 size_in(全部块头+载荷);
//! u32 size_out(解码 PCM 字节); i8 flags(bit0 立体声,bit1 16-bit);
//! i8 compression}`。随后背靠背块:`{u16 size_in(载荷字节); u16 size_out
//! (解码字节); i32 id=0x0000DEAF(XCC 从不校验,仅记录)}` + 载荷。
//!
//! IMA 解码要点:
//! - **步长表是 XCC/Westwood 变体,不是标准 IMA 表**(index 27 为 97 非 98,
//!   尾项 32767 非 32762;XCC aud_decode.cpp:8-20 原样嵌入,勿"修正");
//! - 低半字节先(偶数样本取低半字节);
//! - 预测器状态 (index, sample) **文件级、跨块持续**(init(0,0) 一次;
//!   块内无前导,与 WAV IMA 不同;按块复位会从第二块起出垃圾)。
//!
//! 范围外(显式 Unsupported):立体声(flags bit0)、compression=1(WS 编解码器)。

use super::bytes::ByteReader;
use super::FormatError;

pub const COMPRESSION_IMA: u8 = 0x63;
/// 块 id 的固定魔数 0x0000DEAF(XCC 从不校验,仅记录;公开供调用方比对)。
pub const CHUNK_ID: i32 = 0x0000_DEAF;

/// XCC `aud_decode.cpp:8-20` 的步长表(Westwood 变体,勿改成"标准"IMA 表)。
const IMA_STEP_TABLE: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449,
    494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272,
    2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493,
    10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];
const IMA_INDEX_ADJUST: [i32; 8] = [-1, -1, -1, -1, 2, 4, 6, 8];

/// AUD 文件头(存储字段)。
pub struct AudHeader {
    pub sample_rate: u16,
    pub size_in: u32,
    pub size_out: u32,
    pub flags: u8,
    pub compression: u8,
}

/// 块头(存储字段)。
pub struct AudChunkHeader {
    pub size_in: u16,
    pub size_out: u16,
    pub id: i32,
}

/// 解析并解码后的 AUD。
pub struct Aud {
    pub header: AudHeader,
    pub chunks: Vec<AudChunkHeader>,
    /// 解码后的单声道样本(16-bit PCM,每样本一个 i16)。
    pub samples: Vec<i16>,
}

impl Aud {
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        if data.len() < 12 {
            return Err(FormatError::UnexpectedEof {
                needed: 12,
                remaining: data.len(),
            });
        }
        let mut r = ByteReader::new(data);
        let sample_rate = r.read_u16_le()?;
        let size_in = r.read_u32_le()?;
        let size_out = r.read_u32_le()?;
        let flags = r.read_u8()?;
        let compression = r.read_u8()?;
        if !(8000..=48000).contains(&sample_rate) {
            return Err(FormatError::Invalid(format!(
                "采样率 {sample_rate} 超出 8000..=48000"
            )));
        }
        if size_in as usize != data.len() - 12 {
            return Err(FormatError::Invalid(format!(
                "size_in={size_in} 与文件长不符(应为 {})",
                data.len() - 12
            )));
        }
        if flags & 1 != 0 {
            return Err(FormatError::Unsupported("立体声 AUD 暂不支持".to_string()));
        }
        if compression != COMPRESSION_IMA {
            return Err(FormatError::Unsupported(format!(
                "暂不支持的 AUD 压缩 0x{compression:02X}(仅 IMA 0x63;WS 编解码器不在本里程碑)"
            )));
        }
        if flags & 0xFE != 2 {
            return Err(FormatError::Invalid(format!(
                "IMA AUD flags={flags} 应为 2(16-bit mono)"
            )));
        }
        // 块游走 + 解码(状态跨块持续,init(0,0) 一次)
        let mut chunks = Vec::new();
        let mut samples: Vec<i16> = Vec::new();
        let mut index: i32 = 0;
        let mut sample: i32 = 0;
        let mut sp = 12usize;
        while sp < data.len() {
            if data.len() - sp < 8 {
                return Err(FormatError::UnexpectedEof {
                    needed: 8,
                    remaining: data.len() - sp,
                });
            }
            let size_in_chunk = u16::from_le_bytes([data[sp], data[sp + 1]]);
            let size_out_chunk = u16::from_le_bytes([data[sp + 2], data[sp + 3]]);
            let id = i32::from_le_bytes([data[sp + 4], data[sp + 5], data[sp + 6], data[sp + 7]]);
            let payload_start = sp + 8;
            let payload_end = payload_start + size_in_chunk as usize;
            if payload_end > data.len() {
                return Err(FormatError::UnexpectedEof {
                    needed: size_in_chunk as usize,
                    remaining: data.len() - payload_start,
                });
            }
            let payload = &data[payload_start..payload_end];
            chunks.push(AudChunkHeader {
                size_in: size_in_chunk,
                size_out: size_out_chunk,
                id,
            });
            // 每样本 4-bit 码,低半字节先;状态持续
            for i in 0..(size_out_chunk as usize / 2) {
                let byte = payload
                    .get(i >> 1)
                    .ok_or_else(|| FormatError::Invalid("块载荷不足以解码全部样本".to_string()))?;
                let code = if i % 2 == 0 { byte & 0x0F } else { byte >> 4 };
                let step = IMA_STEP_TABLE[index as usize];
                let mut delta = step >> 3;
                if code & 1 != 0 {
                    delta += step >> 2;
                }
                if code & 2 != 0 {
                    delta += step >> 1;
                }
                if code & 4 != 0 {
                    delta += step;
                }
                if code & 8 != 0 {
                    sample -= delta;
                    if sample < -32768 {
                        sample = -32768;
                    }
                } else {
                    sample += delta;
                    if sample > 32767 {
                        sample = 32767;
                    }
                }
                index = (index + IMA_INDEX_ADJUST[(code & 7) as usize]).clamp(0, 88);
                samples.push(sample as i16);
            }
            sp = payload_end;
        }
        // 总量交叉校验(研究文档 §2.6.7 建议)
        let sum_in: usize = chunks.iter().map(|c| 8 + c.size_in as usize).sum();
        let sum_out: usize = chunks.iter().map(|c| c.size_out as usize).sum();
        if sum_in != size_in as usize {
            return Err(FormatError::Invalid(format!(
                "块 size_in 总和 {sum_in} 与头 size_in={size_in} 不符"
            )));
        }
        if sum_out != size_out as usize || samples.len() * 2 != size_out as usize {
            return Err(FormatError::Invalid(format!(
                "块 size_out 总和 {sum_out}(解码 {} 字节)与头 size_out={size_out} 不符",
                samples.len() * 2
            )));
        }
        Ok(Aud {
            header: AudHeader {
                sample_rate,
                size_in,
                size_out,
                flags,
                compression,
            },
            chunks,
            samples,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aud_bytes(chunks: &[(&[u8], u16)], flags: u8, compression: u8) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&22050u16.to_le_bytes());
        let size_in: u32 = chunks.iter().map(|(p, _)| 8 + p.len() as u32).sum();
        let size_out: u32 = chunks.iter().map(|(_, o)| *o as u32).sum();
        v.extend_from_slice(&size_in.to_le_bytes());
        v.extend_from_slice(&size_out.to_le_bytes());
        v.push(flags);
        v.push(compression);
        for (payload, out) in chunks {
            v.extend_from_slice(&(payload.len() as u16).to_le_bytes());
            v.extend_from_slice(&out.to_le_bytes());
            v.extend_from_slice(&CHUNK_ID.to_le_bytes());
            v.extend_from_slice(payload);
        }
        v
    }

    #[test]
    fn decodes_two_samples_from_one_byte() {
        // index=0(step=7),code 7(低半字节):delta=0+1+3+7=11 → sample 11;
        // index→8(step 16),code 0:delta=2 → sample 13
        let data = aud_bytes(&[(&[0x07], 4)], 2, COMPRESSION_IMA);
        let aud = Aud::parse(&data).unwrap();
        assert_eq!(aud.samples, [11, 13]);
        assert_eq!(aud.chunks.len(), 1);
        assert_eq!(aud.chunks[0].id, 0x0000_DEAF);
        assert_eq!(aud.header.sample_rate, 22050);
    }

    #[test]
    fn state_carries_across_chunks() {
        // 块 0 末状态 (index=7, sample=13);块 1 首字节 0x77:
        // step=14:delta=1+3+7+14=25 → 38;再 step=31:delta=3+7+15+31=56 → 94
        // 若按块复位状态,块 1 会从 (0,0) 重新解码,得不到 [38, 94]
        // (勘误 2026-11-03:brief 原写块 1 载荷 0x07,但其第二个样本取高半字节
        // =0,只能得 41 而非 94;注释的 delta=3+7+15+31 要求两半字节均为 code 7,
        // 故按注释本意改为 0x77。已手工按步长表逐步复算验证。)
        let data = aud_bytes(&[(&[0x07], 4), (&[0x77], 4)], 2, COMPRESSION_IMA);
        let aud = Aud::parse(&data).unwrap();
        assert_eq!(aud.samples, [11, 13, 38, 94]);
        assert_eq!(aud.chunks.len(), 2);
    }

    #[test]
    fn rejects_stereo() {
        let data = aud_bytes(&[(&[0x07], 4)], 3, COMPRESSION_IMA); // flags bit0=1
        assert!(matches!(
            Aud::parse(&data),
            Err(FormatError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_non_ima_compression() {
        // WS(=1)不在本里程碑
        let data = aud_bytes(&[(&[0x07], 4)], 0, 1);
        assert!(matches!(
            Aud::parse(&data),
            Err(FormatError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_flags_compression_mismatch() {
        // IMA 要求 bit1 置位(flags 2/3);flags=0 为畸形
        let data = aud_bytes(&[(&[0x07], 4)], 0, COMPRESSION_IMA);
        assert!(matches!(Aud::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_size_in_mismatch() {
        let mut data = aud_bytes(&[(&[0x07], 4)], 2, COMPRESSION_IMA);
        let bad = 10u32.to_le_bytes(); // 实际 8+1=9
        data[2..6].copy_from_slice(&bad);
        assert!(matches!(Aud::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn rejects_rate_out_of_range() {
        let mut data = aud_bytes(&[(&[0x07], 4)], 2, COMPRESSION_IMA);
        data[0..2].copy_from_slice(&1000u16.to_le_bytes());
        assert!(matches!(Aud::parse(&data), Err(FormatError::Invalid(_))));
    }

    #[test]
    fn accepts_empty_audio() {
        let data = aud_bytes(&[], 2, COMPRESSION_IMA);
        let aud = Aud::parse(&data).unwrap();
        assert!(aud.chunks.is_empty());
        assert!(aud.samples.is_empty());
    }
}
