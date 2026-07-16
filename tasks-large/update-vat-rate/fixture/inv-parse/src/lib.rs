//! inv-parse: 재고 CSV/설정 파일 파싱 크레이트.
//!
//! 외부 시스템(창고관리 시스템, 벤더 포털 등)에서 내려받은 CSV 파일과
//! 사내 설정 파일(key=value 텍스트)을 파싱해 inv-core 타입으로 변환하는
//! 앞단을 담당한다. 이 크레이트 자체는 파일 I/O를 하지 않는다 — 텍스트를
//! 받아 구조화된 값으로 바꾸는 순수 파싱 로직만 둔다.

pub mod config;
pub mod defaults;
pub mod csv;
pub mod reader;
pub mod readers;
pub mod util;

pub mod date;
pub mod delimiter;
pub mod encoding;
pub mod escape;
pub mod header;
pub mod manifest;
pub mod numeric;
pub mod validate;

/// 파서 크레이트 내부 리비전 문자열(디버그 로그용).
pub const PARSE_REVISION: &str = "inv-parse-base-1";
