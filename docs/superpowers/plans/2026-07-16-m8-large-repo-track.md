# M8 대형 저장소 트랙 구현 플랜

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `tasks-large/`(대형 워크스페이스 픽스처 1벌×3사본 + 과제 3개)를 만들어 `--verify` 3/3을 통과시키고, gemma·ornith 8K 베이스라인 + ornith 32K 민감도 + 실측 사양표를 측정해 실패 분류와 M9 요구사항 후보를 문서화한다.

**Architecture:** 하니스 코드 변경 0. 베이스 워크스페이스(재고/물류 `inv-*` 5크레이트, 10~20K LOC)를 과제 3(find-definition-large)의 fixture로 제작하고, 과제 1·2는 그 사본에 버그/판정을 심는다. 함정 11종은 베이스에 상주하고 함정 대장은 `tasks-large/README.md`(fixture/ 밖)에 기록한다.

**Tech Stack:** Rust(픽스처는 edition 2021·의존성 0), 기존 eval 하니스(`cargo run -- eval`), LM Studio.

**스펙:** `docs/superpowers/specs/2026-07-16-m8-large-repo-track-design.md` (리뷰 3라운드 Ready=Yes). **기준 커밋: `23dc1fd`.**

## Global Constraints

- 하니스 코드(src/) 변경 0 — src/·기존 tasks/ 를 건드리는 태스크는 없다. 최종 게이트에서 `git diff <기준커밋> -- src tasks`가 빈 값이어야 한다
- 픽스처는 외부 의존성 0 (워크스페이스 내부 path 의존만), 루트 `.gitignore`에 `/target` 필수
- 픽스처 식별자는 영문, 주석은 일부 한국어 혼용(사내 코드 현실성 — 함정 앵커·grep 대상)
- 모든 크레이트에 `tests/` 디렉토리 필수 (protected 목록의 로더 존재-검사 대응, 스펙 §4)
- **베이스 테스트 금지 구역**: 베이스(공통) 테스트는 ①`monthly_total`/`calc_total_v2`의 합계값 ②부가세율 파생값(`apply_tax`/`invoice_total`/`forecast_projection`/`DEFAULT_VAT_PERCENT`)을 절대 단정하지 않는다 — 과제 1의 심은 버그·과제 2의 세율 변경 후에도 베이스 테스트는 전부 통과해야 하기 때문(판정 신호 = 판정 테스트만)
- `tasks-large/` 변경 후에는 반드시 `cargo run -- eval tasks-large --verify` 실행 (CLAUDE.md의 tasks/ 규칙과 동일)
- 커밋은 conventional commits(제목 한국어 가능), 태스크당 1커밋 이상
- 측정(Task 10~12)은 서브에이전트 위임 금지·모델 로드는 사용자 협조·측정 중 cargo 빌드 금지 (docs/baselines.md 프로토콜)
- **"핀 고정 인터페이스" 절과 "공통 protected 목록"은 전 태스크의 요구사항에 암묵 포함** — 태스크 단위로 디스패치할 때 반드시 태스크 본문과 함께 제공할 것

## 핀 고정 인터페이스 (전 태스크 공통 참조)

플랜 전체에서 아래 시그니처·값은 바이트 단위로 고정이다. 판정 테스트·버그 패치·솔루션이 전부 여기에 걸린다.

