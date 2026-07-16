//! inv-report: 재고/물류 워크스페이스의 정산·보고서 크레이트.
//!
//! 원장(ledger) 라인을 받아 합계/월간 정산/인보이스/전망을 계산하고,
//! 그 결과를 사람이 읽는 보고서 형태로 조립한다. 조립 계층은 세 개의
//! 모듈(`report`/`reporting`/`report_v2`)로 나뉘어 있는데, 이는 정리되지
//! 않은 리팩터링 이력 때문이다 — 자세한 사용처는 각 모듈 문서를 참고.

pub mod totals;
pub mod monthly;
pub mod invoice;
pub mod forecast;
pub mod report;
pub mod reporting;
pub mod report_v2;
pub mod util;

pub mod summary;
pub mod trend;
pub mod comparison;
pub mod ranking;
pub mod period;
pub mod export;
pub mod variance;
pub mod snapshot;

/// 보고서 크레이트 내부 리비전 문자열(semver와 별개, 디버그 로그용).
pub const REPORT_REVISION: &str = "inv-report-base-1";
