fn main() {
    let home = std::env::var("HOME").unwrap();

    // Step 1: 서식5에서 데이터 추출
    let src = std::fs::read(format!("{}/Downloads/[서식5] 참여인력 이력사항_변민욱 (1).hwpx", home)).unwrap();
    let src_files = hwpx_filler::zipper::extract_text_files(&src).unwrap();
    let src_xml = src_files.get("Contents/section0.xml").unwrap();
    let extracted = hwpx_filler::extractor::extract_data(src_xml);

    println!("=== Step 1: 추출된 데이터 ({}) ===", extracted.len());
    for f in &extracted {
        let val: String = f.value.chars().take(25).collect();
        println!("  [{}] {} = '{}'", f.key, f.raw_label, val);
    }

    // Step 2: 코이카 서식 분석
    let dst = std::fs::read(format!("{}/Downloads/코이카서식.hwpx", home)).unwrap();
    let dst_files = hwpx_filler::zipper::extract_text_files(&dst).unwrap();
    let dst_xml = dst_files.get("Contents/section0.xml").unwrap();
    let analysis = hwpx_filler::filler::analyze(dst_xml);

    println!("\n=== Step 2: 코이카 필드 ({}) ===", analysis.fields.len());
    for f in &analysis.fields {
        println!("  [{}] {} (T{}:R{}:C{})", f.canonical_key, f.label, f.table_index, f.row, f.col);
    }

    // Step 3: 자동 매핑
    let mapping = hwpx_filler::extractor::map_extracted_to_form_detailed(&extracted, &analysis.fields);

    println!("\n=== Step 3: 매핑 결과 ===");
    let mut matched = 0;
    let mut unmatched = 0;
    for m in &mapping.mappings {
        let status = match m.match_type.as_str() {
            "canonical" => { matched += 1; "✅ canonical" },
            "normalized" => { matched += 1; "✅ normalized" },
            "fuzzy" => { matched += 1; "🟡 fuzzy" },
            _ => { unmatched += 1; "❌ unmatched" },
        };
        let val: String = m.source_value.chars().take(15).collect();
        println!("  {} {} ← {} = '{}'", status, m.target_label, m.source_key, val);
    }
    println!("\n  매핑: {}/{}, 미매핑: {}", matched, mapping.mappings.len(), unmatched);

    // Step 4: 채움 + 저장
    let patched = hwpx_filler::filler::fill(dst_xml, &mapping.patches.iter()
        .map(|p| (p.table_index, p.row, p.col, p.value.clone()))
        .collect::<Vec<_>>()).unwrap();

    let mut modified = std::collections::HashMap::new();
    modified.insert("Contents/section0.xml".to_string(), patched);
    let output = hwpx_filler::zipper::patch_hwpx(&dst, &modified).unwrap();
    let path = format!("{}/Desktop/wizard_서식5to코이카.hwpx", home);
    std::fs::write(&path, &output).unwrap();
    println!("\n✓ {}", path);
}
