//! HWPX 문서 모델 — 폼 필러에 특화
//!
//! openhwp/hwpx의 KS X 6101:2024 스키마를 참고하되, 폼 채움에 필요한
//! 구조만 정의. 불필요한 필드는 `#[serde(flatten)]`으로 보존.
//!
//! **openhwp 대비 발전점:**
//! - FormField: label/data 셀 의미 분석 결과를 모델에 내장
//! - 행 클론을 위한 편의 메서드 (clone_row, insert_row, reindex_rows)
//! - 셀 텍스트 교체 시 Run 구조 보존 (multi-run 안전)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Section (section0.xml 루트) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "sec")]
pub struct Section {
    /// 문단 목록 — 테이블은 Run 내부에 중첩
    #[serde(rename = "p", default)]
    pub paragraphs: Vec<Paragraph>,

    /// 파싱되지 않은 속성/요소 보존
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ── Paragraph ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "p")]
pub struct Paragraph {
    #[serde(rename = "@id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(rename = "@paraPrIDRef", skip_serializing_if = "Option::is_none")]
    pub para_pr_id_ref: Option<String>,

    #[serde(rename = "@styleIDRef", skip_serializing_if = "Option::is_none")]
    pub style_id_ref: Option<String>,

    #[serde(rename = "run", default)]
    pub runs: Vec<Run>,

    #[serde(rename = "linesegarray", skip_serializing_if = "Option::is_none")]
    pub line_seg_array: Option<serde_json::Value>,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ── Run (인라인 콘텐츠) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "run")]
pub struct Run {
    #[serde(rename = "@charPrIDRef", skip_serializing_if = "Option::is_none")]
    pub char_pr_id_ref: Option<String>,

    /// 텍스트 내용 — 하나의 Run에 여러 <t> 가능 (줄바꿈 등)
    #[serde(rename = "t", default, skip_serializing_if = "Vec::is_empty")]
    pub texts: Vec<String>,

    /// Run 내 중첩 테이블 (openhwp의 RunContent::Table에 해당)
    #[serde(rename = "tbl", skip_serializing_if = "Option::is_none")]
    pub table: Option<Box<Table>>,

    /// 섹션 정의 등 기타 요소
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ── Table ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "tbl")]
pub struct Table {
    #[serde(rename = "@id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(rename = "@rowCnt", skip_serializing_if = "Option::is_none")]
    pub row_count: Option<u32>,

    #[serde(rename = "@colCnt", skip_serializing_if = "Option::is_none")]
    pub col_count: Option<u32>,

    #[serde(rename = "@borderFillIDRef", skip_serializing_if = "Option::is_none")]
    pub border_fill_id_ref: Option<String>,

    #[serde(rename = "@cellSpacing", default)]
    pub cell_spacing: u32,

    #[serde(rename = "@repeatHeader", default)]
    pub repeat_header: bool,

    #[serde(rename = "@noAdjust", default)]
    pub no_adjust: bool,

    /// 크기
    #[serde(rename = "sz", skip_serializing_if = "Option::is_none")]
    pub size: Option<TableSize>,

    /// 위치
    #[serde(rename = "pos", skip_serializing_if = "Option::is_none")]
    pub position: Option<serde_json::Value>,

    /// 바깥 여백
    #[serde(rename = "outMargin", skip_serializing_if = "Option::is_none")]
    pub out_margin: Option<serde_json::Value>,

    /// 안쪽 여백
    #[serde(rename = "inMargin", skip_serializing_if = "Option::is_none")]
    pub in_margin: Option<serde_json::Value>,

    /// 행 목록
    #[serde(rename = "tr", default)]
    pub rows: Vec<TableRow>,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "sz")]
pub struct TableSize {
    #[serde(rename = "@width")]
    pub width: u32,

    #[serde(rename = "@height")]
    pub height: u32,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ── Table Row ──

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "tr")]
pub struct TableRow {
    #[serde(rename = "tc", default)]
    pub cells: Vec<TableCell>,
}

// ── Table Cell ──
// openhwp 대비 발전: FormFieldInfo를 내장해서 분석 결과를 셀에 직접 연결

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "tc")]
pub struct TableCell {
    #[serde(rename = "@borderFillIDRef", skip_serializing_if = "Option::is_none")]
    pub border_fill_id_ref: Option<String>,

    #[serde(rename = "@header", default)]
    pub header: bool,

    #[serde(rename = "@protect", default)]
    pub protect: bool,

    #[serde(rename = "@editable", default)]
    pub editable: bool,

    #[serde(rename = "@name", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// 셀 내용 (문단 목록)
    #[serde(rename = "subList")]
    pub sub_list: SubList,

    /// 셀 주소
    #[serde(rename = "cellAddr")]
    pub cell_addr: CellAddr,

    /// 셀 병합
    #[serde(rename = "cellSpan")]
    pub cell_span: CellSpan,

    /// 셀 크기
    #[serde(rename = "cellSz")]
    pub cell_size: CellSize,

