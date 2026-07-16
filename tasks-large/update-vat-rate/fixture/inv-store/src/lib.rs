//! inv-store: 재고 저장소 크레이트.
//!
//! 파싱된 재고 데이터(inv-parse)를 실제로 보관하고, 재고 이동/조회/락
//! 등의 저장소 연산을 제공한다. 인메모리 저장소와 파일 포맷(인코딩/디코딩
//! 로직만 — 실제 파일 I/O는 상위 계층인 inv-cli의 몫이다) 두 가지 구현을
//! 담는다.

pub mod file;
pub mod legacy_import;
pub mod location;
pub mod memory;
pub mod movement;
pub mod retry;
pub mod util;

pub mod audit_trail;
pub mod capacity;
pub mod index;
pub mod lock;
pub mod query;
pub mod reservation;
pub mod snapshot;
pub mod transfer;

/// 저장소 크레이트 내부 리비전 문자열(디버그 로그용).
pub const STORE_REVISION: &str = "inv-store-base-1";
