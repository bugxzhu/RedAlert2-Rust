# VXL / HVA byte-level layouts (Red Alert 2 voxel models + animations)

Authoritative layout spec, grounded in the GPL XCC source (`triatomic/XCC-Tacitus`, branch
`master`, files fetched 2026-11-03) and verified against real game files extracted from
`ra2.mix -> local.mix` (E:\hongjing\Reunion 2023) plus the repo fixture
`E:\hongjing\ra2-rs\fixtures\samples\schp.hva`.

All multi-byte integers are **little-endian**. All XCC structs are compiled under
`#pragma pack(push, 1)` (XCC misc/cc_structures.h:35, pop at :546), so **packed** sizes below
are the on-disk sizes; no padding anywhere.

Provenance shorthand: `XCC cc_structures.h:456` = line 456 of `misc/cc_structures.h`;
`sample 1tnkbarl.vxl@0` = byte offset 0 of the extracted sample.

---

## 1. VXL format (`.vxl` — voxel model)

Three regions in order:

```
[ file header            802 bytes                ]  offset 0
[ section header array   28 bytes x S             ]  offset 802
[ section bodies         total = header.size      ]  offset 802 + 28*S   ("body region")
[ section tailer array   92 bytes x S             ]  offset file_size - 92*S  (ends AT EOF)
```

`S = c_section_headers = c_section_tailers`; XCC requires the two counts to be equal
(XCC vxl_file.cpp:18). Total file size is therefore `802 + 28*S + header.size + 92*S`.

### 1.1 File header — 802 bytes

Struct `t_vxl_header` (XCC cc_structures.h:456-465); palette entry `t_palette_entry { byte r,
g, b }` (XCC palette.h:7-10); magic constant `vxl_id[] = "Voxel Animation"`
(XCC cc_structures.h:494).

| Offset | Size | Type    | Name             | Meaning |
|-------:|-----:|---------|------------------|---------|
| 0      | 16   | char[16]| id               | Magic: ASCII `"Voxel Animation"` + NUL, exactly 15 chars + `\0` = 16 bytes. Hex `56 6F 78 65 6C 20 41 6E 69 6D 61 74 69 6F 6E 00`. XCC validates with `strcmp(h.id, vxl_id)` (vxl_file.cpp:16). |
| 16     | 4    | i32     | one              | Version sentinel; must be `1` (XCC vxl_file.cpp:17 `h.one != 1`). |
| 20     | 4    | u32     | c_section_headers| Number of sections (limbs). |
| 24     | 4    | u32     | c_section_tailers| Must equal `c_section_headers` (XCC vxl_file.cpp:18). |
| 28     | 4    | u32     | size             | Size in bytes of the **body region** (all sections' span lists + span data), excluding the file header, the section header array and the tailer array. XCC check: `h.size == file_size - sizeof(t_vxl_header) - c_section_headers*28 - c_section_tailers*92` (XCC vxl_file.cpp:19). Writer sets it to `w - body_start` (XCC vxl_file.cpp:307). |
| 32     | 2    | i16     | unknown          | Must be `0x1F10` (XCC vxl_file.cpp:20 `h.unknown != 0x1f10`). Not a normal table selector (that is the per-section tailer byte, see 1.4). |
| 34     | 768  | u8[768] | palette          | 256 entries x 3 bytes, per entry `r, g, b` (XCC palette.h:7-10; indexed at vxl_file.cpp:153-159, 230-238). Raw byte triplets — XCC never renders with this palette (rendering uses an external game palette, converted 18->24 bit: vxl_file.cpp:38-40); only its XIF export reads these bytes. Sample 1tnkbarl.vxl entry 0 = (255,0,246), entry 255 = (255,0,255). |

Sample verification (sample 1tnkbarl.vxl@0..33): `one=1`, `c_section_headers=1`,
`c_section_tailers=1`, `size=242`, `unknown=0x1f10`; `802 + 28*1 + 242 + 92*1 = 1164` = actual
file size. Sample 1tnk.vxl (12537 B): `size=11615`, `802 + 28 + 11615 + 92 = 12537`.

### 1.2 Section header array — 28 bytes each, at offset 802

Struct `t_vxl_section_header` (XCC cc_structures.h:467-473). `get_section_header(i)` =
`data + 802 + 28*i` (XCC vxl_file.h:42-45). The array holds ALL S headers before the first
body byte; the body region starts after the LAST header (XCC vxl_file.h:37-40:
`get_section_body() = data + sizeof(t_vxl_header) + sizeof(t_vxl_section_header) *
c_section_tailers`).

| Offset | Size | Type | Name      | Meaning |
|-------:|-----:|------|-----------|---------|
| +0     | 16   | char[16] | id    | Section (limb) name, NUL-terminated, NUL-padded. Trim at first NUL; garbage may follow the NUL (see 4.9). Samples: `"BARREL01"` (1tnkbarl.vxl@802), `"B-BODY"` (1tnk.vxl@802). |
| +16    | 4    | i32  | section_i | Section index, `== i`. Writer enforces `section_i = i` (XCC vxl_file.cpp:246) and encoder asserts it (XCC vxl_file.cpp:485). |
| +20    | 4    | i32  | one       | Must be `1` (writer sets 1, XCC vxl_file.cpp:247; reader check commented out at vxl_file.cpp:27). Samples: 1. |
| +24    | 4    | i32  | zero      | Must be `0` (XCC vxl_file.cpp:248/29). Sample: 0. |

### 1.3 Body region — per-section layout

Body region = concatenation of each section's body **in section order** (writer loop,
XCC vxl_file.cpp:252-306). Each section's body is, contiguously:

