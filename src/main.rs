//! hwpx-fill CLI — 서식5 테스트
//!
//! Usage: hwpx-fill <template.hwpx> <output.hwpx>

use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: hwpx-fill <template.hwpx> <output.hwpx>");
        std::process::exit(1);
    }

    let template_path = &args[1];
    let output_path = &args[2];

    println!("=== hwpx-filler (Rust) ===\n");

    // 1. Read HWPX
    println!("1. Reading {}...", template_path);
    let hwpx_bytes = fs::read(template_path).expect("파일 읽기 실패");

    // 2. Extract text files
    println!("2. Extracting text files...");
    let text_files = hwpx_filler::zipper::extract_text_files(&hwpx_bytes)
        .expect("ZIP 추출 실패");
    println!("   {} text files extracted", text_files.len());

    // 3. Parse section0.xml
    let section0_xml = text_files.get("Contents/section0.xml")
        .expect("section0.xml not found");
    println!("3. Parsing section0.xml ({} bytes)...", section0_xml.len());

    let mut section = hwpx_filler::parser::parse_section(section0_xml)
        .expect("XML 파싱 실패");

    // 4. Analyze tables
    println!("4. Analyzing form structure...");
    let tables = hwpx_filler::filler::collect_tables(&section);
    for (i, table) in tables.iter().enumerate() {
        let analysis = hwpx_filler::analyzer::analyze_table(table, i);
        println!("   Table {}: {} fields found", i, analysis.fields.len());
        for field in &analysis.fields {
            println!("     {} → {} ({}%)",
                field.label, field.canonical_key,
                (field.confidence * 100.0) as u32
            );
        }
    }

    // 5. Fill with test data
    println!("\n5. Filling test data...");
    let fields = vec![
        hwpx_filler::filler::FillField { table_index: 1, row: 0, col: 1, value: "김보람".into() },
        hwpx_filler::filler::FillField { table_index: 1, row: 0, col: 4, value: "AXR팀장".into() },
        hwpx_filler::filler::FillField { table_index: 1, row: 0, col: 6, value: "1995.01.15".into() },
        hwpx_filler::filler::FillField { table_index: 1, row: 1, col: 1, value: "boram@mysc.co.kr".into() },
        hwpx_filler::filler::FillField { table_index: 1, row: 1, col: 4, value: "010-1234-5678".into() },
    ];

    let warnings = hwpx_filler::filler::fill_fields(&mut section, &fields)
        .expect("채움 실패");
    for w in &warnings {
        println!("   WARNING: {}", w);
    }
    if warnings.is_empty() {
        println!("   {} fields filled successfully", fields.len());
    }

    // 6. Validate
    println!("\n6. Validating...");
    let validation = hwpx_filler::validate::validate_section(&section);
    if validation.valid {
        println!("   ✓ Structure valid");
    } else {
        for err in &validation.errors {
            println!("   ✗ {}", err);
        }
    }

    // 7. Serialize + patch ZIP
    println!("\n7. Repacking HWPX (binary-safe)...");
    let new_xml = hwpx_filler::parser::serialize_section(&section)
        .expect("직렬화 실패");

    let mut modified = std::collections::HashMap::new();
    modified.insert("Contents/section0.xml".to_string(), new_xml);

    let output = hwpx_filler::zipper::patch_hwpx(&hwpx_bytes, &modified)
        .expect("ZIP 패치 실패");

    fs::write(output_path, &output).expect("파일 쓰기 실패");
    println!("   ✓ Saved to {} ({} bytes)", output_path, output.len());

    println!("\n=== Done! ===");
}