```rust
// inv-core/src/ledger.rs
pub enum LineKind { Sale, Refund, Adjustment }
pub struct LedgerLine { pub sku: String, pub kind: LineKind, pub amount_krw: i64, pub adj_krw: i64 }
impl LedgerLine { pub fn adjustment_krw(&self) -> i64 { self.adj_krw } }

// inv-core/src/rules/pricing.rs — 함정7 지점A
pub fn apply_tax(amount_krw: i64) -> i64 { amount_krw + amount_krw * 10 / 100 }

// inv-core/src/rules/mod.rs 하단(700줄+ 갓파일) — 함정10·11, 과제3 정답 위치
pub enum WarehouseGrade { Central, Regional, Local }
pub fn restock_threshold(daily_avg: u32, lead_days: u32, grade: WarehouseGrade) -> u32 {
    let base = daily_avg.saturating_mul(lead_days);
    match grade {
        WarehouseGrade::Central => base.saturating_add(daily_avg.saturating_mul(7)),
        WarehouseGrade::Regional => base.saturating_add(daily_avg.saturating_mul(3)),
        WarehouseGrade::Local => base,
    }
}

// inv-core/src/inventory.rs — 함정11 재수출 사슬(임포트 경로≠정의 위치)
pub use crate::rules::{restock_threshold, WarehouseGrade};

// inv-core/src/config.rs — 함정5 짝A
pub const MAX_RETRY: u32 = 3;

// inv-parse/src/defaults.rs — 함정7 지점D
pub const DEFAULT_VAT_PERCENT: u32 = 10;

// inv-parse/src/config.rs
pub struct Config { pub vat_percent: u32, pub warehouse_count: u32, pub currency: String }
pub fn parse_config(text: &str) -> Config   // 누락 키는 defaults.rs 값으로 채움

// inv-store/src/retry.rs — 함정5 짝B (짝A와 딴 이름·같은 값·딴 크레이트)
pub const RETRY_LIMIT: u32 = 3;

// inv-store/src/movement.rs — 실구현
pub fn apply_movement(qty: i64, delta: i64) -> i64
// inv-store/tests/support/mod.rs — 함정3: 같은 이름·같은 시그니처의 테스트 목(mock)

// inv-store/src/location.rs — 함정6 실본(호출됨)
pub fn normalize_location(raw: &str) -> String
// inv-store/src/legacy_import.rs — 함정6 사본(거의 동일 본문, 외부 비호출)
pub fn normalize_location(raw: &str) -> String

// inv-report/src/totals.rs — 함정2 v1 + 함정9 거짓 FIXME(코드는 정상)
// FIXME: 반품 부호 처리가 의심스럽다 — 확인 필요 (2024-11)
pub fn calc_total(lines: &[LedgerLine]) -> i64 {
    let mut total = 0i64;
    for line in lines {
        match line.kind {
            LineKind::Sale => total += line.amount_krw,
            LineKind::Refund => total -= line.amount_krw,
            LineKind::Adjustment => total += line.adjustment_krw(),
        }
    }
    total
}

// inv-report/src/monthly.rs — 함정2 v2 + 함정8 거짓주석. 베이스는 아래 "정상" 본문.
// 월간 정산 보고서 합계. (함정8: 바로 위에 "반품은 여기 오기 전에 이미 차감되어
// 들어온다"는 거짓 주석 배치 — 실제로는 차감 안 된 원본이 들어옴)
pub fn calc_total_v2(lines: &[LedgerLine]) -> i64 {
    lines.iter().fold(0i64, |acc, line| match line.kind {
        LineKind::Sale => acc + line.amount_krw,      // 과제1은 여기를 `acc - `로 심는다
        LineKind::Refund => acc - line.amount_krw,
        LineKind::Adjustment => acc + line.adjustment_krw(),
    })
}
pub fn monthly_total(lines: &[LedgerLine]) -> i64 { calc_total_v2(lines) }

// inv-report/src/invoice.rs — 함정7 지점B
pub fn invoice_total(subtotal_krw: i64) -> i64 { subtotal_krw * 110 / 100 }

// inv-report/src/forecast.rs — 함정7 지점C (표기 형태 다양화: f64)
pub fn forecast_projection(net_krw: i64) -> i64 { (net_krw as f64 * 1.10) as i64 }
```

**과제↔함정 결선(함정 대장에 그대로 기록)**: 과제1 → #2·#8·#9(+주변 #1·#4) / 과제2 → #7(4지점)·#1(주석 옛코드에 `vat` 옛 계산 포함) / 과제3 → #11·#10·#3·#6. #5는 상주(ambient) — grep 오염 관찰용. **정답 파일 집합**: 과제1 `inv-report/src/monthly.rs` / 과제2 `inv-core/src/rules/pricing.rs`·`inv-report/src/invoice.rs`·`inv-report/src/forecast.rs`·`inv-parse/src/defaults.rs` / 과제3 `inv-core/src/rules/mod.rs`.

**공통 protected 목록(3과제 동일, task.toml에 그대로)**:

```toml
protected = [
  "Cargo.toml",
  "inv-core/Cargo.toml",   "inv-core/tests",
  "inv-parse/Cargo.toml",  "inv-parse/tests",
  "inv-store/Cargo.toml",  "inv-store/tests",
  "inv-report/Cargo.toml", "inv-report/tests",
  "inv-cli/Cargo.toml",    "inv-cli/tests",
]
```

---

### Task 1: 브랜치 + tasks-large 골격 + 워크스페이스 루트 + inv-core

**Files:**
- Create: 브랜치 `m8-large-repo-track` (main `23dc1fd`에서)
- Create: `tasks-large/README.md` (함정 대장 골격 + 드리프트 절차)
- Create: `tasks-large/find-definition-large/fixture/Cargo.toml`, `fixture/.gitignore`
- Create: `tasks-large/find-definition-large/fixture/inv-core/` 전체 (~25파일, ~4K LOC)

**Interfaces:**
- Produces: 핀 고정 인터페이스의 inv-core 항목 전부(`LineKind`/`LedgerLine`/`apply_tax`/`restock_threshold`/`WarehouseGrade`/`inventory` 재수출/`MAX_RETRY`) — Task 2~6이 사용

- [ ] **Step 1: 브랜치 생성**

