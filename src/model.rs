//! HWPX 문서 모델 — 폼 필러에 특화
//!
//! openhwp의 `$value` enum 패턴을 채택하되, 폼 채움에 불필요한 도형/OLE/비디오 등은
//! `Unknown(serde_json::Value)`로 묶어서 보존만 함. 파싱 + 재직렬화 시 데이터 무손실.
//!
//! 핵심 구조:
//!   Section > Paragraph > Run > RunContent(enum) > Text/Table/SectionDef/...
//!   Table > TableRow > TableCell > SubList > Paragraph > ...

use serde::{Deserialize, Serialize};

// ── Section (section0.xml 루트) ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "sec")]
pub struct Section {
    #[serde(rename = "p", default)]
    pub paragraphs: Vec<Paragraph>,
}

// ── Paragraph ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "p")]
pub struct Paragraph {
    #[serde(rename = "run", default)]
    pub runs: Vec<Run>,

    #[serde(rename = "linesegarray", skip_serializing_if = "Option::is_none")]
    pub line_segments: Option<LineSegmentArray>,

    #[serde(rename = "@id", default)]
    pub id: u32,

    #[serde(rename = "@paraPrIDRef", skip_serializing_if = "Option::is_none")]
    pub para_pr_id_ref: Option<u32>,

    #[serde(rename = "@styleIDRef", skip_serializing_if = "Option::is_none")]
    pub style_id_ref: Option<u32>,

    #[serde(rename = "@pageBreak", default)]
    pub page_break: u32,

    #[serde(rename = "@columnBreak", default)]
    pub column_break: u32,

    #[serde(rename = "@merged", default)]
    pub merged: u32,
}

// ── LineSegmentArray (레이아웃 정보, 보존만) ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "linesegarray")]
pub struct LineSegmentArray {
    #[serde(rename = "lineseg", default)]
    pub segments: Vec<LineSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename = "lineseg")]
pub struct LineSegment {
    #[serde(rename = "@textpos", default)]
    pub textpos: i32,
    #[serde(rename = "@vertpos", default)]
    pub vertpos: i32,
    #[serde(rename = "@vertsize", default)]
    pub vertsize: i32,
    #[serde(rename = "@textheight", default)]
    pub textheight: i32,
    #[serde(rename = "@baseline", default)]
    pub baseline: i32,
    #[serde(rename = "@spacing", default)]
    pub spacing: i32,
    #[serde(rename = "@horzpos", default)]
    pub horzpos: i32,
    #[serde(rename = "@horzsize", default)]
    pub horzsize: i32,
    #[serde(rename = "@flags", default)]
    pub flags: u32,
}

// ── Run ──
// 핵심: openhwp의 `$value` + RunContent enum 패턴 채택

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "run")]
pub struct Run {
    /// Run 내부 콘텐츠 — 텍스트, 테이블, 섹션정의 등이 혼합 가능
    #[serde(rename = "$value", default, skip_serializing_if = "Vec::is_empty")]
    pub contents: Vec<RunContent>,

    #[serde(rename = "@charPrIDRef", skip_serializing_if = "Option::is_none")]
    pub char_pr_id_ref: Option<u32>,
}

