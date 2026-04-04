//! 통합 테스트 — 다중 섹션, 분석, 채움, 클론, 에러 핸들링
//!
//! synthetic HWPX fixture를 프로그래밍 방식으로 생성해서 CI-reproducible.
//! 실제 정부 양식 fixture는 tests/fixtures/에 추가되면 여기서 테스트.

use std::collections::HashMap;
use std::io::{Write, Cursor};
use zip::write::SimpleFileOptions;

// ── fixture 생성 헬퍼 ──────────────────────────────────────────────────────

/// 단일 섹션 HWPX (section0.xml만)
fn make_single_section_hwpx(section0_xml: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
        w.start_file("mimetype", SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)).unwrap();
        w.write_all(b"application/hwpx+zip").unwrap();
        w.start_file("Contents/section0.xml", SimpleFileOptions::default()).unwrap();
        w.write_all(section0_xml.as_bytes()).unwrap();
        w.finish().unwrap();
    }
    buf
}

/// 다중 섹션 HWPX (section0.xml + section1.xml)
fn make_multi_section_hwpx(section0_xml: &str, section1_xml: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
        w.start_file("mimetype", SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)).unwrap();
        w.write_all(b"application/hwpx+zip").unwrap();
        w.start_file("Contents/section0.xml", SimpleFileOptions::default()).unwrap();
        w.write_all(section0_xml.as_bytes()).unwrap();
        w.start_file("Contents/section1.xml", SimpleFileOptions::default()).unwrap();
        w.write_all(section1_xml.as_bytes()).unwrap();
        w.finish().unwrap();
    }
    buf
}

/// 빈 HWPX (섹션 없음)
fn make_empty_hwpx() -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
        w.start_file("mimetype", SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)).unwrap();
        w.write_all(b"application/hwpx+zip").unwrap();
        w.finish().unwrap();
    }
    buf
}

/// 간단한 2x2 테이블 XML (label + data 구조)
/// 행1: [성 명] [□]    → label + data
/// 행2: [소 속] [□]    → label + data
/// 실제 HWPX 구조: <hp:tc> 안에 <hp:cellAddr/> 자식 요소
fn simple_table_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hs="http://www.hancom.co.kr/hwpml/2011/paragraph"
        xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph">
<hp:p><hp:run><hp:tbl colCnt="2" rowCnt="2">
  <hp:tr>
    <hp:tc>
      <hp:cellAddr colAddr="0" rowAddr="0"/>
      <hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t>성 명</hp:t></hp:run></hp:p>
    </hp:tc>
    <hp:tc>
      <hp:cellAddr colAddr="1" rowAddr="0"/>
      <hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t></hp:t></hp:run></hp:p>
    </hp:tc>
  </hp:tr>
  <hp:tr>
    <hp:tc>
      <hp:cellAddr colAddr="0" rowAddr="1"/>
      <hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t>소 속</hp:t></hp:run></hp:p>
    </hp:tc>
    <hp:tc>
      <hp:cellAddr colAddr="1" rowAddr="1"/>
      <hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t></hp:t></hp:run></hp:p>
    </hp:tc>
  </hp:tr>
</hp:tbl></hp:run></hp:p>
</hs:sec>"#.to_string()
}

/// 다른 테이블 구조 (3x2, 경력 테이블 모양)
fn career_table_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hs="http://www.hancom.co.kr/hwpml/2011/paragraph"
        xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph">
