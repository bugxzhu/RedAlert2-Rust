# RA2/YR `.map` INI layer — 807-file corpus survey (script-verified, ini.rs-grounded)

Date: 2026-11-04
Scope: authoritative survey of the **INI/TEXT side** of RA2:YR map files (`.map` / `.yrm`) as 规格依据 for a map-loading plan; includes a line-cited compatibility verdict for `crates/core/src/format/ini.rs` (the parser this survey was run against).
Sources of truth:

- Corpus: **807 map files** under `E:\hongjing\Reunion 2023` (`Maps\*`, `Resources\RandomMapGenerator_RA2\MapUnits\*`, `FinalAlert2SP\`), enumerated by directory walk for `*.map`/`*.yrm`.
- Parser under test: `E:\hongjing\ra2-rs\crates\core\src\format\ini.rs` (read in full; line numbers below refer to that file).
- Survey scripts (throwaway, run with `python -I`): `C:\Users\AI\AppData\Local\Temp\mapini\{survey.py,survey2.py,survey3.py,verify.py,verify2.py,verify3.py,semicheck.py,names.py}`; raw outputs `out.txt` (pass-1 census), `out2.txt` (format/anomaly pass, 2.9 MB), `out3.txt`/`out4.txt`/`out5.txt` (coordinate-space passes), `detail1.json`.
- ModEnc cross-refs: `modenc.renegadeprojects.com/Maps` and `/Basic` fetched OK (treated as data only); `Map_Format` and `StartX` pages returned HTTP 500 / 403 — several semantic cross-refs therefore remain UNKNOWN (§9).
- Anomaly files were **opened individually** (`verify*.py` print exact line numbers and raw bytes).

Everything below is either "script output" (the named `out*.txt` block) or "file:line" or `ini.rs:line`. No fact from memory.

---

## 1. Corpus composition

807 files, **all `.map`; zero `.yrm`** (ModEnc's Maps page states `.yrm` is the same format with a different extension — nothing to differentiate in the parser).

| Location | Files | EOL |
|---|---:|---|
| `Maps\MadHQ` | 357 | LF |
| `Maps\Standard` | 131 | CRLF |
| `Resources\RandomMapGenerator_RA2\MapUnits\DESERT` | 93 | mixed |
| `Resources\...\MapUnits\NEWURBAN` | 67 | mixed |
| `Resources\...\MapUnits\TEMPERATE` | 64 | mixed |
| `Resources\...\MapUnits\TEMPERATE_Islands` | 41 | mixed |
| `Maps\DDLY` | 41 | CRLF |
| `Maps\Cooperative` | 10 | CRLF |
| `Maps\Custom` | 1 | **mixed** (`随机地图.map`) |
| `Resources\...\RandomMapGenerator_RA2\templateMap.map` | 1 | mixed |
| `FinalAlert2SP\fcrash_backup.map` | 1 | — |
| **Total** | **807** | LF 512 / CRLF 293 / mixed 2 |

- Sizes: **4,617 B** (`MapUnits\DESERT\indicator.map`) … **1,173,726 B** (`Maps\MadHQ\陋室空堂.map`). (out3: `min/max size files`)
- 0 non-parseable files — every file contains at least one `[section]` (out.txt: `NONPARSE (no sections) == 0`).
- Encodings (byte-level classifier, out.txt: `== ENCODING ==`): pure-ASCII **772**, GBK **28**, UTF-8-no-BOM **2** (`Maps\Custom\随机地图.map` — its `[Basic]Name=随机地图` is genuine UTF-8 bytes `e9 9a 8f e6 9c ba ...`; `Maps\MadHQ\阿拉斯加宝石.map` has UTF-8 non-ASCII outside ASCII `Name=`), **5 files that fail strict UTF-8, GBK *and* Big5 decode**. Those 5 (4× DDLY: `终极海战防守图.map`, `终极防守战.map`, `降维打击三人防守图.map`, `超武作战 保卫克里姆宁宫【加强版】.map`; 1× MadHQ: `帮派战争.map`) all fail because of a **truncated multi-byte GBK sequence** — e.g. `DDLY\终极海战防守图.map` contains `Name=1 cd e6 bc d2 d4 ae be` (a GBK lead byte with no trail byte; verified context at offset 51,614). They are normal maps with one lost byte; a **lossy** GBK decode reads them fine.
- **Zero BOMs** in the entire corpus.
- Line lengths: max **495** chars (`Maps\Cooperative\c3y03md.map`, an `[Actions]` line chaining eight 8-field ambient-sound actions: `09AC3A4C=16,99,7,_Amb_WavesLake,0,0,0,0,GO,99,7,...`). Histogram (bucket = chars): 3,415,358 lines ≤99, 435 in the 100s, 129 in the 200s, 247 in the 300s, 79 in the 400s (out.txt `LINE LENGTH HIST`).
- **Zero** lines with leading whitespace; **zero** garbage lines (non-empty, non-`;`, no `=`, outside any header) — out.txt `GARBAGE lines: 0 {}`.

---

## 2. Section-name census

Core map sections (files containing, out.txt `SECTION CENSUS`):

| Section | Files | | Section | Files |
|---|---:|---|---|---:|
| `[Header]` | 807 | | `[Structures]` | 671 |
| `[SpecialFlags]` | 807 | | `[Waypoints]` | 641 |
| `[Basic]` | 807 | | `[AITriggerTypesEnable]` | 501 |
| `[Lighting]` | 807 | | `[Triggers]` | 458 |
| `[Preview]` | 807 | | `[TaskForces]` | 415 |
| `[PreviewPack]` | 807 | | `[Tags]` | 318 |
| `[Map]` | 807 | | `[Events]` / `[Actions]` | 291 / 291 |
| `[OverlayPack]` | 807 | | `[Units]` | 278 |
| `[OverlayDataPack]` | 807 | | `[CellTags]` | 270 |
| `[IsoMapPack5]` | 806 | | `[FA2SPVersionControl]` | 170 |
| `[Digest]` | 804 | | `[Ranking]` | 144 |
| `[Houses]` / `[Neutral]` / `[Special]` | 765 | | `[TeamTypes]` | 132 |
| `[Terrain]` + 12 house sections | 763 | | `[Infantry]` | 122 |
| `[YuriCountry]` | 691 | | `[ScriptTypes]` | 45 |
| `[Smudge]` | **41** | | `[Aircraft]` | 13 |
| `[VehicleTypes]` | 41 | | `[AITriggerTypes]` | 12 |
| `[General]` | 39 | | `[VariableNames]` | 11 |

Non-standard-but-real observations:

- **The smudge object section is `[Smudge]` (41 files), not `[Smudges]`.** `[Smudges]` never occurs. `[SmudgeTypes]` occurs in 3 files and is a rules-style list, not the object section. Any plan that names the section `[Smudges]` will read 0 rows from real maps.
- **No `[Footers]`, no `[Hash]`, no `[Difficulties]`, no `[MultiplayerChecksum]`** anywhere in the corpus.
- **Hundreds of embedded rules-override sections** ride along inside maps: `[NAyard]` (98 files), `[GAyard]` (97), `[AircraftTypes]` (96), `[YAyard]` (92), per-object sections (`[GACNST]` 18, `[HTNK]` 15, `[AMCV]` 16, …), weapon/warhead sections (`[GrandCannonWeapon]` 9, `[HollowPoint]` 6, …), superweapon sections (`[NukeSpecial]`, `[LightningStormSpecial]`, … 4 files each), `[CrateRules]` (8), `[General]` (39), plus **~400 hex-named trigger-object sections** (`[01000018]` ×14 files, `[06BF9DBC]` ×1, …) that are the rules-definitions of triggers defined in `[Triggers]`.
- Section order is **not canonical**. First-section census (out5): `[Header]` 550, `[Actions]` 59, `[Preview]` 41, `[SpecialFlags]` 40, `[Africans]` 30, `[YAyard]` 29, `[GAyard]` 18, `[NEGLAMP]` 9, `[CASLAB]` 8 (`Maps\Cooperative\c1a01md.map` literally starts with `[CASLAB]`), `[General]` 7, … Consumers must not assume any ordering.
- Sections per file: min 16, max 164, median 33 (out2).
- Spelling is uniformly the standard CamelCase (`PreviewPack`, `OverlayDataPack`, `TaskForces`, …); the census found no lower-case or shout-case variants of core sections (out.txt `SECTION CASE SPELLINGS`).

---

## 3. Key-syntax stress vs `ini.rs` — the verdict table

`ini.rs` mechanics recap (line-cited): `for line in text.lines()` + `trim()` + skip empty/`;` (ini.rs:20-24); header = `strip_prefix('[').and_then(strip_suffix(']'))` (ini.rs:25); duplicate sections merge case-insensitively (ini.rs:27-40); `split_once('=')` (ini.rs:41); empty key skipped (ini.rs:43-45); key=value before any section silently dropped (ini.rs:46-47); duplicate key = last-wins overwrite (ini.rs:48-54); everything else skipped (ini.rs:57).

| # | Stress | Corpus evidence | `ini.rs` behavior | Verdict |
|---|---|---|---|---|
| 1 | CRLF / LF / mixed EOL | LF 512, CRLF 293, mixed 2 files | `text.lines()` + `trim()` strips `\r` (ini.rs:20-21) | **OK as-is** |
| 2 | BOM | zero BOMs | — | n/a |
| 3 | Non-ASCII text (GBK names, 2 UTF-8 files, 5 truncated-GBK files) | 33 files non-ASCII (§1) | `parse(&str)` — decoding is the caller's problem | **Required: decode policy** (§8-3). Strict GBK breaks 5 files. |
| 4 | Key=value **before any section** | **1,348 lines across 12 files**, all `Resources\RandomMapGenerator_RA2\MapUnits\NEWURBAN\*.map` — e.g. `1,1,1,1,01,spawn.map` line 9 `Size=0,0,60,30` followed by numbered base64 lines (verified: file has **no section header at all** until later) | silently dropped (ini.rs:46-47) | **Policy decision** (§8-4): fine for real maps, fatal for RMG MapUnit half-maps |
| 5 | Section header with trailing comment/whitespace | **1 file**: `Maps\DDLY\全新油桶一块地.map:156` = `[FlakWeaponE6]\t\t; This belongs to Flak Cannon` (3 sibling DDLY files have the balanced `[FlakWeaponE6]` and parse fine) | `strip_suffix(']')` fails → line falls through, no `=` → skipped (ini.rs:25,57); `current` still points at `[NAFLAK]` (line 112) → **all following keys (`Damage=55`, `ROF=15`, `Range=12.5`, `Projectile=FlakProj`, … lines 157-164) are misfiled into `[NAFLAK]`** | **Bug: misfiling** (§8-1). Worst single finding of the survey. |
| 6 | Lone `=` line (empty key) | **1**: `Maps\MadHQ\城市海啸.map:21`, inside `[CAOILD]` between the header and `Strength=1500` | skipped (ini.rs:43-44) | **OK as-is** (matches editor intent; it is stray garbage) |
| 7 | Garbage lines (no `=`) | 0 | skipped (ini.rs:57) | OK |
| 8 | **Duplicate keys within a section** | **Zero occurrences corpus-wide** (out.txt: all three dup-key counters empty) | last-wins overwrite (ini.rs:48-54) | **Verified safe** — maps express lists with distinct numbered keys, never by repeating a key |
| 9 | **Duplicate section headers** | **1 file**: `Maps\DDLY\6人沙漠尤里趣味图.map` has `[SMCV]` at lines 1932 **and** 1967 (verified) | merged case-insensitively; second block's keys appended/overwritten into first (ini.rs:27-40) | **OK as-is** (same as game behavior; document it) |
| 10 | Values containing `;` | **127 line occurrences** (116 distinct stored values), all inside embedded rules-override sections (top: `[Spy]` 8, `[JumpJet]` 8, `[LUNR]` 8, `[Terror]` 7, `[Ghost]` 6, `[NAFLAK]` 4); **none** in core map-data sections | value kept verbatim (documented by test at ini.rs:126-131); `get_int` then silently returns the default (ini.rs:72-76) | **Required: comment stripping / lenient getters** (§8-2). In-game evidence: `CanRetaliate=no; Won't fire back when hit` (`[Spy]`, DDLY maps) works in-game ⇒ Westwood strips `;`-comments mid-token, unquoted. Real counterexample to naive parsing: `Soylent=900;2000` (`[Spy]`) means 900. |
| 11 | Leading whitespace | 0 lines | `trim()` handles | OK |
| 12 | Very long lines | max 495 chars | no line-length limit | OK |
| 13 | Case of sections/keys | uniform CamelCase for real sections | ci matching everywhere (ini.rs:30,50,65,93) | OK (ci merge keeps uniform corpus safe) |
| 14 | Weird keys | `(2)Percent=0` in 2 files (`Maps\MadHQ\敌方行动.map`, `Maps\MadHQ\锦标赛 - 山脊.map`) — duplicated-with-suffix key from a botched edit | stored as ordinary key | OK (generic storage survives) |

Empty section names (`[]`), unbalanced `[` without `]` elsewhere, keys with `]` inside: **zero** occurrences.

---

## 4. Field census — per-section

### 4.1 `[Basic]` (807/807 files)

| Key | Files | Sample / range |
|---|---:|---|
| `NextScenario`, `AltNextScenario` | 807 | `GDI2A.map` / `GDI9C.MAP` |
| `NewINIFormat` | 807 | always `4` |
| `CarryOverCap` | 807 | always `0` |
| `EndOfGame`, `SkipScore`, `OneTimeOnly`, `SkipMapSelect`, `IgnoreGlobalAITriggers`, `TruckCrate`, `TrainCrate`, `FreeRadar` | 807 | yes/no |
| `Official` | 807 | `yes` |
| `MultiplayerOnly` | 807 | `1` |
| `TiberiumGrowthEnabled`, `VeinGrowthEnabled`, `IceGrowthEnabled`, `TiberiumDeathToVisceroid` | 807 | yes/no |
| `InitTime` | 807 | 10000–20000 |
| `Percent` | 805 | `0` (also stray `(2)Percent` ×2, §3-14) |
| `Name` | 665 | **GBK Chinese in 8 files** (`[4]分而治之`, `海啸`, …); rest ASCII |
| `GameMode` | 665 | `standard` |
| `HomeCell` / `AltHomeCell` | 665 | `98` / `99` (waypoint-style numbers, see §6) |
| `MaxPlayer` / `MinPlayer` | 665 | `6` / `2` |
| `CarryOverMoney` | 665 | `0.000000` (float formatting) |
| `Theme` | 143 | `No theme` |
| `Author` | 69 | `Handama` |
| `Briefing` | 27 | `Brief:ALL01md` (CSF label) |
| rare | ≤1 | `Player=Americans`; `Intro/Brief/Win/Lose/Action/PostScore/PreMapSelect=<none>`; `TimerInherit=no`; `FillSilos`; `StartingDropships` |

### 4.2 `[Map]` (807/807)

Exactly **3 keys, always present**:

- `Theater=` — census: TEMPERATE 324, DESERT 170, NEWURBAN 128, SNOW 106, URBAN 78, **LUNAR 1** (the lone LUNAR map is a MadHQ moon-themed map).
- `Size=` — always 4 ints, first two always `0,0`: `0,0,87,71` = x,y,w,h.
- `LocalSize=` — always 4 ints: `2,6,82,59` (small non-zero origin typical).

Relations (out4, 807 rows): `LocalSize.w` ≤ `Size.w`, `LocalSize.h` ≤ `Size.h` everywhere; exceptions only in the 4 RMG `indicator.map` files: `Header.Width=-10000` with `LocalSize=3,5,-5,-10` — **negative width/height** in a deliberately degenerate editor-marker file.

### 4.3 `[Header]` (807/807)

| Key | Files | Sample / range |
|---|---:|---|
| `StartX`, `StartY` | 807 | `215`, `50` — see §6: no closed form found |
| `Width`, `Height` | 807 | `Width == LocalSize.w − 1` in **802/807** (out5 distribution: −1 ×802, **−9995 ×4** (indicator.map), **+3 ×1**); `Height − LocalSize.h ∈ 0..5` (×471/175/95/35/14/6) |
| `NumberStartingPoints` | 807 | `4` |
| `Waypoint1..8` | 806/806/806/806/806/806/805/805 | comma pairs `219,71`; 575+ files lack some/none; many `0,0` placeholders |
| `NumCoopHumanStartSpots` | 143 | `2` (co-op maps) |

`[Header]WaypointN="a,b"` is a **different coordinate space** from `[Waypoints]` — checked all 2723 (N, N−1) pairs across 272 files: 0 equal, 0 half-scale, 2723 "other" (out3). UNKNOWN semantics (§9).

### 4.4 `[Lighting]` (807/807)

14 keys, all floats with 6 decimals. Per-key ranges (out2 `RANGES`):

| Key | min | max |
|---|---:|---:|
| `Ambient` | 0.25 | 1.25 |
| `Red` | 0.30 | 1.50 |
| `Green` | 0.00 | 1.20 |
| `Blue` | 0.00 | 1.50 |
| `Ground` | 0.00 | 0.25 |
| `Level` | 0.00 | 0.06 |
| `IonAmbient` | 0.25 | 1.00 |
| `IonRed`/`IonGreen`/`IonBlue`/`IonGround` | — | — (present ×807) |
| `DominatorAmbient`/`DominatorRed`/`DominatorGreen` | 0.76 | 1.50 (present ×801) |

### 4.5 `[Preview]` + pack family

- `[Preview]`: single key `Size=0,0,162,59` (4 ints; arity 4 in all 807).
- Pack sections hold **1-based integer keys** whose values are base64 chunks of 2–74 chars (pure standard alphabet; **0 charset violations** corpus-wide):

| Section | Files | distinct keys | key range | total b64/file (min..max) | mod-4 OK |
|---|---:|---:|---|---|---|
| `[PreviewPack]` | 807 | up to 3090 | 1..3090 | 20 .. 216,296 chars | 800/807 (**7 files mod 2** — GBK-named DDLY community maps, previews truncated mid-quantum) |
| `[OverlayPack]` | 807 | up to 224 | 1..224 | 384 .. 15,636 | 807/807 |
| `[OverlayDataPack]` | 807 | up to 370 | 1..370 | 384 .. 25,832 | 807/807 |
| `[IsoMapPack5]` | 806 | up to 9568 | 1..9568 | 28 .. 669,712 | 806/806 |

Sample chunk (`[OverlayDataPack]3=` in nearly every file): `gQD+dgoAgQT+/wEAgQX+/wEA/wACdwyCAwj+/Q0AgQb/iQF4EIChAAAggQD+dQAAgQX+/w`. Concatenated-stream length divisible by 4 holds for every overlay/iso/preview stream except the 7 truncated previews — loader must tolerate short tails (§8-7).

### 4.6 `[Waypoints]` (641 files)

- Keys are **plain integers** (waypoint numbers), never names. Full observed key set: dense 0..259, then sparse 312..701 (out2 `WAYPOINT all keys`).
- Values are **100% integers** (`int` ×8611, no other shape) — the packed form `Y*1000+X` (§6). Observed value range −39,897 .. 490,098.
- Spawn waypoints `0..7` occur in 568/546/448/408/254/221/152/126 files respectively; special waypoints `98` ×145, `99` ×133 files.
- **10 negative waypoint values** in 4 maps (verified): `rushhr.map` `203=-5915`; `xlostlake.map` `49..53 = -905,-19890,-29917,-39897,-14944`; `xmp20t6.map` `81=-13844`; `xmp22s8.map` `50..52`. Junk data, must not crash the loader.

### 4.7 Object sections — `[Structures]` / `[Units]` / `[Infantry]` / `[Aircraft]`

Value = comma-CSV; field counts are **exact and uniform** (out.txt `STRUCT/UNIT FIELD COUNTS`; 1 exception below):

| Section | Fields | Sample | Max entries/file |
|---|---:|---|---:|
| `[Structures]` | **17** | `Neutral,CABHUT,256,103,64,0,None,0,0,1,0,0,None,None,None,0,0` | 2253 |
| `[Units]` | **14** | `Neutral,CAR,256,66,104,0,Sleep,None,0,-1,0,-1,1,1` | 503 |
| `[Infantry]` | **14** | `Neutral,COW,256,82,81,3,Guard,125,None,0,-1,0,1,1` | 488 |
| `[Aircraft]` | **12** | `Neutral,PDPLANE,256,108,168,64,Guard,None,0,-1,1,1` | 11 |

Field positions (0-based, from the samples):

- Structures: `0` owner house, `1` type, `2` strength (256), `3` cell X, `4` cell Y, `5` facing (0/64/…), `6` tag (`None` or hex8), `7..11` flags (typ. `0,0,1,0,0`), `12..14` `None`, `15`, `16` `0/1`.
- Units/Infantry/Aircraft: `0` owner, `1` type, `2` strength, `3` X, `4` Y, `5` facing (units) / **subcell 0–5 (infantry)**, `6` mission-or-tag, `7` tag-or-veterancy, then numeric flags.
  - Field 6 is **overloaded**: across all objects it holds `None` ×65,834, mission words `Guard` ×10,725 / `Sleep` ×759 / `Move` ×81 / `Sticky` ×46 / `Hunt` ×41 / `Area Guard` ×23 / `Patrol` ×2 / `Harmless` ×2, and hex8 tag IDs ×1,260 (out2 `TAG SHAPES`). Position semantics differ per section — treat with per-section layouts, not a shared struct.
- Owner vocabulary (out2 `OWNER VOCAB`): `Neutral` 78,488, `Special` 223, `Americans` 60, `YuriCountry` 2, plus **one corrupt row**.
- Key shapes (out2 `KEY SHAPE`): `[Structures]` keys are `0..n-1` in 488 files but **sorted-not-contiguous big IDs** (e.g. `1025970`, `1025971`, …) in 183 files (MadHQ-style); Units 185/17, Infantry and Aircraft similar. **Never use the key as an index — enumerate the entries.**
- The one corrupt row (verified, `Maps\MadHQ\勇士与巨龙.map:6187`): `69=NeutralGAWEAT,256,138,264,0,None,1,0,1,0,0,None,None,None,1,0` — **16 fields**; owner+type merged (`Neutral,GAWEAT` lost its comma). Real maps ship real corruption; parse permissively, report, continue.

### 4.8 `[Terrain]` / `[Smudge]` / `[CellTags]`

- `[Terrain]` (763 files): key = integer cell number `Y*1000+X` (**447,588 keys** total, 100% int-keys), value = **one token**: `101043=TIBTRE01`, `39021=TREE13`.
- `[Smudge]` (41 files): key = **sequential integer 0..n-1**, value = 4 CSV fields: `CRATER07,92,153,0` (name, X, Y, `0`). Samples x41 in out2.
- `[CellTags]` (270 files): key = integer cell number, value = hex8 tag ID: `12012=01000001` (63 files share this exact run of CellTags 12012+).

### 4.9 Houses

- `[Houses]` (765 files): numbered keys `0..13` → `Americans,Alliance,French,Germans,British,Africans,Arabs,Confederation,Russians,YuriCountry,GDI,Nod,Neutral[,Special]` (`13` present ×692).
- Per-house sections (e.g. `[Americans]` ×763): `Country`, `TechLevel` (10 for majors, 1–9 for minors/Special), `Credits` (`150` / `0`), `IQ` (0–5), `PlayerControl` (yes/no), `Color` (`DarkBlue`, `Grey`, …), `Allies` (comma list), `PercentBuilt=0`, `NodeCount=0`, `Edge` (`North`/`South`/…; ×733–740 of 763–765).
- `[Special]`: `Country=Special`, `Color=Grey`, `Allies=Special`, TechLevel 1–9, IQ 0–5.

### 4.10 Fixed small sections

- `[SpecialFlags]` — **exactly 13 yes/no keys in all 807 files**: `TiberiumGrows, TiberiumSpreads, TiberiumExplosive, DestroyableBridges, MCVDeploy, InitialVeteran, FixedAlliance, HarvesterImmune, FogOfWar, Inert, IonStorms, Meteorites, Visceroids`.
- `[Ranking]` (144): `ParTimeEasy/Medium/Hard=00:01:00`, `UnderParTitle/Message`, `OverParTitle/Message` = CSF labels `GUI:NoUnderParTitle` etc.
- `[FA2SPVersionControl]` (170): single `Version=157`.
- `[AITriggerTypesEnable]` (501): keys are hex8 or **hex8 + `-G`** suffix (e.g. `0CAD0B2C-G=yes`), value always `yes`.
- `[Digest]` (804 of 807; missing in 3 GBK-named community maps): single key `1`, value = **28-char standard base64** (22 raw bytes): `1=qSt1w6I0r+9EWkfWl7XFZMt0Vo4=`. All 804 exactly 28 chars.
- `[VariableNames]` (11): `0=Rig_DA_Captured,1` — name + initial value.

---

## 5. Logic sections census

`[Triggers]`/`[Events]`/`[Actions]`/`[Tags]`/`[TeamTypes]`/`[TaskForces]`/`[ScriptTypes]`/`[VariableNames]`/`[AITriggerTypes]` (out.txt `LOGIC SECTIONS`):

| Section | Files | Total entries | Max entries/file | Max line | Representative value |
|---|---:|---:|---:|---:|---|
| `[Triggers]` | 458 | 1987 | 78 | 83 | `0c44c1dc=Americans,<none>,Ambient1,0,1,1,1,0` |
| `[Events]` | 291 | 1987 | 78 | 47 | `01000000=1,13,0,0` |
| `[Actions]` | 291 | 1987 | 78 | **495** | `0c44c1dc=11,99,7,_Amb_OceanHeavy,0,0,0,0,AA,99,7,...` (8-field groups chained with commas) |
| `[Tags]` | 318 | 1934 | 78 | 71 | `0c44d03c=0,Sounds,0C42D3AC` (tag **names can be GBK Chinese**) |
| `[TeamTypes]` | 132 | 969 | 68 | 11 | `0=0725A4BC` |
| `[TaskForces]` | 415 | 496 | 32 | 11 | `0=0725A13C` |
| `[ScriptTypes]` | 45 | 436 | 43 | 12 | `0=0725A24C` |
| `[AITriggerTypes]` | 12 | 249 | 43 | 182 | `06c0a3bc=AirBackdoor_A_North,06BF97BC,<all>,10,0,GAWEAP,0100000003000000000000000000000000000000000000000000000000000000,50.000000,30.000000,60.000000,1,0,2,0,` — 17 fields incl. a **56-hex-char** condition block and a **trailing comma** |
| `[VariableNames]` | 11 | 50 | 15 | 21 | `0=Rig_DA_Captured,1` |

Notes: `[Events]` ids are hex8 keys; the trigger graph `[Tags]→[Triggers]→[Events]/[Actions]` and `[TeamTypes]→[TaskForces]→[ScriptTypes]` cross-reference by hex8 id; `[Actions]` lines embed sub-action chains, so the "fields per action" count is not fixed — parse as `first,followup,mode,……` comma stream per the 8-field group shape shown above.

---

## 6. Coordinate spaces

Three distinct spaces are in play (all empirically checked, out3/out4/out5):

1. **Cell numbers** (`[Terrain]` keys, `[CellTags]` keys, `[Waypoints]` values): `n = Y*1000 + X` with `0 ≤ X,Y < Size.w + Size.h`. Verified clean in **796/807** files (out5: `clean 796, viol 11`); the violations are junk waypoints, e.g. `Maps\MadHQ\中央山脉.map` `[Waypoints]100..114 = 290281, 262239, …` against a bound of 200 (23 out-of-bound values, verified).
   Empirical diamond-transform candidates for tile↔cell conversion fit all observed bounding boxes: `tileX = cellX − cellY_offset`, `cellX = tileX + tileY`, `cellY = tileX − tileY + Height`; the exact ±1 offset between the two conventions is **UNRESOLVED** (§9) — pin it against the `[IsoMapPack5]` decoder when that lands.
2. **`[Header]Waypoint1..8`** pairs (`219,71`): a *different* space — never equals `[Waypoints]` N−1 in any of 2723 pairs (0 equal, 0 half-scale). Semantics **UNKNOWN** (§9).
3. **`[Header]StartX/StartY`**: no integer-linear form over `(LocalSize.x,y,w,h, Size.w,h)` fits (out3 candidates: best mismatch 804/807; least-squares residuals huge). Semantics **UNKNOWN** (§9).

---

## 7. `ini.rs` compatibility verdict (summary)

| `ini.rs` line(s) | Behavior | Corpus verdict |
|---|---|---|
| 20-24 | `lines()`+`trim()`, skip blank/`;` | Correct for LF/CRLF/mixed; zero-BOM corpus |
| 25 | header via `strip_prefix('[')+strip_suffix(']')` | **Breaks** on trailing-comment headers → silent key misfiling into previous section (§3-5) |
| 27-40 | ci duplicate-section merge | Matches the 1 observed duplicate; OK |
| 41-45 | `split_once('=')`, skip empty key | Correct; the 1 lone `=` is skipped as intended |
| 46-47 | drop key=value before any section | Loses 1,348 RMG MapUnit lines (§3-4) — policy needed |
| 48-54 | dup key last-wins | Verified safe (zero dup keys corpus-wide) |
| 57 | skip everything else | Zero garbage lines; OK |
| 62-70 | `section()`/`value()` ci lookup | OK |
| 72-76 | `get_int` parse-or-default | **Silently wrong** on `;`-commented values (`Damage=4;Number of…` → default). Requires comment stripping (§8-2) |
| 78-86 | `get_bool` non-bool→false | OK for the all-yes/no `[SpecialFlags]`; keep |
| entries: `Vec<(String,String)>` | ordered | **Load-bearing invariant** — pack streams and object lists need entry order + per-key values; do not switch to a map type (§8-5) |

## 8. REQUIRED IniDocument adaptations

1. **Section headers: accept trailing comments/whitespace.** Parse a header as "starts with `[`, take name up to the last `]`" (or strip an unquoted `;`-comment before the suffix check). Real example: `Maps\DDLY\全新油桶一块地.map:156` `[FlakWeaponE6]\t\t; This belongs to Flak Cannon` — today its 8 body keys are silently appended to `[NAFLAK]` (last-wins may additionally clobber NAFLAK's own values). Fix at `ini.rs:25`.
2. **Value-level `;` comments.** Strip an unquoted `;`-comment from values at parse time (Westwood semantics — in-game `CanRetaliate=no; Won't fire back when hit` reads as `no`; `Soylent=900;2000` reads as 900). 127 occurrences, all in embedded rules-override sections; core map data is `;`-free, so stripping is safe for map loading and mandatory for rules overrides. Alternatively keep values verbatim but make `get_int`/`get_float` parse the leading numeric token — decision point for the plan, but *something* must change at `ini.rs:48-54/72-76`.
3. **Decode policy outside `IniDocument`** (it takes `&str`): try UTF-8, fall back to GBK **lossy** (never strict). 28 GBK files, 2 UTF-8 files, 5 files with a truncated GBK sequence fail strict decoding of every CJK codec (§1). `[Basic]Name`/`[Tags]` names are the only user-visible strings affected.
4. **Orphan-line policy for section-less prefixes.** 12 RMG `MapUnits\NEWURBAN\*.map` files begin with bare `Size=0,0,60,30` + numbered base64 chunks before any `[section]` (1,348 lines; verified head of `1,1,1,1,01,spawn.map`). Today all are dropped (ini.rs:46-47). For M2 real-map loading: keep dropping but *count and report*; if RMG half-map templates ever get loaded, add a pseudo-section or a dedicated parser. Explicit decision required.
5. **Lock the ordered-entries invariant.** `IniSection::entries` must remain an order-preserving sequence with duplicate-capable keys: `[OverlayPack]`/`[IsoMapPack5]` reassembly concatenates values in key order; object sections are enumerated by entry, not by key value (keys jump: `1025970, 1025971, …`). Add a regression test so nobody swaps in `HashMap`.
6. **Keep and test last-wins duplicate keys + ci duplicate-section merge.** Corpus-verified: 0 duplicate keys, 1 duplicate-section file (`Maps\DDLY\6人沙漠尤里趣味图.map`, `[SMCV]` ×2). Behavior is already correct — freeze it with tests.
7. **Tolerate broken optional data at loader level** (not `IniDocument` itself): missing `[Digest]` (3 files), missing `[IsoMapPack5]` (1 file: `templateMap.map`), `[PreviewPack]` stream lengths ≠ mod 4 (7 files), 16-field `[Structures]` rows (`Maps\MadHQ\勇士与巨龙.map:6187`), negative/garbage `[Waypoints]` values (4 files), negative `Header.Width` + negative `LocalSize` w/h (4 RMG `indicator.map`). None may abort the load; report and continue.

## 9. UNKNOWN (explicit)

- Exact semantics/origin of `[Header]StartX`, `StartY` and `Waypoint1..8` pairs (different space from `[Waypoints]`; no integer-linear fit over LocalSize/Size; ModEnc `StartX` page unfetchable — 403). Cross-ref against the FA2/FA2sp source or the `[IsoMapPack5]` work.
- Exact tile↔cell diamond transform (±1 offset ambiguity), to be pinned together with the iso pack decoder.
- What the 5 truncated-GBK files' lost bytes originally were (irrelevant for loading; lossy decode suffices).
- `[AITriggerTypes]` full 17-field semantics (only 12 files carry them; community docs exist but pages did not fetch).

## 10. Provenance

- Corpus walk + all censuses: `C:\Users\AI\AppData\Local\Temp\mapini\survey.py` → `out.txt`, `detail1.json` (pass 1); `survey2.py` → `out2.txt` (EOL/encoding, packs, waypoints, key shapes, owner vocab, misc key census, ranges); `survey3.py` → `out3.txt` (coord space, preview/specialflags/ranking/fa2sp censuses, anomaly pin-down, StartX/StartY fit); inline passes → `out4.txt` (Header.W−Local.w distribution, StartX candidate forms), `out5.txt` (W+H bound clean/viol, first-section census).
- Anomaly verification (files opened, exact lines printed): `verify.py`, `verify2.py` (unbalanced-header context + undecodable-byte contexts), `verify3.py` (ASCII-escaped names), `semicheck.py` (`;` position classification), `names.py` (largest file, corrupt-struct file, `(2)Percent` files, OOB waypoint file).
- Parser ground truth: `E:\hongjing\ra2-rs\crates\core\src\format\ini.rs` (152 lines, read in full).
- ModEnc: `/Maps` and `/Basic` fetched (data only); `/Map_Format` HTTP 500, `/StartX` 403.
- Repo untouched except this document; nothing committed.
