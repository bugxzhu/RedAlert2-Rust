# M0+M1a 格式层基础(INI/LCW/CRC/MIX/PAL + CLI)实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 建立 ra2-rs 仓库脚手架,并实现第一批格式解析器(INI、LCW、Westwood CRC、MIX 容器、PAL)加 `ra2-mix` 命令行工具,能对真实游戏 MIX 列目录、按名提取文件。

**Architecture:** cargo workspace,`ra2-core`(纯解析库,零外部依赖)+ `tools/ra2-mix`(CLI)。MIX 布局与 CRC/LCW 算法已从 RA1 GPL 源码与真实资产字节双向验证(见规格依据)。解析器全部接受内存字节切片,IO 只发生在 CLI 层。

**Tech Stack:** Rust stable(edition 2021),零第三方依赖,GitHub Actions CI。

**规格依据(计划中算法的出处,实现时不需要再考证):**
- LCW 解压:移植 `F:\redalert\CnC_Red_Alert\CODE\LCW.CPP`(GPL-3.0,可直接移植)
- Westwood CRC:移植 `CODE\CRC.CPP` + `CODE\CRC.H`;MIX 文件名 ID = CRC(ASCII 大写文件名)(`CODE\MIXFILE.CPP:538` 用法)
- MIX 布局(已用 `E:\hongjing\Reunion 2023` 下真实文件字节验证):
  - 头 10 字节:`u32 LE flags`(目标资产集全为 0)、`u16 LE count`、`u32 LE body_size`
  - 索引:count × 12 字节 `{u32 LE id; u32 LE offset(相对 body); u32 LE size}`
  - `data_start = 10 + count*12`;ares.mix 实测:count=4,entries[0]={id:0x8F96AFF1, offset:0, size:1760},data_start=0x3A,body 首字节为 " FSC"(CSF 魔数)
- 不支持加密索引(flags≠0):本资产集不存在,遇到时返回明确错误

## Global Constraints

- 许可证:GPL-3.0-only。移植自 RA1 源码的文件(lcw.rs、crc.rs)必须保留来源注释头(示例见任务内代码)
- 仓库不提交任何 EA 资产;提取出的样本一律放 `fixtures/`(已 gitignore);真实资产测试用环境变量 `RA2_ASSETS_DIR` 指向游戏目录,测试标 `#[ignore]`
- 每个 src 文件第一行:`// SPDX-License-Identifier: GPL-3.0-only`
- crate 布局遵循 spec §3:`crates/core` + `tools/`
- 每个任务收尾:`cargo fmt --all`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace` 全绿后才 commit
- commit 用 conventional commits(中文描述可)
- 零第三方依赖(blowfish 未在范围内;INI/LCW/CRC/MIX/PAL 全手写)
- 本计划不做:SHP/TMP/CSF/VXL/HVA/AUD、加密 MIX、GUI 查看器(属计划二)

---

### Task 1: 仓库脚手架 + CI

**Files:**
- Create: `Cargo.toml`(workspace 根)
- Create: `crates/core/Cargo.toml`、`crates/core/src/lib.rs`
- Create: `tools/ra2-mix/Cargo.toml`、`tools/ra2-mix/src/main.rs`
- Create: `.gitignore`、`.github/workflows/ci.yml`、`LICENSE`、`README.md`

**Interfaces:**
- Produces: workspace 成员 `ra2-core`(lib)与 `ra2-mix`(bin),后续任务向 `ra2-core` 添加 `format` 模块

- [ ] **Step 1: 写 workspace 根 `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = ["crates/core", "tools/ra2-mix"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "GPL-3.0-only"

[profile.release]
lto = true
```

- [ ] **Step 2: 写 `crates/core/Cargo.toml` 与 `crates/core/src/lib.rs`**

`crates/core/Cargo.toml`:
```toml
[package]
name = "ra2-core"
description = "RA2 engine reimplementation: format parsers and simulation core"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
```

`crates/core/src/lib.rs`:
```rust
// SPDX-License-Identifier: GPL-3.0-only

//! ra2-core:RA2 引擎重实现的纯逻辑层(格式解析 + 模拟)。

pub mod format;
```

`crates/core/src/format/mod.rs`(本任务先建占位,内容仅错误类型;后续任务追加 `pub mod`):
```rust
// SPDX-License-Identifier: GPL-3.0-only

//! 原版资产容器与数据格式解析。

use std::fmt;

/// 格式解析错误。
#[derive(Debug)]
pub enum FormatError {
    /// 输入数据不足。
    UnexpectedEof { needed: usize, remaining: usize },
    /// 格式合法但本版本不支持(如加密 MIX 索引)。
    Unsupported(String),
    /// 数据不符合格式。
    Invalid(String),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::UnexpectedEof { needed, remaining } => {
                write!(f, "数据不足:需要 {needed} 字节,剩余 {remaining}")
            }
            FormatError::Unsupported(s) => write!(f, "不支持:{}", s),
            FormatError::Invalid(s) => write!(f, "无效数据:{}", s),
        }
    }
}

impl std::error::Error for FormatError {}
```

- [ ] **Step 3: 写 `tools/ra2-mix/Cargo.toml` 与占位 `main.rs`**

`tools/ra2-mix/Cargo.toml`:
```toml
[package]
name = "ra2-mix"
description = "MIX archive command-line tool (list/extract/cat)"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "ra2-mix"
path = "src/main.rs"

[dependencies]
ra2-core = { path = "../../crates/core" }
```

`tools/ra2-mix/src/main.rs`:
```rust
// SPDX-License-Identifier: GPL-3.0-only

