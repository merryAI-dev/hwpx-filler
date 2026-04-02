//! 테이블 구조를 LLM-friendly 텍스트로 포맷
//!
//! LLM이 한눈에 "Row 0은 라벨행, Row 1은 데이터행" 파악할 수 있도록
//! 테이블을 구조화된 텍스트로 변환.

use crate::stream_analyzer::TableInfo;

/// 모든 테이블을 LLM용 텍스트로 포맷
pub fn format_tables_for_llm(tables: &[TableInfo]) -> String {
    let mut out = String::new();

    for table in tables {
        if table.rows.is_empty() { continue; }

        out.push_str(&format!(
            "Table {} ({}행 × {}열):\n",
            table.index, table.rows.len(), table.col_count
        ));

        for row in &table.rows {
            let row_addr = row.cells.first().map(|c| c.row).unwrap_or(0);
            out.push_str(&format!("  Row {}:", row_addr));

            for cell in &row.cells {
                let text = cell.text.replace('\n', " ").trim().to_string();
                let display = if text.is_empty() {
                    "□".to_string() // 빈 셀 표시
                } else if text.chars().count() > 25 {
                    let short: String = text.chars().take(22).collect();
                    format!("{}...", short)
                } else {
                    text
                };
                out.push_str(&format!(" [{}]", display));
            }
            out.push('\n');
        }
        out.push('\n');
    }

    out
}

/// 단일 테이블을 LLM용 텍스트로 포맷
pub fn format_table_for_llm(table: &TableInfo) -> String {
    format_tables_for_llm(&[table.clone()])
}
