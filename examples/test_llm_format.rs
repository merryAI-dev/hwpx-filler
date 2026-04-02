fn main() {
    let home = std::env::var("HOME").unwrap();
    
    // 참여인력 이력사항 (해민영 — 문제 케이스)
    let hwpx = std::fs::read(format!("{}/Downloads/참여인력 이력사항.hwpx", home)).unwrap();
    let files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();
    let xml = files.get("Contents/section0.xml").unwrap();
    let tables = hwpx_filler::stream_analyzer::analyze_xml(xml);
    
    println!("=== 소스: 참여인력 이력사항 (해민영) ===\n");
    // Table 0만 (첫 번째 사람)
    println!("{}", hwpx_filler::llm_format::format_table_for_llm(&tables[0]));
    
    // 코이카 서식
    let hwpx2 = std::fs::read(format!("{}/Downloads/코이카서식.hwpx", home)).unwrap();
    let files2 = hwpx_filler::zipper::extract_text_files(&hwpx2).unwrap();
    let xml2 = files2.get("Contents/section0.xml").unwrap();
    let tables2 = hwpx_filler::stream_analyzer::analyze_xml(xml2);
    
    println!("=== 대상: 코이카 서식 ===\n");
    println!("{}", hwpx_filler::llm_format::format_tables_for_llm(&tables2));
}