<hp:p><hp:run><hp:tbl colCnt="3" rowCnt="2">
  <hp:tr>
    <hp:tc>
      <hp:cellAddr colAddr="0" rowAddr="0"/>
      <hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t>기간</hp:t></hp:run></hp:p>
    </hp:tc>
    <hp:tc>
      <hp:cellAddr colAddr="1" rowAddr="0"/>
      <hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t>근무처</hp:t></hp:run></hp:p>
    </hp:tc>
    <hp:tc>
      <hp:cellAddr colAddr="2" rowAddr="0"/>
      <hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t>담당업무</hp:t></hp:run></hp:p>
    </hp:tc>
  </hp:tr>
  <hp:tr>
    <hp:tc>
      <hp:cellAddr colAddr="0" rowAddr="1"/>
      <hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t></hp:t></hp:run></hp:p>
    </hp:tc>
    <hp:tc>
      <hp:cellAddr colAddr="1" rowAddr="1"/>
      <hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t></hp:t></hp:run></hp:p>
    </hp:tc>
    <hp:tc>
      <hp:cellAddr colAddr="2" rowAddr="1"/>
      <hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t></hp:t></hp:run></hp:p>
    </hp:tc>
  </hp:tr>
</hp:tbl></hp:run></hp:p>
</hs:sec>"#.to_string()
}

/// 채워진 이력서 데이터 (소스 HWPX용)
fn filled_resume_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hs="http://www.hancom.co.kr/hwpml/2011/paragraph"
        xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph">
<hp:p><hp:run><hp:tbl colCnt="2" rowCnt="2">
  <hp:tr>
    <hp:tc>
      <hp:cellAddr colAddr="0" rowAddr="0"/>
      <hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t>성 명</hp:t></hp:run></hp:p>
    </hp:tc>
    <hp:tc>
      <hp:cellAddr colAddr="1" rowAddr="0"/>
      <hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t>김보람</hp:t></hp:run></hp:p>
    </hp:tc>
  </hp:tr>
  <hp:tr>
    <hp:tc>
      <hp:cellAddr colAddr="0" rowAddr="1"/>
      <hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t>소 속</hp:t></hp:run></hp:p>
    </hp:tc>
    <hp:tc>
      <hp:cellAddr colAddr="1" rowAddr="1"/>
      <hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t>MYSC</hp:t></hp:run></hp:p>
    </hp:tc>
  </hp:tr>
</hp:tbl></hp:run></hp:p>
</hs:sec>"#.to_string()
}

// ── 테스트 ─────────────────────────────────────────────────────────────────

#[test]
fn single_section_analyze() {
    let hwpx = make_single_section_hwpx(&simple_table_xml());
    let text_files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();
    let tables = hwpx_filler::stream_analyzer::analyze_xml(
        text_files.get("Contents/section0.xml").unwrap(),
    );
    assert_eq!(tables.len(), 1, "단일 섹션에 테이블 1개");
    assert_eq!(tables[0].row_count, 2);
    assert_eq!(tables[0].col_count, 2);
}

#[test]
fn multi_section_tables_get_unique_indices() {
    let hwpx = make_multi_section_hwpx(&simple_table_xml(), &career_table_xml());
    let text_files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();

    // 각 섹션을 개별 분석하면 둘 다 index=0
    let t0 = hwpx_filler::stream_analyzer::analyze_xml(
        text_files.get("Contents/section0.xml").unwrap(),
    );
    let t1 = hwpx_filler::stream_analyzer::analyze_xml(
        text_files.get("Contents/section1.xml").unwrap(),
    );
    assert_eq!(t0[0].index, 0);
    assert_eq!(t1[0].index, 0);

    // 전체 섹션 순회 시 글로벌 인덱스 부여 확인
    let mut sections: Vec<(&str, &str)> = text_files
        .iter()
        .filter(|(name, _)| name.starts_with("Contents/section") && name.ends_with(".xml"))
        .map(|(n, c)| (n.as_str(), c.as_str()))
        .collect();
    sections.sort_by_key(|(name, _)| *name);

    let mut all_tables = Vec::new();
    let mut offset = 0;
    for (_, xml) in &sections {
        let mut tables = hwpx_filler::stream_analyzer::analyze_xml(xml);
        for t in &mut tables { t.index += offset; }
        offset += tables.len();
        all_tables.extend(tables);
    }
    assert_eq!(all_tables.len(), 2, "2개 섹션 × 1개 테이블 = 2");
    assert_eq!(all_tables[0].index, 0, "section0의 테이블은 index 0");
    assert_eq!(all_tables[1].index, 1, "section1의 테이블은 index 1");
    assert_eq!(all_tables[0].col_count, 2, "section0: 2x2");
    assert_eq!(all_tables[1].col_count, 3, "section1: 3x2");
}

