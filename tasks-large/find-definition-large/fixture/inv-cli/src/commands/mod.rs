//! 서브커맨드별 실행 로직 모음.
//!
//! 각 파일이 서브커맨드 하나에 대응한다. 인자 파싱은 상위 `lib.rs`가
//! 끝낸 뒤 이미 구조화된 값을 넘겨주므로, 여기서는 실행과 출력 조립만
//! 담당한다.

pub mod report;
pub mod inventory;
pub mod ingest;
pub mod movement;