/// Run 내부에 올 수 있는 모든 요소 — openhwp KS X 6101:2024 완전 커버
/// Text/Table은 구조화, 나머지는 serde_json::Value로 보존
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RunContent {
    // ── 폼 필러 핵심 ──
    /// 텍스트 요소 — 내부에 lineBreak, tab 등 포함 가능
    #[serde(rename = "t")]
    Text(TextElement),
    /// 표
    #[serde(rename = "tbl")]
    Table(Box<Table>),

    // ── 구조 요소 ──
    #[serde(rename = "secPr")]
    SectionDef(serde_json::Value),
    #[serde(rename = "ctrl")]
    Control(serde_json::Value),

    // ── 도형 ──
    #[serde(rename = "pic")]
    Picture(serde_json::Value),
    #[serde(rename = "line")]
    Line(serde_json::Value),
    #[serde(rename = "rect")]
    Rectangle(serde_json::Value),
    #[serde(rename = "ellipse")]
    Ellipse(serde_json::Value),
    #[serde(rename = "arc")]
    Arc(serde_json::Value),
    #[serde(rename = "polygon")]
    Polygon(serde_json::Value),
    #[serde(rename = "curve")]
    Curve(serde_json::Value),
    #[serde(rename = "connectLine")]
    ConnectLine(serde_json::Value),

    // ── OLE/수식/컨테이너 ──
    #[serde(rename = "ole")]
    Ole(serde_json::Value),
    #[serde(rename = "equation")]
    Equation(serde_json::Value),
    #[serde(rename = "container")]
    Container(serde_json::Value),

    // ── 텍스트 장식 ──
    #[serde(rename = "textart")]
    TextArt(serde_json::Value),
    #[serde(rename = "compose")]
    Compose(serde_json::Value),
    #[serde(rename = "dutmal")]
    Dutmal(serde_json::Value),

    // ── 폼 컨트롤 (openhwp에 있지만 우리에게 빠졌던 것들) ──
    #[serde(rename = "btn")]
    Button(serde_json::Value),
    #[serde(rename = "radioBtn")]
    RadioButton(serde_json::Value),
    #[serde(rename = "checkBtn")]
    CheckButton(serde_json::Value),
    #[serde(rename = "comboBox")]
    ComboBox(serde_json::Value),
    #[serde(rename = "listBox")]
    ListBox(serde_json::Value),
    #[serde(rename = "edit")]
    Edit(serde_json::Value),
    #[serde(rename = "scrollBar")]
    ScrollBar(serde_json::Value),

    // ── 미디어 ──
    #[serde(rename = "video")]
    Video(serde_json::Value),
    #[serde(rename = "chart")]
    Chart(serde_json::Value),

    // ── 기타 ──
    #[serde(rename = "unknownObject")]
    UnknownObject(serde_json::Value),
}

// ── TextElement: <hp:t> 내부 — 텍스트 + 인라인 마크업 ──
// openhwp의 TextElement + TextMarkup 패턴

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "t")]
pub struct TextElement {
    #[serde(rename = "$value", default, skip_serializing_if = "Vec::is_empty")]
    pub contents: Vec<TextMarkup>,
}

impl TextElement {
    /// 텍스트만 추출 (lineBreak는 \n으로)
    pub fn text(&self) -> String {
        let mut s = String::new();
        for m in &self.contents {
            match m {
                TextMarkup::Text(t) => s.push_str(t),
                TextMarkup::LineBreak(_) => s.push('\n'),
                TextMarkup::Tab(_) => s.push('\t'),
                TextMarkup::NonBreakingSpace(_) => s.push(' '),
                TextMarkup::FixedWidthSpace(_) => s.push(' '),
                _ => {} // markpen, titleMark 등은 텍스트 없음
            }
        }
        s
    }

    /// 텍스트 교체 — 모든 마크업을 제거하고 단순 텍스트로
    pub fn set_text(&mut self, new_text: &str) {
        self.contents = vec![TextMarkup::Text(new_text.to_string())];
    }
}

/// <hp:t> 내부의 마크업 — openhwp TextMarkup 완전 커버
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TextMarkup {
    /// 순수 텍스트
    #[serde(rename = "$text")]
    Text(String),
    /// 줄바꿈
    #[serde(rename = "lineBreak")]
    LineBreak(serde_json::Value),
    /// 탭
    #[serde(rename = "tab")]
    Tab(serde_json::Value),
    /// 하이픈
    #[serde(rename = "hyphen")]
    Hyphen(serde_json::Value),
    /// 비분리 공백
    #[serde(rename = "nbSpace")]
    NonBreakingSpace(serde_json::Value),
    /// 고정폭 공백
    #[serde(rename = "fwSpace")]
    FixedWidthSpace(serde_json::Value),
    /// 형광펜 시작
    #[serde(rename = "markpenBegin")]
    MarkPenBegin(serde_json::Value),
    /// 형광펜 종료
    #[serde(rename = "markpenEnd")]
    MarkPenEnd(serde_json::Value),
    /// 제목 표시
    #[serde(rename = "titleMark")]
    TitleMark(serde_json::Value),
    /// 삽입 추적 시작
    #[serde(rename = "insertBegin")]
    InsertBegin(serde_json::Value),
    /// 삽입 추적 종료
    #[serde(rename = "insertEnd")]
    InsertEnd(serde_json::Value),
    /// 삭제 추적 시작
    #[serde(rename = "deleteBegin")]
    DeleteBegin(serde_json::Value),
    /// 삭제 추적 종료
    #[serde(rename = "deleteEnd")]
    DeleteEnd(serde_json::Value),
}

