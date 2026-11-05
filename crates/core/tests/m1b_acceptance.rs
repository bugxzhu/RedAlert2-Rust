// SPDX-License-Identifier: GPL-3.0-only
//! M1b 全链路验收(真实资产):摘要位 MIX 链 + 六类解析器全部吃真字节。
//! 运行:RA2_ASSETS_DIR="E:\hongjing\Reunion 2023" cargo test -p ra2-core --test m1b_acceptance -- --ignored

use ra2_core::format::aud::Aud;
use ra2_core::format::csf::CsfDocument;
use ra2_core::format::hva::HvaFile;
use ra2_core::format::mix::MixArchive;
use ra2_core::format::pal::Palette;
use ra2_core::format::shp::ShpFile;
use ra2_core::format::shp_ts::ShpTsFile;
use ra2_core::format::vxl::VxlFile;
use ra2_core::format::FormatError;

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

#[test]
#[ignore]
fn m1b_full_pipeline_over_real_assets() {
    // 1) 摘要位链:ra2md.mix → isourbmd.mix(0x00010000)→ 按 ID 读取首条目
    let ra2md = load("ra2md.mix");
    let iso =
        MixArchive::parse(ra2md.read("isourbmd.mix").unwrap().to_vec()).expect("摘要位应被接受");
    assert_eq!(iso.entries().len(), 57);
    assert!(!iso.read_by_id(iso.entries()[0].id).unwrap().is_empty());
    // 加密位真库仍拒
    assert!(matches!(
        MixArchive::parse(ra2md.read("sidec02md.mix").unwrap().to_vec()),
        Err(FormatError::Unsupported(_))
    ));

    // 2) SHP 全链:ra2md.mix → conqmd.mix → gunfirey.shp(按名,真库直读)
    let conq = MixArchive::parse(ra2md.read("conqmd.mix").unwrap().to_vec()).unwrap();
    let shp = ShpFile::parse(conq.read("gunfirey.shp").unwrap()).unwrap();
    assert_eq!(shp.frames.len(), 8);
    assert!(shp.frames[0].pixels.iter().all(|&p| (p as usize) < 256)); // 调色板索引域

    // 3) SHP(剧场装饰精灵,M1b 误判已纠正,研究 §4):ra2.mix → snow.mix → burnt01.sno
    //    (按名;该库 LMD 名单裁决)
    let ra2 = load("ra2.mix");
    let snow = MixArchive::parse(ra2.read("snow.mix").unwrap().to_vec()).unwrap();
    let tmp = ShpTsFile::parse(snow.read("burnt01.sno").unwrap()).unwrap();
    assert_eq!(tmp.frames.len(), 2);
    assert_eq!(tmp.frames[0].pixels.len(), 29 * 17);

    // 4) CSF:langmd.mix → ra2md.csf
    let langmd = load("langmd.mix");
    let csf = CsfDocument::parse(langmd.read("ra2md.csf").unwrap()).unwrap();
    assert_eq!(csf.labels.len(), 5280);
    assert_eq!(csf.labels[0].name, "THEME:Intro");

    // 5) VXL + HVA + AUD:ra2.mix → local.mix
    let local = MixArchive::parse(ra2.read("local.mix").unwrap().to_vec()).unwrap();
    let vxl = VxlFile::parse(local.read("1tnk.vxl").unwrap()).unwrap();
    assert_eq!(vxl.sections[0].columns.len(), 465);
    let hva = HvaFile::parse(local.read("1tnk.hva").unwrap()).unwrap();
    assert_eq!(hva.section_names, ["B-BODY"]);
    let aud = Aud::parse(local.read("text1.aud").unwrap()).unwrap();
    assert_eq!(aud.samples.len(), 1586);

    // 6) 调色板:ra2md.mix → localmd.mix → mpyscrn.pal(768 字节,VXL/SHP 的邻近断言:
    //    SHP/TS SHP/VXL 产出的都是 0..=255 调色板索引,渲染上色属 view 层)
    let localmd = MixArchive::parse(ra2md.read("localmd.mix").unwrap().to_vec()).unwrap();
    let pal = Palette::parse(localmd.read("mpyscrn.pal").unwrap()).unwrap();
    assert_eq!(pal.rgba(0).len(), 4);
}