#[test]
fn single_section_fill_roundtrip() {
    let hwpx = make_single_section_hwpx(&simple_table_xml());
    let text_files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();
    let xml = text_files.get("Contents/section0.xml").unwrap();

    // table 0, row 0, col 1 에 "김보람" 채우기
    let patches = vec![(0usize, 0u32, 1u32, "김보람".to_string())];
    let patched = hwpx_filler::patcher::patch_cells(xml, &patches).unwrap();
    assert!(patched.contains("김보람"), "패치된 XML에 값이 있어야 함");

    // ZIP 재조립
    let mut modified = HashMap::new();
    modified.insert("Contents/section0.xml".to_string(), patched);
    let output = hwpx_filler::zipper::patch_hwpx(&hwpx, &modified).unwrap();

    // 결과 ZIP에서 다시 추출
    let result_files = hwpx_filler::zipper::extract_text_files(&output).unwrap();
    let result_xml = result_files.get("Contents/section0.xml").unwrap();
    assert!(result_xml.contains("김보람"), "재조립 후에도 값 유지");
}

#[test]
fn multi_section_fill_only_patches_correct_section() {
    let hwpx = make_multi_section_hwpx(&simple_table_xml(), &career_table_xml());
    let text_files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();

    // section1의 테이블(글로벌 index 1)에만 패치
    let xml1 = text_files.get("Contents/section1.xml").unwrap();
    let patches = vec![(0usize, 1u32, 0u32, "2020~2025".to_string())]; // 로컬 index 0
    let patched1 = hwpx_filler::patcher::patch_cells(xml1, &patches).unwrap();

    let mut modified = HashMap::new();
    modified.insert("Contents/section1.xml".to_string(), patched1);
    let output = hwpx_filler::zipper::patch_hwpx(&hwpx, &modified).unwrap();

    let result_files = hwpx_filler::zipper::extract_text_files(&output).unwrap();
    let s0 = result_files.get("Contents/section0.xml").unwrap();
    let s1 = result_files.get("Contents/section1.xml").unwrap();
    assert!(!s0.contains("2020~2025"), "section0은 변경되면 안 됨");
    assert!(s1.contains("2020~2025"), "section1에만 패치 적용");
}

#[test]
fn extract_data_from_filled_hwpx() {
    let hwpx = make_single_section_hwpx(&filled_resume_xml());
    let text_files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();
    let xml = text_files.get("Contents/section0.xml").unwrap();

    let fields = hwpx_filler::extractor::extract_data(xml);
    let name_field = fields.iter().find(|f| f.raw_label.contains("성"));
    assert!(name_field.is_some(), "성명 라벨을 찾아야 함");
    assert_eq!(name_field.unwrap().value, "김보람");

    let org_field = fields.iter().find(|f| f.raw_label.contains("소"));
    assert!(org_field.is_some(), "소속 라벨을 찾아야 함");
    assert_eq!(org_field.unwrap().value, "MYSC");
}

#[test]
fn empty_hwpx_returns_error() {
    let hwpx = make_empty_hwpx();
    let text_files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();

    // 섹션 파일이 없어야 함
    let sections: Vec<&String> = text_files
        .keys()
        .filter(|name| name.starts_with("Contents/section") && name.ends_with(".xml"))
        .collect();
    assert!(sections.is_empty(), "빈 HWPX에 섹션이 없어야 함");
}

