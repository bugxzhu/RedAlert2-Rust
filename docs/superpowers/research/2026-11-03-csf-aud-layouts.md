# CSF and AUD byte-level layouts (XCC-grounded, sample-verified)

Date: 2026-11-03
Scope: authoritative layout spec for Westwood **CSF** (localized string table) and **AUD** (IMA-ADPCM audio) for Rust parsers in ra2-rs.
Sources of truth:

- XCC source: repo `triatomic/XCC-Tacitus` branch `master`, files `misc/csf_file.{h,cpp}`, `misc/aud_file.{h,cpp}`, `misc/aud_decode.{h,cpp}`, `misc/aud_file_write.cpp`, `misc/cc_structures.h` (all `#pragma pack(push,1)` around the structs — `XCC cc_structures.h:35`, pop at `:546`). Downloaded copies kept at `%TEMP%\csfaud\src\`.
- Samples:
  - `ra2md.csf` (YR Chinese string table, 336,648 B) — extracted with the repo's own `tools/ra2-mix` from `E:\hongjing\Reunion 2023\langmd.mix` (surprise: **not** directly present in `ra2md.mix`; `ra2md.mix` holds only expand-style entries). Analysis copy at `%TEMP%\csfaud\samples\ra2md.csf`.
  - `text1.aud` (821 B) from `E:\hongjing\ra2-rs\fixtures\samples\text1.aud` (`ra2.mix::local.mix` provenance).
  - `ares.mix` first CSF blob (read-only peek into `E:\hongjing\Reunion 2023\ares.mix`).
- Analysis scripts (run with `python -I`): `%TEMP%\csfaud\scripts\analyze.py`, `details.py`.

Everything below is either "XCC file:line" or "sample name@offset". No fact from memory.

---

## 1. CSF — localized string table

### 1.1 File header — 24 bytes (struct `t_csf_header`, `XCC cc_structures.h:110-117`)

| Off | Size | Type | XCC name | Meaning | ra2md.csf value |
|----:|-----:|------|----------|---------|-----------------|
| 0   | 4    | i32 LE | `id`   | file magic, must equal `csf_file_id = 'CSF '` (`cc_structures.h:120`). **On disk the bytes are `20 46 53 43` = ASCII `" FSC"`** — the C multi-char constant `'CSF '` = `0x43534620` is stored little-endian, so the spelling is reversed in the file. Read as u32 LE the magic is `0x43534620`. | `20 46 53 43` |
| 4   | 4    | i32 LE | `flags1` | opaque attributes. XCC always writes `3` (`XCC csf_file.cpp:160`); reader never checks it. Both samples have `3`. Treat as opaque, do not validate. | `03 00 00 00` |
| 8   | 4    | i32 LE | `count1` | **number of labels** — the only loop bound the reader uses (`XCC csf_file.cpp:77`) | 5280 (`a0 14 00 00`) |
| 12  | 4    | i32 LE | `count2` | number of strings; XCC writes it equal to `count1` (`csf_file.cpp:162`) and never reads it. Verify-equal only, do not drive parsing with it. | 5280 |
| 16  | 4    | i32 LE | `zero`   | always 0 (XCC writes 0, `csf_file.cpp:163`) | 0 |
| 20  | 4    | i32 LE | `flags2` | community name: "language id". XCC writes 0 (`csf_file.cpp:164`) and never reads it. Observed values: 0 in ra2md.csf, **3** in the ares.mix CSF → treat as opaque u32, do not validate. | 0 |

Total header size = 24 bytes (`sizeof(t_csf_header)`), used as the reader start offset (`XCC csf_file.cpp:76`) and in the `is_valid` size check (`csf_file.h:25-30`: fail if file smaller than header or `id` mismatch).

### 1.2 Label entry stream (repeated `count1` times, back to back, no padding)

Each label, in file order (reader at `XCC csf_file.cpp:77-93`, bounded variant `csf_file.cpp:199-235`):

| Order | Size | Type | Field | Meaning |
|------:|-----:|------|-------|---------|
| 1 | 4 | i32 LE | label magic | `csf_label_id = 'LBL '` (`cc_structures.h:121`). On-disk bytes `20 4C 42 4C` = ASCII `" LBL"` (reversed spelling; LE u32 = `0x4C424C20`). XCC reads and discards it (`csf_file.cpp:210`). |
| 2 | 4 | i32 LE | `flags` (XCC) | XCC tests bit 0: `if (flags & 1)` → a string value follows (`csf_file.cpp:82`). Community semantics for the same field: "number of strings in this label", normally 1. All 5280 labels in ra2md.csf have exactly `1`. If bit0 clear → label carries no value at all (empty label, `csf_file.cpp:91-92`). |
| 3 | 4 | i32 LE | name length `cb_name` | bytes of label name that follow (`csf_file.cpp:81` via `read_string`, `csf_file.cpp:38-44`) |
| 4 | `cb_name` | raw bytes | label name | **plain, un-inverted ASCII/ANSI bytes** (e.g. `THEME:Intro`). No NOT-transform. |
| 5a | 4 | i32 LE | string magic | only if `flags & 1`. `csf_string_id = 'STR '` → on-disk `20 52 54 53` (`" RTS"`, LE u32 `0x53545220`) = plain string; `csf_string_w_id = 'STRW'` → on-disk `57 52 54 53` (`"WRTS"`, LE u32 `0x53545257`) = string **plus extra value**. XCC: `has_extra_value = read_int(r) == csf_string_w_id` (`csf_file.cpp:84` / `:218`). |
| 5b | 4 | i32 LE | string length `cb_str` | **in UTF-16 code units (chars), not bytes** — XCC advances `r += cb_s << 1` (`csf_file.cpp:50`, `:219-221`) |
| 5c | `2*cb_str` | raw bytes | string value | UTF-16LE code units, **each unit bitwise-NOT'ed (XOR 0xFFFF)**. See 1.3. |
| 6 | 4 | i32 LE | extra length `cb_extra` | only if string magic was `'STRW'` (`csf_file.cpp:87-88`, `:224-228`) |
| 7 | `cb_extra` | raw bytes | extra value | raw ANSI/ASCII bytes, **no inversion, no fixed encoding guarantee** |

Byte-size identity (XCC writer, `csf_file.cpp:142-152`): `file_size == 24 + Σ_labels (20 + cb_name + 2*cb_str [+ 4 + cb_extra if STRW])`.
The `20` = label magic(4) + flags(4) + name_len(4) + str magic(4) + str_len(4).
**Verified exactly on ra2md.csf: formula total 336,648 == file size 336,648** (sample; see scripts).

### 1.3 String encoding — the inversion, exactly

XCC `encode()` (`csf_file.cpp:15-23`): builds a `wstring` from the raw little-endian 16-bit units, then `r[i] = ~s[i]` — one's-complement **per 16-bit unit**, not per byte. Equivalent, byte-exact transform for Rust:

1. Read `2*cb_str` bytes as `cb_str` u16 LE values.
2. XOR each with `0xFFFF`.
3. Interpret the resulting sequence as UTF-16LE (surrogate pairs appear naturally, already inverted).

Worked example, `sample ra2md.csf@0x18..` (first label `THEME:Intro`, `cb_str=2`):

- stored bytes: `FF A0 C5 A8`
- stored u16 LE units: `A0FF A8C5`
- after `^0xFFFF`: `5F00 573A` → U+5F00 U+573A → `开场` ("opening")

Second label `THEME:MadRap`, `cb_str=4`: stored units `A8CF 8D0E B1F6 98F7` → `5730 72F1 4E09 6708` → `地狱三月`.

Label names are NOT inverted (raw `THEME:Intro` visible in hex dump — sample ra2md.csf@0x24, immediately after the 12-byte label header at 0x18).

### 1.4 Sample verification — ra2md.csf (full header + first 2 labels)

- File: 336,648 B (`0x52308`). Head 24 bytes: `20 46 53 43 03 00 00 00 A0 14 00 00 A0 14 00 00 00 00 00 00 00 00 00 00` (sample ra2md.csf@0x00).
- Decoded header: magic OK (`" FSC"`), `flags1=3`, `count1=5280`, `count2=5280`, `zero=0`, `flags2/language=0`.
- Walked all labels sequentially: **5280 labels parsed, stream ends exactly at EOF (0x52308)** — no padding, no trailing data.
- Composition: 4076 plain `'STR '` strings, 1204 `'STRW'` strings-with-extra, 0 labels with `flags&1==0`. No duplicate names. `count1==count2`.
- Label[0]: name `THEME:Intro`, flags=1, `'STR '`, len 2 → `开场` (decode shown above).
- Label[1]: name `THEME:MadRap`, flags=1, `'STR '`, len 4 → `地狱三月`.
- STRW example: label `VOX:ceva001`, len 12 (Chinese value), extra = `b"ceva001c"` (8 raw ASCII bytes).
- ares.mix corroboration: a CSF header starts at `ares.mix@0x3A`: `" FSC"`, flags1=3, **count1=count2=16, zero=0, language=3** (sample ares.mix@0x3A). The container around it is a custom Ares blob bag (u16 blob count at +4), not a game MIX — parse the CSF at 0x3A directly.

### 1.5 CSF edge cases (all observed in samples, not hypothetical)

1. **Empty value, `cb_str == 0`**: 4 labels in ra2md.csf (`GUI:Blank`, `TXT_NONE`, `GUI:Empty`, `STT:Empty`). Layout is complete anyway: `'STR '` magic + i32 0 + zero bytes — i.e. an empty value still costs 8 bytes, not 0 (sample ra2md.csf; consistent with XCC writer which always writes a value when `flags&1`, `csf_file.cpp:170-171`).
2. **`flags & 1 == 0`**: label ends right after the name; no value at all. Not present in ra2md.csf but supported by XCC reader (`csf_file.cpp:91-92`) — a Rust parser must accept it (map entry with empty value + empty extra).
3. **STRW with empty value**: does not occur in ra2md.csf (0 of 1204), but XCC writer permits it (empty wstring + non-empty extra, `csf_file.cpp:170-173`). Reader must handle.
4. **Extra value**: always raw bytes; in ra2md.csf all 1204 extras are printable ASCII of length 7-8, and 1203/1204 start with the label's own stem (`ceva001` → `ceva001c`): it is a derived ANSI name (used by YR for speech/subtitle mapping), never inverted UTF-16.
5. **Label order**: ra2md.csf names are **not sorted** (`THEME:Intro, THEME:MadRap, THEME:Defense, ...`). XCC's writer re-sorts because it stores a `std::map` (`csf_file.cpp:15, 165`) — an XCC round-trip reorders entries. Parsers must not assume order; writers that want byte-identical output must preserve file order, not sort.
6. **Duplicate names**: none in ra2md.csf; XCC collapses them last-wins via `m_map[name] = ...` (`csf_file.cpp:135-140`). Rust should either reject or keep last-wins semantics; document the choice.
7. **`count2` can drift from the real string count in foreign files** — XCC never reads it; only use `count1`.
8. **Truncation**: the bounded reader `Ccsf_file_rd::post_open` (`csf_file.cpp:199-235`) breaks out (rather than erroring) when the next label header, name, or string would cross EOF (`csf_file.cpp:209, 213, 217, 220, 226`). For Rust: prefer a hard error, but note XCC tolerates.

### 1.6 Rust-facing notes (shapes + error cases, no code)

Suggested shape:

- `CsfHeader { magic: u32, flags1: u32, count1: u32, count2: u32, zero: u32, language: u32 }` — validate `magic == 0x4353_4620` and `file_len >= 24`; treat all other fields as data, not constraints.
- `CsfLabel { name: String (lossy from raw bytes), value: Option<Utf16Inv> , extra: Option<Vec<u8>> }`; store raw name bytes so byte-identical rewrite is possible.
- Value representation: keep the u16 units; convert to `String` lazily so unpaired surrogates round-trip.

Error cases to model: bad magic; file shorter than 24 B; label magic mismatch mid-stream; name/length fields that cross EOF; `cb_str` units*2 crossing EOF; unknown string magic (XCC treats any non-`'STRW'` as "no extra" and still reads a wstring — recommend rejecting `'STR '`-and-nothing-else strictly only if byte-exact round-trip is required); count1 disagreement with entries actually parsed; duplicate names.

Round-trip guarantee available: the size identity in 1.2 lets a writer assert exact reproduction before touching disk.

---

## 2. AUD — IMA-ADPCM audio

### 2.1 File header — 12 bytes (struct `t_aud_header`, `XCC cc_structures.h:37-44`, packed)

| Off | Size | Type | XCC name | Meaning | text1.aud value |
|----:|-----:|------|----------|---------|-----------------|
| 0 | 2 | u16 LE | `samplerate` | Hz. Validated `8000..=48000` (`XCC aud_file.cpp:10`) | 22050 (`22 56`) |
| 2 | 4 | u32 LE | `size_in` | total bytes of **all chunk headers + chunk payloads** after this header. Validated `== file_size - 12` (`aud_file.cpp:11`) | 809 (`29 03 00 00`) |
| 6 | 4 | u32 LE | `size_out` | total decoded PCM bytes = Σ chunk `size_out`. Sample count = `size_out / cb_sample` (`XCC aud_file.h:25-28`) | 3172 (`64 0c 00 00`) |
| 10 | 1 | i8 | `flags` | bit0 = stereo (`get_c_channels`, `aud_file.h:40-43`), bit1 = 16-bit (`get_cb_sample`, `aud_file.h:45-48`). Validity depends on codec: WS: `(flags & 0xFE) == 0` → {0,1}; IMA: `(flags & 0xFE) == 2` → {2,3} (`aud_file.cpp:13-19`) | 0x02 = 16-bit mono |
| 11 | 1 | i8 | `compression` | 1 = Westwood WS (custom 2/4-bit, 8-bit out); 0x63 = IMA ADPCM, 16-bit out (`aud_file.cpp:13-19`) | 0x63 |

XCC writer emits exactly `flags = 0x02, compression = 0x63` (`aud_file_write.cpp:28-29, 65-66`) and asserts mono only (`aud_file_write.cpp:60`).

### 2.2 Chunk stream (after the 12-byte header, back to back)

Chunk header — 8 bytes (struct `t_aud_chunk_header`, `XCC cc_structures.h:46-51`, packed):

| Off | Size | Type | XCC name | Meaning | chunk[0] of text1.aud |
|----:|-----:|------|----------|---------|------------------------|
| 0 | 2 | u16 LE | `size_in` | payload bytes following this header | 512 (`00 02`) |
| 2 | 2 | u16 LE | `size_out` | decoded PCM bytes this chunk; samples = `size_out / cb_sample` (`aud_file.cpp:73`) | 2048 (`00 08`) |
| 4 | 4 | i32 LE | `id` | `aud_chunk_id = 0x0000DEAF` (`cc_structures.h:53`) — on-disk bytes `AF DE 00 00`. **XCC never validates the id** (no comparison anywhere in `aud_file.cpp`/`aud_decode.cpp`) — informational only. | 0x0000DEAF |

Walk rule: chunk stride = `8 + size_in`; XCC indexes chunks by skipping exactly that (`XCC aud_file.cpp:34`) with EOF bounds checks at `aud_file.cpp:31-37`. Decode until the write cursor has consumed `size_out` bytes (`aud_file.cpp:55, 68, 74`).

Sample-count arithmetic (mono 16-bit): `size_in ≈ ceil(samples/2)` per chunk (nibble-packed; odd sample count pads a half byte — `aud_file_write.cpp:36, 79`). For the whole file XCC's writer predicts `size_in = ceil(samples/0x400)*8 + ceil(samples/2)` and `size_out = samples << 1` (`aud_file_write.cpp:26, 63-64`) — i.e. **1024-sample chunks** (one 8-byte header per 0x400 samples). text1.aud matches: chunk[0] = 1024 samples, chunk[1] = final 562.

### 2.3 IMA ADPCM decode (compression 0x63) — exact algorithm

`aud_decode_ima_chunk` (`XCC aud_decode.cpp:30-66`); tables `aud_ima_index_adjust_table[8] = {-1,-1,-1,-1,2,4,6,8}` and standard 89-entry `aud_ima_step_table` (`aud_decode.cpp:6-20`).

Per chunk, for `sample_index in 0..size_out/cb_sample`:

1. `code = data[sample_index >> 1]`; take **low nibble for even sample_index, high nibble for odd** (`aud_decode.cpp:37-38`) — low nibble first within each byte.
2. `step = step_table[index]; delta = step >> 3`, plus `step>>2` if `code&1`, `step>>1` if `code&2`, `step` if `code&4`.
3. `code&8` → subtract else add; clamp sample to `[-32768, 32767]` (`aud_decode.cpp:48-58`).
4. `index += adjust[code & 7]`, clamp `0..=88` (`aud_decode.cpp:60-64`).
5. Emit sample as i16 LE.

**Critical state rule: there is NO predictor/index preamble inside chunk payloads** (unlike WAV-IMA blocks). A single decoder is `init()`-ed once per file with defaults `index = 0, sample = 0` (`XCC aud_decode.h:8`, `aud_file.cpp:65-66`) and state **persists across chunk boundaries** (`aud_file.cpp:63-77` — one `aud_decode` object, `decode_chunk` per chunk). Do not reset state per chunk.

XCC writer symmetry: encode keeps one `aud_decode` for the whole file (`aud_file_write.cpp:68-69, 77`), chunk = `min(remaining, 1024)` samples (`aud_file_write.cpp:76`).

### 2.4 WS codec (compression 1) — for completeness

`aud_decode_ws_chunk` (`XCC aud_decode.cpp:75-125`), output is **unsigned 8-bit** PCM, running u8 `sample` starting at `0x80` **reset at every chunk** (`aud_decode.cpp:83` — local variable). If a chunk has `size_in == size_out` it is stored uncompressed (`memcpy`, `aud_decode.cpp:77-81`). Otherwise a control byte stream: `count = b & 0x3F`, `method = b >> 6`:

- method 0 (2-bit): `count+1` code bytes, each byte yields 4 output bytes, steps from `aud_ws_step_table2 = {-2,-1,0,1}` on nibble pairs `code&3`, `code>>2&3`, `code>>4&3`, `code>>6` (`aud_decode.cpp:89-98`).
- method 1 (4-bit): `count+1` code bytes, each yields 2 outputs, **low nibble first**, steps `aud_ws_step_table4 = {-9,-8,-6,-5,-4,-3,-2,-1,0,1,2,3,4,5,6,8}` (`aud_decode.cpp:99-108`).
- method 2: if `count & 0x20`: one output byte, `sample += sign_extended_3bit(count)` (`aud_decode.cpp:109-111`); else literal run: copy `count+1` bytes verbatim, `sample` = last byte (`aud_decode.cpp:112-118`).
- method 3: run — write `count+1` copies of the current sample (`aud_decode.cpp:120-123`).

RA2/TS-game AUDs are almost universally 0x63; codec 1 appears in older Westwood material.

### 2.5 Sample verification — text1.aud

File 821 B; head `22 56 29 03 00 00 64 0C 00 00 02 63 00 02 00 08` (sample text1.aud@0x00).

Header decode: rate 22050, size_in 809, size_out 3172, flags 0x02 (16-bit mono), compression 0x63.
Validation per XCC: `809 == 821 - 12` ✓; rate in range ✓; `(flags & 0xFE) == 2` ✓.

Chunk walk (sample text1.aud):

| # | file off | size_in | size_out | id | samples | payload off |
|---|---------:|--------:|---------:|----|--------:|------------|
| 0 | 0x0C | 512 | 2048 | 0x0000DEAF (`AF DE 00 00`) | 1024 | 0x14 |
| 1 | 0x214 | 281 | 1124 | 0x0000DEAF | 562 | 0x21C |

Walk total `809 == size_in` ✓ (2 headers × 8 = 16 + 512 + 281). Decoded with persistent state (init 0,0):

- **1586 samples** = `size_out / 2` ✓ = 3172 B PCM16 = **71.9 ms** at 22050 Hz.
- First 8 decoded samples: `[11, 41, -22, 114, -179, 452, -543, 649]`; range `min=-11630, max=17293`.
- State after chunk[0] (carried into chunk[1]): `index=50, sample=-77`.
- Nibble order demo: chunk[0] payload starts `77 7F 7F 4D` (text1.aud@0x14): sample 0 uses low nibble of 0x77 (=7), sample 1 the high nibble.

### 2.6 AUD edge cases

1. **8-bit vs 16-bit**: `flags & 2`; but note XCC validation pins the bit per codec — IMA files always have bit1 set (flags 2/3), WS files always clear (flags 0/1) (`aud_file.cpp:13-19`). WS output is u8; IMA output is i16 LE.
2. **Mono/stereo**: `flags & 1` (`aud_file.h:40-43`). XCC's writer asserts mono (`aud_file_write.cpp:60`), and XCC's IMA decode path treats a stereo file as a single interleaved sample stream — it does **not** maintain per-channel predictor state (`aud_file.cpp:73` divides `size_out` by `cb_sample` only). For correct stereo IMA playback the engine expects two independent (index, sample) states, L/R interleaved, even sample_index → left. Not exercised by any sample here; treat as low-confidence (XCC limitation observed, stereo behaviour inferred) and reject or downmix on `flags & 1` until a stereo sample is verified.
3. **Chunk `size` semantics**: `size_in` counts payload only (header excluded), u16 → max 65535 B per chunk; `size_out` is decoded byte count and is the only source of sample counts (`aud_file.cpp:73`). Odd sample counts cost `ceil(n/2)` bytes.
4. **Chunk id `0x0000DEAF`**: never validated by XCC — do not hard-fail on a different id.
5. **No IMA block preamble**: state is file-global (init 0,0 once), unlike WAV IMA ADPCM 4-byte block headers. Resetting per chunk produces garbage from chunk 2 onward.
6. **Trailing/short data**: `size_in == file_size - 12` is a hard validity rule (`aud_file.cpp:11`); chunk walk must bounds-check like `get_chunk_header` (`aud_file.cpp:31-37`).
7. **`size_out` vs actual chunks**: XCC decodes chunk-by-chunk until the output buffer is full (`aud_file.cpp:68, 74`) — i.e. it trusts `size_out` over the chunk chain. Rust should cross-check Σ`size_out` == header `size_out` and Σ(8+`size_in`) == header `size_in`.

### 2.7 Rust-facing notes (shapes + error cases, no code)

Suggested shape:

- `AudHeader { sample_rate: u16, size_in: u32, size_out: u32, flags: u8, compression: u8 }` with derived `channels()`, `cb_sample()`, `sample_count = size_out / cb_sample`.
- `AudChunkHeader { size_in: u16, size_out: u16, id: i32 }` + payload slice.
- Decoder struct holding `{ index: i32, sample: i32 }` created once per file (IMA), or per-chunk u8 state for WS.

Error cases: file < 12 B; rate outside 8000..48000; `size_in != len - 12`; compression not in {1, 0x63}; flags/compression mismatch per the `flags & 0xFE` rules; chunk header or payload crossing EOF; Σ sizes mismatching the header totals; stereo-IMA (unsupported — see 2.6.2); empty file with `size_out == 0` (XCC would write-start 0 bytes — define behavior explicitly).

---

## 3. Provenance index

XCC (branch master, repo triatomic/XCC-Tacitus):

- CSF header struct/ids: `XCC cc_structures.h:110-123`; pack pragma `:35`
- CSF read: `XCC csf_file.cpp:70-95` (classic), `:199-235` (bounded); inversion `:5-23`; length-in-chars `:46-52`; writer `:142-176` (flags1=3 `:160`, count2 `:162`, language 0 `:164`, size formula `:142-152`)
- CSF validity: `XCC csf_file.h:25-30`
- AUD header/chunk structs + deaf id: `XCC cc_structures.h:37-53`
- AUD validity & codec/flag rules: `XCC aud_file.cpp:7-21`
- Chunk walk: `XCC aud_file.cpp:23-39`; decode loop/state persistence: `:46-80`
- flags → channels/bits: `XCC aud_file.h:40-48`; sample count `:25-28`
- IMA tables + decoder: `XCC aud_decode.cpp:6-66`; WS decoder `:75-125`; state init defaults `XCC aud_decode.h:8`
- AUD writer (chunk size 1024, formulas, flags/comp): `XCC aud_file_write.cpp:22-31, 57-86`

Samples:

- ra2md.csf header@0x00; label[0]@0x18 (`THEME:Intro`, stored `FF A0 C5 A8`); label[1] `THEME:MadRap`; STRW example `VOX:ceva001`; empties `GUI:Blank`/`TXT_NONE`/`GUI:Empty`/`STT:Empty`; size identity 336,648 == 336,648; extracted from `langmd.mix` via `tools/ra2-mix`.
- text1.aud: header@0x00, chunk0 hdr@0x0C, chunk0 data@0x14, chunk1 hdr@0x214, chunk1 data@0x21C.
- ares.mix: CSF magic " FSC"@0x3A with count1=count2=16, language=3.
