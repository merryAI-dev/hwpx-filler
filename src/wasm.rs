//! WASM 바인딩 — 브라우저에서 직접 HWPX 폼 채움
//!
//! ## 프라이버시 설계 원칙
//!
//! WASM 모듈은 신뢰 경계(trust boundary) 역할을 한다.
//! 개인정보(이름, 전화번호, 생년월일)와 API 키는
//! WASM 선형 메모리 안에만 존재하며 JS로 노출되지 않는다.
//!
//! ```text
//! JS 영역 (신뢰 불가):          WASM 영역 (신뢰):
//!   라벨 목록만 ───────────▶  값 조회 (label → value)
//!   LLM 라벨 쌍   ─────────▶  좌표 조회 (label → row/col)
//!   ◀────────────────────── 완성된 HWPX bytes
//!
//!   API 키 (1회 전달) ──────▶  thread_local 보관
//!                              fetch() 직접 호출
//!   ◀────────────────────── 응답 JSON (키 미포함)
//! ```

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;
#[cfg(feature = "wasm")]
use wasm_bindgen::JsCast;
#[cfg(feature = "wasm")]
use std::cell::RefCell;

// ── API 키: WASM 선형 메모리에만 보관 ──────────────────────────────────────
//
// JS는 setApiKey() 호출 후 키에 재접근 불가.
// fetch()는 Rust가 직접 호출 — x-api-key 헤더가 JS를 경유하지 않음.
// 탭 닫기/새로고침 시 WASM 인스턴스와 함께 소멸 (localStorage 미사용).
#[cfg(feature = "wasm")]
thread_local! {
    static API_KEY: RefCell<Option<String>> = RefCell::new(None);
}

/// API 키 설정 — WASM 메모리에만 저장, JS/localStorage로 반출 불가
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "setApiKey")]
pub fn set_api_key(key: &str) {
    API_KEY.with(|k| *k.borrow_mut() = if key.is_empty() { None } else { Some(key.to_string()) });
}

/// API 키 해제 (민감 세션 종료 시)
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "clearApiKey")]
pub fn clear_api_key() {
    API_KEY.with(|k| *k.borrow_mut() = None);
}

/// API 키 설정 여부 확인 (키 값 자체는 반환하지 않음)
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "hasApiKey")]
pub fn has_api_key() -> bool {
    API_KEY.with(|k| k.borrow().is_some())
}

/// Anthropic API 직접 호출 — JS는 body만 전달, 키는 WASM 내부에서 헤더에 주입
///
/// JS가 알 수 있는 것: 요청 body(라벨 목록), 응답 JSON(라벨 쌍)
/// JS가 알 수 없는 것: API 키
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "callAnthropic")]
pub async fn call_anthropic(body_json: &str) -> Result<String, JsError> {
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Headers, Request, RequestInit, Response};

    let key = API_KEY.with(|k| k.borrow().clone())
        .ok_or_else(|| JsError::new("API 키가 설정되지 않았습니다. setApiKey()를 먼저 호출하세요."))?;

    // 헤더 구성 (키 포함) — JS 코드가 이 객체에 접근하기 전에 Request로 소비됨
    let headers = Headers::new()
        .map_err(|e| JsError::new(&format!("Headers: {:?}", e)))?;
    headers.set("Content-Type", "application/json")
        .map_err(|e| JsError::new(&format!("header set: {:?}", e)))?;
    headers.set("x-api-key", &key)
        .map_err(|e| JsError::new(&format!("header set: {:?}", e)))?;
    headers.set("anthropic-version", "2023-06-01")
        .map_err(|e| JsError::new(&format!("header set: {:?}", e)))?;
    headers.set("anthropic-dangerous-direct-browser-access", "true")
        .map_err(|e| JsError::new(&format!("header set: {:?}", e)))?;

    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_headers(&headers);
    opts.set_body(&JsValue::from_str(body_json));

    let request = Request::new_with_str_and_init(
        "https://api.anthropic.com/v1/messages",
        &opts,
    ).map_err(|e| JsError::new(&format!("Request: {:?}", e)))?;

    let window = web_sys::window()
        .ok_or_else(|| JsError::new("window 없음"))?;
    let resp_val = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| JsError::new(&format!("fetch 실패: {:?}", e)))?;

    let resp: Response = resp_val.dyn_into()
        .map_err(|_| JsError::new("Response 타입 캐스트 실패"))?;

    let text_promise = resp.text()
        .map_err(|e| JsError::new(&format!("resp.text(): {:?}", e)))?;
    let text_val = JsFuture::from(text_promise)
        .await
        .map_err(|e| JsError::new(&format!("text await: {:?}", e)))?;

    text_val.as_string()
        .ok_or_else(|| JsError::new("응답이 문자열이 아님"))
}

