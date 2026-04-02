fn main() {
    let home = std::env::var("HOME").unwrap();
    let hwpx = std::fs::read(format!(
        "{}/Downloads/[서식5] 참여인력 이력사항_변민욱 (1).hwpx", home
    )).unwrap();
    let files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();
    let xml = files.get("Contents/section0.xml").unwrap();

    // stream_analyzer가 보는 테이블
    let tables = hwpx_filler::stream_analyzer::analyze_xml(xml);
    println!("stream_analyzer sees {} tables:", tables.len());
    for t in &tables {
        let first_text = t.rows.first()
            .and_then(|r| r.cells.first())
            .map(|c| c.text.chars().take(20).collect::<String>())
            .unwrap_or_default();
        println!("  Table {}: {}rows × {}cols, first='{}'", t.index, t.row_count, t.col_count, first_text);
    }

    // patcher가 보는 테이블 — quick-xml로 tbl 태그 카운트
    use quick_xml::Reader;
    use quick_xml::events::Event;
    let mut reader = Reader::from_str(xml);
    let mut tbl_count = 0;
    let mut depth = 0;
    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");
                if name == "tbl" {
                    if depth == 0 {
                        println!("\npatcher tbl #{}: depth={}, offset={}", tbl_count, depth, reader.buffer_position());
                    }
                    depth += 1;
                    if depth == 1 { tbl_count += 1; }
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");
                if name == "tbl" { depth -= 1; }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
    }
    println!("\npatcher sees {} top-level tbls", tbl_count);
}
