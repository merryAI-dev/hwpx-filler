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

/// 분석된 필드 (label → data 매핑)
#[derive(Debug, Clone, serde::Serialize)]
pub struct FieldInfo {
    pub table_index: usize,
    pub row: u32,
    pub col: u32,
    pub label: String,
    pub canonical_key: String,
    pub confidence: f32,
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
pub fn extract_fields(tables: &[TableInfo]) -> Vec<FieldInfo> {
    let mut fields = Vec::new();

    for table in tables {
        for row in &table.rows {
            for (i, cell) in row.cells.iter().enumerate() {
                if !cell.is_label { continue; }

                // 오른쪽에 data 셀이 있는지
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
                        });
                    }
                }
            }
        }
    }

    fields
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
