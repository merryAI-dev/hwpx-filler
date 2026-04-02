//! XML 패처 — 구조체 분석 결과를 원본 XML에 적용
//!
//! 전략: 파싱은 serde로 완벽하게, 출력은 원본 XML 문자열 패치로.
//! 이렇게 하면:
//! - 파싱: 타입 안전, 중첩 처리, unknown 요소 보존
//! - 출력: 원본 XML의 모든 요소/속성/네임스페이스 100% 보존
//! - 수정: <hp:t> 텍스트만 교체, 나머지는 건드리지 않음

use quick_xml::events::{Event, BytesText};
use quick_xml::{Reader, Writer};
use std::io::Cursor;

use crate::error::Result;

/// 원본 XML에서 특정 셀의 텍스트를 교체
/// cell_address: (rowAddr, colAddr) 튜플
/// 원본 XML을 스트리밍하면서 해당 셀의 <hp:t> 내용만 교체
pub fn patch_cell_text(
    xml: &str,
    table_index: usize,
    row_addr: u32,
    col_addr: u32,
    new_text: &str,
) -> Result<String> {
    let mut reader = Reader::from_str(xml);
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    let mut current_table = 0usize;
    let mut in_target_table = false;
    let mut in_target_cell = false;
    let mut in_t_tag = false;
    let mut t_replaced = false;
    let mut table_depth = 0;

    // 현재 셀의 rowAddr/colAddr 추적
    let mut current_row_addr: Option<u32> = None;
    let mut current_col_addr: Option<u32> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");

                match name {
                    "tbl" => {
                        if table_depth == 0 {
                            in_target_table = current_table == table_index;
                            current_table += 1;
                        }
                        table_depth += 1;
                    }
                    "tc" if in_target_table => {
                        // 새 셀 시작 — 주소 초기화
                        current_row_addr = None;
                        current_col_addr = None;
                        t_replaced = false;
                    }
                    "t" if in_target_cell && !t_replaced => {
                        in_t_tag = true;
                        writer.write_event(Event::Start(e.clone()))?;
                        continue; // 텍스트 내용은 Text 이벤트에서 처리
                    }
                    _ => {}
                }

                writer.write_event(Event::Start(e.clone()))?;
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");

                if name == "cellAddr" && in_target_table {
                    // cellAddr 속성에서 rowAddr, colAddr 추출
                    for attr in e.attributes().filter_map(|a| a.ok()) {
                        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                        let val = std::str::from_utf8(&attr.value).unwrap_or("");
                        match key {
                            "rowAddr" => current_row_addr = val.parse().ok(),
                            "colAddr" => current_col_addr = val.parse().ok(),
                            _ => {}
                        }
                    }
                    in_target_cell = current_row_addr == Some(row_addr)
                        && current_col_addr == Some(col_addr);
                }

                writer.write_event(Event::Empty(e.clone()))?;
            }
            Ok(Event::Text(ref e)) => {
                if in_t_tag && in_target_cell && !t_replaced {
                    // 텍스트 교체!
                    writer.write_event(Event::Text(BytesText::new(new_text)))?;
                    t_replaced = true;
                    in_t_tag = false;
                } else {
                    writer.write_event(Event::Text(e.clone()))?;
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");

                match name {
                    "tbl" => {
                        table_depth -= 1;
                        if table_depth == 0 {
                            in_target_table = false;
                        }
                    }
                    "tc" if in_target_table => {
                        in_target_cell = false;
                    }
                    "t" => {
                        in_t_tag = false;
                    }
                    _ => {}
                }

                writer.write_event(Event::End(e.clone()))?;
            }
            Ok(Event::Eof) => break,
            Ok(e) => writer.write_event(e)?,
            Err(e) => return Err(crate::error::FillerError::Validation(
                format!("XML read error: {}", e)
            )),
        }
    }

    let result = writer.into_inner().into_inner();
    Ok(String::from_utf8(result).map_err(|e|
        crate::error::FillerError::Validation(format!("UTF-8 error: {}", e))
    )?)
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

