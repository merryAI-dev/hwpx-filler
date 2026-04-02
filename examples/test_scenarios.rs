//! 다양한 시나리오 테스트
//!
//! 시나리오 1: 행 클론 + 각 행에 다른 데이터 채움 (실제 사용 케이스)
//! 시나리오 2: 서식6에서 분석 + 채움
//! 시나리오 3: 경력 10개인 사람 — 행 대량 클론
//! 시나리오 4: 분석 → 채움 → 검증 → 한컴오피스 열기 full pipeline

fn main() {
    let home = std::env::var("HOME").unwrap();

    println!("========================================");
    println!("  시나리오 1: 행 클론 + 각 행에 다른 데이터");
    println!("========================================\n");
    scenario_1_clone_and_fill(&home);

    println!("\n========================================");
    println!("  시나리오 2: 서식6 분석 + 채움");
    println!("========================================\n");
    scenario_2_form6(&home);

    println!("\n========================================");
    println!("  시나리오 3: 경력 10개 — 대량 클론");
    println!("========================================\n");
    scenario_3_mass_clone(&home);

    println!("\n========================================");
    println!("  시나리오 4: full pipeline (서식5)");
    println!("========================================\n");
    scenario_4_full_pipeline(&home);
}

/// 시나리오 1: 경력 테이블의 빈 행(row 2~5)을 활용해서 4개 회사 데이터 채움
fn scenario_1_clone_and_fill(home: &str) {
    let hwpx = std::fs::read(format!(
        "{}/Downloads/[서식5] 참여인력 이력사항_변민욱 (1).hwpx", home
    )).unwrap();

    let files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();
    let xml = files.get("Contents/section0.xml").unwrap();

    // Table 2의 기존 데이터 행(row 2~5)에 새 데이터 채움
    // 클론 없이 기존 행만 활용
    let career_data = vec![
        (2, 0, "Google Korea"),    (2, 1, "2023.01 ~ 현재"), (2, 2, "Senior Engineer"), (2, 3, "AI Platform"),
        (2, 1, "Naver"),           (3, 1, "2021.03 ~ 2022.12"), (3, 2, "팀장"), (3, 3, "검색 엔진 개발"),
        (2, 0, "Google Korea"),    // 이미 위에서 설정
    ];

    // 더 깔끔하게: 행별로 채움
    let patches: Vec<(usize, u32, u32, String)> = vec![
        // Table 2, Row 2 (첫 번째 경력)
        (2, 2, 0, "Google Korea".into()),
        (2, 2, 1, "2023.01 ~ 현재".into()),
        (2, 2, 2, "Senior Engineer".into()),
        (2, 2, 3, "AI Platform 개발".into()),
        // Table 2, Row 3 (두 번째 경력)
        (2, 3, 0, "Naver".into()),
        (2, 3, 1, "2021.03 ~ 2022.12".into()),
        (2, 3, 2, "팀장".into()),
        (2, 3, 3, "검색 엔진 개발".into()),
        // Table 2, Row 4 (세 번째 경력)
        (2, 4, 0, "카카오".into()),
        (2, 4, 1, "2019.06 ~ 2021.02".into()),
        (2, 4, 2, "개발자".into()),
        (2, 4, 3, "카카오톡 백엔드".into()),
        // Table 2, Row 5 (네 번째 경력)
        (2, 5, 0, "스타트업X".into()),
        (2, 5, 1, "2018.01 ~ 2019.05".into()),
        (2, 5, 2, "인턴".into()),
        (2, 5, 3, "웹 개발".into()),
    ];

    let patched = hwpx_filler::patcher::patch_cells(xml, &patches).unwrap();

    // 검증
    let tables = hwpx_filler::stream_analyzer::analyze_xml(&patched);
    let row2_text: String = tables[2].rows[2].cells.iter()
        .map(|c| c.text.clone()).collect::<Vec<_>>().join(" | ");
    println!("  Row 2: {}", row2_text);
    let row3_text: String = tables[2].rows[3].cells.iter()
        .map(|c| c.text.clone()).collect::<Vec<_>>().join(" | ");
    println!("  Row 3: {}", row3_text);

    let validation = hwpx_filler::validate::validate_stream(&tables);
    println!("  Valid: {}", validation.valid);

    // 저장
    let mut modified = std::collections::HashMap::new();
    modified.insert("Contents/section0.xml".to_string(), patched);
    let output = hwpx_filler::zipper::patch_hwpx(&hwpx, &modified).unwrap();
    let path = "/tmp/scenario1_career_fill.hwpx";
    std::fs::write(path, &output).unwrap();
    println!("  ✓ Saved: {} ({} bytes)", path, output.len());
}

