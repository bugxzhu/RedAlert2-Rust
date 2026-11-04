# Map pack sections and binary data: IsoMapPack5 / OverlayPack / PreviewPack / theater chain

Date: 2026-11-04
Status: verified against XCC source (GPL, `triatomic/XCC-Tacitus`), the Linux kernel `lzo1x` reference decoder, `gamemd.exe` and the raw MIX archives of a real YR install, and the full local map corpus (540 `.map` files). Every decoding claim below was executed over real files; arithmetic is shown inline. Byte-exactness beats prose; every fact carries a provenance tag.
Audience: implementers of the M2 map loader in ra2-rs. Companion documents: `2026-11-04-map-ini-survey.md` (INI side of the same files), `2026-11-03-shp-tmp-layouts.md` (TMP/SHP codecs referenced here), `2026-11-03-mix-format.md` (MIX container).

## 0. Sources and method

- **XCC source** (downloaded via GitHub into a temp dir, read-only, never executed):
  - `misc/shp_decode.cpp` — `LCWCompress`/`LCWDecompress` (:632-710), pack chunk loop `decode5` (:1127-1155), `encode5s`/`decode5s` = miniLZO wrappers
  - `misc/map_ts_encoder.cpp` — per-section codec choice (`encode5` call sites), `overlay_decode4`/`overlay_data_decode4` (:134-241), `preview_decode4`, `write_map` (:560-612), diamond walk helpers
  - `misc/cc_structures.h` — `t_iso_map_pack_entry` (:207-216), `t_mix_header` union (:163-172)
  - `misc/mix_file_rd.cpp`, `misc/mix_decode.cpp` — MIX index reading, TS-style encryption path