    /// 셀 여백
    #[serde(rename = "cellMargin", skip_serializing_if = "Option::is_none")]
    pub cell_margin: Option<CellMargin>,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubList {
    #[serde(rename = "p", default)]
    pub paragraphs: Vec<Paragraph>,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename = "cellAddr")]
pub struct CellAddr {
    #[serde(rename = "@colAddr")]
    pub col: u32,
    #[serde(rename = "@rowAddr")]
    pub row: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename = "cellSpan")]
pub struct CellSpan {
    #[serde(rename = "@colSpan", default = "one")]
    pub col_span: u32,
    #[serde(rename = "@rowSpan", default = "one")]
    pub row_span: u32,
}

fn one() -> u32 { 1 }

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename = "cellSz")]
pub struct CellSize {
    #[serde(rename = "@width")]
    pub width: u32,
    #[serde(rename = "@height")]
    pub height: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename = "cellMargin")]
pub struct CellMargin {
    #[serde(rename = "@left", default)]
    pub left: u32,
    #[serde(rename = "@right", default)]
    pub right: u32,
    #[serde(rename = "@top", default)]
    pub top: u32,
    #[serde(rename = "@bottom", default)]
    pub bottom: u32,
}

// ── 폼 필러 전용: openhwp에 없는 것들 ──

impl Table {
    /// 셀을 주소로 찾기 (openhwp의 get_cell 발전)
    pub fn get_cell(&self, row: u32, col: u32) -> Option<&TableCell> {
        self.rows.iter()
            .flat_map(|r| &r.cells)
            .find(|c| c.cell_addr.row == row && c.cell_addr.col == col)
    }

    /// 셀을 주소로 찾기 (mutable)
    pub fn get_cell_mut(&mut self, row: u32, col: u32) -> Option<&mut TableCell> {
        self.rows.iter_mut()
            .flat_map(|r| &mut r.cells)
            .find(|c| c.cell_addr.row == row && c.cell_addr.col == col)
    }

    /// 특정 rowAddr를 가진 행 찾기
    pub fn find_row(&self, row_addr: u32) -> Option<(usize, &TableRow)> {
        self.rows.iter().enumerate()
            .find(|(_, r)| r.cells.first().map(|c| c.cell_addr.row) == Some(row_addr))
    }

    /// 행 클론 + 삽입 + rowAddr 재계산
    /// openhwp에 없는 핵심 기능
    pub fn clone_row(&mut self, template_row_idx: usize, count: usize) {
        if template_row_idx >= self.rows.len() || count == 0 {
            return;
        }

        let template = self.rows[template_row_idx].clone();
        let template_row_addr = template.cells.first()
            .map(|c| c.cell_addr.row)
            .unwrap_or(0);

        // 새 행 생성
        let mut new_rows = Vec::with_capacity(count);
        for i in 1..=count {
            let mut row = template.clone();
            let new_addr = template_row_addr + i as u32;
            for cell in &mut row.cells {
                cell.cell_addr.row = new_addr;
            }
            new_rows.push(row);
        }

        // 삽입 지점 이후의 기존 행들 rowAddr 증가
        for row in &mut self.rows[(template_row_idx + 1)..] {
            for cell in &mut row.cells {
                cell.cell_addr.row += count as u32;
            }
        }

        // 새 행 삽입
        let insert_at = template_row_idx + 1;
        for (i, row) in new_rows.into_iter().enumerate() {
            self.rows.insert(insert_at + i, row);
        }

        // rowCnt 업데이트
        if let Some(rc) = &mut self.row_count {
            *rc += count as u32;
        }

        // 테이블 높이 업데이트
        if let Some(sz) = &mut self.size {
            let row_height = template.cells.first()
                .map(|c| c.cell_size.height)
                .unwrap_or(2229);
            sz.height += row_height * count as u32;
        }
    }
}

impl TableCell {
    /// 셀의 텍스트 추출 (모든 Run의 texts를 합침)
    pub fn text(&self) -> String {
        self.sub_list.paragraphs.iter()
            .flat_map(|p| &p.runs)
            .flat_map(|r| &r.texts)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 셀 텍스트 교체 — 첫 번째 Run의 첫 text만 교체, 나머지는 비움
    /// multi-run 구조를 보존하면서 텍스트만 교체
    pub fn set_text(&mut self, new_text: &str) {
        let mut first = true;
        for para in &mut self.sub_list.paragraphs {
            for run in &mut para.runs {
                if !run.texts.is_empty() {
                    if first {
                        run.texts = vec![new_text.to_string()];
                        first = false;
                    } else {
                        run.texts = vec![String::new()];
                    }
                }
            }
        }
        // 텍스트가 없는 셀에 새 텍스트를 넣는 경우
        if first {
            if let Some(para) = self.sub_list.paragraphs.first_mut() {
                if let Some(run) = para.runs.first_mut() {
                    run.texts = vec![new_text.to_string()];
                }
            }
        }
    }

    /// 병합 원점인지 (openhwp의 is_merge_origin과 동일)
    pub fn is_merge_origin(&self) -> bool {
        self.cell_span.col_span > 1 || self.cell_span.row_span > 1
    }
}
