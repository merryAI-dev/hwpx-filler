//! 구조 검증 — 재직렬화 왕복으로 무결성 확인
//!
//! openhwp 대비 발전: regex 태그 카운트가 아닌 실제 구조 검증
//! - 파싱 → 수정 → 직렬화 → 재파싱 왕복 테스트
//! - rowAddr 순서 검증
//! - rowCnt vs 실제 행 수 검증

use crate::error::Result;
use crate::model::Section;
use crate::parser::{parse_section, serialize_section};
use crate::filler::collect_tables;

/// 검증 결과
#[derive(Debug)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

/// Section 구조를 검증
pub fn validate_section(section: &Section) -> ValidationResult {
    let mut errors = Vec::new();

    let tables = collect_tables(section);

    for (i, table) in tables.iter().enumerate() {
        // 1. rowCnt vs 실제 행 수
        if let Some(declared) = table.row_count {
            let actual = table.rows.len() as u32;
            if declared != actual {
                errors.push(format!(
                    "테이블 {}: rowCnt={} 실제={}",
                    i, declared, actual
                ));
            }
        }

        // 2. rowAddr 순서 검증
        let mut prev_row_addr: Option<u32> = None;
        for row in &table.rows {
            if let Some(cell) = row.cells.first() {
                let addr = cell.cell_addr.row;
                if let Some(prev) = prev_row_addr {
                    if addr <= prev {
                        errors.push(format!(
                            "테이블 {}: rowAddr 역전 {} → {}",
                            i, prev, addr
                        ));
                    }
                }
                prev_row_addr = Some(addr);
            }
        }

        // 3. 같은 행 내 셀들의 rowAddr 일관성
        for row in &table.rows {
            let addrs: Vec<u32> = row.cells.iter().map(|c| c.cell_addr.row).collect();
            if let Some(first) = addrs.first() {
                if !addrs.iter().all(|a| a == first) {
                    errors.push(format!(
                        "테이블 {}: 같은 행에 다른 rowAddr {:?}",
                        i, addrs
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

/// 왕복 검증: serialize → deserialize → 비교
pub fn validate_roundtrip(section: &Section) -> Result<ValidationResult> {
    let xml = serialize_section(section)?;
    let reparsed = parse_section(&xml)?;

    let original_tables = collect_tables(section);
    let reparsed_tables = collect_tables(&reparsed);

    let mut errors = Vec::new();

    if original_tables.len() != reparsed_tables.len() {
        errors.push(format!(
            "왕복 후 테이블 수 불일치: {} → {}",
            original_tables.len(),
            reparsed_tables.len()
        ));
    }

    for (i, (orig, re)) in original_tables.iter().zip(reparsed_tables.iter()).enumerate() {
        if orig.rows.len() != re.rows.len() {
            errors.push(format!(
                "테이블 {}: 왕복 후 행 수 불일치 {} → {}",
                i, orig.rows.len(), re.rows.len()
            ));
        }
    }

    Ok(ValidationResult {
        valid: errors.is_empty(),
        errors,
    })
}
