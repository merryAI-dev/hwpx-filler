//! 스트리밍 XML 분석 — serde 없이 어떤 HWPX든 분석
//!
//! serde 파싱은 알려진 태그만 처리하므로 새 양식에서 깨짐.
//! 이 모듈은 quick-xml Reader로 XML을 스트리밍하면서
//! 테이블 구조만 추출. 알 수 없는 태그는 무시하고 통과.
//!
//! 이게 진짜 범용 엔진의 핵심.

use quick_xml::Reader;
use quick_xml::events::Event;

/// 분석된 셀
#[derive(Debug, Clone)]
pub struct CellInfo {
    pub row: u32,
    pub col: u32,
    pub col_span: u32,
    pub row_span: u32,
    pub border_fill_id_ref: String,
    pub text: String,
    pub is_label: bool,
}

/// 분석된 행
#[derive(Debug, Clone)]
pub struct RowInfo {
    pub cells: Vec<CellInfo>,
}

/// 분석된 테이블
#[derive(Debug, Clone)]
pub struct TableInfo {
    pub index: usize,
    pub row_count: u32,
    pub col_count: u32,
    pub rows: Vec<RowInfo>,
}

/// 셀 내용물 타입 — serde enrichment로 알 수 있는 것
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    /// 순수 텍스트만 — 교체 안전
    TextOnly,
    /// 그림 포함 — 텍스트 교체 시 그림 손상 위험
    HasPicture,
    /// 수식 포함 — 건드리지 말 것
    HasEquation,
    /// 폼 컨트롤 (btn, checkBtn, edit 등) — 특수 처리 필요
    HasFormControl,
    /// 도형 (line, rect, ellipse 등) — 건드리지 말 것
    HasDrawing,
    /// 텍스트 + 기타 혼합
    Mixed,
    /// serde 파싱 실패로 알 수 없음 (streaming fallback)
    Unknown,
}

/// 분석된 필드 (label → data 매핑)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldInfo {
    pub table_index: usize,
    pub row: u32,
    pub col: u32,
    pub label: String,
    pub canonical_key: String,
    pub confidence: f32,
    /// 이 data 셀에 어떤 내용이 있는지 (serde enrichment)
    /// Unknown이면 serde 파싱 실패 — streaming 결과만 사용 중
    pub content_type: ContentType,
}

/// XML에서 테이블 구조 추출
///
/// 기본은 streaming 파서로 처리하지만, 중첩 테이블이 있는 문서는
/// serde 모델을 따라 leaf table만 재귀적으로 추출한다.
/// 서식 3-5처럼 "바깥 1x1 래퍼 + 안쪽 실제 테이블" 구조를 위해 필요하다.
pub fn analyze_xml(xml: &str) -> Vec<TableInfo> {
    if let Ok(section) = crate::parser::parse_section(xml) {
        if section_has_nested_tables(&section) {
            let nested_tables = analyze_nested_tables_with_serde(&section);
            if !nested_tables.is_empty() {
                return nested_tables;
            }
        }
    }

    analyze_xml_streaming(xml)
}

