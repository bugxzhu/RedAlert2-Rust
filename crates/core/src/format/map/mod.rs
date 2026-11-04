// SPDX-License-Identifier: GPL-3.0-only

//! 地图/地形解析层(M2)。
//!
//! 模块族:`pack`(四包段容器)、`iso`(IsoMapPack5 瓦片网格)、`overlay`
//! (双 512×512 网格)、`preview`(裸 RGB)、`objects`(对象段)、`meta`
//! (元数据段)、`mapdoc`(解码策略)、`gbk`/`gbk_table`(GBK 有损解码)。
//! 总入口 `MapFile::parse` 在后续任务落地。

pub mod gbk;
pub mod gbk_table;
pub mod mapdoc;
pub mod pack;
