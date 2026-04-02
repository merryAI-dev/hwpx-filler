fn main() {
    let home = std::env::var("HOME").unwrap();
    let hwpx_bytes = std::fs::read(
        format!("{}/Downloads/[서식5] 참여인력 이력사항_변민욱 (1).hwpx", home)
    ).unwrap();
    
    let text_files = hwpx_filler::zipper::extract_text_files(&hwpx_bytes).unwrap();
    let section0 = text_files.get("Contents/section0.xml").unwrap();
    
    let tables_before = hwpx_filler::stream_analyzer::analyze_xml(section0);
    println!("BEFORE: Table 2 has {} rows", tables_before[2].rows.len());
    
    // Table 2의 row 2를 3번 클론
    let cloned = hwpx_filler::patcher::patch_clone_rows(section0, 2, 2, 3).unwrap();
    
    let tables_after = hwpx_filler::stream_analyzer::analyze_xml(&cloned);
    println!("AFTER:  Table 2 has {} rows", tables_after[2].rows.len());
    
    let addrs: Vec<u32> = tables_after[2].rows.iter()
        .map(|r| r.cells.first().map(|c| c.row).unwrap_or(999))
        .collect();
    println!("rowAddrs: {:?}", addrs);
    
    let validation = hwpx_filler::validate::validate_stream(&tables_after);
    println!("Valid: {}", validation.valid);
    for e in &validation.errors {
        println!("  ERR: {}", e);
    }
    
    let mut modified = std::collections::HashMap::new();
    modified.insert("Contents/section0.xml".to_string(), cloned);
    let output = hwpx_filler::zipper::patch_hwpx(&hwpx_bytes, &modified).unwrap();
    std::fs::write("/tmp/hwpx-clone-test.hwpx", &output).unwrap();
    println!("\nSaved to /tmp/hwpx-clone-test.hwpx ({} bytes)", output.len());
}
