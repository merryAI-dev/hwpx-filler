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
        .map_err(|e| JsError::new(&format!("네트워크 연결 실패: 인터넷 연결을 확인해주세요. ({:?})", e)))?;

    let resp: Response = resp_val.dyn_into()
        .map_err(|_| JsError::new("Response 타입 캐스트 실패"))?;

    let status = resp.status();

    let text_promise = resp.text()
        .map_err(|e| JsError::new(&format!("응답 읽기 실패: {:?}", e)))?;
    let text_val = JsFuture::from(text_promise)
        .await
        .map_err(|e| JsError::new(&format!("응답 대기 실패: {:?}", e)))?;

    let body = text_val.as_string()
        .ok_or_else(|| JsError::new("응답이 문자열이 아닙니다"))?;

    // HTTP 상태 코드별 한글 에러 메시지
    match status {
        200..=299 => Ok(body),
        401 => Err(JsError::new("API 키가 유효하지 않습니다. 키를 확인해주세요.")),
        429 => Err(JsError::new("API 요청 한도를 초과했습니다. 잠시 후 다시 시도해주세요.")),
        400 => {
            // 모델 거부 또는 잘못된 요청 — 응답 본문에서 힌트 추출
            if body.contains("refusal") || body.contains("content_policy") {
                Err(JsError::new("AI가 이 요청을 처리할 수 없습니다. 입력 내용을 확인해주세요."))
            } else {
                Err(JsError::new(&format!("잘못된 요청입니다 (400): {}", &body[..body.len().min(200)])))
            }
        }
        500..=599 => Err(JsError::new(&format!("Anthropic 서버 오류입니다 ({}). 잠시 후 다시 시도해주세요.", status))),
        _ => Err(JsError::new(&format!("예상치 못한 응답 (HTTP {}): {}", status, &body[..body.len().min(200)]))),
    }
}

// ── Privacy-preserving 매핑 ─────────────────────────────────────────────────

