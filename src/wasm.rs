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

    // 스트리밍 분석 — 어떤 HWPX든 동작
    let tables = crate::stream_analyzer::analyze_xml(section0);
    let fields = crate::stream_analyzer::extract_fields(&tables);

    let json = serde_json::to_string(&fields)
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

/// HWPX 행 클론 — 특정 테이블의 행을 N번 복제
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "cloneRows")]
pub fn clone_rows(
    hwpx_bytes: &[u8],
    clones_json: &str,
) -> Result<Vec<u8>, JsError> {
    // clones_json: [{"tableIndex": 2, "templateRowAddr": 2, "count": 3}]
    let clones: Vec<serde_json::Value> = serde_json::from_str(clones_json)
        .map_err(|e| JsError::new(&format!("JSON parse error: {}", e)))?;

    let text_files = crate::zipper::extract_text_files(hwpx_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;

    let section0 = text_files.get("Contents/section0.xml")
        .ok_or_else(|| JsError::new("section0.xml not found"))?;

    let clone_list: Vec<(usize, u32, usize)> = clones.iter().map(|c| {
        (
            c["tableIndex"].as_u64().unwrap_or(0) as usize,
            c["templateRowAddr"].as_u64().unwrap_or(0) as u32,
            c["count"].as_u64().unwrap_or(0) as usize,
        )
    }).collect();

    let patched_xml = crate::patcher::patch_clone_rows_multi(section0, &clone_list)
        .map_err(|e| JsError::new(&e.to_string()))?;

    let mut modified = std::collections::HashMap::new();
    modified.insert("Contents/section0.xml".to_string(), patched_xml);

    let output = crate::zipper::patch_hwpx(hwpx_bytes, &modified)
        .map_err(|e| JsError::new(&e.to_string()))?;

    Ok(output)
}

// ── Wizard용 exports ──

/// HWPX 테이블 구조를 LLM-friendly 텍스트로 포맷
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "formatForLLM")]
pub fn format_for_llm_wasm(hwpx_bytes: &[u8]) -> Result<String, JsError> {
    let text_files = crate::zipper::extract_text_files(hwpx_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let section0 = text_files.get("Contents/section0.xml")
        .ok_or_else(|| JsError::new("section0.xml not found"))?;
    let tables = crate::stream_analyzer::analyze_xml(section0);
    Ok(crate::llm_format::format_tables_for_llm(&tables))
}

/// HWPX 데이터 추출 — 채워진 양식에서 label:value 쌍 추출
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "extractData")]
pub fn extract_data_wasm(hwpx_bytes: &[u8]) -> Result<AnalysisResult, JsError> {
    let text_files = crate::zipper::extract_text_files(hwpx_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let section0 = text_files.get("Contents/section0.xml")
        .ok_or_else(|| JsError::new("section0.xml not found"))?;
    let fields = crate::extractor::extract_data(section0);
    let json = serde_json::to_string(&fields)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(AnalysisResult { json })
}

/// CSV 데이터 추출 — Firebase export 등
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "extractCsv")]
pub fn extract_csv_wasm(csv_text: &str) -> Result<AnalysisResult, JsError> {
    let fields = crate::extractor::extract_csv(csv_text);
    let json = serde_json::to_string(&fields)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(AnalysisResult { json })
}

/// 데이터 → 양식 매핑 (상세 결과 + match_type)
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "mapToForm")]
pub fn map_to_form_wasm(
    extracted_json: &str,
    form_fields_json: &str,
) -> Result<AnalysisResult, JsError> {
    let extracted: Vec<crate::extractor::ExtractedField> =
        serde_json::from_str(extracted_json)
            .map_err(|e| JsError::new(&format!("extracted JSON error: {}", e)))?;
    let form_fields: Vec<crate::stream_analyzer::FieldInfo> =
        serde_json::from_str(form_fields_json)
            .map_err(|e| JsError::new(&format!("fields JSON error: {}", e)))?;
    let result = crate::extractor::map_extracted_to_form_detailed(&extracted, &form_fields);
    let json = serde_json::to_string(&result)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(AnalysisResult { json })
}
