//! inv-core: 재고/물류 워크스페이스의 코어 도메인 크레이트.
//!
//! 원장(ledger) 라인, 재고 스냅샷, 배분/이동/유효성 규칙, SKU·창고 모델을
//! 제공한다. inv-parse/inv-store/inv-report/inv-cli는 모두 이 크레이트의
//! 타입을 재사용한다 — 워크스페이스에서 유일하게 외부에 의존하지 않는
//! "leaf" 크레이트다.

pub mod config;
pub mod ledger;
pub mod inventory;
pub mod rules;
pub mod sku;
pub mod warehouse;
pub mod util;

pub mod alert;
pub mod audit;
pub mod batch;
pub mod category;
pub mod currency;
pub mod history;
pub mod metrics;
pub mod schedule;
pub mod units;
pub mod vendor;

/// 코어 크레이트 내부 리비전 문자열(semver와 별개, 디버그 로그용).
pub const CORE_REVISION: &str = "inv-core-base-1";