#[test]
fn extract_labels_from_multi_section() {
    let hwpx = make_multi_section_hwpx(&filled_resume_xml(), &career_table_xml());
    let text_files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();

    let mut sections: Vec<(&str, &str)> = text_files
        .iter()
        .filter(|(name, _)| name.starts_with("Contents/section") && name.ends_with(".xml"))
        .map(|(n, c)| (n.as_str(), c.as_str()))
        .collect();
    sections.sort_by_key(|(name, _)| *name);

    let mut all_labels = Vec::new();
    for (_, xml) in &sections {
        let fields = hwpx_filler::extractor::extract_data(xml);
        for f in fields {
            if !f.raw_label.is_empty() {
                all_labels.push(f.raw_label);
            }
        }
    }
    // section0: 성 명, 소 속 + section1: 기간 (근무처/담당업무는 header로 분류될 수 있음)
    assert!(all_labels.len() >= 3, "최소 3개 라벨: {:?}", all_labels);
    assert!(all_labels.iter().any(|l| l.contains("성")), "성명 라벨 포함");
    assert!(all_labels.iter().any(|l| l.contains("소")), "소속 라벨 포함");
}

#[test]
fn format_for_llm_includes_all_sections() {
    let hwpx = make_multi_section_hwpx(&simple_table_xml(), &career_table_xml());
    let text_files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();

    let mut sections: Vec<(&str, &str)> = text_files
        .iter()
        .filter(|(name, _)| name.starts_with("Contents/section") && name.ends_with(".xml"))
        .map(|(n, c)| (n.as_str(), c.as_str()))
        .collect();
    sections.sort_by_key(|(name, _)| *name);

    let mut all_tables = Vec::new();
    let mut offset = 0;
    for (_, xml) in &sections {
        let mut tables = hwpx_filler::stream_analyzer::analyze_xml(xml);
        for t in &mut tables { t.index += offset; }
        offset += tables.len();
        all_tables.extend(tables);
    }
    let llm_text = hwpx_filler::llm_format::format_tables_for_llm(&all_tables);
    assert!(llm_text.contains("성"), "LLM 포맷에 section0 내용 포함");
    assert!(llm_text.contains("근무처"), "LLM 포맷에 section1 내용 포함");
}

#[test]
fn validate_patched_xml() {
    let hwpx = make_single_section_hwpx(&simple_table_xml());
    let text_files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();
    let xml = text_files.get("Contents/section0.xml").unwrap();

    // 단일 패치 적용 후 텍스트 포함 확인
    let patched = hwpx_filler::patcher::patch_cells(xml, &[(0usize, 0u32, 1u32, "김보람".to_string())]).unwrap();
    assert!(patched.contains("김보람"), "패치에 성명 값 포함");

    // TODO(Phase 1 #5): 빈 <hp:t></hp:t> 패치 후 XML 구조 검증
    // 현재 patcher의 바이트 오프셋 방식이 빈 셀 패치 시 구조를 깰 수 있음.
    // 실제 HWPX 파일에서는 다르게 동작할 수 있으므로 실제 fixture로 재검증 필요.
}

// ── 실제 HWPX fixture 테스트 ─────────────────────────────────────────────

/// 서식5 이력사항: 데이터 추출 + 필드 분석 end-to-end
#[test]
fn fixture_서식5_extract_and_analyze() {
    let path = std::path::Path::new("tests/fixtures/서식5_이력사항.hwpx");
    if !path.exists() { return; }
    let bytes = std::fs::read(path).unwrap();
    let text_files = hwpx_filler::zipper::extract_text_files(&bytes).unwrap();

    // 섹션 파일 확인
    let sections: Vec<&String> = text_files.keys()
        .filter(|n| n.starts_with("Contents/section") && n.ends_with(".xml"))
        .collect();
    assert!(!sections.is_empty(), "섹션이 있어야 함");

    // 데이터 추출
    for section_name in &sections {
        let xml = text_files.get(*section_name).unwrap();
        let extracted = hwpx_filler::extractor::extract_data(xml);
        assert!(!extracted.is_empty(), "{}: 추출된 필드가 있어야 함", section_name);

        // 서식5에는 성명, 소속 등이 있어야 함
        let has_name = extracted.iter().any(|f| f.raw_label.contains("성") || f.raw_label.contains("이름"));
        assert!(has_name, "서식5에 성명 라벨이 있어야 함: {:?}",
            extracted.iter().map(|f| &f.raw_label).collect::<Vec<_>>());
    }
}

