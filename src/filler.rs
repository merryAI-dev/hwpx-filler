//! 폼 채움 엔진

use crate::error::{FillerError, Result};
use crate::model::{Section, Table, RunContent};

#[derive(Debug, Clone)]
pub struct FillField {
    pub table_index: usize,
    pub row: u32,
    pub col: u32,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct DynamicRows {
    pub table_index: usize,
    pub template_row_addr: u32,
    pub rows: Vec<Vec<String>>,
}

pub fn collect_tables(section: &Section) -> Vec<&Table> {
    section.paragraphs.iter()
        .flat_map(|p| &p.runs)
        .flat_map(|r| &r.contents)
        .filter_map(|c| match c {
            RunContent::Table(t) => Some(t.as_ref()),
            _ => None,
        })
        .collect()
}

pub fn collect_tables_mut(section: &mut Section) -> Vec<&mut Table> {
    section.paragraphs.iter_mut()
        .flat_map(|p| &mut p.runs)
        .flat_map(|r| &mut r.contents)
        .filter_map(|c| match c {
            RunContent::Table(t) => Some(t.as_mut()),
            _ => None,
        })
        .collect()
}

pub fn fill_fields(section: &mut Section, fields: &[FillField]) -> Result<Vec<String>> {
    let mut warnings = Vec::new();
    let mut tables = collect_tables_mut(section);

    for field in fields {
        if field.table_index >= tables.len() {
            warnings.push(format!("테이블 {} 없음", field.table_index));
            continue;
        }
        match tables[field.table_index].get_cell_mut(field.row, field.col) {
            Some(cell) => cell.set_text(&field.value),
            None => warnings.push(format!(
                "셀 ({}, {}) 없음 (테이블 {})", field.row, field.col, field.table_index
            )),
        }
    }
    Ok(warnings)
}

pub fn fill_dynamic_rows(section: &mut Section, dynamic: &DynamicRows) -> Result<Vec<String>> {
    let mut warnings = Vec::new();
    let mut tables = collect_tables_mut(section);

    if dynamic.table_index >= tables.len() {
        return Err(FillerError::RowNotFound { table: dynamic.table_index, row: dynamic.template_row_addr });
    }

    let table = &mut tables[dynamic.table_index];
    let template_idx = table.find_row(dynamic.template_row_addr)
        .map(|(idx, _)| idx)
        .ok_or(FillerError::RowNotFound { table: dynamic.table_index, row: dynamic.template_row_addr })?;

    if dynamic.rows.len() > 1 {
        table.clone_row(template_idx, dynamic.rows.len() - 1);
    }

    for (i, row_data) in dynamic.rows.iter().enumerate() {
        let row_idx = template_idx + i;
        if row_idx < table.rows.len() {
            for (col_idx, value) in row_data.iter().enumerate() {
                if col_idx < table.rows[row_idx].cells.len() {
                    table.rows[row_idx].cells[col_idx].set_text(value);
                } else {
                    warnings.push(format!("행 {} 열 {} 초과", dynamic.template_row_addr + i as u32, col_idx));
                }
            }
        }
    }
    Ok(warnings)
}
