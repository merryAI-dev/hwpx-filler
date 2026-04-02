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
