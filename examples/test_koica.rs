fn main() {
    let home = std::env::var("HOME").unwrap();
    let hwpx = std::fs::read(format!("{}/Downloads/코이카서식.hwpx", home)).unwrap();
    let files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();
    let xml = files.get("Contents/section0.xml").unwrap();

    // 분석
    let result = hwpx_filler::filler::analyze(xml);
    println!("Fields: {}", result.fields.len());
    for f in &result.fields {
        println!("  {} → {}", f.label, f.canonical_key);
    }

    // 빈 양식에 데이터 채움
    let patches: Vec<(usize, u32, u32, String)> = vec![
        (0, 0, 1, "김보람".into()),         // 성명
        (0, 0, 5, "MYSC".into()),           // 소속
        (0, 0, 8, "AXR팀장".into()),        // 직책
        (0, 1, 1, "한국외대 경영학".into()), // 학력
        (0, 1, 8, "5년".into()),            // 근무기간
        (0, 2, 8, "정보처리기사".into()),    // 자격증
        (0, 3, 2, "AI 자동화 리드".into()),  // 담당업무
        (0, 3, 8, "26.01~26.12 (50%)".into()), // 참여기간
    ];

    let patched = hwpx_filler::filler::fill(xml, &patches).unwrap();

    // 확인: 값이 들어갔는지
    let tables = hwpx_filler::stream_analyzer::analyze_xml(&patched);
    println!("\nAfter fill:");
    for cell in &tables[0].rows[0].cells {
        if !cell.text.is_empty() {
            println!("  [{},{}] = '{}'", cell.row, cell.col, cell.text);
        }
    }

    // 저장
    let mut modified = std::collections::HashMap::new();
    modified.insert("Contents/section0.xml".to_string(), patched);
    let output = hwpx_filler::zipper::patch_hwpx(&hwpx, &modified).unwrap();
    let path = format!("{}/Desktop/코이카_채움테스트.hwpx", home);
    std::fs::write(&path, &output).unwrap();
    println!("\n✓ Saved: {}", path);
}
