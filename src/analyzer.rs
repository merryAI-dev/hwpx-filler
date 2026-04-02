//! 양식 구조 자동 분석 — openhwp에 없는 핵심 기능
//!
//! 테이블 구조를 분석해서 label/data 셀을 식별하고,
//! 필드 의미를 추론한다. 세 가지 전략:
//! 1. 구조 기반: header 속성, borderFillIDRef 패턴
//! 2. 텍스트 기반: 한국어 양식 필드명 패턴 매칭
//! 3. 위치 기반: label 옆의 data 셀 추론

use crate::model::{Table, TableCell};

/// 분석된 폼 필드
#[derive(Debug, Clone)]
pub struct FormField {
    pub table_index: usize,
    pub row: u32,
    pub col: u32,
    pub label: String,
    pub canonical_key: String,
    pub confidence: f32,
    pub cell_type: CellType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CellType {
    Label,
    Data,
    Header,   // 테이블 전체 제목 (colSpan = 전체)
    Empty,
}

/// 테이블 분석 결과
#[derive(Debug)]
pub struct FormAnalysis {
    pub fields: Vec<FormField>,
    pub label_border_fills: Vec<String>, // 이 양식에서 label에 사용된 borderFillIDRef
    pub data_border_fills: Vec<String>,
}

/// 테이블을 분석해서 label/data 셀을 식별
///
/// openhwp 대비 발전: borderFillIDRef를 하드코딩하지 않고,
/// 테이블 내 패턴을 분석해서 자동으로 label 스타일을 추론.
pub fn analyze_table(table: &Table, table_index: usize) -> FormAnalysis {
    let mut fields = Vec::new();
    let mut border_fill_counts: std::collections::HashMap<String, (usize, usize)> = std::collections::HashMap::new();

    // Pass 1: borderFillIDRef 빈도 분석
    // label 셀은 보통 같은 borderFillIDRef를 공유 (배경색 있는 스타일)
    for row in &table.rows {
        for cell in &row.cells {
            let bf = cell.border_fill_id_ref.clone().unwrap_or_default();
            let text = cell.text();
            let is_likely_label = is_korean_label(&text);
            let entry = border_fill_counts.entry(bf).or_insert((0, 0));
            if is_likely_label {
                entry.0 += 1; // label count
            } else {
                entry.1 += 1; // data count
            }
        }
    }

    // borderFillIDRef가 label에 더 많이 쓰이면 label 스타일로 판정
    let label_fills: Vec<String> = border_fill_counts.iter()
        .filter(|(_, (label, data))| *label > *data && *label > 0)
        .map(|(bf, _)| bf.clone())
        .collect();

    let data_fills: Vec<String> = border_fill_counts.iter()
        .filter(|(_, (label, data))| *data >= *label || *label == 0)
        .map(|(bf, _)| bf.clone())
        .collect();

    // Pass 2: 셀 타입 분류 + 필드 매핑
    for row in &table.rows {
        for (i, cell) in row.cells.iter().enumerate() {
            let text = cell.text();
            let bf = cell.border_fill_id_ref.clone().unwrap_or_default();

            let cell_type = classify_cell(cell, &text, &label_fills, table);

            if cell_type == CellType::Label {
                // 오른쪽에 data 셀이 있는지 확인
                if let Some(data_cell) = row.cells.get(i + 1) {
                    let data_text = data_cell.text();
                    let data_type = classify_cell(data_cell, &data_text, &label_fills, table);
                    if data_type == CellType::Data {
                        let canonical = infer_canonical_key(&text);
                        fields.push(FormField {
                            table_index,
                            row: data_cell.cell_addr.row,
                            col: data_cell.cell_addr.col,
                            label: text.trim().to_string(),
                            canonical_key: canonical.to_string(),
                            confidence: if canonical != "unknown" { 0.95 } else { 0.5 },
                            cell_type: CellType::Data,
                        });
                    }
                }
            }
        }
    }

    FormAnalysis {
        fields,
        label_border_fills: label_fills,
        data_border_fills: data_fills,
    }
}

/// 셀 타입 분류 — 여러 신호를 종합
fn classify_cell(
    cell: &TableCell,
    text: &str,
    label_fills: &[String],
    table: &Table,
) -> CellType {
    let bf = cell.border_fill_id_ref.clone().unwrap_or_default();

    // 전체 너비 병합 = 섹션 헤더
    if let Some(col_cnt) = table.col_count {
        if cell.cell_span.col_span >= col_cnt {
            return CellType::Header;
        }
    }

    // 빈 셀
    if text.trim().is_empty() {
        return CellType::Empty;
    }

    // header 속성이 true면 label
    if cell.header {
        return CellType::Label;
    }

    // borderFillIDRef로 판정
    if label_fills.contains(&bf) {
        return CellType::Label;
    }

    // 텍스트 패턴으로 판정
    if is_korean_label(text) {
        return CellType::Label;
    }

    CellType::Data
}

/// 한국어 양식 label 패턴 매칭
fn is_korean_label(text: &str) -> bool {
    let t = text.replace(char::is_whitespace, "");
    let patterns = [
        "성명", "이름", "직책", "직위", "생년", "이메일", "E-mail",
        "휴대전화", "전화", "연락처", "경력", "유사경력", "자격증",
        "참여임무", "참여기간", "사업참여기간", "참여율",
        "회사명", "근무기간", "담당업무", "비고", "발주처",
        "프로젝트", "상세경력", "상주근무",
    ];
    patterns.iter().any(|p| t.contains(p))
}

/// 한국어 label → 정규화된 영어 키
fn infer_canonical_key(label: &str) -> &'static str {
    let t = label.replace(char::is_whitespace, "");
    let map: &[(&[&str], &str)] = &[
        (&["성명", "이름"], "name"),
        (&["E-mail", "이메일"], "email"),
        (&["직책", "직위"], "position"),
        (&["생년"], "birth_date"),
        (&["휴대전화", "전화", "연락처"], "phone"),
        (&["유사경력", "경력"], "experience"),
        (&["자격증"], "certification"),
        (&["참여임무"], "task"),
        (&["사업참여기간", "참여기간"], "period"),
        (&["참여율"], "participation_rate"),
        (&["회사명"], "company"),
        (&["근무기간"], "work_period"),
        (&["담당업무"], "duties"),
        (&["비고"], "notes"),
        (&["발주처"], "client"),
    ];

    for (patterns, key) in map {
        if patterns.iter().any(|p| t.contains(p)) {
            return key;
        }
    }
    "unknown"
}
