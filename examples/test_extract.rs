//! 서식5 → 서식6: 데이터 추출 + 자동 매핑 + 채움

fn main() {
    let home = std::env::var("HOME").unwrap();

    // 1. 서식5에서 데이터 추출
    println!("=== 서식5에서 데이터 추출 ===\n");
    let src_hwpx = std::fs::read(format!(
        "{}/Downloads/[서식5] 참여인력 이력사항_변민욱 (1).hwpx", home
    )).unwrap();
    let src_files = hwpx_filler::zipper::extract_text_files(&src_hwpx).unwrap();
    let src_xml = src_files.get("Contents/section0.xml").unwrap();

    let extracted = hwpx_filler::extractor::extract_data(src_xml);
    println!("  추출된 필드: {}개", extracted.len());
    for f in &extracted {
        println!("    [{}] {} = '{}'", f.key, f.raw_label,
            if f.value.len() > 30 { format!("{}...", &f.value[..30]) } else { f.value.clone() });
    }

    // 2. 서식6 양식 분석
    println!("\n=== 서식6 양식 분석 ===\n");
    let dst_hwpx = std::fs::read(format!(
        "{}/Downloads/(서식 6) 참여인력 이력사항-변민욱.hwpx", home
    )).unwrap();
    let dst_files = hwpx_filler::zipper::extract_text_files(&dst_hwpx).unwrap();
    let dst_xml = dst_files.get("Contents/section0.xml").unwrap();

    let analysis = hwpx_filler::filler::analyze(dst_xml);
    println!("  서식6 필드: {}개", analysis.fields.len());
    for f in &analysis.fields {
        println!("    {} → {}", f.label, f.canonical_key);
    }

    // 3. 추출 데이터 → 서식6 자동 매핑
    println!("\n=== 자동 매핑 ===\n");
    let patches = hwpx_filler::extractor::map_extracted_to_form(&extracted, &analysis.fields);
    println!("  매핑된 필드: {}/{}", patches.len(), analysis.fields.len());
    for (ti, row, col, val) in &patches {
        let label = analysis.fields.iter()
            .find(|f| f.table_index == *ti && f.row == *row && f.col == *col)
            .map(|f| f.label.as_str())
            .unwrap_or("?");
        println!("    {} → '{}'", label, if val.len() > 30 { format!("{}...", &val[..30]) } else { val.clone() });
    }

    // 4. 서식6에 채움
    println!("\n=== 서식6에 채움 ===\n");
    let patched = hwpx_filler::filler::fill(dst_xml, &patches).unwrap();
    let validation = hwpx_filler::filler::validate_patched(&patched);
    println!("  Valid: {}", validation.valid);

    let mut modified = std::collections::HashMap::new();
    modified.insert("Contents/section0.xml".to_string(), patched);
    let output = hwpx_filler::zipper::patch_hwpx(&dst_hwpx, &modified).unwrap();
    let path = format!("{}/Desktop/서식5to6_자동변환.hwpx", home);
    std::fs::write(&path, &output).unwrap();
    println!("  ✓ Saved: {} ({} bytes)", path, output.len());
}
