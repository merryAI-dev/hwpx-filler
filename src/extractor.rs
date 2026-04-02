//! HWPX 데이터 추출기 — 채워진 양식에서 label-data 쌍 추출
//!
//! 용도: 서식5로 작성된 변민욱 데이터 → 서식6 양식에 자동 채움
//!
//! 전략:
//! 1. 구조 분석 (stream_analyzer)으로 label/data 셀 식별
//! 2. label 텍스트를 key, data 텍스트를 value로 추출
//! 3. 공백 정규화: "성  명" → "성명", "직    위" → "직위"
//! 4. canonical key도 함께 제공 (알려진 패턴이면)

use crate::stream_analyzer;

/// 추출된 필드 데이터
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedField {
    /// 원본 라벨 텍스트 (공백 포함, 그대로)
    pub raw_label: String,
    /// 정규화된 라벨 (공백 제거)
    pub normalized_label: String,
    /// canonical key (알려진 패턴이면 영어 키, 아니면 normalized_label)
    pub key: String,
    /// 데이터 값
    pub value: String,
    /// 어느 테이블, 어느 셀에서 왔는지
    pub table_index: usize,
    pub row: u32,
    pub col: u32,
}

/// 채워진 HWPX에서 label-data 쌍 추출
pub fn extract_data(xml: &str) -> Vec<ExtractedField> {
    let tables = stream_analyzer::analyze_xml(xml);
    let mut fields = Vec::new();

    for table in &tables {
        for row in &table.rows {
            for (i, cell) in row.cells.iter().enumerate() {
                if !cell.is_label { continue; }
                if cell.text.trim().is_empty() { continue; }

                // 오른쪽에 data 셀이 있는지
                if let Some(data_cell) = row.cells.get(i + 1) {
                    if !data_cell.is_label && !data_cell.text.trim().is_empty() {
                        let raw = cell.text.trim().to_string();
                        let normalized = normalize_label(&raw);
                        let key = canonical_or_normalized(&normalized);

                        fields.push(ExtractedField {
                            raw_label: raw,
                            normalized_label: normalized,
                            key,
                            value: data_cell.text.trim().to_string(),
                            table_index: table.index,
                            row: data_cell.row,
                            col: data_cell.col,
                        });
                    }
                }
            }
        }
    }

    fields
}

/// HWPX에서 추출한 데이터를 다른 양식에 매핑
///
/// 매칭 순서:
/// 1. canonical key 일치 (가장 정확)
/// 2. normalized label 일치 (공백 무시)
/// 3. normalized label 부분 포함 (fuzzy)
pub fn map_extracted_to_form(
    extracted: &[ExtractedField],
    form_fields: &[stream_analyzer::FieldInfo],
) -> Vec<(usize, u32, u32, String)> {
    let mut patches = Vec::new();
    let mut used = vec![false; extracted.len()];

    for field in form_fields {
        let form_key = &field.canonical_key;
        let form_label = normalize_label(&field.label);

        // 1. canonical key 매칭
        if let Some((i, ex)) = extracted.iter().enumerate()
            .find(|(i, ex)| !used[*i] && ex.key == *form_key && form_key != "unknown")
        {
            patches.push((field.table_index, field.row, field.col, ex.value.clone()));
            used[i] = true;
            continue;
        }

        // 2. normalized label 완전 일치
        if let Some((i, ex)) = extracted.iter().enumerate()
            .find(|(i, ex)| !used[*i] && ex.normalized_label == form_label)
        {
            patches.push((field.table_index, field.row, field.col, ex.value.clone()));
            used[i] = true;
            continue;
        }

        // 3. 부분 포함 (양쪽 모두)
        if let Some((i, ex)) = extracted.iter().enumerate()
            .find(|(i, ex)| {
                !used[*i] && (
                    ex.normalized_label.contains(&form_label) ||
                    form_label.contains(&ex.normalized_label)
                ) && ex.normalized_label.len() > 1 && form_label.len() > 1
            })
        {
            patches.push((field.table_index, field.row, field.col, ex.value.clone()));
            used[i] = true;
        }
    }

    patches
}

/// 라벨 정규화: 공백 제거, 줄바꿈 → 공백, 특수문자 제거
fn normalize_label(label: &str) -> String {
    label.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .replace("▪", "")
        .replace("●", "")
        .replace("○", "")
}

/// 알려진 패턴이면 canonical key, 아니면 normalized label 그대로
fn canonical_or_normalized(normalized: &str) -> String {
    let map: &[(&[&str], &str)] = &[
        (&["성명", "이름", "담당자명"], "name"),
        (&["E-mail", "이메일", "전자우편"], "email"),
        (&["직책", "직위", "직급"], "position"),
        (&["생년", "생년월일"], "birth_date"),
        (&["휴대전화", "전화", "연락처", "전화번호"], "phone"),
        (&["유사경력", "경력", "해당분야근무경력", "근무경력"], "experience"),
        (&["자격증", "관련자격증"], "certification"),
        (&["참여임무", "본사업참여임무"], "task"),
        (&["사업참여기간", "참여기간"], "period"),
        (&["참여율"], "participation_rate"),
        (&["회사명", "소속", "근무처"], "company"),
        (&["근무기간"], "work_period"),
        (&["담당업무"], "duties"),
        (&["비고"], "notes"),
        (&["발주처", "주사업자"], "client"),
        (&["학력"], "education"),
        (&["전공"], "major"),
        (&["주소"], "address"),
        (&["연령", "나이"], "age"),
    ];

    for (patterns, key) in map {
        if patterns.iter().any(|p| normalized.contains(p)) {
            return key.to_string();
        }
    }

    normalized.to_string()
}
