//! WASM 바인딩 — 브라우저에서 직접 HWPX 폼 채움
//!
//! 개인정보(이름, 전화번호, 생년월일)가 서버를 거치지 않음.
//! 사용자 브라우저에서 모든 처리 완료.

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

/// HWPX 양식 분석 결과
#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub struct AnalysisResult {
    json: String,
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
impl AnalysisResult {
    #[wasm_bindgen(getter)]
    pub fn json(&self) -> String {
        self.json.clone()
    }
}

/// HWPX 양식 분석 — 업로드된 바이트에서 테이블 구조 + 필드 매핑 추출
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "analyzeForm")]
pub fn analyze_form(hwpx_bytes: &[u8]) -> Result<AnalysisResult, JsError> {
    let text_files = crate::zipper::extract_text_files(hwpx_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;

    let section0 = text_files.get("Contents/section0.xml")
        .ok_or_else(|| JsError::new("section0.xml not found"))?;

    let section = crate::parser::parse_section(section0)
        .map_err(|e| JsError::new(&e.to_string()))?;

    let tables = crate::filler::collect_tables(&section);
    let mut all_fields = Vec::new();

    for (i, table) in tables.iter().enumerate() {
        let analysis = crate::analyzer::analyze_table(table, i);
        all_fields.extend(analysis.fields);
    }

    let json = serde_json::to_string(&all_fields.iter().map(|f| {
        serde_json::json!({
            "tableIndex": f.table_index,
            "row": f.row,
            "col": f.col,
            "label": f.label,
            "canonicalKey": f.canonical_key,
            "confidence": f.confidence,
        })
    }).collect::<Vec<_>>())
    .map_err(|e| JsError::new(&e.to_string()))?;

    Ok(AnalysisResult { json })
}

/// HWPX 양식 채움 — 원본 바이트 + 패치 목록 → 채워진 HWPX 바이트
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "fillForm")]
pub fn fill_form(
    hwpx_bytes: &[u8],
    patches_json: &str,
) -> Result<Vec<u8>, JsError> {
    // patches_json: [{"tableIndex": 1, "row": 0, "col": 1, "value": "김보람"}, ...]
    let patches: Vec<serde_json::Value> = serde_json::from_str(patches_json)
        .map_err(|e| JsError::new(&format!("JSON parse error: {}", e)))?;

    let text_files = crate::zipper::extract_text_files(hwpx_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;

    let section0 = text_files.get("Contents/section0.xml")
        .ok_or_else(|| JsError::new("section0.xml not found"))?;

    // Build patch list
    let patch_list: Vec<(usize, u32, u32, String)> = patches.iter().map(|p| {
        (
            p["tableIndex"].as_u64().unwrap_or(0) as usize,
            p["row"].as_u64().unwrap_or(0) as u32,
            p["col"].as_u64().unwrap_or(0) as u32,
            p["value"].as_str().unwrap_or("").to_string(),
        )
    }).collect();

    // Patch XML
    let patched_xml = crate::patcher::patch_cells(section0, &patch_list)
        .map_err(|e| JsError::new(&e.to_string()))?;

    // Patch ZIP
    let mut modified = std::collections::HashMap::new();
    modified.insert("Contents/section0.xml".to_string(), patched_xml);

    let output = crate::zipper::patch_hwpx(hwpx_bytes, &modified)
        .map_err(|e| JsError::new(&e.to_string()))?;

    Ok(output)
}
