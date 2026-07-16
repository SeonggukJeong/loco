# tasks-large — 대형 저장소 트랙 (M8)

이 파일은 fixture/ 밖에 있어 샌드박스로 복사되지 않는다(모델 비노출).

## 함정 대장
| # | 함정 | 위치 | 결선 과제 | 발동 판독 방법 |
|---|---|---|---|---|
| 1 | 주석 처리된 구버전 구현(vat 계산 포함) | inv-parse/src/csv.rs (`parse_row` 바로 아래, 약 90줄 주석 블록 — 옛 `parse_row_v0`) | 과제2 | "vat"/"세율"로 grep하면 주석 블록 안의 `let vat = subtotal * 10 / 100; // 구버전 세율` 줄이 함께 걸려, 실제 세율 산정 지점(#7 A~D)과 뒤섞여 결과가 오염된다 — 주석(죽은 코드)임을 확인하지 않으면 살아있는 로직으로 오인하기 쉽다 |
| 2 | v1/v2 합계 함수 공존 + 호출부 혼재 | v1: inv-report/src/totals.rs `calc_total` / v2: inv-report/src/monthly.rs `calc_total_v2`(+ `monthly_total` 래퍼). v1은 inv-report/src/report.rs `build_report`와 inv-cli/src/commands/report.rs `execute_legacy` 딱 두 곳에서만 호출되고, 그 외 조립 경로(report_v2.rs 등)는 전부 v2 계열을 쓴다 | 과제1 | `calc_total(`로 grep하면 report.rs와 cli 레거시 경로 두 군데만 걸려 "아직 마이그레이션 안 끝난 예전 경로가 남아있다"는 신호처럼 보이지만, 실제로 몇 달 전 버그가 심긴 쪽은 최신 v2(`calc_total_v2`)다 — 오래돼 보이는 코드가 의심스럽다는 직관이 여기서는 반대로 작동한다 |
| 3 | 테스트 목(mock)이 실구현과 동일 시그니처 | inv-store/tests/support/mod.rs `apply_movement(qty, delta) -> i64`(고정값 반환, 실구현은 src/movement.rs) | 상주(ambient) | `grep -rn "fn apply_movement"`를 하면 테스트 지원 목과 실제 도메인 함수가 함께 걸린다 — 파일 경로(tests/ vs src/)까지 봐야 어느 쪽이 실제 저장소 로직인지 구분된다 |
| 4 | 동명/유사명 파일 중복(util.rs, reader/readers, report 3연속) | util.rs: inv-core·inv-parse·inv-store·inv-report·inv-cli 5개 크레이트(각기 다른 헬퍼) / reader.rs·readers.rs: inv-parse(단수=행 단위 스트리밍 리더, 복수=여러 파일 일괄 처리) / report.rs·reporting.rs·report_v2.rs: inv-report의 세 조립 계층(현재 경로·옛 텍스트 출력 계층·신규 v2 경로 — 서로 다른 파일이지만 이름이 한 단어씩만 다르다) | 과제1(주변) | `find . -name util.rs`가 5건 히트하며 크레이트 접두 없이는 목적 특정 불가 — `reader.rs`/`readers.rs`도 파일명이 한 글자 차이라 목록을 스치듯 보면 같은 파일로 착각하기 쉽다. `report*.rs` 3파일도 이름이 서로 한 단어 차이라(report/reporting/report_v2) 파일 목록만 훑어서는 어느 것이 현재 쓰이는 경로인지 구분되지 않는다 |
| 5 | 이름 다른 동일값 상수 쌍(짝A·짝B) | 짝A: inv-core/src/config.rs `MAX_RETRY = 3` / 짝B: inv-store/src/retry.rs `RETRY_LIMIT = 3` | 상주(ambient) | `MAX_RETRY`로만 검색하면 짝B는 안 잡히고 `RETRY_LIMIT`으로만 검색하면 짝A가 안 잡힌다 — 재시도 로직이 두 크레이트에 나뉘어 있다는 사실을 이름만으로는 알 수 없다 |
| 6 | 함수 사본(사소한 로직 차이) | 실본: inv-store/src/location.rs `normalize_location`(trim→대문자→구분자 통일, 저장소 내부에서 호출) / 사본: inv-store/src/legacy_import.rs `normalize_location`(공백 트림 방식 한 줄만 다름, 외부에서 호출되지 않음) | 상주(ambient) | 함수 시그니처와 본문 대부분이 동일해 `grep -rn "fn normalize_location"`이 2건 히트한다 — 어느 쪽이 실제로 저장소 로직에 연결되어 있는지는 호출 관계를 추적해야만 판별된다 |
| 7 | 세율 계산 다지점 산개(지점A~D) | A: inv-core/src/rules/pricing.rs `apply_tax` / B: inv-report/src/invoice.rs `invoice_total` / C: inv-report/src/forecast.rs `forecast_projection`(f64 배율 표기) / D: inv-parse/src/defaults.rs `DEFAULT_VAT_PERCENT = 10` | 과제2 | `amount_krw * 10 / 100` 하드코딩(A), `* 110 / 100`(B), `* 1.10` f64(C), `DEFAULT_VAT_PERCENT` 상수(D) — 네 지점 모두 표현 형태가 달라 같은 키워드로 grep해도 한 번에 다 잡히지 않는다 |
| 8 | 거짓 주석(반품이 이미 차감되어 들어온다) | inv-report/src/monthly.rs, `calc_total_v2` 바로 위 doc 주석("월간 정산 보고서 합계. 반품은 여기 들어오기 전 단계에서 이미 차감되어 들어오므로...") | 과제1 | 실제로는 원장 라인이 차감되지 않은 원본 그대로 들어온다 — 주석을 그대로 믿으면 `LineKind::Refund` 분기가 왜 있는지, 그 분기의 부호가 맞는지를 점검하지 않고 넘어가기 쉽다. 동일 주석이 "월간 정산 보고서" grep 앵커(#9와 별개 용도)도 겸한다 |
| 9 | 거짓 FIXME(엉뚱한 함수를 의심하게 유도) | inv-report/src/totals.rs, `calc_total`(v1) 바로 위 `// FIXME: 반품 부호 처리가 의심스럽다 — 확인 필요 (2024-11)` | 과제1 | "FIXME"/"의심"으로 grep하면 v1 `calc_total` 위 주석이 걸리지만, 그 함수의 반품 부호 처리는 정상이다 — 실제 버그는 v2 `calc_total_v2`의 `Sale` 분기(과제1이 심음)에 있다. 오래된 FIXME 날짜(2024-11)가 신뢰도를 더해 잘못된 파일을 고치도록 유도한다 |
| 10 | 갓파일(rules/mod.rs 720+줄) | inv-core/src/rules/mod.rs | 과제3 | 파일 하단까지 스크롤하지 않으면 `restock_threshold`/`WarehouseGrade` 정의를 못 찾는다 — 상단에 쌓인 규칙 함수 더미에 묻혀 있다 |
| 11 | 재수출 사슬(임포트 경로≠정의 위치) | 선언: inv-core/src/inventory.rs `pub use crate::rules::{restock_threshold, WarehouseGrade};` / 발동면: inv-cli/src/commands/report.rs가 `use inv_core::inventory::{restock_threshold, WarehouseGrade};`로 재수출 경로를 통해서만 가져다 쓴다(`rules::restock_threshold`를 직접 import하는 곳은 워크스페이스 어디에도 없음) | 과제3 | `inventory::restock_threshold`를 임포트해도 실제 정의는 `rules::mod`에 있다 — find-definition류 작업에서 재수출문을 정의로 오인하기 쉽다. inv-cli의 report 커맨드 호출부를 따라가도 재수출 지점에서 멈추기 쉬워, 정의를 찾으려면 재수출문 자체를 한 번 더 타고 들어가야 한다 |

(스펙 §3 카탈로그 11종 — 전부 배치 완료, Task3 기준)

forecast.rs의 `required_net_for_target`가 옛 1.10 제수를 유지한다 — 판정 비대상 유사 VAT
지점으로, update-vat-rate 실행 시 모델이 건드려도 pass/fail에 무영향(측정 해석 시 참고).

## 과제별 정답 파일 집합
(§4 참조 — Task 4~6에서 확정)

### 트리 노출 판독 (§3 의도 변수, Task7)
시스템 프롬프트에 주입되는 프로젝트 트리는 `src/agent/prompt.rs`의 `project_tree()`가
`src/tools/list_files.rs`의 `walk_entries`/`walker`(`ignore::WalkBuilder`, `require_git(false)`,
`sort_by_file_name`=OsStr 바이트 비교 — 디렉터리별 알파벳 정렬 pre-order DFS)를 재사용해
만든다. 상한이 **둘** 걸려 있다: `TREE_DEPTH=3`(fixture 루트 기준 경로 세그먼트 3단계까지만
— 예: `inv-core/src/rules/`는 depth3라 목록에 뜨지만 그 안의 `mod.rs`/`pricing.rs`는 depth4라
알파벳 순서와 무관하게 무조건 제외) + `TREE_MAX_ENTRIES=100`(depth 필터를 통과한 항목을
정렬 순서대로 최대 100개, 넘으면 `[tree truncated]` 마커 추가). 판독 방법: src/를 수정하지
않고 동일 `ignore` 크레이트(0.4.27, 로컬 캐시)로 `walker`/`walk_entries`/`project_tree`를
세션 스크래치 바이너리에 그대로 복제해 각 fixture 루트에 대해 실행하고, 정답 파일 경로를
정확히(전체 라인 매치) grep해 확인했다.

| 과제 | 정답 파일 | 트리 노출 | 비고 |
|---|---|---|---|
| 과제1 | inv-report/src/monthly.rs | O | depth3, 트리 99줄(100개 상한 미도달, 자연 종료) |
| 과제2 | inv-core/src/rules/pricing.rs | X | depth4 — depth 상한으로 무조건 제외(알파벳 순서 무관) |
| 과제2 | inv-report/src/invoice.rs | O | depth3, 100개 절삭 지점보다 훨씬 앞에서 등장 |
| 과제2 | inv-report/src/forecast.rs | O | depth3, 100개 절삭 지점보다 훨씬 앞에서 등장 |
| 과제2 | inv-parse/src/defaults.rs | O | depth3, 100개 절삭 지점보다 훨씬 앞에서 등장 |
| 과제3 | inv-core/src/rules/mod.rs | X | depth4 — depth 상한으로 무조건 제외(알파벳 순서 무관) |

**플랜 가정과의 괴리**: 플랜(§3)은 "inv-parse/inv-report/inv-store가 알파벳 순서상 100개
상한을 넘어갈 수 있다"고 가정했다. 실측 결과 세 과제 중 update-vat-rate만 실제로 100개
상한에 걸려 절삭되지만(check_vat_*.rs 판정 파일 3벌이 추가되어 트리가 101줄 — 100개+
`[tree truncated]`), 절삭 지점은 `inv-store/tests/store_basic.rs` 직후로, update-vat-rate
정답 파일 중 depth≤3인 3개(invoice.rs·forecast.rs·defaults.rs)는 그보다 훨씬 앞에서
이미 노출된 뒤고, pricing.rs는 depth4라 절삭 여부와 무관하게 애초에 노출되지 않는다.
즉 depth3 정답 파일은 세 과제 어디에서도 개수 상한에 걸려 잘린 적이 없다 — 실제로
노출을 좌우하는 것은 개수
상한이 아니라 **depth=3 재귀 상한**이다: `inv-core/src/rules/` 아래(mod.rs, pricing.rs)처럼
src/ 밑에 한 단계 더 들어간 파일만 정확히, 그리고 항상 제외된다. 나머지 5개 정답 파일
(전부 `inv-*/src/*.rs`, depth3)은 100개 상한 도달 여부와 무관하게 전부 노출된다.

## 판별력 수동 확인 기록
과제2(update-vat-rate)의 지점별 부분 오버레이 확인 결과(Task 6). solution 4파일 중
1개만 제외한 부분 오버레이를 fixture 사본(스크래치, tasks-large/ 밖)에 적용하고
`cargo test`를 실행해, 제외된 지점의 판정 테스트만 단독으로 실패하는지 확인했다.
사전 점검으로 solution 4파일 전부를 적용한 라운드(round0-full)도 실행해 전체
통과(0 failed)를 확인했다.

| 제외 파일 | 결과 | 실패한 테스트 | 기타 실패 |
|---|---|---|---|
| inv-core/src/rules/pricing.rs | FAIL (예상대로) | `apply_tax_uses_12_percent` | 없음 |
| inv-report/src/invoice.rs | FAIL (예상대로) | `invoice_total_uses_12_percent` | 없음(같은 파일의 `forecast_projection_uses_12_percent`는 통과) |
| inv-report/src/forecast.rs | FAIL (예상대로) | `forecast_projection_uses_12_percent` | 없음(같은 파일의 `invoice_total_uses_12_percent`는 통과) |
| inv-parse/src/defaults.rs | FAIL (예상대로) | `default_config_vat_is_12` | 없음 |

4/4 라운드 모두 기대한 판정 테스트 정확히 1개만 실패했고, 다른 판정 테스트나
베이스 테스트에는 영향이 없었다(4지점이 서로 독립적으로 판별됨을 확인).

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
