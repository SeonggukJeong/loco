//! inv-parse 파싱 시 사용할 기본값 모음.
//!
//! 설정 텍스트에 값이 없을 때 채워 넣는 폴백들이다. 값을 바꾸면 설정
//! 파일에 해당 키가 없는 모든 배치의 동작이 함께 바뀌므로, 변경 시
//! 영향 범위를 넓게 확인해야 한다.

/// 부가세율(%) 기본값. 설정 텍스트에 `vat_percent` 키가 없을 때 사용한다.
pub const DEFAULT_VAT_PERCENT: u32 = 12;

/// 창고 수 기본값. 설정 텍스트에 `warehouse_count` 키가 없을 때 사용한다.
pub const DEFAULT_WAREHOUSE_COUNT: u32 = 1;

/// 통화 코드 기본값. 설정 텍스트에 `currency` 키가 없을 때 사용한다.
pub const DEFAULT_CURRENCY_CODE: &str = "KRW";

/// CSV 필드 구분자 기본값.
pub const DEFAULT_DELIMITER: char = ',';

/// 헤더 행 존재 여부 기본값(사내 CSV는 대부분 헤더를 포함해 내려온다).
pub const DEFAULT_HAS_HEADER: bool = true;

/// 헤더 불일치를 의심하기 시작하는 컬럼 수 상한.
pub const MAX_EXPECTED_COLUMNS: usize = 32;

/// 배치 하나에서 허용하는 최대 오류 행 수(이 값을 넘으면 배치 자체를
/// 의심스러운 입력으로 간주해 상위 계층이 재확인하도록 한다).
pub const MAX_TOLERATED_ERROR_ROWS: usize = 500;

/// 날짜 필드가 비어 있을 때 대체할 기본 문자열("확인 필요"라는 뜻으로
/// 보고서에 그대로 노출될 수 있다).
pub const DEFAULT_UNKNOWN_DATE: &str = "0000-00-00";

/// SKU 필드가 비어 있을 때 대체할 플레이스홀더(정상 SKU 포맷과 절대
/// 겹치지 않도록 언더스코어를 포함한다).
pub const DEFAULT_UNKNOWN_SKU: &str = "UNKNOWN_SKU";

/// 카테고리 필드 기본값.
pub const DEFAULT_CATEGORY: &str = "UNSPECIFIED";

/// 설정 텍스트에서 알 수 없는 키를 만났을 때 로그에 남길지 여부의 기본값.
pub const DEFAULT_WARN_ON_UNKNOWN_KEY: bool = false;