// ── Table ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "tbl")]
pub struct Table {
    #[serde(rename = "sz", skip_serializing_if = "Option::is_none")]
    pub size: Option<TableSize>,

    #[serde(rename = "pos", skip_serializing_if = "Option::is_none")]
    pub position: Option<serde_json::Value>,

    #[serde(rename = "outMargin", skip_serializing_if = "Option::is_none")]
    pub out_margin: Option<serde_json::Value>,

    #[serde(rename = "inMargin", skip_serializing_if = "Option::is_none")]
    pub in_margin: Option<serde_json::Value>,

    #[serde(rename = "tr", default)]
    pub rows: Vec<TableRow>,

    #[serde(rename = "@id", skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    #[serde(rename = "@zOrder", default)]
    pub z_order: i32,
    #[serde(rename = "@numberingType", skip_serializing_if = "Option::is_none")]
    pub numbering_type: Option<String>,
    #[serde(rename = "@textWrap", skip_serializing_if = "Option::is_none")]
    pub text_wrap: Option<String>,
    #[serde(rename = "@textFlow", skip_serializing_if = "Option::is_none")]
    pub text_flow: Option<String>,
    #[serde(rename = "@lock", default)]
    pub lock: u32,
    #[serde(rename = "@dropcapstyle", skip_serializing_if = "Option::is_none")]
    pub dropcap_style: Option<String>,
    #[serde(rename = "@pageBreak", skip_serializing_if = "Option::is_none")]
    pub page_break: Option<String>,
    #[serde(rename = "@repeatHeader", default)]
    pub repeat_header: u32,
    #[serde(rename = "@rowCnt", skip_serializing_if = "Option::is_none")]
    pub row_count: Option<u32>,
    #[serde(rename = "@colCnt", skip_serializing_if = "Option::is_none")]
    pub col_count: Option<u32>,
    #[serde(rename = "@cellSpacing", default)]
    pub cell_spacing: u32,
    #[serde(rename = "@borderFillIDRef", skip_serializing_if = "Option::is_none")]
    pub border_fill_id_ref: Option<String>,
    #[serde(rename = "@noAdjust", default)]
    pub no_adjust: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "sz")]
pub struct TableSize {
    #[serde(rename = "@width")]
    pub width: u32,
    #[serde(rename = "@height")]
    pub height: u32,
    #[serde(rename = "@widthRelTo", skip_serializing_if = "Option::is_none")]
    pub width_rel_to: Option<String>,
    #[serde(rename = "@heightRelTo", skip_serializing_if = "Option::is_none")]
    pub height_rel_to: Option<String>,
    #[serde(rename = "@protect", default)]
    pub protect: u32,
}

// ── Table Row ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "tr")]
pub struct TableRow {
    #[serde(rename = "tc", default)]
    pub cells: Vec<TableCell>,
}

// ── Table Cell ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "tc")]
pub struct TableCell {
    #[serde(rename = "subList")]
    pub sub_list: SubList,

    #[serde(rename = "cellAddr")]
    pub cell_addr: CellAddr,

    #[serde(rename = "cellSpan")]
    pub cell_span: CellSpan,

    #[serde(rename = "cellSz")]
    pub cell_size: CellSize,

    #[serde(rename = "cellMargin", skip_serializing_if = "Option::is_none")]
    pub cell_margin: Option<CellMargin>,

    #[serde(rename = "@name", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename = "@header", default)]
    pub header: u32,
    #[serde(rename = "@hasMargin", default)]
    pub has_margin: u32,
    #[serde(rename = "@protect", default)]
    pub protect: u32,
    #[serde(rename = "@editable", default)]
    pub editable: u32,
    #[serde(rename = "@dirty", default)]
    pub dirty: u32,
    #[serde(rename = "@borderFillIDRef", skip_serializing_if = "Option::is_none")]
    pub border_fill_id_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename = "subList")]
pub struct SubList {
    #[serde(rename = "p", default)]
    pub paragraphs: Vec<Paragraph>,