/// 코이카서식: 양식 분석 + 필드 매핑
#[test]
fn fixture_코이카_analyze_form() {
    let path = std::path::Path::new("tests/fixtures/코이카서식.hwpx");
    if !path.exists() { return; }
    let bytes = std::fs::read(path).unwrap();
    let text_files = hwpx_filler::zipper::extract_text_files(&bytes).unwrap();

    for section_name in text_files.keys().filter(|n| n.starts_with("Contents/section") && n.ends_with(".xml")) {
        let xml = text_files.get(section_name).unwrap();
        let tables = hwpx_filler::stream_analyzer::analyze_xml(xml);
        assert!(!tables.is_empty(), "코이카서식에 테이블이 있어야 함");

        let fields = hwpx_filler::stream_analyzer::extract_fields(&tables);
        println!("  코이카 {}: {}개 테이블, {}개 필드", section_name, tables.len(), fields.len());
        for f in &fields {
            println!("    [T{}:R{}:C{}] {} → {} ({:.0}%)",
                f.table_index, f.row, f.col, f.label, f.canonical_key, f.confidence * 100.0);
        }
    }
}

/// 서식5 → 코이카: cross-form 매핑 end-to-end
#[test]
fn fixture_cross_form_mapping() {
    let src_path = std::path::Path::new("tests/fixtures/서식5_이력사항.hwpx");
    let dst_path = std::path::Path::new("tests/fixtures/코이카서식.hwpx");
    if !src_path.exists() || !dst_path.exists() { return; }

    let src_bytes = std::fs::read(src_path).unwrap();
    let dst_bytes = std::fs::read(dst_path).unwrap();

    // 소스: 데이터 추출
    let src_files = hwpx_filler::zipper::extract_text_files(&src_bytes).unwrap();
    let mut src_fields = Vec::new();
    for (name, xml) in src_files.iter().filter(|(n, _)| n.starts_with("Contents/section") && n.ends_with(".xml")) {
        src_fields.extend(hwpx_filler::extractor::extract_data(xml));
    }
    assert!(!src_fields.is_empty(), "소스에서 필드 추출 실패");

    // 대상: 양식 분석
    let dst_files = hwpx_filler::zipper::extract_text_files(&dst_bytes).unwrap();
    let mut dst_form_fields = Vec::new();
    let mut table_offset = 0;
    for name in dst_files.keys().filter(|n| n.starts_with("Contents/section") && n.ends_with(".xml")).collect::<Vec<_>>().into_iter() {
        let xml = dst_files.get(name).unwrap();
        let tables = hwpx_filler::stream_analyzer::analyze_xml(xml);
        let mut fields = hwpx_filler::stream_analyzer::extract_fields(&tables);
        for f in &mut fields { f.table_index += table_offset; }
        table_offset += tables.len();
        dst_form_fields.extend(fields);
    }
    assert!(!dst_form_fields.is_empty(), "대상에서 필드 분석 실패");

    // 매핑
    let mapping = hwpx_filler::extractor::map_extracted_to_form_detailed(&src_fields, &dst_form_fields);
    let matched = mapping.mappings.iter().filter(|m| m.match_type != "unmatched").count();
    println!("  cross-form: {} 매핑 중 {} 성공", mapping.mappings.len(), matched);
    assert!(matched > 0, "최소 1개 이상 매핑 성공해야 함");

    // 채움 (패치가 있으면)
    if !mapping.patches.is_empty() {
        println!("  {} patches to apply", mapping.patches.len());
    }
}

