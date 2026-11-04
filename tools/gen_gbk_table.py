#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""生成 CP936(GBK)双字节区 → Unicode 查找表(地图名有损解码用)。

用法:python tools/gen_gbk_table.py crates/core/src/format/map/gbk_table.rs
依赖 Python 3 标准库 gbk 编解码器(全平台内置);表生成后由 gbk.rs 的
钉字节单测验证正确性。布局:lead 0x81..=0xFE(126 个)× trail 0x40..=0xFE
除 0x7F(190 个)= 23940 项,未定义码位填 0xFFFD。
"""
import sys


def main() -> None:
    out = sys.argv[1] if len(sys.argv) > 1 else "crates/core/src/format/map/gbk_table.rs"
    chars = []
    for lead in range(0x81, 0xFF):
        for trail in range(0x40, 0xFF):
            if trail == 0x7F:
                continue
            try:
                chars.append(bytes([lead, trail]).decode("gbk"))
            except UnicodeDecodeError:
                chars.append("�")
    assert len(chars) == 23940, len(chars)
    with open(out, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("// SPDX-License-Identifier: GPL-3.0-only\n\n")
        fh.write("//! CP936(GBK)双字节区 → Unicode 查找表。\n//!\n")
        fh.write("//! 本文件由 `tools/gen_gbk_table.py` 生成(Python 标准库 gbk 编解码器),\n")
        fh.write("//! 勿手改;重生成命令见该脚本 docstring。索引 = (lead-0x81)*190 +\n")
        fh.write("//! trail 偏移(0x40..=0x7E → trail-0x40;0x80..=0xFE → trail-0x41);\n")
        fh.write("//! 未定义码位为 0xFFFD。\n\n")
        fh.write("// 23940 项 u16(~47 KB)const 数组:查表数据本就是常量语义,允许该 lint。\n")
        fh.write("#[allow(clippy::large_const_arrays)]\n")
        fh.write("#[rustfmt::skip]\n")
        fh.write("pub const GBK: [u16; 23940] = [\n")
        for i in range(0, len(chars), 16):
            row = ", ".join(f"0x{ord(c):04X}" for c in chars[i:i + 16])
            fh.write("    " + row + ",\n")
        fh.write("];\n")
    print(f"written: {out}")


if __name__ == "__main__":
    main()