```
[ span_start list  4 * cx * cy bytes  ]  at body offset tailer.span_start_ofs
[ span_end   list  4 * cx * cy bytes  ]  at body offset tailer.span_end_ofs
[ span data        variable           ]  at body offset tailer.span_data_ofs
```

All three offsets are **body-relative** (relative to body region start = file offset
`802 + 28*S`). Writer: `span_start_list_ofs[i] = w - body_start` etc. (XCC
vxl_file.cpp:261, 264, 267). Readers: `get_span_start_list(i) = get_section_body() +
tailer.span_start_ofs` (XCC vxl_file.h:64-67), same for end list (:69-72).

All offsets in a sample section are `0, 4*cx*cy, 8*cx*cy` (lists immediately followed by
data); XCC's disabled sanity check expects `span_end_ofs - span_start_ofs ==
span_data_ofs - span_end_ofs` (XCC vxl_file.cpp:29). Samples confirm:
1tnkbarl `0/72/144` (cx*cy=18), 1tnk `0/1860/3720` (cx*cy=465).

**Span table entries** (`__int32`, XCC vxl_file.h:64-72; count per list =
`cx * cy`, equivalently `(span_end_ofs - span_start_ofs) >> 2`, XCC vxl_file.h:22-25):

- `span_start[j] == -1` and `span_end[j] == -1`: column `j` is **empty** (no voxels).
  `get_span_data` returns NULL on start == -1 (XCC vxl_file.h:52-57); writer stores
  `-1/-1` together (XCC vxl_file.cpp:288-291).
- otherwise both are **inclusive byte offsets into this section's span data**: span `j`
  occupies `span_data[span_start[j] .. span_end[j]]` inclusive; its byte length is
  `span_end[j] - span_start[j] + 1` (XCC vxl_file.h:59-62). Absolute span data pointer =
  `section_body + tailer.span_data_ofs + span_start[j]` (XCC vxl_file.h:52-57).

**Column index** `j = y * cx + x` (reader iterates y outer, x inner incrementing j from 0:
XCC vxl_file.cpp:50-54, 93-97, 173-179).

**Span data record stream** (per non-empty column; reader XCC vxl_file.cpp:54-70 and
97-110; writer XCC vxl_file.cpp:276-303):

```
z = 0
while z < cz:                       # the span always encodes exactly cz layers
    skip  : u8                      # empty voxels to advance z
    count : u8                      # voxels in this run
    z += skip
    count times:
        color : u8                  # palette index
        normal: u8                  # normal index -> normal table, see 1.5
        z += 1
    pad : u8                        # discarded by readers; == count for voxel runs,
                                    # == 0 for the final empty run