/// 시나리오 2: 서식6 — 다른 레이아웃의 양식
fn scenario_2_form6(home: &str) {
    let hwpx = std::fs::read(format!(
        "{}/Downloads/(서식 6) 참여인력 이력사항-변민욱.hwpx", home
    )).unwrap();

    let files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();
    let xml = files.get("Contents/section0.xml").unwrap();

    // 분석
    let result = hwpx_filler::filler::analyze(xml);
    println!("  Tables: {}, Fields: {}", result.tables.len(), result.fields.len());
    for f in &result.fields {
        println!("    {} → {} [{:?}]", f.label, f.canonical_key, f.content_type);
    }

    // 채움
    let patches: Vec<(usize, u32, u32, String)> = vec![
        (0, 0, 1, "이보람".into()),         // 성명
        (0, 0, 4, "MYSC".into()),           // 소속
        (0, 0, 6, "AXR팀장".into()),        // 직책
        (0, 1, 7, "5년 0월".into()),         // 경력
        (0, 5, 1, "AI 자동화 리드".into()),  // 참여임무
        (0, 5, 5, "26.01～26.12".into()),    // 참여기간
        (0, 5, 8, "50%".into()),             // 참여율
    ];

    let patched = hwpx_filler::patcher::patch_cells(xml, &patches).unwrap();
    let validation = hwpx_filler::filler::validate_patched(&patched);
    println!("  Valid: {}", validation.valid);

    let mut modified = std::collections::HashMap::new();
    modified.insert("Contents/section0.xml".to_string(), patched);
    let output = hwpx_filler::zipper::patch_hwpx(&hwpx, &modified).unwrap();
    let path = "/tmp/scenario2_form6.hwpx";
    std::fs::write(path, &output).unwrap();
    println!("  ✓ Saved: {} ({} bytes)", path, output.len());
}

/// 시나리오 3: 경력이 10개인 사람 — 기존 4행에서 6행 추가 클론
fn scenario_3_mass_clone(home: &str) {
    let hwpx = std::fs::read(format!(
        "{}/Downloads/[서식5] 참여인력 이력사항_변민욱 (1).hwpx", home
    )).unwrap();

    let files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();
    let xml = files.get("Contents/section0.xml").unwrap();

    let tables_before = hwpx_filler::stream_analyzer::analyze_xml(xml);
    println!("  Before: Table 2 has {} rows", tables_before[2].rows.len());

    // Row 2를 9번 클론 → 원본 1 + 클론 9 = 10행 (나머지 기존 행은 시프트)
    let cloned = hwpx_filler::patcher::patch_clone_rows(xml, 2, 2, 9).unwrap();

    let tables_after = hwpx_filler::stream_analyzer::analyze_xml(&cloned);
    println!("  After: Table 2 has {} rows", tables_after[2].rows.len());

    // 10개 경력 데이터 채움
    let companies = [
        "Google", "Apple", "Microsoft", "Amazon", "Meta",
        "Netflix", "Tesla", "Nvidia", "Samsung", "MYSC",
    ];

    let mut patches = Vec::new();
    for (i, company) in companies.iter().enumerate() {
        let row = 2 + i as u32;
        patches.push((2, row, 0, company.to_string()));
        patches.push((2, row, 1, format!("20{}.01 ~ 20{}.12", 15 + i, 16 + i)));
        patches.push((2, row, 2, "Engineer".to_string()));
        patches.push((2, row, 3, format!("{} 프로젝트", company)));
    }

    let filled = hwpx_filler::patcher::patch_cells(&cloned, &patches).unwrap();

    let validation = hwpx_filler::filler::validate_patched(&filled);
    println!("  Valid: {} (errors: {})", validation.valid, validation.errors.len());

    let mut modified = std::collections::HashMap::new();
    modified.insert("Contents/section0.xml".to_string(), filled);
    let output = hwpx_filler::zipper::patch_hwpx(&hwpx, &modified).unwrap();
    let path = "/tmp/scenario3_10careers.hwpx";
    std::fs::write(path, &output).unwrap();
    println!("  ✓ Saved: {} ({} bytes)", path, output.len());
}

/// 시나리오 4: 통합 API로 분석 → 자동 채움 → 검증 → 저장
fn scenario_4_full_pipeline(home: &str) {
    let hwpx = std::fs::read(format!(
        "{}/Downloads/[서식5] 참여인력 이력사항_변민욱 (1).hwpx", home
    )).unwrap();

    let files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();
    let xml = files.get("Contents/section0.xml").unwrap();

    // 1. 분석 (filler 통합 API)
    let analysis = hwpx_filler::filler::analyze(xml);
    println!("  Fields: {}", analysis.fields.len());

    // 2. 자동 매핑: field의 canonical_key로 데이터 매칭
    let data: std::collections::HashMap<&str, &str> = [
        ("name", "박지성"),
        ("email", "jspark@example.com"),
        ("position", "감독"),
        ("birth_date", "1981.02.25"),
        ("phone", "010-0000-0000"),
        ("experience", "20년 0개월"),
        ("certification", "AFC Pro License"),
        ("task", "축구단 운영"),
        ("period", "26.01～26.12"),
        ("participation_rate", "100%"),
    ].into();

    let patches: Vec<(usize, u32, u32, String)> = analysis.fields.iter()
        .filter_map(|f| {
            data.get(f.canonical_key.as_str())
                .map(|val| (f.table_index, f.row, f.col, val.to_string()))
        })
        .collect();

    println!("  Matched: {}/{} fields", patches.len(), analysis.fields.len());

    // 3. 채움
    let patched = hwpx_filler::filler::fill(xml, &patches).unwrap();

    // 4. 검증
    let validation = hwpx_filler::filler::validate_patched(&patched);
    println!("  Valid: {}", validation.valid);

    // 5. 저장
    let mut modified = std::collections::HashMap::new();
    modified.insert("Contents/section0.xml".to_string(), patched);
    let output = hwpx_filler::zipper::patch_hwpx(&hwpx, &modified).unwrap();
    let path = "/tmp/scenario4_full_pipeline.hwpx";
    std::fs::write(path, &output).unwrap();
    println!("  ✓ Saved: {} ({} bytes)", path, output.len());
}