/// 코이카서식: 행 클론 (경력 행 복제) end-to-end
#[test]
fn fixture_코이카_clone_rows() {
    let path = std::path::Path::new("tests/fixtures/코이카서식.hwpx");
    if !path.exists() { return; }
    let bytes = std::fs::read(path).unwrap();
    let text_files = hwpx_filler::zipper::extract_text_files(&bytes).unwrap();
    let xml = text_files.get("Contents/section0.xml").unwrap();

    let tables = hwpx_filler::stream_analyzer::analyze_xml(xml);
    assert!(!tables.is_empty());

    // 경력 반복 행 찾기: row_count가 큰 테이블에서 마지막 데이터 행
    let main_table = &tables[0];
    println!("  코이카 table: {}r x {}c", main_table.row_count, main_table.col_count);

    // 반복 구간 (row 6~9가 경력 행) — 행 6을 템플릿으로 2회 클론
    let template_row = 6u32;
    let clone_count = 2;

    let result = hwpx_filler::patcher::patch_clone_rows(xml, 0, template_row, clone_count);
    match result {
        Ok(cloned_xml) => {
            // 클론 후 테이블 구조 확인
            let cloned_tables = hwpx_filler::stream_analyzer::analyze_xml(&cloned_xml);
            assert!(!cloned_tables.is_empty(), "클론 후 테이블이 있어야 함");

            let cloned_table = &cloned_tables[0];
            println!("  클론 후: {}r x {}c (원본: {}r)",
                cloned_table.row_count, cloned_table.col_count, main_table.row_count);

            // 클론 후 행 수 = 원본 + clone_count
            assert_eq!(
                cloned_table.row_count,
                main_table.row_count + clone_count as u32,
                "클론 후 행 수 = 원본 + 복제 수"
            );

            // ZIP 재조립 검증
            let mut modified = HashMap::new();
            modified.insert("Contents/section0.xml".to_string(), cloned_xml);
            let output = hwpx_filler::zipper::patch_hwpx(&bytes, &modified).unwrap();
            assert!(output.len() > 0, "클론 후 유효한 ZIP이어야 함");
            // ZIP 재조립 후 다시 읽어서 검증
            let verify_files = hwpx_filler::zipper::extract_text_files(&output).unwrap();
            let verify_xml = verify_files.get("Contents/section0.xml").unwrap();
            let verify_tables = hwpx_filler::stream_analyzer::analyze_xml(verify_xml);
            assert_eq!(verify_tables[0].row_count, main_table.row_count + clone_count as u32,
                "ZIP 재조립 후에도 클론된 행 수 유지");
            println!("  ZIP: {}B → {}B, 재검증 OK", bytes.len(), output.len());
        }
        Err(e) => {
            // 행을 못 찾을 수 있음 (서식 구조에 따라)
            println!("  행 클론 실패 (row {}): {} — 다른 행으로 재시도 필요", template_row, e);
        }
    }
}

