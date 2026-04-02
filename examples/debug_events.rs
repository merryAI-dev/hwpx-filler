use quick_xml::Reader;
use quick_xml::events::Event;

fn main() {
    let home = std::env::var("HOME").unwrap();
    let hwpx = std::fs::read(format!(
        "{}/Downloads/[서식5] 참여인력 이력사항_변민욱 (1).hwpx", home
    )).unwrap();
    let files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();
    let xml = files.get("Contents/section0.xml").unwrap();

    let mut reader = Reader::from_str(xml);
    let mut tbl_count = 0;
    let mut tbl_depth = 0;
    let mut in_tbl0 = false;
    let mut event_count = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");
                if name == "tbl" {
                    if tbl_depth == 0 {
                        println!("  tbl #{} START (depth before={})", tbl_count, tbl_depth);
                        if tbl_count == 0 { in_tbl0 = true; }
                        tbl_count += 1;
                    }
                    tbl_depth += 1;
                }
                if in_tbl0 && (name == "tc" || name == "t") {
                    println!("    <{}>", name);
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");
                if in_tbl0 && name == "cellAddr" {
                    let attrs: Vec<String> = e.attributes()
                        .filter_map(|a| a.ok())
                        .map(|a| format!("{}={}", 
                            std::str::from_utf8(a.key.as_ref()).unwrap_or("?"),
                            std::str::from_utf8(&a.value).unwrap_or("?")))
                        .collect();
                    println!("    <cellAddr {} />", attrs.join(" "));
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_tbl0 {
                    let text = e.unescape().unwrap_or_default();
                    if !text.trim().is_empty() {
                        println!("    text: '{}'", text.chars().take(30).collect::<String>());
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");
                if name == "tbl" {
                    tbl_depth -= 1;
                    if tbl_depth == 0 {
                        in_tbl0 = false;
                        println!("  tbl END\n");
                        if tbl_count >= 2 { break; } // 처음 2개만
                    }
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        event_count += 1;
        if event_count > 5000 { break; }
    }
}
