# SHP (TS/RA2 sprite) and TMP (theater tile) byte-level layouts

Date: 2026-11-03
Status: verified against XCC source (GPL, `triatomic/XCC-Tacitus` @ master) and 5 real samples from the project's MIX extracts.
Audience: implementers of Rust parsers in ra2-rs. Byte-exactness beats prose; every fact carries a provenance tag.

## 0. Sources and method

- **XCC source** (downloaded via GitHub contents API into a temp dir, read-only):
  - `misc/cc_structures.h` — struct definitions
  - `misc/shp_ts_file.h` / `misc/shp_ts_file.cpp` — TS/RA2 SHP reader
  - `misc/shp_decode.cpp` — RLEZeroTS + LCW codecs
  - `misc/tmp_ts_file.h` / `misc/tmp_ts_file.cpp` — TS TMP reader
  - `Mixer/XCCFileView.cpp` — SHP playback path (confirms compressed-frame decoder choice)
  - `TMP Editor/XCC TMP EditorDoc.cpp` — writes TS TMPs (confirms field semantics)
- **Samples** (`E:\hongjing\ra2-rs\fixtures\samples\`, SHA-256 recorded in §6):
  - `pwrlvl.shp` (48 B), `mindanimr.shp` (1144 B), `gunfirey.shp` (4000 B)
  - `burnt01.sno` (520 B), `lobrdb08.sno` (3360 B) — RA2 snow-theater tiles
- All decoding claims were checked by executing parsers (Python, in a temp dir) over the samples; arithmetic shown inline.

## 1. SHP — TS/RA2 sprite format (`.shp`)

This is the format XCC calls `shp_ts` (`Cshp_ts_file`). RA1/Dune2 SHP (`t_shp_header`, `t_shp_dune2_*`, `Ctmp_file`/RA1 TMP) is a different family and is **not** covered beyond the compression note in §1.4.

### 1.1 File header — 8 bytes, all fields little-endian

Source: XCC `misc/cc_structures.h:284-290`:

```c
struct t_shp_ts_header
{
    __int16 zero;
    __int16 cx;
    __int16 cy;
    __int16 c_images;
};
```

| Off | Size | Type | XCC name | Meaning |
|----|------|------|----------|---------|
| 0x00 | 2 | i16 | `zero` | must be 0 (validated `shp_ts_file.cpp:94`) |
| 0x02 | 2 | u16 | `cx` | canvas width in pixels |
| 0x04 | 2 | u16 | `cy` | canvas height in pixels |
| 0x06 | 2 | u16 | `c_images` | frame count; XCC accepts 1..=10000 (`shp_ts_file.cpp:95`) |

No magic value. The only structural anchor is `zero == 0` plus self-consistency.

### 1.2 Frame header — 24 bytes each, immediately after file header

Array starts at offset 8, stride 24, `c_images` entries. Source: XCC `misc/cc_structures.h:292-305` + accessor `shp_ts_file.h:79-82`:

```c
struct t_shp_ts_image_header
{
    __int16 x;
    __int16 y;
    __int16 cx;
    __int16 cy;
    __int32 flags;
    unsigned __int8 red;
    unsigned __int8 green;
    unsigned __int8 blue;
    unsigned __int8 alpha;
    __int32 zero;
    __int32 offset;
};
```

| Off | Size | Type | XCC name | Meaning |
|----|------|------|----------|---------|
| +0x00 | 2 | i16 | `x` | left edge of frame inside canvas |
| +0x02 | 2 | i16 | `y` | top edge of frame inside canvas |
| +0x04 | 2 | u16 | `cx` | frame width |
| +0x06 | 2 | u16 | `cy` | frame height |
| +0x08 | 4 | u32 | `flags` | bitfield; **bit 1 (value 2) = frame data is RLEZeroTS-compressed** (XCC `shp_ts_file.h:69-72`: `flags & 2`). Bit 0 (value 1): set on every frame of every sample, incl. empty ones — semantics unknown, XCC ignores it |
| +0x0C | 3 | u8×3 | `red,green,blue` | per-frame RGB triple (avg/tint color). Read by XCC, unused in decode paths |
| +0x0F | 1 | u8 | `alpha` | observed 0 in all samples |
| +0x10 | 4 | i32 | `zero` | must be 0 (`shp_ts_file.cpp:105`) |
| +0x14 | 4 | u32 | `offset` | absolute file offset of frame pixel data |

Frame data region: file offsets `8 + 24*c_images` onward. XCC requires `offset >= 8 + 24*c_images` (`shp_ts_file.cpp:106`).

**Empty frame convention**: `cx == cy == offset == 0` → frame is skipped (XCC `shp_ts_file.cpp:101`). Note the flags field is still nonzero (observed 3) on empty frames — do not use `flags == 0` as the emptiness test. Real example: `gunfirey.shp` frames 4-6.

### 1.3 Frame pixel data

- **flags bit1 clear (uncompressed)**: exactly `cx*cy` raw palette-index bytes at `offset`, row-major. (XCC `shp_ts_file.cpp:113-116` bounds check.)
- **flags bit1 set (compressed)**: `cy` chained blocks, one per scanline. Each block:
  - `u16 count` — byte length of the block **including these 2 bytes** (XCC reads `count - 2` data bytes, `shp_decode.cpp:754-755`)
  - `count-2` data bytes, decoded as: `v != 0` → one literal palette pixel; `v == 0` → next `u8` is a zero-run length (transparent pixels)
  - Zero-runs are **clamped** at the row width `cx` (XCC `shp_decode.cpp:769-771`); remaining bytes of the block are still consumed as tokens.
  - This is XCC `RLEZeroTSDecompress` (`shp_decode.cpp:748-778`), the same routine the Mixer SHP player uses (`XCCFileView.cpp:2646`).

This is **NOT LCW/Format-80**. `LCWDecompress` (`shp_decode.cpp:632`) is used for Dune2/RA1-era formats (and `shp_dune2_file` writing picks LCW vs encode5 by `format == 80`, `shp_decode.cpp:1121`), never for TS/RA2 `.shp`. The existing `crates/core/src/format/lcw.rs` is therefore not the decoder for these frames; a `rle_zero_ts` decoder is needed. (It *is* the same row-block scheme as RA2 TMP tiles, §2.3, so one routine can serve both.)

**Encoder quirks observed in samples (handle in decoder):**
1. When a scanline ends in transparent pixels, the encoder emits one final zero-run that is **1 longer than the remaining width** (rows decode to `cx+1` pixels before clamping). Observed on every file: mindanimr rows sum to `cx` or `cx+1`; lobrdb08 rows to `120` or `121` with width 120.
2. Compressed frame data does not necessarily reach the next frame's `offset`: 2-7 bytes of zero padding can follow the last row block (`mindanimr` f0: rows end 0x131, next frame 0x138; `gunfirey` f3: ends 0xd6c, next data 0xd70). The `count`-chained rows are authoritative; treat everything up to the next frame offset as padding.

### 1.4 Edge cases

- **0-frame SHP**: rejected by XCC (`c_images < 1` → invalid, `shp_ts_file.cpp:95`). No sample. Rust: treat `frames == 0` as an error (or accept with a warning — no data would follow the 8-byte header).
- **1-frame minimal**: `pwrlvl.shp` — header (8) + 1 frame header (24) + 16 raw bytes = 48 B file. Works fine.
- **flags semantics summary**: bit1 = RLEZeroTS compression; bit0 observed set on 100% of frames including empty ones (cannot be "is shadow"); flags == 0 never observed but harmless (uncompressed). There is **no** "compression == 2" algorithm here — that enum lore (0=raw, 1/2/3=LCW variants) belongs to Dune2 SHP (`t_shp_dune2_image_header.compression`, `cc_structures.h:263-271`).
- **Shadow frames**: XCC does NOT use flags to identify shadows; it pairs frames by position (`i >= c_images>>1` is the shadow half, `shp_ts_file.cpp:153-159`). `gunfirey.shp` f7 (`rgb=(0,0,0)`) is a shadow by appearance; nothing in the header says so except the color triple.
- Canvas bounds: every non-empty frame must satisfy `x+cx <= header.cx` and `y+cy <= header.cy` (XCC `shp_ts_file.cpp:103-104`).

### 1.5 Sample verification (arithmetic)

Sample heads match the task's known-facts line for line.

**pwrlvl.shp** (48 B) — `e0837c15…`
- hdr: zero=0, cx=8, cy=2, frames=1
- table end = 8 + 24·1 = 32 = 0x20; f0.offset = 0x20 ✓ (data starts exactly after table)
- f0: x=0 y=0 cx=8 cy=2 flags=0x1 rgb=(255,0,0) zero2=0 offset=0x20
- uncompressed (bit1 clear): needs 8·2 = 16 B → 0x20+16 = 0x30 = 48 = file size ✓ **exact EOF fit**
- pixels: `fe`×10 `00`×4 `fe`×2 (16 B) — 4 transparent pixels inside a 1-frame sprite
  (勘误 2026-11-04: 原记录 "`fe`×9 `00`×4 `fe`×3" 系转写笔误; 经哈希钉死样本直读复核为 `fe`×10 `00`×4 `fe`×2)

**mindanimr.shp** (1144 B = 0x478) — `ef4617cf…`
- hdr: zero=0, cx=30, cy=22, frames=8
- table end = 8 + 24·8 = 200 = 0xC8 = f0.offset ✓
- 8 frames, all flags=0x3 (compressed), zero2=0, offsets strictly increasing:
  f0 (7,5,14,8)@0xC8, f1 (7,5,15,9)@0x138, f2 (6,5,17,9)@0x1C0, f3 (4,4,20,11)@0x260, f4 (5,4,19,11)@0x320, f5 (10,7,8,5)@0x3B0, f6 (9,6,10,6)@0x3E8, f7 (8,6,12,7)@0x428
- row chaining per frame: f0 8 rows 0xC8→0x131 (+7 pad → 0x138 ✓), f1 9 rows 0x138→0x1BE (+2 pad), f2 9 rows 0x1C0→0x259 (+7 pad), f3 11 rows 0x260→0x320 (+0 pad), f4 11 rows 0x320→0x3AE (+2), f5 5 rows 0x3B0→0x3E1 (+7), f6 6 rows 0x3E8→0x422 (+6), f7 7 rows 0x428→0x475 (+3 → EOF 0x478 ✓)
- row pixel counts ∈ {cx, cx+1} per frame (clamp quirk §1.3); rows decoded = cy exactly in all 8 frames ✓
- bounds: max(x+cx)=24≤30, max(y+cy)=15≤22 ✓

**gunfirey.shp** (4000 B = 0xFA0) — `4425f5bd…`
- hdr: zero=0, cx=97, cy=94, frames=8
- table end = 200 = 0xC8 = f0.offset ✓
- f0 (0,2,97,90)@0xC8, f1 (0,2,97,90)@0x448, f2 (0,2,97,90)@0x850, f3 (2,31,94,32)@0xC18, f4-f6 empty (0,0,0,0,flags=3,offset=0), f7 (1,0,94,94)@0xD70 rgb=(0,0,0)
- chains: f0 90 rows 0xC8→0x446 (+2 pad → 0x448 ✓), f1 90 rows 0x448→0x84D (+3 → 0x850 ✓), f2 90 rows 0x850→0xC14 (+4 → 0xC18 ✓), f3 32 rows 0xC18→0xD6C (+4 → 0xD70 ✓), f7 94 rows 0xD70→0xF9B (+5 → EOF 0xFA0 ✓)
- bounds: f3 y+cy = 63 ≤ 94, x+cx = 96 ≤ 97; f7 x+cx = 95 ≤ 97 ✓
- rgb triples: flashes near-white (227,250,251)/(175,215,248)/(187,222,246)/(235,240,252), shadow black — consistent with a muzzle-flash anim + shadow frame

XCC `is_valid` (shp_ts_file.cpp:89-120) passes for all three: zero=0 ✓, 1≤frames≤10000 ✓, offsets ≥ 200 ✓, per-frame zero2=0 ✓, uncompressed bound check ok (pwrlvl) ✓.

## 2. TMP — RA2 theater tile format (`.sno`/`.tem`/`.urb`/… from RA2 mixes)

### 2.1 WARNING: XCC's TS TMP structures do NOT match RA2 theater files

XCC models theater TMPs as (`misc/cc_structures.h:356-362`):

```c
struct t_tmp_ts_header
{
    __int32 cblocks_x;
    __int32 cblocks_y;
    __int32 cx;
    __int32 cy;
};
```

Reading `burnt01.sno` through that struct gives `cblocks_x = 0x00780000`, `cx = 0x0040002C` — both nonsense, and XCC's own `is_valid` (`tmp_ts_file.h:16-34`, requires `cx == 48 || cx == 60`, `2*cy == cx`) rejects the file. **The RA2 isometric theater TMP is a different container from the TS TMP.** The layout below was reverse-engineered from the two `.sno` samples and is fully self-consistent for both; community memory of "48x60 diamond tiles with 52-byte headers" applies to TS only. Where XCC and samples conflict, the samples win (and win here).

The TS layout is still documented in §2.5 for contrast (ra2-rs may want both some day; TS tile data is uncompressed, RA2 tile data is RLEZeroTS).

### 2.2 RA2 TMP file header — 8 bytes, little-endian

| Off | Size | Type | Meaning | Sample evidence |
|----|------|------|---------|-----------------|
| 0x00 | 2 | u16 | always 0 | both samples |
| 0x02 | 2 | u16 | canvas **width** W | burnt01: 120; lobrdb08: 180 |
| 0x04 | 2 | u16 | canvas **height** H | burnt01: 120; lobrdb08: 120 |
| 0x06 | 2 | u16 | tile entry **count** N | burnt01: 2; lobrdb08: 6 |

W-ordering proof: lobrdb08's only graphic rect is x=30, cx=120 → needs W ≥ 150, so W must be the field holding 180 (offset 0x02); H=120 ≥ y+cy = 89 ✓.

Tile-entry table starts at offset 8; stride is **24 bytes**; pixel data starts at `8 + 24*N`:
- burnt01: 8 + 24·2 = 56 = 0x38 = tile0.data_offset ✓
- lobrdb08: 8 + 24·6 = 152 = 0x98 = tile1.data_offset ✓
(Linear fit `data_start = 8 + 24*N` holds exactly for both samples.)

### 2.3 RA2 TMP tile entry — 24 bytes

Reverse-engineered; XCC has no RA2 TMP reader. Field semantics marked **(observed)** are grounded only in these 2 files.

| Off | Size | Type | Meaning | burnt01 t0 | lobrdb08 t0 | lobrdb08 t1 |
|----|------|------|---------|-----------|-------------|-------------|
| +0x00 | 2 | i16 | rect x in canvas | 44 | 0 | 30 |
| +0x02 | 2 | i16 | rect y in canvas | 64 | 0 | 20 |
| +0x04 | 2 | u16 | rect width = RLE row width | 29 | 0 | 120 |
| +0x06 | 2 | u16 | rect height = RLE row count | 17 | 0 | 69 |
| +0x08 | 4 | u32 | **unknown (observed)**: 3 on most entries, 4 on lobrdb08 t0 | 3 | 4 | 3 |
| +0x0C | 2 | u16 | zero (observed) | 0 | 0 | 0 |
| +0x0E | 2 | u16 | **unknown (observed)**: 0 on empty entries; 1 (burnt01 graphic), 6 (lobrdb08 graphic). Possibly ramp/terrain/elevation class | 1 | 0 | 6 |
| +0x10 | 4 | u32 | **suspected radar/minimap RGB** (3×u8 + pad) (observed): 0 except lobrdb08 t0 = `00 fb 12 00` = bright green | 0 | green | 0 |
| +0x14 | 4 | u32 | absolute file offset of this tile's RLE data; **0 = no data (empty tile)** | 0x38 | 0 | 0x98 |

Empty-tile convention: rect (0,0,0,0) with data_offset 0. lobrdb08 has 5 empty entries of 6; burnt01 has 1 of 2.

### 2.4 RA2 TMP tile pixel data — RLEZeroTS, identical scheme to SHP

`cy` chained blocks (one per rect scanline), each `u16 byte_count` (inclusive) followed by `count-2` bytes: literal pixel byte, or `00` + `u8` zero-run length, clamped at rect width. Same decoder as §1.3 (XCC `RLEZeroTSDecompress`, `shp_decode.cpp:748-778`) with `cx` = rect width, `cy` = rect height. The same encoder quirks apply (+1 trailing zero-run; trailing padding before EOF).

### 2.5 TS TMP (contrast, from XCC only — no TS sample)

- Header 16 B: `cblocks_x:i32, cblocks_y:i32, cx:i32 (48|60), cy:i32 (=cx/2)` — `cc_structures.h:356-362`; validated `tmp_ts_file.h:16-34`.
- Index: `cblocks_x*cblocks_y` **i32 absolute offsets**, 0 = absent tile — `tmp_ts_file.h:184-187`.
- Tile header `t_tmp_image_header` 52 B — `cc_structures.h:364-388`: `x, y, extra_ofs, z_ofs, extra_z_ofs, x_extra, y_extra, cx_extra, cy_extra` (9×i32); bitfield u32 with **bit0 has_extra_data, bit1 has_z_data, bit2 has_damaged_data**; `height:i8, terrain_type:i8, ramp_type:i8`; radar colors: left `RGB` + right `RGB` (6×u8); `pad[3]`.
- Pixel data: raw (uncompressed) isometric diamond of `cx*cy/2` bytes directly after the 52-byte header (`get_image = header+1`, `tmp_ts_file.h:161-164`; `get_cb_diamond = cx*cy>>1`, `:151-154`). Diamond row expansion: start x=cx/2, width 0; top half rows grow width by 4 and shift x by −2; bottom half shrink — `decode_tile`, `tmp_ts_file.cpp:109-135`.
- z (height) data at `index[i]+z_ofs`; TMP Editor writes `z_ofs = cb_diamond + sizeof(t_tmp_image_header)` and `extra_ofs = 2*cb_diamond + sizeof(header)` (`XCC TMP EditorDoc.cpp:73-74`), extra-z after extra.

### 2.6 Sample verification (arithmetic)

**burnt01.sno** (520 B) — `bc1a2370…`
- hdr: 0, W=120, H=120, N=2; table 0x08..0x37; data @0x38 (matches tile0.offset ✓)
- t0 @0x08: rect (44,64,29,17), u32@+8=3, +0x0E=1, radar 0, offset 0x38; t1 @0x20: empty, offset 0
- RLE chain: 17 rows, 0x38 → 0x207 (last row count 11 @0x1FC); 1 trailing byte `00` at 0x207
- rows decode to 29 or 30 px (clamp at width 29) ✓; 17 rows = rect height ✓
- bounds: 44+29 = 73 ≤ 120 ✓; 64+17 = 81 ≤ 120 ✓
- decoded 29×17 image = compact blob (scorch mark), fully consumed `fully=True`

**lobrdb08.sno** (3360 B) — `7a1309fc…`
- hdr: 0, W=180, H=120, N=6; table 0x08..0x97; data @0x98 (matches tile1.offset ✓)
- t0 @0x08: empty, u32@+8=4, radar bytes `00 fb 12 00`, offset 0
- t1 @0x20: rect (30,20,120,69), u32@+8=3, +0x0E=6, offset 0x98
- t2..t5 @0x38/0x50/0x68/0x80: empty, offset 0
- RLE chain: 69 rows, 0x98 → 0xD1E (last row @0xD16 count 8); 2 trailing bytes `00 00`
- rows decode to 120 or 121 px (clamp at 120) ✓; 69 rows = rect height ✓
- bounds: 30+120 = 150 ≤ 180 ✓; 20+69 = 89 ≤ 120 ✓
- decoded 120×69 image = clean isometric diamond (verified by nonzero-pixel mask: grows then shrinks symmetrically) — a 2×2-tile deck plate

### 2.7 Data anomaly to be aware of

`lobrdb08.sno` returned **different bytes** in two reads during this session (8 zero bytes at 0x90-0x97 absent in the first read, present afterwards; size and mtime unchanged; header fields identical in both). The current on-disk state is the one fully consistent with its own header (data at 0x98 per tile1.offset) and is what §2.6 verifies; the first read was almost certainly a torn snapshot while fixtures were being written. If a parser test ever fails at offset 0x90 vs 0x98, re-hash the file. Current SHA-256: `7a1309fc…` (§6).

## 3. Rust-facing notes (no code)

**SHP reader**
- `ShpHeader { zero: u16, cx: u16, cy: u16, frames: u16 }`, `ShpFrameHeader { x: i16, y: i16, cx: u16, cy: u16, flags: u32, rgb: [u8; 3], _alpha: u8, _zero: u32, offset: u32 }` — `#[repr(C, packed)]`-equivalent manual reads, all LE; do NOT `zerocopy` a struct over the buffer without checking padding assumptions (the 24-byte layout has no implicit padding if typed as above, but manual reads are safest).
- Error cases: `zero != 0`; `frames == 0` (XCC-rejected); frame table exceeds file; `offset < 8+24*frames` or `> file_len`; `offset` fields not sorted (tolerate, but sort for streaming); uncompressed frame `offset + cx*cy > file_len`; compressed frame row `count < 4` or block crossing file end / next frame offset; `x+cx > canvas.cx || y+cy > canvas.cy`; `image.zero != 0`.
- Empty frame test: `cx == 0 && cy == 0 && offset == 0` (NOT flags).
- Compressed decode: `cy` row blocks; clamp zero-runs at row width; keep consuming tokens inside a block even after clamp (XCC-faithful); tolerate `cx+1` spill and inter-frame zero padding. Output buffer exactly `cx*cy`.
- One shared `rle_zero_ts::decode(&[u8], cx, cy) -> Vec<u8>` serves SHP frames and RA2 TMP tiles.

**RA2 TMP reader**
- `TmpRa2Header { zero: u16, width: u16, height: u16, tiles: u16 }`, `TmpRa2Tile { x: i16, y: i16, cx: u16, cy: u16, unknown_a: u32, _b: u16, unknown_d: u16, radar: [u8; 3], _pad: u8, data_offset: u32 }` (勘误 2026-11-04: 原清单多一个 `_c` u16,与 24 字节步长及 +0x10 radar 实测矛盾,已删;实现与真库 3 测通过背书)。
- Errors: `zero != 0`; `tiles == 0`; `8 + 24*tiles > file_len`; rect fields nonzero but `data_offset == 0` (or vice versa: offset != 0 with cx*cy == 0); rect out of canvas; RLE chain breaks.
- Empty tile = `cx == 0 && cy == 0 && data_offset == 0` — treat as "present but draws nothing" (the entry still exists; do not compact the table).
- Compose tiles onto the W×H canvas at their (x, y) rects; the graphic pieces are arbitrary rects (one file had a 120×69 diamond inside a 180×120 canvas), not per-tile diamonds.
- Do **not** reuse a TS-TMP parser for RA2 files (different header, index, compression). If TS support is added later, gate on a discriminator: TS header reads as two i32 "cblocks" with cx ∈ {48,60} — RA2 files fail that.

**Unknowns to keep behind named fields (not `reserved: [u8; N]`)**: SHP `flags` bit0; TMP `+0x08` u32, `+0x0E` u16, `+0x10` radar RGB.

## 4. Facts that contradict common lore / task assumptions

1. **TS/RA2 `.shp` compressed frames do not use LCW/Format-80** — they use XCC's `RLEZeroTS` row scheme (`shp_decode.cpp:748`). LCW remains relevant only for Dune2/RA1 SHP (and the existing `crates/core/src/format/lcw.rs` stays useful there).
2. **RA2 theater `.sno` TMPs are not TS TMPs** — no 16-byte i32 header, no diamond-table index, no 52-byte tile headers; they are 8-byte header + 24-byte rect entries + RLEZeroTS data.
3. **TMP tile graphics can be compressed** — TS TMPs are raw; the RA2 ones observed here are RLE-compressed (that's why a 120×69 tile fits in ~3.2 KB).
4. "flags bit0 = shadow" is not usable: bit0 was set on every frame of every SHP sample including empty and body frames; XCC identifies shadows by frame position instead.

## 5. Provenance index

- XCC `misc/cc_structures.h:284-290` → SHP file header fields.
- XCC `misc/cc_structures.h:292-305` → SHP frame header fields (names, sizes, order).
- XCC `misc/shp_ts_file.h:69-72` → compression test `flags & 2`; `:79-82` → frame header stride/location; `:94-97` → max frame size formula.
- XCC `misc/shp_ts_file.cpp:89-120` → validation rules (zero, frame range, offset bounds, empty-frame skip at :101).
- XCC `misc/shp_ts_file.cpp:42-48,146-152` → compressed frames decoded via `RLEZeroTSDecompress`, raw frames used directly.
- XCC `misc/shp_decode.cpp:748-778` → RLEZeroTS row format (u16 inclusive count, zero-run escape, clamp at :769-771); `:632-…` → LCW (not used for TS SHP); `:1121` → Dune2 format-80 branch.
- XCC `Mixer/XCCFileView.cpp:2636-2647` → Mixer SHP playback calls `RLEZeroTSDecompress` for `flags & 2` frames.
- XCC `misc/cc_structures.h:356-362,364-388` → TS TMP header + tile header (incl. extra/z flag bits at :375-377, radar bytes).
- XCC `misc/tmp_ts_file.h:16-34` → TS TMP validation (cx 48/60, cy=cx/2); `:151-154` diamond size; `:161-172` image/z placement; `:184-187` index.
- XCC `misc/tmp_ts_file.cpp:109-135` → diamond row expansion; `:55-107` full draw incl. extra data.
- XCC `TMP Editor/XCC TMP EditorDoc.cpp:73-74` → canonical z_ofs/extra_ofs layout for TS TMP tiles.
- Sample facts: every number in §1.5 and §2.6 is tagged `sample <name>@<offset>` implicitly by its own row (offsets given in hex); SHA-256 in §6 pins the exact bytes analyzed.

## 6. Sample fingerprints

| File | Size | SHA-256 (first 16 hex) | Role |
|------|------|------------------------|------|
| pwrlvl.shp | 48 | e0837c1520ff9538 | minimal 1-frame uncompressed SHP |
| mindanimr.shp | 1144 | ef4617cf48248f0c | 8-frame compressed SHP, padding gaps |
| gunfirey.shp | 4000 | 4425f5bdf013e1a9 | 8-frame SHP incl. 3 empty + shadow |
| burnt01.sno | 520 | bc1a23700e17563f | RA2 TMP, 1 graphic rect 29×17 |
| lobrdb08.sno | 3360 | 7a1309fc8c192d82 | RA2 TMP, 1 graphic rect 120×69 (diamond), 5 empty entries |