/// XML을 스트리밍하면서 테이블 구조 추출 — 어떤 HWPX든 동작
fn analyze_xml_streaming(xml: &str) -> Vec<TableInfo> {
    let mut reader = Reader::from_str(xml);
    let mut tables: Vec<TableInfo> = Vec::new();

    // 상태 머신
    let mut in_table = false;
    let mut table_depth = 0; // 중첩 테이블 대응
    let mut current_table = TableInfo { index: 0, row_count: 0, col_count: 0, rows: Vec::new() };
    let mut current_row = RowInfo { cells: Vec::new() };
    let mut current_cell: Option<CellInfo> = None;
    let mut in_row = false;
    let mut in_cell = false;
    let mut in_t = false; // <hp:t> 안에 있는지
    let mut text_buf = String::new();
    let mut table_count = 0usize;

    // borderFillIDRef 빈도 — label 스타일 자동 추론용
    let mut bf_label_count: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut bf_data_count: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");

                match name {
                    "tbl" => {
                        if !in_table {
                            in_table = true;
                            current_table = TableInfo {
                                index: table_count,
                                row_count: attr_u32(e, b"rowCnt"),
                                col_count: attr_u32(e, b"colCnt"),
                                rows: Vec::new(),
                            };
                            table_count += 1;
                        }
                        table_depth += 1;
                    }
                    "tr" if in_table && table_depth == 1 => {
                        in_row = true;
                        current_row = RowInfo { cells: Vec::new() };
                    }
                    "tc" if in_row && table_depth == 1 => {
                        in_cell = true;
                        current_cell = Some(CellInfo {
                            row: 0, col: 0,
                            col_span: 1, row_span: 1,
                            border_fill_id_ref: attr_str(e, b"borderFillIDRef"),
                            text: String::new(),
                            is_label: false,
                        });
                    }
                    "t" if in_cell => {
                        in_t = true;
                        text_buf.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");

                if let Some(ref mut cell) = current_cell {
                    match name {
                        "cellAddr" => {
                            cell.row = attr_u32(e, b"rowAddr");
                            cell.col = attr_u32(e, b"colAddr");
                        }
                        "cellSpan" => {
                            cell.col_span = attr_u32_default(e, b"colSpan", 1);
                            cell.row_span = attr_u32_default(e, b"rowSpan", 1);
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_t {
                    text_buf.push_str(&e.unescape().unwrap_or_default());
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");

                match name {
                    "t" if in_t => {
                        if let Some(ref mut cell) = current_cell {
                            if !cell.text.is_empty() {
                                cell.text.push('\n');
                            }
                            cell.text.push_str(&text_buf);
                        }
                        in_t = false;
                    }
                    "tc" if in_cell => {
                        if let Some(mut cell) = current_cell.take() {
                            cell.text = cell.text.trim().to_string();
                            // label 판정: 텍스트 패턴 + borderFillIDRef 빈도
                            cell.is_label = is_korean_label(&cell.text);
                            let bf = cell.border_fill_id_ref.clone();
                            if cell.is_label {
                                *bf_label_count.entry(bf).or_insert(0) += 1;
                            } else if !cell.text.is_empty() {
                                *bf_data_count.entry(bf).or_insert(0) += 1;
                            }
                            current_row.cells.push(cell);
                        }
                        in_cell = false;
                    }
                    "tr" if in_row => {
                        if !current_row.cells.is_empty() {
                            current_table.rows.push(current_row.clone());
                        }
                        current_row = RowInfo { cells: Vec::new() };
                        in_row = false;
                    }
                    "tbl" => {
                        if table_depth > 0 {
                            table_depth -= 1;
                        }
                        if table_depth == 0 && in_table {
                            // Pass 2: borderFillIDRef 기반 label 보정
                            // 보수적 기준: 이 fill style이 데이터 셀에 한 번도 나타나지 않을 때만 프로모션.
                            // "label_count > data_count" 조건은 fill style을 공유하는 경우(서식3-5처럼
                            // 모든 셀이 같은 스타일) 데이터 셀을 label로 잘못 분류한다.
                            let label_fills: std::collections::HashSet<String> = bf_label_count.iter()
                                .filter(|(bf, _count)| {
                                    let data = bf_data_count.get(*bf).unwrap_or(&0);
                                    *data == 0   // 이 스타일이 데이터 셀에 전혀 없을 때만
                                })
                                .map(|(bf, _)| bf.clone())
                                .collect();

                            for row in &mut current_table.rows {
                                for cell in &mut row.cells {
                                    if !cell.is_label && label_fills.contains(&cell.border_fill_id_ref) && !cell.text.is_empty() {
                                        cell.is_label = true;
                                    }
                                }
                            }

                            tables.push(current_table.clone());
                            bf_label_count.clear();
                            bf_data_count.clear();
                            in_table = false;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    tables
}

fn section_has_nested_tables(section: &crate::model::Section) -> bool {
    collect_root_tables(section)
        .into_iter()
        .any(table_contains_nested_tables)
}

fn analyze_nested_tables_with_serde(section: &crate::model::Section) -> Vec<TableInfo> {
    let mut tables = Vec::new();
    let mut next_index = 0usize;

    for table in collect_root_tables(section) {
        collect_leaf_tables(table, &mut tables, &mut next_index);
    }

    tables
}

fn collect_leaf_table_refs<'a>(
    table: &'a crate::model::Table,
    out: &mut Vec<&'a crate::model::Table>,
) {
    let mut has_nested = false;

    for para in table.rows.iter().flat_map(|r| &r.cells).flat_map(|c| &c.sub_list.paragraphs) {
        for run in &para.runs {
            for content in &run.contents {
                if let crate::model::RunContent::Table(nested) = content {
                    has_nested = true;
                    collect_leaf_table_refs(nested.as_ref(), out);
                }
            }
        }
    }

    if !has_nested {
        out.push(table);
    }
}

fn collect_leaf_root_tables<'a>(section: &'a crate::model::Section) -> Vec<&'a crate::model::Table> {
    let mut out = Vec::new();
    for table in collect_root_tables(section) {
        collect_leaf_table_refs(table, &mut out);
    }
    out
}

fn collect_root_tables(section: &crate::model::Section) -> Vec<&crate::model::Table> {
    section.paragraphs.iter()
        .flat_map(|p| &p.runs)
        .flat_map(|r| &r.contents)
        .filter_map(|c| match c {
            crate::model::RunContent::Table(t) => Some(t.as_ref()),
            _ => None,
        })
        .collect()
}

fn table_contains_nested_tables(table: &crate::model::Table) -> bool {
    table.rows.iter()
        .flat_map(|r| &r.cells)
        .flat_map(|c| &c.sub_list.paragraphs)
        .flat_map(|p| &p.runs)
        .flat_map(|r| &r.contents)
        .any(|c| matches!(c, crate::model::RunContent::Table(_)))
}

fn collect_leaf_tables(
    table: &crate::model::Table,
    out: &mut Vec<TableInfo>,
    next_index: &mut usize,
) {
    let mut has_nested = false;

    for para in table.rows.iter().flat_map(|r| &r.cells).flat_map(|c| &c.sub_list.paragraphs) {
        for run in &para.runs {
            for content in &run.contents {
                if let crate::model::RunContent::Table(nested) = content {
                    has_nested = true;
                    collect_leaf_tables(nested.as_ref(), out, next_index);
                }
            }
        }
    }

    if !has_nested {
        let index = *next_index;
        *next_index += 1;
        out.push(table_info_from_serde(table, index));
    }
}

fn table_info_from_serde(table: &crate::model::Table, index: usize) -> TableInfo {
    let mut info = TableInfo {
        index,
        row_count: table.row_count.unwrap_or(table.rows.len() as u32),
        col_count: table.col_count.unwrap_or_else(|| {
            table.rows.iter()
                .flat_map(|r| &r.cells)
                .map(|c| c.cell_addr.col + c.cell_span.col_span)
                .max()
                .unwrap_or(0)
        }),
        rows: table.rows.iter().map(|row| {
            RowInfo {
                cells: row.cells.iter().map(|cell| CellInfo {
                    row: cell.cell_addr.row,
                    col: cell.cell_addr.col,
                    col_span: cell.cell_span.col_span,
                    row_span: cell.cell_span.row_span,
                    border_fill_id_ref: cell.border_fill_id_ref.clone().unwrap_or_default(),
                    text: cell.text().trim().to_string(),
                    is_label: false,
                }).collect(),
            }
        }).collect(),
    };

    classify_labels_in_table(&mut info);
    info
}

fn classify_labels_in_table(table: &mut TableInfo) {
    let mut bf_label_count: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut bf_data_count: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for row in &mut table.rows {
        for cell in &mut row.cells {
            cell.is_label = is_korean_label(&cell.text);
            let bf = cell.border_fill_id_ref.clone();
            if cell.is_label {
                *bf_label_count.entry(bf).or_insert(0) += 1;
            } else if !cell.text.is_empty() {
                *bf_data_count.entry(bf).or_insert(0) += 1;
            }
        }
    }

    let label_fills: std::collections::HashSet<String> = bf_label_count.iter()
        .filter(|(bf, _)| {
            let data = bf_data_count.get(*bf).unwrap_or(&0);
            *data == 0
        })
        .map(|(bf, _)| bf.clone())
        .collect();

    for row in &mut table.rows {
        for cell in &mut row.cells {
            if !cell.is_label && !cell.text.is_empty() && label_fills.contains(&cell.border_fill_id_ref) {
                cell.is_label = true;
            }
        }
    }
}

/// 테이블에서 label→data 필드 매핑 추출
///
/// 두 가지 패턴을 감지:
/// 1. 가로 패턴: [Label] [Data] — 같은 행에서 라벨 옆에 데이터
/// 2. 세로 패턴: 헤더 행(전부 label/text) 아래에 데이터 행(전부 empty) — 컬럼별 필드
pub fn extract_fields(tables: &[TableInfo]) -> Vec<FieldInfo> {
    let mut fields = Vec::new();

    for table in tables {
        // Pass 1: 가로 패턴 (기존)
        for row in &table.rows {
            for (i, cell) in row.cells.iter().enumerate() {
                if !cell.is_label { continue; }
                if let Some(data_cell) = row.cells.get(i + 1) {
                    if !data_cell.is_label {
                        let key = infer_canonical_key(&cell.text);
                        fields.push(FieldInfo {
                            table_index: table.index,
                            row: data_cell.row,
                            col: data_cell.col,
                            label: cell.text.clone(),
                            canonical_key: key.to_string(),
                            confidence: if key != "unknown" { 0.95 } else { 0.5 },
                            content_type: ContentType::Unknown,
                        });
                    }
                }
            }
        }

        // Pass 2: 세로 패턴 — 헤더 행 감지 + 아래 데이터 행들
        // 헤더 행 조건: 모든 셀이 텍스트 있음 + 바로 아래 행의 셀이 대부분 비어있음
        for (row_idx, row) in table.rows.iter().enumerate() {
            if row.cells.is_empty() { continue; }
            if row.cells.len() < 2 { continue; } // 최소 2열 이상

            // 이 행의 셀이 전부 텍스트가 있는가?
            let all_have_text = row.cells.iter().all(|c| !c.text.trim().is_empty());
            if !all_have_text { continue; }

            // 바로 아래 행이 있고, 대부분 비어있는가?
            let next_row = table.rows.get(row_idx + 1);
            let is_header_row = if let Some(next) = next_row {
                let empty_count = next.cells.iter().filter(|c| c.text.trim().is_empty()).count();
                let total = next.cells.len();
                total > 0 && empty_count as f32 / total as f32 >= 0.7
            } else {
                false
            };

            if !is_header_row { continue; }

            // 이 행 = 세로 테이블 헤더! 아래 데이터 행들에 대해 컬럼별 필드 생성
            let header_cells: Vec<&CellInfo> = row.cells.iter().collect();

            // 가로 패턴에서 이미 잡힌 필드의 위치를 제외
            let already_mapped: std::collections::HashSet<(u32, u32)> = fields.iter()
                .map(|f| (f.row, f.col))
                .collect();

            for data_row in table.rows.iter().skip(row_idx + 1) {
                // 데이터 행이 아닌 다른 헤더 행이 나오면 중단
                let has_data = data_row.cells.iter().any(|c| !c.text.trim().is_empty());
                let mostly_labels = data_row.cells.iter()
                    .filter(|c| !c.text.trim().is_empty())
                    .all(|c| c.is_label);
                if has_data && mostly_labels { break; } // 다음 섹션 헤더

                for data_cell in &data_row.cells {
                    if already_mapped.contains(&(data_cell.row, data_cell.col)) { continue; }

                    // 같은 col을 가진 헤더 셀 찾기
                    if let Some(header_cell) = header_cells.iter()
                        .find(|h| h.col == data_cell.col)
                    {
                        let key = infer_canonical_key(&header_cell.text);
                        fields.push(FieldInfo {
                            table_index: table.index,
                            row: data_cell.row,
                            col: data_cell.col,
                            label: header_cell.text.clone(),
                            canonical_key: key.to_string(),
                            confidence: if key != "unknown" { 0.8 } else { 0.4 },
                            content_type: ContentType::Unknown,
                        });
                    }
                }
            }
        }
    }

    fields
}

/// serde enrichment — 파싱 성공 시 각 필드의 content_type을 갱신
///
/// streaming 분석은 텍스트만 보지만, serde는 셀 안의 Picture, Equation,
/// FormControl 등을 타입으로 구분할 수 있다.
/// serde 파싱이 실패하면 이 함수를 호출하지 않으면 됨 — content_type은 Unknown으로 남음.
pub fn enrich_with_serde(fields: &mut [FieldInfo], xml: &str) {
    let section = match crate::parser::parse_section(xml) {
        Ok(s) => s,
        Err(_) => return, // serde 실패 → enrichment 없이 진행
    };

    // analyze_xml()와 동일한 leaf-table 순서를 사용해야 table_index가 맞는다.
    let serde_tables = collect_leaf_root_tables(&section);

    for field in fields.iter_mut() {
        if field.table_index >= serde_tables.len() {
            continue;
        }

        let table = serde_tables[field.table_index];
        if let Some(cell) = table.get_cell(field.row, field.col) {
            field.content_type = classify_cell_content(cell);
        }
    }
}

/// serde TableCell의 RunContent를 분석해서 ContentType 결정
fn classify_cell_content(cell: &crate::model::TableCell) -> ContentType {
    use crate::model::RunContent;

    let mut has_text = false;
    let mut has_other = false;
    let mut specific_type: Option<ContentType> = None;

    for para in &cell.sub_list.paragraphs {
        for run in &para.runs {
            for content in &run.contents {
                match content {
                    RunContent::Text(_) => has_text = true,
                    RunContent::Picture(_) => {
                        has_other = true;
                        specific_type = Some(ContentType::HasPicture);
                    }
                    RunContent::Equation(_) => {
                        has_other = true;
                        specific_type = Some(ContentType::HasEquation);
                    }
                    RunContent::Button(_) | RunContent::RadioButton(_) |
                    RunContent::CheckButton(_) | RunContent::ComboBox(_) |
                    RunContent::ListBox(_) | RunContent::Edit(_) |
                    RunContent::ScrollBar(_) => {
                        has_other = true;
                        specific_type = Some(ContentType::HasFormControl);
                    }
                    RunContent::Line(_) | RunContent::Rectangle(_) |
                    RunContent::Ellipse(_) | RunContent::Arc(_) |
                    RunContent::Polygon(_) | RunContent::Curve(_) |
                    RunContent::ConnectLine(_) => {
                        has_other = true;
                        specific_type = Some(ContentType::HasDrawing);
                    }
                    // Table, SectionDef, TextArt, Ole 등은 무시
                    _ => {}
                }
            }
        }
    }

    if has_text && has_other {
        ContentType::Mixed
    } else if has_other {
        specific_type.unwrap_or(ContentType::Unknown)
    } else {
        ContentType::TextOnly
    }
}

// ── 헬퍼 ──

fn attr_u32(e: &quick_xml::events::BytesStart, key: &[u8]) -> u32 {
    e.attributes()
        .filter_map(|a| a.ok())
        .find(|a| a.key.as_ref() == key)
        .and_then(|a| std::str::from_utf8(&a.value).ok()?.parse().ok())
        .unwrap_or(0)
}

fn attr_u32_default(e: &quick_xml::events::BytesStart, key: &[u8], default: u32) -> u32 {
    e.attributes()
        .filter_map(|a| a.ok())
        .find(|a| a.key.as_ref() == key)
        .and_then(|a| std::str::from_utf8(&a.value).ok()?.parse().ok())
        .unwrap_or(default)
}

fn attr_str(e: &quick_xml::events::BytesStart, key: &[u8]) -> String {
    e.attributes()
        .filter_map(|a| a.ok())
        .find(|a| a.key.as_ref() == key)
        .map(|a| String::from_utf8_lossy(&a.value).to_string())
        .unwrap_or_default()
}

/// 라벨 셀 판별 — kordoc의 3단계 휴리스틱 채택
///
/// 1. 키워드 매칭 (한국 공문서 필드명)
/// 2. 짧은 한글 텍스트 (2-8자, 숫자 없음, 괄호/콜론 허용)
/// 3. "라벨:" 또는 "라벨 :" 패턴
///
/// 참고: kordoc (https://github.com/chrisryugj/kordoc) src/form/recognize.ts
fn is_korean_label(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() { return false; }

    // 공백 제거 버전 (키워드 매칭용)
    let normalized: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    let char_count = normalized.chars().count();

    // 너무 긴 텍스트는 라벨이 아님 (데이터 설명이나 본문)
    if char_count > 20 { return false; }

    // 1. 키워드 매칭 — 한국 공문서에서 흔한 필드명
    // "전공" 같은 짧은 키워드가 긴 데이터 텍스트에 포함될 수 있으므로
    // 지나치게 긴 문자열에는 contains 매칭을 적용하지 않는다.
    let keywords = [
        // 인적사항
        "성명", "이름", "소속", "직책", "직위", "직급", "부서",
        "생년", "생년월일", "주민등록번호", "연령",
        "이메일", "E-mail", "전화", "휴대전화", "연락처", "팩스",
        "주소", "학력", "전공", "학교",
        // 사업/경력
        "경력", "유사경력", "자격증", "근무경력",
        "참여임무", "참여기간", "사업참여기간", "참여율",
        "회사명", "근무기간", "담당업무", "비고", "발주처",
        "상세경력", "사업명", "사업개요",
        "투입기간", "기술분야", "관련기술", "담당임무", "전문분야",
        // 이름/언어 구분
        "국문", "영문",
        // 인적사항 추가
        "성별", "사업유형", "학위", "훈격",
        // 공문서 일반
        "신청인", "대표자", "담당자", "작성자", "확인자", "승인자",
        "일시", "날짜", "기간", "장소", "목적", "사유",
        "금액", "수량", "단가", "합계",
    ];
    if keywords.iter().any(|kw| {
        normalized == *kw || (char_count <= 10 && normalized.contains(kw))
    }) {
        return true;
    }

    // 2. 짧은 한글 텍스트 — 보수적 적용
    // 원본 텍스트(공백 포함)에서 판단해서 "성 명" (공백 있는 라벨) 감지
    // 단, 사람 이름(2-3자 한글)이나 직위("사업총괄") 같은 데이터와 구분하기 위해:
    //   - 공백으로 분리된 한자어 패턴 ("성 명", "직 책", "학 력") → label
    //   - 순수 한글 2-3자 ("해민영", "팀장") → 이름/직위일 수 있으므로 비허용
    //   - 4자 이상 순수 한글 ("사업총괄", "벤처전문위원") → 데이터일 수 있으므로 비허용
    // 결론: 규칙 2는 "공백으로 분리된 한자어 라벨"에만 적용
    let original_trimmed = text.trim();
    let words: Vec<&str> = original_trimmed.split_whitespace().collect();
    if words.len() >= 2 && words.len() <= 4 {
        // "성 명", "직 책", "학 력", "비 고"처럼 각 음절이 띄어진 패턴만 허용.
        // "수석 컨설턴트", "사업 관리 및 멘토링" 같은 정상 데이터는 제외해야 한다.
        let all_short_korean = words.iter().all(|w| {
            let wlen = w.chars().count();
            wlen == 1 && w.chars().all(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c))
        });
        let no_digits = !original_trimmed.chars().any(|c| c.is_ascii_digit());
        if all_short_korean && no_digits {
            return true;
        }
    }

    // 3. "라벨:" 또는 "라벨 :" 패턴
    if normalized.ends_with(':') || normalized.ends_with('：') {
        let without_colon: String = normalized.chars().take(char_count - 1).collect();
        if without_colon.chars().count() >= 2 {
            return true;
        }
    }

    false
}

fn infer_canonical_key(label: &str) -> &'static str {
    let t: String = label.chars().filter(|c| !c.is_whitespace()).collect();
    let map: &[(&[&str], &str)] = &[
        (&["성명", "이름"], "name"),
        (&["E-mail", "이메일"], "email"),
        (&["직책", "직위", "직급"], "position"),
        (&["생년"], "birth_date"),
        (&["휴대전화", "전화", "연락처"], "phone"),
        (&["유사경력", "경력"], "experience"),
        (&["자격증"], "certification"),
        (&["참여임무"], "task"),
        (&["사업참여기간", "참여기간"], "period"),
        (&["참여율"], "participation_rate"),
        (&["회사명", "소속"], "company"),
        (&["근무기간"], "work_period"),
        (&["담당업무"], "duties"),
        (&["비고"], "notes"),
        (&["발주처"], "client"),
        (&["학력"], "education"),
        (&["전공"], "major"),
        (&["주소"], "address"),
    ];
    for (patterns, key) in map {
        if patterns.iter().any(|p| t.contains(p)) { return key; }
    }
    "unknown"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nested_table_xml() -> &'static str {
        r#"
<sec>
  <p>
    <run>
      <tbl rowCnt="1" colCnt="1">
        <tr>
          <tc borderFillIDRef="1">
            <subList>
              <p>
                <run>
                  <tbl rowCnt="2" colCnt="4">
                    <tr>
                      <tc borderFillIDRef="6">
                        <subList><p><run><t>성 명</t></run></p></subList>
                        <cellAddr colAddr="0" rowAddr="0"/>
                        <cellSpan/>
                        <cellSz width="100" height="100"/>
                      </tc>
                      <tc borderFillIDRef="6">
                        <subList><p><run><t>김영우</t></run></p></subList>
                        <cellAddr colAddr="1" rowAddr="0"/>
                        <cellSpan/>
                        <cellSz width="100" height="100"/>
                      </tc>
                      <tc borderFillIDRef="6">
                        <subList><p><run><t>직 책</t></run></p></subList>
                        <cellAddr colAddr="2" rowAddr="0"/>
                        <cellSpan/>
                        <cellSz width="100" height="100"/>
                      </tc>
                      <tc borderFillIDRef="6">
                        <subList><p><run><t>수석 컨설턴트</t></run></p></subList>
                        <cellAddr colAddr="3" rowAddr="0"/>
                        <cellSpan/>
                        <cellSz width="100" height="100"/>
                      </tc>
                    </tr>
                    <tr>
                      <tc borderFillIDRef="6">
                        <subList><p><run><t>소속 회사</t></run></p></subList>
                        <cellAddr colAddr="0" rowAddr="1"/>
                        <cellSpan/>
                        <cellSz width="100" height="100"/>
                      </tc>
                      <tc borderFillIDRef="6">
                        <subList><p><run><t>엠와이소셜컴퍼니</t></run></p></subList>
                        <cellAddr colAddr="1" rowAddr="1"/>
                        <cellSpan colSpan="3" rowSpan="1"/>
                        <cellSz width="300" height="100"/>
                      </tc>
                    </tr>
                  </tbl>
                </run>
              </p>
            </subList>
            <cellAddr colAddr="0" rowAddr="0"/>
            <cellSpan/>
            <cellSz width="400" height="200"/>
          </tc>
        </tr>
      </tbl>
    </run>
  </p>
</sec>
        "#
    }

    #[test]
    fn analyze_xml_prefers_leaf_nested_tables() {
        let tables = analyze_xml(nested_table_xml());
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].row_count, 2);
        assert_eq!(tables[0].col_count, 4);
        assert_eq!(tables[0].rows[0].cells[0].text, "성 명");
        assert_eq!(tables[0].rows[0].cells[1].text, "김영우");
    }

    #[test]
    fn extract_fields_from_nested_leaf_table() {
        let tables = analyze_xml(nested_table_xml());
        let fields = extract_fields(&tables);
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].label, "성 명");
        assert_eq!(fields[0].canonical_key, "name");
        assert_eq!(fields[1].label, "직 책");
        assert_eq!(fields[1].canonical_key, "position");
        assert_eq!(fields[2].label, "소속 회사");
        assert_eq!(fields[2].canonical_key, "company");
    }

    #[test]
    fn extract_data_from_nested_leaf_table() {
        let fields = crate::extractor::extract_data(nested_table_xml());
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].key, "name");
        assert_eq!(fields[0].value, "김영우");
        assert_eq!(fields[1].key, "position");
        assert_eq!(fields[1].value, "수석 컨설턴트");
        assert_eq!(fields[2].key, "company");
        assert_eq!(fields[2].value, "엠와이소셜컴퍼니");
    }
}