/// 행 클론 — template row를 N번 복제하고 rowAddr 재계산
///
/// 전략: quick-xml로 바이트 위치만 찾고, 실제 수정은 문자열 수준.
/// 이래야 원본 XML(네임스페이스, 속성 순서, 공백)이 byte-perfect 보존됨.
///
/// 수정 사항:
/// 1. template row 뒤에 클론 행 삽입 (rowAddr 증가)
/// 2. 기존 후속 행들의 rowAddr 시프트
/// 3. 테이블 rowCnt 속성 갱신
pub fn patch_clone_rows(
    xml: &str,
    table_index: usize,
    template_row_addr: u32,
    clone_count: usize,
) -> Result<String> {
    if clone_count == 0 {
        return Ok(xml.to_string());
    }

    // Pass 1: quick-xml Reader로 바이트 위치 수집
    let mut reader = Reader::from_str(xml);
    let mut positions = RowClonePositions {
        tbl_rowcnt_pos: None,
        tbl_rowcnt_val: 0,
        template_row_start: 0,
        template_row_end: 0,
        subsequent_row_addrs: Vec::new(),
        found: false,
    };

    let mut current_table = 0usize;
    let mut table_depth = 0;
    let mut in_target_table = false;
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
                        if table_depth == 0 {
                            in_target_table = current_table == table_index;
                            if in_target_table {
                                // rowCnt 위치와 값 기록 — tbl 태그 내부만 검색 (> 이전까지)
                                let tag_end = xml[offset..].find('>').unwrap_or(500);
                                let tag_raw = &xml[offset..offset + tag_end];
                                if let Some(rc_match) = find_attr_in_raw(tag_raw, "rowCnt") {
                                    positions.tbl_rowcnt_pos = Some(offset + rc_match.0);
                                    positions.tbl_rowcnt_val = rc_match.1;
                                }
                            }
                            current_table += 1;
                        }
                        table_depth += 1;
                    }
                    "tr" if in_target_table && table_depth == 1 => {
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

                if name == "cellAddr" && in_target_table && in_tr && current_row_addr.is_none() {
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

                if name == "tr" && in_target_table && table_depth == 1 {
                    let tr_end_offset = reader.buffer_position() as usize;

                    if let Some(addr) = current_row_addr {
                        if addr == template_row_addr {
                            positions.template_row_start = tr_start_offset;
                            positions.template_row_end = tr_end_offset;
                            positions.found = true;
                            past_template = true;
                        } else if past_template {
                            // template 이후의 행 — rowAddr 위치 기록
                            let row_raw = &xml[tr_start_offset..tr_end_offset];
                            collect_row_addr_positions(
                                row_raw,
                                tr_start_offset,
                                &mut positions.subsequent_row_addrs,
                            );
                        }
                    }
                    in_tr = false;
                }
                if name == "tbl" {
                    table_depth -= 1;
                    if table_depth == 0 {
                        in_target_table = false;
                        past_template = false;
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    if !positions.found {
        return Err(crate::error::FillerError::RowNotFound {
            table: table_index,
            row: template_row_addr,
        });
    }

    // Pass 2: 문자열 수준 수정 (뒤에서부터 — offset 안 밀리게)
    let mut result = xml.to_string();

    // 2a. 후속 행들의 rowAddr 시프트 (뒤에서부터)
    for &(pos, old_val) in positions.subsequent_row_addrs.iter().rev() {
        let old_str = format!("rowAddr=\"{}\"", old_val);
        let new_str = format!("rowAddr=\"{}\"", old_val + clone_count as u32);
        let end = pos + old_str.len();
        if result[pos..].starts_with(&old_str) {
            result.replace_range(pos..end, &new_str);
        }
    }

    // 2b. 클론 행 삽입 (template row raw 복제 + rowAddr 수정)
    let template_raw = &xml[positions.template_row_start..positions.template_row_end];
    let mut clones = String::new();
    for i in 1..=clone_count {
        let new_addr = template_row_addr + i as u32;
        let cloned = rewrite_all_row_addrs(template_raw, new_addr);
        clones.push_str("\n");
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

    Ok(result)
}

/// 여러 테이블에 행 클론 적용 (순차)
pub fn patch_clone_rows_multi(
    xml: &str,
    clones: &[(usize, u32, usize)], // (table_index, template_row_addr, clone_count)
) -> Result<String> {
    let mut result = xml.to_string();
    for (table_index, row_addr, count) in clones {
        result = patch_clone_rows(&result, *table_index, *row_addr, *count)?;
    }
    Ok(result)
}

// ── 행 클론 내부 헬퍼 ──

struct RowClonePositions {
    tbl_rowcnt_pos: Option<usize>,  // "rowCnt=" 시작 바이트
    tbl_rowcnt_val: u32,
    template_row_start: usize,      // <hp:tr> 시작 바이트
    template_row_end: usize,        // </hp:tr> 끝 바이트
    subsequent_row_addrs: Vec<(usize, u32)>, // (바이트 위치, 현재 값) of rowAddr in subsequent rows
    found: bool,
}

/// raw XML 문자열에서 특정 속성의 위치와 값을 찾기
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

/// 행 XML 내 cellAddr의 rowAddr 위치를 수집 — nested table 내부는 skip
fn collect_row_addr_positions(
    row_raw: &str,
    global_offset: usize,
    positions: &mut Vec<(usize, u32)>,
) {
    let pattern = "rowAddr=\"";
    let mut search_from = 0;
    let mut tbl_depth = 0; // nested table 추적

    while search_from < row_raw.len() {
        // nested table 진입/퇴장 추적
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
                search_from = val_start + val_end + 1; // 닫는 따옴표 이후
            } else {
                break;
            }
        } else {
            break;
        }
    }
}

/// 행 XML 내 outer-level rowAddr 값만 new_addr로 교체 — nested table 내부 보존
fn rewrite_all_row_addrs(row_raw: &str, new_addr: u32) -> String {
    let pattern = "rowAddr=\"";
    let mut result = String::with_capacity(row_raw.len());
    let mut search_from = 0;
    let mut tbl_depth = 0;

    while search_from < row_raw.len() {
        // nested table 추적
        if row_raw[search_from..].starts_with("<hp:tbl") || row_raw[search_from..].starts_with("<tbl") {
            tbl_depth += 1;
        }
        if row_raw[search_from..].starts_with("</hp:tbl") || row_raw[search_from..].starts_with("</tbl") {
            tbl_depth -= 1;
        }

        if tbl_depth > 0 {
            // nested table 내부 — 그대로 복사
            result.push(row_raw.as_bytes()[search_from] as char);
            search_from += 1;
            continue;
        }

        if let Some(rel_pos) = row_raw[search_from..].find(pattern) {
            // nested table 시작이 pattern보다 앞에 있는지 확인
            let next_tbl = row_raw[search_from..].find("<hp:tbl").or_else(|| row_raw[search_from..].find("<tbl"));
            if let Some(tbl_pos) = next_tbl {
                if tbl_pos < rel_pos {
                    // nested table이 먼저 나옴 — 거기까지만 복사하고 depth 추적
                    result.push_str(&row_raw[search_from..search_from + tbl_pos]);
                    search_from += tbl_pos;
                    continue;
                }
            }

            // pattern 앞부분 + pattern 복사
            result.push_str(&row_raw[search_from..search_from + rel_pos + pattern.len()]);
            let val_start = search_from + rel_pos + pattern.len();

            // 기존 값 건너뛰고 새 값 삽입
            if let Some(val_end) = row_raw[val_start..].find('"') {
                result.push_str(&new_addr.to_string());
                search_from = val_start + val_end; // 닫는 따옴표 직전
            } else {
                search_from = val_start;
            }
        } else {
            break;
        }
    }
    // 나머지
    result.push_str(&row_raw[search_from..]);
    result
}
