//! 바이너리 안전 ZIP 처리
//!
//! openhwp 대비 발전: 원본 ZIP 패치 방식
//! - 텍스트 파일만 추출/수정
//! - 바이너리 파일은 원본 ZIP에서 그대로 유지
//! - 재패킹 시 수정된 파일만 교체

use crate::error::Result;
use std::io::{Read, Write, Cursor};
use zip::ZipArchive;
use zip::write::SimpleFileOptions;

/// HWPX에서 텍스트 파일만 추출
pub fn extract_text_files(hwpx_bytes: &[u8]) -> Result<std::collections::HashMap<String, String>> {
    let reader = Cursor::new(hwpx_bytes);
    let mut archive = ZipArchive::new(reader)?;
    let mut files = std::collections::HashMap::new();

    let text_extensions = ["xml", "hpf", "txt", "rdf"];

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();

        let ext = name.rsplit('.').next().unwrap_or("");
        let is_text = text_extensions.contains(&ext) || name == "mimetype";

        if is_text && !entry.is_dir() {
            let mut content = String::new();
            entry.read_to_string(&mut content)?;
            files.insert(name, content);
        }
    }

    Ok(files)
}

/// 원본 ZIP을 기반으로 수정된 텍스트 파일만 교체
pub fn patch_hwpx(
    original: &[u8],
    modified: &std::collections::HashMap<String, String>,
) -> Result<Vec<u8>> {
    let reader = Cursor::new(original);
    let mut archive = ZipArchive::new(reader)?;

    let mut output = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(Cursor::new(&mut output));

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let name = entry.name().to_string();

            if entry.is_dir() {
                writer.add_directory(&name, SimpleFileOptions::default())?;
                continue;
            }

            let options = if name == "mimetype" {
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored)
            } else {
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)
            };

            writer.start_file(&name, options)?;

            if let Some(new_content) = modified.get(&name) {
                // 수정된 파일 — 새 내용 쓰기
                writer.write_all(new_content.as_bytes())?;
            } else {
                // 원본 그대로 복사 — 바이너리 파일 byte-perfect
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf)?;
                writer.write_all(&buf)?;
            }
        }

        writer.finish()?;
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_preserves_binary() {
        // mimetype + text file + fake binary를 가진 ZIP 생성
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(Cursor::new(&mut buf));
            writer.start_file("mimetype", SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)).unwrap();
            writer.write_all(b"application/hwpx+zip").unwrap();

            writer.start_file("Contents/section0.xml", SimpleFileOptions::default()).unwrap();
            writer.write_all(b"<sec><p><run><t>original</t></run></p></sec>").unwrap();

            writer.start_file("Preview/PrvImage.png", SimpleFileOptions::default()).unwrap();
            writer.write_all(&[0x89, 0x50, 0x4E, 0x47, 0xFF, 0x00, 0xAB]).unwrap();

            writer.finish().unwrap();
        }

        // section0.xml만 수정
        let mut modified = std::collections::HashMap::new();
        modified.insert(
            "Contents/section0.xml".to_string(),
            "<sec><p><run><t>replaced</t></run></p></sec>".to_string(),
        );

        let patched = patch_hwpx(&buf, &modified).unwrap();

        // 검증: PNG가 byte-perfect인지
        let reader = Cursor::new(&patched);
        let mut archive = ZipArchive::new(reader).unwrap();

        let mut png_entry = archive.by_name("Preview/PrvImage.png").unwrap();
        let mut png_bytes = Vec::new();
        png_entry.read_to_end(&mut png_bytes).unwrap();
        assert_eq!(png_bytes, vec![0x89, 0x50, 0x4E, 0x47, 0xFF, 0x00, 0xAB]);

        // 검증: XML이 교체됐는지
        let mut xml_entry = archive.by_name("Contents/section0.xml").unwrap();
        let mut xml_content = String::new();
        xml_entry.read_to_string(&mut xml_content).unwrap();
        assert!(xml_content.contains("replaced"));
    }
}
