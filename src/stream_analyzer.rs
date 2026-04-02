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

/// XML을 스트리밍하면서 테이블 구조 추출 — 어떤 HWPX든 동작
pub fn analyze_xml(xml: &str) -> Vec<TableInfo> {
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
                    "tc" if in_row => {
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
                        table_depth -= 1;
                        if table_depth == 0 && in_table {
                            // Pass 2: borderFillIDRef 기반 label 보정
                            let label_fills: std::collections::HashSet<String> = bf_label_count.iter()
                                .filter(|(bf, count)| {
                                    let data = bf_data_count.get(*bf).unwrap_or(&0);
                                    **count > *data
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

    // serde 모델에서 테이블 추출
    let serde_tables: Vec<&crate::model::Table> = section.paragraphs.iter()
        .flat_map(|p| &p.runs)
        .flat_map(|r| &r.contents)
        .filter_map(|c| match c {
            crate::model::RunContent::Table(t) => Some(t.as_ref()),
            _ => None,
        })
        .collect();

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

fn is_korean_label(text: &str) -> bool {
    let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if t.is_empty() { return false; }
    let patterns = [
        "성명", "이름", "직책", "직위", "생년", "이메일", "E-mail",
        "휴대전화", "전화", "연락처", "경력", "유사경력", "자격증",
        "참여임무", "참여기간", "사업참여기간", "참여율",
        "회사명", "근무기간", "담당업무", "비고", "발주처",
        "프로젝트", "상세경력", "상주근무", "기간", "직급",
        "소속", "부서", "학력", "전공", "주소",
    ];
    patterns.iter().any(|p| t.contains(p))
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