fn main() {
    eprintln!("ra2-mix: 尚未实现(Task 8 完成)");
    std::process::exit(2);
}
```

- [ ] **Step 4: 写 `.gitignore`、CI、README、LICENSE**

`.gitignore`:
```
/target
/fixtures
```

`.github/workflows/ci.yml`:
```yaml
name: ci
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - run: cargo fmt --all --check
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo test --workspace
```

`README.md`:
```markdown
# ra2-rs

红色警戒2(RA2/YR)引擎的 Rust 开源重实现。加载玩家自备的原版资产(MIX/INI/地图)。

- 设计文档:`docs/superpowers/specs/`
- 实施计划:`docs/superpowers/plans/`
- 许可证:GPL-3.0-only。本仓库不分发任何 EA 版权资产。

## 快速开始

    cargo build --release
    cargo test --workspace                 # 合成样本测试(CI 可跑)
    RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test --workspace -- --ignored
                                           # 真实资产集成测试(本地)

## ra2-mix

    ra2-mix list <file.mix>
    ra2-mix extract <file.mix> <文件名> -o <目录|->
    ra2-mix cat <file.mix> <文件名>
```

LICENSE 获取(GPL-3.0 全文,不手打):
```bash
curl -sL https://www.gnu.org/licenses/gpl-3.0.txt -o LICENSE
head -3 LICENSE   # 确认非空且为 GPL 文本
```

- [ ] **Step 5: 验证构建与测试**

Run: `cargo build --workspace && cargo test --workspace`
Expected: 编译成功,0 个测试(尚无 #[test]),退出码 0

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "chore: 仓库脚手架(workspace/CI/GPL 许可证)"
```

---

### Task 2: ByteReader 字节游标

**Files:**
- Create: `crates/core/src/format/bytes.rs`
- Modify: `crates/core/src/format/mod.rs`(追加 `pub mod bytes;`)

**Interfaces:**
- Consumes: `FormatError`(Task 1)
- Produces:
  `pub struct ByteReader<'a>`;方法 `new(&'a [u8]) -> Self`、`position(&self) -> usize`、`remaining(&self) -> usize`、`read_u8(&mut self) -> Result<u8, FormatError>`、`read_u16_le(&mut self) -> Result<u16, FormatError>`、`read_u32_le(&mut self) -> Result<u32, FormatError>`、`read_bytes(&mut self, n: usize) -> Result<&'a [u8], FormatError>`、`skip(&mut self, n: usize) -> Result<(), FormatError>`

- [ ] **Step 1: 写失败测试(追加到 `bytes.rs` 底部)**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! 小端字节游标。