```bash
git checkout -b m8-large-repo-track
```

- [ ] **Step 2: 워크스페이스 루트 작성**

`tasks-large/find-definition-large/fixture/Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["inv-core", "inv-parse", "inv-store", "inv-report", "inv-cli"]
```

`tasks-large/find-definition-large/fixture/.gitignore`:

```
/target
```

(멤버 크레이트가 아직 없으므로 이 시점 빌드는 실패해도 됨 — Step 4에서 core만 우선 등록해도 무방하나, 최종 상태 그대로 두고 Step 4에서 미완 멤버는 빈 골격 크레이트로 먼저 만들어 컴파일 가능 상태 유지를 권장)

- [ ] **Step 3: README 골격 작성**

`tasks-large/README.md` — 아래 구조로(내용은 태스크 진행하며 채움):

```markdown
# tasks-large — 대형 저장소 트랙 (M8)

이 파일은 fixture/ 밖에 있어 샌드박스로 복사되지 않는다(모델 비노출).

## 함정 대장
| # | 함정 | 위치 | 결선 과제 | 발동 판독 방법 |
(스펙 §3 카탈로그 11종 — 각 태스크에서 심을 때마다 행 추가)

## 과제별 정답 파일 집합
(§4 참조 — Task 4~6에서 확정)

## 판별력 수동 확인 기록
(과제 2의 지점별 부분 오버레이 확인 결과 — Task 6에서 기록)

## 드리프트 방지 절차 (베이스 수정 시)
베이스 = find-definition-large/fixture (판정 파일 tests/check_*.rs 제외).
1. 베이스에서 수정 후 `cargo test` 녹색 확인
2. rsync -a --delete --exclude 'tests/check_*.rs' --exclude 'target'
   find-definition-large/fixture/ fix-monthly-total/fixture/
   (update-vat-rate도 동일)
3. fix-monthly-total에 버그 패치 재적용 (아래 diff)
4. solution/ 3벌 재검토 → `cargo run -- eval tasks-large --verify`

### fix-monthly-total 버그 패치 (calc_total_v2 Sale arm)
- LineKind::Sale => acc + line.amount_krw,
+ LineKind::Sale => acc - line.amount_krw,
```

- [ ] **Step 4: inv-core 작성 (브리프)**

멤버 매니페스트 공통 형식(크레이트명·의존만 바꿔 전 크레이트 동일):

```toml
[package]
name = "inv-core"
version = "0.1.0"
edition = "2021"

[dependencies]
# 내부 path 의존만 허용. 예) inv-parse에서: inv-core = { path = "../inv-core" }
```

파일 구성(±20% 재량, 합계 ~4K LOC — 컴파일·자체 테스트 통과가 우선):

