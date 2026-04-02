//! 구조 검증
//!
//! 두 가지 검증 경로:
//! 1. validate_stream() — streaming 분석 결과 기반 (항상 동작)
//! 2. validate_section() — serde 모델 기반 (serde 파싱 성공 시)

use crate::error::Result;
use crate::stream_analyzer::TableInfo;

/// 검증 결과
#[derive(Debug)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

/// Streaming 기반 검증 — 어떤 HWPX든 동작
pub fn validate_stream(tables: &[TableInfo]) -> ValidationResult {
    let mut errors = Vec::new();

    for table in tables {
        // 1. rowCnt vs 실제 행 수
        let actual = table.rows.len() as u32;
        if table.row_count != 0 && table.row_count != actual {
            errors.push(format!(
                "테이블 {}: rowCnt={} 실제={}",
                table.index, table.row_count, actual
            ));
        }

        // 2. rowAddr 순서 검증
        let mut prev_row_addr: Option<u32> = None;
        for row in &table.rows {
            if let Some(cell) = row.cells.first() {
                if let Some(prev) = prev_row_addr {
                    if cell.row <= prev {
                        errors.push(format!(
                            "테이블 {}: rowAddr 역전 {} → {}",
                            table.index, prev, cell.row
                        ));
                    }
                }
                prev_row_addr = Some(cell.row);
            }
        }

        // 3. 같은 행 내 셀들의 rowAddr 일관성
        for row in &table.rows {
            let addrs: Vec<u32> = row.cells.iter().map(|c| c.row).collect();
            if let Some(first) = addrs.first() {
                if !addrs.iter().all(|a| a == first) {
                    errors.push(format!(
                        "테이블 {}: 같은 행에 다른 rowAddr {:?}",
                        table.index, addrs
                    ));
                }
            }
        }
    }

    ValidationResult {
        valid: errors.is_empty(),
        errors,
    }
}

/// Serde 모델 기반 검증 (선택적 — serde 파싱 성공 시에만)
pub fn validate_section(section: &crate::model::Section) -> ValidationResult {
    use crate::model::RunContent;
    let mut errors = Vec::new();

    // serde 모델에서 테이블 추출
    let tables: Vec<&crate::model::Table> = section.paragraphs.iter()
        .flat_map(|p| &p.runs)
        .flat_map(|r| &r.contents)
        .filter_map(|c| match c {
            RunContent::Table(t) => Some(t.as_ref()),
            _ => None,
        })
        .collect();

    for (i, table) in tables.iter().enumerate() {
        if let Some(declared) = table.row_count {
            let actual = table.rows.len() as u32;
            if declared != actual {
                errors.push(format!(
                    "테이블 {}: rowCnt={} 실제={}", i, declared, actual
                ));
            }
        }
    }

    ValidationResult {
        valid: errors.is_empty(),
        errors,
    }
}

/// 왕복 검증: serialize → deserialize
pub fn validate_roundtrip(section: &crate::model::Section) -> Result<ValidationResult> {
    let xml = crate::parser::serialize_section(section)?;
    let reparsed = crate::parser::parse_section(&xml)?;

    let orig_count = section.paragraphs.len();
    let re_count = reparsed.paragraphs.len();

    let mut errors = Vec::new();
    if orig_count != re_count {
        errors.push(format!("왕복 후 문단 수 불일치: {} → {}", orig_count, re_count));
    }

    Ok(ValidationResult {
        valid: errors.is_empty(),
        errors,
    })
}
