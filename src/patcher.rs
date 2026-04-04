//! XML 패처 — 원본 XML 보존하면서 셀 텍스트 교체 + 행 클론
//!
//! 전략: 2-pass 문자열 수준 패치
//! - Pass 1: quick-xml로 바이트 위치 수집 (어떤 바이트 범위가 대상인지)
//! - Pass 2: 문자열 치환 (원본 XML byte-perfect 보존)
//!
//! 왜 Writer가 아닌 문자열 치환인가:
//! quick-xml Writer는 속성 순서, 공백, 네임스페이스를 바꿀 수 있어서
//! 한컴오피스 호환성이 깨질 수 있음. 문자열 치환은 대상만 변경.

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::Result;

/// 원본 XML에서 특정 셀의 텍스트를 교체
///
/// 2-pass 접근:
/// Pass 1: quick-xml Reader로 대상 셀의 <hp:t> 바이트 위치 수집
/// Pass 2: 문자열 치환
pub fn patch_cell_text(
    xml: &str,
    table_index: usize,
    row_addr: u32,
    col_addr: u32,
    new_text: &str,
) -> Result<String> {
    // Pass 1: 대상 셀의 <hp:t>TEXT</hp:t> 위치 찾기
    let target = find_cell_text_position(xml, table_index, row_addr, col_addr)?;

    match target {
        Some(CellTextTarget::ReplaceText(text_start, text_end)) => {
            // 기존 텍스트 교체 (또는 빈 <hp:t></hp:t>에 삽입)
            let mut result = String::with_capacity(xml.len() + new_text.len());
            result.push_str(&xml[..text_start]);
            result.push_str(&escape_xml(new_text));
            result.push_str(&xml[text_end..]);
            Ok(result)
        }
        Some(CellTextTarget::InsertIntoEmptyRun(slash_pos)) => {
            // <hp:run charPrIDRef="X"/> → <hp:run charPrIDRef="X"><hp:t>TEXT</hp:t></hp:run>
            // slash_pos는 "/" 위치, 그 다음이 ">"
            let mut result = String::with_capacity(xml.len() + new_text.len() + 30);
            result.push_str(&xml[..slash_pos]); // <hp:run charPrIDRef="X" 까지
            result.push_str(&format!("><hp:t>{}</hp:t></hp:run>", escape_xml(new_text)));
            result.push_str(&xml[slash_pos + 2..]); // "/>" 이후
            Ok(result)
        }
        None => Ok(xml.to_string()),
    }
}

/// 셀 텍스트 위치 결과
enum CellTextTarget {
    /// 기존 텍스트 교체: (text_start, text_end) 범위
    ReplaceText(usize, usize),
    /// 빈 셀 삽입: self-closing <hp:run .../> 의 /> 위치
    InsertIntoEmptyRun(usize),
}

/// 셀 내 텍스트 위치 또는 빈 run 위치 찾기
fn find_cell_text_position(
    xml: &str,
    table_index: usize,
    row_addr: u32,
    col_addr: u32,
) -> Result<Option<CellTextTarget>> {
    let Some((table_start, table_end)) = find_leaf_table_span(xml, table_index)? else {
        return Ok(None);
    };

    find_cell_text_position_in_leaf_table(
        &xml[table_start..table_end],
        table_start,
        row_addr,
        col_addr,
    )
}

