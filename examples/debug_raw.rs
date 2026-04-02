fn main() {
    let home = std::env::var("HOME").unwrap();
    let hwpx = std::fs::read(format!(
        "{}/Downloads/[서식5] 참여인력 이력사항_변민욱 (1).hwpx", home
    )).unwrap();
    let files = hwpx_filler::zipper::extract_text_files(&hwpx).unwrap();
    let xml = files.get("Contents/section0.xml").unwrap();

    let patched = hwpx_filler::patcher::patch_cell_text(xml, 0, 0, 0, "XXXX").unwrap();
    
    println!("Original contains XXXX: {}", xml.contains("XXXX"));
    println!("Patched contains XXXX: {}", patched.contains("XXXX"));
    println!("Strings equal: {}", xml == &patched);
    println!("Original len: {}, Patched len: {}", xml.len(), patched.len());
}
