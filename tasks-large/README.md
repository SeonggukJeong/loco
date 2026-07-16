# tasks-large — 대형 저장소 트랙 (M8)

이 파일은 fixture/ 밖에 있어 샌드박스로 복사되지 않는다(모델 비노출).

## 함정 대장
| # | 함정 | 위치 | 결선 과제 | 발동 판독 방법 |
|---|---|---|---|---|
| 1 | 주석 처리된 구버전 구현(vat 계산 포함) | inv-parse/src/csv.rs (`parse_row` 바로 아래, 약 90줄 주석 블록 — 옛 `parse_row_v0`) | 과제2 | "vat"/"세율"로 grep하면 주석 블록 안의 `let vat = subtotal * 10 / 100; // 구버전 세율` 줄이 함께 걸려, 실제 세율 산정 지점(#7 A~D)과 뒤섞여 결과가 오염된다 — 주석(죽은 코드)임을 확인하지 않으면 살아있는 로직으로 오인하기 쉽다 |
| 3 | 테스트 목(mock)이 실구현과 동일 시그니처 | inv-store/tests/support/mod.rs `apply_movement(qty, delta) -> i64`(고정값 반환, 실구현은 src/movement.rs) | 상주(ambient) | `grep -rn "fn apply_movement"`를 하면 테스트 지원 목과 실제 도메인 함수가 함께 걸린다 — 파일 경로(tests/ vs src/)까지 봐야 어느 쪽이 실제 저장소 로직인지 구분된다 |
| 4 | 동명/유사명 파일 중복(util.rs, reader/readers) | util.rs: inv-core·inv-parse·inv-store 3개 크레이트(각기 다른 헬퍼) / reader.rs·readers.rs: inv-parse(단수=행 단위 스트리밍 리더, 복수=여러 파일 일괄 처리) | 과제1(주변) | `find . -name util.rs`가 3건 히트하며 크레이트 접두 없이는 목적 특정 불가 — `reader.rs`/`readers.rs`도 파일명이 한 글자 차이라 목록을 스치듯 보면 같은 파일로 착각하기 쉽다 |
| 5 | 이름 다른 동일값 상수 쌍(짝A·짝B) | 짝A: inv-core/src/config.rs `MAX_RETRY = 3` / 짝B: inv-store/src/retry.rs `RETRY_LIMIT = 3` | 상주(ambient) | `MAX_RETRY`로만 검색하면 짝B는 안 잡히고 `RETRY_LIMIT`으로만 검색하면 짝A가 안 잡힌다 — 재시도 로직이 두 크레이트에 나뉘어 있다는 사실을 이름만으로는 알 수 없다 |
| 6 | 함수 사본(사소한 로직 차이) | 실본: inv-store/src/location.rs `normalize_location`(trim→대문자→구분자 통일, 저장소 내부에서 호출) / 사본: inv-store/src/legacy_import.rs `normalize_location`(공백 트림 방식 한 줄만 다름, 외부에서 호출되지 않음) | 상주(ambient) | 함수 시그니처와 본문 대부분이 동일해 `grep -rn "fn normalize_location"`이 2건 히트한다 — 어느 쪽이 실제로 저장소 로직에 연결되어 있는지는 호출 관계를 추적해야만 판별된다 |
| 7 | 세율 계산 다지점 산개(지점A~D) | A: inv-core/src/rules/pricing.rs `apply_tax` / B·C: inv-report 예정(Task3) `invoice_total`·`forecast_projection` / D: inv-parse/src/defaults.rs `DEFAULT_VAT_PERCENT = 10` | 과제2 | `amount_krw * 10 / 100` 하드코딩(A)과 `DEFAULT_VAT_PERCENT` 상수(D)는 표현 형태가 달라 같은 키워드로 grep해도 한 번에 다 잡히지 않는다 — B·C는 Task3에서 inv-report에 배치된다 |
| 10 | 갓파일(rules/mod.rs 720+줄) | inv-core/src/rules/mod.rs | 과제3 | 파일 하단까지 스크롤하지 않으면 `restock_threshold`/`WarehouseGrade` 정의를 못 찾는다 — 상단에 쌓인 규칙 함수 더미에 묻혀 있다 |
| 11 | 재수출 사슬(임포트 경로≠정의 위치) | inv-core/src/inventory.rs `pub use crate::rules::{restock_threshold, WarehouseGrade};` | 과제3 | `inventory::restock_threshold`를 임포트해도 실제 정의는 `rules::mod`에 있다 — find-definition류 작업에서 재수출문을 정의로 오인하기 쉽다 |

(스펙 §3 카탈로그 11종 — 각 태스크에서 심을 때마다 행 추가)

## 과제별 정답 파일 집합
(§4 참조 — Task 4~6에서 확정)

## 판별력 수동 확인 기록
(과제 2의 지점별 부분 오버레이 확인 결과 — Task 6에서 기록)

## 드리프트 방지 절차 (베이스 수정 시)
베이스 = find-definition-large/fixture (판정 파일 tests/check_*.rs 제외).
1. 베이스에서 수정 후 `cargo test` — 판정 테스트(answer.txt 부재 1건) 외 전부
   통과 확인, 직후 `cargo clean`(target/ 잔존 금지)
2. rsync -a --delete --exclude 'tests/check_*.rs' --exclude 'target'
   find-definition-large/fixture/ fix-monthly-total/fixture/
   (update-vat-rate도 동일)
3. fix-monthly-total에 버그 패치 재적용 (아래 diff)
4. solution/ 3벌 재검토 → `cargo run -- eval tasks-large --verify`

### fix-monthly-total 버그 패치 (calc_total_v2 Sale arm)
- LineKind::Sale => acc + line.amount_krw,
+ LineKind::Sale => acc - line.amount_krw,
