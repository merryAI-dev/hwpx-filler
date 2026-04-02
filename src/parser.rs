//! HWPX XML 파서 — quick-xml + serde 기반
//!
//! openhwp처럼 serde 역직렬화를 사용하되,
//! 알 수 없는 필드는 `#[serde(flatten)]`으로 보존해서
//! 재직렬화 시 데이터 손실 없음.

use crate::error::Result;
use crate::model::Section;

/// section0.xml 문자열을 Section 구조체로 파싱
pub fn parse_section(xml: &str) -> Result<Section> {
    let section: Section = quick_xml::de::from_str(xml)?;
    Ok(section)
}

/// Section 구조체를 XML 문자열로 직렬화
pub fn serialize_section(section: &Section) -> Result<String> {
    let xml = quick_xml::se::to_string(section)?;
    // quick-xml은 XML 선언을 안 넣으므로 추가
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?>{}"#,
        xml
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_section() {
        let xml = r#"<sec><p id="0" paraPrIDRef="1"><run charPrIDRef="2"><t>hello</t></run></p></sec>"#;
        let section = parse_section(xml).unwrap();
        assert_eq!(section.paragraphs.len(), 1);
        assert_eq!(section.paragraphs[0].runs.len(), 1);
        let text = match &section.paragraphs[0].runs[0].contents[0] {
            crate::model::RunContent::Text(s) => s.clone(),
            _ => panic!("expected Text"),
        };
        assert_eq!(text, "hello");
    }

    #[test]
    fn parse_run_with_mixed_content() {
        // Run에 secPr + t가 섞인 경우 — openhwp의 $value 패턴
        let xml = r#"<sec><p id="0"><run charPrIDRef="1"><secPr id=""/><t>text</t></run></p></sec>"#;
        let section = parse_section(xml).unwrap();
        let run = &section.paragraphs[0].runs[0];
        assert_eq!(run.contents.len(), 2);
        assert!(matches!(&run.contents[0], crate::model::RunContent::Other { tag, .. } if tag == "secPr"));
        assert!(matches!(&run.contents[1], crate::model::RunContent::Text(s) if s == "text"));
    }
}
