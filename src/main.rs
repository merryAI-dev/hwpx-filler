//! hwpx-fill CLI — 범용 HWPX 폼 채움
//!
//! 전략: 스트리밍 분석(어떤 HWPX든) + 스트리밍 패치(무손실) + ZIP 패치(바이너리 안전)

use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: hwpx-fill <template.hwpx> [output.hwpx]");
        eprintln!("       hwpx-fill <template.hwpx>                  # analyze only");
        eprintln!("       hwpx-fill <template.hwpx> <output.hwpx>    # analyze + fill test data");
        std::process::exit(1);
    }

    let template_path = &args[1];
    let output_path = args.get(2);

    println!("=== hwpx-filler (Rust) ===\n");

    // 1. Read
    println!("1. Reading {}...", template_path);
    let hwpx_bytes = fs::read(template_path).expect("파일 읽기 실패");

    // 2. Extract
    println!("2. Extracting...");
    let text_files = hwpx_filler::zipper::extract_text_files(&hwpx_bytes).expect("ZIP 실패");
    println!("   {} text files", text_files.len());

    let section0 = text_files.get("Contents/section0.xml").expect("section0.xml 없음");

    // 3. Analyze (스트리밍 — 어떤 HWPX든 동작)
    println!("3. Analyzing (streaming)...");
    let tables = hwpx_filler::stream_analyzer::analyze_xml(section0);

    for table in &tables {
        println!("\n   Table {}: {}rows × {}cols", table.index, table.row_count, table.col_count);
        for row in &table.rows {
            let cells: Vec<String> = row.cells.iter().map(|c| {
                let tag = if c.is_label { "L" } else { "D" };
                let text: String = c.text.chars().take(20).collect();
                let text = if c.text.chars().count() > 20 { format!("{}...", text) } else { text };
                format!("[{}:{}] {}\"{}\"", c.row, c.col, tag, text)
            }).collect();
            println!("     {}", cells.join(" | "));
        }
    }

    // 4. Extract fields
    let fields = hwpx_filler::stream_analyzer::extract_fields(&tables);
    println!("\n4. Fields detected: {}", fields.len());
    for f in &fields {
        println!("     {} → {} ({}%)", f.label, f.canonical_key, (f.confidence * 100.0) as u32);
    }

    // 5. Fill + save (if output path given)
    if let Some(output) = output_path {
        println!("\n5. Filling...");
        // 자동 매핑: 필드를 테스트 데이터로 채움
        let patches: Vec<(usize, u32, u32, String)> = fields.iter().filter_map(|f| {
            let val = match f.canonical_key.as_str() {
                "name" => "김보람",
                "email" => "boram@mysc.co.kr",
                "position" => "AXR팀장",
                "birth_date" => "1995.01.15",
                "phone" => "010-1234-5678",
                "experience" => "5년 3개월",
                "certification" => "정보처리기사",
                "task" => "AI 자동화",
                "period" => "26.01～26.12",
                "participation_rate" => "50%",
                _ => return None,
            };
            Some((f.table_index, f.row, f.col, val.to_string()))
        }).collect();

        let patched = hwpx_filler::patcher::patch_cells(section0, &patches).expect("패치 실패");
        println!("   {} cells patched", patches.len());

        let mut modified = std::collections::HashMap::new();
        modified.insert("Contents/section0.xml".to_string(), patched);
        let out = hwpx_filler::zipper::patch_hwpx(&hwpx_bytes, &modified).expect("ZIP 실패");

        fs::write(output, &out).expect("쓰기 실패");
        println!("   ✓ {} ({} bytes)", output, out.len());
    }

    println!("\n=== Done! ===");
}