- **Linux kernel** `lib/lzo/lzo1x_decompress_safe.c` (GPL-2.0, downloaded reference, never executed) — normative description of the LZO1X variant miniLZO produces.
- **gamemd.exe** (4813072 B, from the install): theater definition table at file offset 4070264-4070930, `.PAL` filename strings, `[PublicKey]` string at 4070016.
- **Live install** `E:\hongjing\Reunion 2023`: `ra2.mix`, `ra2md.mix`, `thememd.mix`, `expandmd01.mix`, `langmd.mix`, `language.mix`, `ares.mix` parsed directly from disk (see §7.1 — they are plaintext TS-style MIX files; the identity `10 + 12·c_files + size == filesize` holds exactly on all seven).
- **Map corpus** `E:\hongjing\Reunion 2023\Maps\**\*.map` — 540 files (sibling survey counts 807 including RandomMapGenerator and FinalAlert2SP copies; all claims here were re-run over this repo's 540-file view).
- **Method**: throwaway Python decoders in a temp dir (`pack_probe.py` = container+codecs, `probe2/3/4.py` = record/grid/preview analysis, `probe5.py` = parallel corpus sweep, `mix_decrypt.py`/`mix_ls.py` = MIX tools). Hand decoding was used only to generate hypotheses; a hand-decode mistake early in this research produced two wrong conclusions (per-chunk header size, overlay stream order) that the scripts corrected — see §10.

## 1. Pack sections in the INI — container level

The four binary sections of a `.map` are stored as normal INI sections with numbered keys:

| Section | Key format | Decoded content |
|---|---|---|
| `[IsoMapPack5]` | `1=`,`2=`,… | compressed diamond tile stream (§3) |
| `[OverlayPack]` | `1=`,`2=`,… | compressed 512×512 overlay id grid (§4) |
| `[OverlayDataPack]` | `1=`,`2=`,… | compressed 512×512 overlay frame grid (§4) |
| `[PreviewPack]` | `1=`,`2=`,… | compressed raw RGB minimap bitmap (§5) |

- Values are base64 text wrapped at ≤70 characters per key; **concatenation of all values in key order is one standard base64 stream** (RFC 4648 alphabet `A-Za-z0-9+/`, `=` padding allowed and must be skipped). Provenance: sibling survey §4.5 (0 charset violations corpus-wide); verified by executing the concat+decode over all 540 maps.
- Corpus anomalies a loader must tolerate: 7 maps end `[PreviewPack]` with a truncated base64 group (GBK-named DDLY maps, sibling survey §4.5) — decode what is present, drop the partial tail group, treat missing preview bytes as "no preview" rather than an error. In this 540-file view **all 540 carry `[IsoMapPack5]`** (the sibling survey's wider 807-file corpus contains one map without it — treat a missing section as "empty flat map", not an error); 3 GBK-named maps have no `[Digest]` (names in §12).
- Observed key-count ranges (sibling survey §4.5): IsoMapPack5 1..9568, PreviewPack 1..3090, OverlayPack 1..224, OverlayDataPack 1..370.

## 2. Chunk container and per-section codec

The decoded base64 bytes are a sequence of **chunks**:

| Off | Size | Type | Meaning |
|---|---|---|---|
| 0 | 2 | u16 LE | `size_in` — payload bytes following this header |
| 2 | 2 | u16 LE | `size_out` — bytes this chunk decompresses to; **≤ 8192 for every encoder-produced chunk** |
| 4 | `size_in` | bytes | codec payload |

The next chunk starts at `4 + size_in`. XCC reference: `decode5(s, cb_s, format)` at `shp_decode.cpp:1127-1155`; `write_map` at `map_ts_encoder.cpp:560-612` encodes overlays as fixed 32×8192 chunks and tiles/previews as `encode5` chunks.

**Codec selection is per section** (empirically confirmed by decoding all 540 maps — see §12; the wrong codec errors on chunk 0 of every file):

| Section | Codec | `format` argument in XCC |
|---|---|---|
| `[IsoMapPack5]` | **LZO1X** (miniLZO) | 5 |
| `[PreviewPack]` | **LZO1X** | 5 |
| `[OverlayPack]` | **LCW** | 80 |
| `[OverlayDataPack]` | **LCW** | 80 |

The "5" in *IsoMapPack5* **is the codec id**, not a version number. Final chunk of a stream may decode to fewer than 8192 bytes (e.g. c1a01md `[IsoMapPack5]` chunks are 16×8192 + 4685 = 135117).

### 2.1 LCW codec (overlays)

Identical opcode table in XCC `shp_decode.cpp:632-710` and repo `crates/core/src/format/lcw.rs`:

| Opcode | Meaning | Encoding length |
|---|---|---|
| `op & 0x80 == 0` | **relative** back-copy, `count = (op>>4)+3`, `offset = ((op&0xF)<<8) \| next_byte` | 2 |
| `0x80` | end of stream | 1 |
| `0x81-0xBF` | literal run, `count = op & 0x3F` | 1+count |
| `0xC0-0xFD` | **absolute** copy, `count = (op&0x3F)+3`, offset u16 LE | 3 |
| `0xFE` | long fill, count u16 LE, fill byte | 4 |
| `0xFF` | long absolute copy, count u16 LE, offset u16 LE | 5 |

Copies may reach before the output start (decoders copy from a zeroed buffer; c1a01md chunk 0 relies on this).

**Worked example** — first chunk of `c1a01md.map` `[OverlayPack]`, base64 bytes:

```
09 00 | 00 20 | 81 FF FE FE 1F FF 81 FF 80      header: size_in=9, size_out=0x2000=8192
   81 FF        literal run, count 1:            out[0]     = FF
   FE FE 1F FF  long fill, count 0x1FFE = 8190:  out[1..8190] = FF
   81 FF        literal run, count 1:            out[8191]  = FF
   80           end
```

9 payload bytes → 8192 bytes, all `0xFF`: this grid block holds no overlays. Chunk 1 header follows immediately: `36 00 00 20` = size_in 54, size_out 8192.

### 2.2 LZO1X codec (tiles, preview)

Normative reference: Linux kernel `lib/lzo/lzo1x_decompress_safe.c` (the "safe" variant; miniLZO output is compatible). Salient traits confirmed while porting:

- Streams may begin with a 2-byte version header `17, <ver>`; handle both with and without.
- Opcode families: literal runs (`t = byte-17` style prologue), short matches (2-byte, `M2`, offset ≤ 0x800), `M1` (2-byte + 255-extension), `M3`/`M4` (2-byte + extension runs up to ~4140); trailing `next = le16 & 3` literals after matches (state machine).
- EOF is signalled by a match opcode whose source equals its destination (`m_pos == op`, requires `t == 3` in the reference); the kernel *safe* decoder additionally reports overflow/underflow — map those to `FormatError`.
- Rust note: must be implemented from scratch (zero third-party deps); the kernel file is the only reference needed. Kernel bugfix history (CVE-2014-4608 family, 2017 hardening) is a useful checklist for input validation.

## 3. `[IsoMapPack5]` — decompressed tile stream

Decoded length: **`11·(2W−1)·H`** bytes, where `[Map]Size = 0,0,W,H` (first two integers are always 0,0 — sibling survey §4.2), **plus a 4-zero-byte tail on EA/FinalAlert-packed maps** (producer-dependent — §3.3). Exact on every corpus map carrying the section (§12). Example: c1a01md W=87,H=71 → 11·173·71+4 = 135117 ✓.

### 3.1 Record — 11 bytes (`t_iso_map_pack_entry`, XCC `cc_structures.h:207-216`)

| Off | Size | Type | XCC name | Meaning |
|---|---|---|---|---|
| 0 | 2 | u16 LE | x | diamond X, range 1..W+H−1 |
| 2 | 2 | u16 LE | y | diamond Y, range 1..W+H−1 |
| 4 | 2 | i16 LE | tile | theater tile index; **−1 = no tile** (cell on a slope edge) |
| 6 | 1 | u8 | zero1 | always 0 in corpus (34.4M records total, §12) |
| 7 | 1 | u8 | zero2 | always 0 in corpus |
| 8 | 1 | u8 | sub_tile | sub-frame within the tile (observed 0..44) |
| 9 | 1 | u8 | z | ground height, 0..14 (observed 0..12) |
| 10 | 1 | u8 | zero3 | always 0 in corpus |

### 3.2 Stream order — the XCC diamond walk

Records appear in a fixed traversal order (XCC `map_ts_encoder.cpp` walk helpers), so (x,y) per record is *implied* by position but is also stored, and the stored pair matches the walk on 536/540 corpus maps (§3.6 documents the 4 exceptions):

```
cx, cy = W, H
x_line, r_line, y = cx, cx + 1, 1
while x_line < r_line:
    for x in x_line .. r_line-1:  emit (x, y)
    x_line += -1 if y < cx else +1
    r_line += +1 if y < cy else (-1 if y > cy else 0)
    y += 1
```

Verified: record *i*'s (x,y) equals walk cell *i* on **every** record of **536/540** corpus maps — 34.4M records total (c1a01md: 12283/12283; §3.6 for the four legacy maps that omit cells; §12).

### 3.3 Terminator — producer-dependent

EA/FinalAlert-packed maps end with **4 zero bytes** after the last record (a truncated `(0,0)` record head): c1a01md last record `48 00 9d 00 00 00 00 00 00 02 00` then `00 00 00 00`. Community-packed maps (GBK-named Custom-folder files) instead end at an **exact `11·records` boundary with no tail** — same records, same order, same count identity. The invariant to enforce is:

- `decoded_len % 11 ∈ {0, 4}` (4 ⇒ tail must be `00 00 00 00`)
- `records == (2W−1)·H` — with **one documented exception class** (§3.6)

Both forms verified corpus-wide (§12); the tail split per producer is counted there too.

### 3.4 Coordinate identity — resolves the sibling survey's UNKNOWN ("tile↔cell ±1 transform")

`[Terrain]` keys are cell numbers `Y·1000 + X` over the **same** diamond coordinates as tile records — identity, no offset:

| Theater | Map | `[Terrain]` cells matching a tile-record (x,y) |
|---|---|---|
| TEMPERATE | c1a01md | 749/749 |
| SNOW | c1a03md | 1227/1227 |
| DESERT | c2s02md | 500/500 |
| URBAN | 城郊之战 | 486/486 |
| NEWURBAN | c1a02md | 1154/1154 |

(LUNAR: the corpus's single lunar map `moonpatr.map` carries no `[Terrain]` entries — nothing to test there.)

`[Structures]` field 3,4 (X,Y) hit the same space (c1a01md 65/65); `[Waypoints]` values are the same `Y·1000+X` numbers. M2 can therefore index tile/overlay/object cells in one shared diamond coordinate system.

### 3.5 Worked records

c1a01md chunk 0 payload starts `00 05 57 00 01 00 3C 01 …` (LZO opcode bytes `00 05`, then record data):

```
rec0: 57 00 | 01 00 | 3C 01 | 00 | 00 | 01 | 02 | 00   →  x=87, y=1, tile=316, sub=1, z=2
rec1: 56 00 | 02 00 | 3C 01 | 00 | 00 | 02 | 02 | 00   →  x=86, y=2, tile=316, sub=2, z=2
```

(W=87 diamond walk starts (87,1), then row y=2 from x=86 — matches.)

### 3.6 Legacy exception — TS-era official maps omit void cells (4 corpus maps)

Four Westwood-origin maps (`E:\hongjing\Reunion 2023\Maps\Standard\`: `riverram`, `frstbite`, `hillbtwn`, `isleofoades` — their `[Basic]NextScenario` values are TS campaign names like `GDI2A.map`) carry **fewer records than the walk**: 6255 vs 6283, 19898 vs 19900, 8081 vs 8092, 16557 vs 16575. Structure, verified exactly on all four:

- stored `(x,y)` pairs are **unique** and a **subset** of the walk cells (`stored − walk = ∅`);
- records match the walk one-for-one up to the first omitted cell, then stay shifted by one per omission (`first_mismatch + mismatch_total == record_count` exactly on all four: 4377+1878=6255, 8718+11180=19898, 757+7324=8081, 6145+10412=16557);
- the omitted cells cluster in a compact high-x/high-y region (e.g. riverram: x 84-93, y 69-79; frstbite: x=76, y 94-95) — void/cliff cells the TS-era packer left out.

These maps still carry the 4-zero tail. Loader contract: **treat the stored `(x,y)` as authoritative**, not the walk position; build the cell map by insertion and leave walk cells that never appear as `tile = −1`. The walk remains valid as a cross-check with `records ≤ (2W−1)·H`.

## 4. `[OverlayPack]` / `[OverlayDataPack]` — 512×512 grids

- Both decode to **exactly 262144 bytes** = 512×512 on every corpus map (§12). XCC's RA2 writer produces 32 chunks of 8192 (`map_ts_encoder.cpp:560-612`), but chunking is transport detail — treat the decoded bytes as one stream.
- **Grid index is row-major over the 512×512 grid: `x = i & 511`, `y = i >> 9`** — *not* a diamond walk, *not* `[Map]`-Size row-major. This is the exact expression XCC's own file writer fills: `overlay_decode4` does `d[x + (y << 9)] = *r++` (`map_ts_encoder.cpp:202-220`). Diamond coordinates run 1..W+H−1; grid cells outside the map's diamond are `0xFF`.
- `OverlayPack[i]` = overlay type id: `0xFF` = empty, otherwise the rules `[Overlays]` index (0..254 observed; semantics are game data, out of scope for M2).
- `OverlayDataPack[i]` = frame number for that overlay cell (0 = first frame; 0 is a *valid* value). The two layers are **independent grids**: 46/540 corpus maps (including EA's own YR campaign maps `c3y01md`/`c3y02md`/`c3y03md`/`c2s01md`/`c4w01md` and the Westwood RA2 `x*` multiplayer maps) contain cells where `data[i] != 0` while `overlay[i] == 0xFF` — 1-495 stray cells per map, e.g. c3y01md has 153. These are **stale frame bytes under later-emptied overlay cells**, tolerated by the format itself: XCC's `overlay_data_decode4` skips data bytes whose overlay byte is `0xFF` (`map_ts_encoder.cpp:221-241`) and would silently drop the strays on any round-trip. Loader contract: read `data[i]` only when `overlay[i] != 0xFF`; never treat strays as corruption (warn at most).
- Proof this is the right indexing: for each map, every non-`0xFF` cell decodes to (x,y) inside the diamond bounds under `x+(y<<9)` (c1a01md: 774/774), while the diamond-walk hypothesis misplaces all 774 (non-`0xFF` values appear at stream positions far beyond the 12283 walk slots, impossible under a compact-walk reading); the `[Map]`-Size row-major hypothesis (width 87) fails bounds for 100% of wide rows.
- **XCC reading note** — XCC's `overlay_decode4`/`overlay_data_decode4` (`map_ts_encoder.cpp:202-241`) are *internal↔file converters*, not file-order descriptions: XCC keeps overlays internally as a compact diamond-walk stream (packed by `overlay_encode4`/`overlay_data_encode4`, :174-200) and `write_map` expands them into the full `1<<18` grid before LCW (:586-593). Anyone implementing from `overlay_decode4` alone and feeding it raw file bytes gets garbage; the file side of the pair is the 512-grid, matching the files. `map_decode4` (:137-166) likewise confirms tile records: x,y derived from walk order, pad bytes written as 0, `count` asserted to cover exactly the walk (the `(2W−1)·H` identity), and z stored absolute in the file (internal form carries deltas).

## 5. `[PreviewPack]` — raw RGB bitmap

- Decodes (LZO1X) to a **raw 24-bpp RGB bitmap with no header**: length exactly `3·pw·ph`, where `[Preview]Size = 0,0,pw,ph`. c1a01md: 3·162·59 = 28674 ✓ (4 chunks); all six sampled maps exact (§13 table).
- Pixel order is row-major starting at… (row/channel order is an M3 rendering question; M2 stores the bytes verbatim — see §14 UNKNOWN).
- **Producer variants** (community GBK-named Custom maps): preview streams exist that (a) end mid-chunk — all earlier chunks decode, the final one errors (LZO `lookbehind_overrun`/`input_overrun`), and (b) carry an anomalous final chunk header (`size_out` ≫ 8192, e.g. 56994 in 城郊之战.map —— 勘误 2026-11-04:原文名"城墙之战"系同图误名,§13 勘误同源). Both are *producer bugs*, not a format variant: the correct EA-convention length is `3·pw·ph` and holds on all EA-packed maps. Loader contract: decode chunks until one fails, keep the clean prefix, degrade to "no preview" (warning, not rejection).
- **Lore contradicted**: "PreviewPack is a packed SHP" — there is no SHP header, no palette, no color count. XCC's `preview_decode4` (u16 color entries + 6-byte RGB triples) describes XCC's *internal* conversion, not the file; `write_map`'s commented-out raw write path plus the empirical length identity settle it.

## 6. `[Digest]`

Single key (`1=`) whose value is base64 decoding to **20 bytes** (28 base64 chars), e.g. c1a01md: `qSt1w6I0r+9EWkfWl7XFZMt0Vo4=`. Server-side map-validation token; 804/807 maps in the sibling survey's wider corpus carry it, 3 lack the section. ra2-rs: document and skip; no verification semantics.

## 7. Theater chain — `[Map]Theater` → files

### 7.1 The exe table (authoritative)

`gamemd.exe` file offset **4070264-4070930** holds six 8-field records (NUL-terminated Pascal-ish strings). Cross-checked against ModEnc's "Theaters" page — identical:

| INI name | base name | ISO mix name | ISO palette / YR mix | tile ext | MM ext | letter |
|---|---|---|---|---|---|---|
| TEMPERATE | TEMPERAT | ISOTEMP | ISOTEM | TEM | MMT | T |
| SNOW | SNOW | ISOSNOW | ISOSNO | SNO | MMS | A |
| URBAN | URBAN | ISOURB | ISOURB | URB | MMU | U |
| DESERT | DESERT | ISODES | ISODES | DES | MMD | D |
| NEWURBAN | URBANN | ISOUBN | ISOUBN | UBN | MMT | N |
| LUNAR | LUNAR | ISOLUN | ISOLUN | LUN | MML | L |

Field 2 of each record is the CSF UI name (`Name:Temperate`, `Name:Snow`, …). The engine resolves `[Map]Theater=<ini name>` (case-sensitive caps in corpus) to this row; the tile extension is field 6, the `.pal` names are fields 4/5 (+ `unit<letter>` for units). `.PAL` strings `TEMPERAT.PAL`, `UNITSNO.PAL`, `WAYPOINT.PAL`, `ANIM.PAL` etc. also exist as standalone exe strings (fallback/UI palettes), exe offset 4350112-4350168.

### 7.2 Verified install layout (Reunion 2023, YR 1.001)

All seven root `.mix` files parse as **plaintext TS-style MIX** (§7.3) with exact size identity. Named content (by CRC32 id, `crc.rs:49-65` algorithm):

- `ra2.mix` (22 files) — RA2 base theaters:
  - `TEMPERAT.MIX`, `URBAN.MIX` (2.7 MB raw tile blobs), `SNOW.MIX` (nested MIX, 379 files)
  - `ISOTEMP.MIX`, `ISOSNOW.MIX`, `ISOURB.MIX` — single-blob iso tile atlases, 29.2 / 28.8 / 31.8 MB
  - nested 44-file mix (id `3b5a96de`) containing the **palette set**: `temperat.pal`, `isotem.pal`, `unittem.pal`, `snow.pal`, `isosno.pal`, `unitsno.pal`, `urban.pal`, `isourb.pal`, `uniturb.pal` — each exactly 768 B = 256×RGB
  - `tem.mix`, `sno.mix`, `urb.mix` (~10.8 KB each) — nested TS-style MIXes **with the checksums flag set** (`0x00010000`, 3 files each, 20-byte SHA-1-style trailer): the only nonzero-flags mixes found in the install. `tem.mix` and `urb.mix` are **byte-identical** (md5 `aa29bf5aff0c2bfcb3301aab0f9bf232`); `sno.mix` differs (md5 `446843d5e3c8bf4184b803bb8e82085a`). Their 3 sub-files are unnamed binary tables (ids `f9cc181a`/`1641f05a`/`77431ff9` for tem; 5480/2800/2488 B of u16-structured data) — no INI text, no name-id or content-CRC match against common theater names (see §14 #2)
- `ra2md.mix` (26 files) — YR additions:
  - `SNOWMD.MIX` — nested MIX, 82 files (YR snow tile additions)
  - `DESERT.MIX`, `LUNAR.MIX` — raw tile blobs (2.8 MB); `ISODES.MIX`, `ISOLUN.MIX`, `ISOUBN.MIX` — raw atlases (35.7 / 23.5 / 32.4 MB)
  - `DES.MIX`, `LUN.MIX`, `UBN.MIX` — **byte-identical** (md5 `04220fb0bcc9d75819a9495b66492786`, 12946 B): a shared placeholder tileset for the three YR-added theaters
  - an unnamed 42-file tile MIX and a 208-file MIX holding the game config INIs (rules/art/sound text); `[PublicKey]` and XCC markers again
- `thememd.mix` (36 files) — **music themes** (contains `; THEME.INI` text), *not* theaters.
- `expandmd01.mix` (40 files, no theater names), `langmd.mix` (12), `language.mix` (8), `ares.mix` (4).

**Map for M2:** `Theater=SNOW` ⇒ tile files `.SNO`; base tiles in `SNOW.MIX` (ra2.mix) + YR additions in `SNOWMD.MIX` (ra2md.mix); iso tiles `ISOSNOW.MIX`; palettes `snow.pal`/`isosno.pal`/`unitsno.pal`. DESERT/NEWURBAN/LUNAR exist only in the YR halves (ra2md.mix). YR palettes `isodes/isoubn/isolun.pal` are **absent from this install** (probably in retail `expandmd02+.mix`) — irrelevant for M2, see §14.

### 7.3 Root MIXes are plaintext TS-style (correction to common lore, and to this research's own first pass)

| Off | Size | Type | Meaning |
|---|---|---|---|
| 0 | 4 | u32 LE | flags — `0x00020000` = encrypted index; `0x00010000` = SHA-1 checksums; **0 in this entire install** |
| 4 | 2 | u16 LE | `c_files` |
| 6 | 4 | i32 LE | `size` — body bytes after index |
| 10 | 12·`c_files` | entries | `{u32 id; u32 offset; u32 size}`, offsets **relative to data base `10 + 12·c_files`** |

Identity `10 + 12·c_files + size == filesize` holds exactly on all seven root mixes (XCC: `cc_structures.h:163-172`, `mix_file_rd.cpp:50-52`, offset rebase at `:151`). The RSA+BlowFish encrypted path (`mix_decode.cpp`) was **not needed** for roots; repo `mix.rs` rejecting encrypted indexes costs nothing here. **The checksums flag is not lore**: the nested theater mixes `tem/sno/urb.mix` carry `0x00010000` with a 20-byte trailer after the body (filesize − (10+12·c_files) − size = 20 = `cb_mix_checksum`) — a reader may need to accept (and skip) that trailer (§7.2).

## 8. Size-validation hooks (memory prevalidation)

All checks are O(text length) arithmetic before any allocation:

1. **Container bound**: decoded ≤ `6·L/8 + 2` where L = concatenated b64 chars. Section missing ⇒ documented default (`[IsoMapPack5]` absent = empty map, 1 corpus case; `[PreviewPack]` absent = no preview).
2. **Chunk chain**: need ≥ 4 bytes for a header; reject `size_in == 0` (would loop forever); reject `size_in > remaining − 4`; reject `size_out == 0` or `size_out > 8192` — except in `[PreviewPack]`, where community producers emit anomalous final chunks (§5): there, stop at the first failing chunk and keep the prefix.
3. **Aggregate bound** `out_cap = Σ size_out`, checked *before* allocating:
   - `OverlayPack` / `OverlayDataPack`: `out_cap == 262144` (reject otherwise; do not allocate-and-trim).
   - `IsoMapPack5`: expected `11·(2W−1)·H (+4 if the producer writes the zero tail)`; **accept `out_cap ≤` that** (four TS-era official maps omit void cells, §3.6) with `out_cap % 11 ∈ {0,4}`; reject anything larger (W,H from `[Map]Size`; if the INI is unavailable, fall back to `out_cap ≤ 1.98 MB` = the 300×300 bound, then verify the identity after decode).
   - `PreviewPack`: `out_cap ≤ 3·pw·ph` (truncated previews decode to *less* — accept short, reject long).
4. **Corpus-wide observed maxima** (bounds, not limits — over all 540 maps, §12;**勘误 2026-11-04:尺寸极值以 [Map] 段锚定重扫为准,见 §12 勘误块**——max W+H = 499,最大解压瓦片流 = 943,734 B(勘误二段:实测无零尾,原 943,738 系"+4 尾零"假设,见 §12);原"898/1,889,210"系 [Preview] `Size=` 误读): largest `[IsoMapPack5]` text = 669,712 b64 chars; largest `[PreviewPack]` text = 216,296 chars → ~162 KB compressed. The ini-survey's "W+H ≤ 600" cap **holds** (observed 499). Size claims must still come from `[Map]Size` per map plus a hard cap (e.g. decompressed ≤ 2 MB ⇒ W·H bounded).
5. **Allocation**: single `Vec::with_capacity(out_cap)`; decoder must error on overrun (`FormatError::UnexpectedEof`) and on `consumed != size_in` / `produced != size_out` (`FormatError::Invalid`).

## 9. Rust-facing notes (no code)

- New zero-dep module for **LZO1X decompression** beside `lcw.rs`; port from the kernel reference. Reject any match offset that reaches before output start *for LZO* (kernel-safe behavior), unlike LCW which legally pre-copies zeros (c1a01md overlay chunk 0).
- Decode pipeline per section: ordered-section lookup (`IniDocument` invariant, sibling survey §8) → concat numbered keys skipping `=`, tolerate truncated tail group → base64 → chunk walk with §8 prevalidation → per-chunk codec → exact consumption/production check.
- `IsoMapPack5` target type: `{x: u16, y: u16, tile: i16, sub_tile: u8, z: u8}` (drop the three always-zero pad bytes; treat nonzero pad as `FormatError::Invalid` — corpus never violates). Index cells by the **stored** `(x,y)` (§3.6: four legacy maps omit walk cells); missing walk cells default to `tile = −1`.
- Overlay grids: two `[u8; 262144]`-style buffers; conversion `x = i & 511, y = i >> 9` only when leaving the flat grid (iterating the diamond directly avoids per-cell branching).
- `Theater`: 6-variant enum + `FromStr` (exact caps in corpus; accept case-insensitive fallback), carrying the tile extension letter for the M3 mix lookup.
- `[Digest]`: store raw 20 bytes if present; never validated.

## 10. Facts that contradict common lore / earlier assumptions

1. **"All pack sections are LCW."** No — `[IsoMapPack5]` and `[PreviewPack]` are LZO1X; only the overlay sections are LCW.
2. **"IsoMapPack5 chunks have a 5-byte mini-header."** The header is 4 bytes (`u16 size_in, u16 size_out`); LZO's leading bytes created the +1 illusion.
3. **"Overlay streams are diamond-walk ordered / compacted"** — a natural misreading of XCC's `overlay_decode4`/`overlay_data_decode4` (they convert XCC's compact *internal* form into the full file grid; `write_map:586-593`). Real RA2 files store both overlay packs as full row-major 512×512 grids.
4. **"PreviewPack is a packed SHP / palette-based"** — raw 24-bpp RGB, no header, `3·W·H` bytes.
5. **"Root MIXes are encrypted"** — none in this install are; plaintext TS headers with exact size identity.
6. **"thememd.mix is the theater mix"** — it holds music themes (`THEME.INI`); theaters live in `ra2.mix`/`ra2md.mix`.
7. **"the IsoMapPack5 stream ends with a 4-zero-byte terminator"** — true for EA/FinalAlert-packed maps only; community tools stop at the exact record boundary (§3.3).
8. **"OverlayData must be 1:1 aligned with OverlayPack — nonzero data under `0xFF` overlay means corruption."** Real EA maps carry 1-495 stale data bytes under empty overlay cells (§4); the format ignores data under `0xFF` cells, and XCC's own converter drops them silently.

## 11. Provenance index

| Fact | Source |
|---|---|
| chunk container, per-section codec | XCC `shp_decode.cpp:1127-1155`; `map_ts_encoder.cpp` encode5/write_map call sites; executed on 540 maps (§12) |
| LCW opcode table | XCC `shp_decode.cpp:632-710`; repo `crates/core/src/format/lcw.rs` |
| LZO1X algorithm | Linux `lib/lzo/lzo1x_decompress_safe.c` |
| tile record fields | XCC `cc_structures.h:207-216`; field ranges from corpus |
| diamond walk order | XCC `map_ts_encoder.cpp` walk helpers; verified record-by-record corpus-wide |
| overlay grid model | empirical (probe4/probe3 over corpus); XCC `write_map:586-593` writes the full grid, `overlay_decode4/overlay_data_decode4:202-241` are internal↔file converters |
| preview raw RGB | empirical length identity; XCC `write_map` commented-out raw path |
| mix header/entries/encryption | XCC `cc_structures.h:163-172`, `mix_file_rd.cpp:22-55,124-151`, `mix_decode.cpp` |
| theater table | gamemd.exe 4070264-4070930; ModEnc "Theaters" (identical) |
| install content listing | direct parse of the seven root mixes (§7.1-7.3) |
| mix_name_id algorithm | repo `crates/core/src/format/crc.rs:49-65` |
| INI-side facts (cell numbers, key ranges, anomalies) | sibling `2026-11-04-map-ini-survey.md` |

Scripts (temp dir, not committed): `pack_probe.py` (container+codecs), `probe2.py`/`probe3.py` (record/grid/preview analysis + `--all` sweep), `probe4.py` (overlay position analysis), `probe5.py` (parallel full-model corpus sweep), `probe8.py` (parallel census: tail split, record counts, stray counts), `stats.py` (INI-only aggregate stats), `mix_decrypt.py` (MIX id + unused RSA/BlowFish decryptor), `mix_ls.py`.

## 12. Corpus verification

Two parallel passes over all 540 maps (`probe5.py` = full decode model; `probe8.py` = per-map census of tail kind, record counts, stray counts). Checks: per-section codec decode with exact consumption/production, chunk-chain completeness, tile record count/walk order/pad zeros, overlay grid size/bounds, data-layer strays, preview length identity.

| Check | Result |
|---|---|
| all four sections decode cleanly under the per-section codec model | 491/540 strict; **all 540 explained** by the two documented producer behaviors below — zero unexplained failures |
| wrong codec on a section (e.g. LCW for `[IsoMapPack5]`) | errors on chunk 0 of every file (per-section codec selection is unambiguous) |
| `[IsoMapPack5]` present | 540/540 in this corpus (0 missing) |
| total tile records decoded | **34,397,764** |
| `records == (2W−1)·H` | 536/540 — exceptions: `frstbite`, `hillbtwn`, `isleofoades`, `riverram` (§3.6, all short by 2/11/18/28) |
| walk order exact | 536/540 (same 4 exceptions) |
| pad bytes (offsets 6, 7, 10) nonzero | **0 of 34.4M records** |
| tail kind | **142 maps** end with the 4-zero tail (EA/FinalAlert + the 4 legacy §3.6 maps); **398 maps** end at the exact `11·records` boundary (community tools) |
| `[OverlayPack]`/`[OverlayDataPack]` length | 262144 B on 540/540 |
| non-`0xFF` overlay cells inside diamond bounds under `x+(y<<9)` | 0 violations corpus-wide |
| data strays (`data[i]≠0` under `overlay[i]=0xFF`) | 0 strays on 494 maps; **46 maps carry 1-495 strays** (median 48; max 495 `triplecrossed.map`; includes EA campaign maps — §4) |
| `[PreviewPack]` length == `3·pw·ph` | exact on every EA-packed map; **18 previews truncate mid-chunk** (kept as prefix + warning, §5) |
| `[Digest]` present | 537/540 (3 GBK-named maps lack the section) |
| theaters | TEMPERATE 217, SNOW 106, URBAN 78, DESERT 77, NEWURBAN 61, LUNAR 1 |

Corpus maxima (feeds §8): largest `[IsoMapPack5]` text = 669,712 b64 chars; largest `[PreviewPack]` text = 216,296 chars (≈162 KB compressed).

> **勘误(2026-11-04 pre-flight 实测)**:原条目另称"最大解压瓦片流 1,889,210 B(`2+1=3.map` 544×158)""最大图幅 W+H = 898(798×100)""400×200 图解压 1,757,804 B"——三数同根错误:把 **[Preview] 段的 `Size=`(预览图像素尺寸,如 顶峰相见.map 的 0,0,400,200,其真 [Map] 为 200×200)误读为 [Map] 尺寸**,再纯算术推导,未经解码实测。以 [Map] 段锚定全语料重扫(controller 复核 + 计划作者 LZO 实测双确认):**max W+H = 499**(`避暑山庄.map`/`非常之观.map`,399×100,INI 普查的 ≤600 上界因此成立);**最大解压瓦片流 = 943,738 B = 11·543·158+4**(`2+1=3.map`,真尺寸 272×158,含 4 零尾;85,794 格为全语料最大菱形格数)。计划 `2026-11-04-m2-map-terrain.md` 规格依据 §2 同裁。
>
> **勘误二段(2026-11-04 M2 终审实测)**:该图瓦片流实测 **943,734 B = 11·543·158,无 4 零尾**——上段的"+4"系生产者尾零假设的推测,经 T11 独立 Python 复测与 807 全量走查证伪。上界公式 `11·(2W−1)·H (+4 if producer writes zero tail)` 作为 bound 仍正确(§8-3 原样),只是该图不写零尾。

## 13. Sample fingerprints

| Map | Theater | Size | SHA-256 (first 16) | Bytes | IsoMapPack5 (lines/b64 B/chunks/out) | OverlayPack | OverlayDataPack | PreviewPack |
|---|---|---|---|---|---|---|---|---|
| c1a01md.map | TEMPERATE | 87×71 | `312f2905e88442bd` | 178280 | 1308/68653/17/135117 | 27/1370/32/262144 | 29/1498/32/262144 | 468/24542/4/28674 = 3·162·59 |
| c1a03md.map | SNOW | 110×80 | `1559ecef3a2b7555` | 266801 | 1858/97497/24/192724 | 31/1591/32/262144 | 35/1806/32/262144 | 764/40096/6/43056 = 3·208·69 |
| c2s02md.map | DESERT | 90×103 | `247ee2aa2c2b12d1` | 268718 | 1838/96458/25/202811 | 37/1922/32/262144 | 30/1555/32/262144 | 839/44033/6/46920 = 3·170·92 |
| c1a02md.map | NEWURBAN | 105×110 | `222a4983b9dd7c62` | 376458 | 2486/130508/31/252894 | 29/1513/32/262144 | 33/1723/32/262144 | 1093/57367/8/58200 = 3·200·97 |
| moonpatr.map | LUNAR | 60×62 | `b95c3dc97b8a71df` | 115102 | 740/38825/10/81162 | 19/951/32/262144 | 18/915/32/262144 | 330/17316/3/17316 = 3·111·52 |
| 城郊之战.map | URBAN | 90×90 | `bfa85ec60aae9841` | 189438 | 1529/80240/22/177210 (=11·16110, no tail — community producer) | 27/1368/32/262144 | 35/1815/32/262144 | 482/25067/7/48600 then anomalous final chunk (§5 producer variant) |

Tile-stream stats per sample: record counts all equal `(2W−1)·H`; `tile = −1` counts 371/503/1286/1142/52/958; z ranges 0-7 / 2-9 / 2-6 / 0-12 / 0-3 / 0-8; sub_tile max 44/23/44/44/24/27; **zero pad bytes violated in 0 records of any sample**.

MIX files cited: `ra2.mix` (336 MB, 22 entries), `ra2md.mix` (221983717 B, 26), `thememd.mix` (181204253 B, 36), `expandmd01.mix` (16018345 B, 40); contents listed in §7.2 (DES/LUN/UBN md5-checked).

## 14. UNKNOWNs and the experiments that would resolve them

1. **YR theater palettes** (`isodes.pal`, `isoubn.pal`, `isolun.pal`) are not in any mix of this install. Experiment: parse a retail YR `expandmd02.mix`/`expandmd03.mix` (same plaintext reader) and match ids. M2 impact: none (render-time only).
2. **Theater INI (tile id → TMP file mapping)** — not located in this install; needed for M3 rendering, not M2. `tem/sno/urb.mix` turned out to hold 3 unnamed binary tables each, not INI text (§7.2). Experiment: brute-force the three ids (`f9cc181a`, `1641f05a`, `77431ff9`) against a large tile-name candidate list; list `SNOW.MIX`'s 379 entries and the unnamed 42-file MIX in ra2md; or decompile the exe's theater loader (`gamemd.exe` theater-table references at 4070264).
3. **Preview pixel order** (RGB vs BGR, top-down vs bottom-up): M2 stores bytes verbatim, so it can wait. Experiment: render one preview buffer to PNG and compare with the in-game minimap.
4. **RA2 (pre-YR) `.map` files**: corpus is YR-only; codec selection could differ on original-RA2 maps. Experiment: run the same probes over an RA2-only map set.
5. **`tile` index → concrete TMP resolution** — requires #2; the binary layout itself is fully determined.