    #[serde(rename = "@id", default)]
    pub id: String,
    #[serde(rename = "@textDirection", skip_serializing_if = "Option::is_none")]
    pub text_direction: Option<String>,
    #[serde(rename = "@lineWrap", skip_serializing_if = "Option::is_none")]
    pub line_wrap: Option<String>,
    #[serde(rename = "@vertAlign", skip_serializing_if = "Option::is_none")]
    pub vert_align: Option<String>,
    #[serde(rename = "@linkListIDRef", default)]
    pub link_list_id_ref: u32,
    #[serde(rename = "@linkListNextIDRef", default)]
    pub link_list_next_id_ref: u32,
    #[serde(rename = "@textWidth", default)]
    pub text_width: u32,
    #[serde(rename = "@textHeight", default)]
    pub text_height: u32,
    #[serde(rename = "@hasTextRef", default)]
    pub has_text_ref: u32,
    #[serde(rename = "@hasNumRef", default)]
    pub has_num_ref: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename = "cellAddr")]
pub struct CellAddr {
    #[serde(rename = "@colAddr")]
    pub col: u32,
    #[serde(rename = "@rowAddr")]
    pub row: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename = "cellSpan")]
pub struct CellSpan {
    #[serde(rename = "@colSpan", default = "one")]
    pub col_span: u32,
    #[serde(rename = "@rowSpan", default = "one")]
    pub row_span: u32,
}

fn one() -> u32 { 1 }

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename = "cellSz")]
pub struct CellSize {
    #[serde(rename = "@width")]
    pub width: u32,
    #[serde(rename = "@height")]
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

// ── 폼 필러 전용 메서드 ──

impl Table {
    pub fn get_cell(&self, row: u32, col: u32) -> Option<&TableCell> {
        self.rows.iter()
            .flat_map(|r| &r.cells)
            .find(|c| c.cell_addr.row == row && c.cell_addr.col == col)
    }

    pub fn get_cell_mut(&mut self, row: u32, col: u32) -> Option<&mut TableCell> {
        self.rows.iter_mut()
            .flat_map(|r| &mut r.cells)
            .find(|c| c.cell_addr.row == row && c.cell_addr.col == col)
    }

    pub fn find_row(&self, row_addr: u32) -> Option<(usize, &TableRow)> {
        self.rows.iter().enumerate()
            .find(|(_, r)| r.cells.first().map(|c| c.cell_addr.row) == Some(row_addr))
    }

    pub fn clone_row(&mut self, template_row_idx: usize, count: usize) {
        if template_row_idx >= self.rows.len() || count == 0 {
            return;
        }

        let template = self.rows[template_row_idx].clone();
        let template_row_addr = template.cells.first()
            .map(|c| c.cell_addr.row)
            .unwrap_or(0);

        let mut new_rows = Vec::with_capacity(count);
        for i in 1..=count {
            let mut row = template.clone();
            let new_addr = template_row_addr + i as u32;
            for cell in &mut row.cells {
                cell.cell_addr.row = new_addr;
            }
            new_rows.push(row);
        }

        // 기존 아래 행들 rowAddr 증가 (독립 리뷰 #8 해결!)
        for row in &mut self.rows[(template_row_idx + 1)..] {
            for cell in &mut row.cells {
                cell.cell_addr.row += count as u32;
            }
        }

        let insert_at = template_row_idx + 1;
        for (i, row) in new_rows.into_iter().enumerate() {
            self.rows.insert(insert_at + i, row);
        }

        if let Some(rc) = &mut self.row_count {
            *rc += count as u32;
        }

        if let Some(sz) = &mut self.size {
            let row_height = template.cells.first()
                .map(|c| c.cell_size.height)
                .unwrap_or(2229);
            sz.height += row_height * count as u32;
        }
    }
}

impl TableCell {
    /// 셀 텍스트 추출 — 모든 TextElement의 text()를 합침
    pub fn text(&self) -> String {
        self.sub_list.paragraphs.iter()
            .flat_map(|p| &p.runs)
            .flat_map(|r| &r.contents)
            .filter_map(|c| match c {
                RunContent::Text(te) => Some(te.text()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// 셀 텍스트 교체 — 첫 번째 TextElement만 교체, 나머지는 비움
    pub fn set_text(&mut self, new_text: &str) {
        let mut first = true;
        for para in &mut self.sub_list.paragraphs {
            for run in &mut para.runs {
                for content in &mut run.contents {
                    if let RunContent::Text(te) = content {
                        if first {
                            te.set_text(new_text);
                            first = false;
                        } else {
                            te.set_text("");
                        }
                    }
                }
            }
        }
    }

    pub fn is_merge_origin(&self) -> bool {
        self.cell_span.col_span > 1 || self.cell_span.row_span > 1
    }
}
