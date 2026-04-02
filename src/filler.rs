//! 폼 채움 엔진 — 구조체 수준 조작
//!
//! openhwp는 파싱/직렬화만 제공. 이 모듈은 그 위에 폼 채움 로직을 추가:
//! - 셀 텍스트 교체 (서식 보존)
//! - 동적 행 클론 + 주소 재계산
//! - 배치 채움 (N명분)

use crate::error::{FillerError, Result};
use crate::model::{Section, Table, Paragraph, Run};

/// 채울 필드 정의
#[derive(Debug, Clone)]
pub struct FillField {
    pub table_index: usize,
    pub row: u32,
    pub col: u32,
    pub value: String,
}

/// 동적 행 데이터
#[derive(Debug, Clone)]
pub struct DynamicRows {
    pub table_index: usize,
    pub template_row_addr: u32,
    pub rows: Vec<Vec<String>>, // 각 행의 열 데이터
}

/// Section에서 모든 테이블을 추출 (Run 내부 중첩 포함)
pub fn collect_tables(section: &Section) -> Vec<&Table> {
    let mut tables = Vec::new();
    for para in &section.paragraphs {
        for run in &para.runs {
            if let Some(ref tbl) = run.table {
                tables.push(tbl.as_ref());
            }
        }
    }
    tables
}

/// Section에서 모든 테이블을 추출 (mutable)
pub fn collect_tables_mut(section: &mut Section) -> Vec<&mut Table> {
    let mut tables = Vec::new();
    for para in &mut section.paragraphs {
        for run in &mut para.runs {
            if let Some(ref mut tbl) = run.table {
                tables.push(tbl.as_mut());
            }
        }
    }
    tables
}

/// 필드 목록으로 Section의 셀 텍스트를 교체
pub fn fill_fields(section: &mut Section, fields: &[FillField]) -> Result<Vec<String>> {
    let mut warnings = Vec::new();
    let mut tables = collect_tables_mut(section);

    for field in fields {
        if field.table_index >= tables.len() {
            warnings.push(format!("테이블 {} 없음", field.table_index));
            continue;
        }

        let table = &mut tables[field.table_index];
        match table.get_cell_mut(field.row, field.col) {
            Some(cell) => cell.set_text(&field.value),
            None => warnings.push(format!(
                "셀 ({}, {}) 없음 (테이블 {})",
                field.row, field.col, field.table_index
            )),
        }
    }

    Ok(warnings)
}

/// 동적 행 클론 + 데이터 채움
pub fn fill_dynamic_rows(
    section: &mut Section,
    dynamic: &DynamicRows,
) -> Result<Vec<String>> {
    let mut warnings = Vec::new();
    let mut tables = collect_tables_mut(section);

    if dynamic.table_index >= tables.len() {
        return Err(FillerError::RowNotFound {
            table: dynamic.table_index,
            row: dynamic.template_row_addr,
        });
    }

    let table = &mut tables[dynamic.table_index];

    // 템플릿 행 인덱스 찾기
    let template_idx = table.find_row(dynamic.template_row_addr)
        .map(|(idx, _)| idx)
        .ok_or(FillerError::RowNotFound {
            table: dynamic.table_index,
            row: dynamic.template_row_addr,
        })?;

    // 행 클론 (추가 행 수 = 데이터 행 수 - 1, 템플릿 행 재사용)
    if dynamic.rows.len() > 1 {
        table.clone_row(template_idx, dynamic.rows.len() - 1);
    }

    // 데이터 채움
    for (i, row_data) in dynamic.rows.iter().enumerate() {
        let row_addr = dynamic.template_row_addr + i as u32;

        // 이 rowAddr의 셀들을 순서대로 채움
        let row_idx = template_idx + i;
        if row_idx < table.rows.len() {
            for (col_idx, value) in row_data.iter().enumerate() {
                if col_idx < table.rows[row_idx].cells.len() {
                    table.rows[row_idx].cells[col_idx].set_text(value);
                } else {
                    warnings.push(format!("행 {} 열 {} 초과", row_addr, col_idx));
                }
            }
        }
    }

    Ok(warnings)
}
