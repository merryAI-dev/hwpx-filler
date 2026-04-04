//! 에러 타입

use thiserror::Error;

#[derive(Error, Debug)]
pub enum FillerError {
    #[error("XML 파싱 실패: {0}")]
    XmlParse(#[from] quick_xml::DeError),

    #[error("XML 직렬화 실패: {0}")]
    XmlSerialize(#[from] quick_xml::SeError),

    #[error("ZIP 처리 실패: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("IO 에러: {0}")]
    Io(#[from] std::io::Error),

    #[error("HWPX에 섹션 파일이 없습니다 (Contents/sectionN.xml)")]
    NoSection,

    #[error("테이블 {table}의 셀 ({row}, {col})을 찾을 수 없습니다")]
    CellNotFound { table: usize, row: u32, col: u32 },

    #[error("행 클론 실패: 테이블 {table}의 행 {row}을 찾을 수 없습니다")]
    RowNotFound { table: usize, row: u32 },

    #[error("검증 실패: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, FillerError>;