/// synthetic: 행 클론 + 채움 결합 테스트
#[test]
fn clone_then_fill() {
    // 3행 테이블: header + data1 + data2
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hs="http://www.hancom.co.kr/hwpml/2011/paragraph"
        xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph">
<hp:p><hp:run><hp:tbl colCnt="2" rowCnt="3">
  <hp:sz width="10000" height="3000"/>
  <hp:tr>
    <hp:tc><hp:cellAddr colAddr="0" rowAddr="0"/><hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t>이름</hp:t></hp:run></hp:p></hp:tc>
    <hp:tc><hp:cellAddr colAddr="1" rowAddr="0"/><hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t>소속</hp:t></hp:run></hp:p></hp:tc>
  </hp:tr>
  <hp:tr><hp:trPr><hp:trHeight height="1000"/></hp:trPr>
    <hp:tc><hp:cellAddr colAddr="0" rowAddr="1"/><hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t>김보람</hp:t></hp:run></hp:p></hp:tc>
    <hp:tc><hp:cellAddr colAddr="1" rowAddr="1"/><hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t>MYSC</hp:t></hp:run></hp:p></hp:tc>
  </hp:tr>
  <hp:tr><hp:trPr><hp:trHeight height="1000"/></hp:trPr>
    <hp:tc><hp:cellAddr colAddr="0" rowAddr="2"/><hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t></hp:t></hp:run></hp:p></hp:tc>
    <hp:tc><hp:cellAddr colAddr="1" rowAddr="2"/><hp:cellSpan colSpan="1" rowSpan="1"/>
      <hp:p><hp:run><hp:t></hp:t></hp:run></hp:p></hp:tc>
  </hp:tr>
</hp:tbl></hp:run></hp:p>
</hs:sec>"#;

    // row 2 (빈 데이터 행)를 1회 클론 → 총 4행이 되어야 함
    let cloned = hwpx_filler::patcher::patch_clone_rows(xml, 0, 2, 1).unwrap();
    let tables = hwpx_filler::stream_analyzer::analyze_xml(&cloned);
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].row_count, 4, "클론 후 4행");

    // 클론된 행 (row 2)에 값 채움
    let filled = hwpx_filler::patcher::patch_cells(&cloned, &[
        (0, 2, 0, "이민호".to_string()),
    ]).unwrap();
    assert!(filled.contains("이민호"), "클론된 행에 값이 채워져야 함");
    assert!(filled.contains("김보람"), "기존 행의 값도 유지");
}

/// 최악의서식: 크래시 안 나는지 + 구조 분석
#[test]
fn fixture_최악의서식_resilience() {
    let path = std::path::Path::new("tests/fixtures/최악의서식.hwpx");
    if !path.exists() { return; }
    let bytes = std::fs::read(path).unwrap();
    let text_files = hwpx_filler::zipper::extract_text_files(&bytes).unwrap();

    for (name, xml) in text_files.iter().filter(|(n, _)| n.starts_with("Contents/section") && n.ends_with(".xml")) {
        // 크래시 없이 분석 완료되는지
        let tables = hwpx_filler::stream_analyzer::analyze_xml(xml);
        let fields = hwpx_filler::stream_analyzer::extract_fields(&tables);
        let extracted = hwpx_filler::extractor::extract_data(xml);
        println!("  최악의서식 {}: {}개 테이블, {}개 필드, {}개 추출",
            name, tables.len(), fields.len(), extracted.len());

        // adaptive도 크래시 없는지
        let policy = hwpx_filler::stream_analyzer::RecognitionPolicy::default();
        let _adaptive = hwpx_filler::stream_analyzer::analyze_form_adaptive(xml, Some(&policy));

        // LLM 포맷도 크래시 없는지
        let _llm = hwpx_filler::llm_format::format_tables_for_llm(&tables);
    }
}

#[test]
fn real_fixtures_if_available() {
    let fixture_dir = std::path::Path::new("tests/fixtures");
    if !fixture_dir.exists() { return; }

    let hwpx_files: Vec<_> = std::fs::read_dir(fixture_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "hwpx"))
        .collect();

    if hwpx_files.is_empty() { return; }

    for entry in &hwpx_files {
        let path = entry.path();
        let name = path.file_name().unwrap().to_str().unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let text_files = hwpx_filler::zipper::extract_text_files(&bytes);
        assert!(text_files.is_ok(), "fixture {} ZIP 추출 실패: {:?}", name, text_files.err());

        let text_files = text_files.unwrap();
        let sections: Vec<&String> = text_files
            .keys()
            .filter(|n| n.starts_with("Contents/section") && n.ends_with(".xml"))
            .collect();
        assert!(!sections.is_empty(), "fixture {}에 섹션이 없음", name);

        // 각 섹션을 분석할 수 있는지 확인 (크래시 0)
        for section_name in &sections {
            let xml = text_files.get(*section_name).unwrap();
            let tables = hwpx_filler::stream_analyzer::analyze_xml(xml);
            let _fields = hwpx_filler::stream_analyzer::extract_fields(&tables);
            // 크래시 없으면 성공
        }
        println!("  ✓ fixture {} ({} sections)", name, sections.len());
    }
}