| 파일 | ~LOC | 책임 / 필수 요소 |
|---|---|---|
| src/lib.rs | 30 | `pub mod config; pub mod ledger; pub mod inventory; pub mod rules; pub mod sku; pub mod warehouse; pub mod util;` |
| src/ledger.rs | 150 | 핀 고정 타입 + 정렬/필터 헬퍼 |
| src/config.rs | 80 | **함정5 짝A** `MAX_RETRY` + 코어 설정 타입 |
| src/inventory.rs | 60 | **함정11**: `pub use crate::rules::{restock_threshold, WarehouseGrade};` + 재고 스냅샷 타입 |
| src/rules/mod.rs | **720+** | **함정10 갓파일**: 최상단 `pub mod pricing; pub mod allocation;` 선언 → 할당/이동/유효성 규칙 함수 다수(각각 한국어 doc 주석) → **최하단에 핀 고정 `restock_threshold`+`WarehouseGrade`** |
| src/rules/pricing.rs | 120 | **함정7 지점A** 핀 고정 `apply_tax` + 할인 규칙 2~3개 |
| src/rules/allocation.rs | 250 | 창고 배분 규칙(자유) |
| src/sku.rs, src/warehouse.rs | 각 200 | SKU 파싱/검증, 창고 모델(자유) |
| src/util.rs | 100 | **함정4**: 문자열/숫자 헬퍼(다른 크레이트에도 동명 util.rs를 둔다) |
| 나머지 src/*.rs 10~14개 | 각 100~250 | 도메인 잡동사니(이력, 감사 로그, 단위 변환 등 — 자유, 일부 한국어 주석) |
| tests/core_basic.rs | 100 | 베이스 테스트(금지 구역 준수: ledger 정렬, restock_threshold, sku 검증 등만) |

- [ ] **Step 5: 빌드·테스트 확인**

```bash
cd tasks-large/find-definition-large/fixture && cargo test
```
Expected: PASS (미완 멤버는 빈 골격이어도 각자 컴파일)

- [ ] **Step 6: 커밋**

```bash
git add tasks-large && git commit -m "feat(eval): tasks-large 골격 + inv-core 베이스 픽스처 (M8 Task1)"
```

### Task 2: inv-parse + inv-store

**Files:**
- Create: `.../fixture/inv-parse/` (~15파일, ~3K LOC), `.../fixture/inv-store/` (~15파일, ~3K LOC)

**Interfaces:**
- Consumes: inv-core 핀 타입
- Produces: `parse_config`/`Config`/`DEFAULT_VAT_PERCENT`, `apply_movement`/`normalize_location`/`RETRY_LIMIT` (핀 고정)

- [ ] **Step 1: inv-parse 작성 (브리프)**

| 파일 | ~LOC | 책임 / 필수 요소 |
|---|---|---|
| src/lib.rs | 20 | 모듈 선언 |
| src/defaults.rs | 60 | **함정7 지점D** 핀 고정 `DEFAULT_VAT_PERCENT` + 기타 기본값 |
| src/config.rs | 180 | 핀 고정 `Config`/`parse_config` — `key=value` 줄 단위 손파싱, 누락 키는 defaults |
| src/csv.rs | 350 | 재고 CSV 손파싱. **함정1**: 파일 중간에 80~120줄 주석화된 `parse_row_v0` 옛 구현(주석 안에 `let vat = subtotal * 10 / 100; // 구버전 세율` 한 줄 포함 — 과제2 grep 오염) |
| src/util.rs | 80 | **함정4** 동명 헬퍼 |
| src/reader.rs / src/readers.rs | 각 120 | **함정4**: 유사 파일명 짝(단수=행 단위, 복수=일괄) |
| 나머지 src/*.rs 6~8개 | 각 100~250 | 검증/이스케이프/날짜 파싱 등(자유) |
| tests/parse_basic.rs | 80 | 베이스 테스트(금지 구역 준수 — vat 값 단정 금지, 구조 파싱만) |

- [ ] **Step 2: inv-store 작성 (브리프)**

| 파일 | ~LOC | 책임 / 필수 요소 |
|---|---|---|
| src/lib.rs | 20 | 모듈 선언 |
| src/retry.rs | 70 | **함정5 짝B** 핀 고정 `RETRY_LIMIT` + 재시도 로직 |
| src/movement.rs | 150 | 핀 고정 `apply_movement` 실구현 |
| src/location.rs | 130 | **함정6 실본** `normalize_location`(trim→대문자→구분자 통일), store 내부에서 호출 |
| src/legacy_import.rs | 160 | **함정6 사본**: 거의 동일한 `normalize_location`(공백 처리 한 줄만 다름) + 옛 임포트 루틴, 외부 비호출 |
| src/memory.rs, src/file.rs | 각 250 | 인메모리/파일 저장 구현(자유) |
| src/util.rs | 80 | **함정4** |
| 나머지 6~8개 | 각 100~200 | 인덱스/스냅샷/락 등(자유) |
| tests/store_basic.rs | 100 | 베이스 테스트 |
| tests/support/mod.rs | 60 | **함정3**: 실구현과 같은 시그니처의 `pub fn apply_movement(qty: i64, delta: i64) -> i64` 목(고정값 반환 + `// 테스트 전용 목` 한국어 주석) |

- [ ] **Step 3: 테스트·커밋**

```bash
cd tasks-large/find-definition-large/fixture && cargo test
git add tasks-large && git commit -m "feat(eval): inv-parse·inv-store — 함정 1·3·4·5·6·7D 배치 (M8 Task2)"
```

README 함정 대장에 이번 태스크에서 심은 행 추가(위치·결선 포함).

### Task 3: inv-report + inv-cli — 베이스 완성

**Files:**
- Create: `.../fixture/inv-report/` (~15파일, ~3K LOC), `.../fixture/inv-cli/` (~8파일, ~1.5K LOC)

**Interfaces:**
- Consumes: core/parse/store 핀 인터페이스
- Produces: `calc_total`/`calc_total_v2`/`monthly_total`/`invoice_total`/`forecast_projection` (핀 고정 — Task 5·6 판정이 여기 걸림)

- [ ] **Step 1: inv-report 작성 (브리프)**

| 파일 | ~LOC | 책임 / 필수 요소 |
|---|---|---|
| src/lib.rs | 20 | 모듈 선언 |
| src/totals.rs | 140 | **함정2 v1 + 함정9**: 핀 고정 `calc_total` 전문 그대로(FIXME 주석 포함, 코드는 정상) |
| src/monthly.rs | 180 | **함정2 v2 + 함정8**: 핀 고정 `calc_total_v2`(베이스는 정상 본문)·`monthly_total` + 거짓 주석 + `// 월간 정산 보고서` 주석(과제1 1-hop grep 앵커) |
| src/invoice.rs | 150 | **함정7 지점B** 핀 고정 `invoice_total` + 인보이스 조립 |
| src/forecast.rs | 130 | **함정7 지점C** 핀 고정 `forecast_projection` + 전망 로직 |
| src/report.rs / src/reporting.rs / src/report_v2.rs | 각 100~150 | **함정4**: 3연속 유사 파일명(얇은 조립 계층·옛 계층·신 계층). report.rs와 cli의 레거시 경로만 v1 `calc_total`을 호출(함정2의 "호출부 혼재") |
| src/util.rs | 80 | **함정4** |
| 나머지 4~6개 | 각 100~200 | 집계 보조(자유) |
| tests/report_basic.rs | 90 | 베이스 테스트(금지 구역 엄수 — v1 `calc_total`은 단정 가능, v2/vat 계열 금지) |

- [ ] **Step 2: inv-cli 작성 (브리프)**

bin+lib 구조(테스트 가능성): `src/main.rs`(얇은 진입, ~80), `src/lib.rs`(서브커맨드 라우팅 ~200), `src/commands/*.rs` 3~4개(각 150 — report 커맨드가 `inv_core::inventory::restock_threshold`를 **재수출 경로로** 사용: 함정11 발동면), `src/util.rs`(**함정4**), `tests/cli_basic.rs`(라우팅 단위 테스트).

- [ ] **Step 3: 베이스 전체 검증**

```bash
cd tasks-large/find-definition-large/fixture && cargo test
find . -name '*.rs' | wc -l && wc -l $(find . -name '*.rs') | tail -1
```
Expected: 전부 PASS, 파일 50개+·합계 10~20K LOC 범위 확인(범위 밖이면 잡동사니 모듈로 증감)

- [ ] **Step 4: 커밋**

```bash
git add tasks-large && git commit -m "feat(eval): inv-report·inv-cli — 베이스 완성, 함정 2·4·7~9·11 배치 (M8 Task3)"
```

### Task 4: 과제 3 — find-definition-large 완성

**Files:**
- Create: `tasks-large/find-definition-large/task.toml`
- Create: `.../fixture/inv-core/tests/check_answer.rs`
- Create: `tasks-large/find-definition-large/solution/answer.txt`

**Interfaces:**
- Consumes: 베이스 픽스처(Task 1~3)
- Produces: 동작하는 과제 1개 — `--verify` 1/1

- [ ] **Step 1: 판정 테스트 작성** — `fixture/inv-core/tests/check_answer.rs` 전문:

```rust
//! 과제 판정: answer.txt가 restock_threshold의 실제 정의 파일을 가리키는가.
//! 정규화 사다리는 M6 §3 관례(트림 → 짝 따옴표 제거 → 경로 정규화)의 이식이며
//! 같은 파일 안에서 자기시험한다.
use std::fs;

const TARGET: &str = "inv-core/src/rules/mod.rs";

fn strip_quotes(s: &str) -> &str {
    let b = s.as_bytes();
    if b.len() >= 2
        && ((b[0] == b'"' && b[b.len() - 1] == b'"')
            || (b[0] == b'\'' && b[b.len() - 1] == b'\''))
    {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

fn normalize(raw: &str) -> String {
    let t = strip_quotes(raw.trim());
    let t = t.replace('\\', "/");
    let t = t.strip_prefix("./").unwrap_or(&t);
    t.trim_end_matches('/').to_string()
}

fn matches_target(raw: &str) -> bool {
    let n = normalize(raw);
    n == TARGET || n.ends_with(&format!("/{TARGET}"))
}

#[test]
fn answer_names_definition_file() {
    let raw = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../answer.txt"))
        .expect("answer.txt가 프로젝트 루트에 없다");
    assert!(matches_target(&raw), "정의 파일 경로가 아니다: {raw:?}");
}

#[test]
fn ladder_accepts_variants() {
    for ok in [
        "inv-core/src/rules/mod.rs",
        "./inv-core/src/rules/mod.rs",
        "\"inv-core/src/rules/mod.rs\"",
        "inv-core\\src\\rules\\mod.rs",
        "  inv-core/src/rules/mod.rs\n",
        "/sandbox/proj/inv-core/src/rules/mod.rs",
    ] {
        assert!(matches_target(ok), "수용해야 함: {ok:?}");
    }
}

#[test]
fn ladder_rejects_wrong_paths() {
    for bad in [
        "inv-core/src/inventory.rs",        // 함정11: 재수출 지점
        "inv-store/tests/support/mod.rs",   // 함정3: 목
        "inv-store/src/legacy_import.rs",   // 함정6: 사본
        "inv-core/src/rules",
    ] {
        assert!(!matches_target(bad), "기각해야 함: {bad:?}");
    }
}
```

- [ ] **Step 2: task.toml 작성** — 전문:

```toml
prompt = "함수 restock_threshold의 정의(구현 본문)가 있는 파일의 상대 경로를 프로젝트 루트의 answer.txt에 한 줄로 저장하라. 재수출(re-export) 지점이나 테스트 헬퍼가 아닌 실제 정의 위치여야 한다. 저장 후 cargo test로 확인하라."
check = "cargo test"
timeout_secs = 600
check_timeout_secs = 300
# 공통 protected 목록(플랜 서두) 그대로
```

- [ ] **Step 3: solution 작성** — `solution/answer.txt` 전문(개행 포함 1줄):

```
inv-core/src/rules/mod.rs
```

- [ ] **Step 4: 판별력·해결가능성 확인 + 커밋**

```bash
cargo run -- eval tasks-large --verify
```
Expected: 1/1 (원본 FAIL: answer.txt 부재 / solution PASS). README에 함정 대장 #10·#11 행과 과제3 정답 파일 집합 기록.

```bash
git add tasks-large && git commit -m "feat(eval): 과제3 find-definition-large — 판정 사다리 이식·verify 1/1 (M8 Task4)"
```

### Task 5: 과제 1 — fix-monthly-total (증상 버그 수정)

**Files:**
- Create: `tasks-large/fix-monthly-total/` = fixture 사본 + 버그 + 판정 + task.toml + solution

**Interfaces:**
- Consumes: 베이스 + 핀 고정 `calc_total_v2`/`monthly_total`
- Produces: `--verify` 2/2

- [ ] **Step 1: 사본 생성 + 버그 심기**

```bash
rsync -a --exclude 'tests/check_*.rs' --exclude 'target' \
  tasks-large/find-definition-large/fixture/ tasks-large/fix-monthly-total/fixture/
```
`fixture/inv-report/src/monthly.rs`의 `calc_total_v2`에서 Sale arm을 `acc - line.amount_krw`로 변경(README의 버그 패치 diff 그대로).

- [ ] **Step 2: 판정 테스트** — `fixture/inv-report/tests/check_monthly.rs` 전문:

```rust
//! 과제 판정: 월간 정산 합계의 부호 정상성.
use inv_core::ledger::{LedgerLine, LineKind};
use inv_report::monthly::monthly_total;

fn line(kind: LineKind, amount: i64) -> LedgerLine {
    LedgerLine { sku: "SKU-1".into(), kind, amount_krw: amount, adj_krw: 0 }
}

#[test]
fn monthly_total_with_refund_is_positive_net() {
    let lines = vec![
        line(LineKind::Sale, 120_000),
        line(LineKind::Sale, 80_000),
        line(LineKind::Refund, 30_000),
    ];
    assert_eq!(monthly_total(&lines), 170_000);
}

#[test]
fn monthly_total_sales_only_equals_sum() {
    let lines = vec![line(LineKind::Sale, 50_000), line(LineKind::Sale, 70_000)];
    assert_eq!(monthly_total(&lines), 120_000);
}
```

- [ ] **Step 3: task.toml** — 전문(protected는 공통 목록):

```toml
prompt = "월간 정산 보고서에서 반품이 포함된 달의 합계가 음수로 나온다는 현장 보고가 있다. 판매는 더해지고 반품만 차감되는 것이 정상이다. 원인을 찾아 고치고 cargo test로 확인하라."
check = "cargo test"
timeout_secs = 600
check_timeout_secs = 300
```

- [ ] **Step 4: solution** — `solution/inv-report/src/monthly.rs` = fixture 사본에서 Sale arm만 `acc + line.amount_krw`로 되돌린 파일 전문(그 외 바이트 동일).

- [ ] **Step 5: 검증 + 커밋**

```bash
cd tasks-large/fix-monthly-total/fixture && cargo test 2>&1 | tail -5   # 판정 2건만 FAIL 확인
cd - && cargo run -- eval tasks-large --verify                          # 2/2
git add tasks-large && git commit -m "feat(eval): 과제1 fix-monthly-total — v2 부호 버그·verify 2/2 (M8 Task5)"
```
README에 함정 #2·#8·#9 행 + 과제1 정답 파일 집합 + 버그 패치 diff 기록.

### Task 6: 과제 2 — update-vat-rate (크레이트 경계 산포 변경)

**Files:**
- Create: `tasks-large/update-vat-rate/` = fixture 사본 + 판정 3파일 + task.toml + solution 4파일

**Interfaces:**
- Consumes: 함정7 4지점(핀 고정)
- Produces: `--verify` 3/3 + 지점별 판별력 확인 기록

- [ ] **Step 1: 사본 생성** (버그 없음 — "세상이 바뀐" 과제)

```bash
rsync -a --exclude 'tests/check_*.rs' --exclude 'target' \
  tasks-large/find-definition-large/fixture/ tasks-large/update-vat-rate/fixture/
```

- [ ] **Step 2: 판정 테스트 3파일** — 전문:

`fixture/inv-core/tests/check_vat_core.rs`:
```rust
#[test]
fn apply_tax_uses_12_percent() {
    assert_eq!(inv_core::rules::pricing::apply_tax(10_000), 11_200);
}
```
`fixture/inv-parse/tests/check_vat_default.rs`:
```rust
#[test]
fn default_config_vat_is_12() {
    assert_eq!(inv_parse::config::parse_config("").vat_percent, 12);
}
```
`fixture/inv-report/tests/check_vat_report.rs`:
```rust
#[test]
fn invoice_total_uses_12_percent() {
    assert_eq!(inv_report::invoice::invoice_total(100_000), 112_000);
}
#[test]
fn forecast_projection_uses_12_percent() {
    assert_eq!(inv_report::forecast::forecast_projection(200_000), 224_000);
}
```

- [ ] **Step 3: task.toml** — 전문(protected 공통):

```toml
prompt = "부가세율이 10%에서 12%로 변경되었다. 시스템 전체의 세율 적용을 빠짐없이 반영하라. 설정 기본값도 포함된다. 반영 후 cargo test로 확인하라."
check = "cargo test"
timeout_secs = 600
check_timeout_secs = 300
```

- [ ] **Step 4: solution 4파일** — 각각 fixture 사본에서 해당 지점만 수정한 전문: `pricing.rs`(`* 12 / 100`), `invoice.rs`(`* 112 / 100`), `forecast.rs`(`* 1.12`), `defaults.rs`(`= 12`).

- [ ] **Step 5: 지점별 판별력 수동 확인 (스펙 §4 요구)** — 4회 반복: solution에서 파일 1개만 임시 제외한 부분 오버레이를 fixture 사본(scratch)에 적용 → `cargo test`가 **여전히 FAIL**(남은 지점의 판정 테스트가 잡음)임을 확인. 4회 전부의 결과(제외 파일→실패한 테스트명)를 README "판별력 수동 확인 기록"에 기록.

- [ ] **Step 6: 검증 + 커밋**

```bash
cargo run -- eval tasks-large --verify   # 3/3
git add tasks-large && git commit -m "feat(eval): 과제2 update-vat-rate — 산포 4지점 판별력 확인·verify 3/3 (M8 Task6)"
```

### Task 7: 제작 게이트 + 문서

**Files:**
- Modify: `tasks-large/README.md` (함정 대장 11종 완성도 점검 — 미기록 행 보완)
- Modify: `CLAUDE.md` (tasks-large 절 추가: 3과제·verify 규칙·README=함정 대장·드리프트 절차 참조, 4~6줄)

- [ ] **Step 1: 전체 게이트 실행**

```bash
cargo test                                     # 기존 261 tests 그대로
cargo clippy --all-targets -- -D warnings      # 클린
cargo run -- eval tasks-large --verify         # 3/3
cargo run -- eval tasks --verify               # 12/12 (기존 불변)
git diff 23dc1fd -- src tasks                  # 빈 값 (성공 기준 2·6)
```

- [ ] **Step 2: 알파벳 편향 기록** — 각 과제 fixture에서 시스템 프롬프트 트리(100항목 상한) 노출 여부를 정답 파일 기준으로 판독해 README 함정 대장에 "트리 노출: O/X" 열로 기록(§3 의도 변수).

- [ ] **Step 3: CLAUDE.md 갱신 + 커밋**

```bash
git add tasks-large CLAUDE.md && git commit -m "docs: tasks-large 함정 대장 완성·CLAUDE.md M8 트랙 반영 (M8 Task7)"
```

### Task 8: 레퍼런스 노트 — aider repo-map

**Files:**
- Create: `docs/research/2026-07-16-aider-repo-map.md`

- [ ] **Step 1**: aider 공식 문서(repomap 페이지)와 소스(`aider/repomap.py`)를 조사해 노트 작성 — 필수 절: ① tree-sitter 태그 추출 방식 ② 심볼 랭킹(그래프 기반) 알고리즘 ③ 토큰 예산 맞춤 로직 ④ **loco에의 시사점**(Rust·8K·의존성 제약(스펙 고정 목록: tree-sitter 크레이트 추가는 사용자 승인 필요) 하의 이식 형태 스케치)
- [ ] **Step 2**: 커밋 `docs: aider repo-map 레퍼런스 노트 (M8 Task8)`

### Task 9: 레퍼런스 노트 — codex-rs

**Files:**
- Create: `docs/research/2026-07-16-codex-rs.md`

- [ ] **Step 1**: openai/codex의 codex-rs를 조사해 노트 작성 — 필수 절: ① 컨텍스트 압축/이력 관리 ② 도구 표면(apply_patch·검색 설계) ③ 소형/로컬 모델(`--oss`) 대응 흔적 ④ **loco에의 시사점**
- [ ] **Step 2**: 커밋 `docs: codex-rs 레퍼런스 노트 (M8 Task9)`

(Task 8·9는 픽스처 태스크와 독립 — 병행 가능. 실패 데이터가 특정 주제를 가리키면 깊이 제한 없이 후속 노트 확장 — 스펙 §7)

### Task 10: 측정 준비 — ornith 실측 사양표 + 32K 로드값 확정 (사용자 협조)

**Files:**
- Create: `./.loco/config.toml` (git-ignored 로컬)
- Modify: `docs/baselines.md` (M8 측정 조건·사양표 절 신설)

- [ ] **Step 1: 로컬 config 작성**

```toml
context_tokens = 8192
max_output_tokens = 4096
command_timeout_secs = 240
```

- [ ] **Step 2: 사양표 측정 (사용자 협조 체크포인트)** — 사용자에게 ornith를 8192→16384→32768 로드 컨텍스트로 차례로 로드 요청. 각 설정에서: `curl -s localhost:1234/api/v0/models`로 로드 상태·ctx 확인, 메모리 점유는 시스템 모니터로 사용자 판독, 프리필은 6K 토큰급 고정 프롬프트(베이스 픽스처의 rules/mod.rs 앞 600줄 붙여넣기)를 스트리밍 호출해 첫 토큰 지연 측정 → tok/s 환산(스펙 §5 방법). 결과를 baselines.md 사양표에 기록 — "실운용 로드는 여유분 포함(32K 운용 = 로드 40960~49152)" 각주 필수.

- [ ] **Step 3: 32K 로드값 확정** — 사양표의 토큰당 KV 비용으로 40960 vs 49152의 메모리 여유를 판정해 로드값을 확정하고 baselines.md에 근거와 함께 기록(스펙 §5 순서). 커밋 `docs: ornith 실측 사양표·32K 로드값 확정 (M8 Task10)`

### Task 11: 8K 베이스라인 측정 (사용자 협조·위임 금지)

- [ ] **Step 1**: `cargo build` 선완료(측정 중 빌드 금지). 트립와이어 사전 점검 `ls ${TMPDIR}/.cargo`(존재 시 수동 제거).
- [ ] **Step 2**: 사용자에게 gemma-4-e4b 단독 로드(ctx 12288) 요청 → `cargo run -- eval tasks-large --repeats 3 --seed 0` → report.json 보존(`.loco/eval/<stamp>` 경로 기록)
- [ ] **Step 3**: 사용자에게 ornith 단독 로드(ctx 12288) 요청 → 동일 실행
- [ ] **Step 4**: 오버플로 하네스 중단 발생 시 스펙 §5 재시작 프로토콜(해당 배치 전체 재실행, 발생 사실은 분석에 별도 행). 두 배치 수치(관대/엄격/거짓finish/avg s/런)를 baselines.md M8 절에 기록·커밋

### Task 12: 32K 민감도 (사용자 협조·위임 금지)

- [ ] **Step 1**: `./.loco/config.toml`의 `context_tokens = 32768`로 변경, 사용자에게 ornith를 Task 10 확정 로드값으로 재로드 요청
- [ ] **Step 2**: `cargo run -- eval tasks-large --repeats 3 --seed 0` → 수치 기록. 완료 후 config를 8192로 원복. effective_config 대조(context_tokens 차이 자증) 확인·커밋

### Task 13: 실패 분류·분석 노트·M9 요구사항

**Files:**
- Create: `docs/research/<작성일 YYYY-MM-DD>-m8-failure-analysis.md` (실행 당일 날짜로 명명)
- Modify: `docs/baselines.md` (M8 최종 절), `CLAUDE.md` (M8 결과 요약 갱신)

- [ ] **Step 1: 런별 분류표 작성** — 3배치 × 3과제 × 3런: outcome(직렬화 실명 `finished`/`max_turns`/`repetition_stop`/`parse_failed`/`timeout` grep), 검증 타임아웃 계수(`grep -c "command timed out" run-*.jsonl`), 발동 함정 번호(트랜스크립트 판독 — README 정답 파일 집합·함정 위치 대조), 도구 사용 패턴(첫 정답 파일 도달 턴 수, grep/list_files/read_file 비율)
- [ ] **Step 2: M9 요구사항 후보 도출** — 실패 유형별 빈도 → 스펙 §8 백로그(repo-map·검색 강화·오버플로 내성 4건)와 대조해 우선순위 제안
- [ ] **Step 3: 문서 3종 갱신·커밋** — 분석 노트 + baselines.md + CLAUDE.md. 커밋 `docs: M8 실패 분류·M9 요구사항 후보 (M8 Task13)`. 이후 최종 브랜치 리뷰 → main 머지(사용자 확인)는 finishing-a-development-branch 절차.
