fn main() {
    let home = std::env::var("HOME").unwrap();
    let hwpx = std::fs::read(format!(
        "{}/Downloads/[서식5] 참여인력 이력사항_변민욱 (1).hwpx", home
    )).unwrap();
    let files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();
    let xml = files.get("Contents/section0.xml").unwrap();

    // Table 0에 패치 (table_index=0)
    let p0 = hwpx_filler::patcher::patch_cell_text(xml, 0, 0, 0, "TABLE0_TEST").unwrap();
    let t0 = hwpx_filler::stream_analyzer::analyze_xml(&p0);
    println!("Table 0, Row 0, Col 0 = '{}'", t0[0].rows[0].cells[0].text);

    // Table 1에 패치 (table_index=1)
    let p1 = hwpx_filler::patcher::patch_cell_text(xml, 1, 0, 1, "TABLE1_TEST").unwrap();
    let t1 = hwpx_filler::stream_analyzer::analyze_xml(&p1);
    println!("Table 1, Row 0, Col 1 = '{}'", t1[1].rows[0].cells[1].text);

    // Table 2에 패치 (table_index=2) — 이게 안 되는 것
    let p2 = hwpx_filler::patcher::patch_cell_text(xml, 2, 2, 0, "TABLE2_TEST").unwrap();
    let t2 = hwpx_filler::stream_analyzer::analyze_xml(&p2);
    println!("Table 2, Row 2, Col 0 = '{}'", t2[2].rows[2].cells[0].text);

    // Table 2에 패치 (table_index=2, row 0) — 헤더 행
    let p3 = hwpx_filler::patcher::patch_cell_text(xml, 2, 0, 0, "TABLE2_HDR").unwrap();
    let t3 = hwpx_filler::stream_analyzer::analyze_xml(&p3);
    println!("Table 2, Row 0, Col 0 = '{}'", t3[2].rows[0].cells[0].text);
}