```

- Run size on disk: `3 + 2*count` bytes.
- A column whose voxels do not reach `cz` ends with a terminal empty run `(cz - z, 0, 0)`
  (writer XCC vxl_file.cpp:294-299); after it `z == cz` and the loop exits.
- Every z-layer of the column is covered by exactly one run: voxel runs + skip counts sum
  to exactly `cz`.
- Writer assigns `span_start[j] = span_start - span_data_start` and
  `span_end[j] = w - span_data_start - 1` (XCC vxl_file.cpp:300-301) — end points at the
  last `pad` byte, inclusive.

### 1.4 Section tailer array — 92 bytes each, ending AT EOF

Struct `t_vxl_section_tailer` (XCC cc_structures.h:475-492). `get_section_tailer(i)` =
`data + file_size + 92*(i - S)` = `data + file_size - 92*(S - i)` (XCC vxl_file.h:47-50):
tailer `i` occupies file bytes `[file_size - 92*(S-i), file_size - 92*(S-i) + 92)`. Tailer
of section 0 is the FIRST of the array; array ends exactly at EOF.

| Offset | Size | Type    | Name           | Meaning |
|-------:|-----:|---------|----------------|---------|
| +0     | 4    | i32     | span_start_ofs | Body-relative offset of this section's span start list (see 1.3). |
| +4     | 4    | i32     | span_end_ofs   | Body-relative offset of span end list. Also `(span_end_ofs - span_start_ofs) >> 2 == cx*cy` (XCC vxl_file.h:22-25). |
| +8     | 4    | i32     | span_data_ofs  | Body-relative offset of span data. |
| +12    | 4    | f32     | scale          | Voxel edge scale; samples: `0.0833333358168602` (= 1/12) in both 1tnkbarl.vxl@1084 and 1tnk.vxl@1084. XCC writer defaults it to 0 for files it creates (vxl_file.cpp:317); real Westwood files carry 1/12. |
| +16    | 48   | f32[3][4] | transform    | Section rest-pose transform, same 3x4 row-major layout as an HVA matrix (see 2.3). **Non-zero in real game files** — sample 1tnkbarl.vxl@1088: `[1,0,0,0.169375, 0,1,0,-1.730145, 0,0,1,11.604487]`; 1tnk.vxl: `[1,0,0,-14.48911, 0,1,0,0.03549, 0,0,1,6.08168]`. XCC's own writer fills zeros (vxl_file.cpp:318-322), so treat zeros as valid too. |
| +64    | 12   | 3 x f32 | x/y/z_min_scale | Bounding-box minimum corner in voxel units. Writer converts via `* 1000000` int round-trip (XCC vxl_file.cpp:201-206, 323-325). Sample 1tnkbarl: (3.8163554, 0.30925217, -1.2319936). |
| +76    | 12   | 3 x f32 | x/y/z_max_scale | Bounding-box maximum corner. Sample 1tnkbarl: (12.70798, 2.1940193, 0.65263605). |
| +88    | 1    | u8      | cx             | Section dimension X (columns). |
| +89    | 1    | u8      | cy             | Section dimension Y (rows). Dimensions are bytes: max 255 per axis. |
| +90    | 1    | u8      | cz             | Section dimension Z (layers). |
| +91    | 1    | i8      | unknown        | **Normal table selector** (`normal_type`): `2` selects the 36-entry TS table, anything else the 244-entry RA2/YR table (XCC vxl_normals.h:5-9, 174-184). XCC writer emits 2 (vxl_file.cpp:332). Samples: 2. See 1.5. |

Sample tailer positions: 1tnkbarl.vxl tailer @1072..1163; 1tnk.vxl tailer @12445..12536
(span data last byte @12444 — immediately adjacent).

### 1.5 Voxel color and normal

Each voxel is 2 bytes: `color` (index into the header palette, 1.1) and `normal`
(index into a fixed direction table chosen by the section's `unknown` byte):

- `unknown == 2`: TS table, 36 entries; indices `>= 36` clamp to 35.
- else: RA2/YR table, 244 entries; indices `>= 244` clamp to 243.

(XCC vxl_normals.h:170-184 `lookup()`.) The float direction vectors themselves are tabulated
in XCC vxl_normals.h:20-167 (`ts_table[36]`, `ra2_table[244]`, RGBA->float via
`n = 2*c/255 - 1`). Sample normal indices seen in 1tnkbarl.vxl spans: 15, 34, 4, 35, 6, 30.

### 1.6 XCC "vxl4" intermediate (NOT the game format)

XCC additionally defines a lossy compressed exchange format used by its own tools:
`t_vxl4_header { u32 c_sections }` and `t_vxl4_section_header { char id[16]; i32 section_i;
f32 scale; f32 x/y/z_min_scale; f32 x/y/z_max_scale; u8 cx, cy, cz; i8 unknown }`
(XCC vxl_file.cpp:443-463), where an empty column is encoded as the single byte `cz`
(vxl_file.cpp:520-521, 619-621). Do not confuse this with the on-disk `.vxl` format above.

---

## 2. HVA format (`.hva` — per-frame section transforms)

Two regions (XCC t_hva_header, cc_structures.h:141-146; matrix `t_hva_transform_matrix =
float[3][4]`, cc_structures.h:148):

```
[ file header          24 bytes          ]  offset 0
[ section name table   16 bytes x S      ]  offset 24
[ matrix table         48 bytes x F x S  ]  offset 24 + 16*S   (ends at EOF)
```

Exact size equation: `file_size == 24 + (48*F + 16) * S`, with `F > 0`, `S > 0`
(XCC hva_file.cpp:8-16 `is_valid`).

### 2.1 File header — 24 bytes

| Offset | Size | Type | Name       | Meaning |
|-------:|-----:|------|------------|---------|
| 0      | 16   | char[16] | id     | Source-path string from the 3ds max export pipeline, NOT the model name. Samples: `"D:\RA2X\MODELS\` + NUL (schp.hva@0, hex `44 3A 5C 52 41 32 58 5C 4D 4F 44 45 4C 53 5C 00`); `"C:\3DSMAX\MESHE` + NUL (1tnkbarl.hva@0, truncated by the 16-byte field). XCC's own writer puts `"NONE"` here (hva_file.cpp:63). Ignore for parsing. |
| 16     | 4    | i32  | c_frames   | Animation frame count `F`; must be > 0 (XCC hva_file.cpp:13). |
| 20     | 4    | i32  | c_sections | Section count `S`; must be > 0 (XCC hva_file.cpp:14). |

### 2.2 Section name table — 16 bytes x S, at offset 24

`get_section_id(i) = data + 24 + 16*i` (XCC hva_file.h:22-25). Fixed 16-byte slots,
NUL-terminated names that **correspond 1:1, in order, to the VXL section ids** (sample pair
1tnkbarl.vxl/"BARREL01" + 1tnkbarl.hva/"BARREL01"; 1tnk.vxl/"B-BODY" + 1tnk.hva/"B-BODY").
Bytes after the first NUL are garbage in real files (see 4.9).

### 2.3 Matrix table — 48 bytes x F x S, frame-major

Offset of matrix for section `i`, frame `j`:

```
offset = 24 + 16*S + (S*j + i) * 48
```

(XCC hva_file.h:34-37.) **Frame-major**: frame j outer, section i inner. The current XCC
accessor carries an explicit comment (hva_file.h:27-33) that an earlier section-major
version of this accessor swapped matrices between sections on the first non-zero frame, and
that frame-major is confirmed against Vengi's `HVAFormat.cpp:60`.

Each matrix is 12 f32 = 3 rows x 4 columns, **row-major** `[row][col]`: row `y` occupies
floats `[4*y .. 4*y+3]` (XCC hva_file.cpp:31-38 CSV dump iterates y in 0..2, x in 0..3 over
`[x + 4*y]`). Columns 0..2 = 3x3 rotation part, column 3 = translation (visible in samples:
translation values sit at float offsets 3, 7, 11).

Sample verification, schp.hva (360 B, F=2, S=3; matrices @72..359):

| Frame | Section | File offset | Rotation | Translation (col 3) |
|------:|--------:|------------:|----------|---------------------|
| 0 | 0 CYLINDER19 | 72  | identity | (55.0214, 1.92331, 233.498) |
| 0 | 1 CYLINDER57 | 120 | identity | (-523.366, -8.21036, 161.297) |
| 0 | 2 DUMMY01    | 168 | identity | (-0.770734, 1.80281, 2.88625) |
| 1 | 0 CYLINDER19 | 216 | 30 deg about Z: (0.866025, 0.5, ~0 / -0.5, 0.866025, ~0 / ~0, ~0, 1) | (55.0214, 1.92331, 233.498) — same as frame 0 |
| 1 | 1 CYLINDER57 | 264 | rotation | (-523.366, -8.21036, 161.297) — same as frame 0 |
| 1 | 2 DUMMY01    | 312 | identity | (-0.770734, 1.80281, 2.88625) — same as frame 0 |

This independently confirms frame-major indexing on a real file: each section keeps a
constant translation across frames while only its rotation animates — under a section-major
reading the translations would jump wildly between frames of one section.

Sample verification, 1tnkbarl.hva (88 B, F=1, S=1): matrix @40 =
`(1.0, ~0, ~0, 2.03250, ~0, 0.99999994, ~0, -20.761742, ~0, ~0, 0.99999994, 139.25385)`.

---

## 3. Sample verification summary

All decoded offsets land strictly inside the files; every span stream consumed byte-exactly.

| Check | 1tnkbarl.vxl | 1tnk.vxl | 1tnkbarl.hva | 1tnk.hva | schp.hva |
|---|---|---|---|---|---|
| Source | ra2.mix->local.mix | ra2.mix->local.mix | ra2.mix->local.mix | ra2.mix->local.mix | repo fixture |
| File size | 1164 | 12537 | 88 | 88 | 360 |
| Magic / header eq. | OK (`802+28+242+92`) | OK (`802+28+11615+92`) | OK (`24+(48*1+16)*1`) | OK | OK (`24+(48*2+16)*3`) |
| Sections S | 1 | 1 | 1 | 1 | 3 |
| Frames F | - | - | 1 | 1 | 2 |
| Section ids | BARREL01 | B-BODY | BARREL01 (matches .vxl) | B-BODY (matches .vxl) | CYLINDER19, CYLINDER57, DUMMY01 |
| Dims cx,cy,cz | 9,2,2 | 31,15,11 | - | - | - |
| Spans (total / empty) | 18 / 4 | 465 / 14 | - | - | - |
| Voxels decoded | 28 | 2227 | - | - | - |
| header.size field | 242 = actual | 11615 = actual | - | - | - |
| `unknown@32` | 0x1F10 | 0x1F10 | - | - | - |
| Tailer `scale` | 1/12 | 1/12 | - | - | - |
| Tailer transform | non-zero (rest pose) | non-zero | - | - | - |
| Tailer `unknown` | 2 | 2 | - | - | - |

Span arithmetic, sample 1tnkbarl.vxl (body region starts at file 830; span data @974):
span `j=0` occupies data bytes 0..6, raw hex `00 02 39 0F 39 22 02` = skip 0, count 2,
voxels (color 0x39=57, normal 15) and (57, 34), pad `02` == count; `span_end - span_start
+ 1 = 7` = 3 + 2*2. Span `j=5` and `j=6` are `-1/-1` (empty columns). Max
`span_end = 97` -> last span-data byte at file 1071; tailer array begins at 1072.

Span arithmetic, sample 1tnk.vxl: lists at body 0..1859 / 1860..3719, data at body 3720;
last span data byte at file 12444, tailer array at 12445..12536 — no gaps.

## 4. Edge cases

1. **Empty column** — `span_start[j] == -1` AND `span_end[j] == -1` (always paired; writer
   XCC vxl_file.cpp:288-291). Never treat -1 as an offset.
2. **Terminal empty run** — a non-empty column whose voxels stop below `cz` ends with
   `(cz - z, 0, 0)`: a run with `count == 0` (writer XCC vxl_file.cpp:294-299). Decode
   loop condition is `while z < cz`, so count-0 runs are consumed as 3 bytes and terminate
   the column.
3. **Span always covers exactly cz layers** — voxel runs + skips sum to cz; treat a decode
   that overruns the span window or fails to reach cz as corrupt input.
4. **Sections with no voxels at all** are legal: every column `-1/-1`, `span_data` byte
   length 0 (its `span_data_ofs` then equals the next section's `span_start_ofs`). The
   `DUMMY01`-style HVA sections pair with this.
5. **Multi-section bodies share one origin** — all sections' `span_*_ofs` are relative to
   the same body start (`802 + 28*S`), NOT per-section; bodies are concatenated in section
   order, and the tailer array sits after the last body (XCC vxl_file.h:39-41,
   writer vxl_file.cpp:251-307, reader tailers-at-EOF vxl_file.h:47-50).
6. **`c_section_headers == c_section_tailers` is required** (XCC vxl_file.cpp:18) but both
   fields are still stored independently.
7. **HVA frame count is independent of VXL section count** — F is the animation length
   (1tnkbarl: F=1; schp: F=2), S must match the VXL's section count and order (names align
   1:1 in the pairs above). There is no stored linkage between a .vxl and .hva file;
   pairing is by basename.
8. **HVA `id` is pipeline junk** — a 3ds-max source path truncated to 16 bytes ("D:\RA2X\MODELS\",
   "C:\3DSMAX\MESHE"), sometimes "NONE" from tools (XCC hva_file.cpp:63). Never parse it.
9. **Garbage after the first NUL in 16-byte name fields** — official files carry junk bytes
   past the terminator: 1tnk.hva section-name field hex
   `42 2D 42 4F 44 59 00 17 00 00 00 00 60 EE BF 00` = `"B-BODY\0"` + junk `17 00 00 00 00 60 EE BF 00`
   (sample 1tnk.hva@24); schp.hva names `"CYLINDER19\0\x07(\xDD\xC5"`,
   `"DUMMY01\0\x03\0\x02\0\x05\0\x08"` (schp.hva@24..71). Trim at the first NUL; do not
   validate the padding. (Counter-example: 1tnkbarl.hva@24 = `"BARREL01"` fully zero-padded.)
10. **VXL tailer transform is usually non-zero** in real Westwood files (rest pose with
    translation), despite XCC's writer emitting zeros (vxl_file.cpp:318-322). Do not assume
    zeros; do not reject zeros either.
11. **XCC writer bug to not replicate** — `hva_file_write` (XCC hva_file.cpp:87) stores
    matrices section-major (`12 * (c_frames * i + j)`), contradicting its own frame-major
    reader (hva_file.h:36); the two agree only when F == 1. Real game files are frame-major.
12. **Bounds/normal clamping** — normal indices out of table range clamp (1.5); voxel dims
    are u8 (<= 255 per axis); bounds are f32 so `x_min <= x_max` etc. can be sanity-checked
    but Westwood stores loose boxes (see sample 1tnkbarl min/max vs its cx,cy,cz=9,2,2).

## 5. Rust-facing notes (suggested shapes, no code)

- `VxlHeader { id: [u8;16], one: i32, c_section_headers: u32, c_section_tailers: u32, size: u32, unknown: i16, palette: [[u8;3];256] }` — `#[repr(C, packed)]` or manual `read_exact` slicing; validate `id == b"Voxel Animation\0"`, `one == 1`, `unknown == 0x1F10`, counts equal, size equation against actual len.
- `VxlSectionHeader { id: [u8;16], section_i: i32, one: i32, zero: i32 }`; id via "read up to first NUL", reject `section_i != i`.
- `VxlSectionTailer { span_start_ofs: i32, span_end_ofs: i32, span_data_ofs: i32, scale: f32, transform: [[f32;4];3], min: [f32;3], max: [f32;3], cx: u8, cy: u8, cz: u8, normal_type: i8 }` (92 bytes; pad-less).
- Body access model: slice the body region once (`802 + 28*S .. 802 + 28*S + size`), then per section take three sub-slices from its tailer offsets. Error cases: offsets outside body, `span_end < span_start`, `-1` in only one of the two lists, list byte length `!= 4*cx*cy`.
- Span decode: iterate `while z < cz` over the `[start..=end]` window; errors: window exhausted before `z == cz`, `skip` pushing `z` past `cz`, run overrunning window, `pad != count` on voxel runs (tolerate: only `pad` of terminal run is defined as 0; XCC ignores pad entirely, so treat it as unvalidated).
- Palette: store raw `[[u8;3];256]`; color mapping to RGB is a renderer concern (XCC renders from an external palette instead).
- Normals: expose `(normal_type, index)` and clamp per XCC vxl_normals.h:174-184; embed the two float tables from vxl_normals.h:20-167.
- `Hva { id: [u8;16], c_frames: i32, c_sections: i32, names: Vec<String>, matrices: Vec<[[f32;4];3]> }` with matrices indexed `[frame][section]` (store frame-major as read). Validate the exact size equation, `c_frames > 0`, `c_sections > 0`. Errors: size mismatch, matrix region length `!= 48*F*S`, and a pairing check against the companion .vxl (names equal and ordered) as a warning, not a hard error.
- Reject nothing else aggressively: XCC's stricter invariants (section `one`/`zero`, list/data contiguity) are commented out in XCC itself (vxl_file.cpp:22-32) — real files satisfy them, but the safest parser treats them as warnings.

