fn main() {
    let home = std::env::var("HOME").unwrap();
    let hwpx = std::fs::read(format!(
        "{}/Downloads/[서식5] 참여인력 이력사항_변민욱 (1).hwpx", home
    )).unwrap();
    let files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();
    let xml = files.get("Contents/section0.xml").unwrap();

    // Table 2, Row 2, Col 0 → "TEST_GOOGLE"
    let patched = hwpx_filler::patcher::patch_cell_text(xml, 2, 2, 0, "TEST_GOOGLE").unwrap();

    // 확인
    let tables = hwpx_filler::stream_analyzer::analyze_xml(&patched);
    let cell = &tables[2].rows[2].cells[0];
    println!("Table 2, Row 2, Col 0 = '{}'", cell.text);
    println!("Expected: TEST_GOOGLE");
    println!("Match: {}", cell.text == "TEST_GOOGLE");
}
