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

/// 패치 결과 검증 — 요청한 좌표에 실제 값이 들어갔는지 확인
pub fn verify_patches_applied(
    xml: &str,
    patches: &[(usize, u32, u32, String)],
) -> ValidationResult {
    fn norm(s: &str) -> String {
        s.chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_lowercase()
    }

    let tables = crate::stream_analyzer::analyze_xml(xml);
    let mut errors = Vec::new();

    for (table_index, row, col, expected) in patches {
        if expected.trim().is_empty() {
            continue;
        }

        let Some(table) = tables.iter().find(|t| t.index == *table_index) else {
            errors.push(format!("테이블 {}을(를) 다시 찾지 못함", table_index));
            continue;
        };

        let actual = table.rows.iter()
            .flat_map(|r| &r.cells)
            .find(|c| c.row == *row && c.col == *col)
            .map(|c| c.text.clone());

        let Some(actual) = actual else {
            errors.push(format!("테이블 {} 셀 ({}, {})을(를) 다시 찾지 못함", table_index, row, col));
            continue;
        };

        let expected_norm = norm(expected);
        let actual_norm = norm(&actual);
        if !actual_norm.contains(&expected_norm) {
            errors.push(format!(
                "테이블 {} 셀 ({}, {}) 값 불일치: expected='{}' actual='{}'",
                table_index, row, col, expected, actual
            ));
        }
    }

    ValidationResult {
        valid: errors.is_empty(),
        errors,
    }
}

#[cfg(test)]
mod tests {
    use super::verify_patches_applied;

    #[test]
    fn verify_patches_applied_catches_missed_patch() {
        let xml = r#"
<sec>
  <tbl rowCnt="1" colCnt="2">
    <tr>
      <tc><subList><p><run><t>성 명</t></run></p></subList><cellAddr colAddr="0" rowAddr="0"/></tc>
      <tc><subList><p><run><t></t></run></p></subList><cellAddr colAddr="1" rowAddr="0"/></tc>
    </tr>
  </tbl>
</sec>
        "#;

        let ok = verify_patches_applied(xml, &[(0, 0, 0, "성 명".to_string())]);
        assert!(ok.valid);

        let bad = verify_patches_applied(xml, &[(0, 0, 1, "김보람".to_string())]);
        assert!(!bad.valid);
        assert!(bad.errors[0].contains("값 불일치"));
    }
}
