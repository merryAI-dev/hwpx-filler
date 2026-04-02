//! # hwpx-filler
//!
//! Universal HWPX form-filling engine.
//!
//! openhwp의 타입 시스템에서 영감을 받되, 폼 자동 채움에 특화:
//! - 양식 구조 자동 분석 (label/data 셀 식별)
//! - 서식 보존 텍스트 교체 (구조체 수준 조작)
//! - 동적 행 클론 + 주소 재계산
//! - 바이너리 안전 ZIP 패치
//!
//! ## 설계 원칙
//! 1. regex 금지 — quick-xml + serde로 타입 안전 파싱
//! 2. 원본 ZIP 패치 — 바이너리 byte-perfect 보존
//! 3. 구조체 수준 조작 — XML 문자열이 아닌 Rust 구조체 직접 수정 후 재직렬화

pub mod error;
pub mod model;
pub mod parser;
pub mod filler;
pub mod analyzer;
pub mod zipper;
pub mod patcher;
pub mod stream_analyzer;
pub mod validate;
#[cfg(feature = "wasm")]
pub mod wasm;
