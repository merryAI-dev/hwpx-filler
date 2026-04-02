//! hwpx-fill CLI — 통합 API(filler) 사용

use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: hwpx-fill <template.hwpx> [output.hwpx]");
        std::process::exit(1);
    }

    let template_path = &args[1];
    let output_path = args.get(2);

    println!("=== hwpx-filler (Rust) ===\n");

    // 1. Read + extract
    println!("1. Reading {}...", template_path);
    let hwpx_bytes = fs::read(template_path).expect("파일 읽기 실패");
    let text_files = hwpx_filler::zipper::extract_text_files(&hwpx_bytes).expect("ZIP 실패");
    let section0 = text_files.get("Contents/section0.xml").expect("section0.xml 없음");

    // 2. Analyze (filler 통합 API)
    println!("2. Analyzing...");
    let result = hwpx_filler::filler::analyze(section0);

    for table in &result.tables {
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

    println!("\n   Fields: {}", result.fields.len());
    for f in &result.fields {
        println!("     {} → {} ({}%) [{:?}]", f.label, f.canonical_key, (f.confidence * 100.0) as u32, f.content_type);
    }

    // 3. Fill + validate + save
    if let Some(output) = output_path {
        println!("\n3. Filling...");
        let patches: Vec<(usize, u32, u32, String)> = result.fields.iter().filter_map(|f| {
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

        let patched = hwpx_filler::filler::fill(section0, &patches).expect("패치 실패");
        println!("   {} cells patched", patches.len());

        // 4. Validate (파이프라인에 연결!)
        println!("\n4. Validating...");
        let validation = hwpx_filler::filler::validate_patched(&patched);
        if validation.valid {
            println!("   ✓ Valid");
        } else {
            for err in &validation.errors {
                println!("   ✗ {}", err);
            }
        }

        // 5. Repack
        println!("\n5. Repacking...");
        let mut modified = std::collections::HashMap::new();
        modified.insert("Contents/section0.xml".to_string(), patched);
        let out = hwpx_filler::zipper::patch_hwpx(&hwpx_bytes, &modified).expect("ZIP 실패");
        fs::write(output, &out).expect("쓰기 실패");
        println!("   ✓ {} ({} bytes)", output, out.len());
    }

    println!("\n=== Done! ===");
}