// ── Privacy-preserving 매핑 ─────────────────────────────────────────────────

/// 소스 HWPX에서 라벨 목록만 추출 — 값은 WASM 내부에만 존재
///
/// LLM에 전달하기 위한 함수. 값(PII)은 절대 반환하지 않음.
/// 반환: JSON string array — ["성 명", "직 책", "소 속", ...]
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "extractLabelsOnly")]
pub fn extract_labels_only(hwpx_bytes: &[u8]) -> Result<String, JsError> {
    let text_files = crate::zipper::extract_text_files(hwpx_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let section0 = text_files.get("Contents/section0.xml")
        .ok_or_else(|| JsError::new("section0.xml not found"))?;

    let fields = crate::extractor::extract_data(section0);
    let labels: Vec<&str> = fields.iter()
        .map(|f| f.raw_label.as_str())
        .filter(|l| !l.is_empty())
        .collect();

    serde_json::to_string(&labels)
        .map_err(|e| JsError::new(&e.to_string()))
}

/// LLM 라벨 매핑 적용 — 값 조회 + 좌표 계산을 WASM 내부에서 완결
///
/// JS가 넘기는 것:
///   - source_bytes: 채워진 소스 HWPX (값 포함, 이 함수 안에서만 사용)
///   - template_bytes: 빈 대상 양식
///   - label_pairs_json: [{"sourceLabel": "성 명", "targetLabel": "성 명"}, ...]
///     (LLM이 반환한 라벨 쌍. 값도 좌표도 없음)
///
/// JS가 받는 것: 완성된 HWPX bytes
/// JS가 볼 수 없는 것: 실제 값, 셀 좌표 계산 과정
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "applyLabelMappings")]
pub fn apply_label_mappings(
    source_bytes: &[u8],
    template_bytes: &[u8],
    label_pairs_json: &str,
) -> Result<Vec<u8>, JsError> {
    use std::collections::HashMap;

    // label 정규화 — 공백 제거 후 소문자
    fn norm(s: &str) -> String {
        s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase()
    }

    // 1. 소스: label → value 맵 (WASM 내부에서만 사용)
    let src_files = crate::zipper::extract_text_files(source_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let src_xml = src_files.get("Contents/section0.xml")
        .ok_or_else(|| JsError::new("source section0.xml not found"))?;
    let src_fields = crate::extractor::extract_data(src_xml);
    let src_map: HashMap<String, String> = src_fields.iter()
        .map(|f| (norm(&f.raw_label), f.value.clone()))
        .collect();

    // 2. 대상: label → (tableIndex, row, col) 맵
    let tpl_files = crate::zipper::extract_text_files(template_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let tpl_xml = tpl_files.get("Contents/section0.xml")
        .ok_or_else(|| JsError::new("template section0.xml not found"))?;
    let tpl_tables = crate::stream_analyzer::analyze_xml(tpl_xml);
    let tpl_fields = crate::stream_analyzer::extract_fields(&tpl_tables);
    let tpl_map: HashMap<String, (usize, u32, u32)> = tpl_fields.iter()
        .map(|f| (norm(&f.label), (f.table_index, f.row, f.col)))
        .collect();

    // 3. LLM 라벨 쌍 파싱
    let pairs: Vec<serde_json::Value> = serde_json::from_str(label_pairs_json)
        .map_err(|e| JsError::new(&format!("label_pairs JSON 파싱 실패: {}", e)))?;

    // 4. 패치 리스트 생성 — 값 조회 + 좌표 조회를 여기서 완결
    let patch_list: Vec<(usize, u32, u32, String)> = pairs.iter()
        .filter_map(|p| {
            let src_lbl = norm(p["sourceLabel"].as_str()?);
            let tgt_lbl = norm(p["targetLabel"].as_str()?);
            let value = src_map.get(&src_lbl)?.clone();
            let &(table_idx, row, col) = tpl_map.get(&tgt_lbl)?;
            if value.is_empty() { return None; }
            Some((table_idx, row, col, value))
        })
        .collect();

    // 5. 패치 적용 + ZIP 재조립
    let patched_xml = crate::patcher::patch_cells(tpl_xml, &patch_list)
        .map_err(|e| JsError::new(&e.to_string()))?;

    let mut modified = std::collections::HashMap::new();
    modified.insert("Contents/section0.xml".to_string(), patched_xml);

    crate::zipper::patch_hwpx(template_bytes, &modified)
        .map_err(|e| JsError::new(&e.to_string()))
}

// ── 기존 exports (하위 호환) ─────────────────────────────────────────────────

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
    let tables = crate::stream_analyzer::analyze_xml(section0);
    let fields = crate::stream_analyzer::extract_fields(&tables);
    let json = serde_json::to_string(&fields)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(AnalysisResult { json })
}

/// HWPX 양식 채움 — 원본 바이트 + 패치 목록 → 채워진 HWPX 바이트
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "fillForm")]
pub fn fill_form(hwpx_bytes: &[u8], patches_json: &str) -> Result<Vec<u8>, JsError> {
    let patches: Vec<serde_json::Value> = serde_json::from_str(patches_json)
        .map_err(|e| JsError::new(&format!("JSON parse error: {}", e)))?;
    let text_files = crate::zipper::extract_text_files(hwpx_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let section0 = text_files.get("Contents/section0.xml")
        .ok_or_else(|| JsError::new("section0.xml not found"))?;
    let patch_list: Vec<(usize, u32, u32, String)> = patches.iter().map(|p| (
        p["tableIndex"].as_u64().unwrap_or(0) as usize,
        p["row"].as_u64().unwrap_or(0) as u32,
        p["col"].as_u64().unwrap_or(0) as u32,
        p["value"].as_str().unwrap_or("").to_string(),
    )).collect();
    let patched_xml = crate::patcher::patch_cells(section0, &patch_list)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let mut modified = std::collections::HashMap::new();
    modified.insert("Contents/section0.xml".to_string(), patched_xml);
    crate::zipper::patch_hwpx(hwpx_bytes, &modified)
        .map_err(|e| JsError::new(&e.to_string()))
}

/// HWPX 행 클론 — 특정 테이블의 행을 N번 복제
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "cloneRows")]
pub fn clone_rows(hwpx_bytes: &[u8], clones_json: &str) -> Result<Vec<u8>, JsError> {
    let clones: Vec<serde_json::Value> = serde_json::from_str(clones_json)
        .map_err(|e| JsError::new(&format!("JSON parse error: {}", e)))?;
    let text_files = crate::zipper::extract_text_files(hwpx_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let section0 = text_files.get("Contents/section0.xml")
        .ok_or_else(|| JsError::new("section0.xml not found"))?;
    let clone_list: Vec<(usize, u32, usize)> = clones.iter().map(|c| (
        c["tableIndex"].as_u64().unwrap_or(0) as usize,
        c["templateRowAddr"].as_u64().unwrap_or(0) as u32,
        c["count"].as_u64().unwrap_or(0) as usize,
    )).collect();
    let patched_xml = crate::patcher::patch_clone_rows_multi(section0, &clone_list)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let mut modified = std::collections::HashMap::new();
    modified.insert("Contents/section0.xml".to_string(), patched_xml);
    crate::zipper::patch_hwpx(hwpx_bytes, &modified)
        .map_err(|e| JsError::new(&e.to_string()))
}

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
