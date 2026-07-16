# tasks-large — 대형 저장소 트랙 (M8)

이 파일은 fixture/ 밖에 있어 샌드박스로 복사되지 않는다(모델 비노출).

## 함정 대장
| # | 함정 | 위치 | 결선 과제 | 발동 판독 방법 |
|---|---|---|---|---|
| 4 | 동명 파일 중복(util.rs) | inv-core/src/util.rs (다른 inv-* 크레이트에도 동일명 예정, Task2~3) | 과제1(주변) | 파일명만으로 목적 특정 불가 — `find . -name util.rs`가 여러 건 히트, 크레이트 접두 없이 모듈 경로까지 봐야 어느 크레이트 것인지 특정된다 |
| 5 | 이름 다른 동일값 상수 쌍 — 짝A | inv-core/src/config.rs `MAX_RETRY = 3` (짝B `RETRY_LIMIT`는 inv-store 예정, Task2) | 상주(ambient) | `MAX_RETRY`로만 검색하면 짝B는 안 잡힌다 — 재시도 로직이 두 크레이트에 나뉘어 있다는 사실을 놓치기 쉽다 |
| 7 | 세율 계산 다지점 산개 — 지점A | inv-core/src/rules/pricing.rs `apply_tax` (나머지 지점B/C/D는 inv-report·inv-parse 예정, Task2~3) | 과제2 | `amount_krw * 10 / 100` 하드코딩 — "vat"/"tax"로 grep 시 지점A만 걸리고 B·C·D가 누락될 수 있다 |
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