use super::FormatError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian_values() {
        let data = [0x01, 0x02, 0x03, 0x04, 0xFF, 0x06, 0x07, 0x08];
        let mut r = ByteReader::new(&data);
        assert_eq!(r.read_u8().unwrap(), 0x01);
        assert_eq!(r.read_u16_le().unwrap(), 0x0302);
        assert_eq!(r.read_u32_le().unwrap(), 0x0807_06FF);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn read_bytes_returns_slice_and_advances() {
        let data = b"ABCDEF";
        let mut r = ByteReader::new(data);
        let s = r.read_bytes(3).unwrap();
        assert_eq!(s, b"ABC");
        assert_eq!(r.position(), 3);
        assert_eq!(r.remaining(), 3);
        r.skip(3).unwrap();
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn eof_is_reported() {
        let data = [0x01];
        let mut r = ByteReader::new(&data);
        assert!(matches!(
            r.read_u32_le(),
            Err(FormatError::UnexpectedEof { .. })
        ));
        assert!(r.read_bytes(2).is_err());
    }
}
```

- [ ] **Step 2: 运行确认编译失败**

Run: `cargo test -p ra2-core`
Expected: 编译错误 `cannot find type ByteReader`

- [ ] **Step 3: 实现 ByteReader(测试代码之前)**

```rust
pub struct ByteReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ByteReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        ByteReader { data, pos: 0 }
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    pub fn read_u8(&mut self) -> Result<u8, FormatError> {
        let b = self.read_bytes(1)?;
        Ok(b[0])
    }

    pub fn read_u16_le(&mut self) -> Result<u16, FormatError> {
        let b = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn read_u32_le(&mut self) -> Result<u32, FormatError> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], FormatError> {
        if self.remaining() < n {
            return Err(FormatError::UnexpectedEof {
                needed: n,
                remaining: self.remaining(),
            });
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    pub fn skip(&mut self, n: usize) -> Result<(), FormatError> {
        self.read_bytes(n).map(|_| ())
    }
}
```

- [ ] **Step 4: 运行测试通过**

Run: `cargo test -p ra2-core`
Expected: 3 passed

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/format/
git commit -m "feat(core): ByteReader 字节游标"
```

---

### Task 3: Westwood CRC 与 MIX 文件名 ID

**Files:**
- Create: `crates/core/src/format/crc.rs`
- Modify: `crates/core/src/format/mod.rs`(追加 `pub mod crc;`)

**Interfaces:**
- Produces:
  `pub fn westwood_crc(data: &[u8]) -> u32`
  `pub fn mix_name_id(name: &str) -> u32`(ASCII 大写化后求 CRC)
- 算法(移植 CRC.CPP/CRC.H,勿改语义):
  - `crc=0`;每 4 字节按 LE 组成 u32 `staged`,执行 `crc = crc.rotate_left(1).wrapping_add(staged)`
  - 结束时若有余数(1..3 字节)已按 LE 部分组入 staged:`返回 crc.rotate_left(1).wrapping_add(staged)`;整除则返回 `crc`
  - 空输入返回 0

- [ ] **Step 1: 写失败测试**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! Westwood CRC 引擎与 MIX 文件名 ID。
//!
//! 移植自 Red Alert 源码 CRC.CPP / CRC.H / MIXFILE.CPP(原版权 Electronic Arts,GPL-3.0)。

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_zero() {
        assert_eq!(westwood_crc(b""), 0);
    }

    #[test]
    fn single_byte() {
        // staged=0x54, crc = rol(0,1)+0x54
        assert_eq!(westwood_crc(b"T"), 0x54);
    }

    #[test]
    fn one_full_chunk() {
        // "TEST" -> LE u32 0x54534554, crc = 0 + 0x54534554
        assert_eq!(westwood_crc(b"TEST"), 0x5453_4554);
    }

    #[test]
    fn two_full_chunks() {
        // rol(0x54534554,1)=0xA8A68AA8, +0x54534554 = 0xFCF9CFFC
        assert_eq!(westwood_crc(b"TESTTEST"), 0xFCF9_CFFC);
    }

    #[test]
    fn remainder_bytes_fold_on_finish() {
        // 手算:RULESMD.INI -> 0x72033E38
        // 该向量由算法手推;最终裁决是集成测试(ra2md.mix 中必须找到 rulesmd.ini)
        assert_eq!(westwood_crc(b"RULESMD.INI"), 0x7203_3E38);
    }

    #[test]
    fn name_id_uppercases() {
        assert_eq!(mix_name_id("rulesmd.ini"), mix_name_id("RulesMD.ini"));
        assert_eq!(mix_name_id("rulesmd.ini"), westwood_crc(b"RULESMD.INI"));
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p ra2-core crc`
Expected: 编译错误 `cannot find function westwood_crc`

- [ ] **Step 3: 实现**

```rust
pub fn westwood_crc(data: &[u8]) -> u32 {
    let mut crc: u32 = 0;
    let mut staging: u32 = 0;
    let mut index: usize = 0;
    for &b in data {
        staging |= (b as u32) << (8 * index);
        index += 1;
        if index == 4 {
            crc = crc.rotate_left(1).wrapping_add(staging);
            staging = 0;
            index = 0;
        }
    }
    if index != 0 {
        crc = crc.rotate_left(1).wrapping_add(staging);
    }
    crc
}

pub fn mix_name_id(name: &str) -> u32 {
    let upper: Vec<u8> = name.to_ascii_uppercase().into_bytes();
    westwood_crc(&upper)
}
```

- [ ] **Step 4: 运行测试通过**

Run: `cargo test -p ra2-core crc`
Expected: 6 passed。若 `remainder_bytes_fold_on_finish` 失败而其余通过,先核对实现与算法描述;若实现与描述一致,删除该手算向量并在 commit message 注明(集成测试才是裁决)

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/format/
git commit -m "feat(core): Westwood CRC 与 MIX 文件名 ID(移植 RA1)"
```

---

### Task 4: LCW 解压

**Files:**
- Create: `crates/core/src/format/lcw.rs`
- Modify: `crates/core/src/format/mod.rs`(追加 `pub mod lcw;`)

**Interfaces:**
- Produces: `pub fn lcw_decode(src: &[u8], out: &mut Vec<u8>) -> Result<usize, FormatError>` — 返回写入总字节数(即结束时 `out.len()`)
- 操作码语义(移植 LCW.CPP,全部已从源码核对):
  - `op < 0x80`:短回拷。`count = (op>>4)+3`;回距 `back = src下一字节 + ((op&0x0F)<<8)`,`back` 不允许为 0 或超出已输出长度
  - `op == 0x80`:结束
  - `0x81..=0xBF`:源字面量。`count = op & 0x3F`(可为 0)
  - `0xC0..=0xFD`(即 0x40 位为 1 且非 FE/FF):中回拷。`count = (op&0x3F)+3`;`off = u16 LE`(绝对位置,自 out 起始)
  - `op == 0xFE`:长填充。`count = u16 LE`;填充字节 = 下一字节
  - `op == 0xFF`:长回拷。`count = u16 LE`;`off = u16 LE`(绝对位置)
  - 回拷一律逐字节复制(允许重叠,即 RLE 式展开)

- [ ] **Step 1: 写失败测试**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! LCW(Format 80)解压。
//!
//! 移植自 Red Alert 源码 LCW.CPP(原版权 Electronic Arts,GPL-3.0)。

use super::FormatError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_marker_only() {
        let mut out = Vec::new();
        assert_eq!(lcw_decode(&[0x80], &mut out).unwrap(), 0);
        assert!(out.is_empty());
    }

    #[test]
    fn literals_short() {
        // 0x83: 复制 3 个字面量
        let mut out = Vec::new();
        lcw_decode(&[0x83, b'A', b'B', b'C', 0x80], &mut out).unwrap();
        assert_eq!(out, b"ABC");
    }

    #[test]
    fn short_back_copy_replicates() {
        // 先输出 "AB"(0x82 + 2 字面量),再 op=0x00: count=(0>>4)+3=3, back=0x02
        // 重叠回拷:AB -> 追加 A,B,A -> "ABABA"
        let mut out = Vec::new();
        lcw_decode(&[0x82, b'A', b'B', 0x00, 0x02, 0x80], &mut out).unwrap();
        assert_eq!(out, b"ABABA");
    }

    #[test]
    fn long_run() {
        // 0xFE, count=4 LE, byte=0x77 -> 4 字节 0x77
        let mut out = Vec::new();
        lcw_decode(&[0xFE, 0x04, 0x00, 0x77, 0x80], &mut out).unwrap();
        assert_eq!(out, vec![0x77u8; 4]);
    }

    #[test]
    fn medium_back_copy_absolute() {
        // 字面量 "XY",然后中回拷 op=0xC5: count=(5)+3=8, off=0x0000 -> "XY" 重复到 10 字节
        let mut out = Vec::new();
        lcw_decode(&[0x82, b'X', b'Y', 0xC5, 0x00, 0x00, 0x80], &mut out).unwrap();
        assert_eq!(out, b"XYXYXYXYXY");
    }

    #[test]
    fn long_back_copy_absolute() {
        // 字面量 "XY",然后长回拷 op=0xFF: count=3, off=0 -> 追加 "XYX"
        let mut out = Vec::new();
        lcw_decode(&[0x82, b'X', b'Y', 0xFF, 0x03, 0x00, 0x00, 0x00, 0x80], &mut out).unwrap();
        assert_eq!(out, b"XYXYX");
    }

    #[test]
    fn bad_back_offset_is_rejected() {
        let mut out = Vec::new();
        assert!(matches!(
            lcw_decode(&[0x01, 0x01, 0x80], &mut out), // back=1 但 out 为空
            Err(FormatError::Invalid(_))
        ));
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p ra2-core lcw`
Expected: 编译错误 `cannot find function lcw_decode`

- [ ] **Step 3: 实现**

```rust
pub fn lcw_decode(src: &[u8], out: &mut Vec<u8>) -> Result<usize, FormatError> {
    let mut sp = 0usize;
    // 读一个字节并前进(显式递增,避免宏/闭包重复读取同一位置)
    fn next(src: &[u8], sp: &mut usize) -> Result<u8, FormatError> {
        let b = *src.get(*sp).ok_or(FormatError::UnexpectedEof {
            needed: 1,
            remaining: 0,
        })?;
        *sp += 1;
        Ok(b)
    }
    fn next_u16(src: &[u8], sp: &mut usize) -> Result<u16, FormatError> {
        let lo = next(src, sp)? as u16;
        let hi = next(src, sp)? as u16;
        Ok(lo | (hi << 8))
    }
    // 从 out 的绝对位置 p 起逐字节复制 count 个(允许重叠,等价 RA1 的 copy_ptr++ 循环)
    fn copy_from_out(out: &mut Vec<u8>, p: usize, count: usize) {
        let mut p = p;
        for _ in 0..count {
            let byte = out[p];
            out.push(byte);
            p += 1;
        }
    }
    loop {
        let op = next(src, &mut sp)?;
        if op & 0x80 == 0 {
            // 短回拷(相对)
            let b = next(src, &mut sp)?;
            let count = ((op >> 4) as usize) + 3;
            let back = (b as usize) + (((op & 0x0F) as usize) << 8);
            if back == 0 || back > out.len() {
                return Err(FormatError::Invalid(format!("LCW 短回拷 back={back} 无效")));
            }
            let start = out.len() - back;
            copy_from_out(out, start, count);
        } else if op & 0x40 == 0 {
            if op == 0x80 {
                return Ok(out.len());
            }
            // 源字面量
            let count = (op & 0x3F) as usize;
            let end = sp + count;
            let bytes = src.get(sp..end).ok_or(FormatError::UnexpectedEof {
                needed: count,
                remaining: src.len() - sp,
            })?;
            sp = end;
            out.extend_from_slice(bytes);
        } else if op == 0xFE {
            // 长填充
            let count = next_u16(src, &mut sp)? as usize;
            let byte = next(src, &mut sp)?;
            out.extend(std::iter::repeat(byte).take(count));
        } else if op == 0xFF {
            // 长回拷(绝对)
            let count = next_u16(src, &mut sp)? as usize;
            let off = next_u16(src, &mut sp)? as usize;
            if off >= out.len() {
                return Err(FormatError::Invalid(format!("LCW 长回拷 off={off} 越界")));
            }
            copy_from_out(out, off, count);
        } else {
            // 中回拷(绝对)
            let count = ((op & 0x3F) as usize) + 3;
            let off = next_u16(src, &mut sp)? as usize;
            if off >= out.len() {
                return Err(FormatError::Invalid(format!("LCW 中回拷 off={off} 越界")));
            }
            copy_from_out(out, off, count);
        }
    }
}
```

- [ ] **Step 4: 运行测试通过**

Run: `cargo test -p ra2-core lcw`
Expected: 7 passed

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/format/
git commit -m "feat(core): LCW 解压(移植 RA1 LCW.CPP)"
```

---

### Task 5: MIX 容器读取

**Files:**
- Create: `crates/core/src/format/mix.rs`
- Modify: `crates/core/src/format/mod.rs`(追加 `pub mod mix;`)
- Create: `crates/core/tests/mix_integration.rs`(真实资产集成测试,`#[ignore]`)

**Interfaces:**
- Consumes: `ByteReader`、`FormatError`(Task 2)、`mix_name_id`(Task 3)
- Produces:
  `pub struct MixEntry { pub id: u32, pub offset: u32, pub size: u32 }`
  `pub struct MixArchive { /* 私有 */ }`;方法:
  - `pub fn parse(data: Vec<u8>) -> Result<Self, FormatError>`
  - `pub fn entries(&self) -> &[MixEntry]`
  - `pub fn find_id(&self, id: u32) -> Option<&MixEntry>`
  - `pub fn find(&self, name: &str) -> Option<&MixEntry>`
  - `pub fn read(&self, name: &str) -> Result<&[u8], FormatError>`
  - `pub fn read_by_id(&self, id: u32) -> Result<&[u8], FormatError>`
- 布局(已验证,勿改):头 `[u32 flags][u16 count][u32 body_size]`;索引项 `{id,offset,size}` 各 u32 LE,offset 相对 body;`data_start = 10 + count*12`;`flags != 0` → `FormatError::Unsupported("加密 MIX 索引暂不支持")`

- [ ] **Step 1: 写失败单元测试(mix.rs 内,合成数据)**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! TS/RA2 时代 MIX 容器(明文索引)。

use super::bytes::ByteReader;
use super::crc::mix_name_id;
use super::FormatError;

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个含 2 个条目的最小合法 MIX:
    /// body = "HELLO"(5字节, 由名 HELLO 的 ID 索引) + "RA2!"
    fn sample_mix() -> Vec<u8> {
        let id_hello = mix_name_id("HELLO");
        let id_ra2 = mix_name_id("RA2");
        let body: Vec<u8> = b"HELLO".iter().chain(b"RA2!").cloned().collect();
        let mut v = Vec::new();
        v.extend_from_slice(&0u32.to_le_bytes()); // flags
        v.extend_from_slice(&2u16.to_le_bytes()); // count
        v.extend_from_slice(&(body.len() as u32).to_le_bytes()); // body_size
        for (id, off, size) in [(id_hello, 0u32, 5u32), (id_ra2, 5u32, 4u32)] {
            v.extend_from_slice(&id.to_le_bytes());
            v.extend_from_slice(&off.to_le_bytes());
            v.extend_from_slice(&size.to_le_bytes());
        }
        v.extend_from_slice(&body);
        v
    }

    #[test]
    fn parses_header_and_index() {
        let mix = MixArchive::parse(sample_mix()).unwrap();
        assert_eq!(mix.entries().len(), 2);
        assert_eq!(mix.entries()[0].id, mix_name_id("HELLO"));
        assert_eq!(mix.entries()[1].offset, 5);
    }

    #[test]
    fn finds_and_reads_by_name() {
        let mix = MixArchive::parse(sample_mix()).unwrap();
        assert_eq!(mix.read("hello").unwrap(), b"HELLO"); // 名字大小写不敏感
        assert_eq!(mix.read("RA2").unwrap(), b"RA2!");
        assert!(mix.find("不存在").is_none());
    }

    #[test]
    fn rejects_encrypted_index() {
        let mut data = sample_mix();
        data[0..4].copy_from_slice(&0x0001_0000u32.to_le_bytes());
        assert!(matches!(
            MixArchive::parse(data),
            Err(FormatError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_entry_out_of_range() {
        let mut data = sample_mix();
        // 把第 2 个条目 size 改成越界值(索引项1 的 size 位于 10+12+8)
        let pos = 10 + 12 + 8;
        data[pos..pos + 4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        assert!(matches!(
            MixArchive::parse(data),
            Err(FormatError::Invalid(_))
        ));
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p ra2-core mix`
Expected: 编译错误 `cannot find type MixArchive`

- [ ] **Step 3: 实现**

```rust
pub struct MixEntry {
    pub id: u32,
    pub offset: u32,
    pub size: u32,
}

pub struct MixArchive {
    data: Vec<u8>,
    entries: Vec<MixEntry>,
    data_start: usize,
}

impl MixArchive {
    pub fn parse(data: Vec<u8>) -> Result<Self, FormatError> {
        let mut r = ByteReader::new(&data);
        let flags = r.read_u32_le()?;
        if flags != 0 {
            return Err(FormatError::Unsupported(format!(
                "加密 MIX 索引暂不支持 (flags=0x{flags:08X})"
            )));
        }
        let count = r.read_u16_le()? as usize;
        let _body_size = r.read_u32_le()?;
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            entries.push(MixEntry {
                id: r.read_u32_le()?,
                offset: r.read_u32_le()?,
                size: r.read_u32_le()?,
            });
        }
        let data_start = r.position();
        for e in &entries {
            let end = e.offset as u64 + e.size as u64;
            if end > (data.len() - data_start) as u64 {
                return Err(FormatError::Invalid(format!(
                    "条目越界: id=0x{:08X} offset={} size={}",
                    e.id, e.offset, e.size
                )));
            }
        }
        Ok(MixArchive { data, entries, data_start })
    }

    pub fn entries(&self) -> &[MixEntry] {
        &self.entries
    }

    pub fn find_id(&self, id: u32) -> Option<&MixEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn find(&self, name: &str) -> Option<&MixEntry> {
        self.find_id(mix_name_id(name))
    }

    pub fn read(&self, name: &str) -> Result<&[u8], FormatError> {
        let e = self.find(name).ok_or_else(|| {
            FormatError::Invalid(format!("MIX 中找不到文件: {name}"))
        })?;
        let s = self.data_start + e.offset as usize;
        Ok(&self.data[s..s + e.size as usize])
    }

    pub fn read_by_id(&self, id: u32) -> Result<&[u8], FormatError> {
        let e = self.find_id(id).ok_or_else(|| {
            FormatError::Invalid(format!("MIX 中找不到 ID: 0x{id:08X}"))
        })?;
        let s = self.data_start + e.offset as usize;
        Ok(&self.data[s..s + e.size as usize])
    }
}
```

- [ ] **Step 4: 运行单元测试通过**

Run: `cargo test -p ra2-core mix`
Expected: 4 passed

- [ ] **Step 5: 写真实资产集成测试 `crates/core/tests/mix_integration.rs`**

```rust
// SPDX-License-Identifier: GPL-3.0-only
//! 真实资产集成测试。需要环境变量 RA2_ASSETS_DIR 指向游戏目录(含原版 MIX)。
//! 运行:RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core -- --ignored

use ra2_core::format::mix::MixArchive;

fn assets_dir() -> std::path::PathBuf {
    std::env::var("RA2_ASSETS_DIR")
        .expect("需要环境变量 RA2_ASSETS_DIR")
        .into()
}

fn load(name: &str) -> MixArchive {
    let path = assets_dir().join(name);
    let data = std::fs::read(&path).unwrap_or_else(|e| panic!("读取 {}: {e}", path.display()));
    MixArchive::parse(data).expect("解析 MIX")
}

/// ares.mix 实测布局(计划文档规格依据):count=4,首条目 id/offset/size,首文件为 CSF。
#[test]
#[ignore]
fn ares_mix_matches_verified_layout() {
    let mix = load("ares.mix");
    assert_eq!(mix.entries().len(), 4);
    let e0 = &mix.entries()[0];
    assert_eq!(e0.id, 0x8F96_AFF1);
    assert_eq!(e0.offset, 0);
    assert_eq!(e0.size, 1760);
    let data = mix.read_by_id(0x8F96_AFF1).expect("读取首条目");
    assert_eq!(&data[0..4], b" FSC", "首条目应为 CSF 字符串表");
}

/// rulesmd.ini 必须能按名从 ra2md.mix 中找到(裁决 mix_name_id 正确性)。
#[test]
#[ignore]
fn ra2md_contains_rulesmd_ini() {
    let mix = load("ra2md.mix");
    assert!(mix.find("rulesmd.ini").is_some(), "mix_name_id 或 MIX 解析有误");
}

/// 社区重打包的扩张 MIX 应包含 xmd(本地混合数据库)。
#[test]
#[ignore]
fn expandmd01_contains_local_mix_database() {
    let mix = load("expandmd01.mix");
    assert!(mix.find("local mix database.dat").is_some());
}
```

- [ ] **Step 6: 运行集成测试(真实资产)**

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core -- --ignored`
Expected: 3 passed。若 `ra2md_contains_rulesmd_ini` 失败:用 `ra2-mix`(Task 8)或临时调试打印核对 CRC 实现与算法描述;本测试是 mix_name_id 的最终裁决

- [ ] **Step 7: Commit**

```bash
git add crates/core/
git commit -m "feat(core): MIX 容器解析(明文索引)+ 真实资产集成测试"
```

---

### Task 6: INI 解析器

**Files:**
- Create: `crates/core/src/format/ini.rs`
- Modify: `crates/core/src/format/mod.rs`(追加 `pub mod ini;`)
- Modify: `crates/core/tests/mix_integration.rs`(追加 rulesmd 解析断言)

**Interfaces:**
- Produces:
  `pub struct IniDocument { pub sections: Vec<IniSection> }`
  `pub struct IniSection { pub name: String, pub entries: Vec<(String, String)> }`
  - `IniDocument::parse(text: &str) -> IniDocument`(解析失败不报错,跳过垃圾行)
  - `IniDocument::section(&self, name: &str) -> Option<&IniSection>`(大小写不敏感)
  - `IniDocument::value(&self, section: &str, key: &str) -> Option<&str>`
  - `IniDocument::get_int(&self, section: &str, key: &str, default: i64) -> i64`
  - `IniDocument::get_bool(&self, section: &str, key: &str, default: bool) -> bool`(接受 yes/no/true/false/1/0,大小写不敏感)
  - `IniSection::value(&self, key: &str) -> Option<&str>`
- 语法规则:行首 `;` 为注释行;`[名字]` 开节;`k=v` 首个 `=` 分割,两侧去空白;同名节合并;同节同名键后者覆盖前者

- [ ] **Step 1: 写失败测试**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! Westwood 风格 INI 解析(rulesmd.ini / 地图 / spawn.ini 共用)。

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
; 顶层注释
[General]
BuildSpeed=0.7
GameSpeedBufferFrames = 0  ; 行尾分号保留在值里
YesKey=yes

[general]
BuildSpeed=1.5

[Units]
List=HTNK,APOC
";

    #[test]
    fn sections_case_insensitive_and_merged() {
        let doc = IniDocument::parse(SAMPLE);
        // [General] 与 [general] 合并,同名键后者覆盖
        assert_eq!(doc.value("GENERAL", "BuildSpeed"), Some("1.5"));
        assert_eq!(doc.sections.len(), 2); // General + Units
    }

    #[test]
    fn values_trimmed_and_comment_lines_skipped() {
        let doc = IniDocument::parse(SAMPLE);
        assert_eq!(
            doc.value("General", "GameSpeedBufferFrames"),
            Some("0  ; 行尾分号保留在值里")
        );
        assert_eq!(doc.value("General", "YesKey"), Some("yes"));
        assert_eq!(doc.value("Units", "List"), Some("HTNK,APOC"));
        assert_eq!(doc.value("General", "不存在"), None);
    }

    #[test]
    fn typed_accessors() {
        let doc = IniDocument::parse(SAMPLE);
        assert_eq!(doc.get_int("Units", "List", -1), -1); // 非整数 -> 默认
        assert_eq!(doc.get_bool("General", "YesKey", false), true);
        // 非布尔字符串按 Westwood 语义视为 no(不回落默认值)
        assert_eq!(doc.get_bool("General", "BuildSpeed", true), false);
        assert_eq!(doc.get_bool("General", "键不存在", true), true); // 键缺失 -> 默认
    }

    #[test]
    fn garbage_lines_are_skipped() {
        let doc = IniDocument::parse("垃圾行\n[OK]\nK=V\n=无键\n[未闭合\n");
        assert_eq!(doc.value("OK", "K"), Some("V"));
        assert_eq!(doc.sections.len(), 1);
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p ra2-core ini`
Expected: 编译错误 `cannot find type IniDocument`

- [ ] **Step 3: 实现**

```rust
pub struct IniSection {
    pub name: String,
    pub entries: Vec<(String, String)>,
}

pub struct IniDocument {
    pub sections: Vec<IniSection>,
}

impl IniDocument {
    pub fn parse(text: &str) -> IniDocument {
        let mut doc = IniDocument { sections: Vec::new() };
        let mut current: Option<usize> = None;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                let name = name.trim().to_string();
                match doc.sections.iter().position(|s| s.name.eq_ignore_ascii_case(&name)) {
                    Some(i) => current = Some(i),
                    None => {
                        doc.sections.push(IniSection { name, entries: Vec::new() });
                        current = Some(doc.sections.len() - 1);
                    }
                }
            } else if let Some((k, v)) = line.split_once('=') {
                let key = k.trim();
                if key.is_empty() {
                    continue;
                }
                if let Some(i) = current {
                    let entries = &mut doc.sections[i].entries;
                    match entries.iter_mut().find(|(ek, _)| ek.eq_ignore_ascii_case(key)) {
                        Some(slot) => slot.1 = v.trim().to_string(),
                        None => entries.push((key.to_string(), v.trim().to_string())),
                    }
                }
            }
            // 其余行(垃圾/无节键值)跳过
        }
        doc
    }

    pub fn section(&self, name: &str) -> Option<&IniSection> {
        self.sections
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    pub fn value(&self, section: &str, key: &str) -> Option<&str> {
        self.section(section)?.value(key)
    }

    pub fn get_int(&self, section: &str, key: &str, default: i64) -> i64 {
        self.value(section, key)
            .and_then(|v| v.trim().parse::<i64>().ok())
            .unwrap_or(default)
    }

    pub fn get_bool(&self, section: &str, key: &str, default: bool) -> bool {
        match self.value(section, key).map(|v| v.trim().to_ascii_lowercase()) {
            Some(v) => matches!(v.as_str(), "yes" | "true" | "1"),
            None => default,
        }
    }
}

impl IniSection {
    pub fn value(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }
}
```

- [ ] **Step 4: 运行测试通过**

Run: `cargo test -p ra2-core ini`
Expected: 4 passed

- [ ] **Step 5: 集成测试追加 rulesmd 断言(`tests/mix_integration.rs` 末尾)**

```rust
/// 从 ra2md.mix 提取 rulesmd.ini 并解析(MIX+INI 联合裁决)。
#[test]
#[ignore]
fn rulesmd_ini_parses_from_mix() {
    use ra2_core::format::ini::IniDocument;
    let mix = load("ra2md.mix");
    let text = std::str::from_utf8(mix.read("rulesmd.ini").expect("提取 rulesmd.ini"))
        .expect("rulesmd.ini 应为 UTF-8/ASCII 文本");
    let doc = IniDocument::parse(text);
    assert!(doc.section("General").is_some());
    assert!(doc.section("InfantryTypes").is_some());
    assert!(doc.section("VehicleTypes").is_some());
    assert!(doc.section("BuildingTypes").is_some());
    assert!(doc.sections.len() > 100, "rulesmd.ini 应有大量节");
}
```

Run: `RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core -- --ignored`
Expected: 4 passed

- [ ] **Step 6: Commit**

```bash
git add crates/core/
git commit -m "feat(core): INI 解析器(合并/覆盖/大小写不敏感)"
```

---

### Task 7: PAL 调色板

**Files:**
- Create: `crates/core/src/format/pal.rs`
- Modify: `crates/core/src/format/mod.rs`(追加 `pub mod pal;`)

**Interfaces:**
- Produces:
  `pub struct Palette { pub rgb: [[u8; 3]; 256] }`
  - `Palette::parse(data: &[u8]) -> Result<Self, FormatError>`(要求恰好 768 字节)
  - `pub fn rgba(&self, index: usize) -> [u8; 4]`(alpha 恒 255)

- [ ] **Step 1: 写失败测试**

```rust
// SPDX-License-Identifier: GPL-3.0-only

//! VGA 768 字节调色板(RA2 各 theater 通用 .pal)。

use super::FormatError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_768_bytes() {
        let mut data = [0u8; 768];
        data[0] = 10;
        data[1] = 20;
        data[2] = 30;
        let pal = Palette::parse(&data).unwrap();
        assert_eq!(pal.rgb[0], [10, 20, 30]);
        assert_eq!(pal.rgb[255], [0, 0, 0]);
        assert_eq!(pal.rgba(0), [10, 20, 30, 255]);
    }

    #[test]
    fn rejects_wrong_size() {
        assert!(matches!(
            Palette::parse(&[0u8; 767]),
            Err(FormatError::Invalid(_))
        ));
        assert!(matches!(
            Palette::parse(&[0u8; 769]),
            Err(FormatError::Invalid(_))
        ));
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p ra2-core pal`
Expected: 编译错误 `cannot find type Palette`

- [ ] **Step 3: 实现**

```rust
pub struct Palette {
    pub rgb: [[u8; 3]; 256],
}

impl Palette {
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        if data.len() != 768 {
            return Err(FormatError::Invalid(format!(
                "调色板应为 768 字节,实际 {}",
                data.len()
            )));
        }
        let mut rgb = [[0u8; 3]; 256];
        for (i, slot) in rgb.iter_mut().enumerate() {
            *slot = [data[i * 3], data[i * 3 + 1], data[i * 3 + 2]];
        }
        Ok(Palette { rgb })
    }

    pub fn rgba(&self, index: usize) -> [u8; 4] {
        let [r, g, b] = self.rgb[index];
        [r, g, b, 255]
    }
}
```

- [ ] **Step 4: 运行测试通过**

Run: `cargo test -p ra2-core pal`
Expected: 2 passed

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/format/
git commit -m "feat(core): PAL 调色板加载"
```

---

### Task 8: ra2-mix CLI(list / extract / cat)

**Files:**
- Modify: `tools/ra2-mix/src/main.rs`(完整替换占位实现)
- Create: `tools/ra2-mix/tests/cli.rs`(调用自身二进制的冒烟测试)

**Interfaces:**
- Consumes: `MixArchive`(Task 5)
- Produces: 可执行文件 `ra2-mix`,用法:
  - `ra2-mix list <file.mix>` → 每行 `id offset size`
  - `ra2-mix extract <file.mix> <文件名> -o <目录|->` → 写 `<目录>/<文件名>`;`-o -` 时写到 stdout
  - `ra2-mix cat <file.mix> <文件名>` → 字节流写 stdout(等价 extract -o -)
  - 退出码:0 成功;1 找不到文件/解析失败;2 用法错误

- [ ] **Step 1: 写失败测试 `tools/ra2-mix/tests/cli.rs`**

```rust
// SPDX-License-Identifier: GPL-3.0-only

use std::process::{Command, Stdio};

fn bin() -> std::path::PathBuf {
    env!("CARGO_BIN_EXE_ra2-mix").into()
}

/// 用 ra2-core 现场构造一个合成 MIX 供 CLI 测试。
fn make_mix(path: &std::path::Path) {
    use ra2_core::format::crc::mix_name_id;
    let body: Vec<u8> = b"HELLO".to_vec();
    let mut v = Vec::new();
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&(body.len() as u32).to_le_bytes());
    let id = mix_name_id("HELLO");
    v.extend_from_slice(&id.to_le_bytes());
    v.extend_from_slice(&0u32.to_le_bytes());
    v.extend_from_slice(&5u32.to_le_bytes());
    v.extend_from_slice(&body);
    std::fs::write(path, v).unwrap();
}

#[test]
fn list_and_cat_work() {
    let dir = std::env::temp_dir().join("ra2-mix-cli-test");
    std::fs::create_dir_all(&dir).unwrap();
    let mix_path = dir.join("t.mix");
    make_mix(&mix_path);

    let out = Command::new(bin())
        .args(["list", mix_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(format!("{:08X}", mix_name_id("HELLO")).as_str()));

    let out = Command::new(bin())
        .args(["cat", mix_path.to_str().unwrap(), "hello"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"HELLO");
}

#[test]
fn missing_file_exits_nonzero() {
    let dir = std::env::temp_dir().join("ra2-mix-cli-test");
    std::fs::create_dir_all(&dir).unwrap();
    let mix_path = dir.join("t.mix");
    make_mix(&mix_path);
    let out = Command::new(bin())
        .args(["cat", mix_path.to_str().unwrap(), "nope.ini"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
}
```

测试 crate 需要能 `use ra2_core` — 在 `tools/ra2-mix/Cargo.toml` 的 `[dependencies]` 已有(构造合成 mix 用)。另外 `[dev-dependencies]` 不需要新增。

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p ra2-mix`
Expected: 断言失败(list 输出为空,当前 main 直接 exit 2)

- [ ] **Step 3: 实现 main.rs**

```rust
// SPDX-License-Identifier: GPL-3.0-only

use ra2_core::format::mix::MixArchive;
use std::io::Write;
use std::process::ExitCode;

fn usage() -> ! {
    eprintln!("用法: ra2-mix list <file.mix>");
    eprintln!("      ra2-mix extract <file.mix> <文件名> -o <目录|->");
    eprintln!("      ra2-mix cat <file.mix> <文件名>");
    std::process::exit(2);
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, mix_path, rest @ ..] => run(cmd, mix_path, rest),
        _ => usage(),
    }
}

fn run(cmd: &str, mix_path: &str, rest: &[String]) -> ExitCode {
    let data = match std::fs::read(mix_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("读取 {mix_path} 失败: {e}");
            return ExitCode::from(1);
        }
    };
    let mix = match MixArchive::parse(data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("解析失败: {e}");
            return ExitCode::from(1);
        }
    };
    match (cmd, rest) {
        ("list", []) => {
            for e in mix.entries() {
                println!("{:08X} {:>10} {:>10}", e.id, e.offset, e.size);
            }
            ExitCode::SUCCESS
        }
        ("cat", [name]) => write_out(&mix, name, None),
        ("extract", [name, out_flag, out_target]) if out_flag == "-o" => {
            if out_target == "-" {
                write_out(&mix, name, None)
            } else {
                write_out(&mix, name, Some(out_target))
            }
        }
        _ => usage(),
    }
}

fn write_out(mix: &MixArchive, name: &str, to_dir: Option<&str>) -> ExitCode {
    let data = match mix.read(name) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    match to_dir {
        None => {
            std::io::stdout().write_all(data).unwrap();
        }
        Some(dir) => {
            let path = std::path::Path::new(dir).join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, data).unwrap_or_else(|e| panic!("写 {}: {e}", path.display()));
            println!("已写入 {}", path.display());
        }
    }
    ExitCode::SUCCESS
}
```

- [ ] **Step 4: 运行全部测试通过**

Run: `cargo test --workspace`
Expected: 单元+CLI 测试全绿(ignored 的 4 个除外)

- [ ] **Step 5: 真实资产手工验收**

```bash
cargo run --release -p ra2-mix -- list "E:\hongjing\Reunion 2023\ares.mix"
# 期望:4 行,第一行 8F96AFF1 0 1760
cargo run --release -p ra2-mix -- extract "E:\hongjing\Reunion 2023\ra2md.mix" rulesmd.ini -o fixtures
# 期望:已写入 fixtures\rulesmd.ini
head -5 fixtures/rulesmd.ini
# 期望:rulesmd.ini 文本内容(注释/节名)
```

- [ ] **Step 6: Commit**

```bash
git add tools/ crates/core/
git commit -m "feat(tools): ra2-mix CLI(list/extract/cat)"
```

---

## 完成定义(计划级验收)

1. `cargo test --workspace` 全绿;`RA2_ASSETS_DIR=... cargo test --workspace -- --ignored` 4 个集成测试全绿
2. `ra2-mix` 能对重聚2023 目录全部 MIX(ares/expandmd01/ra2md/ra2/language/langmd/thememd)执行 list 不 panic(可在 Step 5 顺带抽查 2-3 个)
3. clippy `-D warnings` 零告警,fmt 通过
4. 仓库不含任何 EA 资产文件(fixtures/ 已忽略)

## 移交给计划二的内容(不属本计划)

- SHP/TMP/CSF/VXL/HVA/AUD 解码(用本计划的 `ra2-mix` 从真实 MIX 提取样本做规格取证)
- 加密 MIX 索引(flags≠0,如需要再立项)
- GUI 资产查看器、xmd(local mix database.dat)解析