fn find_cell_text_position_in_leaf_table(
    table_xml: &str,
    global_offset: usize,
    row_addr: u32,
    col_addr: u32,
) -> Result<Option<CellTextTarget>> {
    let mut reader = Reader::from_str(table_xml);

    let mut in_cell = false;
    let mut cell_t_positions: Vec<(usize, usize)> = Vec::new();
    let mut cell_empty_run_pos: Option<usize> = None;
    let mut cell_row_addr: Option<u32> = None;
    let mut cell_col_addr: Option<u32> = None;

    loop {
        let _offset = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");

                match name {
                    "tc" => {
                        in_cell = true;
                        cell_t_positions.clear();
                        cell_empty_run_pos = None;
                        cell_row_addr = None;
                        cell_col_addr = None;
                    }
                    "t" if in_cell => {
                        // <hp:t> 시작 위치 기록 — 텍스트 시작은 태그 닫힘 후
                        let t_content_start = reader.buffer_position() as usize;
                        // 텍스트 끝은 </hp:t> 시작 직전
                        // → Text 이벤트에서 캡처
                        cell_t_positions.push((t_content_start, t_content_start)); // 임시, End에서 갱신
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(_)) if in_cell && !cell_t_positions.is_empty() => {
                // <hp:t> 안의 텍스트 — 끝 위치 갱신
                let text_end = reader.buffer_position() as usize;
                if let Some(last) = cell_t_positions.last_mut() {
                    last.1 = text_end;
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");

                match name {
                    "t" if in_cell => {
                        // </hp:t> — 끝 위치 확정 (Text 이벤트에서 이미 갱신됨)
                        // Text 이벤트가 없었다면 (빈 <hp:t></hp:t>) 시작=끝
                    }
                    "tc" if in_cell => {
                        // 셀 종료 — cellAddr 확인 후 매칭되면 반환
                        in_cell = false;
                        if cell_row_addr == Some(row_addr) && cell_col_addr == Some(col_addr) {
                            // Case 1: 텍스트가 있는 셀 → 교체
                            if let Some(&(start, end)) = cell_t_positions.first() {
                                if start != end {
                                    return Ok(Some(CellTextTarget::ReplaceText(
                                        global_offset + start,
                                        global_offset + end,
                                    )));
                                }
                            }
                            // Case 2: <hp:t></hp:t> (빈 텍스트) → 삽입 위치 = start
                            if let Some(&(start, _)) = cell_t_positions.first() {
                                return Ok(Some(CellTextTarget::ReplaceText(
                                    global_offset + start,
                                    global_offset + start,
                                )));
                            }
                            // Case 3: self-closing <hp:run/> → 확장 삽입
                            if let Some(pos) = cell_empty_run_pos {
                                return Ok(Some(CellTextTarget::InsertIntoEmptyRun(
                                    global_offset + pos,
                                )));
                            }
                            return Ok(None);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");

                // self-closing <hp:run charPrIDRef="X"/> — 빈 셀
                if name == "run" && in_cell && cell_empty_run_pos.is_none() {
                    // /> 직전 위치 = reader.buffer_position() - 2 ("/>")
                    // 실제로는 원본 XML에서 이 태그의 위치를 찾아야 함
                    let pos = reader.buffer_position() as usize;
                    // pos는 /> 다음 바이트. 원본에서 "/>" 를 찾아서 "/" 위치를 기록
                    if pos >= 2 && &table_xml[pos-2..pos] == "/>" {
                        cell_empty_run_pos = Some(pos - 2); // "/" 위치
                    }
                }

                if name == "cellAddr" && in_cell {
                    for attr in e.attributes().filter_map(|a| a.ok()) {
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = std::str::from_utf8(&attr.value).unwrap_or("");
                        match key {
                            "rowAddr" => cell_row_addr = val.parse().ok(),
                            "colAddr" => cell_col_addr = val.parse().ok(),
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(crate::error::FillerError::Validation(
                format!("XML read error: {}", e)
            )),
            _ => {}
        }
    }

    Ok(None)
}

/// 여러 셀을 한 번에 패치 (순차 적용)
pub fn patch_cells(
    xml: &str,
    patches: &[(usize, u32, u32, String)], // (table_index, row, col, text)
) -> Result<String> {
    let mut result = xml.to_string();
    for (table_index, row, col, text) in patches {
        result = patch_cell_text(&result, *table_index, *row, *col, text)?;
    }
    Ok(result)
}

/// 여러 셀을 패치하면서 스킵된 패치 정보를 반환
/// 반환: (패치된 XML, 스킵된 패치 목록)
pub fn patch_cells_with_report(
    xml: &str,
    patches: &[(usize, u32, u32, String)],
) -> Result<(String, Vec<(usize, u32, u32)>)> {
    let mut result = xml.to_string();
    let mut skipped = Vec::new();
    for (table_index, row, col, text) in patches {
        let before_len = result.len();
        result = patch_cell_text(&result, *table_index, *row, *col, text)?;
        // patch_cell_text returns original if cell not found, so length is unchanged
        if result.len() == before_len && !text.is_empty() {
            skipped.push((*table_index, *row, *col));
        }
    }
    Ok((result, skipped))
}

/// 행 클론 — template row를 N번 복제하고 rowAddr 재계산
///
/// 전략: quick-xml로 바이트 위치만 찾고, 실제 수정은 문자열 수준.
pub fn patch_clone_rows(
    xml: &str,
    table_index: usize,
    template_row_addr: u32,
    clone_count: usize,
) -> Result<String> {
    if clone_count == 0 {
        return Ok(xml.to_string());
    }

    let Some((table_start, table_end)) = find_leaf_table_span(xml, table_index)? else {
        return Err(crate::error::FillerError::RowNotFound {
            table: table_index,
            row: template_row_addr,
        });
    };

    let patched_fragment = patch_clone_rows_in_leaf_table(
        &xml[table_start..table_end],
        table_index,
        template_row_addr,
        clone_count,
    )?;

    let mut result = String::with_capacity(xml.len() + patched_fragment.len());
    result.push_str(&xml[..table_start]);
    result.push_str(&patched_fragment);
    result.push_str(&xml[table_end..]);
    Ok(result)
}

fn patch_clone_rows_in_leaf_table(
    table_xml: &str,
    table_index: usize,
    template_row_addr: u32,
    clone_count: usize,
) -> Result<String> {
    let mut reader = Reader::from_str(table_xml);
    let mut positions = RowClonePositions {
        tbl_rowcnt_pos: None,
        tbl_rowcnt_val: 0,
        tbl_height_pos: None,
        tbl_height_val: 0,
        template_row_height: 0,
        template_row_start: 0,
        template_row_end: 0,
        subsequent_row_addrs: Vec::new(),
        found: false,
    };

    let mut in_tr = false;
    let mut tr_start_offset = 0usize;
    let mut current_row_addr: Option<u32> = None;
    let mut past_template = false;

    loop {
        let offset = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");

                match name {
                    "tbl" => {
                        let tag_end = table_xml[offset..].find('>').unwrap_or(500);
                        let tag_raw = &table_xml[offset..offset + tag_end];
                        if let Some(rc_match) = find_attr_in_raw(tag_raw, "rowCnt") {
                            positions.tbl_rowcnt_pos = Some(offset + rc_match.0);
                            positions.tbl_rowcnt_val = rc_match.1;
                        }
                        let sz_search = &table_xml[offset..std::cmp::min(offset + 500, table_xml.len())];
                        if let Some(sz_pos) = sz_search.find("hp:sz") {
                            let sz_raw = &table_xml[offset + sz_pos..];
                            if let Some(h_match) = find_attr_in_raw(sz_raw, "height") {
                                positions.tbl_height_pos = Some(offset + sz_pos + h_match.0);
                                positions.tbl_height_val = h_match.1;
                            }
                        }
                    }
                    "tr" => {
                        in_tr = true;
                        tr_start_offset = offset;
                        current_row_addr = None;
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");

                if name == "cellSz" && in_tr && positions.template_row_height == 0 {
                    for attr in e.attributes().filter_map(|a| a.ok()) {
                        if attr.key.as_ref() == b"height" {
                            if let Ok(h) = std::str::from_utf8(&attr.value).unwrap_or("0").parse::<u32>() {
                                positions.template_row_height = h;
                            }
                        }
                    }
                }

                if name == "cellAddr" && in_tr && current_row_addr.is_none() {
                    for attr in e.attributes().filter_map(|a| a.ok()) {
                        if attr.key.as_ref() == b"rowAddr" {
                            current_row_addr = std::str::from_utf8(&attr.value)
                                .ok()
                                .and_then(|v| v.parse().ok());
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");

                if name == "tr" {
                    let tr_end_offset = reader.buffer_position() as usize;

                    if let Some(addr) = current_row_addr {
                        if addr == template_row_addr {
                            positions.template_row_start = tr_start_offset;
                            positions.template_row_end = tr_end_offset;
                            positions.found = true;
                            past_template = true;
                        } else if past_template {
                            let row_raw = &table_xml[tr_start_offset..tr_end_offset];
                            collect_row_addr_positions(
                                row_raw,
                                tr_start_offset,
                                &mut positions.subsequent_row_addrs,
                            );
                        }
                    }
                    in_tr = false;
                }
                if name == "tbl" { past_template = false; }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(crate::error::FillerError::Validation(
                format!("XML read error: {}", e)
            )),
            _ => {}
        }
    }

    if !positions.found {
        return Err(crate::error::FillerError::RowNotFound {
            table: table_index,
            row: template_row_addr,
        });
    }

    // Pass 2: 문자열 수준 수정 (뒤에서부터)
    let mut result = table_xml.to_string();

    // 2a. 후속 행들의 rowAddr 시프트 (뒤에서부터)
    for &(pos, old_val) in positions.subsequent_row_addrs.iter().rev() {
        let old_str = format!("rowAddr=\"{}\"", old_val);
        let new_str = format!("rowAddr=\"{}\"", old_val + clone_count as u32);
        let end = pos + old_str.len();
        if result[pos..].starts_with(&old_str) {
            result.replace_range(pos..end, &new_str);
        }
    }

    // 2b. 클론 행 삽입
    let template_raw = &table_xml[positions.template_row_start..positions.template_row_end];
    let mut clones = String::new();
    for i in 1..=clone_count {
        let new_addr = template_row_addr + i as u32;
        let cloned = rewrite_all_row_addrs(template_raw, new_addr);
        clones.push('\n');
        clones.push_str(&cloned);
    }
    result.insert_str(positions.template_row_end, &clones);

    // 2c. rowCnt 갱신
    if let Some(pos) = positions.tbl_rowcnt_pos {
        let old_str = format!("rowCnt=\"{}\"", positions.tbl_rowcnt_val);
        let new_str = format!("rowCnt=\"{}\"", positions.tbl_rowcnt_val + clone_count as u32);
        if result[pos..].starts_with(&old_str) {
            result.replace_range(pos..pos + old_str.len(), &new_str);
        }
    }

    // 2d. 테이블 높이 갱신
    if let Some(pos) = positions.tbl_height_pos {
        let row_height = if positions.template_row_height > 0 {
            positions.template_row_height
        } else {
            2229
        };
        let added_height = row_height * clone_count as u32;
        let new_height = positions.tbl_height_val + added_height;
        let old_str = format!("height=\"{}\"", positions.tbl_height_val);
        let new_str = format!("height=\"{}\"", new_height);
        if result[pos..].starts_with(&old_str) {
            result.replace_range(pos..pos + old_str.len(), &new_str);
        }
    }

    Ok(result)
}

/// 여러 테이블에 행 클론 적용 (순차)
pub fn patch_clone_rows_multi(
    xml: &str,
    clones: &[(usize, u32, usize)],
) -> Result<String> {
    let mut result = xml.to_string();
    for (table_index, row_addr, count) in clones {
        result = patch_clone_rows(&result, *table_index, *row_addr, *count)?;
    }
    Ok(result)
}

// ── 헬퍼 ──

fn find_leaf_table_span(xml: &str, target_index: usize) -> Result<Option<(usize, usize)>> {
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<(usize, bool)> = Vec::new(); // (start_offset, has_nested_child)
    let mut leaf_index = 0usize;

    loop {
        let offset = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");
                if name == "tbl" {
                    if let Some((_, has_nested)) = stack.last_mut() {
                        *has_nested = true;
                    }
                    stack.push((offset, false));
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");
                if name == "tbl" {
                    let end = reader.buffer_position() as usize;
                    if let Some((start, has_nested)) = stack.pop() {
                        if !has_nested {
                            if leaf_index == target_index {
                                return Ok(Some((start, end)));
                            }
                            leaf_index += 1;
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(crate::error::FillerError::Validation(
                format!("XML read error: {}", e)
            )),
            _ => {}
        }
    }

    Ok(None)
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}

struct RowClonePositions {
    tbl_rowcnt_pos: Option<usize>,
    tbl_rowcnt_val: u32,
    tbl_height_pos: Option<usize>,
    tbl_height_val: u32,
    template_row_height: u32,
    template_row_start: usize,
    template_row_end: usize,
    subsequent_row_addrs: Vec<(usize, u32)>,
    found: bool,
}

fn find_attr_in_raw(raw: &str, attr_name: &str) -> Option<(usize, u32)> {
    let pattern = format!("{}=\"", attr_name);
    if let Some(start) = raw.find(&pattern) {
        let val_start = start + pattern.len();
        if let Some(val_end) = raw[val_start..].find('"') {
            let val_str = &raw[val_start..val_start + val_end];
            if let Ok(val) = val_str.parse::<u32>() {
                return Some((start, val));
            }
        }
    }
    None
}

/// nested table 내부를 skip하면서 rowAddr 위치 수집
fn collect_row_addr_positions(
    row_raw: &str,
    global_offset: usize,
    positions: &mut Vec<(usize, u32)>,
) {
    let pattern = "rowAddr=\"";
    let mut search_from = 0;
    let mut tbl_depth = 0;

    while search_from < row_raw.len() {
        if row_raw[search_from..].starts_with("<hp:tbl") || row_raw[search_from..].starts_with("<tbl") {
            tbl_depth += 1;
            search_from += 1;
            continue;
        }
        if row_raw[search_from..].starts_with("</hp:tbl") || row_raw[search_from..].starts_with("</tbl") {
            tbl_depth -= 1;
            search_from += 1;
            continue;
        }
        if tbl_depth > 0 {
            search_from += 1;
            continue;
        }
        if let Some(rel_pos) = row_raw[search_from..].find(pattern) {
            let abs_pos = global_offset + search_from + rel_pos;
            let val_start = search_from + rel_pos + pattern.len();
            if let Some(val_end) = row_raw[val_start..].find('"') {
                let val_str = &row_raw[val_start..val_start + val_end];
                if let Ok(val) = val_str.parse::<u32>() {
                    positions.push((abs_pos, val));
                }
                search_from = val_start + val_end + 1;
            } else {
                break;
            }
        } else {
            break;
        }
    }
}

/// nested table 내부 보존하면서 rowAddr 교체
fn rewrite_all_row_addrs(row_raw: &str, new_addr: u32) -> String {
    let pattern = "rowAddr=\"";
    let mut result = String::with_capacity(row_raw.len());
    let mut search_from = 0;
    let mut tbl_depth = 0;

    while search_from < row_raw.len() {
        if row_raw[search_from..].starts_with("<hp:tbl") || row_raw[search_from..].starts_with("<tbl") {
            tbl_depth += 1;
        }
        if row_raw[search_from..].starts_with("</hp:tbl") || row_raw[search_from..].starts_with("</tbl") {
            tbl_depth -= 1;
        }
        if tbl_depth > 0 {
            result.push(row_raw.as_bytes()[search_from] as char);
            search_from += 1;
            continue;
        }
        if let Some(rel_pos) = row_raw[search_from..].find(pattern) {
            let next_tbl = row_raw[search_from..].find("<hp:tbl").or_else(|| row_raw[search_from..].find("<tbl"));
            if let Some(tbl_pos) = next_tbl {
                if tbl_pos < rel_pos {
                    result.push_str(&row_raw[search_from..search_from + tbl_pos]);
                    search_from += tbl_pos;
                    continue;
                }
            }
            result.push_str(&row_raw[search_from..search_from + rel_pos + pattern.len()]);
            let val_start = search_from + rel_pos + pattern.len();
            if let Some(val_end) = row_raw[val_start..].find('"') {
                result.push_str(&new_addr.to_string());
                search_from = val_start + val_end;
            } else {
                search_from = val_start;
            }
        } else {
            break;
        }
    }
    result.push_str(&row_raw[search_from..]);
    result
}

#[cfg(test)]
mod tests {
    use super::{patch_cell_text, patch_clone_rows};

    fn nested_leaf_table_xml() -> &'static str {
        r#"<hs:sec xmlns:hs="urn:hs" xmlns:hp="urn:hp">
<hp:tbl id="outer" rowCnt="1" colCnt="1">
  <hp:tr>
    <hp:tc>
      <hp:subList>
        <hp:p>
          <hp:run>
            <hp:tbl id="info" rowCnt="2" colCnt="2">
              <hp:tr>
                <hp:tc><hp:subList><hp:p><hp:run><hp:t>성 명</hp:t></hp:run></hp:p></hp:subList><hp:cellAddr colAddr="0" rowAddr="0"/></hp:tc>
                <hp:tc><hp:subList><hp:p><hp:run><hp:t></hp:t></hp:run></hp:p></hp:subList><hp:cellAddr colAddr="1" rowAddr="0"/></hp:tc>
              </hp:tr>
              <hp:tr>
                <hp:tc><hp:subList><hp:p><hp:run><hp:t>직 책</hp:t></hp:run></hp:p></hp:subList><hp:cellAddr colAddr="0" rowAddr="1"/></hp:tc>
                <hp:tc><hp:subList><hp:p><hp:run><hp:t></hp:t></hp:run></hp:p></hp:subList><hp:cellAddr colAddr="1" rowAddr="1"/></hp:tc>
              </hp:tr>
            </hp:tbl>
            <hp:tbl id="projects" rowCnt="3" colCnt="2">
              <hp:sz width="100" height="300" protect="0"/>
              <hp:tr>
                <hp:tc><hp:subList><hp:p><hp:run><hp:t>사 업 명</hp:t></hp:run></hp:p></hp:subList><hp:cellAddr colAddr="0" rowAddr="0"/></hp:tc>
                <hp:tc><hp:subList><hp:p><hp:run><hp:t>담당업무</hp:t></hp:run></hp:p></hp:subList><hp:cellAddr colAddr="1" rowAddr="0"/></hp:tc>
              </hp:tr>
              <hp:tr>
                <hp:tc><hp:subList><hp:p><hp:run><hp:t></hp:t></hp:run></hp:p></hp:subList><hp:cellAddr colAddr="0" rowAddr="1"/><hp:cellSz height="40"/></hp:tc>
                <hp:tc><hp:subList><hp:p><hp:run><hp:t></hp:t></hp:run></hp:p></hp:subList><hp:cellAddr colAddr="1" rowAddr="1"/><hp:cellSz height="40"/></hp:tc>
              </hp:tr>
              <hp:tr>
                <hp:tc><hp:subList><hp:p><hp:run><hp:t></hp:t></hp:run></hp:p></hp:subList><hp:cellAddr colAddr="0" rowAddr="2"/><hp:cellSz height="40"/></hp:tc>
                <hp:tc><hp:subList><hp:p><hp:run><hp:t></hp:t></hp:run></hp:p></hp:subList><hp:cellAddr colAddr="1" rowAddr="2"/><hp:cellSz height="40"/></hp:tc>
              </hp:tr>
            </hp:tbl>
          </hp:run>
        </hp:p>
      </hp:subList>
      <hp:cellAddr colAddr="0" rowAddr="0"/>
    </hp:tc>
  </hp:tr>
</hp:tbl>
</hs:sec>"#
    }

    #[test]
    fn patch_cell_text_uses_leaf_table_index() {
        let xml = nested_leaf_table_xml();
        let patched = patch_cell_text(xml, 1, 1, 0, "프로젝트 A").expect("patch failed");

        assert!(patched.contains("<hp:t>프로젝트 A</hp:t>"));
        assert!(patched.contains("<hp:t>성 명</hp:t>"));
    }

    #[test]
    fn patch_clone_rows_uses_leaf_table_index() {
        let xml = nested_leaf_table_xml();
        let patched = patch_clone_rows(xml, 1, 1, 2).expect("clone failed");

        assert!(patched.contains("rowCnt=\"5\""));
        assert!(patched.contains("rowAddr=\"4\""));
    }
}
