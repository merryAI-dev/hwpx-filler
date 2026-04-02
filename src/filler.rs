//! 통합 폼 채움 API
//!
//! stream_analyzer + patcher + validate를 하나의 인터페이스로.
//! main.rs와 wasm.rs 모두 이 모듈을 통해 접근.

use crate::error::Result;
use crate::stream_analyzer::{FieldInfo, TableInfo};

/// 분석 결과
pub struct AnalysisResult {
    pub tables: Vec<TableInfo>,
    pub fields: Vec<FieldInfo>,
}

/// HWPX section XML 분석 — 테이블 구조 + 필드 매핑
pub fn analyze(xml: &str) -> AnalysisResult {
    let tables = crate::stream_analyzer::analyze_xml(xml);
    let fields = crate::stream_analyzer::extract_fields(&tables);
    AnalysisResult { tables, fields }
}

/// 셀 텍스트 교체
pub fn fill(
    xml: &str,
    patches: &[(usize, u32, u32, String)],
) -> Result<String> {
    crate::patcher::patch_cells(xml, patches)
}

/// 행 클론 + 셀 교체 (Phase C에서 구현)
pub fn fill_with_rows(
    xml: &str,
    cell_patches: &[(usize, u32, u32, String)],
    _row_clones: &[(usize, u32, usize)],
) -> Result<String> {
    // TODO Phase C: row_clones 처리 → patcher::patch_clone_rows
    // 현재는 셀 교체만
    crate::patcher::patch_cells(xml, cell_patches)
}

/// 패치 후 구조 검증
pub fn validate_patched(xml: &str) -> crate::validate::ValidationResult {
    let tables = crate::stream_analyzer::analyze_xml(xml);
    crate::validate::validate_stream(&tables)
}
