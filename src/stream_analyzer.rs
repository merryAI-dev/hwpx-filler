//! 스트리밍 XML 분석 — serde 없이 어떤 HWPX든 분석
//!
//! serde 파싱은 알려진 태그만 처리하므로 새 양식에서 깨짐.
//! 이 모듈은 quick-xml Reader로 XML을 스트리밍하면서
//! 테이블 구조만 추출. 알 수 없는 태그는 무시하고 통과.
//!
//! 이게 진짜 범용 엔진의 핵심.

use std::collections::{HashMap, HashSet};

use quick_xml::Reader;
use quick_xml::events::Event;

const LOW_CONFIDENCE_MARGIN: f32 = 0.15;
const STRUCTURE_LEARNING_RATE: f32 = 0.05;
const MEMORY_BOOST: f32 = 3.0;
const MEMORY_PENALTY: f32 = 0.75;

/// 분석된 셀
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellInfo {
    pub row: u32,
    pub col: u32,
    pub col_span: u32,
    pub row_span: u32,
    pub border_fill_id_ref: String,
    pub text: String,
    pub is_label: bool,
}

/// 분석된 행
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowInfo {
    pub cells: Vec<CellInfo>,
}

/// 분석된 테이블
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableInfo {
    pub index: usize,
    pub row_count: u32,
    pub col_count: u32,
    pub rows: Vec<RowInfo>,
}

/// 셀 내용물 타입 — serde enrichment로 알 수 있는 것
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentType {
    /// 순수 텍스트만 — 교체 안전
    TextOnly,
    /// 그림 포함 — 텍스트 교체 시 그림 손상 위험
    HasPicture,
    /// 수식 포함 — 건드리지 말 것
    HasEquation,
    /// 폼 컨트롤 (btn, checkBtn, edit 등) — 특수 처리 필요
    HasFormControl,
    /// 도형 (line, rect, ellipse 등) — 건드리지 말 것
    HasDrawing,
    /// 텍스트 + 기타 혼합
    Mixed,
    /// serde 파싱 실패로 알 수 없음 (streaming fallback)
    Unknown,
}

