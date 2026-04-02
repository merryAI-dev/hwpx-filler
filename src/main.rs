//! hwpx-fill CLI
//!
//! 전략: 파싱은 serde(타입 안전), 출력은 원본 XML 스트리밍 패치(무손실)
//! - serde 파싱 → 테이블 구조 분석 + 필드 감지
//! - quick-xml Reader/Writer → 원본 XML 스트리밍하면서 <hp:t>만 교체
//! - 원본 ZIP 패치 → 바이너리 byte-perfect

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

    // 3. Parse section0.xml (serde — for structure analysis)
    let section0_xml = text_files.get("Contents/section0.xml")
        .expect("section0.xml not found");
    println!("3. Parsing section0.xml ({} bytes)...", section0_xml.len());

    let section = hwpx_filler::parser::parse_section(section0_xml)
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

    // 5. Patch original XML with test data (streaming — preserves everything)
    println!("\n5. Patching cells (streaming XML)...");
    let patches: Vec<(usize, u32, u32, String)> = vec![
        (1, 0, 1, "김보람".into()),
        (1, 0, 4, "AXR팀장".into()),
        (1, 0, 6, "1995.01.15".into()),
        (1, 1, 1, "boram@mysc.co.kr".into()),
        (1, 1, 4, "010-1234-5678".into()),
        (1, 2, 1, "          5년       3개월".into()),
        (1, 2, 4, "정보처리기사".into()),
        (1, 3, 1, "AI 자동화".into()),
        (1, 3, 3, "26. 01～26. 12".into()),
        (1, 3, 6, "     50 %".into()),
    ];

    let patched_xml = hwpx_filler::patcher::patch_cells(section0_xml, &patches)
        .expect("패치 실패");
    println!("   {} cells patched", patches.len());

    // 6. Validate patched XML (re-parse to verify)
    println!("\n6. Validating (re-parse test)...");
    match hwpx_filler::parser::parse_section(&patched_xml) {
        Ok(patched_section) => {
            let validation = hwpx_filler::validate::validate_section(&patched_section);
            if validation.valid {
                println!("   ✓ Re-parse + validation passed");
            } else {
                for err in &validation.errors {
                    println!("   ✗ {}", err);
                }
            }
        }
        Err(e) => println!("   ✗ Re-parse failed: {}", e),
    }

    // 7. Patch ZIP and save
    println!("\n7. Repacking HWPX (binary-safe)...");
    let mut modified = std::collections::HashMap::new();
    modified.insert("Contents/section0.xml".to_string(), patched_xml);

    let output = hwpx_filler::zipper::patch_hwpx(&hwpx_bytes, &modified)
        .expect("ZIP 패치 실패");

    fs::write(output_path, &output).expect("파일 쓰기 실패");
    println!("   ✓ Saved to {} ({} bytes)", output_path, output.len());

    println!("\n=== Done! ===");
}