## 6. Provenance index

XCC (branch `master`, fetched 2026-11-03 via contents API):

- cc_structures.h:35,546 — `#pragma pack(push, 1)` / pop (all structs packed)
- cc_structures.h:141-146 — `t_hva_header { char id[16]; i32 c_frames; i32 c_sections }`
- cc_structures.h:148 — `t_hva_transform_matrix = float[3][4]`
- cc_structures.h:456-465 — `t_vxl_header`
- cc_structures.h:467-473 — `t_vxl_section_header`
- cc_structures.h:475-492 — `t_vxl_section_tailer`
- cc_structures.h:494 — `vxl_id[] = "Voxel Animation"`
- palette.h:7-10 — `t_palette_entry { byte r, g, b }`
- vxl_file.cpp:11-34 — `is_valid` (magic, `one==1`, counts equal, size equation, `unknown==0x1f10`); :22-32 disabled stricter checks
- vxl_file.cpp:38-40 — rendering uses external palette (18->24 conversion), not header palette
- vxl_file.cpp:50-54, 93-97, 173-179 — span iteration `j = y*cx + x`
- vxl_file.cpp:54-70, 97-110 — span record decode loops
- vxl_file.cpp:153-159, 230-238 — header palette as 256x3 bytes
- vxl_file.cpp:246-249, 251-307, 312-333 — writer: body construction, `-1/-1` empty, `(cz-z,0,0)` tail run, offsets body-relative, `header.size = w - body_start`, tailer defaults
- vxl_file.h:22-25, 37-41, 42-45, 47-50, 52-57, 59-62, 64-72 — accessor arithmetic (span count, body start, header/tailer positions, span data pointer, span length, lists)
- vxl_file.cpp:443-463, 520-521, 619-621 — XCC "vxl4" intermediate format (not game format)
- vxl_normals.h:5-9, 20-167, 170-184 — normal_type semantics, tables, clamping
- hva_file.cpp:8-16 — `is_valid` (size equation, F>0, S>0)
- hva_file.cpp:63 — writer id "NONE"
- hva_file.cpp:31-42 — matrix row-major CSV dump (`[x + 4*y]`)
- hva_file.cpp:87 — writer section-major indexing (bug; do not replicate)
- hva_file.h:22-25 — section ids at `24 + 16*i`
- hva_file.h:27-37 — frame-major accessor + bugfix comment

Samples (extracted from genuine game MIXes via CRC-id lookup; scripts under
`%TEMP%\vxlhva\scripts`, extracted binaries under `%TEMP%\vxlhva\`):

- 1tnkbarl.vxl, 1tnkbarl.hva, 1tnk.vxl, 1tnk.hva — `ra2.mix -> local.mix`, E:\hongjing\Reunion 2023
- schp.hva — E:\hongjing\ra2-rs\fixtures\samples\schp.hva
- Additional header-equation validations (size formula only): 2tnk.vxl (14895), 3tnk.vxl (17639), ttnk.vxl (39873), harv.vxl (50469), smcv.vxl (87327), all S=1, all `802+28+size+92 == file_size`.