/// 소스 HWPX에서 라벨 목록만 추출 — 값은 WASM 내부에만 존재
///
/// LLM에 전달하기 위한 함수. 값(PII)은 절대 반환하지 않음.
/// 반환: JSON string array — ["성 명", "직 책", "소 속", ...]
/// 다중 섹션 지원
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "extractLabelsOnly")]
pub fn extract_labels_only(hwpx_bytes: &[u8]) -> Result<String, JsError> {
    let text_files = crate::zipper::extract_text_files(hwpx_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let sections = find_section_xmls(&text_files)?;

    let mut labels: Vec<String> = Vec::new();
    for (_, xml) in &sections {
        let fields = crate::extractor::extract_data(xml);
        for f in fields {
            if !f.raw_label.is_empty() {
                labels.push(f.raw_label);
            }
        }
    }

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
/// 다중 섹션 지원: source/template 모두 모든 섹션 순회
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "applyLabelMappings")]
pub fn apply_label_mappings(
    source_bytes: &[u8],
    template_bytes: &[u8],
    label_pairs_json: &str,
) -> Result<Vec<u8>, JsError> {
    use std::collections::HashMap;

    fn norm(s: &str) -> String {
        s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase()
    }

    // 1. 소스: 모든 섹션에서 label → value 맵 수집
    let src_files = crate::zipper::extract_text_files(source_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let src_sections = find_section_xmls(&src_files)?;
    let mut src_map: HashMap<String, String> = HashMap::new();
    for (_, xml) in &src_sections {
        let fields = crate::extractor::extract_data(xml);
        for f in &fields {
            src_map.entry(norm(&f.raw_label)).or_insert_with(|| f.value.clone());
        }
    }

    // 2. 대상: 모든 섹션에서 label → (section_name, local_table_index, row, col) 맵
    let tpl_files = crate::zipper::extract_text_files(template_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let tpl_sections = find_section_xmls(&tpl_files)?;

    // (normalized_label) → (section_name, local_table_index, row, col)
    let mut tpl_map: HashMap<String, (String, usize, u32, u32)> = HashMap::new();
    for (section_name, xml) in &tpl_sections {
        let tables = crate::stream_analyzer::analyze_xml(xml);
        let fields = crate::stream_analyzer::extract_fields(&tables);
        for f in &fields {
            tpl_map.entry(norm(&f.label)).or_insert_with(|| {
                (section_name.to_string(), f.table_index, f.row, f.col)
            });
        }
    }

    // 3. LLM 라벨 쌍 파싱
    let pairs: Vec<serde_json::Value> = serde_json::from_str(label_pairs_json)
        .map_err(|e| JsError::new(&format!("label_pairs JSON 파싱 실패: {}", e)))?;

    // 4. 섹션별로 패치 그룹핑
    let mut section_patches: HashMap<String, Vec<(usize, u32, u32, String)>> = HashMap::new();
    for p in &pairs {
        let src_lbl = match p["sourceLabel"].as_str() { Some(s) => norm(s), None => continue };
        let tgt_lbl = match p["targetLabel"].as_str() { Some(s) => norm(s), None => continue };
        let value = match src_map.get(&src_lbl) { Some(v) if !v.is_empty() => v.clone(), _ => continue };
        let (section_name, table_idx, row, col) = match tpl_map.get(&tgt_lbl) {
            Some(t) => t.clone(),
            None => continue,
        };
        section_patches
            .entry(section_name)
            .or_default()
            .push((table_idx, row, col, value));
    }

    // 5. 각 섹션 독립 패치 + ZIP 재조립
    let mut modified = HashMap::new();
    for (section_name, patches) in &section_patches {
        let xml = tpl_files.get(section_name)
            .ok_or_else(|| JsError::new(&format!("{} not found", section_name)))?;
        let patched_xml = crate::filler::fill(xml, patches)
            .map_err(|e| JsError::new(&e.to_string()))?;
        modified.insert(section_name.clone(), patched_xml);
    }

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

#[cfg(feature = "wasm")]
fn parse_policy_json(policy_json: &str) -> Result<crate::stream_analyzer::RecognitionPolicy, JsError> {
    if policy_json.trim().is_empty() {
        Ok(crate::stream_analyzer::RecognitionPolicy::default())
    } else {
        serde_json::from_str(policy_json)
            .map_err(|e| JsError::new(&format!("policy JSON error: {}", e)))
    }
}

// ── 다중 섹션 헬퍼 ─────────────────────────────────────────────────────────
//
// HWPX 문서는 Contents/section0.xml, section1.xml, ... 등 여러 섹션을 가질 수 있다.
// 아래 헬퍼는 모든 섹션을 찾아 정렬된 순서로 반환하고,
// table_index를 글로벌하게 유니크하게 만들어주는 유틸리티.

#[cfg(feature = "wasm")]
fn find_section_xmls<'a>(
    text_files: &'a std::collections::HashMap<String, String>,
) -> Result<Vec<(&'a str, &'a str)>, JsError> {
    let mut sections: Vec<(&str, &str)> = text_files
        .iter()
        .filter(|(name, _)| {
            name.starts_with("Contents/section") && name.ends_with(".xml")
        })
        .map(|(name, content)| (name.as_str(), content.as_str()))
        .collect();
    sections.sort_by_key(|(name, _)| *name);
    if sections.is_empty() {
        return Err(JsError::new("HWPX에 섹션 파일이 없습니다 (Contents/sectionN.xml)"));
    }
    Ok(sections)
}

/// 섹션별 테이블 수를 파악해서 글로벌 table_index → (섹션 이름, 로컬 index) 매핑
#[cfg(feature = "wasm")]
struct SectionTableMap {
    entries: Vec<(String, usize)>, // (section_name, table_count)
}

#[cfg(feature = "wasm")]
impl SectionTableMap {
    fn build(
        text_files: &std::collections::HashMap<String, String>,
    ) -> Result<Self, JsError> {
        let sections = find_section_xmls(text_files)?;
        let mut entries = Vec::new();
        for (name, xml) in &sections {
            let tables = crate::stream_analyzer::analyze_xml(xml);
            entries.push((name.to_string(), tables.len()));
        }
        Ok(Self { entries })
    }

    /// 글로벌 table_index를 (섹션 이름, 로컬 table_index)로 변환
    fn resolve(&self, global_idx: usize) -> Option<(&str, usize)> {
        let mut offset = 0;
        for (name, count) in &self.entries {
            if global_idx < offset + count {
                return Some((name.as_str(), global_idx - offset));
            }
            offset += count;
        }
        None
    }
}

/// HWPX 양식 분석 — 업로드된 바이트에서 테이블 구조 + 필드 매핑 추출
/// 다중 섹션 지원: 모든 sectionN.xml을 순회하며 table_index를 글로벌하게 부여
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "analyzeForm")]
pub fn analyze_form(hwpx_bytes: &[u8]) -> Result<AnalysisResult, JsError> {
    let text_files = crate::zipper::extract_text_files(hwpx_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let sections = find_section_xmls(&text_files)?;
    let mut all_fields = Vec::new();
    let mut table_offset = 0;
    for (_, xml) in &sections {
        let tables = crate::stream_analyzer::analyze_xml(xml);
        let mut fields = crate::stream_analyzer::extract_fields(&tables);
        for f in &mut fields {
            f.table_index += table_offset;
        }
        table_offset += tables.len();
        all_fields.extend(fields);
    }
    let json = serde_json::to_string(&all_fields)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(AnalysisResult { json })
}

/// HWPX 양식 분석 — adaptive table recognition trace 포함
/// 다중 섹션 지원: 모든 sectionN.xml 순회, 글로벌 table_index
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "analyzeFormAdaptive")]
pub fn analyze_form_adaptive_wasm(
    hwpx_bytes: &[u8],
    policy_json: &str,
) -> Result<AnalysisResult, JsError> {
    let policy = parse_policy_json(policy_json)?;
    let text_files = crate::zipper::extract_text_files(hwpx_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let sections = find_section_xmls(&text_files)?;
    let mut merged = crate::stream_analyzer::AdaptiveFieldAnalysis {
        tables: Vec::new(),
        fields: Vec::new(),
        trace: Vec::new(),
    };
    let mut table_offset = 0;
    for (_, xml) in &sections {
        let mut result = crate::stream_analyzer::analyze_form_adaptive(xml, Some(&policy));
        let section_table_count = result.tables.len();
        for t in &mut result.tables { t.index += table_offset; }
        for f in &mut result.fields { f.table_index += table_offset; }
        for tr in &mut result.trace { tr.table_index += table_offset; }
        merged.tables.extend(result.tables);
        merged.fields.extend(result.fields);
        merged.trace.extend(result.trace);
        table_offset += section_table_count;
    }
    let json = serde_json::to_string(&merged)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(AnalysisResult { json })
}

/// HWPX 테이블 inspection — adaptive trace + raw grid
/// 다중 섹션 지원
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "inspectTables")]
pub fn inspect_tables_wasm(
    hwpx_bytes: &[u8],
    policy_json: &str,
) -> Result<AnalysisResult, JsError> {
    let policy = parse_policy_json(policy_json)?;
    let text_files = crate::zipper::extract_text_files(hwpx_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let sections = find_section_xmls(&text_files)?;
    let mut merged = crate::stream_analyzer::InspectTablesResult {
        tables: Vec::new(),
        trace: Vec::new(),
    };
    let mut table_offset = 0;
    for (_, xml) in &sections {
        let mut result = crate::stream_analyzer::inspect_tables_adaptive(xml, Some(&policy));
        let section_table_count = result.tables.len();
        for t in &mut result.tables { t.index += table_offset; }
        for tr in &mut result.trace { tr.table_index += table_offset; }
        merged.tables.extend(result.tables);
        merged.trace.extend(result.trace);
        table_offset += section_table_count;
    }
    let json = serde_json::to_string(&merged)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(AnalysisResult { json })
}

/// HWPX 양식 채움 — 원본 바이트 + 패치 목록 → 채워진 HWPX 바이트
/// 다중 섹션 지원: 글로벌 tableIndex를 섹션별 로컬 인덱스로 변환
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "fillForm")]
pub fn fill_form(hwpx_bytes: &[u8], patches_json: &str) -> Result<Vec<u8>, JsError> {
    let patches: Vec<serde_json::Value> = serde_json::from_str(patches_json)
        .map_err(|e| JsError::new(&format!("JSON parse error: {}", e)))?;
    let text_files = crate::zipper::extract_text_files(hwpx_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let section_map = SectionTableMap::build(&text_files)?;

    // 패치를 글로벌 index로 파싱
    let patch_list: Vec<(usize, u32, u32, String)> = patches.iter().map(|p| (
        p["tableIndex"].as_u64().unwrap_or(0) as usize,
        p["row"].as_u64().unwrap_or(0) as u32,
        p["col"].as_u64().unwrap_or(0) as u32,
        p["value"].as_str().unwrap_or("").to_string(),
    )).collect();

    // 섹션별로 패치 그룹핑 (로컬 index로 변환)
    let mut section_patches: std::collections::HashMap<String, Vec<(usize, u32, u32, String)>> =
        std::collections::HashMap::new();
    for (global_idx, row, col, value) in &patch_list {
        if let Some((section_name, local_idx)) = section_map.resolve(*global_idx) {
            section_patches
                .entry(section_name.to_string())
                .or_default()
                .push((local_idx, *row, *col, value.clone()));
        }
    }

    // 각 섹션을 독립적으로 패치
    let mut modified = std::collections::HashMap::new();
    for (section_name, patches) in &section_patches {
        let xml = text_files.get(section_name)
            .ok_or_else(|| JsError::new(&format!("{} not found", section_name)))?;
        let patched_xml = crate::filler::fill(xml, patches)
            .map_err(|e| JsError::new(&e.to_string()))?;
        modified.insert(section_name.clone(), patched_xml);
    }

    crate::zipper::patch_hwpx(hwpx_bytes, &modified)
        .map_err(|e| JsError::new(&e.to_string()))
}

/// HWPX 행 클론 — 특정 테이블의 행을 N번 복제
/// 다중 섹션 지원: 글로벌 tableIndex를 섹션별로 분배
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "cloneRows")]
pub fn clone_rows(hwpx_bytes: &[u8], clones_json: &str) -> Result<Vec<u8>, JsError> {
    let clones: Vec<serde_json::Value> = serde_json::from_str(clones_json)
        .map_err(|e| JsError::new(&format!("JSON parse error: {}", e)))?;
    let text_files = crate::zipper::extract_text_files(hwpx_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let section_map = SectionTableMap::build(&text_files)?;

    let clone_list: Vec<(usize, u32, usize)> = clones.iter().map(|c| (
        c["tableIndex"].as_u64().unwrap_or(0) as usize,
        c["templateRowAddr"].as_u64().unwrap_or(0) as u32,
        c["count"].as_u64().unwrap_or(0) as usize,
    )).collect();

    // 섹션별로 클론 요청 그룹핑
    let mut section_clones: std::collections::HashMap<String, Vec<(usize, u32, usize)>> =
        std::collections::HashMap::new();
    for (global_idx, row_addr, count) in &clone_list {
        if let Some((section_name, local_idx)) = section_map.resolve(*global_idx) {
            section_clones
                .entry(section_name.to_string())
                .or_default()
                .push((local_idx, *row_addr, *count));
        }
    }

    let mut modified = std::collections::HashMap::new();
    for (section_name, clones) in &section_clones {
        let xml = text_files.get(section_name)
            .ok_or_else(|| JsError::new(&format!("{} not found", section_name)))?;
        let patched_xml = crate::patcher::patch_clone_rows_multi(xml, clones)
            .map_err(|e| JsError::new(&e.to_string()))?;
        modified.insert(section_name.clone(), patched_xml);
    }

    crate::zipper::patch_hwpx(hwpx_bytes, &modified)
        .map_err(|e| JsError::new(&e.to_string()))
}

/// HWPX 테이블 구조를 LLM-friendly 텍스트로 포맷
/// 다중 섹션 지원: 모든 섹션의 테이블을 합쳐서 포맷
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "formatForLLM")]
pub fn format_for_llm_wasm(hwpx_bytes: &[u8]) -> Result<String, JsError> {
    let text_files = crate::zipper::extract_text_files(hwpx_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let sections = find_section_xmls(&text_files)?;
    let mut all_tables = Vec::new();
    let mut table_offset = 0;
    for (_, xml) in &sections {
        let mut tables = crate::stream_analyzer::analyze_xml(xml);
        for t in &mut tables { t.index += table_offset; }
        table_offset += tables.len();
        all_tables.extend(tables);
    }
    Ok(crate::llm_format::format_tables_for_llm(&all_tables))
}

/// HWPX 데이터 추출 — 채워진 양식에서 label:value 쌍 추출
/// 다중 섹션 지원
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "extractData")]
pub fn extract_data_wasm(hwpx_bytes: &[u8]) -> Result<AnalysisResult, JsError> {
    let text_files = crate::zipper::extract_text_files(hwpx_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let sections = find_section_xmls(&text_files)?;
    let mut all_fields = Vec::new();
    for (_, xml) in &sections {
        all_fields.extend(crate::extractor::extract_data(xml));
    }
    let json = serde_json::to_string(&all_fields)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(AnalysisResult { json })
}

/// HWPX 데이터 추출 — adaptive table recognition trace 포함
/// 다중 섹션 지원
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "extractDataAdaptive")]
pub fn extract_data_adaptive_wasm(
    hwpx_bytes: &[u8],
    policy_json: &str,
) -> Result<AnalysisResult, JsError> {
    let policy = parse_policy_json(policy_json)?;
    let text_files = crate::zipper::extract_text_files(hwpx_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let sections = find_section_xmls(&text_files)?;
    let mut merged = crate::extractor::AdaptiveExtractAnalysis {
        tables: Vec::new(),
        fields: Vec::new(),
        trace: Vec::new(),
    };
    let mut table_offset = 0;
    for (_, xml) in &sections {
        let mut result = crate::extractor::extract_data_adaptive(xml, Some(&policy));
        let section_table_count = result.tables.len();
        for t in &mut result.tables { t.index += table_offset; }
        for f in &mut result.fields { f.table_index += table_offset; }
        for tr in &mut result.trace { tr.table_index += table_offset; }
        merged.tables.extend(result.tables);
        merged.fields.extend(result.fields);
        merged.trace.extend(result.trace);
        table_offset += section_table_count;
    }
    let json = serde_json::to_string(&merged)
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

/// HWPX 테이블을 HTML <table>로 렌더링 — 채움 결과 미리보기용
/// 다중 섹션 지원: 모든 섹션의 테이블을 순서대로 렌더링
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "renderToHtml")]
pub fn render_to_html(hwpx_bytes: &[u8]) -> Result<String, JsError> {
    let text_files = crate::zipper::extract_text_files(hwpx_bytes)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let sections = find_section_xmls(&text_files)?;

    let mut html = String::new();
    let mut table_offset = 0;
    for (section_name, xml) in &sections {
        let tables = crate::stream_analyzer::analyze_xml(xml);
        if sections.len() > 1 {
            html.push_str(&format!(
                "<h4 style=\"margin:16px 0 8px;color:#64748b;font-size:12px\">{}</h4>",
                section_name.replace("Contents/", "")
            ));
        }
        for table in &tables {
            html.push_str(&format!(
                "<table class=\"preview-table\" data-table-index=\"{}\" style=\"width:100%;border-collapse:collapse;font-size:13px;margin:8px 0 16px\">",
                table.index + table_offset
            ));
            for row in &table.rows {
                html.push_str("<tr>");
                for cell in &row.cells {
                    let is_label = cell.is_label;
                    let bg = if is_label { "#f1f5f9" } else { "#fff" };
                    let fw = if is_label { "600" } else { "400" };
                    let cs = if cell.col_span > 1 { format!(" colspan=\"{}\"", cell.col_span) } else { String::new() };
                    let rs = if cell.row_span > 1 { format!(" rowspan=\"{}\"", cell.row_span) } else { String::new() };
                    let text = if cell.text.is_empty() { "&nbsp;" } else { &cell.text };
                    html.push_str(&format!(
                        "<td{}{} style=\"padding:6px 8px;border:1px solid #e2e8f0;background:{};font-weight:{}\">{}</td>",
                        cs, rs, bg, fw, html_escape(text)
                    ));
                }
                html.push_str("</tr>");
            }
            html.push_str("</table>");
        }
        table_offset += tables.len();
    }
    Ok(html)
}

#[cfg(feature = "wasm")]
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
     .replace('\n', "<br>")
}

/// 구조 피드백으로 adaptive policy 갱신
#[cfg(feature = "wasm")]
#[wasm_bindgen(js_name = "updateRecognitionPolicy")]
pub fn update_recognition_policy_wasm(
    policy_json: &str,
    feedback_json: &str,
) -> Result<AnalysisResult, JsError> {
    let policy = parse_policy_json(policy_json)?;
    let feedbacks: Vec<crate::stream_analyzer::StructureFeedback> =
        serde_json::from_str(feedback_json)
            .map_err(|e| JsError::new(&format!("feedback JSON error: {}", e)))?;
    let updated = crate::stream_analyzer::update_policy_with_feedback(&policy, &feedbacks);
    let json = serde_json::to_string(&updated)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(AnalysisResult { json })
}
