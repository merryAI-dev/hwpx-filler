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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
        // Pass 1: 가로 패턴 — [Label] [Data] 같은 행
        for (row_idx, row) in table.rows.iter().enumerate() {
            let next_row = table.rows.get(row_idx + 1);
            if looks_like_vertical_header_row(row, next_row) {
                continue;
            }

            for (i, cell) in row.cells.iter().enumerate() {
                if !cell.is_label { continue; }
                if cell.text.trim().is_empty() { continue; }

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

        // Pass 1에서 추출된 행 기록 — 세로 패턴에서 중복 방지
        let horizontal_rows: std::collections::HashSet<u32> = fields.iter()
            .filter(|f| f.table_index == table.index)
            .map(|f| f.row)
            .collect();

        // Pass 2: 세로 패턴 — 헤더 행 아래 데이터 행에서 값 추출
        for (row_idx, row) in table.rows.iter().enumerate() {
            if row.cells.is_empty() || row.cells.len() < 3 { continue; } // 최소 3열

            // 이 행이 가로 패턴에서 이미 추출된 행이면 건너뜀
            let row_addr = row.cells.first().map(|c| c.row).unwrap_or(0);
            if horizontal_rows.contains(&row_addr) { continue; }

            let next_row = table.rows.get(row_idx + 1);
            if !looks_like_vertical_header_row(row, next_row) {
                continue;
            }

            let header_cells: Vec<_> = row.cells.iter().collect();

            // 아래 데이터 행들에서 값 추출
            for data_row in table.rows.iter().skip(row_idx + 1) {
                let has_any_data = data_row.cells.iter().any(|c| !c.text.trim().is_empty());
                if !has_any_data { continue; } // 완전 빈 행은 건너뜀

                // 다음 헤더 행이면 중단
                let mostly_labels = data_row.cells.iter()
                    .filter(|c| !c.text.trim().is_empty())
                    .all(|c| c.is_label);
                if mostly_labels && has_any_data { break; }

                // 행 번호 (0-based, 헤더 이후 몇 번째 데이터인지)
                let data_row_num = data_row.cells.first().map(|c| c.row).unwrap_or(0);

                for data_cell in &data_row.cells {
                    if data_cell.text.trim().is_empty() { continue; }

                    // 같은 col의 헤더 셀 찾기
                    if let Some(header_cell) = header_cells.iter().find(|h| h.col == data_cell.col) {
                        let raw = header_cell.text.trim().to_string();
                        let normalized = normalize_label(&raw);
                        let base_key = canonical_or_normalized(&normalized);
                        // 같은 헤더에 여러 행 데이터면 key에 행 번호 추가
                        let key = format!("{}_{}", base_key, data_row_num);

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

fn looks_like_vertical_header_row(
    row: &stream_analyzer::RowInfo,
    next_row: Option<&stream_analyzer::RowInfo>,
) -> bool {
    let non_empty: Vec<_> = row.cells.iter()
        .filter(|c| !c.text.trim().is_empty())
        .collect();
    if non_empty.len() < 3 {
        return false;
    }

    let next_has_any = next_row.map(|r| {
        r.cells.iter().any(|c| !c.text.trim().is_empty())
    }).unwrap_or(false);
    if !next_has_any {
        return false;
    }

    let label_count = non_empty.iter().filter(|c| c.is_label).count();
    label_count * 3 >= non_empty.len() * 2
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

/// CSV 텍스트에서 데이터 추출 (Firebase export 등)
/// 첫 행 = 헤더(key), 데이터 행 = value
pub fn extract_csv(csv_text: &str) -> Vec<ExtractedField> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .has_headers(true)
        .from_reader(csv_text.as_bytes());

    let headers: Vec<String> = match reader.headers() {
        Ok(h) => h.iter().map(|s| s.to_string()).collect(),
        Err(_) => return Vec::new(),
    };

    // 첫 번째 데이터 행만 사용
    let record = match reader.records().next() {
        Some(Ok(r)) => r,
        _ => return Vec::new(),
    };

    headers.iter().enumerate().filter_map(|(i, header)| {
        let value = record.get(i).unwrap_or("").trim().to_string();
        if header.trim().is_empty() || value.is_empty() {
            return None;
        }
        let raw = header.trim().to_string();
        let normalized = normalize_label(&raw);
        let key = canonical_or_normalized(&normalized);
        Some(ExtractedField {
            raw_label: raw,
            normalized_label: normalized,
            key,
            value,
            table_index: 0,
            row: 0,
            col: i as u32,
        })
    }).collect()
}

// ── 상세 매핑 (wizard용) ──

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingResult {
    pub patches: Vec<PatchInfo>,
    pub mappings: Vec<MappingInfo>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchInfo {
    pub table_index: usize,
    pub row: u32,
    pub col: u32,
    pub value: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingInfo {
    pub source_key: String,
    pub source_value: String,
    pub target_label: String,
    pub target_table_index: usize,
    pub target_row: u32,
    pub target_col: u32,
    pub match_type: String,
}

/// 상세 매핑 — wizard의 미리보기 테이블용
/// 기존 map_extracted_to_form과 같은 3-tier 로직이지만 match_type 기록 + unmatched 포함
pub fn map_extracted_to_form_detailed(
    extracted: &[ExtractedField],
    form_fields: &[stream_analyzer::FieldInfo],
) -> MappingResult {
    let mut patches = Vec::new();
    let mut mappings = Vec::new();
    let mut used = vec![false; extracted.len()];

    for field in form_fields {
        let form_key = &field.canonical_key;
        let form_label = normalize_label(&field.label);

        // 1. canonical key 매칭
        if let Some((i, ex)) = extracted.iter().enumerate()
            .find(|(i, ex)| !used[*i] && ex.key == *form_key && form_key != "unknown")
        {
            patches.push(PatchInfo {
                table_index: field.table_index, row: field.row, col: field.col,
                value: ex.value.clone(),
            });
            mappings.push(MappingInfo {
                source_key: ex.key.clone(), source_value: ex.value.clone(),
                target_label: field.label.clone(),
                target_table_index: field.table_index, target_row: field.row, target_col: field.col,
                match_type: "canonical".to_string(),
            });
            used[i] = true;
            continue;
        }

        // 2. normalized label 완전 일치
        if let Some((i, ex)) = extracted.iter().enumerate()
            .find(|(i, ex)| !used[*i] && ex.normalized_label == form_label)
        {
            patches.push(PatchInfo {
                table_index: field.table_index, row: field.row, col: field.col,
                value: ex.value.clone(),
            });
            mappings.push(MappingInfo {
                source_key: ex.key.clone(), source_value: ex.value.clone(),
                target_label: field.label.clone(),
                target_table_index: field.table_index, target_row: field.row, target_col: field.col,
                match_type: "normalized".to_string(),
            });
            used[i] = true;
            continue;
        }

        // 3. fuzzy
        if let Some((i, ex)) = extracted.iter().enumerate()
            .find(|(i, ex)| {
                !used[*i] && (
                    ex.normalized_label.contains(&form_label) ||
                    form_label.contains(&ex.normalized_label)
                ) && ex.normalized_label.len() > 1 && form_label.len() > 1
            })
        {
            patches.push(PatchInfo {
                table_index: field.table_index, row: field.row, col: field.col,
                value: ex.value.clone(),
            });
            mappings.push(MappingInfo {
                source_key: ex.key.clone(), source_value: ex.value.clone(),
                target_label: field.label.clone(),
                target_table_index: field.table_index, target_row: field.row, target_col: field.col,
                match_type: "fuzzy".to_string(),
            });
            used[i] = true;
            continue;
        }

        // 4. unmatched
        mappings.push(MappingInfo {
            source_key: String::new(), source_value: String::new(),
            target_label: field.label.clone(),
            target_table_index: field.table_index, target_row: field.row, target_col: field.col,
            match_type: "unmatched".to_string(),
        });
    }

    MappingResult { patches, mappings }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn vertical_table_xml() -> &'static str {
        r#"
<sec>
  <p>
    <run>
      <tbl rowCnt="4" colCnt="3">
        <tr>
          <tc borderFillIDRef="1">
            <subList><p><run><t>주요 프로젝트 수행 경험</t></run></p></subList>
            <cellAddr colAddr="0" rowAddr="0"/>
            <cellSpan colSpan="3" rowSpan="1"/>
            <cellSz width="300" height="100"/>
          </tc>
        </tr>
        <tr>
          <tc borderFillIDRef="1">
            <subList><p><run><t>사 업 명</t></run></p></subList>
            <cellAddr colAddr="0" rowAddr="1"/>
            <cellSpan/>
            <cellSz width="100" height="100"/>
          </tc>
          <tc borderFillIDRef="1">
            <subList><p><run><t>참여기간 ( 년 월～ 년 월)</t></run></p></subList>
            <cellAddr colAddr="1" rowAddr="1"/>
            <cellSpan/>
            <cellSz width="100" height="100"/>
          </tc>
          <tc borderFillIDRef="1">
            <subList><p><run><t>담당업무</t></run></p></subList>
            <cellAddr colAddr="2" rowAddr="1"/>
            <cellSpan/>
            <cellSz width="100" height="100"/>
          </tc>
        </tr>
        <tr>
          <tc borderFillIDRef="2">
            <subList><p><run><t>프로젝트 A</t></run></p></subList>
            <cellAddr colAddr="0" rowAddr="2"/>
            <cellSpan/>
            <cellSz width="100" height="100"/>
          </tc>
          <tc borderFillIDRef="2">
            <subList><p><run><t>24.01 ~ 24.03</t></run></p></subList>
            <cellAddr colAddr="1" rowAddr="2"/>
            <cellSpan/>
            <cellSz width="100" height="100"/>
          </tc>
          <tc borderFillIDRef="2">
            <subList><p><run><t>운영</t></run></p></subList>
            <cellAddr colAddr="2" rowAddr="2"/>
            <cellSpan/>
            <cellSz width="100" height="100"/>
          </tc>
        </tr>
        <tr>
          <tc borderFillIDRef="2">
            <subList><p><run><t>프로젝트 B</t></run></p></subList>
            <cellAddr colAddr="0" rowAddr="3"/>
            <cellSpan/>
            <cellSz width="100" height="100"/>
          </tc>
          <tc borderFillIDRef="2">
            <subList><p><run><t>24.04 ~ 24.06</t></run></p></subList>
            <cellAddr colAddr="1" rowAddr="3"/>
            <cellSpan/>
            <cellSz width="100" height="100"/>
          </tc>
          <tc borderFillIDRef="2">
            <subList><p><run><t>기획</t></run></p></subList>
            <cellAddr colAddr="2" rowAddr="3"/>
            <cellSpan/>
            <cellSz width="100" height="100"/>
          </tc>
        </tr>
      </tbl>
    </run>
  </p>
</sec>
        "#
    }

    #[test]
    fn extract_data_prefers_vertical_header_over_bogus_horizontal_pair() {
        let fields = extract_data(vertical_table_xml());

        assert!(fields.iter().any(|f| f.raw_label == "사 업 명" && f.value == "프로젝트 A"));
        assert!(fields.iter().any(|f| f.raw_label == "참여기간 ( 년 월～ 년 월)" && f.value == "24.01 ~ 24.03"));
        assert!(fields.iter().any(|f| f.raw_label == "담당업무" && f.value == "운영"));
        assert!(!fields.iter().any(|f| f.raw_label == "사 업 명" && f.value == "참여기간 ( 년 월～ 년 월)"));
        assert!(!fields.iter().any(|f| f.raw_label == "24.01 ~ 24.03" && f.value == "24.04 ~ 24.06"));
        assert!(!fields.iter().any(|f| f.raw_label == "운영" && f.value == "기획"));
    }
}