/// 분석된 필드 (label → data 매핑)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldInfo {
    pub table_index: usize,
    pub row: u32,
    pub col: u32,
    pub label: String,
    pub canonical_key: String,
    pub confidence: f32,
    /// 이 data 셀에 어떤 내용이 있는지 (serde enrichment)
    /// Unknown이면 serde 파싱 실패 — streaming 결과만 사용 중
    pub content_type: ContentType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellRole {
    Label,
    Value,
    CompoundHint,
    Ignore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowKind {
    Header,
    Data,
    SectionBreak,
    BlankTemplate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableKind {
    HorizontalPairs,
    VerticalHeader,
    RowspanCompound,
    RepeatedHistory,
    WrapperIgnore,
    Mixed,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableFingerprint {
    pub family: String,
    pub row_count: u32,
    pub col_count: u32,
    pub header_tokens: Vec<String>,
    pub span_histogram: Vec<String>,
    pub border_fill_histogram: Vec<String>,
    pub empty_row_pattern: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionScore {
    pub action: String,
    pub score: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellRecognitionTrace {
    pub row: u32,
    pub col: u32,
    pub text: String,
    pub selected_role: CellRole,
    pub confidence: f32,
    pub low_confidence: bool,
    pub scores: Vec<DecisionScore>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowRecognitionTrace {
    pub row: u32,
    pub selected_kind: RowKind,
    pub confidence: f32,
    pub low_confidence: bool,
    pub scores: Vec<DecisionScore>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableRecognitionTrace {
    pub table_index: usize,
    pub fingerprint: TableFingerprint,
    pub selected_table_kind: TableKind,
    pub confidence: f32,
    pub low_confidence: bool,
    pub scores: Vec<DecisionScore>,
    pub rows: Vec<RowRecognitionTrace>,
    pub cells: Vec<CellRecognitionTrace>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdaptiveFieldAnalysis {
    pub tables: Vec<TableInfo>,
    pub fields: Vec<FieldInfo>,
    pub trace: Vec<TableRecognitionTrace>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectTablesResult {
    pub tables: Vec<TableInfo>,
    pub trace: Vec<TableRecognitionTrace>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowKindFeedback {
    pub row: u32,
    pub kind: RowKind,
    #[serde(default)]
    pub predicted_kind: Option<RowKind>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellRoleFeedback {
    pub row: u32,
    pub col: u32,
    pub role: CellRole,
    #[serde(default)]
    pub predicted_role: Option<CellRole>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructureFeedback {
    pub fingerprint: TableFingerprint,
    #[serde(default)]
    pub table_index: Option<usize>,
    #[serde(default)]
    pub table_kind: Option<TableKind>,
    #[serde(default)]
    pub predicted_table_kind: Option<TableKind>,
    #[serde(default)]
    pub row_kinds: Vec<RowKindFeedback>,
    #[serde(default)]
    pub cell_roles: Vec<CellRoleFeedback>,
    #[serde(default)]
    pub reward: Option<f32>,
    #[serde(default)]
    pub outcome: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AdaptiveActionWeights {
    #[serde(default)]
    pub rewards: HashMap<String, f32>,
    #[serde(default)]
    pub counts: HashMap<String, u32>,
}

impl AdaptiveActionWeights {
    fn score(&self, action: &str) -> f32 {
        let reward = self.rewards.get(action).copied().unwrap_or(0.0);
        let count = self.counts.get(action).copied().unwrap_or(0).max(1) as f32;
        reward / count
    }

    fn update(&mut self, action: &str, reward: f32) {
        *self.rewards.entry(action.to_string()).or_insert(0.0) += reward * STRUCTURE_LEARNING_RATE;
        *self.counts.entry(action.to_string()).or_insert(0) += 1;
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionPolicy {
    #[serde(default = "default_policy_version")]
    pub version: u32,
    #[serde(default)]
    pub feedback_events: u32,
    #[serde(default)]
    pub table_kind_biases: HashMap<String, AdaptiveActionWeights>,
    #[serde(default)]
    pub row_kind_biases: HashMap<String, AdaptiveActionWeights>,
    #[serde(default)]
    pub cell_role_biases: HashMap<String, AdaptiveActionWeights>,
    #[serde(default)]
    pub table_kind_memory: HashMap<String, String>,
    #[serde(default)]
    pub row_kind_memory: HashMap<String, HashMap<String, String>>,
    #[serde(default)]
    pub cell_role_memory: HashMap<String, HashMap<String, String>>,
}

impl Default for RecognitionPolicy {
    fn default() -> Self {
        Self {
            version: default_policy_version(),
            feedback_events: 0,
            table_kind_biases: HashMap::new(),
            row_kind_biases: HashMap::new(),
            cell_role_biases: HashMap::new(),
            table_kind_memory: HashMap::new(),
            row_kind_memory: HashMap::new(),
            cell_role_memory: HashMap::new(),
        }
    }
}

fn default_policy_version() -> u32 {
    1
}

/// XML에서 테이블 구조 추출
///
/// 기본은 streaming 파서로 처리하지만, 중첩 테이블이 있는 문서는
/// serde 모델을 따라 leaf table만 재귀적으로 추출한다.
/// 서식 3-5처럼 "바깥 1x1 래퍼 + 안쪽 실제 테이블" 구조를 위해 필요하다.
pub fn analyze_xml(xml: &str) -> Vec<TableInfo> {
    if let Ok(section) = crate::parser::parse_section(xml) {
        if section_has_nested_tables(&section) {
            let nested_tables = analyze_nested_tables_with_serde(&section);
            if !nested_tables.is_empty() {
                return nested_tables;
            }
        }
    }

    analyze_xml_streaming(xml)
}

pub fn inspect_tables_adaptive(xml: &str, policy: Option<&RecognitionPolicy>) -> InspectTablesResult {
    let mut tables = analyze_xml(xml);
    let trace = apply_recognition_policy(&mut tables, policy.unwrap_or(&RecognitionPolicy::default()));
    InspectTablesResult { tables, trace }
}

pub fn analyze_form_adaptive(xml: &str, policy: Option<&RecognitionPolicy>) -> AdaptiveFieldAnalysis {
    let mut tables = analyze_xml(xml);
    let trace = apply_recognition_policy(&mut tables, policy.unwrap_or(&RecognitionPolicy::default()));
    let mut fields = extract_fields_with_trace(&tables, Some(&trace));
    enrich_with_serde(&mut fields, xml);
    AdaptiveFieldAnalysis { tables, fields, trace }
}

pub fn update_policy_with_feedback(
    policy: &RecognitionPolicy,
    feedbacks: &[StructureFeedback],
) -> RecognitionPolicy {
    let mut next = policy.clone();

    for feedback in feedbacks {
        next.feedback_events += 1;
        let family = feedback.fingerprint.family.clone();
        let reward = feedback.reward.unwrap_or_else(|| reward_for_outcome(feedback.outcome.as_deref()));

        if let Some(kind) = feedback.table_kind {
            next.table_kind_biases
                .entry(family.clone())
                .or_default()
                .update(table_kind_name(kind), reward);
            next.table_kind_memory.insert(family.clone(), table_kind_name(kind).to_string());

            if let Some(predicted) = feedback.predicted_table_kind.filter(|predicted| *predicted != kind) {
                next.table_kind_biases
                    .entry(family.clone())
                    .or_default()
                    .update(table_kind_name(predicted), -reward.abs() / 2.0);
            }
        }

        for row_feedback in &feedback.row_kinds {
            let row_key = row_memory_key(row_feedback.row);
            next.row_kind_biases
                .entry(family.clone())
                .or_default()
                .update(row_kind_name(row_feedback.kind), reward);
            next.row_kind_memory
                .entry(family.clone())
                .or_default()
                .insert(row_key.clone(), row_kind_name(row_feedback.kind).to_string());

            if let Some(predicted) = row_feedback.predicted_kind.filter(|predicted| *predicted != row_feedback.kind) {
                next.row_kind_biases
                    .entry(family.clone())
                    .or_default()
                    .update(row_kind_name(predicted), -reward.abs() / 2.0);
            }
        }

        for cell_feedback in &feedback.cell_roles {
            let cell_key = cell_memory_key(cell_feedback.row, cell_feedback.col);
            next.cell_role_biases
                .entry(family.clone())
                .or_default()
                .update(cell_role_name(cell_feedback.role), reward);
            next.cell_role_memory
                .entry(family.clone())
                .or_default()
                .insert(cell_key.clone(), cell_role_name(cell_feedback.role).to_string());

            if let Some(predicted) = cell_feedback.predicted_role.filter(|predicted| *predicted != cell_feedback.role) {
                next.cell_role_biases
                    .entry(family.clone())
                    .or_default()
                    .update(cell_role_name(predicted), -reward.abs() / 2.0);
            }
        }

        if feedback.table_kind.is_none() && feedback.row_kinds.is_empty() && feedback.cell_roles.is_empty() {
            if let Some(kind_name) = next.table_kind_memory.get(&family).cloned() {
                next.table_kind_biases.entry(family).or_default().update(&kind_name, reward);
            }
        }
    }

    next
}

/// XML을 스트리밍하면서 테이블 구조 추출 — 어떤 HWPX든 동작
fn analyze_xml_streaming(xml: &str) -> Vec<TableInfo> {
    let mut reader = Reader::from_str(xml);
    let mut tables: Vec<TableInfo> = Vec::new();

    // 상태 머신
    let mut in_table = false;
    let mut table_depth = 0; // 중첩 테이블 대응
    let mut current_table = TableInfo { index: 0, row_count: 0, col_count: 0, rows: Vec::new() };
    let mut current_row = RowInfo { cells: Vec::new() };
    let mut current_cell: Option<CellInfo> = None;
    let mut in_row = false;
    let mut in_cell = false;
    let mut in_t = false; // <hp:t> 안에 있는지
    let mut text_buf = String::new();
    let mut table_count = 0usize;

    // borderFillIDRef 빈도 — label 스타일 자동 추론용
    let mut bf_label_count: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut bf_data_count: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");

                match name {
                    "tbl" => {
                        if !in_table {
                            in_table = true;
                            current_table = TableInfo {
                                index: table_count,
                                row_count: attr_u32(e, b"rowCnt"),
                                col_count: attr_u32(e, b"colCnt"),
                                rows: Vec::new(),
                            };
                            table_count += 1;
                        }
                        table_depth += 1;
                    }
                    "tr" if in_table && table_depth == 1 => {
                        in_row = true;
                        current_row = RowInfo { cells: Vec::new() };
                    }
                    "tc" if in_row && table_depth == 1 => {
                        in_cell = true;
                        current_cell = Some(CellInfo {
                            row: 0, col: 0,
                            col_span: 1, row_span: 1,
                            border_fill_id_ref: attr_str(e, b"borderFillIDRef"),
                            text: String::new(),
                            is_label: false,
                        });
                    }
                    "t" if in_cell => {
                        in_t = true;
                        text_buf.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");

                if let Some(ref mut cell) = current_cell {
                    match name {
                        "cellAddr" => {
                            cell.row = attr_u32(e, b"rowAddr");
                            cell.col = attr_u32(e, b"colAddr");
                        }
                        "cellSpan" => {
                            cell.col_span = attr_u32_default(e, b"colSpan", 1);
                            cell.row_span = attr_u32_default(e, b"rowSpan", 1);
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_t {
                    text_buf.push_str(&e.unescape().unwrap_or_default());
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");

                match name {
                    "t" if in_t => {
                        if let Some(ref mut cell) = current_cell {
                            if !cell.text.is_empty() {
                                cell.text.push('\n');
                            }
                            cell.text.push_str(&text_buf);
                        }
                        in_t = false;
                    }
                    "tc" if in_cell => {
                        if let Some(mut cell) = current_cell.take() {
                            cell.text = cell.text.trim().to_string();
                            // label 판정: 텍스트 패턴 + borderFillIDRef 빈도
                            cell.is_label = is_korean_label(&cell.text);
                            let bf = cell.border_fill_id_ref.clone();
                            if cell.is_label {
                                *bf_label_count.entry(bf).or_insert(0) += 1;
                            } else if !cell.text.is_empty() {
                                *bf_data_count.entry(bf).or_insert(0) += 1;
                            }
                            current_row.cells.push(cell);
                        }
                        in_cell = false;
                    }
                    "tr" if in_row => {
                        if !current_row.cells.is_empty() {
                            current_table.rows.push(current_row.clone());
                        }
                        current_row = RowInfo { cells: Vec::new() };
                        in_row = false;
                    }
                    "tbl" => {
                        if table_depth > 0 {
                            table_depth -= 1;
                        }
                        if table_depth == 0 && in_table {
                            // Pass 2: borderFillIDRef 기반 label 보정
                            // 보수적 기준: 이 fill style이 데이터 셀에 한 번도 나타나지 않을 때만 프로모션.
                            // "label_count > data_count" 조건은 fill style을 공유하는 경우(서식3-5처럼
                            // 모든 셀이 같은 스타일) 데이터 셀을 label로 잘못 분류한다.
                            let label_fills: std::collections::HashSet<String> = bf_label_count.iter()
                                .filter(|(bf, _count)| {
                                    let data = bf_data_count.get(*bf).unwrap_or(&0);
                                    *data == 0   // 이 스타일이 데이터 셀에 전혀 없을 때만
                                })
                                .map(|(bf, _)| bf.clone())
                                .collect();

                            for row in &mut current_table.rows {
                                for cell in &mut row.cells {
                                    if !cell.is_label && label_fills.contains(&cell.border_fill_id_ref) && !cell.text.is_empty() {
                                        cell.is_label = true;
                                    }
                                }
                            }

                            tables.push(current_table.clone());
                            bf_label_count.clear();
                            bf_data_count.clear();
                            in_table = false;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    tables
}

fn section_has_nested_tables(section: &crate::model::Section) -> bool {
    collect_root_tables(section)
        .into_iter()
        .any(table_contains_nested_tables)
}

fn analyze_nested_tables_with_serde(section: &crate::model::Section) -> Vec<TableInfo> {
    let mut tables = Vec::new();
    let mut next_index = 0usize;

    for table in collect_root_tables(section) {
        collect_leaf_tables(table, &mut tables, &mut next_index);
    }

    tables
}

fn collect_leaf_table_refs<'a>(
    table: &'a crate::model::Table,
    out: &mut Vec<&'a crate::model::Table>,
) {
    let mut has_nested = false;

    for para in table.rows.iter().flat_map(|r| &r.cells).flat_map(|c| &c.sub_list.paragraphs) {
        for run in &para.runs {
            for content in &run.contents {
                if let crate::model::RunContent::Table(nested) = content {
                    has_nested = true;
                    collect_leaf_table_refs(nested.as_ref(), out);
                }
            }
        }
    }

    if !has_nested {
        out.push(table);
    }
}

fn collect_leaf_root_tables<'a>(section: &'a crate::model::Section) -> Vec<&'a crate::model::Table> {
    let mut out = Vec::new();
    for table in collect_root_tables(section) {
        collect_leaf_table_refs(table, &mut out);
    }
    out
}

fn collect_root_tables(section: &crate::model::Section) -> Vec<&crate::model::Table> {
    section.paragraphs.iter()
        .flat_map(|p| &p.runs)
        .flat_map(|r| &r.contents)
        .filter_map(|c| match c {
            crate::model::RunContent::Table(t) => Some(t.as_ref()),
            _ => None,
        })
        .collect()
}

fn table_contains_nested_tables(table: &crate::model::Table) -> bool {
    table.rows.iter()
        .flat_map(|r| &r.cells)
        .flat_map(|c| &c.sub_list.paragraphs)
        .flat_map(|p| &p.runs)
        .flat_map(|r| &r.contents)
        .any(|c| matches!(c, crate::model::RunContent::Table(_)))
}

fn collect_leaf_tables(
    table: &crate::model::Table,
    out: &mut Vec<TableInfo>,
    next_index: &mut usize,
) {
    let mut has_nested = false;

    for para in table.rows.iter().flat_map(|r| &r.cells).flat_map(|c| &c.sub_list.paragraphs) {
        for run in &para.runs {
            for content in &run.contents {
                if let crate::model::RunContent::Table(nested) = content {
                    has_nested = true;
                    collect_leaf_tables(nested.as_ref(), out, next_index);
                }
            }
        }
    }

    if !has_nested {
        let index = *next_index;
        *next_index += 1;
        out.push(table_info_from_serde(table, index));
    }
}

fn table_info_from_serde(table: &crate::model::Table, index: usize) -> TableInfo {
    let mut info = TableInfo {
        index,
        row_count: table.row_count.unwrap_or(table.rows.len() as u32),
        col_count: table.col_count.unwrap_or_else(|| {
            table.rows.iter()
                .flat_map(|r| &r.cells)
                .map(|c| c.cell_addr.col + c.cell_span.col_span)
                .max()
                .unwrap_or(0)
        }),
        rows: table.rows.iter().map(|row| {
            RowInfo {
                cells: row.cells.iter().map(|cell| CellInfo {
                    row: cell.cell_addr.row,
                    col: cell.cell_addr.col,
                    col_span: cell.cell_span.col_span,
                    row_span: cell.cell_span.row_span,
                    border_fill_id_ref: cell.border_fill_id_ref.clone().unwrap_or_default(),
                    text: cell.text().trim().to_string(),
                    is_label: false,
                }).collect(),
            }
        }).collect(),
    };

    classify_labels_in_table(&mut info);
    info
}

fn classify_labels_in_table(table: &mut TableInfo) {
    let mut bf_label_count: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut bf_data_count: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for row in &mut table.rows {
        for cell in &mut row.cells {
            cell.is_label = is_korean_label(&cell.text);
            let bf = cell.border_fill_id_ref.clone();
            if cell.is_label {
                *bf_label_count.entry(bf).or_insert(0) += 1;
            } else if !cell.text.is_empty() {
                *bf_data_count.entry(bf).or_insert(0) += 1;
            }
        }
    }

    let label_fills: std::collections::HashSet<String> = bf_label_count.iter()
        .filter(|(bf, _)| {
            let data = bf_data_count.get(*bf).unwrap_or(&0);
            *data == 0
        })
        .map(|(bf, _)| bf.clone())
        .collect();

    for row in &mut table.rows {
        for cell in &mut row.cells {
            if !cell.is_label && !cell.text.is_empty() && label_fills.contains(&cell.border_fill_id_ref) {
                cell.is_label = true;
            }
        }
    }
}

fn apply_recognition_policy(
    tables: &mut [TableInfo],
    policy: &RecognitionPolicy,
) -> Vec<TableRecognitionTrace> {
    let mut traces = Vec::new();

    for table in tables {
        let fingerprint = fingerprint_table(table);
        let family = fingerprint.family.clone();
        let snapshot = table.clone();

        let mut cell_traces = Vec::new();
        for row in &mut table.rows {
            for cell in &mut row.cells {
                let default_role = default_cell_role(&snapshot, cell);
                let scores = score_cell_role(&snapshot, cell, &family, policy);
                let selection = choose_scored_action(scores, cell_role_name(default_role));
                let selected_role = parse_cell_role(&selection.action).unwrap_or(default_role);
                cell.is_label = !cell.text.trim().is_empty()
                    && matches!(selected_role, CellRole::Label | CellRole::CompoundHint);
                cell_traces.push(CellRecognitionTrace {
                    row: cell.row,
                    col: cell.col,
                    text: cell.text.clone(),
                    selected_role,
                    confidence: selection.confidence,
                    low_confidence: selection.low_confidence,
                    scores: selection.scores,
                });
            }
        }

        let row_traces = build_row_trace(table, &family, policy, &cell_traces);
        let table_selection = score_table_kind(table, &family, policy, &row_traces, &cell_traces);
        let default_table_kind = default_table_kind(table, &row_traces, &cell_traces);
        let table_kind = parse_table_kind(&table_selection.action).unwrap_or(default_table_kind);
        let low_confidence = table_selection.low_confidence
            || row_traces.iter().any(|row| row.low_confidence)
            || cell_traces.iter().any(|cell| cell.low_confidence);

        traces.push(TableRecognitionTrace {
            table_index: table.index,
            fingerprint,
            selected_table_kind: table_kind,
            confidence: table_selection.confidence,
            low_confidence,
            scores: table_selection.scores,
            rows: row_traces,
            cells: cell_traces,
        });
    }

    traces
}

fn build_row_trace(
    table: &TableInfo,
    family: &str,
    policy: &RecognitionPolicy,
    cell_traces: &[CellRecognitionTrace],
) -> Vec<RowRecognitionTrace> {
    let mut traces = Vec::new();

    for (row_idx, row) in table.rows.iter().enumerate() {
        let row_addr = row.cells.first().map(|cell| cell.row).unwrap_or(row_idx as u32);
        let default_kind = default_row_kind(table, row_idx, cell_traces);
        let scores = score_row_kind(table, row_idx, family, policy, cell_traces);
        let selection = choose_scored_action(scores, row_kind_name(default_kind));
        traces.push(RowRecognitionTrace {
            row: row_addr,
            selected_kind: parse_row_kind(&selection.action).unwrap_or(default_kind),
            confidence: selection.confidence,
            low_confidence: selection.low_confidence,
            scores: selection.scores,
        });
    }

    traces
}

fn score_cell_role(
    table: &TableInfo,
    cell: &CellInfo,
    family: &str,
    policy: &RecognitionPolicy,
) -> Vec<DecisionScore> {
    let mut scores = Vec::new();
    let label_prior = label_signal_score(&cell.text);
    let compound_prior = compound_signal_score(table, cell);
    let empty = cell.text.trim().is_empty();
    let value_prior = if empty { 0.0 } else { (0.85 - label_prior * 0.55).clamp(0.05, 0.95) };
    let ignore_prior = if empty { 0.95 } else { 0.1 + digit_ratio(&cell.text) * 0.2 };

    for (role, prior) in [
        (CellRole::Label, label_prior),
        (CellRole::Value, value_prior),
        (CellRole::CompoundHint, compound_prior),
        (CellRole::Ignore, ignore_prior),
    ] {
        let action = cell_role_name(role);
        let bias = policy.cell_role_biases.get(family).map(|weights| weights.score(action)).unwrap_or(0.0);
        let memory = memory_bonus(
            policy.cell_role_memory.get(family),
            &cell_memory_key(cell.row, cell.col),
            action,
        );
        scores.push(DecisionScore {
            action: action.to_string(),
            score: prior + bias + memory,
        });
    }

    scores
}

fn score_row_kind(
    table: &TableInfo,
    row_idx: usize,
    family: &str,
    policy: &RecognitionPolicy,
    cell_traces: &[CellRecognitionTrace],
) -> Vec<DecisionScore> {
    let Some(row) = table.rows.get(row_idx) else { return Vec::new(); };
    let row_addr = row.cells.first().map(|cell| cell.row).unwrap_or(row_idx as u32);
    let row_cells: Vec<&CellRecognitionTrace> = cell_traces.iter()
        .filter(|cell| cell.row == row_addr)
        .collect();
    let non_empty = row.cells.iter().filter(|cell| !cell.text.trim().is_empty()).count();
    let label_count = row_cells.iter()
        .filter(|cell| matches!(cell.selected_role, CellRole::Label | CellRole::CompoundHint))
        .count();
    let label_ratio = if non_empty == 0 { 0.0 } else { label_count as f32 / non_empty as f32 };
    let next_empty_ratio = next_row_empty_ratio(table, row_idx);
    let all_have_text = row.cells.iter().all(|cell| !cell.text.trim().is_empty());
    let has_any = non_empty > 0;
    let base_header = is_vertical_header_row(table, row_idx);

    let mut scores = Vec::new();
    let priors = [
        (
            RowKind::Header,
            if base_header {
                0.9
            } else {
                (0.15
                    + if all_have_text { 0.2 } else { 0.0 }
                    + label_ratio * 0.25
                    + next_empty_ratio * 0.35)
                    .clamp(0.0, 0.95)
            },
        ),
        (
            RowKind::BlankTemplate,
            if !has_any {
                0.92
            } else {
                (0.1 + next_empty_ratio * 0.2 + (1.0 - label_ratio) * 0.1).clamp(0.0, 0.8)
            },
        ),
        (
            RowKind::SectionBreak,
            if has_any && label_ratio >= 0.8 && row.cells.len() <= 2 {
                0.72
            } else {
                (0.05 + label_ratio * 0.2).clamp(0.0, 0.45)
            },
        ),
        (
            RowKind::Data,
            if has_any {
                (0.35 + (1.0 - label_ratio) * 0.35 + (1.0 - next_empty_ratio) * 0.1).clamp(0.0, 0.9)
            } else {
                0.12
            },
        ),
    ];

    for (kind, prior) in priors {
        let action = row_kind_name(kind);
        let bias = policy.row_kind_biases.get(family).map(|weights| weights.score(action)).unwrap_or(0.0);
        let memory = memory_bonus(
            policy.row_kind_memory.get(family),
            &row_memory_key(row_addr),
            action,
        );
        scores.push(DecisionScore {
            action: action.to_string(),
            score: prior + bias + memory,
        });
    }

    scores
}

fn score_table_kind(
    table: &TableInfo,
    family: &str,
    policy: &RecognitionPolicy,
    row_traces: &[RowRecognitionTrace],
    cell_traces: &[CellRecognitionTrace],
) -> ScoredSelection {
    let header_rows = row_traces.iter().filter(|row| row.selected_kind == RowKind::Header).count();
    let blank_rows = row_traces.iter().filter(|row| row.selected_kind == RowKind::BlankTemplate).count();
    let compound_cells = cell_traces.iter().filter(|cell| cell.selected_role == CellRole::CompoundHint).count();
    let label_value_pairs = count_horizontal_label_value_pairs(table);
    let total_non_empty = table.rows.iter()
        .flat_map(|row| &row.cells)
        .filter(|cell| !cell.text.trim().is_empty())
        .count();

    let priors = [
        (
            TableKind::HorizontalPairs,
            if label_value_pairs > 0 {
                (0.45 + label_value_pairs as f32 * 0.12).clamp(0.0, 0.95)
            } else {
                0.18
            },
        ),
        (
            TableKind::VerticalHeader,
            if header_rows > 0 {
                (0.55 + header_rows as f32 * 0.14).clamp(0.0, 0.95)
            } else {
                0.15
            },
        ),
        (
            TableKind::RowspanCompound,
            if compound_cells > 0 {
                (0.5 + compound_cells as f32 * 0.2).clamp(0.0, 0.95)
            } else {
                0.12
            },
        ),
        (
            TableKind::RepeatedHistory,
            if header_rows > 0 && blank_rows >= 1 && table.row_count >= 4 {
                (0.58 + blank_rows as f32 * 0.1).clamp(0.0, 0.95)
            } else {
                0.14
            },
        ),
        (
            TableKind::WrapperIgnore,
            if table.row_count <= 1 && table.col_count <= 1 && total_non_empty <= 1 {
                0.82
            } else {
                0.05
            },
        ),
        (TableKind::Mixed, 0.3),
    ];

    let mut scores = Vec::new();
    for (kind, prior) in priors {
        let action = table_kind_name(kind);
        let bias = policy.table_kind_biases.get(family).map(|weights| weights.score(action)).unwrap_or(0.0);
        let memory = memory_bonus(
            Some(&policy.table_kind_memory),
            family,
            action,
        );
        scores.push(DecisionScore {
            action: action.to_string(),
            score: prior + bias + memory,
        });
    }

    choose_scored_action(scores, table_kind_name(default_table_kind(table, row_traces, cell_traces)))
}

fn default_cell_role(table: &TableInfo, cell: &CellInfo) -> CellRole {
    if cell.text.trim().is_empty() {
        CellRole::Ignore
    } else if looks_like_inline_compound_target(table, cell) {
        CellRole::CompoundHint
    } else if is_korean_label(&cell.text) {
        CellRole::Label
    } else {
        CellRole::Value
    }
}

fn default_row_kind(
    table: &TableInfo,
    row_idx: usize,
    cell_traces: &[CellRecognitionTrace],
) -> RowKind {
    let Some(row) = table.rows.get(row_idx) else { return RowKind::Data; };
    let row_addr = row.cells.first().map(|cell| cell.row).unwrap_or(row_idx as u32);
    let has_any = row.cells.iter().any(|cell| !cell.text.trim().is_empty());
    if !has_any {
        return RowKind::BlankTemplate;
    }
    if is_vertical_header_row(table, row_idx) {
        return RowKind::Header;
    }

    let label_count = cell_traces.iter()
        .filter(|cell| cell.row == row_addr)
        .filter(|cell| matches!(cell.selected_role, CellRole::Label | CellRole::CompoundHint))
        .count();
    let non_empty = row.cells.iter().filter(|cell| !cell.text.trim().is_empty()).count();
    if non_empty > 0 && label_count * 4 >= non_empty * 3 && row.cells.len() <= 2 {
        RowKind::SectionBreak
    } else {
        RowKind::Data
    }
}

fn default_table_kind(
    table: &TableInfo,
    row_traces: &[RowRecognitionTrace],
    cell_traces: &[CellRecognitionTrace],
) -> TableKind {
    let header_rows = row_traces.iter().filter(|row| row.selected_kind == RowKind::Header).count();
    let compound_cells = cell_traces.iter().filter(|cell| cell.selected_role == CellRole::CompoundHint).count();
    if table.row_count <= 1 && table.col_count <= 1 {
        return TableKind::WrapperIgnore;
    }
    if header_rows > 0 && table.row_count >= 4 {
        return TableKind::RepeatedHistory;
    }
    if header_rows > 0 {
        return TableKind::VerticalHeader;
    }
    if compound_cells > 0 {
        return TableKind::RowspanCompound;
    }
    if count_horizontal_label_value_pairs(table) > 0 {
        return TableKind::HorizontalPairs;
    }
    TableKind::Mixed
}

fn count_horizontal_label_value_pairs(table: &TableInfo) -> usize {
    table.rows.iter().map(|row| {
        row.cells.windows(2)
            .filter(|pair| pair[0].is_label && !pair[1].is_label && !pair[1].text.trim().is_empty())
            .count()
    }).sum()
}

fn fingerprint_table(table: &TableInfo) -> TableFingerprint {
    let mut header_tokens = Vec::new();
    for row in table.rows.iter().take(3) {
        for cell in &row.cells {
            let token = normalize_text_token(&cell.text);
            if token.is_empty() {
                continue;
            }
            if header_tokens.iter().any(|existing| existing == &token) {
                continue;
            }
            header_tokens.push(token);
            if header_tokens.len() >= 6 {
                break;
            }
        }
        if header_tokens.len() >= 6 {
            break;
        }
    }

    let mut span_hist = HashMap::new();
    let mut border_hist = HashMap::new();
    let mut empty_row_pattern = String::new();
    for row in &table.rows {
        let mut row_non_empty = 0usize;
        for cell in &row.cells {
            *span_hist.entry(format!("c{}r{}", cell.col_span, cell.row_span)).or_insert(0usize) += 1;
            if !cell.border_fill_id_ref.is_empty() {
                *border_hist.entry(cell.border_fill_id_ref.clone()).or_insert(0usize) += 1;
            }
            if !cell.text.trim().is_empty() {
                row_non_empty += 1;
            }
        }
        empty_row_pattern.push(match row_non_empty {
            0 => 'E',
            n if n == row.cells.len() => 'T',
            _ => 'M',
        });
    }

    let span_histogram = top_histogram_entries(span_hist);
    let border_fill_histogram = top_histogram_entries(border_hist);
    let family = format!(
        "r{}-c{}-h{}-s{}-b{}-e{}",
        table.row_count,
        table.col_count,
        header_tokens.join("+"),
        span_histogram.join("+"),
        border_fill_histogram.join("+"),
        empty_row_pattern,
    );

    TableFingerprint {
        family,
        row_count: table.row_count,
        col_count: table.col_count,
        header_tokens,
        span_histogram,
        border_fill_histogram,
        empty_row_pattern,
    }
}

fn top_histogram_entries(map: HashMap<String, usize>) -> Vec<String> {
    let mut items: Vec<_> = map.into_iter().collect();
    items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    items.into_iter().take(4).map(|(key, count)| format!("{}:{}", key, count)).collect()
}

fn next_row_empty_ratio(table: &TableInfo, row_idx: usize) -> f32 {
    let Some(next) = table.rows.get(row_idx + 1) else { return 0.0; };
    let total = next.cells.len();
    if total == 0 {
        return 0.0;
    }
    let empty = next.cells.iter().filter(|cell| cell.text.trim().is_empty()).count();
    empty as f32 / total as f32
}

fn normalize_text_token(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .take(12)
        .collect()
}

fn digit_ratio(text: &str) -> f32 {
    let chars: Vec<_> = text.chars().collect();
    if chars.is_empty() {
        return 0.0;
    }
    let digits = chars.iter().filter(|ch| ch.is_ascii_digit()).count();
    digits as f32 / chars.len() as f32
}

fn label_signal_score(text: &str) -> f32 {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0.0;
    }
    let normalized: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    let char_count = normalized.chars().count();
    if char_count > 20 {
        return 0.0;
    }

    let mut score: f32 = 0.0;
    if keyword_match(&normalized, char_count) {
        score += 0.65;
    }
    if spaced_korean_pattern(trimmed) {
        score += 0.3;
    }
    if colon_suffix_pattern(&normalized) {
        score += 0.2;
    }
    if digit_ratio(trimmed) > 0.25 {
        score -= 0.25;
    }
    score.clamp(0.0, 1.0)
}

fn compound_signal_score(table: &TableInfo, cell: &CellInfo) -> f32 {
    if looks_like_inline_compound_target(table, cell) {
        return 0.88;
    }

    let normalized: String = cell.text.chars().filter(|c| !c.is_whitespace()).collect();
    if cell.col_span >= 2 && normalized.contains("전공") {
        0.6
    } else {
        0.05
    }
}

fn keyword_match(normalized: &str, char_count: usize) -> bool {
    korean_label_keywords().iter().any(|kw| normalized == *kw || (char_count <= 10 && normalized.contains(kw)))
}

fn spaced_korean_pattern(original_trimmed: &str) -> bool {
    let words: Vec<&str> = original_trimmed.split_whitespace().collect();
    if words.len() < 2 || words.len() > 4 {
        return false;
    }
    words.iter().all(|word| {
        word.chars().count() == 1 && word.chars().all(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c))
    }) && !original_trimmed.chars().any(|c| c.is_ascii_digit())
}

fn colon_suffix_pattern(normalized: &str) -> bool {
    (normalized.ends_with(':') || normalized.ends_with('：')) && normalized.chars().count() >= 3
}

fn memory_bonus(memory: Option<&HashMap<String, String>>, key: &str, action: &str) -> f32 {
    let Some(memory) = memory else { return 0.0; };
    match memory.get(key) {
        Some(saved) if saved == action => MEMORY_BOOST,
        Some(_) => -MEMORY_PENALTY,
        None => 0.0,
    }
}

fn row_memory_key(row: u32) -> String {
    format!("r{}", row)
}

fn cell_memory_key(row: u32, col: u32) -> String {
    format!("r{}c{}", row, col)
}

fn reward_for_outcome(outcome: Option<&str>) -> f32 {
    match outcome.unwrap_or_default() {
        "fill_success" => 1.0,
        "fill_failure" => -2.0,
        "manual_mapping" => -1.0,
        "structure_correction" => 2.0,
        _ => 0.5,
    }
}

fn cell_role_name(role: CellRole) -> &'static str {
    match role {
        CellRole::Label => "label",
        CellRole::Value => "value",
        CellRole::CompoundHint => "compound_hint",
        CellRole::Ignore => "ignore",
    }
}

fn parse_cell_role(value: &str) -> Option<CellRole> {
    match value {
        "label" => Some(CellRole::Label),
        "value" => Some(CellRole::Value),
        "compound_hint" => Some(CellRole::CompoundHint),
        "ignore" => Some(CellRole::Ignore),
        _ => None,
    }
}

fn row_kind_name(kind: RowKind) -> &'static str {
    match kind {
        RowKind::Header => "header",
        RowKind::Data => "data",
        RowKind::SectionBreak => "section_break",
        RowKind::BlankTemplate => "blank_template",
    }
}

fn parse_row_kind(value: &str) -> Option<RowKind> {
    match value {
        "header" => Some(RowKind::Header),
        "data" => Some(RowKind::Data),
        "section_break" => Some(RowKind::SectionBreak),
        "blank_template" => Some(RowKind::BlankTemplate),
        _ => None,
    }
}

fn table_kind_name(kind: TableKind) -> &'static str {
    match kind {
        TableKind::HorizontalPairs => "horizontal_pairs",
        TableKind::VerticalHeader => "vertical_header",
        TableKind::RowspanCompound => "rowspan_compound",
        TableKind::RepeatedHistory => "repeated_history",
        TableKind::WrapperIgnore => "wrapper_ignore",
        TableKind::Mixed => "mixed",
    }
}

fn parse_table_kind(value: &str) -> Option<TableKind> {
    match value {
        "horizontal_pairs" => Some(TableKind::HorizontalPairs),
        "vertical_header" => Some(TableKind::VerticalHeader),
        "rowspan_compound" => Some(TableKind::RowspanCompound),
        "repeated_history" => Some(TableKind::RepeatedHistory),
        "wrapper_ignore" => Some(TableKind::WrapperIgnore),
        "mixed" => Some(TableKind::Mixed),
        _ => None,
    }
}

struct ScoredSelection {
    action: String,
    confidence: f32,
    low_confidence: bool,
    scores: Vec<DecisionScore>,
}

fn choose_scored_action(mut scores: Vec<DecisionScore>, default_action: &str) -> ScoredSelection {
    if scores.is_empty() {
        return ScoredSelection {
            action: default_action.to_string(),
            confidence: 0.0,
            low_confidence: true,
            scores,
        };
    }

    scores.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.action.cmp(&b.action)));
    let top = scores.first().cloned().unwrap_or(DecisionScore { action: default_action.to_string(), score: 0.0 });
    let second = scores.get(1).cloned();
    let margin = second.as_ref().map(|next| top.score - next.score).unwrap_or(top.score);
    let low_confidence = margin < LOW_CONFIDENCE_MARGIN;
    let action = if low_confidence {
        default_action.to_string()
    } else {
        top.action.clone()
    };
    ScoredSelection {
        action,
        confidence: margin.max(0.0),
        low_confidence,
        scores,
    }
}

fn trace_header_rows(trace: &[TableRecognitionTrace], table_index: usize) -> HashSet<u32> {
    trace.iter()
        .find(|table| table.table_index == table_index)
        .map(|table| {
            table.rows.iter()
                .filter(|row| row.selected_kind == RowKind::Header)
                .map(|row| row.row)
                .collect()
        })
        .unwrap_or_default()
}

fn trace_compound_cells(trace: &[TableRecognitionTrace], table_index: usize) -> HashSet<(u32, u32)> {
    trace.iter()
        .find(|table| table.table_index == table_index)
        .map(|table| {
            table.cells.iter()
                .filter(|cell| cell.selected_role == CellRole::CompoundHint)
                .map(|cell| (cell.row, cell.col))
                .collect()
        })
        .unwrap_or_default()
}

fn trace_ignored_tables(trace: &[TableRecognitionTrace]) -> HashSet<usize> {
    trace.iter()
        .filter(|table| table.selected_table_kind == TableKind::WrapperIgnore)
        .map(|table| table.table_index)
        .collect()
}

/// 테이블에서 label→data 필드 매핑 추출
///
/// 두 가지 패턴을 감지:
/// 1. 가로 패턴: [Label] [Data] — 같은 행에서 라벨 옆에 데이터
/// 2. 세로 패턴: 헤더 행(전부 label/text) 아래에 데이터 행(전부 empty) — 컬럼별 필드
pub fn extract_fields(tables: &[TableInfo]) -> Vec<FieldInfo> {
    extract_fields_with_trace(tables, None)
}

pub fn extract_fields_with_trace(
    tables: &[TableInfo],
    trace: Option<&[TableRecognitionTrace]>,
) -> Vec<FieldInfo> {
    let mut fields = Vec::new();
    let ignored_tables = trace.map(trace_ignored_tables).unwrap_or_default();

    for table in tables {
        if ignored_tables.contains(&table.index) {
            continue;
        }

        let vertical_header_rows: HashSet<u32> = match trace {
            Some(trace) => trace_header_rows(trace, table.index),
            None => table.rows.iter()
                .enumerate()
                .filter_map(|(row_idx, row)| {
                    is_vertical_header_row(table, row_idx)
                        .then_some(row.cells.first().map(|cell| cell.row).unwrap_or(row_idx as u32))
                })
                .collect(),
        };
        let compound_cells = trace
            .map(|trace| trace_compound_cells(trace, table.index))
            .unwrap_or_default();

        // Pass 1: 가로 패턴 (기존)
        for row in &table.rows {
            let row_addr = row.cells.first().map(|cell| cell.row).unwrap_or(0);
            if vertical_header_rows.contains(&row_addr) { continue; }
            for (i, cell) in row.cells.iter().enumerate() {
                if !cell.is_label { continue; }
                if let Some(data_cell) = row.cells.get(i + 1) {
                    if !data_cell.is_label {
                        let key = infer_canonical_key(&cell.text);
                        fields.push(FieldInfo {
                            table_index: table.index,
                            row: data_cell.row,
                            col: data_cell.col,
                            label: cell.text.clone(),
                            canonical_key: key.to_string(),
                            confidence: if key != "unknown" { 0.95 } else { 0.5 },
                            content_type: ContentType::Unknown,
                        });
                    }
                }
            }
        }

        // Pass 1.5: rowspan 라벨 + 오른쪽 복합 셀 패턴
        // 예: [학력](rowspan=2) [전공], 다음 행 [대학원 전공]
        // 이런 셀은 비어 있거나 라벨 조각만 있어도 실제로는 fill target인 경우가 많다.
        let mut already_mapped: std::collections::HashSet<(u32, u32)> = fields.iter()
            .filter(|f| f.table_index == table.index)
            .map(|f| (f.row, f.col))
            .collect();

        for row in &table.rows {
            for cell in &row.cells {
                if already_mapped.contains(&(cell.row, cell.col)) { continue; }
                let explicit_compound = compound_cells.contains(&(cell.row, cell.col));
                if !explicit_compound && !looks_like_inline_compound_target(table, cell) { continue; }

                let Some(anchor_label) = find_covering_left_label(table, cell) else { continue; };
                let label = derive_inline_field_label(anchor_label, cell);
                let key = infer_canonical_key(&label);

                fields.push(FieldInfo {
                    table_index: table.index,
                    row: cell.row,
                    col: cell.col,
                    label,
                    canonical_key: key.to_string(),
                    confidence: if key != "unknown" { 0.72 } else { 0.45 },
                    content_type: ContentType::Unknown,
                });
                already_mapped.insert((cell.row, cell.col));
            }
        }

        // Pass 2: 세로 패턴 — 헤더 행 감지 + 아래 데이터 행들
        // 헤더 행 조건: 모든 셀이 텍스트 있음 + 바로 아래 행의 셀이 대부분 비어있음
        for row in &table.rows {
            if row.cells.is_empty() { continue; }
            if row.cells.len() < 2 { continue; } // 최소 2열 이상
            let row_addr = row.cells.first().map(|cell| cell.row).unwrap_or(0);
            if !vertical_header_rows.contains(&row_addr) { continue; }

            // 이 행 = 세로 테이블 헤더! 아래 데이터 행들에 대해 컬럼별 필드 생성
            let header_cells: Vec<&CellInfo> = row.cells.iter().collect();

            // 가로 패턴에서 이미 잡힌 필드의 위치를 제외
            let already_mapped: std::collections::HashSet<(u32, u32)> = fields.iter()
                .map(|f| (f.row, f.col))
                .collect();

            let start_idx = table.rows.iter()
                .position(|candidate| candidate.cells.first().map(|cell| cell.row) == Some(row_addr))
                .map(|idx| idx + 1)
                .unwrap_or(0);

            for data_row in table.rows.iter().skip(start_idx) {
                // 데이터 행이 아닌 다른 헤더 행이 나오면 중단
                let has_data = data_row.cells.iter().any(|c| !c.text.trim().is_empty());
                let mostly_labels = data_row.cells.iter()
                    .filter(|c| !c.text.trim().is_empty())
                    .all(|c| c.is_label);
                let data_row_addr = data_row.cells.first().map(|cell| cell.row).unwrap_or(0);
                if data_row_addr != row_addr && vertical_header_rows.contains(&data_row_addr) {
                    break;
                }
                if has_data && mostly_labels && !compound_cells.iter().any(|(row, _)| *row == data_row_addr) {
                    break; // 다음 섹션 헤더
                }

                for data_cell in &data_row.cells {
                    if already_mapped.contains(&(data_cell.row, data_cell.col)) { continue; }

                    // 같은 col을 가진 헤더 셀 찾기
                    if let Some(header_cell) = header_cells.iter()
                        .find(|h| h.col == data_cell.col)
                    {
                        let key = infer_canonical_key(&header_cell.text);
                        fields.push(FieldInfo {
                            table_index: table.index,
                            row: data_cell.row,
                            col: data_cell.col,
                            label: header_cell.text.clone(),
                            canonical_key: key.to_string(),
                            confidence: if key != "unknown" { 0.8 } else { 0.4 },
                            content_type: ContentType::Unknown,
                        });
                    }
                }
            }
        }
    }

    fields
}

fn is_vertical_header_row(table: &TableInfo, row_idx: usize) -> bool {
    let Some(row) = table.rows.get(row_idx) else { return false; };
    if row.cells.len() < 2 { return false; }

    let all_have_text = row.cells.iter().all(|c| !c.text.trim().is_empty());
    if !all_have_text { return false; }

    let Some(next) = table.rows.get(row_idx + 1) else { return false; };
    let total = next.cells.len();
    if total == 0 { return false; }

    let empty_count = next.cells.iter().filter(|c| c.text.trim().is_empty()).count();
    empty_count as f32 / total as f32 >= 0.7
}

fn looks_like_inline_compound_target(table: &TableInfo, cell: &CellInfo) -> bool {
    if cell.text.trim().is_empty() { return false; }
    if cell.col_span < 2 { return false; }
    if !cell.is_label { return false; }

    let normalized: String = cell.text.chars().filter(|c| !c.is_whitespace()).collect();
    if normalized.chars().count() > 20 { return false; }

    let Some(anchor) = find_covering_left_label(table, cell) else { return false; };
    if anchor.col >= cell.col { return false; }
    if anchor.row_span < 2 { return false; }

    // 라벨만 있는 넓은 셀 전체를 다 target으로 만들면 오탐이 많다.
    // "전공", "대학원 전공"처럼 복합 입력 힌트 역할을 하는 짧은 조각만 허용한다.
    normalized == "전공"
        || normalized.contains("대학원")
        || normalized.contains("석사")
        || normalized.contains("박사")
        || normalized.contains("학위")
}

fn find_covering_left_label<'a>(table: &'a TableInfo, cell: &CellInfo) -> Option<&'a CellInfo> {
    table.rows.iter()
        .flat_map(|r| &r.cells)
        .filter(|candidate| {
            candidate.is_label
                && candidate.row <= cell.row
                && cell.row < candidate.row + candidate.row_span
                && candidate.col + candidate.col_span <= cell.col
        })
        .max_by_key(|candidate| (candidate.row, candidate.col + candidate.col_span))
}

fn derive_inline_field_label(anchor: &CellInfo, cell: &CellInfo) -> String {
    let anchor_label = anchor.text.trim();
    let inline = cell.text.split_whitespace().collect::<Vec<_>>().join(" ");
    let inline_norm: String = inline.chars().filter(|c| !c.is_whitespace()).collect();

    if inline_norm == "전공" {
        return anchor_label.to_string();
    }

    if inline_norm.contains("대학원") {
        return inline;
    }

    if inline.is_empty() {
        return anchor_label.to_string();
    }

    format!("{} {}", anchor_label, inline)
}

/// serde enrichment — 파싱 성공 시 각 필드의 content_type을 갱신
///
/// streaming 분석은 텍스트만 보지만, serde는 셀 안의 Picture, Equation,
/// FormControl 등을 타입으로 구분할 수 있다.
/// serde 파싱이 실패하면 이 함수를 호출하지 않으면 됨 — content_type은 Unknown으로 남음.
pub fn enrich_with_serde(fields: &mut [FieldInfo], xml: &str) {
    let section = match crate::parser::parse_section(xml) {
        Ok(s) => s,
        Err(_) => return, // serde 실패 → enrichment 없이 진행
    };

    // analyze_xml()와 동일한 leaf-table 순서를 사용해야 table_index가 맞는다.
    let serde_tables = collect_leaf_root_tables(&section);

    for field in fields.iter_mut() {
        if field.table_index >= serde_tables.len() {
            continue;
        }

        let table = serde_tables[field.table_index];
        if let Some(cell) = table.get_cell(field.row, field.col) {
            field.content_type = classify_cell_content(cell);
        }
    }
}

/// serde TableCell의 RunContent를 분석해서 ContentType 결정
fn classify_cell_content(cell: &crate::model::TableCell) -> ContentType {
    use crate::model::RunContent;

    let mut has_text = false;
    let mut has_other = false;
    let mut specific_type: Option<ContentType> = None;

    for para in &cell.sub_list.paragraphs {
        for run in &para.runs {
            for content in &run.contents {
                match content {
                    RunContent::Text(_) => has_text = true,
                    RunContent::Picture(_) => {
                        has_other = true;
                        specific_type = Some(ContentType::HasPicture);
                    }
                    RunContent::Equation(_) => {
                        has_other = true;
                        specific_type = Some(ContentType::HasEquation);
                    }
                    RunContent::Button(_) | RunContent::RadioButton(_) |
                    RunContent::CheckButton(_) | RunContent::ComboBox(_) |
                    RunContent::ListBox(_) | RunContent::Edit(_) |
                    RunContent::ScrollBar(_) => {
                        has_other = true;
                        specific_type = Some(ContentType::HasFormControl);
                    }
                    RunContent::Line(_) | RunContent::Rectangle(_) |
                    RunContent::Ellipse(_) | RunContent::Arc(_) |
                    RunContent::Polygon(_) | RunContent::Curve(_) |
                    RunContent::ConnectLine(_) => {
                        has_other = true;
                        specific_type = Some(ContentType::HasDrawing);
                    }
                    // Table, SectionDef, TextArt, Ole 등은 무시
                    _ => {}
                }
            }
        }
    }

    if has_text && has_other {
        ContentType::Mixed
    } else if has_other {
        specific_type.unwrap_or(ContentType::Unknown)
    } else {
        ContentType::TextOnly
    }
}

// ── 헬퍼 ──

fn attr_u32(e: &quick_xml::events::BytesStart, key: &[u8]) -> u32 {
    e.attributes()
        .filter_map(|a| a.ok())
        .find(|a| a.key.as_ref() == key)
        .and_then(|a| std::str::from_utf8(&a.value).ok()?.parse().ok())
        .unwrap_or(0)
}

fn attr_u32_default(e: &quick_xml::events::BytesStart, key: &[u8], default: u32) -> u32 {
    e.attributes()
        .filter_map(|a| a.ok())
        .find(|a| a.key.as_ref() == key)
        .and_then(|a| std::str::from_utf8(&a.value).ok()?.parse().ok())
        .unwrap_or(default)
}

fn attr_str(e: &quick_xml::events::BytesStart, key: &[u8]) -> String {
    e.attributes()
        .filter_map(|a| a.ok())
        .find(|a| a.key.as_ref() == key)
        .map(|a| String::from_utf8_lossy(&a.value).to_string())
        .unwrap_or_default()
}

/// 라벨 셀 판별 — kordoc의 3단계 휴리스틱 채택
///
/// 1. 키워드 매칭 (한국 공문서 필드명)
/// 2. 짧은 한글 텍스트 (2-8자, 숫자 없음, 괄호/콜론 허용)
/// 3. "라벨:" 또는 "라벨 :" 패턴
///
/// 참고: kordoc (https://github.com/chrisryugj/kordoc) src/form/recognize.ts
fn is_korean_label(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() { return false; }

    // 공백 제거 버전 (키워드 매칭용)
    let normalized: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    let char_count = normalized.chars().count();

    // 너무 긴 텍스트는 라벨이 아님 (데이터 설명이나 본문)
    if char_count > 20 { return false; }

    // 1. 키워드 매칭 — 한국 공문서에서 흔한 필드명
    // "전공" 같은 짧은 키워드가 긴 데이터 텍스트에 포함될 수 있으므로
    // 지나치게 긴 문자열에는 contains 매칭을 적용하지 않는다.
    if korean_label_keywords().iter().any(|kw| {
        normalized == *kw || (char_count <= 10 && normalized.contains(kw))
    }) {
        return true;
    }

    // 2. 짧은 한글 텍스트 — 보수적 적용
    // 원본 텍스트(공백 포함)에서 판단해서 "성 명" (공백 있는 라벨) 감지
    // 단, 사람 이름(2-3자 한글)이나 직위("사업총괄") 같은 데이터와 구분하기 위해:
    //   - 공백으로 분리된 한자어 패턴 ("성 명", "직 책", "학 력") → label
    //   - 순수 한글 2-3자 ("해민영", "팀장") → 이름/직위일 수 있으므로 비허용
    //   - 4자 이상 순수 한글 ("사업총괄", "벤처전문위원") → 데이터일 수 있으므로 비허용
    // 결론: 규칙 2는 "공백으로 분리된 한자어 라벨"에만 적용
    let original_trimmed = text.trim();
    let words: Vec<&str> = original_trimmed.split_whitespace().collect();
    if words.len() >= 2 && words.len() <= 4 {
        // "성 명", "직 책", "학 력", "비 고"처럼 각 음절이 띄어진 패턴만 허용.
        // "수석 컨설턴트", "사업 관리 및 멘토링" 같은 정상 데이터는 제외해야 한다.
        let all_short_korean = words.iter().all(|w| {
            let wlen = w.chars().count();
            wlen == 1 && w.chars().all(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c))
        });
        let no_digits = !original_trimmed.chars().any(|c| c.is_ascii_digit());
        if all_short_korean && no_digits {
            return true;
        }
    }

    // 3. "라벨:" 또는 "라벨 :" 패턴
    if normalized.ends_with(':') || normalized.ends_with('：') {
        let without_colon: String = normalized.chars().take(char_count - 1).collect();
        if without_colon.chars().count() >= 2 {
            return true;
        }
    }

    false
}

fn korean_label_keywords() -> &'static [&'static str] {
    &[
        // 인적사항
        "성명", "이름", "소속", "직책", "직위", "직급", "부서",
        "생년", "생년월일", "주민등록번호", "연령",
        "이메일", "E-mail", "전화", "휴대전화", "연락처", "팩스",
        "주소", "학력", "전공", "학교",
        // 사업/경력
        "경력", "유사경력", "자격증", "근무경력",
        "참여임무", "참여기간", "사업참여기간", "참여율",
        "회사명", "근무기간", "담당업무", "비고", "발주처",
        "상세경력", "사업명", "사업개요",
        "투입기간", "기술분야", "관련기술", "담당임무", "전문분야",
        // 이름/언어 구분
        "국문", "영문",
        // 인적사항 추가
        "성별", "사업유형", "학위", "훈격",
        // 공문서 일반
        "신청인", "대표자", "담당자", "작성자", "확인자", "승인자",
        "일시", "날짜", "기간", "장소", "목적", "사유",
        "금액", "수량", "단가", "합계",
    ]
}

fn infer_canonical_key(label: &str) -> &'static str {
    let t: String = label.chars().filter(|c| !c.is_whitespace()).collect();
    let map: &[(&[&str], &str)] = &[
        (&["성명", "이름"], "name"),
        (&["E-mail", "이메일"], "email"),
        (&["직책", "직위", "직급"], "position"),
        (&["생년"], "birth_date"),
        (&["휴대전화", "전화", "연락처"], "phone"),
        (&["유사경력", "경력"], "experience"),
        (&["자격증"], "certification"),
        (&["참여임무"], "task"),
        (&["사업참여기간", "참여기간"], "period"),
        (&["참여율"], "participation_rate"),
        (&["회사명", "소속"], "company"),
        (&["근무기간"], "work_period"),
        (&["담당업무"], "duties"),
        (&["비고"], "notes"),
        (&["발주처"], "client"),
        (&["학력"], "education"),
        (&["전공"], "major"),
        (&["주소"], "address"),
    ];
    for (patterns, key) in map {
        if patterns.iter().any(|p| t.contains(p)) { return key; }
    }
    "unknown"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nested_table_xml() -> &'static str {
        r#"
<sec>
  <p>
    <run>
      <tbl rowCnt="1" colCnt="1">
        <tr>
          <tc borderFillIDRef="1">
            <subList>
              <p>
                <run>
                  <tbl rowCnt="2" colCnt="4">
                    <tr>
                      <tc borderFillIDRef="6">
                        <subList><p><run><t>성 명</t></run></p></subList>
                        <cellAddr colAddr="0" rowAddr="0"/>
                        <cellSpan/>
                        <cellSz width="100" height="100"/>
                      </tc>
                      <tc borderFillIDRef="6">
                        <subList><p><run><t>김영우</t></run></p></subList>
                        <cellAddr colAddr="1" rowAddr="0"/>
                        <cellSpan/>
                        <cellSz width="100" height="100"/>
                      </tc>
                      <tc borderFillIDRef="6">
                        <subList><p><run><t>직 책</t></run></p></subList>
                        <cellAddr colAddr="2" rowAddr="0"/>
                        <cellSpan/>
                        <cellSz width="100" height="100"/>
                      </tc>
                      <tc borderFillIDRef="6">
                        <subList><p><run><t>수석 컨설턴트</t></run></p></subList>
                        <cellAddr colAddr="3" rowAddr="0"/>
                        <cellSpan/>
                        <cellSz width="100" height="100"/>
                      </tc>
                    </tr>
                    <tr>
                      <tc borderFillIDRef="6">
                        <subList><p><run><t>소속 회사</t></run></p></subList>
                        <cellAddr colAddr="0" rowAddr="1"/>
                        <cellSpan/>
                        <cellSz width="100" height="100"/>
                      </tc>
                      <tc borderFillIDRef="6">
                        <subList><p><run><t>엠와이소셜컴퍼니</t></run></p></subList>
                        <cellAddr colAddr="1" rowAddr="1"/>
                        <cellSpan colSpan="3" rowSpan="1"/>
                        <cellSz width="300" height="100"/>
                      </tc>
                    </tr>
                  </tbl>
                </run>
              </p>
            </subList>
            <cellAddr colAddr="0" rowAddr="0"/>
            <cellSpan/>
            <cellSz width="400" height="200"/>
          </tc>
        </tr>
      </tbl>
    </run>
  </p>
</sec>
        "#
    }

    fn inline_rowspan_label_xml() -> &'static str {
        r#"
<sec>
  <p>
    <run>
      <tbl rowCnt="3" colCnt="6">
        <tr>
          <tc borderFillIDRef="6">
            <subList><p><run><t>학 력</t></run></p></subList>
            <cellAddr colAddr="0" rowAddr="0"/>
            <cellSpan colSpan="1" rowSpan="2"/>
            <cellSz width="100" height="100"/>
          </tc>
          <tc borderFillIDRef="6">
            <subList><p><run><t>전공</t></run></p></subList>
            <cellAddr colAddr="1" rowAddr="0"/>
            <cellSpan colSpan="3" rowSpan="1"/>
            <cellSz width="300" height="100"/>
          </tc>
          <tc borderFillIDRef="6">
            <subList><p><run><t>보 유자격증</t></run></p></subList>
            <cellAddr colAddr="4" rowAddr="0"/>
            <cellSpan colSpan="1" rowSpan="2"/>
            <cellSz width="100" height="100"/>
          </tc>
          <tc borderFillIDRef="6">
            <subList><p><run><t></t></run></p></subList>
            <cellAddr colAddr="5" rowAddr="0"/>
            <cellSpan colSpan="1" rowSpan="2"/>
            <cellSz width="100" height="100"/>
          </tc>
        </tr>
        <tr>
          <tc borderFillIDRef="6">
            <subList><p><run><t>대학원 전공</t></run></p></subList>
            <cellAddr colAddr="1" rowAddr="1"/>
            <cellSpan colSpan="3" rowSpan="1"/>
            <cellSz width="300" height="100"/>
          </tc>
        </tr>
        <tr>
          <tc borderFillIDRef="6">
            <subList><p><run><t>참여율</t></run></p></subList>
            <cellAddr colAddr="0" rowAddr="2"/>
            <cellSpan colSpan="1" rowSpan="1"/>
            <cellSz width="100" height="100"/>
          </tc>
          <tc borderFillIDRef="6">
            <subList><p><run><t>0%</t></run></p></subList>
            <cellAddr colAddr="1" rowAddr="2"/>
            <cellSpan colSpan="1" rowSpan="1"/>
            <cellSz width="100" height="100"/>
          </tc>
        </tr>
      </tbl>
    </run>
  </p>
</sec>
        "#
    }

    fn vertical_header_table_xml() -> &'static str {
        r#"
<sec>
  <tbl rowCnt="3" colCnt="3">
    <tr>
      <tc borderFillIDRef="7">
        <subList><p><run><t>사 업 명</t></run></p></subList>
        <cellAddr colAddr="0" rowAddr="0"/>
        <cellSpan colSpan="1" rowSpan="1"/>
      </tc>
      <tc borderFillIDRef="7">
        <subList><p><run><t>참여기간</t></run></p><p><run><t>( 년 월～ 년 월)</t></run></p></subList>
        <cellAddr colAddr="1" rowAddr="0"/>
        <cellSpan colSpan="1" rowSpan="1"/>
      </tc>
      <tc borderFillIDRef="7">
        <subList><p><run><t>담당업무</t></run></p></subList>
        <cellAddr colAddr="2" rowAddr="0"/>
        <cellSpan colSpan="1" rowSpan="1"/>
      </tc>
    </tr>
    <tr>
      <tc borderFillIDRef="7"><subList><p><run><t></t></run></p></subList><cellAddr colAddr="0" rowAddr="1"/><cellSpan colSpan="1" rowSpan="1"/></tc>
      <tc borderFillIDRef="7"><subList><p><run><t></t></run></p></subList><cellAddr colAddr="1" rowAddr="1"/><cellSpan colSpan="1" rowSpan="1"/></tc>
      <tc borderFillIDRef="7"><subList><p><run><t></t></run></p></subList><cellAddr colAddr="2" rowAddr="1"/><cellSpan colSpan="1" rowSpan="1"/></tc>
    </tr>
    <tr>
      <tc borderFillIDRef="7"><subList><p><run><t></t></run></p></subList><cellAddr colAddr="0" rowAddr="2"/><cellSpan colSpan="1" rowSpan="1"/></tc>
      <tc borderFillIDRef="7"><subList><p><run><t></t></run></p></subList><cellAddr colAddr="1" rowAddr="2"/><cellSpan colSpan="1" rowSpan="1"/></tc>
      <tc borderFillIDRef="7"><subList><p><run><t></t></run></p></subList><cellAddr colAddr="2" rowAddr="2"/><cellSpan colSpan="1" rowSpan="1"/></tc>
    </tr>
  </tbl>
</sec>
        "#
    }

    #[test]
    fn analyze_xml_prefers_leaf_nested_tables() {
        let tables = analyze_xml(nested_table_xml());
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].row_count, 2);
        assert_eq!(tables[0].col_count, 4);
        assert_eq!(tables[0].rows[0].cells[0].text, "성 명");
        assert_eq!(tables[0].rows[0].cells[1].text, "김영우");
    }

    #[test]
    fn extract_fields_from_nested_leaf_table() {
        let tables = analyze_xml(nested_table_xml());
        let fields = extract_fields(&tables);
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].label, "성 명");
        assert_eq!(fields[0].canonical_key, "name");
        assert_eq!(fields[1].label, "직 책");
        assert_eq!(fields[1].canonical_key, "position");
        assert_eq!(fields[2].label, "소속 회사");
        assert_eq!(fields[2].canonical_key, "company");
    }

    #[test]
    fn extract_data_from_nested_leaf_table() {
        let fields = crate::extractor::extract_data(nested_table_xml());
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].key, "name");
        assert_eq!(fields[0].value, "김영우");
        assert_eq!(fields[1].key, "position");
        assert_eq!(fields[1].value, "수석 컨설턴트");
        assert_eq!(fields[2].key, "company");
        assert_eq!(fields[2].value, "엠와이소셜컴퍼니");
    }

    #[test]
    fn extract_fields_detects_inline_rowspan_targets() {
        let tables = analyze_xml(inline_rowspan_label_xml());
        let fields = extract_fields(&tables);

        assert!(fields.iter().any(|f| f.row == 0 && f.col == 1 && f.label == "학 력"));
        assert!(fields.iter().any(|f| f.row == 1 && f.col == 1 && f.label == "대학원 전공"));
        assert!(fields.iter().any(|f| f.row == 2 && f.col == 1 && f.label == "참여율"));
    }

    #[test]
    fn extract_fields_skips_horizontal_pass_for_vertical_header_rows() {
        let tables = analyze_xml(vertical_header_table_xml());
        let fields = extract_fields(&tables);

        assert!(!fields.iter().any(|f| f.row == 0));
        assert!(fields.iter().any(|f| f.row == 1 && f.col == 0 && f.label == "사 업 명"));
        assert!(fields.iter().any(|f| f.row == 1 && f.col == 1 && f.label.contains("참여기간")));
        assert!(fields.iter().any(|f| f.row == 1 && f.col == 2 && f.label == "담당업무"));
    }

    #[test]
    fn adaptive_policy_can_override_header_row_for_same_table_family() {
        let baseline = analyze_form_adaptive(vertical_header_table_xml(), None);
        assert!(baseline.fields.iter().any(|field| field.row == 1 && field.col == 0));
        let trace = &baseline.trace[0];

        let feedback = StructureFeedback {
            fingerprint: trace.fingerprint.clone(),
            table_index: Some(trace.table_index),
            table_kind: Some(TableKind::HorizontalPairs),
            predicted_table_kind: Some(TableKind::VerticalHeader),
            row_kinds: vec![RowKindFeedback {
                row: 0,
                kind: RowKind::Data,
                predicted_kind: Some(RowKind::Header),
            }],
            cell_roles: Vec::new(),
            reward: Some(2.0),
            outcome: Some("structure_correction".to_string()),
        };
        let policy = update_policy_with_feedback(&RecognitionPolicy::default(), &[feedback]);
        let adapted = analyze_form_adaptive(vertical_header_table_xml(), Some(&policy));

        assert!(adapted.fields.iter().all(|field| field.row != 1));
        assert_eq!(
            policy.row_kind_memory
                .get(&trace.fingerprint.family)
                .and_then(|rows| rows.get("r0"))
                .map(String::as_str),
            Some("data")
        );
    }
}
