# M15 실레포 트랙과 토큰 회계 구현 플랜

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 판별력이 남은 계측기를 짓는다 — 실제 OSS 레포의 닫힌 이슈로 만든 `tasks-real/` 트랙, 토큰 회계 계측, 그리고 32K 운용점의 베이스라인 배치.

**Architecture:** 조달(`git archive`)을 하네스 밖 명시 단계로 분리해 `<task_dir>/fixture`를 실체화하고, 기존 `eval`/`--verify` 경로는 그대로 재사용한다. 축 C는 서버 실측 `prompt_tokens`와 `estimate_tokens` 추정치를 턴마다 나란히 기록하는 **순수 기록**이며 개입이 아니다. 32K 운용점은 과제별 `TaskSpec.context_tokens`로만 먹이고 `RunRecord`가 실효값을 자증한다 — 코드 기본값과 기존 두 트리의 8K 앵커는 불변.

**Tech Stack:** Rust edition 2024, `serde`/`thiserror`/`anyhow`/`similar`/`tempfile`. 스크립트는 POSIX `sh`와 stdlib-only Python 3.

**기준 커밋:** `e74fa21` (스펙 **개정 10** — 플랜 1R이 스펙에서 찾은 사실 오류 2건 정정 반영)
**스펙:** `docs/superpowers/specs/2026-07-20-m15-real-repo-track-design.md` — **유일한 진실. 이 플랜과 충돌하면 스펙이 이긴다. 충돌을 발견하면 스스로 결정하지 말고 에스컬레이션할 것.**

**개정 이력:**
- 초판 `86ff07b` — 3축 병렬 리뷰(사실정합성 / 실현가능성 / 측정설계). **Critical 6 · Important 10 · Minor 9, 3축 중 2축 `Ready: No`**
- **개정 2** `0f914ff` — 1R 전건 반영. 2R에서 **Critical 3 · Important 8 · Minor 13**, 2축 모두 `Ready: No`.
  ⚠ **2R의 축 A(1R 수정 검증)는 0건이었다** — 개정 2의 수정 6종이 2×2까지 채워 전건 확인됐다.
  **지적은 전부 축 B, 즉 "개정 2가 새로 만든 것"이다**
- **개정 3** — 2R 전건 반영. 아래 "라운드가 남긴 것"이 그 요약이다

## 라운드가 남긴 것 — 이 플랜이 실패한 두 형태

**형태 2 (2R): 고치는 자리에서 같은 결함을 다시 만든다.**

2R의 **축 A(1R 수정이 작동하는가)는 0건**이었다 — 개정 2의 수정 6종이 정상=통과 / 변조=실패의 2×2까지 채워 전건 확인됐다. **Critical 3은 전부 개정 2가 새로 만든 것이다.**

그중 하나가 특히 말해 준다: 1R Critical 5는 *"`run_metrics` 반환 튜플을 바꾸고 그것에 의존하던 곳을 전수로 안 갱신했다"*였는데, **개정 2는 그 Critical을 고치는 바로 그 자리에서 `tok`에 새 이름 넷을 참조로 넣고 선언을 다른 태스크에 두어 T14를 단독 실행 불가로 만들었다.** 스펙 §11-2가 일곱 번 추적한 형태이고, 이제 플랜에서 두 번째다.

나머지 둘도 같은 계열이다 — 캐시 목록을 `.extracted`/`.files`로 **나누면서** 필터에 새 이름을 안 넣었고(매니페스트가 자기를 셌다), `overlay_tree`에 mode 복원을 **더하면서** 세 번째 read+write 사이트를 안 셌다(그리고 그것이 `tasks-real`이 실제로 타는 경로였다).

> **개정 3이 추가하는 규율: 무언가를 "둘로 나누거나" "하나 더 추가"할 때는, 그 이름·목록·분기의 전수를 `grep`으로 다시 세고 개수를 커밋 메시지에 적는다.**

---

## 형태 1 (1R): 검증 장치가 실패할 수 없다

**Critical 6건 중 4건이 같은 형태였다: 검증 장치를 만들고 그 장치가 실제로 실패할 수 있는지 확인하지 않았다.**

- T4의 스테일 테스트는 macOS `/bin/sh`(bash 3.2)에서 `-nt`가 **초 단위로 절삭**돼 **변조 없이도 실패**했다 — 반증 확인이 공허했고, Step 2의 "FAIL이면 T2·T3이 회귀시킨 것"은 구현자를 없는 회귀로 몰았다
- T13의 `failed_dispatch_is_not_recorded_as_a_touch`는 **본문이 주석 한 줄뿐인 빈 함수**로 출하됐다. 빈 테스트는 항상 통과하므로 그 반증 단계는 **절대 실패할 수 없다**
- T6의 H9 자증은 **자기가 겨눈 배선 단절을 못 잡았다** — `Agent::new`가 `config.context_tokens`를 스냅샷하는데 `EffectiveRun`이 같은 `cfg`에서 다시 읽어, 오버라이드를 `Agent::new` 뒤로 옮겨도 **73개 테스트 전건 초록불에 report.json만 거짓말**을 했다
- `leak_audit.py`의 `SKIP_RE`가 `failures:` 구간 **안쪽**에도 적용돼 테스트 자신의 `note:` 단언 메시지를 컴파일러 진단으로 오인, **진짜 누설을 삼켰다**

**읽기 리뷰 두 축은 이것을 하나도 못 잡았다** — 사실정합성 축은 `Ready: Yes`를 냈다. 잡은 것은 **코드를 실제로 적용하고 돌린 축 하나**다. 프로젝트 메모리 *"플랜 리뷰는 예시 코드를 검증하지 않는다 — 뮤테이션 검사를 의무화할 것"*이 정확히 이 사건을 예측했다.

**그래서 개정 2가 추가하는 규율**(모든 태스크에 적용):

> **반증 단계는 "변조 시 실패"만이 아니라 "정상 시 통과"도 함께 확인한다.**
> 2×2를 채우지 못한 반증은 반증이 아니다 — T4가 정확히 그 형태로 실패했다
> (정상·변조 **양쪽에서 FAIL**이라 아무것도 구별하지 못했다).

## Global Constraints

이 절의 요구사항은 **모든 태스크에 암묵적으로 포함된다.**

- **Edition 2024. 의존성 추가 금지** — 스펙이 목록을 고정한다. 새 크레이트가 필요하면 착수 전 사용자에게 물을 것
- **모델 대면 텍스트(SYSTEM_PROMPT·교정 문구·상태선·노트)는 영문. 사용자 대면 CLI 메시지는 한국어.** 식별자는 영문
- **에러**: `llm` 모듈은 `thiserror`, 앱 레벨은 `anyhow`
- **커밋**: Conventional Commits (subject는 한국어 가능)
- **브랜치**: Task 1에서 `main`(`0c96a72`)에 `m15/real-repo-track` 생성. **main 병합은 Task 25 판정 후에만**
- **매 태스크 종료 게이트**: `cargo test` 전건 통과 + `cargo clippy --all-targets -- -D warnings` 무경고. `--all-targets`가 중요하다(테스트 코드도 린트)
- **`tasks/`·`tasks-large/` 픽스처를 건드리지 않는다.** 변경이 생기면 그 자체가 이상 신호다. `src/eval/` 변경 태스크는 종료 전 `cargo run -- eval tasks --verify`(12/12)와 `cargo run -- eval tasks-large --verify`(3/3)를 돌릴 것
- **`exp_metrics.py`는 Rust 상수·술어를 손으로 복사한다**(`MAX_SR_CORRECTIONS`·`BADARGS_KEY_PREFIX`·`normalize`·상태선 매처·마커 문자열). 자동 검출이 없으므로 **Rust 쪽을 고치면 수동 미러가 필수**이고, 새 컬럼도 `--selftest`에 넣을 것
- **축 C는 개입이 아니다** — 상태선·UI·모델 대면 텍스트에 토큰 수를 노출하지 않는다(§5-6). 노출하는 순간 M14 상태선 문자열과의 비교가능성이 깨진다
- **측정 중 병행 빌드 금지**(PROTOCOL 2) — Task 22·24에서 CPU 경합이 타이밍 판정을 왜곡한다
- **이 머신에 `timeout`/`gtimeout`이 없다** — 스크립트가 쓰면 rc=127로 조용히 무동작한다(§10-7). 셸 스크립트에서 사용 금지
- **`docs/` 문서는 한국어, `CLAUDE.md`는 영문**

---

## 소비자 전수 추적 규율 (스펙 8라운드 리뷰가 남긴 것)

스펙 리뷰 Critical 추이는 13 → 9 → 6 → 1 → 1 → 1 → 1 → 0이었고, **4R 이후 매 라운드의 Critical은 전부 "직전 개정이 새로 만든 것"이며 일곱 번 다 같은 형태**였다(스펙 §11-2):

> **무언가를 바꾸고 그것에 의존하던 소비자를 전수로 갱신하지 않았다.**

따라서 **모든 태스크는 다음 두 항목을 강제로 포함한다:**

1. **"이 변경의 소비자" 목록** — 각 태스크의 `**Consumers:**` 블록. 소비자 0건이면 **"0건"이라고 명시**한다(빈칸 금지). 리뷰 체크리스트에서 전수 확인한다
2. **기호·상수·임계값을 도입·치환할 때는 그 기호의 모든 등장 지점을 `grep`으로 세고 결과를 태스크 커밋 메시지에 첨부한다.** 개정 7의 `마진`/`여유` 기호 붕괴(정의문 삭제로 산식이 항등식 순환)는 grep 한 번이면 잡혔다

---

## 파일 구조

| 파일 | 책임 | 태스크 |
|---|---|---|
| `docs/experiments/2026-07-20-m15-real-repo-baseline/thresholds.md` | **데이터 이전 동결값**(마진·최소 표본) | T1 |
| `src/eval/sandbox.rs` | `copy_tree` read+write+mode, 심링크 스킵 | T2, T3, T9 |
| `src/eval/verify.rs` | 스테일 뮤테이션 테스트 | T4 |
| `src/eval/task.rs` | `TaskSpec`에 `context_tokens`·`command_timeout_secs` | T5 |
| `src/eval/mod.rs` | `run_once` 오버라이드 배선, `judge` 실효값·protected 카운터 | T5, T6, T7, T8 |
| `src/eval/report.rs` | `RunRecord` 실효값·`protected_edits`, `TaskReport.procure` | T6, T7, T8 |
| `src/eval/procure.rs` | **신설** — `procure.toml` 로더 | T8 |
| `src/llm/types.rs` | `Usage.prompt_tokens` | T10 |
| `src/session.rs` | `total_tokens` 공개, `pack()` 축약 기록 | T11 |
| `src/agent/mod.rs` | `context_tokens()` 게터(T6) · 턴별 `usage` 기록 · 오버플로 최종 경로 · 접촉 파일 기록 | **T6**, T11, T12, T13 |
| `scripts/exp_metrics.py` | 토큰 회계 컬럼·항해/수선 지표·풀링·세션 모드 | T14, T15, T16 |
| `scripts/procure_real.sh` | **신설** — `git archive` 조달 | T17 |
| `scripts/leak_audit.py` | **신설** — §3-4-3 지목 판정 추출기 | T18 |
| `tasks-real/` | **신설** — 실레포 과제 트리 | T21 |
| `CLAUDE.md`·`docs/experiments/PROTOCOL.md` | `--verify` 성질 변화, 4①·4③·항목 5 | T19 |

**`src/agent/mod.rs`에 T11·T12·T13이 순차로 들어간다** — 같은 `run()` 루프를 건드리므로 병렬 위임 금지.

---

## 스펙이 못박은 수행 순서

스펙 §4-1-1의 순서 제약이다. **T20 이후는 이 순서를 벗어날 수 없다:**

> **`마진` 확정(커밋 해시)** → **rope 캡처** → **H4·H8·H11 구현**(+H5·H6) → **H12·H13·H19 구현** → **조달(스모크 대상 1과제)** → 스모크 → 분기 확정 → 사전등록 → 배치

플랜의 태스크 순서가 이것을 이미 만족한다: T1(동결) → T20(rope+실사) → T2·T3·T8·T17(조달 선행 요건) → T10·T11·T16(축 C·세션 모드) → T21(조달) → T22(스모크·분기) → T23(사전등록) → T24(배치).

⚠ **T1의 동결은 어떤 데이터도 보기 전에 커밋되어야 한다.** 실사 결과(T20)를 보고 최소 표본을 쓰면 결코 미달할 수 없다(§6-4-2).

---

### Task 1: 데이터 이전 동결 — `마진`과 최소 표본

**Files:**
- Create: `docs/experiments/2026-07-20-m15-real-repo-baseline/thresholds.md`

**Interfaces:**
- Consumes: 없음 (첫 태스크)
- Produces: 동결 커밋 해시 — T22의 분기 판정과 T23 사전등록 §6-4-8이 인용한다

**Consumers:** 이 문서를 읽는 곳 셋 — T22(분기 판정의 `마진` 입력항), T23(사전등록 항목 2·8), T20(실사가 최소 표본을 넘는지 판단). **코드 소비자 0건.**

- [ ] **Step 1: 브랜치 생성**

```bash
cd /Users/sgj/develop/loco
git checkout -b m15/real-repo-track
git rev-parse --short HEAD   # 0c96a72 여야 한다
```

- [ ] **Step 2: 동결 문서 작성**

`docs/experiments/2026-07-20-m15-real-repo-baseline/thresholds.md`:

```markdown
# M15 베이스라인 배치 — 데이터 이전 동결값

이 문서는 **어떤 측정·실사 결과도 보기 전에** 커밋된다. 스펙 §4-1-1과 §6-4-2가
같은 규율을 요구한다: 임계값을 데이터 뒤에 쓰면 결코 미달할 수 없다.

## 1. `마진` (스펙 §4-1-1)

> **`마진` = 1024 토큰** (절대 토큰 수. 비율이 아니다)

필요 로드 산식:

> **`L_req` = ⌈(ctx − mo) · 0.9 · r_obs + mo + 마진⌉**
> 이 배치에서 `ctx` = 32768(§2-2 결정 2), `mo` = 4096(§4-5).

근거: `∂L_req/∂mo = 1 − 0.9r ≈ −0.125`이므로 `mo`가 2048↔4096으로 바뀌어도
`L_req`는 ~30토큰만 움직여 분기 판정을 뒤집지 못한다(스펙 8R m3). 1024는
`serve.sh` 핀 헤더가 경고하는 "값을 바꾸면 비교 불가능"의 대상이므로
**이 배치 이후 변경 시 비교가능성 각주가 필요하다**.

## 2. 최소 표본과 미달 처분 (스펙 §6-4-2)

> **하한 = 16과제.**

유도: 정규근사 반폭 `1.96·√(0.25/N)`이 ±0.25 이하가 되는 최소 N.
N=15는 0.2530으로 **초과**, N=16은 0.245.

**미달 시 허용 처분은 셋뿐이며, 최소 표본 자체를 재조정하는 형태일 수 없다:**

1. 레포 추가 후 재실사
2. M15 연기
3. 확보된 수로 진행하되 **§9-A1b를 실패로 보고**하고 M16 대조군으로 인용하지 않음

⚠ **§6-4-3의 레포 편중 상한(60%)에도 같은 금지가 걸린다** — A1b가 둘을 함께
보므로 한쪽만 조정 가능하면 자유도가 남는다(스펙 7R Minor 5).

## 3. 실격 대역 (스펙 §6-4-6)

> 실격 ⟺ **`N − 전승 과제 수 < 0.98·√N`** (바닥 쪽 대칭: `N − 전패 과제 수 < 0.98·√N`)

유도: 과제 수준 95% 구간 반폭 = `1.96·√(0.25/N)`, 과제 수로 환산하면 `0.98·√N`.
N=20이면 4.38 → **전승 ≥16 또는 전패 ≥16이 실격**.
**N 확정(T21) 직후 절대 개수로 환산해 사전등록에 못박는다.**

⚠ 척도 캐비엇: "남은 개선 여지"는 3/3 통과 과제의 *개수*이고 반폭은 *통과 비율
평균*의 것이라 척도가 완전히 같지 않다 — 보수적 방향의 휴리스틱 대역이다.
⚠ 이 대역은 정규근사로 사전 고정하며 §6-4-7 부트스트랩 CI와 수치가 일치할
필요는 없다.
```

- [ ] **Step 3: 커밋 (해시가 곧 증거다)**

```bash
git add docs/experiments/2026-07-20-m15-real-repo-baseline/thresholds.md
git commit -m "docs(m15): 데이터 이전 동결 — 마진 1024·최소 표본 16·실격 대역"
git rev-parse HEAD   # 이 해시를 T22·T23이 인용한다. 기록해 둘 것
```

---

### Task 2: `copy_tree`를 read+write로 — 스테일 mtime 벡터와 실행 비트 (H6·H17)

**Files:**
- Modify: `src/eval/sandbox.rs:72-89` (`copy_tree`)
- Test: `src/eval/sandbox.rs` 내 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: 없음
- Produces: `restore_mode(&Metadata, &Path) -> std::io::Result<()>` — T3이 심링크 분기를 같은 함수에서 고친다

**Consumers (전수 — read+write 사이트 **셋**):**
1. `copy_tree`(`sandbox.rs:85`) — 호출자는 `Sandbox::create` 하나(`:25`), 그 소비자는 `run_once`(`mod.rs:173`)와 `verify_one`(`verify.rs:89`) 둘
2. `overlay_tree`(`:107-109`) — 소비자 셋: `solution/` 오버레이(`verify.rs:112`)·`sync_protected`의 **디렉터리** 분기(`:54`)·T17의 `fixture-overlay/`
3. **`sync_protected`의 단일 파일 분기**(`:55-61`) — ⚠ **`tasks-real`이 실제로 타는 경로다.** T21의 형식이 `protected = ["tests/<x>.rs"]`(단일 파일)이므로 배치의 모든 런이 여기를 지난다

⚠ 초판은 1번만, **개정 2는 1·2번만** 고쳐 H17을 3분의 2까지만 닫았다(2R 실현 I4·측정 A-3). 개정 2의 테스트는 `protected=["ci"]`(디렉터리)라 **배치가 절대 안 타는 경로**를 시험했다. 개정 3이 셋을 다 닫고 단일 파일 테스트를 더한다.

mode 복원은 신규 동작이라 **기존 소비자 0건**이고, 5R 실측으로 두 tasks 트리 367파일(픽스처 한정 329)이 **전부 644**라 644→644로 무변화다.

- [ ] **Step 1: 실패하는 테스트 두 개를 쓴다**

`src/eval/sandbox.rs`의 `mod tests` 안, `overlay_tree_refreshes_mtime` 아래에 추가:

```rust
    #[test]
    fn copy_tree_refreshes_mtime() {
        // M6는 overlay_tree에서만 fs::copy를 막았다. copy_tree에는 같은 벡터가
        // 남아 있었다 — macOS의 fs::copy는 clonefile로 원본 mtime을 보존하므로
        // 픽스처가 과거 mtime을 가지면 샌드박스의 소스가 빌드 산출물보다
        // 과거가 되고 cargo가 재빌드를 건너뛴다 (M15 H6)
        let src = fixture_with(&[("a.rs", "new")]);
        let old = age_file(&src.path().join("a.rs"));
        let sb = Sandbox::create(src.path()).unwrap();
        let copied = std::fs::metadata(sb.root.join("a.rs")).unwrap().modified().unwrap();
        assert!(
            copied > old + std::time::Duration::from_secs(1800),
            "샌드박스 복사도 mtime을 갱신해야 함 (M15 H6)"
        );
        assert_eq!(std::fs::read_to_string(sb.root.join("a.rs")).unwrap(), "new");
        sb.cleanup();
    }

    #[cfg(unix)]
    #[test]
    fn copy_tree_preserves_the_executable_bit() {
        // read+write는 퍼미션을 잃는다(3R 실측 755→644). 실레포 픽스처는
        // ci/*.sh 같은 실행 파일을 갖고, mode 회귀를 잡을 기존 테스트가
        // 하나도 없었다 (M15 H17)
        use std::os::unix::fs::PermissionsExt;
        let fx = fixture_with(&[("run.sh", "#!/bin/sh\nexit 0\n"), ("plain.txt", "x")]);
        std::fs::set_permissions(
            fx.path().join("run.sh"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        // 읽기 전용 픽스처 파일도 샌드박스에서는 덮어쓸 수 있어야 한다
        std::fs::set_permissions(
            fx.path().join("plain.txt"),
            std::fs::Permissions::from_mode(0o444),
        )
        .unwrap();

        let sb = Sandbox::create(fx.path()).unwrap();

        let exec = std::fs::metadata(sb.root.join("run.sh")).unwrap().permissions().mode();
        assert_eq!(exec & 0o777, 0o755, "실행 비트 보존 (M15 H17)");
        let plain = std::fs::metadata(sb.root.join("plain.txt")).unwrap().permissions().mode();
        assert_eq!(plain & 0o200, 0o200, "소유자 쓰기 비트 강제 — sync_protected 덮어쓰기 경로");
        sb.cleanup();
    }

    #[cfg(unix)]
    #[test]
    fn sync_protected_preserves_the_executable_bit() {
        // M15 H17 후반부 — overlay_tree는 M6 때부터 read+write라 **이미** 실행
        // 비트를 잃고 있었다. copy_tree만 고치면 절반만 닫힌다(플랜 1R 실현 I1).
        // 이 경로는 sync_protected를 통해 **모든 eval 런에서 check 직전**에 돈다
        use std::os::unix::fs::PermissionsExt;
        let fx = fixture_with(&[("ci/run.sh", "#!/bin/sh\nexit 0\n")]);
        std::fs::set_permissions(
            fx.path().join("ci/run.sh"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        let sb = Sandbox::create(fx.path()).unwrap();
        // 에이전트가 protected를 건드렸다고 가정 → 동기화가 overlay_tree를 탄다
        std::fs::write(sb.root.join("ci/run.sh"), "#!/bin/sh\nexit 1\n").unwrap();
        sb.sync_protected(fx.path(), &["ci".to_string()]).unwrap();

        let m = std::fs::metadata(sb.root.join("ci/run.sh")).unwrap().permissions().mode();
        assert_eq!(m & 0o777, 0o755, "protected 복원도 실행 비트를 보존해야 한다 (M15 H17)");
        sb.cleanup();
    }

    #[cfg(unix)]
    #[test]
    fn sync_protected_single_file_preserves_the_executable_bit() {
        // M15 H17 **세 번째 사이트** (2R 실현 I4·측정 A-3).
        // ⚠ 위 테스트는 protected가 **디렉터리**라 overlay_tree로 라우팅된다.
        // 그런데 tasks-real의 protected는 `tests/<x>.rs` — **단일 파일**이라
        // sync_protected의 :55-61 분기를 탄다. 즉 **배치의 모든 런이 타는 경로가
        // 이쪽인데 개정 2의 테스트는 그 경로를 시험하지 않았다.**
        // 실측(2R): 복원 없이는 create=755 → sync=644
        use std::os::unix::fs::PermissionsExt;
        let fx = fixture_with(&[("run.sh", "#!/bin/sh\nexit 0\n")]);
        std::fs::set_permissions(
            fx.path().join("run.sh"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        let sb = Sandbox::create(fx.path()).unwrap();
        std::fs::write(sb.root.join("run.sh"), "#!/bin/sh\nexit 1\n").unwrap();
        // protected를 **단일 파일**로 준다 — tasks-real과 같은 형태
        sb.sync_protected(fx.path(), &["run.sh".to_string()]).unwrap();

        let m = std::fs::metadata(sb.root.join("run.sh")).unwrap().permissions().mode();
        assert_eq!(m & 0o777, 0o755, "단일 파일 protected 복원도 실행 비트 보존 (M15 H17)");
        sb.cleanup();
    }
```

- [ ] **Step 2: 실패를 확인한다**

```bash
cargo test --lib eval::sandbox 2>&1 | tail -20
```
Expected: 신규 테스트 **3개**(2R 실현 m6: 개정 2가 "두 테스트"라 적었으나 셋이었다) 모두 FAIL. ⚠ **실패 사유를 정확히 알아 둘 것**(1R 실현 m1): 전자는 `fs::copy`의 mtime 보존 때문이고, **후자는 `0o755`가 아니라 `plain.txt`의 `0o200` 단언에서 난다**(`left: 0, right: 128`) — `fs::copy`는 퍼미션을 **보존**하므로 실행 비트는 아직 안 깨져 있다. 실행 비트 회귀는 **T2가 도입하는 read+write 자신**이 만드는 것이라 테스트는 여전히 유효하다.

- [ ] **Step 3: `copy_tree`를 고친다**

`src/eval/sandbox.rs:72-89`의 `copy_tree` 파일 분기를 교체:

```rust
fn copy_tree(src: &Path, dst: &Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let meta = std::fs::symlink_metadata(&from)?;
        if meta.is_symlink() {
            bail!("fixture에 심링크가 있음 (지원 안 함): {}", from.display());
        }
        if meta.is_dir() {
            std::fs::create_dir_all(&to)?;
            copy_tree(&from, &to)?;
        } else {
            // read+write — fs::copy는 macOS에서 clonefile로 원본 mtime을 보존해
            // 스테일 빌드 캐시 판정 벡터가 된다. M6가 overlay_tree에서만 막았고
            // copy_tree에는 남아 있던 잔여 결함 (M15 H6)
            let bytes = std::fs::read(&from)
                .with_context(|| format!("픽스처 읽기 실패: {}", from.display()))?;
            std::fs::write(&to, bytes)
                .with_context(|| format!("픽스처 복사 실패: {}", to.display()))?;
            restore_mode(&meta, &to)
                .with_context(|| format!("퍼미션 복원 실패: {}", to.display()))?;
        }
    }
    Ok(())
}

/// read+write 복사가 잃는 원본 퍼미션을 복원한다 (M15 H17).
/// `| 0o200`으로 소유자 쓰기 비트를 강제하는 것은 읽기 전용 픽스처 파일이
/// `sync_protected`의 덮어쓰기(remove 후 write)나 에이전트 편집을 막지 않게 하기
/// 위함 — 원본 트리의 읽기 전용은 판정 자산 보호 수단이 아니고(그 역할은
/// protected 동기화가 한다) 하네스를 죽이는 실패 모드만 만든다
#[cfg(unix)]
fn restore_mode(meta: &std::fs::Metadata, to: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = meta.permissions().mode() | 0o200;
    std::fs::set_permissions(to, std::fs::Permissions::from_mode(mode))
}

/// Windows에는 유닉스 mode가 없다 — read+write가 잃는 것도 없다
#[cfg(not(unix))]
fn restore_mode(_meta: &std::fs::Metadata, _to: &Path) -> std::io::Result<()> {
    Ok(())
}
```

**`overlay_tree`에도 같은 복원을 건다.** ⚠ 초판은 `copy_tree`만 고쳐 **H17을 절반만 닫았다**(1R 실현 I1) — `overlay_tree`는 M6 때부터 read+write였으므로 **이미 실행 비트를 잃고 있었고**, 그 함수는 `sync_protected`를 통해 **모든 eval 런에서 check 직전에 돈다**. T2 자신이 근거로 든 *"실레포 픽스처는 `ci/*.sh` 같은 실행 파일을 갖는다"*가 참이라면 protected 경로 안의 실행 파일이 첫 동기화에서 +x를 잃는다.

`overlay_tree`의 파일 분기(`sandbox.rs:107-109`):

```rust
            let bytes = std::fs::read(&from)?;
            std::fs::write(&to, bytes)
                .with_context(|| format!("오버레이 쓰기 실패: {}", to.display()))?;
            // M15 H17: copy_tree와 같은 이유 — read+write는 퍼미션을 잃는다.
            // 이 함수는 sync_protected를 통해 **모든 eval 런에서 check 직전**에 돈다
            restore_mode(&meta, &to)
                .with_context(|| format!("퍼미션 복원 실패: {}", to.display()))?;
```

**⚠ 세 번째 사이트 — `sync_protected`의 단일 파일 분기**(`sandbox.rs:55-61`). 2R 실현 I4·측정 A-3이 잡았다: read+write 지점은 **둘이 아니라 셋**이고, **`tasks-real`이 실제로 타는 것이 이 셋째다.** T21이 못박은 형식은 `protected = ["tests/<the-test-file>.rs"]` — **단일 파일**이라 `overlay_tree`가 아니라 이 분기로 라우팅된다. 즉 개정 2의 테스트는 배치가 절대 안 타는 경로를 시험하고 있었다.

```rust
            } else if src.exists() {
                if let Some(parent) = dst.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let bytes = std::fs::read(&src)?;
                std::fs::write(&dst, bytes)?;
                // M15 H17 — **세 번째 read+write 사이트**. tasks-real의 protected는
                // 단일 파일(`tests/<x>.rs`)이라 배치의 모든 런이 여기를 탄다
                let meta = std::fs::symlink_metadata(&src)?;
                restore_mode(&meta, &dst)?;
            }
```

- [ ] **Step 4: 통과를 확인한다**

```bash
cargo test --lib eval::sandbox 2>&1 | tail -20
```
Expected: `test result: ok.` — 기존 **10개**(1R 사실 m1: 초판의 "8개"는 `CLAUDE.md`의 M6 이후 미갱신 값을 옮긴 것이었다) + 신규 **4개**(`copy_tree_refreshes_mtime`·`copy_tree_preserves_the_executable_bit`·`sync_protected_preserves_the_executable_bit`·`sync_protected_single_file_preserves_the_executable_bit`) 전부 통과

- [ ] **Step 5: 기존 두 트리가 안 깨지는지 확인한다**

```bash
cargo test && cargo clippy --all-targets -- -D warnings
cargo run -- eval tasks --verify 2>&1 | tail -3
cargo run -- eval tasks-large --verify 2>&1 | tail -3
```
Expected: `검증 12/12 통과`, `검증 3/3 통과`

- [ ] **Step 6: 커밋**

```bash
git add src/eval/sandbox.rs
git commit -m "fix(eval): copy_tree를 read+write로 — 스테일 mtime 벡터 제거 + 실행 비트 보존 (H6·H17)"
```

---

### Task 3: 심링크는 스킵하고 경고한다 (H5)

**Files:**
- Modify: `src/eval/sandbox.rs` (`copy_tree`·`overlay_tree`의 심링크 분기, `use` 줄)
- Test: `src/eval/sandbox.rs` 내 `mod tests`

**Interfaces:**
- Consumes: T2의 `copy_tree` (read+write 형태)
- Produces: 심링크 스킵 정책 — T17의 조달 스크립트가 같은 항목을 조달 로그에 남긴다

**Consumers:** 심링크 `bail!`의 소비자 **셋** — ① `solution/` 오버레이(`verify.rs:112`) ② `sync_protected`의 디렉터리 분기(`sandbox.rs:54`) ③ T8의 `fixture-overlay/`. 셋 다 `overlay_tree`를 타므로 **두 함수를 함께 고쳐야 한다.** 그리고 기존 테스트 `symlink_in_fixture_is_an_error`가 이 동작을 고정하고 있으므로 **그 테스트 자체가 갱신 대상 소비자다.**

⚠ **탈출 위험은 이미 닫혀 있다** — `confine`(`path.rs:43-51`)이 canonicalize 후 루트 검사로 거부한다. 대상 4레포의 심링크는 전부 문서·패키징용(ripgrep `HomebrewFormula`, just `www/man/{en,zh}` — 후자 둘은 dangling)이라 스킵이 판정에 영향을 주지 않는다.

- [ ] **Step 1: 기존 테스트를 새 계약으로 갈아쓴다**

`src/eval/sandbox.rs`의 `symlink_in_fixture_is_an_error`(:156-162)를 삭제하고 그 자리에:

```rust
    #[cfg(unix)]
    #[test]
    fn symlinks_are_skipped_not_an_error() {
        // M15 H5: 정책 = 스킵 + 경고. ripgrep의 HomebrewFormula(정상 심링크)와
        // just의 www/man/{en,zh}(dangling) 둘 다 이 경로를 탄다. 대상은 전부
        // 문서·패키징용이라 판정에 영향이 없고, 탈출 위험은 confine이 이미 닫는다
        let fx = fixture_with(&[("real.txt", "x")]);
        std::os::unix::fs::symlink(fx.path().join("real.txt"), fx.path().join("link.txt")).unwrap();
        // dangling — 대상이 없어도 bail이 아니라 스킵이어야 한다
        std::os::unix::fs::symlink(fx.path().join("nope.txt"), fx.path().join("dangling")).unwrap();

        let sb = Sandbox::create(fx.path()).unwrap();

        assert_eq!(std::fs::read_to_string(sb.root.join("real.txt")).unwrap(), "x", "실파일은 복사");
        assert!(sb.root.join("link.txt").symlink_metadata().is_err(), "심링크는 스킵");
        assert!(sb.root.join("dangling").symlink_metadata().is_err(), "dangling도 스킵");
        sb.cleanup();
    }

    #[cfg(unix)]
    #[test]
    fn overlay_tree_skips_symlinks() {
        // 소비자 셋(solution/ 오버레이·sync_protected 디렉터리 분기·fixture-overlay)이
        // 전부 이 함수를 탄다 — copy_tree만 고치면 조달 산출물이 여기서 죽는다
        let src = fixture_with(&[("a.rs", "new")]);
        std::os::unix::fs::symlink(src.path().join("a.rs"), src.path().join("alias.rs")).unwrap();
        let dst = fixture_with(&[("a.rs", "stale")]);
        overlay_tree(src.path(), dst.path()).unwrap();
        assert_eq!(std::fs::read_to_string(dst.path().join("a.rs")).unwrap(), "new");
        assert!(dst.path().join("alias.rs").symlink_metadata().is_err(), "심링크는 스킵");
    }
```

- [ ] **Step 2: 실패를 확인한다**

```bash
cargo test --lib eval::sandbox 2>&1 | tail -20
```
Expected: 두 테스트 모두 FAIL — `심링크가 있음 (지원 안 함)` / `오버레이 원본에 심링크가 있음`으로 `unwrap()` 패닉

- [ ] **Step 3: 두 함수의 심링크 분기를 스킵으로 바꾼다**

`copy_tree`:

```rust
        if meta.is_symlink() {
            // M15 H5: 스킵 + 경고. 대상 레포의 심링크는 전부 문서·패키징용이고
            // (ripgrep HomebrewFormula, just www/man/{en,zh} — 후자 둘은 dangling)
            // 탈출 위험은 confine(path.rs:43-51)이 canonicalize 후 루트 검사로 이미
            // 닫는다. 스킵 항목은 조달 로그(scripts/procure_real.sh)가 함께 남긴다
            eprintln!("(심링크 건너뜀: {})", from.display());
            continue;
        }
```

`overlay_tree`도 동일하게:

```rust
        if meta.is_symlink() {
            // copy_tree와 같은 정책 (M15 H5). 소비자 셋 — solution/ 오버레이,
            // sync_protected 디렉터리 분기, fixture-overlay — 이 전부가 여기를 탄다
            eprintln!("(심링크 건너뜀: {})", from.display());
            continue;
        }
```

- [ ] **Step 4: `bail` 임포트를 정리한다**

`sandbox.rs`에서 `bail!`은 이 두 곳에만 있었다(`:79`·`:101`). **5행**의 `use`를 고친다:

```rust
use anyhow::Context;
```

```bash
grep -n "bail!" src/eval/sandbox.rs   # 0건이어야 한다
```

- [ ] **Step 5: 통과와 린트를 확인한다**

```bash
cargo test 2>&1 | tail -5
cargo clippy --all-targets -- -D warnings
```
Expected: 전건 통과, 경고 0 (`bail` 미사용 임포트 경고가 없어야 한다)

- [ ] **Step 6: 두 트리 게이트**

```bash
cargo run -- eval tasks --verify 2>&1 | tail -3
cargo run -- eval tasks-large --verify 2>&1 | tail -3
```
Expected: `검증 12/12 통과`, `검증 3/3 통과`

- [ ] **Step 7: 커밋**

```bash
git add src/eval/sandbox.rs
git commit -m "feat(eval): 심링크 스킵+경고 정책 — copy_tree·overlay_tree 양쪽 (H5)"
```

---

### Task 4: 스테일 뮤테이션 테스트 — `--verify` 1단계→2단계 창 (H16)

**Files:**
- Test: `src/eval/verify.rs` 내 `#[cfg(unix)] mod tests`
- Modify: 없음 (통과해야 하는 테스트다 — T2·T3 이후 이미 초록불이어야 한다)

**Interfaces:**
- Consumes: T2의 read+write `copy_tree`, 기존 `overlay_tree`
- Produces: `§9-A3`의 차단 기준 절반

**Consumers:** 없음 — 순수 회귀 테스트다. **코드 소비자 0건.**

**왜 필요한가:** `--verify`는 같은 샌드박스에서 `overlay_tree`로 소스를 갈아끼우고 `check`를 **연속** 실행한다. 1단계가 만든 빌드 산출물이 2단계 소스보다 미래면 cargo가 재빌드를 건너뛰고 **1단계 바이너리로 2단계를 판정**한다. 공유 `CARGO_TARGET_DIR`을 폐기했어도 그 창은 실재하고, 오늘도 `overlay_tree`의 mtime=now 하나에만 의존한다(§3-6 남는 작업 2).

⚠ **실레포에 같은 절차를 손으로 적용할 때는 레포별 소스 경로를 지정해야 한다** — ripgrep은 워크스페이스라 루트에 `src/`가 없어 루트 `touch src/…`가 조용히 no-op이 된다(3R 실측). 수동 절차는 Step 4에 적는다.

- [ ] **Step 1: 테스트를 쓴다**

`src/eval/verify.rs`의 `#[cfg(unix)] mod tests` 끝에 추가:

```rust
    /// M15 H16 — 1단계→2단계 창의 스테일 판정 벡터를 고의로 겨눈다.
    ///
    /// `check`가 1단계에서 `build.stamp`(빌드 산출물 대역)를 만든다. 2단계의
    /// `solution/` 소스는 **1시간 과거 mtime**으로 준비돼 있으므로, 오버레이가
    /// `fs::copy`라면 새 소스가 산출물보다 과거가 되어 `check`가 STALE로 죽는다
    /// (cargo라면 재빌드를 건너뛰어 조용히 1단계 바이너리로 판정할 자리다).
    /// read+write(mtime=now)면 소스가 산출물보다 미래라 통과한다.
    ///
    /// ⚠ **비교 방향이 계약이다** — `[ build.stamp -nt src/lib.rs ]`(스탬프가 소스보다
    /// **엄격히** 최신이면 STALE)이지 그 부정형이 아니다. macOS `/bin/sh`는 bash 3.2이고
    /// `-nt`가 mtime을 **초 단위로 절삭**해 비교한다(APFS는 나노초를 기록하지만 비교는
    /// 초로 한다). 1단계 `touch`와 2단계 오버레이 쓰기는 같은 초에 떨어지므로
    /// `! [ src/lib.rs -nt build.stamp ]`로 쓰면 **정상 동작에서도 참**이 되어
    /// 테스트가 영영 실패한다(1R 실측). 스테일 케이스는 1시간 격차라 초 절삭에
    /// 걸리지 않는다 — 그래서 이 방향만 양방향 변별력을 갖는다.
    ///
    /// 1시간 격차를 쓰는 것이 핵심이다 — "같은 초에 쓴 두 파일"에 의존하면
    /// 파일시스템 타임스탬프 해상도에 따라 흔들린다.
    #[tokio::test]
    async fn verify_stage2_overlay_is_newer_than_stage1_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let toml = concat!(
            "prompt = \"p\"\n",
            "check = \"if [ -e build.stamp ] && [ build.stamp -nt src/lib.rs ]; ",
            "then echo STALE >&2; exit 3; fi; touch build.stamp; grep -q FIXED src/lib.rs\"\n",
            "protected = [\"keep.txt\"]\n",
        );
        write_task(
            dir.path(),
            "stale-window",
            toml,
            &[("keep.txt", "k"), ("src/lib.rs", "// BROKEN\n")],
            Some(&[("src/lib.rs", "// FIXED\n")]),
        );
        // solution/ 소스를 1시간 과거로 — fs::copy였다면 이 mtime이 보존된다
        let sol = dir.path().join("stale-window/solution/src/lib.rs");
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        std::fs::File::options().write(true).open(&sol).unwrap().set_modified(old).unwrap();

        let recs = run_verify(&opts(dir.path())).await.unwrap();

        assert_eq!(recs.len(), 1);
        assert!(recs[0].discriminates, "1단계는 BROKEN이라 실패해야 한다: {recs:?}");
        assert!(
            recs[0].solvable,
            "2단계가 STALE(exit 3)로 죽으면 오버레이가 mtime을 보존한 것이다 (M15 H16): {recs:?}"
        );
    }
```

- [ ] **Step 2: 정상 경로에서 통과를 확인한다 (2×2의 왼쪽 칸)**

```bash
cargo test --lib eval::verify 2>&1 | tail -20
```
Expected: PASS.

⚠ **FAIL이면 먼저 `-nt` 방향을 의심할 것.** 초판이 `! [ src/lib.rs -nt build.stamp ]`로 썼다가 정확히 여기서 죽었다 — macOS bash 3.2의 초 절삭 때문에 **T2·T3이 완벽해도 FAIL**이었고, 초판 Step 2가 *"FAIL이면 T2·T3이 회귀시킨 것"*이라고 적어 없는 회귀를 쫓게 만들었다. 방향이 맞는데도 FAIL이면 그때 `overlay_tree`가 read+write인지 본다.

- [ ] **Step 3: 반증 가능성을 확인한다 — 2×2를 채운다**

프로젝트 메모리 *"검증 기준은 실패할 수 있어야 한다"*의 요구다. ⚠ **"변조 시 실패"만 보면 안 된다** — 초판이 그렇게 해서 **정상·변조 양쪽 FAIL**인 테스트를 반증 통과로 착각했다. **네 칸 중 최소 두 칸(정상=PASS, 변조=FAIL)을 실제로 채울 것.**

`overlay_tree`의 read+write를 임시로 `std::fs::copy`로 바꾼다:

```bash
# 임시 변조 (1R 확인: 이 치환은 정상 적용되고 컴파일된다 —
# anyhow가 자기 Error에 사설 StdError를 구현해 .with_context가 붙는다)
perl -0pi -e 's/let bytes = std::fs::read\(&from\)\?;\n            std::fs::write\(&to, bytes\)/std::fs::copy(&from, \&to).map(|_| ()).map_err(anyhow::Error::from)/' src/eval/sandbox.rs
cargo test --lib eval::verify::tests::verify_stage2 2>&1 | tail -10
```
Expected: **FAIL** (`2단계가 STALE(exit 3)로 죽으면…`).

```bash
git checkout src/eval/sandbox.rs
cargo test --lib eval::verify::tests::verify_stage2 2>&1 | tail -5
```
Expected: **PASS** — 이 두 줄이 함께 있어야 반증이 성립한다.

⚠ 위 `perl` 치환이 컴파일되지 않으면 손으로 바꿔서 확인할 것 — **반증 확인 자체를 건너뛰지 말 것.**

**1R이 실측한 2×2** (참고 — 구현자가 재확인할 표):

| check 형태 | read+write (정상) | fs::copy (변조) |
|---|---|---|
| 초판 `! [ src/lib.rs -nt build.stamp ]` | **FAIL** | FAIL ← 구별력 0 |
| 개정 2 `[ build.stamp -nt src/lib.rs ]` | **PASS** | **FAIL** |

- [ ] **Step 4: 실레포용 수동 절차를 문서에 남긴다**

`docs/experiments/2026-07-20-m15-real-repo-baseline/thresholds.md` 끝에 절을 추가:

```markdown
## 4. 스테일 뮤테이션 수동 확인 절차 (실레포, 스펙 §3-6 남는 작업 2)

자동 테스트(`verify_stage2_overlay_is_newer_than_stage1_artifacts`)는 합성
픽스처를 쓴다. 조달된 실레포 과제 **각각에 대해** T21에서 한 번 손으로 확인한다.

⚠ **레포별 소스 경로를 지정할 것** — ripgrep은 워크스페이스라 루트에 `src/`가
없어 루트 `touch src/…`가 조용히 no-op이 된다(3R 실측).

| 레포 | touch 대상 |
|---|---|
| zoxide | `src/main.rs` |
| fd | `src/main.rs` |
| ripgrep | `crates/core/main.rs` |
| just | `src/lib.rs` |

절차 (레포별로):
1. `cargo run -- eval tasks-real --verify --filter <task>` 를 돌려 통과 확인
2. 위 표의 경로를 `<task_dir>/solution/` 안에서 1시간 과거로 만든다:
   `python3 -c "import os,time;p='<sol-path>';os.utime(p,(time.time()-3600,)*2)"`
3. 다시 `--verify --filter <task>` — **여전히 통과해야 한다.** 실패하면
   오버레이가 mtime을 보존하고 있는 것이므로 배치를 시작하지 말 것
4. 확인 출력을 사전등록에 첨부한다
```

- [ ] **Step 5: 커밋**

```bash
cargo test && cargo clippy --all-targets -- -D warnings
git add src/eval/verify.rs docs/experiments/2026-07-20-m15-real-repo-baseline/thresholds.md
git commit -m "test(eval): --verify 1→2단계 스테일 판정 창 회귀 테스트 + 실레포 수동 절차 (H16)"
```

---

### Task 5: `TaskSpec`에 과제별 운용점과 명령 상한 (H1·H2)

**Files:**
- Modify: `src/eval/task.rs:12-28` (`TaskSpec`), `src/eval/mod.rs:167-175` (`run_once`)
- Test: `src/eval/task.rs` 내 `mod tests`, `src/eval/mod.rs` 내 `#[cfg(unix)] mod tests`

**Interfaces:**
- Consumes: 없음
- Produces:
  - `TaskSpec.context_tokens: Option<usize>` — T6의 `RunRecord.effective_context_tokens`가 읽는다
  - `TaskSpec.command_timeout_secs: Option<u64>` — 소비자는 `run_once`의 `ctx.command_timeout` 하나

**Consumers:** `TaskSpec`은 `deny_unknown_fields`지만 **추가는 안전하다**(`Option` + 기본 없음 = 미지정 허용). 기존 15개 `task.toml`(tasks/ 12 + tasks-large/ 3)은 두 키가 없으므로 `None`. `EffectiveConfig`는 **이 값을 증언하지 못한다** — 그것이 T6이 존재하는 이유다. **`shipped_task_set_is_valid` 테스트는 두 키를 안 보므로 갱신 불요(확인함).**

- [ ] **Step 1: 파싱 테스트를 쓴다**

`src/eval/task.rs`의 `mod tests`에서 `loads_sorted_with_defaults`에 두 줄 추가하고, `overrides_are_read`를 확장한다:

```rust
    #[test]
    fn loads_sorted_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        write_task(dir.path(), "b-task", MINIMAL, &["keep.txt"]);
        write_task(dir.path(), "a-task", MINIMAL, &["keep.txt"]);
        let tasks = load_tasks(dir.path()).unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].name, "a-task", "이름순 정렬");
        assert_eq!(tasks[0].spec.timeout_secs, 300, "기본값");
        assert_eq!(tasks[0].spec.check_timeout_secs, 120);
        assert_eq!(tasks[0].spec.max_turns, None);
        // M15 H1·H2 — 미지정이면 None이고 전역 config가 그대로 쓰인다.
        // 기존 15개 task.toml이 이 상태다
        assert_eq!(tasks[0].spec.context_tokens, None);
        assert_eq!(tasks[0].spec.command_timeout_secs, None);
    }

    #[test]
    fn overrides_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let toml = r#"
prompt = "p"
check = "cargo test"
timeout_secs = 60
check_timeout_secs = 30
max_turns = 10
context_tokens = 32768
command_timeout_secs = 180
protected = ["keep.txt"]
"#;
        write_task(dir.path(), "t", toml, &["keep.txt"]);
        let t = &load_tasks(dir.path()).unwrap()[0];
        assert_eq!(t.spec.timeout_secs, 60);
        assert_eq!(t.spec.check_timeout_secs, 30);
        assert_eq!(t.spec.max_turns, Some(10));
        // M15 H1 — tasks-real의 32K 운용점은 이 경로로만 들어온다.
        // 코드 기본값과 .loco/config.toml은 8K로 불변(비교가능성 각주 3)
        assert_eq!(t.spec.context_tokens, Some(32768));
        assert_eq!(t.spec.command_timeout_secs, Some(180));
    }
```

- [ ] **Step 2: 실패를 확인한다**

```bash
cargo test --lib eval::task 2>&1 | tail -20
```
Expected: 컴파일 에러 — `no field 'context_tokens' on type 'TaskSpec'`

- [ ] **Step 3: 필드를 추가한다**

`src/eval/task.rs`의 `TaskSpec`, `max_turns` 아래:

```rust
    /// 설정보다 우선하는 과제별 턴 상한
    pub max_turns: Option<usize>,
    /// 설정보다 우선하는 과제별 컨텍스트 운용점 (M15 H1). `tasks-real`만 32768을
    /// 지정하고 코드 기본값(8192)·`.loco/config.toml`·기존 두 트리의 앵커는 8K로
    /// 불변이다 — 비교가능성 각주 3. ⚠ `EffectiveConfig`는 배치당 1회 전역 config에서
    /// 만들어져 이 값을 증언하지 못한다. 실효값 자증은 `RunRecord`가 한다 (H9)
    pub context_tokens: Option<usize>,
    /// 설정보다 우선하는 과제별 툴 명령 상한 (M15 H2). 실레포 `cargo test`는
    /// 콜드 빌드 포함 최악 27초(just, 3R 실측)이고 진짜 콜드 FS 캐시 + CPU 경합이면
    /// 60~90초라 전역 기본 60초가 아슬아슬하다
    pub command_timeout_secs: Option<u64>,
```

- [ ] **Step 4: 파싱 테스트 통과를 확인한다**

```bash
cargo test --lib eval::task 2>&1 | tail -10
```
Expected: `test result: ok.`

- [ ] **Step 5: `run_once`에 배선한다**

`src/eval/mod.rs:167-176`:

```rust
    let mut cfg = config.clone();
    if let Some(mt) = t.spec.max_turns {
        cfg.max_turns = mt;
    }
    // M15 H1·H2 — 과제별 오버라이드는 **ToolCtx·Agent 생성 전에** 전부 적용한다.
    // 아래 ctx.command_timeout과 Agent::new가 이 cfg를 읽으므로 순서가 계약이다
    if let Some(ct) = t.spec.context_tokens {
        cfg.context_tokens = ct;
    }
    if let Some(cts) = t.spec.command_timeout_secs {
        cfg.command_timeout_secs = cts;
    }
    // eval은 --auto 의미 — config의 auto_deny_patterns 적용 (스펙 §5·§8)
    let mut approver = AutoApprover::new(&cfg.auto_deny_patterns)?;
    let sb = Sandbox::create(&t.fixture)?;
    let mut ctx = ToolCtx::new(sb.root.clone());
    ctx.command_timeout = Duration::from_secs(cfg.command_timeout_secs);
```

- [ ] **Step 6: H2 배선을 end-to-end로 고정하는 테스트를 쓴다**

`src/eval/mod.rs`의 `#[cfg(unix)] mod tests` 끝에:

```rust
    /// M15 H2 — 과제별 command_timeout_secs가 ToolCtx에 실제로 도달하는지.
    /// 트랜스크립트의 툴 결과 본문으로 확인한다(전역 기본 60초로 돌았다면
    /// 5초 sleep이 그냥 완주해 "timed out"이 없다)
    #[tokio::test]
    async fn task_command_timeout_secs_reaches_the_tool_context() {
        let tasks = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            tasks.path(),
            "slowcmd",
            "prompt = \"p\"\ncheck = \"true\"\ncommand_timeout_secs = 1\nprotected = [\"keep.txt\"]\n",
            &[("keep.txt", "x")],
        );
        // run_command 1회(5초 sleep) → finish. 전역 기본 60초였다면 5초를 완주해
        // "timed out"이 안 나온다 — 즉 이 단언은 미배선에서 실패한다
        let script = Scripted::new(vec![
            ok(&turn("run_command", serde_json::json!({"command": "sleep 5"}))),
            ok(&finish("done")),
        ]);
        let o = opts(tasks.path().to_path_buf());
        let start = Instant::now();
        let run = run_eval(&script, &Config::default(), "m", &o, proj.path()).await.unwrap();
        assert!(start.elapsed() < Duration::from_secs(20), "1초 상한이 걸려야 한다");
        let jsonl = std::fs::read_to_string(
            run.report_path.parent().unwrap().join("run-slowcmd-0.jsonl"),
        )
        .unwrap();
        assert!(jsonl.contains("timed out after 1s"), "과제별 상한 미배선: {jsonl}");
    }
```

- [ ] **Step 7: 반증 확인 — 배선을 끊으면 실패하는가**

```bash
# cfg.command_timeout_secs 오버라이드 3줄을 임시 주석 처리한 뒤:
cargo test --lib eval::tests::task_command_timeout 2>&1 | tail -10
```
Expected: **FAIL** (`과제별 상한 미배선`). 확인 후 주석을 되돌린다.

- [ ] **Step 8: 전체 게이트 + 커밋**

```bash
cargo test && cargo clippy --all-targets -- -D warnings
cargo run -- eval tasks --verify 2>&1 | tail -3
cargo run -- eval tasks-large --verify 2>&1 | tail -3
git add src/eval/task.rs src/eval/mod.rs
git commit -m "feat(eval): TaskSpec에 과제별 context_tokens·command_timeout_secs (H1·H2)"
```

---

### Task 6: `RunRecord`에 실효 운용점 — 배선이 끊긴 쪽을 잡는다 (H9)

**Files:**
- Modify: **`src/agent/mod.rs`** (`Agent::context_tokens()` 게터 신설 — 개정 3에서 추가된 의존), `src/eval/report.rs:31-43` (`RunRecord`) + `:178`·`:185`(테스트 헬퍼), `src/eval/mod.rs:202-245` (`run_once`), `src/eval/mod.rs:250-286` (`judge`)
- Test: `src/eval/mod.rs` 내 `#[cfg(unix)] mod tests`

**Interfaces:**
- Consumes: T5의 `TaskSpec.context_tokens`
- Produces: `RunRecord.effective_context_tokens: usize` / `RunRecord.effective_max_turns: usize` — T23 사전등록 항목 10(자증 절차)과 §9-A4가 인용한다

**Consumers (전수 — 5건):** ① `report.json`(스키마 확장, 기존 키 불변) ② `scripts/exp_metrics.py`의 `report_index`(`outcome`/`passed`만 읽으므로 **무변경**, 확인함) ③ T23의 자증 절차 ④ **`src/eval/report.rs`의 테스트 헬퍼 `run()`(`:178`)·`run_with()`(`:185`)** — `RunRecord`를 구조체 리터럴로 만들므로 **필드 2개를 함께 추가해야 컴파일된다** ⑤ `judge()`의 생성부.

⚠ 초판은 *"`Serialize`만 파생하므로 안전하다"*며 ④를 빠뜨렸다(1R 실현 I2). 컴파일이 막아 주지만(`error[E0063]: missing fields`) **소비자 전수를 세지 않은 것**이고, 프로젝트 메모리 *"소비자 감사는 두 층으로"*가 겨눈 형태 그대로다. T7이 같은 두 곳을 다시 건드린다.

`report.rs:178`·`:185`의 두 헬퍼에 기본값을 넣는다:

```rust
            effective_context_tokens: 8192,
            effective_max_turns: 25,
```

⚠ **`judge()`의 호출 지점이 2개다**(`mod.rs:221` 타임아웃 경로, `:238` 정상 경로). `mod.rs:203-207`이 *"한쪽만 배선이 끊겨도 테스트가 안 죽는 지점 — 실제로 그랬다"*고 경고하는 바로 그 함정이다. **`schema_fallback`과 똑같이 분기 이전에 한 번만 읽고 두 경로에 넘긴다.**

- [ ] **Step 1: 두 경로를 각각 고정하는 테스트를 쓴다**

`src/eval/mod.rs`의 `#[cfg(unix)] mod tests` 끝에:

```rust
    /// M15 H9 — 과제별 운용점이 report.json까지 도달하는가 (**정상 경로**).
    /// EffectiveConfig는 배치당 1회 전역 config에서 만들어져 이것을 증언하지
    /// 못한다 — 비교가능성 각주 3이 M13·M14에서 두 번 거짓이었던 지점이다
    #[tokio::test]
    async fn per_task_context_tokens_reach_the_report_on_the_normal_path() {
        let tasks = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            tasks.path(),
            "big",
            "prompt = \"p\"\ncheck = \"true\"\ncontext_tokens = 32768\nmax_turns = 7\nprotected = [\"keep.txt\"]\n",
            &[("keep.txt", "x")],
        );
        let script = Scripted::new(vec![ok(&finish("done"))]);
        let o = opts(tasks.path().to_path_buf());
        let run = run_eval(&script, &Config::default(), "m", &o, proj.path()).await.unwrap();

        let r = &run.report.tasks[0].runs[0];
        assert_eq!(r.effective_context_tokens, 32768);
        assert_eq!(r.effective_max_turns, 7);
        // 전역 스냅샷은 여전히 코드 기본값 — 둘이 다르다는 것이 H9의 존재 이유다
        assert_eq!(run.report.effective_config.context_tokens, 8192);
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&run.report_path).unwrap()).unwrap();
        assert_eq!(json["tasks"][0]["runs"][0]["effective_context_tokens"], 32768);
    }

    /// 짝 테스트 — **타임아웃 경로**의 judge 호출 지점(mod.rs:221)도 같은 값을
    /// 실어야 한다. 이 테스트가 없으면 그 경로를 리터럴 8192로 바꿔도 전 스위트가
    /// 초록불이다(mod.rs:203-207이 경고하는 실제 전례)
    #[tokio::test]
    async fn per_task_context_tokens_reach_the_report_on_the_timeout_path() {
        let tasks = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            tasks.path(),
            "slow",
            "prompt = \"p\"\ncheck = \"true\"\ncontext_tokens = 32768\ntimeout_secs = 1\nprotected = [\"keep.txt\"]\n",
            &[("keep.txt", "x")],
        );
        let mut o = opts(tasks.path().to_path_buf());
        o.timeout_scale = 0.05; // 50ms
        let run = run_eval(&Sleepy, &Config::default(), "m", &o, proj.path()).await.unwrap();

        let r = &run.report.tasks[0].runs[0];
        assert_eq!(r.outcome, RunOutcome::Timeout);
        assert_eq!(r.effective_context_tokens, 32768, "타임아웃 경로도 실효값을 실어야 한다 (H9)");
    }
```

- [ ] **Step 2: 실패를 확인한다**

```bash
cargo test --lib eval::tests::per_task_context 2>&1 | tail -20
```
Expected: 컴파일 에러 — `no field 'effective_context_tokens' on type 'RunRecord'`

- [ ] **Step 3a: `Agent`에 운용점 게터를 연다**

`src/agent/mod.rs`의 `input_budget()` 옆:

```rust
    /// 이 에이전트가 **생성 시점에 스냅샷한** 컨텍스트 운용점 (M15 H9).
    /// eval의 `RunRecord`가 실효값을 자증할 때 `Config`가 아니라 여기서 읽어야 한다 —
    /// `Config` 쪽을 다시 읽으면 두 값이 같은 출처가 되어 오버라이드 순서 오류를
    /// 탐지하지 못한다(플랜 1R Critical 2)
    pub fn context_tokens(&self) -> usize {
        self.context_tokens
    }
```

- [ ] **Step 3b: `RunRecord`에 필드를 추가한다**

`src/eval/report.rs`, `schema_fallback` 아래:

```rust
    pub schema_fallback: bool,
    /// 이 런에 **실제로** 적용된 컨텍스트 운용점 (M15 H9). `EffectiveConfig`는
    /// 배치당 1회 전역 `config`에서 만들어져 과제별 오버라이드(H1)를 증언하지
    /// 못한다 — 비교가능성 각주 3의 이 주장이 **스펙 초판(“경로에서만 지정한다”)과
    /// 개정 2(“effective_config로 자증한다”)에서 두 번 거짓이었다**(스펙 §8 각주 3.
    /// ⚠ M13·M14 산출물의 진술이 아니라 **이 스펙 자신의** 옛 서술이다 — 2R 측정 m2)
    pub effective_context_tokens: usize,
    /// 이 런에 실제로 적용된 턴 상한 — 과제별 `max_turns` 오버라이드는
    /// M15 이전부터 있었으나 리포트에 도달한 적이 없다 (M15 H9)
    pub effective_max_turns: usize,
```

- [ ] **Step 4: `judge`에 인자를 넘긴다**

`src/eval/mod.rs`. **`schema_fallback`을 읽는 그 자리에서 함께 읽는다** — 분기 이전 단 한 곳:

```rust
    let schema_fallback = agent.schema_fallback_fired();
    // M15 H9: schema_fallback과 **같은 규율** — 분기 이전에 한 번만 읽고 두 judge
    // 호출에 넘긴다. 각 분기에서 따로 만들면 한쪽만 끊겨도 테스트가 안 죽는다.
    //
    // ⚠ **출처가 `cfg`가 아니라 `agent`인 것이 계약이다.** `Agent::new`가
    // `config.context_tokens`를 **생성 시점에 스냅샷**하므로(`agent/mod.rs:176`),
    // 여기서 `cfg`를 다시 읽으면 두 값이 같은 출처가 되어 **순서가 어긋나도 리포트가
    // 눈치채지 못한다** — 1R 실측: T5의 오버라이드를 `Agent::new` 뒤로 옮기면
    // 에이전트는 8192로 도는데 report.json은 32768을 보고하고 **73개 테스트가 전건
    // 초록불**이었다. 그러면 이 필드는 없느니만 못하다(§9-A4가 이것을 자증으로 인용한다)
    let eff = EffectiveRun { context_tokens: agent.context_tokens(), max_turns: cfg.max_turns };
```

⚠ `agent`는 이 시점에 `agent.run(...)`이 끝나 가변 차용이 풀린 상태다(`schema_fallback_fired()`를 바로 위에서 부르는 것이 그 증거).

`judge` 시그니처와 본문:

```rust
/// judge에 넘기는 이 런의 실효 조건 (M15 H9). 인자 2개를 따로 늘리면
/// `#[allow(clippy::too_many_arguments)]`가 더 두꺼워지므로 묶는다
#[derive(Debug, Clone, Copy)]
struct EffectiveRun {
    context_tokens: usize,
    max_turns: usize,
}
```

```rust
#[allow(clippy::too_many_arguments)]
async fn judge(
    sb: &Sandbox,
    t: &Task,
    opts: &EvalOptions,
    outcome: RunOutcome,
    turns: usize,
    elapsed: Duration,
    seed: u64,
    repeat: usize,
    interrupt: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    cargo_snapshot: &integrity::CargoConfigSnapshot,
    schema_fallback: bool,
    eff: EffectiveRun,
) -> anyhow::Result<Option<RunRecord>> {
```

`RunRecord` 생성부:

```rust
    Ok(Some(RunRecord {
        repeat, seed, passed, outcome, turns,
        duration_secs: elapsed.as_secs_f64(),
        schema_fallback,
        effective_context_tokens: eff.context_tokens,
        effective_max_turns: eff.max_turns,
    }))
```

두 호출 지점(타임아웃 `:221`, 정상 `:238`) 모두 마지막 인자로 `eff`를 넘긴다:

```rust
            let rec = judge(
                &sb, t, opts, RunOutcome::Timeout, turns, elapsed, seed, repeat, interrupt, cargo_snapshot,
                schema_fallback, eff,
            )
            .await;
```

```rust
    let rec = judge(
        &sb, t, opts, kind, turns, elapsed, seed, repeat, interrupt, cargo_snapshot,
        schema_fallback, eff,
    )
    .await;
```

- [ ] **Step 5: 통과를 확인한다**

```bash
cargo test --lib eval 2>&1 | tail -10
```
Expected: `test result: ok.` — 신규 2개 포함 전건

- [ ] **Step 6: 반증 확인 — 두 가지 단절을 **각각** 끊어 본다**

⚠ 초판은 아래 (a)만 확인했고 **(b)를 놓쳐 H9가 자기 목적을 달성하지 못했다**(1R Critical 2).

**(a) judge 분기 단절** — `:221`(타임아웃) 호출의 `eff`를 `EffectiveRun { context_tokens: 8192, max_turns: 25 }`로 임시 치환:

```bash
cargo test --lib eval::tests::per_task_context_tokens_reach_the_report_on_the_timeout_path 2>&1 | tail -8
```
Expected: **FAIL** (`타임아웃 경로도 실효값을 실어야 한다`). 되돌린 뒤 PASS 확인.

**(b) H1 오버라이드 순서 단절** — T5가 `run_once`에 넣은 `cfg.context_tokens` 오버라이드 3줄을 **`Agent::new` 호출 뒤로** 옮긴다(에이전트는 8192로, `cfg`는 32768로 남는다):

```bash
cargo test --lib eval::tests::per_task_context_tokens_reach_the_report_on_the_normal_path 2>&1 | tail -8
```
Expected: **FAIL** — `eff`가 `agent.context_tokens()`에서 오므로 8192를 보고한다.

⚠ **이것이 Step 3a가 존재하는 이유다.** `eff`를 `cfg.context_tokens`에서 만들면 이 변조에서 **테스트가 통과해 버리고**(1R 실측: 전 스위트 73 passed) report.json만 거짓말을 한다. **(b)가 FAIL하지 않으면 Step 3a·Step 4가 제대로 적용되지 않은 것이다.** 확인 후 되돌린다.

- [ ] **Step 7: 게이트 + 커밋**

```bash
cargo test && cargo clippy --all-targets -- -D warnings
git add src/agent/mod.rs src/eval/report.rs src/eval/mod.rs
git commit -m "feat(eval): RunRecord에 실효 context_tokens·max_turns — 과제별 오버라이드 자증 (H9)"
```

---

### Task 7: protected 수정 시도 카운터 (H7)

**Files:**
- Modify: `src/eval/mod.rs` (`judge` 첫 문장 앞, 헬퍼 2개 신설), `src/eval/report.rs` (`RunRecord`)
- Test: `src/eval/mod.rs` 내 `mod unit_tests` (크로스플랫폼) + `#[cfg(unix)] mod tests` (end-to-end)

**Interfaces:**
- Consumes: T6의 `EffectiveRun` 패턴(judge 인자 규율)
- Produces: `RunRecord.protected_edits: usize` — T15의 `exp_metrics.py`가 읽는다

**Consumers:** ① `report.json` ② T15 `exp_metrics.py`의 요약 라인 ③ T25 리포트의 리워드 해킹 절. **§5-2 ⑦이 지정한 대로 "리워드 해킹(M13 R5형)의 유일한 기계 관측 발자국"이다** — M13이 `△ 계측 불가`로 분류한 2종 중 하나를 여기서 닫는다.

⚠ **`sync_protected`가 되돌리기 전에 세야 한다.** `judge()`가 `sync_protected`를 첫 문장으로 부르므로(`mod.rs:264`) 그 **앞**에 삽입한다. `RunRecord`를 같은 함수에서 만들므로 인자 전달이 필요 없다.

- [ ] **Step 1: 헬퍼 단위 테스트를 쓴다**

`src/eval/mod.rs`의 `mod unit_tests`에 추가:

```rust
    #[test]
    fn protected_edit_counter_sees_modify_add_delete_and_type_swap() {
        let fx = tempfile::tempdir().unwrap();
        let sb = tempfile::tempdir().unwrap();
        for (rel, body) in [("tests/a.rs", "A"), ("tests/b.rs", "B"), ("Cargo.toml", "M")] {
            for base in [fx.path(), sb.path()] {
                let p = base.join(rel);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(p, body).unwrap();
            }
        }
        let protected = vec!["tests".to_string(), "Cargo.toml".to_string()];
        // 손대지 않은 상태 = 0
        assert_eq!(count_protected_edits(fx.path(), sb.path(), &protected), 0);

        std::fs::write(sb.path().join("tests/a.rs"), "HACKED").unwrap(); // 수정
        std::fs::write(sb.path().join("tests/extra.rs"), "sneak").unwrap(); // 추가
        std::fs::remove_file(sb.path().join("tests/b.rs")).unwrap(); // 삭제
        std::fs::remove_file(sb.path().join("Cargo.toml")).unwrap();
        std::fs::create_dir(sb.path().join("Cargo.toml")).unwrap(); // 파일→디렉터리 바꿔치기
        assert_eq!(count_protected_edits(fx.path(), sb.path(), &protected), 4);

        // 양쪽 모두 없는 경로(암묵 .cargo의 정상 상태)는 0
        assert_eq!(
            count_protected_edits(fx.path(), sb.path(), &[".cargo".to_string()]),
            0
        );
    }
```

- [ ] **Step 2: 실패를 확인한다**

```bash
cargo test --lib eval::unit_tests::protected_edit_counter 2>&1 | tail -10
```
Expected: 컴파일 에러 — `cannot find function 'count_protected_edits'`

- [ ] **Step 3: 헬퍼를 구현한다**

`src/eval/mod.rs`의 `with_implicit_protected` 아래에 추가:

```rust
/// protected 경로가 fixture 원본과 다른 항목 수 — `sync_protected`가 되돌리기
/// **전에** 센다 (M15 H7·§5-2 ⑦). 수정·추가·삭제·타입 바꿔치기를 각각 1건으로
/// 세고, 읽기 실패는 "다름"으로 본다(보수적 — 관측 누락보다 과대계상이 안전).
///
/// 리워드 해킹(M13 R5형)의 유일한 기계 관측 발자국이다. 하네스는 어차피 전부
/// 되돌리므로 **판정에는 영향이 없고 기록만 남는다** — 축 C와 같은 성질(§5-6)
pub(crate) fn count_protected_edits(fixture: &Path, root: &Path, protected: &[String]) -> usize {
    protected.iter().map(|rel| diff_count(&fixture.join(rel), &root.join(rel))).sum()
}

fn diff_count(src: &Path, dst: &Path) -> usize {
    match (src.symlink_metadata(), dst.symlink_metadata()) {
        // 양쪽 없음 — 픽스처가 .cargo를 안 갖고 에이전트도 안 만든 정상 상태
        (Err(_), Err(_)) => 0,
        // 한쪽만 존재 = 에이전트가 지웠거나 만들었다
        (Ok(_), Err(_)) | (Err(_), Ok(_)) => 1,
        (Ok(s), Ok(d)) => {
            // 파일 ↔ 디렉터리 ↔ 심링크 바꿔치기. read()는 심링크를 따라가 원본과
            // 같은 내용을 읽을 수 있으므로 **타입을 먼저** 본다
            if s.file_type() != d.file_type() {
                return 1;
            }
            if s.is_dir() {
                let mut names = std::collections::BTreeSet::new();
                for p in [src, dst] {
                    if let Ok(rd) = std::fs::read_dir(p) {
                        names.extend(rd.flatten().map(|e| e.file_name()));
                    }
                }
                return names.iter().map(|n| diff_count(&src.join(n), &dst.join(n))).sum();
            }
            match (std::fs::read(src), std::fs::read(dst)) {
                (Ok(a), Ok(b)) if a == b => 0,
                _ => 1,
            }
        }
    }
}
```

- [ ] **Step 4: 단위 테스트 통과를 확인한다**

```bash
cargo test --lib eval::unit_tests::protected_edit_counter 2>&1 | tail -8
```
Expected: `test result: ok. 1 passed`

- [ ] **Step 5: `judge`에 배선하고 `RunRecord`에 싣는다**

`src/eval/report.rs`, `RunRecord`에:

```rust
    /// `sync_protected` 실행 **전에** 센 protected 경로 변경 항목 수 (M15 H7).
    /// 하네스가 전부 되돌리므로 판정에는 영향이 없다 — 리워드 해킹의 기계 발자국
    pub protected_edits: usize,
```

`src/eval/mod.rs`의 `judge` 첫 두 문장:

```rust
    let all_protected = with_implicit_protected(&t.spec.protected);
    // M15 H7: sync_protected가 되돌리기 **전에** 센다. 순서가 계약이다
    let protected_edits = count_protected_edits(&t.fixture, &sb.root, &all_protected);
    sb.sync_protected(&t.fixture, &all_protected)?;
```

`RunRecord` 생성부에 `protected_edits,` 추가.

- [ ] **Step 6: end-to-end 테스트를 쓴다**

`#[cfg(unix)] mod tests`의 `pass_flow_syncs_protected_before_check`는 이미 protected 수정 + 추가 시나리오를 돌린다. 그 테스트 끝에 단언 한 줄을 더한다:

```rust
        // M15 H7: 되돌리기 전에 센 변경 발자국 — data/expected.txt 수정 1 +
        // data/extra.txt 추가 1 = 2. 판정(pass_rate)에는 영향이 없다
        assert_eq!(t.runs[0].protected_edits, 2, "리워드 해킹 발자국이 기록돼야 한다");
```

- [ ] **Step 7: 통과 + 반증 확인**

```bash
cargo test --lib eval 2>&1 | tail -10
```
Expected: 전건 통과.

반증: `count_protected_edits` 호출을 `sync_protected` **뒤로** 옮기면 위 단언이 0으로 실패해야 한다. 한 번 확인하고 되돌릴 것.

- [ ] **Step 8: 게이트 + 커밋**

```bash
cargo test && cargo clippy --all-targets -- -D warnings
cargo run -- eval tasks --verify 2>&1 | tail -3
git add src/eval/mod.rs src/eval/report.rs
git commit -m "feat(eval): protected 수정 시도 카운터 — 리워드 해킹 기계 발자국 (H7)"
```

---

### Task 8: `procure.toml` 로더와 오라클 동결 (H11)

**Files:**
- Create: `src/eval/procure.rs`
- Modify: `src/eval/mod.rs`(`pub mod procure;`, `TaskReport::from_runs` 호출), `src/eval/report.rs`(`TaskReport`)
- Test: `src/eval/procure.rs` 내 `mod tests`, `src/eval/mod.rs` 내 `#[cfg(unix)] mod tests`

**Interfaces:**
- Consumes: 없음
- Produces:
  - `procure::ProcureSpec { repo, issue_url, fix_sha, parent_sha, oracle_files }` — T17 조달 스크립트가 **같은 파일을 읽는다**(TOML 형식이 계약)
  - `TaskReport.procure: Option<ProcureSpec>` — T15의 `exp_metrics.py`가 `report.json`에서 오라클 목록을 읽는다

**Consumers:** ① T15 `exp_metrics.py`(`tasks[].procure.oracle_files`) ② T17 조달 스크립트(입력 형식) ③ T23 사전등록 항목 4·10(표본 동결·자증). **`load_tasks`는 무변경**(4R 확인: 별도 파일이라 `deny_unknown_fields`에 안 걸린다). **`Sandbox::create`가 `fixture/`만 복사하므로 샌드박스에도 안 실린다** — 모델이 오라클을 읽을 수 없다.

⚠ **`task.toml`에 쓰면 안 된다** — `TaskSpec`은 `deny_unknown_fields`라 파싱이 죽는다.

- [ ] **Step 1: 로더 테스트를 쓴다**

`src/eval/procure.rs` (신규 파일):

```rust
//! 실레포 과제의 조달·오라클 메타데이터 (M15 H11).
//!
//! `task.toml`이 아니라 `<task_dir>/procure.toml`에 사는 이유는 둘이다:
//! ① `TaskSpec`이 `deny_unknown_fields`라 키를 더하면 파싱이 죽는다
//! ② `Sandbox::create`가 `fixture/`만 복사하므로 이 파일은 **샌드박스에 안 실린다**
//!    — 모델이 정답 커밋과 오라클 파일 목록을 읽을 수 없다.
//!
//! `load_tasks`는 이 파일을 모른다(무변경). 읽는 곳은 `run_eval`의 리포트
//! 조립 지점 하나이며, 조달 스크립트(`scripts/procure_real.sh`)가 같은 TOML을
//! 입력으로 쓴다 — **형식이 두 소비자의 계약이다.**

use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

/// `<task_dir>/procure.toml`. 미지 키는 오타로 간주해 거부 — task.toml과 동일 정책
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProcureSpec {
    /// pristine 클론 디렉터리 이름 (예: "ripgrep")
    pub repo: String,
    /// 원 이슈 URL — 사전등록 항목 4(표본 동결)의 좌표
    pub issue_url: String,
    /// 이 이슈를 고친 커밋
    pub fix_sha: String,
    /// 픽스처의 출처 = fix_sha의 부모. 조달은 이 트리를 뽑는다
    pub parent_sha: String,
    /// 오라클 = 정답 커밋의 **비테스트 소스** 파일 (§5-4 제약 2).
    /// CHANGELOG·문서를 배제한 **명시 목록**이다 — 레포마다 관례가 달라
    /// 자동 규칙으로는 못 좁힌다. 리포트에 동결돼 사후 변경이 막힌다
    #[serde(default)]
    pub oracle_files: Vec<String>,
}

/// `<task_dir>/procure.toml`을 읽는다. 파일이 없으면 `Ok(None)` — 기존 두 트리의
/// 15개 과제가 그 상태다. **파싱 실패는 에러다**: 조용히 무시하면 오라클 목록이
/// 빈 채로 배치가 돌고 항해/수선 지표가 전부 "해당 없음"이 되는 fail-open이 된다
pub fn load(task_dir: &Path) -> anyhow::Result<Option<ProcureSpec>> {
    let path = task_dir.join("procure.toml");
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("procure.toml 읽기 실패: {}", path.display()))?;
    let spec: ProcureSpec = toml::from_str(&text)
        .with_context(|| format!("procure.toml 파싱 실패: {}", path.display()))?;
    Ok(Some(spec))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
repo = "ripgrep"
issue_url = "https://github.com/BurntSushi/ripgrep/issues/1234"
fix_sha = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
parent_sha = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
oracle_files = ["crates/core/flags/hiargs.rs"]
"#;

    #[test]
    fn missing_file_is_none_not_an_error() {
        // 기존 두 트리의 15개 과제가 이 경로를 탄다
        let dir = tempfile::tempdir().unwrap();
        assert!(load(dir.path()).unwrap().is_none());
    }

    #[test]
    fn reads_all_fields() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("procure.toml"), SAMPLE).unwrap();
        let s = load(dir.path()).unwrap().unwrap();
        assert_eq!(s.repo, "ripgrep");
        assert_eq!(s.fix_sha.len(), 40);
        assert_eq!(s.oracle_files, vec!["crates/core/flags/hiargs.rs".to_string()]);
    }

    #[test]
    fn oracle_files_defaults_to_empty() {
        let dir = tempfile::tempdir().unwrap();
        let no_oracle = SAMPLE.lines().filter(|l| !l.starts_with("oracle_files")).collect::<Vec<_>>().join("\n");
        std::fs::write(dir.path().join("procure.toml"), no_oracle).unwrap();
        assert!(load(dir.path()).unwrap().unwrap().oracle_files.is_empty());
    }

    #[test]
    fn unknown_key_is_rejected() {
        // 오타가 조용히 무시되면 오라클이 빈 채로 배치가 돈다 (fail-open)
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("procure.toml"),
            format!("{SAMPLE}oracle_file = [\"typo.rs\"]\n"),
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(err.to_string().contains("procure.toml"), "{err:#}");
    }

    #[test]
    fn malformed_toml_is_an_error_not_a_silent_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("procure.toml"), "repo = \n").unwrap();
        assert!(load(dir.path()).is_err());
    }
}
```

- [ ] **Step 2: 모듈을 등록하고 실패를 확인한다**

`src/eval/mod.rs` 상단 모듈 선언에 추가:

```rust
pub mod integrity;
pub mod procure;
pub mod report;
```

```bash
cargo test --lib eval::procure 2>&1 | tail -10
```
Expected: `test result: ok. 5 passed` (로더 자체는 이 시점에 이미 완성이다)

- [ ] **Step 3: `TaskReport`에 실어 report.json까지 보내는 테스트를 쓴다**

`src/eval/mod.rs`의 `#[cfg(unix)] mod tests`에:

```rust
    /// M15 H11 — 오라클 목록이 배치 산출물에 **동결**된다. exp_metrics.py가
    /// 이 경로로 읽으므로(§5-4 입력 계약) 사후 변경이 구조적으로 막힌다
    #[tokio::test]
    async fn procure_metadata_reaches_the_report() {
        let tasks = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            tasks.path(),
            "real",
            "prompt = \"p\"\ncheck = \"true\"\nprotected = [\"keep.txt\"]\n",
            &[("keep.txt", "x")],
        );
        std::fs::write(
            tasks.path().join("real/procure.toml"),
            "repo = \"fd\"\nissue_url = \"https://example.invalid/1\"\n\
             fix_sha = \"a\"\nparent_sha = \"b\"\noracle_files = [\"src/walk.rs\"]\n",
        )
        .unwrap();
        let script = Scripted::new(vec![ok(&finish("done"))]);
        let o = opts(tasks.path().to_path_buf());
        let run = run_eval(&script, &Config::default(), "m", &o, proj.path()).await.unwrap();

        let p = run.report.tasks[0].procure.as_ref().expect("procure.toml이 리포트에 실려야 한다");
        assert_eq!(p.repo, "fd");
        assert_eq!(p.oracle_files, vec!["src/walk.rs".to_string()]);
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&run.report_path).unwrap()).unwrap();
        assert_eq!(json["tasks"][0]["procure"]["oracle_files"][0], "src/walk.rs");
    }

    /// 짝 — procure.toml이 없는 과제(기존 두 트리)는 null이고 배치가 안 죽는다
    #[tokio::test]
    async fn tasks_without_procure_toml_report_null() {
        let tasks = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write_task(
            tasks.path(),
            "plain",
            "prompt = \"p\"\ncheck = \"true\"\nprotected = [\"keep.txt\"]\n",
            &[("keep.txt", "x")],
        );
        let script = Scripted::new(vec![ok(&finish("done"))]);
        let o = opts(tasks.path().to_path_buf());
        let run = run_eval(&script, &Config::default(), "m", &o, proj.path()).await.unwrap();
        assert!(run.report.tasks[0].procure.is_none());
    }
```

- [ ] **Step 4: `TaskReport`를 확장한다**

`src/eval/report.rs`:

```rust
use crate::eval::procure::ProcureSpec;
```

`TaskReport`에 필드 추가 (`runs` 앞):

```rust
    /// `<task_dir>/procure.toml`의 조달·오라클 좌표 (M15 H11). 실레포 트랙이
    /// 아닌 과제는 `None`. **배치 산출물에 동결**되므로 오라클 목록의 사후
    /// 변경이 막힌다 — §5-4가 요구하는 입력 계약
    pub procure: Option<ProcureSpec>,
    pub runs: Vec<RunRecord>,
```

`from_runs` 시그니처와 본문:

```rust
    pub fn from_runs(name: String, runs: Vec<RunRecord>, procure: Option<ProcureSpec>) -> TaskReport {
        let n = runs.len().max(1) as f64;
        TaskReport {
            // …기존 필드 그대로…
            name,
            procure,
            runs,
        }
    }
```

- [ ] **Step 5: `run_eval`에서 읽어 넘긴다**

`src/eval/mod.rs:114`:

```rust
            // M15 H11: task_dir = fixture의 부모. H8이 `<task_dir>/fixture`를
            // 실체화하므로 이 관계가 두 트리 모두에서 참이다(H3가 불요한 이유)
            let task_dir = t.fixture.parent().expect("fixture는 과제 디렉터리 바로 아래");
            let procure = procure::load(task_dir)?;
            task_reports.push(TaskReport::from_runs(t.name.clone(), runs, procure));
```

`report.rs`의 기존 단위 테스트에서 `from_runs` 호출부에 `None`을 추가한다.
**기준 커밋에서 호출 지점은 정확히 11개다** — `report.rs`의 테스트 **10개**
(`:190`, `:191`, `:205`, `:229`, `:259`, `:276`, `:284`, `:291`, `:292`, **`:303`**)와
`mod.rs:114` 1개:

```bash
grep -c "from_runs(" src/eval/report.rs src/eval/mod.rs
# report.rs:11 (정의 1 + 호출 10), mod.rs:1
```

⚠ **`:303`은 헬퍼 `sample_report()` 안에 있다.** 초판은 이 자리를 놓치고 "정확히 10개"라고 적었는데, **`grep | head`가 10줄에서 자른 것을 세었기 때문이다**(1R 사실 I1). 프로젝트 메모리 *"컨트롤러 분석 규율 — 안 센 숫자"*의 사례다. **`head` 없이, `grep -c`로 셀 것.**

찾은 **모든 호출**에 세 번째 인자를 넣을 것. 하나라도 빠지면 컴파일이 실패하므로 침묵 누락은 불가능하다 — 다만 그것이 세지 않아도 된다는 뜻은 아니다.

- [ ] **Step 6: 통과 + 게이트 + 커밋**

```bash
cargo test && cargo clippy --all-targets -- -D warnings
cargo run -- eval tasks --verify 2>&1 | tail -3
cargo run -- eval tasks-large --verify 2>&1 | tail -3
git add src/eval/procure.rs src/eval/mod.rs src/eval/report.rs
git commit -m "feat(eval): procure.toml 로더 + 오라클 목록을 report.json에 동결 (H11)"
```

---

### Task 9: 픽스처가 제공한 `.cargo/config.toml`은 복원된다 (H18)

**Files:**
- Test: `src/eval/sandbox.rs` 내 `mod tests`
- Modify: 없음 (기존 동작을 고정하는 테스트다)

**Interfaces:**
- Consumes: T2·T3의 `copy_tree`, 기존 `sync_protected`
- Produces: 없음

**Consumers:** **0건.** 순수 회귀 테스트다.

**왜 필요한가:** `.cargo`는 암묵 protected다(`with_implicit_protected`). 지금까지 모든 픽스처는 `.cargo`가 **없었고**, 그 경우 `sync_protected`는 샌드박스 쪽을 지우기만 한다. 그런데 fd·ripgrep·zoxide는 `.cargo/config.toml`을 **추적한다**(3R 실측, just만 없음). **픽스처가 실제 `.cargo`를 갖는 경로는 이 프로젝트 역사상 처음이라 무테스트 경로**이고, 정책이 "삭제"가 아니라 "원본 복원"이어야 한다(§3-2 규약 5).

⚠ 부수 사실: 4레포의 rustflags는 전부 Windows/musl 한정이라 darwin에서 불활성이고, `target-dir`/`build.target`을 거는 레포는 0개다 — **레포가 `.cargo/config.toml`을 갖는 것 자체는 무해하다.**

- [ ] **Step 1: 양방향 테스트를 쓴다**

`src/eval/sandbox.rs`의 `mod tests`에 추가:

```rust
    #[test]
    fn fixture_provided_dot_cargo_is_restored_not_deleted() {
        // M15 H18 — fd·ripgrep·zoxide는 .cargo/config.toml을 추적한다.
        // 픽스처가 .cargo를 갖는 경로는 이 프로젝트 역사상 처음이다(§3-2 규약 5).
        // 암묵 protected 정책은 "삭제"가 아니라 "원본 복원"이어야 한다 —
        // 삭제하면 실레포 픽스처가 자기 빌드 설정을 잃은 채 check를 받는다
        use crate::eval::with_implicit_protected;
        let fx = fixture_with(&[(".cargo/config.toml", "[build]\nrustflags = []\n"), ("src/lib.rs", "x")]);
        let sb = Sandbox::create(fx.path()).unwrap();
        // 리워드 해킹 시뮬레이션: 가짜 러너 설정 + 추가 파일
        std::fs::write(
            sb.root.join(".cargo/config.toml"),
            "[target.'cfg(all())']\nrunner = 'true'\n",
        )
        .unwrap();
        std::fs::write(sb.root.join(".cargo/extra.toml"), "sneak").unwrap();

        sb.sync_protected(fx.path(), &with_implicit_protected(&["src".to_string()])).unwrap();

        assert_eq!(
            std::fs::read_to_string(sb.root.join(".cargo/config.toml")).unwrap(),
            "[build]\nrustflags = []\n",
            "픽스처 원본으로 복원 (M15 H18)"
        );
        assert!(!sb.root.join(".cargo/extra.toml").exists(), "에이전트 추가분은 삭제");
        sb.cleanup();
    }

    #[test]
    fn agent_created_dot_cargo_is_deleted_when_the_fixture_has_none() {
        // 짝 테스트 — 기존 15개 과제가 타는 경로. 이것 없이 위 테스트만 있으면
        // "항상 복원"이라는 반대 방향의 회귀를 못 잡는다 (M5 §4.1 벡터)
        use crate::eval::with_implicit_protected;
        let fx = fixture_with(&[("src/lib.rs", "x")]);
        let sb = Sandbox::create(fx.path()).unwrap();
        std::fs::create_dir_all(sb.root.join(".cargo")).unwrap();
        std::fs::write(sb.root.join(".cargo/config.toml"), "runner = 'true'\n").unwrap();

        sb.sync_protected(fx.path(), &with_implicit_protected(&["src".to_string()])).unwrap();

        assert!(!sb.root.join(".cargo").exists(), "픽스처에 없으면 통째로 삭제");
        sb.cleanup();
    }
```

- [ ] **Step 2: 돌린다**

```bash
cargo test --lib eval::sandbox 2>&1 | tail -15
```
Expected: 둘 다 PASS (기존 동작이 이미 옳다).

**FAIL이면 그 자체가 발견이다** — `sync_protected`의 `src.is_dir()` 분기를 확인하고, 스펙 §3-2 규약 5의 전제가 틀린 것이므로 **코드를 고치기 전에 사용자에게 보고할 것.**

- [ ] **Step 3: 커밋**

```bash
cargo test && cargo clippy --all-targets -- -D warnings
git add src/eval/sandbox.rs
git commit -m "test(eval): 픽스처 제공 .cargo/config.toml 복원 + 미제공 시 삭제 (H18)"
```

---

### Task 10: `Usage.prompt_tokens` 파싱 (H12)

**Files:**
- Modify: `src/llm/types.rs:57-62` (`Usage`), `src/llm/types.rs`의 `impl ChatResponse`
- Test: `src/llm/types.rs` 내 `mod tests`

**Interfaces:**
- Consumes: 없음
- Produces: `ChatResponse::prompt_tokens(&self) -> Option<u32>` — T11이 유일한 소비자

**Consumers:** **신규 필드라 기존 소비자 0건.** 4R 확인: `completion_tokens()`의 프로덕션 소비자도 0개이고, 에이전트 경로는 `stream: false`라 `usage`가 실제로 실린다 — **위험 0.** 스트리밍 경로(`/chat`)는 `StreamChunk`를 쓰므로 이 타입을 안 탄다.

- [ ] **Step 1: 테스트를 쓴다**

`src/llm/types.rs`의 `mod tests`에 추가:

```rust
    #[test]
    fn usage_parses_prompt_tokens() {
        // M15 H12 — 축 C의 기준값. estimate_tokens(len/4)가 실측과 대조된 적이
        // 없다는 것이 §5-1이 확인한 착수점이다
        let json = r#"{"choices":[{"message":{"role":"assistant","content":"hi"},
                        "finish_reason":"stop"}],
                       "usage":{"prompt_tokens":1234,"completion_tokens":56}}"#;
        let r: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.prompt_tokens(), Some(1234));
        assert_eq!(r.completion_tokens(), Some(56));
    }

    #[test]
    fn missing_usage_is_none_not_zero() {
        // 0으로 떨어지면 추정기 오차 회귀가 원점을 지나는 거짓 관측을 얻는다
        let json = r#"{"choices":[{"message":{"role":"assistant","content":"hi"},
                        "finish_reason":"stop"}]}"#;
        let r: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.prompt_tokens(), None);
    }

    #[test]
    fn usage_with_only_completion_tokens_still_parses() {
        // 서버가 prompt_tokens를 안 줄 수도 있다 — 그래도 파싱이 죽으면 안 된다
        let json = r#"{"choices":[{"message":{"role":"assistant","content":"hi"},
                        "finish_reason":"stop"}],"usage":{"completion_tokens":7}}"#;
        let r: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(r.prompt_tokens(), None);
        assert_eq!(r.completion_tokens(), Some(7));
    }
```

- [ ] **Step 2: 실패를 확인한다**

```bash
cargo test --lib llm::types 2>&1 | tail -10
```
Expected: 컴파일 에러 — `no method named 'prompt_tokens'`

- [ ] **Step 3: 필드와 접근자를 추가한다**

`src/llm/types.rs`의 `Usage`:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub completion_tokens: Option<u32>,
    /// 서버가 센 **입력** 토큰 — 채팅 템플릿이 렌더한 전체(역할 태그·특수 토큰·BOS)를
    /// 세므로 본문만 세는 `estimate_tokens`와 정의가 다르다. 그 차이를 절편/기울기로
    /// 분해하는 것이 §5-3이다 (M15 H12)
    #[serde(default)]
    pub prompt_tokens: Option<u32>,
}
```

`impl ChatResponse`의 `completion_tokens()` 아래:

```rust
    /// 입력 토큰 소비량. `estimate_tokens`(본문 len/4) 추정기의 **유일한 기준값**이며
    /// 서버가 안 주면 `None`이다 — 0으로 대체하면 §5-3 회귀가 원점을 지나는
    /// 거짓 관측을 얻는다 (M15 H12·§5-2 ①)
    pub fn prompt_tokens(&self) -> Option<u32> {
        self.usage.as_ref().and_then(|u| u.prompt_tokens)
    }
```

- [ ] **Step 4: 통과 + 커밋**

```bash
cargo test --lib llm::types 2>&1 | tail -8
cargo test && cargo clippy --all-targets -- -D warnings
git add src/llm/types.rs
git commit -m "feat(llm): Usage.prompt_tokens 파싱 — 추정기 검증의 기준값 (H12)"
```

---

### Task 11: 턴별 `usage` 기록과 `pack()` 축약 기록 (H13)

**Files:**
- Modify: `src/session.rs:129-158` (`total_tokens` 공개 + `pack` 분리), `src/agent/mod.rs`(resp 루프 직후)
- Test: `src/session.rs` 내 `mod tests`, `src/agent/mod.rs` 내 `mod tests`

**Interfaces:**
- Consumes: T10의 `ChatResponse::prompt_tokens()`
- Produces: 트랜스크립트 이벤트 두 종 — T14의 `exp_metrics.py`가 읽는다
  - `{"kind":"usage","content":"{…JSON…}"}` — 키: `prompt_tokens`, `completion_tokens`, `estimate_tokens`, `messages`, `budget`, `inline_system`, `overflow_shrinks`
  - `{"kind":"pack","content":"{…JSON…}"}` — 키: `budget`, `before`, `after`, `elided`, `dropped`

**Consumers:** ① T14 `exp_metrics.py`(신규 컬럼) ② T16 세션 모드(스모크의 `r_obs`) ③ T23 사전등록 §6-4-19⑤·⑥. **모델 대면 소비자 0건** — 히스토리에 안 들어가고 상태선에도 안 나온다(§5-6).

⚠ **측정 지점의 계약**: 스펙 §5-2 ②는 *"응답을 만들어낸 마지막 반복의, 요청 직렬화 직전 히스토리"*를 요구한다. 세 사실이 이것을 만족시킨다:
1. 오버플로 재시도는 `let resp = loop { … }` **안**에서 축소 예산으로 다시 `pack`한다(`:283`) — 그러므로 **루프 종료 후** 읽어야 마지막 반복의 상태다
2. `chat_with_fallback`은 `inline_system`을 **켜기만 하고 끄지 못한다**(`:744-749`) — 호출 후의 값이 곧 성공한 요청이 쓴 값이다
3. 루프 종료와 기록 사이에 히스토리를 바꾸는 문장이 없다

- [ ] **Step 1: `pack()` 기록 테스트를 쓴다**

`src/session.rs`의 `mod tests`에 추가:

```rust
    #[test]
    fn pack_records_what_it_actually_elided_and_dropped() {
        // §5-1: pack()은 매 턴 무조건 호출되고 예산 미만이면 no-op인데
        // **실제 축약 턴이 어디에도 기록되지 않았다** (M15 H13·§5-2 ③)
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.jsonl");
        let mut s = Session::new(
            vec![
                ChatMessage::system("sys"),
                ChatMessage::user("u1"),
                ChatMessage::assistant("a1"),
                tool_msg(&"X".repeat(4000)),
                ChatMessage::assistant("a2"),
                ChatMessage::user("last"),
            ],
            Transcript::create_at(&path).unwrap(),
        );
        s.pack(100);
        let text = std::fs::read_to_string(&path).unwrap();
        let rec: serde_json::Value = text
            .lines()
            .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
            .find(|v| v["kind"] == "pack")
            .expect("축약이 일어났으면 pack 이벤트가 있어야 한다");
        let body: serde_json::Value = serde_json::from_str(rec["content"].as_str().unwrap()).unwrap();
        assert_eq!(body["budget"], 100);
        assert!(body["elided"].as_u64().unwrap() >= 1, "툴 결과 생략이 먼저: {body}");
        assert!(body["after"].as_u64().unwrap() < body["before"].as_u64().unwrap());
    }

    #[test]
    fn pack_noop_records_nothing() {
        // 예산 미만이면 no-op — 이벤트를 남기면 "pack 발동 수" 컬럼이
        // 매 턴 1이 되어 지표가 무의미해진다
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.jsonl");
        let mut s = Session::new(
            vec![ChatMessage::system("sys"), ChatMessage::user("u"), ChatMessage::assistant("a")],
            Transcript::create_at(&path).unwrap(),
        );
        s.pack(100_000);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("\"kind\":\"pack\""), "no-op은 기록하지 않는다: {text}");
    }
```

⚠ 위에서 쓰는 `ChatMessage::system/user/assistant` 생성자와 `tool_msg` 헬퍼의 실제 이름은 `src/session.rs`의 기존 `mod tests`(`:349-357`)를 그대로 따를 것. `sess()` 헬퍼는 트랜스크립트가 `disabled()`라 이 테스트에 못 쓴다 — **위처럼 `Transcript::create_at`으로 직접 만든다.**

- [ ] **Step 2: 실패를 확인한다**

```bash
cargo test --lib session::tests::pack_records 2>&1 | tail -10
```
Expected: FAIL — `축약이 일어났으면 pack 이벤트가 있어야 한다`

- [ ] **Step 3: `pack()`을 기록하도록 쪼갠다**

`src/session.rs`. `total_tokens`를 공개하고(T11 Step 6이 agent에서 쓴다) `pack`을 래퍼/내부로 나눈다:

```rust
    /// 저장 히스토리의 추정 토큰 합. `estimate_tokens`(본문 len/4) 기준이므로
    /// 서버의 `prompt_tokens`와 정의가 다르다 — 그 차이가 §5-3의 측정 대상이다
    pub fn total_tokens(&self) -> usize {
        self.messages.iter().map(|m| estimate_tokens(&m.content)).sum()
    }

    /// §6 절삭: ① 오래된 툴 결과 본문 생략 → ② 오래된 user+assistant 쌍 원자 제거.
    /// 시스템 프롬프트(0)와 마지막 메시지(현재 요청/결과)는 보존.
    /// 저장 히스토리 자체를 변형한다 — 원문은 트랜스크립트에 이미 있음.
    ///
    /// M15 H13: **실제로 축약이 일어난 경우에만** 트랜스크립트에 기록한다.
    /// no-op까지 기록하면 "pack 발동 수"가 매 턴 1이 되어 지표가 죽는다
    pub fn pack(&mut self, input_budget_tokens: usize) {
        let before = self.total_tokens();
        let (elided, dropped) = self.pack_inner(input_budget_tokens);
        if elided == 0 && dropped == 0 {
            return;
        }
        let after = self.total_tokens();
        self.transcript.record(
            "pack",
            &serde_json::json!({
                "budget": input_budget_tokens,
                "before": before,
                "after": after,
                "elided": elided,
                "dropped": dropped,
            })
            .to_string(),
        );
    }

    /// (생략한 툴 결과 수, 제거한 메시지 수)
    fn pack_inner(&mut self, input_budget_tokens: usize) -> (usize, usize) {
        let (mut elided, mut dropped) = (0usize, 0usize);
        let last = self.messages.len().saturating_sub(1);
        for i in 1..last {
            if self.total_tokens() <= input_budget_tokens {
                return (elided, dropped);
            }
            let m = &mut self.messages[i];
            if m.role == "user" && m.content.starts_with("<tool_result") && !m.content.contains(ELIDED) {
                let first_line = m.content.lines().next().unwrap_or("<tool_result>").to_string();
                // 본문만 생략하고 `</tool_result>` 뒤에 병합된 내용(push_tool_result의 교정
                // 노트, push_user_request의 후속 요청)은 보존한다 — 없으면 빈 문자열
                let suffix = m.content.split_once("</tool_result>").map(|(_, s)| s).unwrap_or("");
                m.content = format!("{first_line}\n{ELIDED}\n</tool_result>{suffix}");
                elided += 1;
            }
        }
        while self.total_tokens() > input_budget_tokens && self.messages.len() > 3 {
            if self.messages[1].role == "user" && self.messages[2].role == "assistant" {
                self.messages.drain(1..=2);
                dropped += 2;
            } else {
                self.messages.remove(1); // 교대가 어긋난 히스토리 — 하나씩 걷어내고 병합으로 복구
                dropped += 1;
            }
            merge_adjacent_same_role(&mut self.messages);
        }
        (elided, dropped)
    }
```

⚠ **동작은 한 글자도 안 바뀐다** — 카운터만 얹었다. 이름이 `pack_`으로 시작하는 기존 테스트 **3개**(`session.rs:364`·`:371`·`:390`)와 `.pack(`을 호출하는 테스트 **6개**가 그것을 지킨다(1R 사실 m2: 초판의 "4개"는 어느 기준으로도 틀렸다).

- [ ] **Step 4: session 테스트 통과를 확인한다**

```bash
cargo test --lib session 2>&1 | tail -10
```
Expected: `test result: ok.` — 기존 pack 테스트 4개 + 신규 2개

- [ ] **Step 5: agent 쪽 `usage` 기록 테스트를 쓴다**

`src/agent/mod.rs`의 `mod tests`에 추가. 기존 테스트가 쓰는 스크립트 클라이언트/헬퍼 이름을 그대로 따를 것:

기존 헬퍼(`Scripted`·`ok`·`finish`·`make_guided_agent`·`run_quiet`)에 **`usage`를 실은 응답 생성자 하나**를 더한다. ⚠ 기존 `ok()`는 `usage: None`이라 그대로 못 쓴다:

```rust
    /// M15 H13 — usage를 실은 응답. 기존 `ok()`는 `usage: None`이다
    fn ok_with_usage(text: &str, prompt: u32, completion: u32) -> Result<ChatResponse, LlmError> {
        Ok(ChatResponse {
            choices: vec![Choice {
                message: ResponseMessage {
                    role: "assistant".into(),
                    content: Some(text.into()),
                    reasoning_content: None,
                },
                finish_reason: Some("stop".into()),
            }],
            usage: Some(crate::llm::types::Usage {
                completion_tokens: Some(completion),
                prompt_tokens: Some(prompt),
            }),
        })
    }
```

```rust
    /// M15 H13·§5-2 ② — 턴마다 서버 실측과 추정치를 나란히 남긴다.
    /// 측정 지점은 **응답을 만들어낸 마지막 반복의 직렬화 직전 히스토리**다.
    /// ⚠ `new_session`은 `Transcript::disabled()`라 이 테스트에 못 쓴다 —
    /// 실 파일이어야 단언이 공허하지 않다(M14가 같은 함정을 겪었다)
    #[tokio::test]
    async fn each_turn_records_usage_next_to_the_estimate() {
        let dir = tempfile::tempdir().unwrap();
        let tpath = dir.path().join("t.jsonl");
        let script = Scripted::new(vec![ok_with_usage(&finish("done"), 999, 11)]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session =
            Session::new(agent.initial_history(), Transcript::create_at(&tpath).unwrap());
        let out = run_quiet(&mut agent, &mut session, "요청").await.unwrap();
        assert!(matches!(out, AgentOutcome::Finished(_)));

        let text = std::fs::read_to_string(&tpath).unwrap();
        let rec: serde_json::Value = text
            .lines()
            .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
            .find(|v| v["kind"] == "usage")
            .expect("턴마다 usage 이벤트가 있어야 한다");
        let body: serde_json::Value =
            serde_json::from_str(rec["content"].as_str().unwrap()).unwrap();
        assert_eq!(body["prompt_tokens"], 999);
        assert_eq!(body["completion_tokens"], 11);
        assert!(body["estimate_tokens"].as_u64().unwrap() > 0, "추정치가 나란히 있어야 한다");
        assert!(body["budget"].as_u64().unwrap() > 0);
        assert_eq!(body["inline_system"], false, "§5-3 층화 키");
        assert_eq!(body["overflow_shrinks"], 0);
    }

    /// 짝 — 서버가 usage를 안 주면 `null`이어야 한다. 0으로 떨어지면 §5-3 회귀가
    /// 원점을 지나는 거짓 관측을 얻는다(T10의 `missing_usage_is_none_not_zero`와 같은 규율)
    #[tokio::test]
    async fn missing_usage_records_null_not_zero() {
        let dir = tempfile::tempdir().unwrap();
        let tpath = dir.path().join("t.jsonl");
        let script = Scripted::new(vec![ok(&finish("done"))]); // usage: None
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session =
            Session::new(agent.initial_history(), Transcript::create_at(&tpath).unwrap());
        run_quiet(&mut agent, &mut session, "요청").await.unwrap();

        let text = std::fs::read_to_string(&tpath).unwrap();
        let rec: serde_json::Value = text
            .lines()
            .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
            .find(|v| v["kind"] == "usage")
            .expect("usage 이벤트는 서버 응답과 무관하게 남아야 한다");
        let body: serde_json::Value =
            serde_json::from_str(rec["content"].as_str().unwrap()).unwrap();
        assert!(body["prompt_tokens"].is_null(), "0이 아니라 null: {body}");
        assert!(body["estimate_tokens"].as_u64().unwrap() > 0, "추정치는 서버와 무관하게 있다");
    }
```

⚠ `Choice`·`ResponseMessage`·`Usage`는 `crate::llm::types`에서 온다 — 기존 `ok()`가 이미 앞의 둘을 쓰므로 `use`는 대체로 갖춰져 있다. `Usage`만 경로가 필요할 수 있다.

- [ ] **Step 6: `run()`에 기록을 넣는다**

`src/agent/mod.rs`, `let resp = loop { … };` **바로 다음**, `finish_reason` 기록 앞:

```rust
            // M15 H13·§5-2 ②: 축 C의 원자료. 측정 지점 = **응답을 만들어낸
            // 마지막 반복의 직렬화 직전 히스토리**. 세 사실이 이것을 보장한다:
            // ① 오버플로 재시도는 위 loop 안에서 축소 예산으로 다시 pack하므로
            //    루프 종료 후 읽어야 마지막 반복의 상태다
            // ② chat_with_fallback은 inline_system을 켜기만 하고 끄지 못하므로
            //    (:744-749) 호출 후 값이 곧 성공한 요청이 쓴 값이다 —
            //    직렬화 메시지 집합을 바꾸므로 §5-3의 층화 키로 남긴다
            // ③ 루프 종료와 이 지점 사이에 히스토리를 바꾸는 문장이 없다
            //
            // ⚠ 기록만 한다 — 히스토리·상태선·모델 대면 텍스트 어디에도 안 나간다(§5-6)
            let est = session.total_tokens();
            let n_msgs = session.messages().len();
            session.record_extra(
                "usage",
                &serde_json::json!({
                    "prompt_tokens": resp.prompt_tokens(),
                    "completion_tokens": resp.completion_tokens(),
                    "estimate_tokens": est,
                    "messages": n_msgs,
                    "budget": self.input_budget(),
                    "inline_system": self.inline_system,
                    "overflow_shrinks": overflow_shrinks,
                })
                .to_string(),
            );
```

- [ ] **Step 7: 통과 + 반증 확인**

```bash
cargo test --lib agent 2>&1 | tail -10
```

반증: `record_extra("usage", …)` 블록을 주석 처리하면 위 테스트가 `턴마다 usage 이벤트가 있어야 한다`로 실패해야 한다. 확인 후 되돌린다.

- [ ] **Step 8: 게이트 + 커밋**

```bash
cargo test && cargo clippy --all-targets -- -D warnings
git add src/session.rs src/agent/mod.rs
git commit -m "feat(agent): 턴별 usage 기록 + pack() 축약 기록 — 축 C 원자료 (H13)"
```

---

### Task 12: 오버플로 최종 포기 경로를 기록한다 (H14)

**Files:**
- Modify: `src/agent/mod.rs:285-290`
- Test: `src/agent/mod.rs` 내 `mod tests`

**Interfaces:**
- Consumes: 없음
- Produces: `{"kind":"notice","content":"(컨텍스트 초과 — …)"}` — T14가 `overflow_giveup` 컬럼으로 센다

**Consumers:** ① T14 `exp_metrics.py`의 `notice` 처리 ② T23 사전등록 §6-4-19. **모델 대면 소비자 0건** — `on_event`만 타던 것을 트랜스크립트에도 남기는 것뿐이다.

⚠ **축소 재시도 경로(`:277-283`)는 M14가 이미 `notice_recorded!`로 기록한다.** 남은 공백은 **최종 포기 경로 하나**다(§5-2 ⑤). 4R 확인: 단일 문장이고 **바로 위 arm이 같은 match에서 이미 매크로를 쓰므로 차용 검사 문제 없이 1줄**이다.

- [ ] **Step 1: 테스트를 쓴다**

`src/agent/mod.rs`의 `mod tests`에 추가:

```rust
    /// M15 H14·§5-2 ⑤ — 오버플로로 **포기**한 런이 트랜스크립트에 남아야 한다.
    /// M13이 △(계측 불가)로 분류한 2종 중 하나다. 축소 재시도(2회)는 M14가
    /// 이미 기록하므로 여기서 닫는 것은 3번째 400(최종 포기)뿐이다
    #[tokio::test]
    async fn overflow_giveup_is_recorded_in_the_transcript() {
        let dir = tempfile::tempdir().unwrap();
        let tpath = dir.path().join("t.jsonl");
        // 컨텍스트 초과 400을 3번 — 2회는 축소 재시도, 3번째가 최종 포기.
        // LlmError는 Clone을 파생하지 않는다 — vec![x; 3]이 아니라 루프로 push
        let mut v = Vec::new();
        for _ in 0..3 {
            v.push(Err(LlmError::Api {
                status: 400,
                body: "context length exceeded".into(),
            }));
        }
        let script = Scripted::new(v);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session =
            Session::new(agent.initial_history(), Transcript::create_at(&tpath).unwrap());
        let err = run_quiet(&mut agent, &mut session, "요청").await.unwrap_err();
        assert!(matches!(err, LlmError::Api { status: 400, .. }), "{err:?}");

        let text = std::fs::read_to_string(&tpath).unwrap();
        let notices: Vec<&str> = text.lines().filter(|l| l.contains("\"kind\":\"notice\"")).collect();
        assert_eq!(
            notices.iter().filter(|l| l.contains("히스토리 절삭 후 재시도")).count(),
            2,
            "축소 재시도 2회 (M14가 이미 기록): {notices:?}"
        );
        assert_eq!(
            notices.iter().filter(|l| l.contains("컨텍스트 초과 — context_tokens")).count(),
            1,
            "최종 포기도 기록돼야 한다 (M15 H14): {notices:?}"
        );
    }
```

- [ ] **Step 2: 실패를 확인한다**

```bash
cargo test --lib agent::tests::overflow_giveup 2>&1 | tail -10
```
Expected: FAIL — `최종 포기도 기록돼야 한다` (0건)

- [ ] **Step 3: 한 줄을 바꾼다**

`src/agent/mod.rs:285-290`:

```rust
                    Err(LlmError::Api { status: 400, body }) if looks_like_context_overflow(&body) => {
                        // M15 H14: on_event만 타던 것을 트랜스크립트에도 남긴다.
                        // 축소 재시도(위 arm)는 M14가 이미 notice_recorded!로 기록하는데
                        // **포기한 런만 흔적이 없어** 배치 후 구분이 불가능했다 (§5-2 ⑤)
                        notice_recorded!(
                            session,
                            on_event,
                            "(컨텍스트 초과 — context_tokens 설정과 서버 로드 설정을 확인하세요)".to_string()
                        );
                        return Err(LlmError::Api { status: 400, body });
                    }
```

- [ ] **Step 4: 통과 + 게이트 + 커밋**

```bash
cargo test --lib agent 2>&1 | tail -8
cargo test && cargo clippy --all-targets -- -D warnings
git add src/agent/mod.rs
git commit -m "feat(agent): 오버플로 최종 포기 경로를 트랜스크립트에 기록 (H14)"
```

---

### Task 13: 툴별로 분리된 접촉 파일 기록 (H10)

**Files:**
- Modify: `src/agent/mod.rs`의 `if dispatch_ok { … }` 블록(`:582-607`)
- Test: `src/agent/mod.rs` 내 `mod tests`

**Interfaces:**
- Consumes: `status_note::normalize`(pub, `status_note.rs:189`)
- Produces: `{"kind":"touch","content":"{\"tool\":…,\"path\":…}"}` — T15의 항해/수선 지표가 유일한 소비자

**Consumers:** ① T15 `exp_metrics.py`(항해/수선 지표) ② T23 사전등록 §6-4-19①. **모델 대면 소비자 0건.**

⚠ **셋을 한 집합으로 합치면 안 된다**(4R I2). §1-1의 축 근거는 M8 실패 분석의 **`"monthly.rs 미열람(grep만)"`** 구분이다. 합치면 그 구분이 사라져 축의 정의와 계측기가 어긋난다.

⚠ **항해 지표는 `read_file` 집합만으로 정의**하고 `grep`/`list_files`는 **호출 계수로만** 남긴다.

⚠ **근거는 "grep이 경로를 못 준다"가 아니다**(스펙 개정 10 정정, 1R 사실 I2). `grep.rs:53`에 `if base.is_file() { vec![base] }` 분기가 있어 **`grep`은 파일 하나를 지목할 수 있다**. `list_files`만 참이다(`walk_entries`가 `if p == base { continue }`로 시작점 자신을 버린다). **올바른 근거는 축의 정의다** — §1-1이 이 트랙의 축을 세운 것이 M8 실패 분석의 `"미열람(grep만)"`이고 그 구분은 *"스쳤는가"*와 *"열었는가"*를 **가르는 것이 목적**이므로, `grep`으로 오라클을 지목한 런을 항해 성공에서 빼는 것은 **의도된 방향**이다.

⚠ **배치 지점은 편집 전용 블록(`:601-606`)이 아니라 `if dispatch_ok`(`:582-607`) 안의 형제 분기다.**

- [ ] **Step 1: 테스트를 쓴다**

`src/agent/mod.rs`의 `mod tests`에 추가:

```rust
    /// M15 H10·§5-4 — 툴별로 **분리**해 남긴다. 합치면 §1-1 축의 근거인
    /// M8 실패 분석의 "미열람(grep만)" 구분이 사라진다
    #[tokio::test]
    async fn touched_files_are_recorded_per_tool() {
        let dir = tempfile::tempdir().unwrap();
        let tpath = dir.path().join("t.jsonl");
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "fn main() {}\n").unwrap();

        let script = Scripted::new(vec![
            ok(&turn("read_file", serde_json::json!({"path": "src/lib.rs"}))),
            ok(&turn("grep", serde_json::json!({"pattern": "fn"}))),
            ok(&turn("write_file", serde_json::json!({"path": "src/lib.rs", "content": "fn main() {}\n// x\n"}))),
            ok(&turn("run_command", serde_json::json!({"command": "true"}))),
            ok(&finish("done")),
            ok(&finish("done")), // VERIFY_NUDGE가 1차 finish를 반려한다
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session =
            Session::new(agent.initial_history(), Transcript::create_at(&tpath).unwrap());
        run_quiet(&mut agent, &mut session, "요청").await.unwrap();

        let touches: Vec<serde_json::Value> = std::fs::read_to_string(&tpath)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
            .filter(|v| v["kind"] == "touch")
            .map(|v| serde_json::from_str(v["content"].as_str().unwrap()).unwrap())
            .collect();

        // read_file은 경로를 준다 — 항해 지표의 유일한 원천
        let read = touches.iter().find(|t| t["tool"] == "read_file").expect("read_file 기록");
        assert_eq!(read["path"], "src/lib.rs");
        // grep의 args에는 path가 **선택적**이고 이 호출은 안 줬으므로 null이다.
        // (grep은 path로 파일을 지목할 수도 있다 — 스펙 개정 10. 항해 지표에서
        //  빼는 것은 축의 정의에서 나오는 설계 결정이지 기술적 제약이 아니다)
        let grep = touches.iter().find(|t| t["tool"] == "grep").expect("grep 호출 계수");
        assert!(grep["path"].is_null(), "path를 안 준 grep 호출은 null로 기록된다: {grep}");
        // write_file은 수선 지표의 원천
        let write = touches.iter().find(|t| t["tool"] == "write_file").expect("write_file 기록");
        assert_eq!(write["path"], "src/lib.rs");
        // finish·run_command는 기록 대상이 아니다
        assert!(touches.iter().all(|t| t["tool"] != "run_command"));
    }

    /// 실패한 디스패치는 접촉이 아니다 — 기록은 `if dispatch_ok` **안**에 있어야 한다.
    ///
    /// ⚠ **이 테스트에 본문이 있어야 반증이 성립한다.** 초판은 이것을 주석 한 줄뿐인
    /// **빈 함수**로 출하했고, 빈 테스트는 항상 통과하므로 Step 4의 "블록을
    /// `dispatch_ok` 밖으로 옮기면 실패해야 한다"가 **절대 실패할 수 없었다**
    /// (플랜 1R 실현 I3). 본문을 넣으면 실제로 `left: 1, right: 0`으로 실패한다
    #[tokio::test]
    async fn failed_dispatch_is_not_recorded_as_a_touch() {
        let dir = tempfile::tempdir().unwrap();
        let tpath = dir.path().join("t.jsonl");
        // 존재하지 않는 파일 → read_file이 Error를 돌려준다 (dispatch_ok=false)
        let script = Scripted::new(vec![
            ok(&turn("read_file", serde_json::json!({"path": "nope.rs"}))),
            ok(&finish("done")),
        ]);
        let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
        let mut session =
            Session::new(agent.initial_history(), Transcript::create_at(&tpath).unwrap());
        run_quiet(&mut agent, &mut session, "요청").await.unwrap();

        let n = std::fs::read_to_string(&tpath)
            .unwrap()
            .lines()
            .filter(|l| l.contains("\"kind\":\"touch\""))
            .count();
        assert_eq!(n, 0, "실패한 디스패치는 접촉이 아니다 (기록이 dispatch_ok 밖으로 샜다)");
    }
```

- [ ] **Step 2: 실패를 확인한다**

```bash
cargo test --lib agent::tests::touched_files 2>&1 | tail -10
```
Expected: FAIL — `read_file 기록` (touch 이벤트 0건)

- [ ] **Step 3: `if dispatch_ok` 블록에 형제 분기를 더한다**

`src/agent/mod.rs`, `if dispatch_ok { … }` 블록의 **끝**(`edit_file | write_file` 분기 다음, 닫는 중괄호 앞):

```rust
                // M15 H10·§5-4: 항해/수선 지표의 원자료. **툴별로 분리**한다 —
                // 셋을 한 집합으로 합치면 §1-1 축의 근거인 M8 실패 분석의
                // "미열람(grep만)" 구분이 사라져 축과 계측기가 어긋난다.
                //
                // 항해 지표를 read_file 집합만으로 정의하는 것은 **축의 정의에서
                // 나오는 설계 결정**이지 기술적 제약이 아니다. §1-1이 이 트랙의
                // 축을 세운 근거가 M8 실패 분석의 "monthly.rs 미열람(grep만)"이고,
                // 그 구분은 "grep으로 스쳤는가"와 "열어서 읽었는가"를 **가르는 것이
                // 목적**이다 — 그래서 grep으로 오라클을 지목한 런을 항해 성공에서
                // 빼는 것은 부작용이 아니라 의도된 방향이다.
                // ⚠ grep은 path로 **파일 하나를 지목할 수 있다**(grep.rs의
                // `if base.is_file()` 분기). "grep은 경로를 못 준다"는 것은 사실이
                // 아니다 — 스펙 개정 10이 이 근거를 정정했다. list_files는 참이다
                // (walk_entries가 `if p == base { continue }`로 시작점을 버린다).
                // 경로는 status_note::normalize로 정규화해 표기 변형을 합산한다
                if matches!(
                    turn.action.tool.as_str(),
                    "read_file" | "edit_file" | "write_file" | "grep" | "list_files"
                ) {
                    let touched = matches!(
                        turn.action.tool.as_str(),
                        "read_file" | "edit_file" | "write_file"
                    )
                    .then(|| turn.action.args.get("path").and_then(|v| v.as_str()))
                    .flatten()
                    .map(status_note::normalize);
                    session.record_extra(
                        "touch",
                        &serde_json::json!({"tool": turn.action.tool, "path": touched}).to_string(),
                    );
                }
```

⚠ **차용 검사는 실제로 통과한다**(1R 실현 확인 — 이 블록 그대로 컴파일된다). 초판의 경고는 예방적 문구였다. 혹시 에러가 나면 `touched`를 먼저 지역 변수에 담고 `session.record_extra`를 그 뒤에 두는 형태로 분리할 것.

- [ ] **Step 4: 통과 + 반증 확인**

```bash
cargo test --lib agent 2>&1 | tail -10
```

반증: 새 블록을 `if dispatch_ok` **밖**으로 옮기면 `failed_dispatch_is_not_recorded_as_a_touch`가 실패해야 한다. 확인 후 되돌린다.

- [ ] **Step 5: 게이트 + 커밋**

```bash
cargo test && cargo clippy --all-targets -- -D warnings
git add src/agent/mod.rs
git commit -m "feat(agent): 툴별 분리 접촉 파일 기록 — 항해/수선 지표 원자료 (H10)"
```

---

### Task 14: `exp_metrics.py` — 토큰 회계 컬럼과 `notice` 처리 (H15 전반부)

**Files:**
- Modify: `scripts/exp_metrics.py` (`run_metrics`, `report_index`, `process`, `COLS`, `selftest`)

**Interfaces:**
- Consumes: T11의 `usage`/`pack` 이벤트, T12의 `notice` 이벤트, T7의 `RunRecord.protected_edits`
- Produces: 신규 컬럼 9종 — T15가 풀링 요약에서 재사용한다

**Consumers:** ① T15(풀링 요약) ② T16(세션 모드가 같은 파서를 쓴다) ③ T23 사전등록 §6-4-19⑤ ④ T25 리포트. **Rust 소비자 0건.**

⚠ **`exp_metrics.py`는 Rust 상수를 손으로 복사한다** — 이 태스크가 더하는 것은 **키 이름 계약**(`usage`/`pack`/`touch`의 JSON 키)이므로, T11·T13의 `serde_json::json!` 키와 **문자 그대로 일치**해야 한다. 드리프트 자동 검출이 없다.

- [ ] **Step 1: `--selftest`에 실패하는 기대를 먼저 넣는다**

`scripts/exp_metrics.py`의 `selftest()`가 쓰는 픽스처 트랜스크립트에 이벤트를 추가하고, `process()` 출력에 대한 단언을 더한다. 기존 셀프테스트의 구성을 그대로 따르되 다음을 포함할 것:

```python
        # M15 H15 — 축 C 이벤트 3종. 키 이름은 Rust의 serde_json::json! 리터럴과
        # 문자 그대로 같아야 한다(session.rs::pack, agent/mod.rs의 usage/notice)
        ev("usage", json.dumps({"prompt_tokens": 1000, "completion_tokens": 20,
                                "estimate_tokens": 800, "messages": 5,
                                "budget": 25804, "inline_system": False,
                                "overflow_shrinks": 0})),
        ev("usage", json.dumps({"prompt_tokens": 2600, "completion_tokens": 30,
                                "estimate_tokens": 2000, "messages": 9,
                                "budget": 25804, "inline_system": True,
                                "overflow_shrinks": 1})),
        ev("pack", json.dumps({"budget": 25804, "before": 30000, "after": 25000,
                               "elided": 2, "dropped": 0})),
        ev("notice", "(컨텍스트 초과로 보임 — 히스토리 절삭 후 재시도 1/2)"),
        ev("notice", "(컨텍스트 초과 — context_tokens 설정과 서버 로드 설정을 확인하세요)"),
```

기대값:

```python
    # max_prompt=2600, max_est=2000 → est_ratio_max = 2600/2000 = 1.30
    # (§4-1-1의 r_obs 정의 = 턴별 prompt_tokens/estimate_tokens의 **최댓값**.
    #  평균이 아니다 — 오버플로를 결정하는 것은 최대 턴이다)
    # ⚠ 기존 selftest는 `row`를 **리스트**로 두고 `row[col["verify_failed"]]`로
    #    접근한다(`scripts/exp_metrics.py:742-748`). 그 형태를 따를 것 —
    #    `row["max_prompt"]`는 리스트 첨자라 TypeError다(2R 실현 m1)
    assert row[col["max_prompt"]] == "2600", row
    assert row["max_est"] == "2000", row
    assert row["est_ratio_max"] == "1.3000", row
    assert row["budget_ratio_max"] == "0.1008", row   # 2600/25804
    assert row["pack_turns"] == "1", row
    assert row["pack_elided"] == "2", row
    assert row["overflow_shrink"] == "1", row
    assert row["overflow_giveup"] == "1", row
    assert row["inline_sys_turns"] == "1", row
    # ⚠ H7 — 축 C 일곱 항목의 ⑦. **셀프테스트 없이 두면 안 된다**(1R 측정 I2):
    #    report_index의 `r.get("protected_edits", 0)`이 필드 부재 시 조용히 0을 주고,
    #    이 컬럼은 §5-2 ⑦이 "리워드 해킹의 **유일한** 기계 관측 발자국"으로 지정한
    #    것이라 0이 "해킹 없음"으로 읽힌다 — 정확히 fail-open이다.
    #    기존 셀프테스트가 이미 tempfile로 합성 report.json을 만들어 process()를
    #    태우므로(:731-746) 그 report.json의 run에 protected_edits를 넣고 단언한다
    assert row["protected_edits"] == "2", row
```

- [ ] **Step 2: 실패를 확인한다**

```bash
python3 scripts/exp_metrics.py --selftest
```
Expected: 신규 컬럼이 없으므로 **`KeyError` 또는 단언 실패**.

⚠ Step 3의 튜플 확장을 먼저 하고 Step 3b(언팩 5곳)를 안 하면 **`ValueError: too many values to unpack (expected 12, got 13)`**로 죽는다 — 그건 신규 단언에 닿지도 못한 것이므로 "실패 확인"으로 세지 말 것. Step 3b까지 마친 뒤의 실패만 유효하다.

- [ ] **Step 3: `MARKS` 밖의 이벤트 파서를 더한다**

`run_metrics`는 지금 마커 계수 위주다. **`usage`/`pack`/`notice`는 마커가 아니라 구조화 이벤트**이므로 별도 누산기를 둔다. `run_metrics`의 지역 변수 초기화부에 추가:

```python
    # M15 §5-2 ①~⑤ 축 C 누산기. MARKS와 달리 부분문자열 계수가 아니라
    # 구조화 JSON(session.rs::pack / agent/mod.rs의 usage)을 읽는다 —
    # 키 이름이 Rust의 serde_json::json! 리터럴과 문자 그대로 같아야 하고
    # 자동 드리프트 검출이 없다(MARKS 문자열과 같은 사정)
    max_prompt = 0        # 턴별 서버 실측 입력 토큰의 최댓값
    max_est = 0           # 같은 턴들의 estimate_tokens 최댓값
    est_ratio_max = 0.0   # r_obs = max(prompt/estimate) — §4-1-1의 정의(평균 아님)
    budget_ratio_max = 0.0
    last_budget = 0
    pack_turns, pack_elided, pack_dropped = 0, 0, 0
    inline_sys_turns = 0
    overflow_shrink, overflow_giveup = 0, 0
    usage_rows = []       # (prompt, est, messages, inline_system) — §5-3 회귀 입력
    # ⚠ **아래 넷은 T15가 채우지만 선언은 여기다**(2R Critical 1). `tok`이 이들을
    # 참조하므로 T14만 적용한 상태에서도 정의돼 있어야 한다 — 개정 2는 선언을
    # T15에 두고 참조만 T14에 넣어 **T14 단독 적용 시 모든 run_metrics 호출이
    # `NameError: name 'read_set' is not defined`로 죽었다.** T14 자신의 Step 6
    # 게이트가 도달 불가였고 Step 7이 망가진 스크립트를 커밋했다.
    # ⚠ 이것은 1R 실현 Critical 5(“튜플을 바꾸고 소비자를 전수로 안 갱신했다”)와
    # **같은 형태이며, 그 Critical을 고치는 자리에서 재발했다**
    read_set, edit_set = set(), set()   # T15 Step 1이 touch 이벤트로 채운다
    grep_calls, list_calls = 0, 0
```

이벤트 루프의 `if kind == "assistant": continue` **다음**, 마커 계수 **다음**에:

```python
        if kind == "usage":
            u = json.loads(content)
            p, est = u.get("prompt_tokens"), u.get("estimate_tokens")
            last_budget = u.get("budget") or last_budget
            if u.get("inline_system"):
                inline_sys_turns += 1
            # prompt_tokens는 서버가 안 주면 None이다 — 0으로 대체하면 §5-3
            # 회귀가 원점을 지나는 거짓 관측을 얻는다(H12 주석과 같은 사정)
            if p is not None and est:
                usage_rows.append((p, est, u.get("messages") or 0, bool(u.get("inline_system"))))
                max_prompt = max(max_prompt, p)
                max_est = max(max_est, est)
                # r_obs는 **턴별 비율의 최댓값**이지 최댓값끼리의 비가 아니다
                est_ratio_max = max(est_ratio_max, p / est)
                if last_budget:
                    budget_ratio_max = max(budget_ratio_max, p / last_budget)
            continue
        if kind == "pack":
            pk = json.loads(content)
            pack_turns += 1
            pack_elided += pk.get("elided") or 0
            pack_dropped += pk.get("dropped") or 0
            continue
        if kind == "notice":
            # 축소 재시도는 M14가, 최종 포기는 M15 H14가 기록한다 — 둘은
            # 다른 사건이므로 절대 합산하지 않는다(전자는 회복, 후자는 사망)
            if "히스토리 절삭 후 재시도" in content:
                overflow_shrink += 1
            elif "컨텍스트 초과 — context_tokens" in content:
                overflow_giveup += 1
            continue
```

⚠ **세 분기 모두 `continue`가 필수다** — 초판은 `notice` 분기에서 이것을 빠뜨렸다(1R 실현 Critical 5 부수). 빠지면 오버플로 포기 notice가 `RepetitionStop` 직전에 올 때 `last_body`를 덮어 `stop_cause`가 `sr` 대신 `other`로 **오분류된다**(재현 확인됨).

⚠ **삽입 위치**: `last_body = content`(`scripts/exp_metrics.py:214`) **보다 앞**, 즉 마커 계수 직후다. 초판은 *"기존 `if kind != "tool_result": continue`(`:221`)보다 앞"*이라고 적었는데 그것만으로는 불변식이 성립하지 않는다 — `last_body` 대입이 그 가드보다 **먼저** 무조건 실행되기 때문이다(1R 사실 m4).

반환 튜플의 **마지막에 `tok` 딕셔너리 하나**를 더한다(T16의 `session_mode`가 이것을 소비한다):

```python
    tok = {
        "max_prompt": max_prompt, "max_est": max_est,
        "est_ratio_max": est_ratio_max, "budget_ratio_max": budget_ratio_max,
        "pack_turns": pack_turns, "pack_elided": pack_elided, "pack_dropped": pack_dropped,
        "overflow_shrink": overflow_shrink, "overflow_giveup": overflow_giveup,
        "inline_sys_turns": inline_sys_turns,
        "usage_rows": usage_rows,          # §5-3 회귀 입력
        "read_set": read_set, "edit_set": edit_set,   # T15가 채운다
        "grep_calls": grep_calls, "list_calls": list_calls,
    }
    return (counts, recovered, denom, fin_max, perturb_turns, last_body,
            first_mut_turn, cargo_after_mut, status_in_args, sr_files, perturb_ext,
            sr_corr_total, tok)
```

- [ ] **Step 3b: ⚠ `selftest()`의 고정폭 언팩 5곳을 함께 고친다**

⚠ **이것을 빠뜨리면 `--selftest`가 새 단언에 닿기도 전에 `ValueError: too many values to unpack (expected 12, got 13)`로 죽는다**(1R 실현 Critical 5). 초판은 이 연쇄를 언급조차 하지 않았다.

기준 커밋에서 `selftest()` 안의 고정폭 언팩은 **정확히 5곳**이다 — `:418`, `:430`, `:435`, `:451`, `:459`. **각각의 꼬리를 `*_`로 바꾼다**(앞으로의 확장에도 안 깨진다).

⚠ **`process()`의 언팩(`:326-328`, 12개)이 여섯 번째다**(2R 실현 m6·측정 m6). Step 4가 `process()`를 어차피 고치지만 **거기서 `tok`을 받도록 명시할 것**:

```python
        (counts, rec, den, fin_max, perturb, last,
         first_mut, cargo_mut, st_args, sr_files, perturb_ext,
         sr_corr_total, tok) = run_metrics(events)
```

```python
# 예: :430
_, _, _, _, perturb2, *_ = run_metrics(events2)
# 예: :451
(c4, _, _, _, _, _, fm4, cm4, sa4, sf4, *_) = run_metrics(events4)
```

```bash
grep -n "run_metrics(" scripts/exp_metrics.py | grep -c "_, _"   # 고친 뒤 재확인
```

⚠ 인덱스 접근(`run_metrics(...)[0]`·`[10]`·`[11]`)은 **끝에 붙이므로 안 깨진다** — 손대지 말 것.

- [ ] **Step 4: `COLS`와 행 조립에 컬럼을 더한다**

```python
COLS = ["run", "outcome", "passed"] + list(MARKS) + [
    "sr_recovered", "sr_recovery_denom", "finish_missing_maxrun", "perturb_turns", "stop_cause",
    "first_mut_turn", "cargo_after_mut", "zero_mut_end", "status_in_args", "sr_files",
    "verify_failed", "sr_corr_total", "perturb_turns_ext", "parse_fail_first",
    "finish_nudge_total", "pipe_unreleased",
    # M15 축 C (§5-2 ①~⑤). est_ratio_max가 §4-1-1의 r_obs이고, 그 정의는
    # **턴별 비율의 최댓값**이다(평균이 아니다 — 오버플로를 결정하는 것은 최대 턴)
    "max_prompt", "max_est", "est_ratio_max", "budget_ratio_max",
    "pack_turns", "pack_elided", "pack_dropped",
    "overflow_shrink", "overflow_giveup", "inline_sys_turns",
    # M15 H7 — report.json에서 온다(트랜스크립트에 없다)
    "protected_edits",
]
```

`report_index`가 `protected_edits`도 싣도록 확장:

```python
def report_index(stamp_dir):
    """run 이름 → (outcome, passed, protected_edits, task_name). M15에서
    protected_edits(H7)와 과제 이름이 추가됐다 — 후자는 §6-4-19①의 과제 단위
    층화 집계에 필요하다(T15)."""
    idx = {}
    path = os.path.join(stamp_dir, "report.json")
    if not os.path.exists(path):
        return idx
    rep = json.load(open(path))
    for t in rep.get("tasks", []):
        for r in t.get("runs", []):
            idx[f"run-{t['name']}-{r['repeat']}"] = (
                r.get("outcome", "?"), r.get("passed"),
                r.get("protected_edits", 0), t["name"],
            )
    return idx
```

⚠ **`idx.get(name, …)`의 기본값을 4-튜플로 함께 고칠 것** — 지금은 `("?", None)`이다.

비율 컬럼은 소수 4자리 고정 포맷으로 찍는다(`f"{est_ratio_max:.4f}"`) — 셀프테스트가 문자열로 비교하므로 포맷이 계약이다.

- [ ] **Step 5: 요약 라인에 배치 수준 값을 더한다**

```python
    print(f"# tokens max_prompt={batch_max_prompt} est_ratio_max={batch_ratio:.4f} "
          f"pack_turns={batch_pack} overflow_shrink={batch_shrink} "
          f"overflow_giveup={batch_giveup} protected_edits={batch_prot}")
```

⚠ **`est_ratio_max`의 배치 값은 런별 값의 최댓값이다** — 평균이 아니다. §4-1-1이 `r_obs`를 그렇게 정의했고, T22의 분기 판정이 이 숫자를 그대로 쓴다.

- [ ] **Step 6: 셀프테스트 통과 + 실배치 회귀 확인**

```bash
python3 scripts/exp_metrics.py --selftest
```
Expected: 통과 메시지.

기존 배치가 여전히 처리되는지(신규 이벤트가 하나도 없는 M14 이전 스탬프):

```bash
python3 scripts/exp_metrics.py .loco/eval/20260719T093254Z 2>&1 | tail -5
```
Expected: 예외 없이 표와 요약. 신규 컬럼은 전부 0 / `0.0000`.

- [ ] **Step 7: 커밋**

```bash
git add scripts/exp_metrics.py
git commit -m "feat(metrics): 토큰 회계 컬럼 + notice 처리 — 축 C 집계 (H15 전반부)"
```

---

### Task 15: `exp_metrics.py` — 항해/수선 지표와 풀링 모드 (H15 후반부)

**Files:**
- Modify: `scripts/exp_metrics.py` (`run_metrics`, `process`, 신규 `pool()`, `selftest`)

**Interfaces:**
- Consumes: T13의 `touch` 이벤트, T8의 `report.json` `tasks[].procure.oracle_files`, T14의 컬럼
- Produces: `--pool` 모드의 배치 수준 요약 — T25 리포트가 인용한다

**Consumers:** ① T23 사전등록 §6-4-19(분석 계획 전체) ② T25 리포트. **Rust 소비자 0건.**

⚠ **§6-1이 배치를 `--filter`로 4~5개 하위 배치로 쪼개므로** 배치 수준 수치(§6-2·§6-3·§6-4-6·§6-4-7·§6-4-19)가 4~5개 요약으로 흩어진다. **풀링 모드가 없으면 사전등록의 분석 계획을 실행할 수 없다.**

**§6-4-19가 못박은 분석 계획 — 이 태스크가 구현하는 계약:**

| # | 요구 | 구현 |
|---|---|---|
| ① | 분모 = 해당 과제의 **층별** 런 수(통과 층/실패 층 각각). 과제별 층내 비율을 먼저 구하고 과제 수준 값을 평균. **층 크기 0인 과제는 그 층 평균에서 제외하고 제외된 과제 수를 함께 보고** | `stratified_rate()` |
| ② | 교집합 판정을 §3-4-3과 동일하게 `≠ ∅` | `set(touched) & set(oracle)` |
| ③ | §5-4 제약 3의 **층화 비합산을 공약으로 승격** — 통과 층과 실패 층을 절대 합산하지 않는다 | 합산 코드를 아예 두지 않는다 |
| ④ | 부트스트랩 재추출 단위 = **과제**. ⚠ ①의 제외와 상호작용 — **"제외 후 남은 집합에서 재추출한다"** | `bootstrap_ci()` |
| ⑤ | §5-3 절편/기울기 추정 = 턴 단위 최소자승 회귀, `inline_system`으로 층화 | `estimator_fit()` |
| ⑥ | §5-5 `prompt_tokens` 의미 확정 결과와 원자료 | T16 세션 모드가 낸다 |

- [ ] **Step 1: `touch` 이벤트 누산을 더한다**

`run_metrics`에 (T14의 누산기 옆):

```python
    # M15 H10·§5-4 — **툴별로 분리**해 모은다. 합치면 §1-1 축의 근거인
    # M8 실패 분석의 "미열람(grep만)" 구분이 사라진다.
    # 항해 지표를 read_set만으로 정의하는 것은 **축의 정의에서 나오는 설계
    # 결정**이다(스펙 개정 10) — "grep이 경로를 못 준다"가 아니다. grep은
    # path로 파일 하나를 지목할 수 있고(grep.rs의 base.is_file() 분기),
    # 그런 런을 항해 성공에서 빼는 것이 "미열람(grep만)" 구분의 목적이다.
    # grep/list_files는 호출 계수로만 남긴다
```

⚠ **네 이름(`read_set`·`edit_set`·`grep_calls`·`list_calls`)의 선언은 T14에 이미 있다** — T15는 채우기만 한다. 여기서 다시 선언하면 매 이벤트마다 초기화된다.

이벤트 루프에 (T14의 `usage`/`pack`/`notice` 분기 옆):

```python
        if kind == "touch":
            t = json.loads(content)
            tool, p = t.get("tool"), t.get("path")
            # ⚠ **normalize_path를 여기서도 건다**(1R 실현 I4). oracle_index()가
            # 정규화한 경로를 쓰므로, 원문 경로를 그대로 넣으면 `./src/x.rs`와
            # `src/x.rs`가 어긋나 **진짜 히트가 nav_hit=0으로 조용히 누락된다** —
            # §6-4-19②의 교집합 의미론을 직접 갉는다.
            # (Rust 쪽 T13이 status_note::normalize를 이미 걸지만 두 정규화가
            #  같다는 보장을 코드로 강제할 수단이 없어 여기서도 건다 — 멱등이다)
            p = normalize_path(p) if p else None
            if tool == "read_file" and p:
                read_set.add(p)
            elif tool in ("edit_file", "write_file") and p:
                edit_set.add(p)
            elif tool == "grep":
                grep_calls += 1
            elif tool == "list_files":
                list_calls += 1
            continue
```

- [ ] **Step 2: 런별 컬럼을 더한다**

```python
COLS += ["nav_hit", "fix_hit", "reads", "greps", "lists"]
```

⚠ **행 조립도 함께 고친다**(2R 측정 m3 — 개정 2는 `COLS`만 늘리고 행에 5칸을 붙이는 곳을 안 보였다. 헤더/행 폭 불일치는 `--selftest`가 새 컬럼을 안 건드리면 **조용히 통과**하고, 그 표가 M15 배치의 1차 산출물이다):

```python
        row += [nav_hit, fix_hit, str(len(tok["read_set"])),
                str(tok["grep_calls"]), str(tok["list_calls"])]
```

`reads`는 **집합 크기**(고유 파일 수), `greps`/`lists`는 **호출 수**다 — 단위가 다르다.

`process()`에서 오라클과 교집합을 판정한다:

```python
        # §6-4-19②: 교집합 판정은 §3-4-3과 **동일하게** `≠ ∅`다.
        # 오라클이 없는 과제(기존 두 트리)는 "-" — 0이 아니다. 0으로 찍으면
        # "항해 실패"로 읽히는데 사실은 "해당 없음"이다(§6-4-19①이 같은 이유로
        # 층 크기 0인 과제를 평균에서 제외한다)
        oracle = set(oracle_by_task.get(task_name, []))
        nav_hit = "-" if not oracle else ("1" if read_set & oracle else "0")
        fix_hit = "-" if not oracle else ("1" if edit_set & oracle else "0")
```

`oracle_by_task`는 `report.json`에서 만든다 — `report_index` 옆에 추가:

```python
def oracle_index(stamp_dir):
    """과제 이름 → 오라클 소스 파일 목록 (M15 H11·§5-4 입력 계약).

    report.json에 **동결**된 것을 읽는다 — 별도 파일에서 읽으면 사후 변경이
    가능해진다. 경로는 normalize_path로 정규화해 트랜스크립트의 표기 변형
    (`./src/x.rs` vs `src/x.rs`)과 합산되게 한다."""
    out = {}
    path = os.path.join(stamp_dir, "report.json")
    if not os.path.exists(path):
        return out
    for t in json.load(open(path)).get("tasks", []):
        pr = t.get("procure") or {}
        out[t["name"]] = [normalize_path(f) for f in pr.get("oracle_files", [])]
    return out
```

- [ ] **Step 3: 풀링 모드를 구현한다**

`process()`가 행을 **반환**하도록 바꾸고(출력은 유지), 신규 `pool()`이 여러 스탬프의 행을 합쳐 §6-4-19의 집계를 낸다.

⚠ **행의 형태가 계약이다**(1R 실현 I5). `pool()`이 받는 행은 **TSV 문자열이 아니라 타입이 살아 있는 딕셔너리**여야 한다. `dict(zip(COLS, row))`로 만들면 `passed` 셀이 `str(passed)`라 `"True"`가 되고, `stratified_rate`의 `r["passed"] is want`가 **예외 없이 전부 거짓**이 되어 **모든 과제가 `excluded`로 빠지고 `mean=nan`**이 된다. `process()`는 출력용 TSV 행과 **별도로** 다음을 반환한다:

```python
        rows.append({
            "run": name,
            "task": task_name,          # report_index가 준 과제 이름
            "passed": passed,           # ⚠ bool 그대로. str()로 감싸지 말 것
            "outcome": outcome,
            "nav_hit": nav_hit,         # "1" | "0" | "-"
            "fix_hit": fix_hit,
            "tok": tok,                 # §5-3 회귀·토큰 집계 입력
            "counts": counts,           # 마커 계수 (기회 분모 계산용)
        })
```

```python
def stratified_rate(rows, metric, stratum):
    """§6-4-19① — 과제별 **층내** 비율을 먼저 구하고 과제 수준으로 평균한다.

    stratum: "pass" | "fail". 층 크기가 0인 과제는 **그 층의 평균에서 제외**하고
    제외 수를 함께 돌려준다 — 3/3 통과 과제를 항해 지표 0으로 넣으면 그것은
    "항해 실패"가 아니라 "해당 없음"이라 지표가 거짓말을 한다(5R I3).

    ⚠ 통과 층과 실패 층은 **절대 합산하지 않는다**(§5-4 제약 3, §6-4-19③ 공약).
    이 함수는 한 층만 본다 — 합산 경로를 코드에 두지 않는 것이 그 공약의 형태다.

    returns (per_task_rates: dict[task]->float, excluded: int)
    """
    want = (stratum == "pass")
    per_task, excluded = {}, 0
    tasks = sorted({r["task"] for r in rows})
    for t in tasks:
        cell = [r for r in rows
                if r["task"] == t and r["passed"] is want and r[metric] in ("0", "1")]
        if not cell:
            excluded += 1
            continue
        per_task[t] = sum(1 for r in cell if r[metric] == "1") / len(cell)
    return per_task, excluded


def bootstrap_ci(values, resamples, seed):
    """§6-4-7·§6-4-19④ — 재추출 단위는 **과제**다. 런 수준 구간은 어떤 형태로도
    보고하지 않는다(사전등록 공약).

    ⚠ §6-4-19①의 제외와 상호작용한다: 여기 들어오는 values는 **이미 제외된 뒤
    남은 집합**이다. 전체에서 재추출하고 정의된 것만 집계하면 추정 대상이
    달라진다(6R M3) — 호출자가 그 순서를 지킨다.

    seed·resamples는 사전등록에 명시된 값을 쓴다(기본 10000·seed 0)."""
    import random
    if not values:
        return (float("nan"), float("nan"))
    rng = random.Random(seed)
    n = len(values)
    means = []
    for _ in range(resamples):
        means.append(sum(rng.choice(values) for _ in range(n)) / n)
    means.sort()
    lo = means[int(0.025 * resamples)]
    hi = means[min(int(0.975 * resamples), resamples - 1)]
    return (lo, hi)


def estimator_fit(usage_rows):
    """§5-3·§6-4-19⑤ — prompt_tokens ≈ 절편(메시지 수 × 상수) + 기울기 × estimate.

    서버의 prompt_tokens는 채팅 템플릿이 렌더한 **전체** 토큰(역할 태그·특수
    토큰·BOS)을 세고 estimate_tokens는 메시지 본문만 센다. pack()의 예산 판단에
    위험한 것은 **기울기**다 — 절편은 메시지 수에 비례해 예측 가능하지만 기울기
    오차는 본문이 길수록 커진다.

    턴 단위 최소자승, `inline_system`으로 층화(직렬화 메시지 집합이 다르다).
    stdlib만 쓰므로 2변수 정규방정식을 직접 푼다."""
    out = {}
    for key in (False, True):
        pts = [(est, msgs, p) for (p, est, msgs, inl) in usage_rows if inl is key]
        if len(pts) < 3:
            out[key] = None
            continue
        # p ≈ a·est + b·msgs  (원점 통과 2변수 — 절편을 "메시지 수 × 상수"로
        # 정의한 것이 §5-3의 분해이므로 상수항을 따로 두지 않는다)
        s_ee = sum(e * e for e, _, _ in pts)
        s_em = sum(e * m for e, m, _ in pts)
        s_mm = sum(m * m for _, m, _ in pts)
        s_ep = sum(e * p for e, _, p in pts)
        s_mp = sum(m * p for _, m, p in pts)
        det = s_ee * s_mm - s_em * s_em
        if det == 0:
            out[key] = None
            continue
        out[key] = {
            "slope_per_est_token": (s_ep * s_mm - s_mp * s_em) / det,
            "intercept_per_message": (s_mp * s_ee - s_ep * s_em) / det,
            "n": len(pts),
        }
    return out


def task_pass_rates(rows):
    """§6-4-7 — 과제별 통과 비율(과제 수준 단위). 주 지표 `passed`의 분석 단위다.
    ⚠ 런 수준이 아니다: 반복은 독립 3시행이 아니라 같은 픽스처·프롬프트를 공유한다."""
    per_task = {}
    for t in sorted({r["task"] for r in rows}):
        cell = [r for r in rows if r["task"] == t]
        per_task[t] = sum(1 for r in cell if r["passed"]) / len(cell)
    return per_task


def disqualification(per_task_rates):
    """§6-4-6 실격 대역 — `N − 전승 과제 수 < 0.98·√N` (바닥 쪽 대칭).
    A5의 판정 입력이다. ⚠ 정규근사로 **사전 고정**된 대역이며 부트스트랩 CI와
    수치가 일치할 필요는 없다(5R M2)."""
    n = len(per_task_rates)
    if not n:
        return None
    sweep = sum(1 for v in per_task_rates.values() if v == 1.0)
    zero = sum(1 for v in per_task_rates.values() if v == 0.0)
    band = 0.98 * (n ** 0.5)
    return {
        "N": n, "all_pass": sweep, "all_fail": zero, "band": band,
        "disqualified": (n - sweep) < band or (n - zero) < band,
    }


def pool(stamp_dirs, resamples=10000, seed=0):
    """§6-1의 4~5분할을 배치 수준 하나로 되돌린다.

    기존 동작(`for d in sys.argv[1:]: process(d)`)은 스탬프마다 **독립 표·요약**을
    찍고 교차 풀링이 없었다 — 그러면 §6-2·§6-3·§6-4-6·§6-4-7·§6-4-19의
    배치 수준 수치를 낼 수 없다(4R 실현 I1).

    ⚠ **§6-1이 풀링 필요 근거로 든 다섯을 전부 낸다**(1R 측정 I1). 초판은
    §6-4-19(항해/수선)만 구현해 **A5(실격 대역)와 A6(추정기 오차)의 산출 경로가
    없었다** — 둘 다 §9의 **차단 기준**인데도 그랬다.
    """
    rows = []
    for d in stamp_dirs:
        rows.extend(process(d))
    print(f"\n# pooled over {len(stamp_dirs)} stamp dir(s), {len(rows)} runs")

    # ── §6-4-7 통과율 (주 지표) ─────────────────────────────────────
    pr = task_pass_rates(rows)
    vals = sorted(pr.values())
    mean = sum(vals) / len(vals) if vals else float("nan")
    lo, hi = bootstrap_ci(vals, resamples, seed)
    print(f"# pass_rate tasks={len(vals)} mean={mean:.4f} ci95=[{lo:.4f},{hi:.4f}] "
          f"resamples={resamples} seed={seed}")

    # ── §6-4-6 실격 대역 (A5 입력) ──────────────────────────────────
    dq = disqualification(pr)
    if dq:
        print(f"# disqualification N={dq['N']} all_pass={dq['all_pass']} "
              f"all_fail={dq['all_fail']} band={dq['band']:.2f} "
              f"disqualified={dq['disqualified']}")

    # ── §6-4-19① 항해/수선 (층별, 비합산) ───────────────────────────
    for metric in ("nav_hit", "fix_hit"):
        for stratum in ("pass", "fail"):
            per_task, excluded = stratified_rate(rows, metric, stratum)
            v = sorted(per_task.values())
            m = sum(v) / len(v) if v else float("nan")
            l2, h2 = bootstrap_ci(v, resamples, seed)
            print(f"# {metric}[{stratum}] tasks={len(v)} excluded={excluded} "
                  f"mean={m:.4f} ci95=[{l2:.4f},{h2:.4f}] "
                  f"resamples={resamples} seed={seed}")

    # ── §6-4-19⑤ 추정기 오차 (A6 입력) ─────────────────────────────
    # ⚠ 초판은 estimator_fit을 T16의 1세션 스모크에서만 불렀다. A6가 **배치**의
    #   추정기 오차 보고를 차단 기준으로 걸므로 여기서 60런 전체를 합쳐 적합한다
    all_usage = [u for r in rows for u in r["tok"]["usage_rows"]]
    for inl, f in estimator_fit(all_usage).items():
        print(f"# estimator inline_system={inl} {f}")
    batch_ratio = max((r["tok"]["est_ratio_max"] for r in rows), default=0.0)
    print(f"# tokens est_ratio_max={batch_ratio:.4f} "
          f"max_prompt={max((r['tok']['max_prompt'] for r in rows), default=0)} "
          f"pack_turns={sum(r['tok']['pack_turns'] for r in rows)} "
          f"overflow_shrink={sum(r['tok']['overflow_shrink'] for r in rows)} "
          f"overflow_giveup={sum(r['tok']['overflow_giveup'] for r in rows)}")

    # ── §6-3 마커 계수와 **기회 분모** (B1 입력) ────────────────────
    # ⚠ "0회도 답이다 — **기회 분모와 함께 볼 때만**"(§1-2 답 1). 분자만 찍으면
    #   0이 "장치가 안 먹었다"인지 "기회가 없었다"인지 구별되지 않는다
    piped = sum(r["counts"]["pipe_note"] for r in rows)
    print(f"# pipe device fired={sum(r['counts']['verify_nudge_pipe'] + r['counts']['finish_nudge_pipe'] + r['counts']['status_pipe_qual'] for r in rows)} "
          f"opportunities={piped}   # 분모 = 파이프 포함 run_command 수(pipe_note 프록시)")
    print(f"# finish_nudge fired={sum(r['counts']['finish_nudge'] + r['counts']['finish_nudge_pipe'] for r in rows)} "
          f"armed_runs~={sum(1 for r in rows if r['counts']['model_diff'] and r['counts']['verify_total'])}"
          f"   # ⚠ APPROX 분모 — 무장 조건(뮤테이션 후 exit 0 검증)의 **근사치**다."
          f" 정확한 값은 finish_nudge.rs 상태기계 재현이 필요하다(perturb_turns·sr_corr_total 선례)."
          f" 리포트에 정확 분모로 인용하지 말 것 — §9-B1이 거짓이 된다")
    diffs = sum(r["counts"]["model_diff"] for r in rows)
    trunc = sum(r["counts"]["model_diff_trunc"] for r in rows)
    print(f"# a3_diff attached={diffs} truncated={trunc} "
          f"truncation_rate={(trunc / diffs if diffs else float('nan')):.4f}"
          f"   # §1-2 답 1: A-3에서 새로 얻는 것은 절단률뿐이다(효과는 측정 불가)")

    print("# NOTE 통과 층과 실패 층은 합산하지 않는다 (§5-4 제약 3·§6-4-19③ 공약)")
    print("# NOTE 재추출 단위는 과제다 — 런 수준 구간은 보고하지 않는다 (§6-4-7)")
```

⚠ **`model_diff_trunc` 마커를 `MARKS`에 새로 넣어야 한다** — 기존 `model_diff`(`" lines, +"`)는 절단·비절단 양쪽 헤더에 매치해 **분모만** 준다(CLAUDE.md 명시). 절단률의 분자는 `tools/diff.rs`가 붙이는 `"[diff truncated]"`다:

```python
    # M15 — A-3 절단률(§6-3)의 분자. model_diff는 양쪽에 매치해 분모 역할이다.
    # 문자열은 tools/diff.rs에서 문자 그대로 복사(수동 미러 — 자동 검출 없음)
    "model_diff_trunc": "[diff truncated]",
```

⚠ **FINISH_NUDGE 무장 분모는 위 식으로 정확히 안 나온다.** 마커만으로는 "무장 조건 충족"을 재현할 수 없다(`finish_nudge.rs`의 상태기계다). **T15에서는 근사치를 찍고, 정확한 분모가 필요하면 `perturb_turns`·`sr_corr_total`이 그랬듯 상태기계 재현을 추가한다** — 그 판단은 T23 사전등록에서 §6-3 항목을 확정할 때 내린다. 근사임을 출력에 표시할 것.

`__main__`에 `--pool` 분기를 더한다. ⚠ **기존 `else: sys.exit(__doc__)`를 반드시 보존한다**(1R 사실 I3) — 초판의 교체본이 그것을 삭제해 인자 없는 호출이 "usage + exit 1"에서 "무출력 + exit 0"으로 바뀌었다. 코드 주석이 그 동작을 *의도*라고 못박고 있다:

```python
if __name__ == "__main__":
    if len(sys.argv) >= 2 and sys.argv[1] == "--selftest":
        selftest()
    elif len(sys.argv) >= 3 and sys.argv[1] == "--pool":
        # 재추출 횟수·시드는 사전등록이 등록한 값을 쓸 수 있어야 한다(§6-4-7).
        # 기본값(10000·0) 외의 값을 쓰려면 코드를 고쳐야 하는 상태를 피한다
        args, resamples, seed = [], 10000, 0
        it = iter(sys.argv[2:])
        for a in it:
            if a == "--resamples":
                resamples = int(next(it))
            elif a == "--seed":
                seed = int(next(it))
            else:
                args.append(a)
        pool(args, resamples=resamples, seed=seed)
    elif len(sys.argv) >= 3 and sys.argv[1] == "--session":
        session_mode(sys.argv[2])
    elif len(sys.argv) >= 2:
        for d in sys.argv[1:]:
            process(d)
    else:
        sys.exit(__doc__)  # 인자 없는 호출은 실패가 의도 — usage를 stderr로, exit 1
```

⚠ **기존 무플래그 동작을 바꾸지 말 것** — M10 이후 모든 배치의 grep 레시피가 그 형식에 붙어 있다. 무플래그 경로의 출력 형식(표 + `# summary` 줄)도 그대로다.

- [ ] **Step 4: 셀프테스트를 확장한다**

`selftest()`에 `--pool` 경로를 태우는 케이스를 더한다. **합성 스탬프 디렉터리 2개**를 임시로 만들고(기존 셀프테스트가 이미 `tempfile`로 `process()`를 태운다), 아래를 **실제 `assert`로** 쓴다.

⚠ **개정 2는 이 자리를 주석 5줄로 두고 `assert`를 하나도 안 썼다**(2R 실현 I5·측정 m7). 그러면 Step 6의 변조 (a)(b)(c)는 검증자가 손으로 단언을 지어내야만 실패하고, 자연스럽게 떠오르는 형태(예: `pool()` 바깥에서 CI를 다시 계산해 비교)는 **변조를 통과한다.** 프로젝트 메모리 *"플랜 리뷰는 예시 코드를 검증하지 않는다"*가 겨눈 형태다.

```python
    # ── 축 C ⑥ (툴별 접촉) — §9-A6의 "일곱 항목" 중 유일하게 단언이 없던 것 ──
    # (2R 측정 I8) read/edit/grep 분기가 뒤바뀌어도 셀프테스트가 초록불이었다.
    # §5-4 축 전체가 이 분리에 걸려 있는데도 그랬다
    touch_ev = [
        ev("touch", json.dumps({"tool": "read_file", "path": "./src/a.rs"})),
        ev("touch", json.dumps({"tool": "read_file", "path": "src/a.rs"})),   # 정규화로 합쳐짐
        ev("touch", json.dumps({"tool": "edit_file", "path": "src/b.rs"})),
        ev("touch", json.dumps({"tool": "grep", "path": None})),
        ev("touch", json.dumps({"tool": "list_files", "path": None})),
    ]
    tk = run_metrics(touch_ev)[12]
    assert tk["read_set"] == {"src/a.rs"}, tk        # normalize_path로 표기 변형이 합산된다
    assert tk["edit_set"] == {"src/b.rs"}, tk        # 수선이 항해에 안 섞인다
    assert tk["grep_calls"] == 1 and tk["list_calls"] == 1, tk
    assert "src/b.rs" not in tk["read_set"], "수선을 항해로 세면 §1-1 축이 무너진다"

    # ── §6-4-19 공약 (pool()의 stdout을 캡처해 검사) ──────────────────
    # ① 층 크기 0인 과제는 **제외**되고 제외 수가 보고된다. 3/3 통과 과제는
    #    실패 층이 비어 nav_hit[fail]에서 빠진다 — 0으로 넣으면 "항해 실패"로 오독된다
    #    ⚠ 값이 **비자명**해야 변조 (a)를 잡는다. 합성 데이터에 맞춰 실제 수를 넣을 것
    assert "nav_hit[fail] tasks=1 excluded=1" in out, out
    # ② 오라클 없는 과제의 nav_hit은 "-" — 0이 아니라 "해당 없음"이다
    assert "\t-\t" in table_out, table_out
    # ③ **비합산** — 합산 라벨이 출력에 아예 없다 (§5-4 제약 3·§6-4-19③ 공약)
    assert "nav_hit[all]" not in out and "nav_hit[pooled]" not in out, out
    # ④ 같은 seed에서 부트스트랩이 재현된다
    assert out == pool_output_second_call, "seed 고정이 안 먹었다"
    # ⑤ estimator_fit이 inline_system **층별**로 나온다
    assert "estimator inline_system=False" in out and "estimator inline_system=True" in out, out
    # ⑥ A5 판정 입력 (§6-4-6)
    assert "disqualification N=" in out and "disqualified=" in out, out
    # ⑦ 주 지표의 불확실성 (§6-4-7)
    assert "pass_rate tasks=" in out and "ci95=[" in out, out
```

- [ ] **Step 4b: `continue` 3개를 단언으로 고정한다**

⚠ 플랜이 세 `continue`를 "필수"라고 적어 놓고 **어떤 단언도 그것을 잡지 않았다**(2R 실현 I6 — `notice`·`usage` 어느 쪽 `continue`를 제거해도 `--selftest`가 rc=0이었다).

```python
    # continue가 빠지면 last_body가 이 이벤트로 덮여 stop_cause가 오분류된다
    ev_stop = [
        ev("tool_result", "Error: edit failed: search and replace are identical"
                          " - no change would be made", "edit_file", {"path": "a.rs"}),
        ev("notice", "(컨텍스트 초과 — context_tokens 설정과 서버 로드 설정을 확인하세요)"),
    ]
    last_s = run_metrics(ev_stop)[5]
    assert stop_cause("repetition_stop", last_s) == "sr", \
        "notice 분기의 continue가 빠지면 last_body가 덮여 stop_cause가 'other'가 된다"
```

- [ ] **Step 5: 셀프테스트 + 실배치 회귀**

```bash
python3 scripts/exp_metrics.py --selftest
python3 scripts/exp_metrics.py .loco/eval/20260719T093254Z 2>&1 | tail -5
python3 scripts/exp_metrics.py --pool .loco/eval/20260719T082030Z .loco/eval/20260719T093254Z 2>&1 | tail -20
```
Expected: 셀프테스트 통과. 실배치는 예외 없이 처리되고, 오라클이 없으므로 `nav_hit`/`fix_hit`이 전부 `-`이며 풀링의 항해/수선 줄이 `tasks=0 excluded=<전체>`가 된다 — **그것이 올바른 출력이다**(0이 아니라 "해당 없음"). `pass_rate`·`disqualification` 줄은 오라클과 무관하므로 **실제 값이 나와야 한다.**

- [ ] **Step 6: 반증 확인 — 세 공약이 실제로 코드에 사는가**

⚠ **초판에서 T15만 반증 단계가 없었다**(1R 측정 m2). 그런데 T15는 §6-4-19의 **층 크기 0 제외**·**제외 후 재추출**·**비합산**이 코드 형태로 사는 **유일한 자리**다. 셀프테스트의 단언은 미구현 상태에서 `NameError`로 죽을 뿐이라 비어있지 않음(non-vacuity)을 증명하지 못한다.

**세 가지를 각각 끊어 보고 셀프테스트가 실패하는지 확인한다:**

| # | 변조 | 실패해야 하는 단언 |
|---|---|---|
| (a) | `stratified_rate`에서 `excluded += 1; continue`를 `per_task[t] = 0.0`으로 바꾼다 | ① 층 크기 0인 과제가 제외되는가 — 전승 과제가 `nav_hit[fail]`에 **0으로** 들어가면 "항해 실패"로 오독된다 |
| (b) | `bootstrap_ci`에 `per_task.values()` 대신 **전체 과제 목록**을 넘긴다 | ④ 제외 후 남은 집합에서 재추출하는가 |
| (c) | `pool()`에 `nav_hit[pass]`와 `nav_hit[fail]`을 합산해 찍는 줄을 추가한다 | ③ 비합산 공약 — ⚠ 이건 코드가 자동으로 못 막는다. **셀프테스트에 "합산 줄이 출력에 없다"는 단언을 넣어** 막는다: `assert "nav_hit[all]" not in out` |

```bash
# 각 변조 후
python3 scripts/exp_metrics.py --selftest ; echo "rc=$?"
```
Expected: (a)·(b) 모두 **rc≠0**. 되돌린 뒤 rc=0. (c)는 단언을 먼저 넣고 합산 줄을 추가해 실패를 본다.

⚠ 어느 하나라도 변조 상태에서 rc=0이면 **그 공약은 코드에 안 살고 문서에만 있는 것이다.**

- [ ] **Step 7: 커밋**

```bash
git add scripts/exp_metrics.py
git commit -m "feat(metrics): 항해/수선 층별 지표 + 과제 단위 부트스트랩 + 풀링 모드 (H15 후반부)"
```

**보조 계수의 집계 규칙**(1R 실현 m3 — 초판이 정의하지 않았다): `reads`/`greps`/`lists`는 **런당 값**이고 배치 집계는 **합**이다(`est_ratio_max`만 최대다). `reads`는 **집합 크기**(고유 파일 수)이고 `greps`/`lists`는 **호출 수**다 — 전자는 "몇 개를 열었나", 후자는 "몇 번 훑었나"라 단위가 다르다. 이 정의를 컬럼 주석에 박을 것.

---

### Task 16: `exp_metrics.py --session` — 스모크 집계 경로 (H19)

**Files:**
- Modify: `scripts/exp_metrics.py` (신규 `session_mode()`, `__main__`, `selftest`)

**Interfaces:**
- Consumes: T11의 `usage`/`pack` 이벤트 (T14의 파서를 재사용)
- Produces: `r_obs` 한 줄 — **T22의 분기 판정이 이 숫자를 그대로 쓴다**

**Consumers:** ① T22(분기 확정) ② T23 사전등록 §6-4-8·§6-4-19⑥. **Rust 소비자 0건.**

⚠ **`exp_metrics.py`는 eval 스탬프 디렉터리만 받는다**(`:304`, `:324`가 `report.json`과 `run-*.jsonl`을 요구한다). 스모크는 **1세션**이고 산출물이 `.loco/sessions/*.jsonl` 하나라 지금은 **읽을 방법이 없다.** T14·T15의 배치용 컬럼·풀링이 이 경로를 안 덮는다.

- [ ] **Step 1: 셀프테스트에 세션 모드 기대를 넣는다**

```python
    # M15 H19 — 1세션 트랜스크립트에서 r_obs를 낸다. report.json이 없다
    # (스모크는 eval이 아니라 `cargo run -- -p …` 1회다)
```

기대: 합성 세션 파일 하나를 임시로 쓰고 `session_mode(path)` 출력에서

```python
    assert "r_obs=1.3000" in out, out
    assert "first_turn_prompt_tokens=1000" in out, out
    assert "pack_fired=1" in out, out
```

- [ ] **Step 2: 실패를 확인한다**

```bash
python3 scripts/exp_metrics.py --selftest
```
Expected: `NameError: name 'session_mode' is not defined`

- [ ] **Step 3: 구현한다**

```python
def session_mode(path):
    """단일 세션 트랜스크립트에서 §4-1-1 스모크 산출을 낸다 (M15 H19).

    exp_metrics.py의 나머지는 eval 스탬프 디렉터리(report.json + run-*.jsonl)를
    받는다 — 스모크는 `cargo run -- -p …` 1회라 .loco/sessions/*.jsonl 하나뿐이고
    그 경로를 읽을 수단이 없었다.

    산출 셋:
      r_obs                      — 턴별 prompt_tokens/estimate_tokens의 **최댓값**
                                   (평균이 아니다. 오버플로를 결정하는 것은 최대 턴)
      first_turn_prompt_tokens   — §5-5의 의미 확정용. 세션 첫 턴은 **정의상 캐시
                                   미적중**이므로 이 값이 "완전 프롬프트" 기준이다
                                   (serve.sh에 캐시 차단 플래그가 없고 추가하면
                                   핀 변경이라 비교가능성에 걸린다 — 7R I1)
      pack_fired                 — §4-1-1의 도달 조건. **0이면 스모크가 예산점에
                                   못 닿은 것이라 §5-3 회귀가 외삽이 된다**
    """
    events = [json.loads(l) for l in open(path)]
    (counts, rec, den, fin_max, perturb, last, first_mut, cargo_mut, st_args,
     sr_files, perturb_ext, sr_corr_total, tok) = run_metrics(events)
    first = None
    for e in events:
        if e.get("kind") == "usage":
            u = json.loads(e.get("content") or "{}")
            if u.get("prompt_tokens") is not None:
                first = u["prompt_tokens"]
                break
    print(f"# session {path}")
    print(f"r_obs={tok['est_ratio_max']:.4f} max_prompt={tok['max_prompt']} "
          f"max_est={tok['max_est']} first_turn_prompt_tokens={first} "
          f"pack_fired={tok['pack_turns']} budget_ratio_max={tok['budget_ratio_max']:.4f} "
          f"overflow_shrink={tok['overflow_shrink']} overflow_giveup={tok['overflow_giveup']}")
    fit = estimator_fit(tok["usage_rows"])
    for inl, f in fit.items():
        print(f"# estimator inline_system={inl} {f}")
    if not tok["pack_turns"]:
        print("# WARN pack 미발동 — 예산점에 못 닿았다. §4-1-1 도달 조건 미충족이므로 "
              "세션을 더 길게 돌릴 것 (§5-3 회귀가 3배 외삽이 된다)")
    return tok
```

⚠ `run_metrics`의 반환에 `tok` 딕셔너리(T14의 축 C 누산기 + `usage_rows`)를 더하는 형태로 T14를 마무리해 둘 것 — **T14 Step 3의 "튜플 확장"은 마지막 원소를 이 `tok` 딕셔너리로 두는 것을 뜻한다.**

`__main__`:

```python
    elif len(sys.argv) >= 3 and sys.argv[1] == "--session":
        session_mode(sys.argv[2])
```

- [ ] **Step 4: 셀프테스트 + 커밋**

```bash
python3 scripts/exp_metrics.py --selftest
python3 scripts/exp_metrics.py .loco/eval/20260719T093254Z 2>&1 | tail -3   # 기존 경로 무회귀
git add scripts/exp_metrics.py
git commit -m "feat(metrics): --session 모드 — 스모크 1세션에서 r_obs 산출 (H19)"
```

---

### Task 17: 조달 스크립트 — `git archive`로 픽스처를 만든다 (H4·H8)

**Files:**
- Create: `scripts/procure_real.sh`
- Modify: `.gitignore` (`tasks-real/*/fixture/`)

**Interfaces:**
- Consumes: T8의 `procure.toml` 형식
- Produces: `<task_dir>/fixture/` 실체화 + `<cache>/<repo>/<sha>/manifest.tsv` — T21이 실행하고 §9-A2가 재조달로 검증한다

**Consumers:** ① T21(실행) ② §9-A2(캐시를 비운 재조달 + 매니페스트 일치) ③ T19의 CLAUDE.md 커맨드 절. **Rust 소비자 0건** — H4가 요구한 대로 `run_eval`/`run_verify`는 이 스크립트를 부르지 않는다.

⚠ **왜 하네스 밖인가**(H4): `run_eval`/`run_verify` 안에 넣으면 **게이트가 네트워크 의존이 되고 기존 두 트리에도 걸린다.** `eval`/`--verify`는 픽스처 부재 시 **명확한 메시지로 실패**하면 된다(`task.rs:61-63`의 기존 `bail!`이 그 역할을 이미 한다).

⚠ **`.git`을 복사하면 정답이 노출된다**: eval은 `AutoApprover`로 돌고(`mod.rs:172`) `auto_deny_patterns` 기본 11종 중 **git 계열은 `git\s+push` 하나뿐**이라(`config.rs:38-49`) `git show <fix-sha>`가 실행 가능하다. `read_file`도 `.git/...`을 읽는다(`confine`에 도트파일 필터 없음). **그래서 `git archive`다.**

⚠ **4레포 전부 shallow clone이다** — `git rev-parse <sha>^`가 경계 근처에서 실패한다. **조달 원본은 파일럿 클론이 아니라 별도 pristine 클론**이다(파일럿엔 로컬 전용 커밋 7개 + 2.7GB 워크트리). unshallow는 사실상 공짜다(4레포 full bare clone 총 33MB·6.2초).

⚠ **이 머신에 `timeout`/`gtimeout`이 없다** — 쓰면 rc=127로 조용히 무동작한다. **사용 금지.**

- [ ] **Step 1: 스크립트를 쓴다**

`scripts/procure_real.sh`:

```sh
#!/bin/sh
# tasks-real 픽스처 조달 (M15 H4·H8·§3-5).
#
# 하네스 밖의 **명시 단계**다 — run_eval/run_verify 안에 넣으면 게이트가 네트워크
# 의존이 되고 기존 두 트리에도 걸린다(H4). eval/--verify는 픽스처가 없으면
# task.rs:61-63의 bail!로 명확히 실패한다.
#
# .git을 샌드박스에 넣지 않는 것이 핵심이다: eval은 AutoApprover로 돌고
# auto_deny_patterns 기본 11종 중 git 계열은 push뿐이라 `git show <fix-sha>`로
# 정답 열람이 가능하다. 그래서 `git archive`로 트리만 뽑는다.
#
# usage:
#   LOCO_REAL_REPOS=~/loco-real-repos \
#   LOCO_TASKS_REAL_CACHE=~/loco-tasks-real-cache \
#     scripts/procure_real.sh tasks-real/<task-dir> [...]
#   scripts/procure_real.sh --all tasks-real
#
# ⚠ 이 머신에 timeout/gtimeout이 없다 — 쓰면 rc=127로 조용히 무동작한다(§10-7).
set -eu

: "${LOCO_REAL_REPOS:?pristine bare 클론들이 있는 디렉터리를 지정하세요}"
: "${LOCO_TASKS_REAL_CACHE:?캐시 디렉터리를 지정하세요 (레포 밖)}"

sha256() {
  if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | cut -d' ' -f1
  elif command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
  else echo "sha256 도구가 없습니다 (shasum/sha256sum)" >&2; exit 1
  fi
}

# 캐시 채우기 — <cache>/<repo>/<sha>/ 는 **읽기 전용 원본**이다.
# 멱등: .complete 마커 기반. 실패 시 부분 디렉터리를 지운다
fill_cache() {
  repo="$1"; sha="$2"
  git_dir="$LOCO_REAL_REPOS/$repo.git"
  dest="$LOCO_TASKS_REAL_CACHE/$repo/$sha"
  [ -f "$dest/meta/.complete" ] && { echo "  캐시 히트: $repo/$sha"; return 0; }

  [ -d "$git_dir" ] || { echo "pristine 클론이 없습니다: $git_dir" >&2; exit 1; }

  # shallow 경계 검증 — 4레포 전부 shallow라 부모 트리가 없을 수 있다.
  # unshallow는 공짜다(4레포 합 33MB·6.2초)이므로 여기서 즉시 고친다
  if [ -f "$git_dir/shallow" ]; then
    echo "  unshallow: $repo"
    git -C "$git_dir" fetch --unshallow origin || git -C "$git_dir" fetch --depth=2147483647 origin
  fi
  git -C "$git_dir" rev-parse --verify "$sha^{commit}" >/dev/null \
    || { echo "SHA가 업스트림에 없습니다: $repo $sha" >&2; exit 1; }

  # ⚠⚠ **메타 파일은 추출 트리 안에 두지 않는다**(2R Critical 2·3, Important 7).
  # 개정 2는 `<sha>/` 하나에 추출물과 메타(manifest.tsv·symlinks.txt·.complete…)를
  # 섞고 `not_meta()` 이름 필터로 갈랐는데, 그 설계가 결함 셋을 한꺼번에 낳았다:
  #   ① 필터에 `.files`를 안 넣어 매니페스트가 **자기 자신을 셌다**(3파일/2파일).
  #      게다가 결정적이라 §9-A2의 재조달 매니페스트 대조가 **통과하면서 틀린다**
  #   ② 픽스처 실체화의 `tar --exclude`가 libarchive에서 **basename 매칭**이라,
  #      레포 어디에 있든 `manifest.tsv`·`symlinks.txt` 같은 이름의 **진짜 파일이
  #      조용히 사라졌다**(exit 0, 매니페스트에는 남아 아무도 못 본다).
  #      컨트롤러 실측: `--exclude=manifest.tsv`가 모든 깊이의 동명 파일을 제거,
  #      앵커 형태(`--exclude=./x`·`--exclude=/x`)도 동일
  #   ③ 루트에 `manifest.tsv`를 가진 정상 레포가 export-ignore 오탐으로 조달 불가
  # **트리와 메타를 형제 디렉터리로 분리하면 셋이 동시에 사라진다** — 이름 필터도,
  # tar 제외도 필요 없어진다
  tree="$dest/tree"; meta="$dest/meta"
  rm -rf "$dest"; mkdir -p "$tree" "$meta"
  trap 'rm -rf "$dest"' EXIT INT TERM
  # ⚠ 파이프 실패 검출: #!/bin/sh라 pipefail이 없다(dash). git archive가 죽어도
  # tar가 0으로 끝나면 조용히 통과하고, 그것이 아래 export-ignore 가드에서
  # "의심"으로 잘못 표면화된다(1R 실현 I7). 아카이브를 **파일로 먼저 받아** 종료
  # 코드를 직접 본다 — 4레포 최대 트리가 수 MB라 임시 파일 비용은 무시할 만하다
  if ! git -C "$git_dir" archive "$sha" > "$meta/archive.tar"; then
    echo "git archive 실패: $repo $sha" >&2; exit 1
  fi
  tar -xf "$meta/archive.tar" -C "$tree"
  rm -f "$meta/archive.tar"

  # 심링크 목록 — H5의 스킵 대상이고, 아래 가드가 이 목록을 **포함해** 비교한다
  ( cd "$tree" && find . -type l | sed 's|^\./||' | LC_ALL=C sort ) > "$meta/symlinks.txt"

  # export-ignore/export-subst 가드 (§3-5). **경로 집합의 차집합**으로 본다 —
  # 파일 *수* 대조는 gitlink에서 거짓 bail이 나고 export-subst를 못 잡는다.
  # ⚠ export-subst는 파일 수도 경로도 안 바꾸므로 이 자동 가드로는 안 잡힌다.
  #    그 몫은 §3-4-2의 사람 감사다
  #
  # ⚠⚠ **심링크를 포함해 비교하는 것이 계약이다**(1R 실현 C4). `git ls-tree -r`는
  # 심링크를 blob으로 실어 주는데 `find -type f`는 심링크를 빼므로, 포함하지 않고
  # 비교하면 **export-ignore가 0건이어도 모든 심링크가 "의심"으로 잡혀 exit 1**이
  # 된다. 대상 4레포 중 ripgrep(HomebrewFormula)·just(www/man/{en,zh})가 심링크를
  # 가지므로 **조달 자체가 불가능해진다** — 그리고 그것은 T3의 "심링크는 스킵한다"
  # 정책과 이 스크립트 자신의 symlinks.txt 주석과도 정면으로 모순이다
  git -C "$git_dir" ls-tree -r --name-only "$sha" | LC_ALL=C sort > "$meta/tree-paths"
  # 비교용: 일반 파일 **+ 심링크**. 트리에는 메타가 없으므로 이름 필터가 불필요하다
  ( cd "$tree" && find . \( -type f -o -type l \) | sed 's|^\./||' | LC_ALL=C sort ) \
      > "$meta/extracted"
  diff_out=$(LC_ALL=C comm -23 "$meta/tree-paths" "$meta/extracted") || true
  if [ -n "$diff_out" ]; then
    echo "export-ignore 의심 — ls-tree에 있고 아카이브에 없는 경로:" >&2
    echo "$diff_out" >&2
    exit 1
  fi

  # 매니페스트 = **산출물 자체**의 파일 목록 + 크기 + SHA-256.
  # **일반 파일만** — 심링크를 넣으면 dangling(just www/man/{en,zh})에서 wc/sha256이
  # 죽는다. 그래서 비교 목록(extracted)과 매니페스트 목록의 정의가 다르다.
  # ⚠ git 트리 해시는 쓸 수 없다(캐시는 .git 없는 추출 트리다).
  # ⚠ 이것은 **자기정합 검사이지 업스트림 검증이 아니다**(§10-5)
  # ⚠ **심링크 집합은 매니페스트가 안 본다** — §9-A2의 재조달 대조는
  #    `manifest.tsv`와 `symlinks.txt`를 **둘 다** 비교해야 한다(2R 측정 m4)
  ( cd "$tree" && find . -type f | sed 's|^\./||' | LC_ALL=C sort \
      | while IFS= read -r f; do
          printf '%s\t%s\t%s\n' "$f" "$(wc -c < "$f" | tr -d ' ')" "$(sha256 "$f")"
        done ) > "$meta/manifest.tsv"

  trap - EXIT INT TERM
  : > "$meta/.complete"
  echo "  조달 완료: $repo/$sha ($(wc -l < "$meta/manifest.tsv" | tr -d ' ')파일, 심링크 $(wc -l < "$meta/symlinks.txt" | tr -d ' ')개)"
}

procure_task() {
  task_dir="$1"
  toml="$task_dir/procure.toml"
  [ -f "$toml" ] || { echo "procure.toml이 없습니다: $task_dir" >&2; exit 1; }
  # ⚠ 줄끝 앵커(`$`)와 CR 제거가 계약이다(1R 실현 I6). 앵커가 없으면 인라인 주석
  # (`repo = "demo"  # 메모`)이 값에 새어 들고, CRLF 줄끝이면 `\r`이 값에 남는다 —
  # `\r`은 터미널에서 커서를 되돌려 **눈에 안 보이는데** 바이트에는 남아
  # `demo\r.git` 같은 오도적 에러를 낸다. 이 프로젝트는 이미 CRLF 픽스처를 다룬다
  val() { tr -d '\r' < "$toml" | sed -n "s/^$1 *= *\"\\([^\"]*\\)\" *$/\\1/p" | head -1; }
  repo=$(val repo); parent=$(val parent_sha); fix=$(val fix_sha)
  [ -n "$repo" ] && [ -n "$parent" ] && [ -n "$fix" ] \
    || { echo "procure.toml에 repo/parent_sha/fix_sha가 필요합니다: $toml" >&2; exit 1; }

  echo "[$task_dir] $repo $parent (fix $fix)"
  fill_cache "$repo" "$parent"

  # 픽스처 실체화 — H8. <task_dir>/fixture는 git-ignore다
  # ⚠ **`--exclude`를 쓰지 않는다**(2R Critical 3). libarchive의 `--exclude`는
  # **basename 매칭**이라 트리 어디에 있든 그 이름의 진짜 파일을 지운다 —
  # 컨트롤러 실측으로 `src/docs/manifest.tsv`까지 조용히 사라졌고 앵커 형태도
  # 동일했다. 캐시가 `tree/`와 `meta/`를 분리하므로 **제외할 것이 아예 없다**
  src="$LOCO_TASKS_REAL_CACHE/$repo/$parent/tree"
  dst="$task_dir/fixture"
  rm -rf "$dst"; mkdir -p "$dst"
  ( cd "$src" && tar -cf - . ) | ( cd "$dst" && tar -xf - )

  # fixture-overlay/ — 백포트 테스트 등 사람이 얹는 것 (§3-3)
  if [ -d "$task_dir/fixture-overlay" ]; then
    ( cd "$task_dir/fixture-overlay" && tar -cf - . ) | ( cd "$dst" && tar -xf - )
    echo "  오버레이 적용: $task_dir/fixture-overlay"
  fi

  # target/ 가드 (§3-5·§9-A3) — 캐시도 <task_dir>/fixture도 **빌드 디렉터리로
  # 겸하지 않는다**. copy_tree는 .gitignore를 안 보므로(ignore 크레이트 미사용)
  # 픽스처에 target/이 있으면 60런 × 최대 1GB 복사가 된다.
  # 실측: zoxide 371M / fd 255M / ripgrep 459M / just 998M
  for guard in "$src" "$dst"; do
    if [ -e "$guard/target" ]; then
      echo "target/이 있습니다 (빌드 디렉터리 겸용 금지, §3-5): $guard/target" >&2
      exit 1
    fi
  done

  # .gitignore의 /target 요구 확인 — 없으면 모델의 bare list_files가
  # target/ 경로 ~14KB를 최종 메시지로 뱉어 컨텍스트 초과 400을 만든다
  grep -qE '^/?target/?$' "$dst/.gitignore" 2>/dev/null \
    || echo "  경고: .gitignore에 /target 규칙이 없습니다 — 배치 전 확인할 것" >&2
}

if [ "${1:-}" = "--all" ]; then
  root="${2:?tasks-real 루트를 지정하세요}"
  for d in "$root"/*/; do [ -f "$d/procure.toml" ] && procure_task "${d%/}"; done
else
  [ $# -ge 1 ] || { echo "usage: $0 <task-dir> [...] | --all <tasks-real>" >&2; exit 1; }
  for d in "$@"; do procure_task "$d"; done
fi
echo "조달 완료."
```

- [ ] **Step 2: 실행 비트와 `.gitignore`**

```bash
chmod +x scripts/procure_real.sh
printf 'tasks-real/*/fixture/\n' >> .gitignore
git check-ignore -v tasks-real/demo/fixture/x 2>&1 || true
```

⚠ 기존 픽스처의 `/target` `.gitignore` 규약과 무충돌임을 3R이 확인했다.

- [ ] **Step 3: `/bin/sh`로 직접 문법 검사한다**

프로젝트 메모리 *"검증 환경이 검증을 무효화한다"* — 셸 함수가 실제 바이너리를 가릴 수 있으므로 **`/bin/sh`로 직접** 돌린다:

```bash
/bin/sh -n scripts/procure_real.sh && echo "문법 OK"
/bin/sh scripts/procure_real.sh 2>&1 | head -3   # 환경변수 미설정 → 명확한 메시지로 실패해야 한다
```
Expected: `문법 OK`, 그리고 `pristine bare 클론들이 있는 디렉터리를 지정하세요`

- [ ] **Step 4: `timeout` 미사용을 확인한다**

⚠ 초판은 `grep -nE '\b(timeout|gtimeout)\b'`를 썼는데 **스크립트 자신의 경고 주석에 걸려** 약속한 "미사용 확인" 대신 그 주석을 출력했다(1R 실현 m5). 주석을 뺀 뒤 **호출 형태만** 본다:

```bash
grep -vE '^\s*#' scripts/procure_real.sh | grep -nE '(^|[;&|(]\s*)(timeout|gtimeout)\s' \
  || echo "timeout 미사용 확인"
```
Expected: `timeout 미사용 확인`

- [ ] **Step 5: 합성 bare 레포로 실제 조달을 돌린다 — 심링크·export-ignore 양쪽**

⚠ **이 단계를 건너뛰지 말 것.** 초판의 조달 스크립트는 `/bin/sh -n` 문법 검사를 통과하고도 **심링크가 하나만 있으면 무조건 exit 1**이었다(1R 실현 C4). 대상 4레포 중 둘이 심링크를 가지므로 조달 자체가 불가능했는데, 문법 검사로는 안 잡힌다.

```bash
SCRATCH=/private/tmp/claude-501/-Users-sgj-develop-loco/a3f41052-f67f-4b07-889b-33f2d9c2a133/scratchpad/procure-test
rm -rf "$SCRATCH" && mkdir -p "$SCRATCH/src" && cd "$SCRATCH/src"
git init -q . && git config user.email t@t && git config user.name t
mkdir -p docs && echo "a" > a.txt && echo "b" > docs/b.md
ln -s a.txt valid-link.txt          # 정상 심링크 (ripgrep HomebrewFormula 형태)
ln -s nope.txt dangling-link.txt    # dangling (just www/man/{en,zh} 형태)
git add -A && git commit -qm one
echo "c" > c.txt && git add -A && git commit -qm two
git clone -q --bare . "$SCRATCH/repos/demo.git"
PARENT=$(git rev-parse HEAD^) ; FIX=$(git rev-parse HEAD)

mkdir -p "$SCRATCH/task"
printf 'repo = "demo"\nissue_url = "https://example.invalid/1"  # 인라인 주석\nfix_sha = "%s"\nparent_sha = "%s"\noracle_files = ["a.txt"]\n' "$FIX" "$PARENT" > "$SCRATCH/task/procure.toml"

cd /Users/sgj/develop/loco
LOCO_REAL_REPOS="$SCRATCH/repos" LOCO_TASKS_REAL_CACHE="$SCRATCH/cache" \
  /bin/sh scripts/procure_real.sh "$SCRATCH/task"; echo "EXIT=$?"
```
Expected: `조달 완료: demo/<sha> (2파일, 심링크 2개)` + `EXIT=0`.

⚠ **`3파일`이 나오면 메타가 트리 안에 섞인 것이다** — 개정 2가 정확히 그 상태였다(2R Critical 2). `<cache>/<repo>/<sha>/`가 `tree/`와 `meta/` 둘만 갖는지 확인할 것:

```bash
ls "$SCRATCH/cache/demo"/*/            # tree  meta 두 개만
ls "$SCRATCH/cache/demo"/*/tree/       # 레포 파일만 (manifest.tsv 등이 없어야 한다)
cat "$SCRATCH/cache/demo"/*/meta/manifest.tsv   # 2행
```

**basename 충돌 회귀 검사** — 개정 2의 `tar --exclude`가 조용히 지웠던 형태:

```bash
cd "$SCRATCH/src" && mkdir -p docs && echo x > docs/manifest.tsv && echo y > manifest.tsv \
  && git add -A && git commit -qm meta-name-collision
# bare를 갱신하고 procure.toml의 parent_sha를 이 커밋으로 바꾼 뒤 재조달
```
Expected: 픽스처에 `docs/manifest.tsv`와 `manifest.tsv`가 **둘 다 살아 있어야 한다**. 하나라도 없으면 Critical 3이 재발한 것이다.

⚠ **`EXIT=1`에 `export-ignore 의심`이 뜨면 C4가 재발한 것이다** — `.tree-paths`↔`.extracted` 비교에서 심링크를 빼고 있는지 확인할 것.
⚠ 파일 수가 `3파일`로 나오면 `.extracted`/`.files`가 자기 자신을 세고 있는 것이다.

가드가 실제로 발동하는지도 함께 본다:

```bash
cd "$SCRATCH/src" && printf 'c.txt export-ignore\n' > .gitattributes \
  && git add -A && git commit -qm three && git push -q "$SCRATCH/repos/demo.git" HEAD:master 2>/dev/null || true
# 위가 여의치 않으면 bare를 다시 클론해 최신 SHA로 procure.toml을 갱신한 뒤 재실행
```
Expected: `export-ignore 의심` + `EXIT=1` (**가드가 실제로 물어야 한다** — 안 물면 가드가 공허하다)

- [ ] **Step 6: 커밋**

```bash
git add scripts/procure_real.sh .gitignore
git commit -m "feat(scripts): tasks-real 조달 — git archive + 매니페스트 + target/·export-ignore 가드 (H4·H8)"
```

---

### Task 18: 누설 감사 추출기 (§3-4-3)

**Files:**
- Create: `scripts/leak_audit.py`

**Interfaces:**
- Consumes: `check` 출력 캡처 파일, `procure.toml`의 `oracle_files`
- Produces: 지목 판정 — T21이 과제마다 실행하고 §3-2 규약 6이 처분을 고정한다

**Consumers:** ① T21(과제 선정) ② T23 사전등록 항목 4·16(**원 출력과 추출 스크립트 포함**). **Rust 소비자 0건.**

**정의 (스펙 §3-4-3):**

> **지목됨** ⟺ `check` 출력의 **테스트 실패 보고 구간**에 등장하는 소스 경로 집합 ∩ 오라클 소스 파일 집합 ≠ ∅

**판정 범위 한정** (4R 측정 I7): **`failures:` 절 이후 및 각 실패 테스트의 패닉 메시지 구간**만 센다. **컴파일러 진단(`warning:`/`error:`의 `-->` 줄)과 `Running`/`Compiling` 줄은 제외**한다. 근거: 항해 단축은 *실패 보고*가 원인 지점을 가리킬 때만 일어나고, 픽스처는 미완성 기능 주변에 경고가 남기 쉬워 그것까지 세면 누설과 무관하게 과제가 제외된다(§10-2의 희소성 악화).

**추출 절차** (4R 측정 I6 — 감사자 간 일치를 구조적으로 보장):
- 실행 고정: `RUST_BACKTRACE=0` + `--test-threads=1`. **`--verify` 샌드박스 안에서** 돌린다(픽스처 디렉터리에서 직접 돌리면 §3-5의 `target/` 가드를 깬다)
- 출력을 **파일로 캡처**해 사전등록에 **원 출력 그대로** 첨부
- 추출은 **스크립트로 한다(사람 판독 금지)**

- [ ] **Step 1: 스크립트를 쓴다**

`scripts/leak_audit.py`:

```python
#!/usr/bin/env python3
"""§3-4-3 픽스처 누설 감사 — `check` 출력에서 "지목된" 소스 경로를 뽑는다 (M15).

초판의 오류는 종류가 **"프롬프트만 보고 픽스처를 안 봤다"**였다. 결론만 고치고
절차를 안 고치면 다음 배치도 누설을 감사하지 않은 채 착수한다 — 그래서 이
추출을 스크립트로 못박는다. **사람 판독 금지**(감사자 간 일치를 구조적으로 보장).

usage:
  python3 scripts/leak_audit.py <check-output.txt> [--sandbox <abs-prefix>]
                                [--oracle a.rs --oracle b.rs]
  python3 scripts/leak_audit.py --selftest

판정: 추출 집합 ∩ 오라클 집합 ≠ ∅ 이면 **지목됨 → 과제 제외**(§3-2 규약 6).
처분은 하나로 고정돼 있다 — "제외하거나 라벨한다"는 표본 수와 쿼터를 동시에
조작할 자유도가 된다.

표준 라이브러리만 사용(폐쇄망 개발 도구).
"""
import re
import sys

# 소스 경로 후보. `:행:열` 접미는 뒤에서 떼므로 여기서는 .rs까지만 문다
# ⚠ 한 줄이 아주 길면 백트래킹으로 느려진다(1R 실현 m4 실측: 10만 자 한 줄에 7.7초).
# 지수 폭발은 아니지만 큰 단언 덤프가 한 줄로 잡히면 감사가 길어지므로, 줄 길이를
# 잘라서 먼다 — 누설 판정에 필요한 경로는 줄 앞부분에 있다
MAX_LINE = 4000
PATH_RE = re.compile(r"[A-Za-z0-9_./-]+\.rs")

# **제외** 대상 줄 — 판정 범위 한정(4R 측정 I7).
# 컴파일러 진단은 미완성 기능 주변에 남기 쉬워 누설과 무관하게 과제를 떨어뜨린다.
# Running/Compiling은 테스트 **바이너리·크레이트** 이름이지 원인 지점이 아니다.
#
# ⚠⚠ **이 패턴은 `failures:` 구간 *밖*에서만 쓴다**(플랜 1R Critical 4). 구간 안쪽에
# 적용하면 `note:`/`help:`/`warning:` 로 시작하는 **테스트 자신의 단언 메시지**를
# 컴파일러 진단으로 오인해 삼킨다 — 실측 재현: 패닉 본문의
# `note: see src/secret.rs for the actual computation` 이 통째로 버려져
# 오라클 `src/secret.rs`가 "지목되지 않음(rc=0)"으로 통과했다. §3-2 규약 6이 이
# 스크립트를 누설 차단 게이트로 쓰므로 안전장치 자체의 결함이었다.
SKIP_RE = re.compile(r"^\s*(-->|Running|Compiling|Finished|warning:|error(\[|:)|note:|help:)")

# `failures:` 구간 **안**에서만 쓰는 좁은 제외 — 컴파일러 진단의 위치 화살표만 뺀다.
# 진단 본문(`warning:` 등)은 구간 안에 나타나면 그것은 libtest가 캡처한 **테스트
# 출력**이지 컴파일러 출력이 아니다(컴파일은 `failures:` 이전에 끝난다)
SKIP_IN_FAILURES_RE = re.compile(r"^\s*-->")


def failure_region(text):
    """실패 보고 구간만 돌려준다.

    두 구간을 센다:
      ① `failures:` 절 이후 (libtest가 실패 테스트 이름과 상세를 모아 찍는 곳)
      ② 각 실패 테스트의 패닉 메시지 (`thread '…' panicked at <path>:<line>`)

    ⚠ 그 밖은 전부 버린다 — 특히 컴파일러 진단. 항해 단축은 *실패 보고*가
    원인 지점을 가리킬 때만 일어난다(§3-4-3 판정 범위 한정).
    """
    lines = text.splitlines()
    out, in_failures = [], False
    for line in lines:
        if re.match(r"^failures:\s*$", line):
            in_failures = True
            continue
        if re.match(r"^test result:", line):
            in_failures = False
            continue
        if in_failures:
            # 구간 안: 좁은 제외만. 넓은 SKIP_RE를 쓰면 테스트 자신의 note:/help:
            # 단언 메시지가 삼켜져 진짜 누설을 놓친다 (1R Critical 4)
            if not SKIP_IN_FAILURES_RE.match(line):
                out.append(line)
            continue
        # 구간 밖: 컴파일러 진단·Running/Compiling을 전부 뺀 뒤, 패닉 줄만 줍는다
        if SKIP_RE.match(line):
            continue
        if "panicked at" in line:
            out.append(line)
    return out


def extract(text, sandbox_prefix=None):
    """실패 보고 구간의 소스 경로 집합. 샌드박스 절대 경로 접두를 제거해
    레포 상대 경로로 정규화하고 `:행:열` 접미를 뗀다."""
    found = set()
    for line in failure_region(text):
        for m in PATH_RE.findall(line[:MAX_LINE]):
            p = m
            if sandbox_prefix:
                pre = sandbox_prefix.rstrip("/") + "/"
                if p.startswith(pre):
                    p = p[len(pre):]
            p = p.lstrip("./")
            found.add(p)
    return found


def main(argv):
    if "--selftest" in argv:
        return selftest()
    if not argv:
        print(__doc__)
        return 2
    path, sandbox, oracle = argv[0], None, []
    i = 1
    while i < len(argv):
        if argv[i] == "--sandbox":
            sandbox = argv[i + 1]; i += 2
        elif argv[i] == "--oracle":
            oracle.append(argv[i + 1].lstrip("./")); i += 2
        else:
            i += 1
    text = open(path, encoding="utf-8", errors="replace").read()
    found = extract(text, sandbox)
    print("# 실패 보고 구간에서 추출된 소스 경로:")
    for p in sorted(found):
        print(f"  {p}")
    if not oracle:
        print("# (오라클 미지정 — 판정은 하지 않는다)")
        return 0
    hit = found & set(oracle)
    print(f"# 오라클: {sorted(oracle)}")
    if hit:
        print(f"지목됨 — 교집합 {sorted(hit)} ≠ ∅ → **과제 제외** (§3-2 규약 6)")
        return 1
    print("지목되지 않음 — 교집합 공집합 → 채택 가능")
    return 0


def selftest():
    # ① 패닉 메시지가 원인 소스를 지목하는 경우 → 지목됨
    leaky = """
   Compiling foo v0.1.0 (/tmp/loco-eval-1/foo)
warning: unused variable: `x`
  --> /tmp/loco-eval-1/src/walk.rs:12:9
    Finished test [unoptimized] target(s)
     Running tests/cli.rs (target/debug/deps/cli-abc)

running 1 test
test walks_hidden ... FAILED

failures:

---- walks_hidden stdout ----
thread 'walks_hidden' panicked at /tmp/loco-eval-1/src/walk.rs:88:5:
assertion failed

failures:
    walks_hidden

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 filtered out
"""
    got = extract(leaky, "/tmp/loco-eval-1")
    assert "src/walk.rs" in got, got
    # 컴파일러 진단(-->)만 있는 경로는 안 잡혀야 한다
    clean = """
warning: unused import
  --> /tmp/loco-eval-1/src/secret.rs:3:5
     Running tests/cli.rs (target/debug/deps/cli-abc)

failures:

---- t stdout ----
thread 't' panicked at tests/cli.rs:10:5:
assertion failed

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 filtered out
"""
    got2 = extract(clean, "/tmp/loco-eval-1")
    assert "src/secret.rs" not in got2, f"컴파일러 진단은 제외해야 한다: {got2}"
    assert "tests/cli.rs" in got2, got2
    # `test result:` 이후 잡음이 구간을 오염시키지 않는다
    assert not any(p.startswith("target/") for p in got2), got2

    # ③ **1R Critical 4 회귀 방지** — `failures:` 구간 **안**의 note:/help:/warning:은
    #    테스트 자신의 단언 메시지다. 컴파일러 진단으로 오인해 삼키면 진짜 누설을 놓친다.
    #    초판은 정확히 이 입력에서 "지목되지 않음(rc=0)"을 냈다
    leaky_note = """
failures:

---- t stdout ----
thread 't' panicked at tests/cli.rs:10:5:
assertion failed
note: see src/secret.rs for the actual computation
help: compare with src/helper.rs
warning: value drifted in src/drift.rs

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 filtered out
"""
    got3 = extract(leaky_note)
    for must in ("src/secret.rs", "src/helper.rs", "src/drift.rs"):
        assert must in got3, f"failures: 구간 안의 단언 메시지를 삼켰다 — {must} 누락: {got3}"

    # ④ 구간 안의 `-->`(컴파일러 위치 화살표 형태)는 여전히 뺀다
    assert "src/arrow.rs" not in extract(
        "failures:\n\n  --> src/arrow.rs:1:1\n\ntest result: FAILED. 0 passed; 1 failed;\n"
    ), "구간 안이라도 --> 는 제외"

    print("leak_audit selftest OK")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
```

- [ ] **Step 2: 셀프테스트**

```bash
python3 scripts/leak_audit.py --selftest
```
Expected: `leak_audit selftest OK`

- [ ] **Step 3: 반증 확인 — 지목 판정이 실제로 실패할 수 있는가**

```bash
printf 'failures:\n\n---- t stdout ----\nthread %s panicked at src/walk.rs:1:1:\nboom\n\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 filtered out\n' "'t'" > /private/tmp/claude-501/-Users-sgj-develop-loco/a3f41052-f67f-4b07-889b-33f2d9c2a133/scratchpad/leak.txt
python3 scripts/leak_audit.py /private/tmp/claude-501/-Users-sgj-develop-loco/a3f41052-f67f-4b07-889b-33f2d9c2a133/scratchpad/leak.txt --oracle src/walk.rs; echo "rc=$?"
python3 scripts/leak_audit.py /private/tmp/claude-501/-Users-sgj-develop-loco/a3f41052-f67f-4b07-889b-33f2d9c2a133/scratchpad/leak.txt --oracle src/other.rs; echo "rc=$?"
```
Expected: 첫 번째 `지목됨 … rc=1`, 두 번째 `지목되지 않음 … rc=0`. **둘 다 같은 결과면 판정이 무의미한 것이다.**

- [ ] **Step 4: 커밋**

```bash
chmod +x scripts/leak_audit.py
git add scripts/leak_audit.py
git commit -m "feat(scripts): §3-4-3 픽스처 누설 감사 추출기 — 지목 판정 자동화"
```

---

### Task 19: 문서 — `--verify`의 성질 변화와 PROTOCOL 개정

**Files:**
- Modify: `docs/experiments/PROTOCOL.md` (항목 4①·4③, 항목 5), `CLAUDE.md`

**Interfaces:**
- Consumes: T1~T18의 산출물 전부
- Produces: 갱신된 배치 전 게이트 — T23·T24가 그대로 따른다

**Consumers (전수):**
- `PROTOCOL.md:12` 4① — *"두 tasks 트리 `--verify` 통과(12/12·3/3)"*로 **하드코딩**돼 있다 → 세 트리로
- `PROTOCOL.md:20` 4③ — `n_ctx_slot == config의 context_tokens` → **동결값 등호**로
- `PROTOCOL.md:41-44` 항목 5 — **4③의 형제 소비자다**(6R I2). *"이 두 출력이 어떤 모델·컨텍스트로 돌았는가를 증언하는 대체 증거"*라고 적는데, 단순 `>=`로 바꾸면 상한만 주고 증언하지 못한다. **동결값 등호 형태는 이것을 자동으로 보존한다**
- `CLAUDE.md` — M15 항목, 신규 커맨드 3종, `--verify` 성질 변화

⚠ **왜 `>=`가 아니라 동결값 등호인가**(6R I1): 스모크 서버를 안 내린 채 배치를 시작하면 `n_ctx_slot=40960 >= 32768`로 통과한다 — `PROTOCOL.md:22`가 *"직전 배치 잔재는 GPU 시간 전체를 무효화"*로 경고하는 실패 모드를 등호가 막고 있었다.

⚠ **적용 시점을 M15 이후로 표시한다** — M13·M14 앵커 리포트가 소급 재해석되지 않게. 둘 다 `n_ctx_slot == 8192 == context_tokens`라 어느 형태로도 통과한다(6R 확인).

- [ ] **Step 1: PROTOCOL 4①을 세 트리로**

`docs/experiments/PROTOCOL.md:12`:

```markdown
4. **배치 전 게이트**: ① 세 tasks 트리 `--verify` 통과
   (`tasks` 12/12 · `tasks-large` 3/3 · `tasks-real` N/N).
   ⚠ **M15 이후** — `tasks-real`은 조달이 만든 상태에 의존한다(스펙 §3-7).
   `--verify` 자체가 네트워크를 타지는 않지만(조달은 `scripts/procure_real.sh`로
   분리된 명시 단계다) **픽스처가 없으면 명확히 실패한다**. N은 그 배치의
   사전등록이 동결한 과제 수다
```

- [ ] **Step 2: PROTOCOL 4③을 동결값 등호로**

`docs/experiments/PROTOCOL.md:20`을 교체:

```markdown
   - 서버 기동 로그의 `n_ctx_slot` == **사전등록에 동결된 서버 로드 ctx**
     (동결값은 그 배치의 **실효 운용점** 이상이어야 한다)
     ⚠ **M15 이후 형태다.** M15 이전 배치(M13 앵커·M14 게이트)는 원문
     *"config의 `context_tokens`"*로 읽으며, 둘 다 `n_ctx_slot == 8192 ==
     context_tokens`라 어느 형태로도 통과하므로 **소급 재해석되지 않는다**.
     ⚠ 하한이 "config의 `context_tokens`"가 아니라 **실효 운용점**인 이유:
     M15 배치 중 전역 config는 8192이고 `tasks-real` 과제만 `TaskSpec.context_tokens`로
     32768을 덮는다 — 원문 그대로 읽으면 하한이 8192가 되어 무의미하다.
     ⚠ **단순 `>=`로 바꾸면 안 된다**: 스모크 서버를 안 내린 채 배치를 시작하면
     `n_ctx_slot=40960 >= 32768`로 통과해, 아래 *"직전 배치 잔재는 GPU 시간
     전체를 무효화"*가 경고하는 실패 모드가 열린다. **동결값 등호는 과잉
     제약(운용점 = 로드 강제)만 없애고 축소 탐지·잔재 탐지·자증을 전부 보존한다**
```

- [ ] **Step 3: PROTOCOL 항목 5의 증언 성질을 명시한다**

`docs/experiments/PROTOCOL.md:41-44`의 괄호 설명에 한 문장을 더한다:

```markdown
   ... 대체 증거다. ⚠ **이 증언 성질은 4③의 형태에 의존한다** — 4③이 동결값
   등호이므로 `n_ctx_slot` 출력이 "어떤 컨텍스트로 돌았는가"를 **한 값으로**
   증언한다. 부등식(`>=`)이었다면 상한만 주고 증언하지 못한다 (M15 §6-4-17) ...
```

- [ ] **Step 4: `CLAUDE.md`에 M15를 더한다** (영문)

CLAUDE.md 첫 문단의 마일스톤 사슬 끝에 M15 항목을 잇고, `## Commands`에 세 줄을 더한다:

```markdown
- `LOCO_REAL_REPOS=<pristine-bare-clones> LOCO_TASKS_REAL_CACHE=<cache-outside-repo> scripts/procure_real.sh --all tasks-real` — materializes `tasks-real/*/fixture` from pristine bare clones via `git archive <parent_sha>` (never copies `.git` — eval runs under `AutoApprover` and only `git push` is auto-denied, so a copied `.git` would let `git show <fix-sha>` read the answer). Writes a self-consistency manifest (path/size/SHA-256) per `(repo, sha)` cache entry, guards `export-ignore` by path-set difference, and **bails if `target/` exists** in either the cache or the fixture (`copy_tree` ignores `.gitignore`, so a fixture `target/` would be copied 60× at up to 1GB). Deliberately NOT called from `run_eval`/`run_verify` — that would make the gate network-dependent and would also hit the two pre-existing trees (M15 H4)
- `python3 scripts/leak_audit.py <check-output.txt> --sandbox <abs-prefix> --oracle <src>...` — §3-4-3 fixture-leak audit: extracts source paths from the **test-failure reporting region only** (`failures:` section and panic messages; compiler diagnostics `-->`/`warning:`/`error:` and `Running`/`Compiling` lines are excluded because a fixture tends to carry warnings around the unfinished feature, which would exclude tasks for reasons unrelated to leakage). Exit 1 = "named" ⇒ the task is excluded (§3-2 rule 6, a fixed disposition — "exclude or label" would be a degree of freedom over sample size and quota). Stdlib only, `--selftest` (M15)
- `python3 scripts/exp_metrics.py --pool <stamp-dir>...` / `--session <session.jsonl>` — `--pool` merges sub-batches (§6-1 splits a batch into 4-5 `--filter` runs, and the default mode prints one independent table per stamp with no cross-pooling) and computes the §6-4-19 analysis plan: navigation/repair rates with **per-stratum denominators** (pass-stratum and fail-stratum are never summed), tasks with an empty stratum excluded and the excluded count reported, and a **task-level** bootstrap CI drawn from the post-exclusion set. `--session` reads a single smoke transcript (the rest of the script requires an eval stamp dir with `report.json`) and prints `r_obs` = **max over turns** of `prompt_tokens / estimate_tokens` (not the mean — overflow is decided by the max turn) plus the first-turn `prompt_tokens` (definitionally a cache miss, which fixes the §5-5 meaning of the field) (M15 H19)
```

- [ ] **Step 4b: CLAUDE.md에 신규 마커·컬럼을 전수 열거한다**

⚠ **CLAUDE.md는 마일스톤별 마커 추가를 이름으로 전수 열거하는 것이 관례다**(M14 항목: *"adds five markers … `verify_nudge_pipe`/`finish_nudge_pipe`/`status_pipe_qual`/`status_no_summary` … and `model_diff`"*). 개정 2는 커맨드 3줄만 적고 **마커·컬럼을 하나도 안 적었다**(2R 측정 A-8·I6). 특히 T15가 `model_diff`를 절단률 **분모**로 쓰는 근거가 CLAUDE.md의 *"matches … in both its untruncated and truncated form"* 문장인데, **짝인 `model_diff_trunc`를 그 옆에 안 적으면 다음 마일스톤이 같은 판단을 재구성할 수 없다.**

`exp_metrics.py` 항목에 이어 붙일 것 (영문):

```markdown
M15 adds one marker and 16 columns. The marker is `model_diff_trunc` (`"[diff truncated]"`, copied verbatim from `tools/diff.rs` — same manual-mirror caveat as `args_tool_key`/`length_retry`); it is the **numerator** of the A-3 truncation rate, whose denominator is the pre-existing `model_diff` (which matches both the truncated and untruncated header, so it counts attachments, not truncations). The columns are eleven token-accounting ones from T14 (`max_prompt`, `max_est`, `est_ratio_max`, `budget_ratio_max`, `pack_turns`, `pack_elided`, `pack_dropped`, `overflow_shrink`, `overflow_giveup`, `inline_sys_turns`, `protected_edits`) and five navigation ones from T15 (`nav_hit`, `fix_hit`, `reads`, `greps`, `lists`). **`est_ratio_max` is the max over turns of `prompt_tokens / estimate_tokens`, never the mean** — overflow is decided by the max turn, and §4-1-1's branch decision reads this number directly. `nav_hit`/`fix_hit` are `"-"` (not `0`) when the task has no oracle: `0` would read as "navigation failed" when the truth is "not applicable", the same distinction §6-4-19① makes when it excludes empty strata from the mean rather than scoring them zero.
```

- [ ] **Step 5: 확인**

```bash
grep -n "세 tasks 트리\|동결된 서버 로드 ctx\|M15 이후" docs/experiments/PROTOCOL.md
grep -c "M15" CLAUDE.md
grep -c "model_diff_trunc\|est_ratio_max\|nav_hit" CLAUDE.md   # 0이면 Step 4b 미수행
```
Expected: 세 지점 모두 갱신됨, CLAUDE.md에 M15 언급 존재

- [ ] **Step 6: 커밋**

```bash
git add docs/experiments/PROTOCOL.md CLAUDE.md
git commit -m "docs: PROTOCOL 4①·4③·항목 5 개정 (동결값 등호, M15 이후) + CLAUDE.md M15"
```

---

### Task 20: rope 캡처 · 공급량 실사 · 레포 선정

**Files:**
- Create: `docs/experiments/2026-07-20-m15-real-repo-baseline/supply-survey.md`

**Interfaces:**
- Consumes: T1의 동결값(최소 표본 16·편중 상한 60%)
- Produces: 레포 목록과 과제 후보 풀 — T21이 조달한다. `n_ctx_train` — T22의 분기 판정에 **선행해야 한다**

**Consumers:** ① T21(과제 선정) ② T22(분기 3 판정) ③ T23 사전등록 항목 1·3·12.

⚠ **순서**: 이 태스크는 **T1 이후여야 한다**(임계값이 데이터보다 먼저). 그리고 **T22보다 먼저여야 한다** — rope 캡처가 분기 3 판정에 선행한다.

⚠ **`n_ctx_train` 값은 아직 이 레포에 캡처된 적이 없다**(ornith `max_context_length` 262144는 6R가 실험 리포트에서 찾은 값이지 `n_ctx_train`이 아니다). §4-1-1의 분기 전제 — `n_ctx_train ≥ 32768` — 가 여기서 실측된다. **미만이면 분기 3 직행이다.**

- [ ] **Step 1: rope 파라미터를 캡처한다** (배치와 무관한 독립 기동)

```bash
pkill -f llama-server || true
LOCO_MODEL_GGUF=<gguf> LOCO_CTX=32768 scripts/serve.sh > /private/tmp/claude-501/-Users-sgj-develop-loco/a3f41052-f67f-4b07-889b-33f2d9c2a133/scratchpad/rope.log 2>&1 &
sleep 20
grep -iE "n_ctx_train|freq_base|freq_scale|n_ctx_slot|rope" /private/tmp/claude-501/-Users-sgj-develop-loco/a3f41052-f67f-4b07-889b-33f2d9c2a133/scratchpad/rope.log
pkill -f llama-server
```

세 값(`n_ctx_train`·`freq_base`·`freq_scale`)과 원 로그 줄을 `supply-survey.md`에 그대로 옮긴다.

⚠ **축 E는 측정이 아니라 설계 결정이다**(§2). 이 캡처는 배치와 무관한 독립 기동이므로 순서 제약이 없고, **분기 3 판정에만 선행하면 된다.**

- [ ] **Step 2: pristine 클론을 만든다**

⚠ **파일럿 클론에서 조달하지 말 것** — 로컬 전용 커밋 7개(M13 작업물)와 2.7GB 워크트리가 있다.

```bash
mkdir -p ~/loco-real-repos && cd ~/loco-real-repos
for r in "ajeetdsouza/zoxide" "sharkdp/fd" "BurntSushi/ripgrep" "casey/just"; do
  name=$(basename "$r")
  git clone --bare "https://github.com/$r.git" "$name.git"
done
du -sh ~/loco-real-repos    # 4레포 full bare clone 총 ~33MB (3R 실측)
```

- [ ] **Step 3: 전 이력 공급량 실사**

⚠ **3R 실측: shallow 창이 레포마다 전혀 다르다**(fd 약 13개월·298커밋 / zoxide 약 8개월·54 / ripgrep 12일·51 / just 8일·50). **2R의 비율(just 88% / ripgrep 8% / fd 4% / zoxide 2%)은 같은 창의 값이 아니므로 인용 금지다** — §6-4-1이 요구하는 것은 **전 이력 재산출**이다.

레포마다 세는 것: **닫힌 이슈 중 그것을 고친 커밋이 테스트를 추가·수정한 것**의 수, 그리고 §3-2 제외 규약 1~5를 적용한 뒤 남는 수.

```bash
# 예시 — 실제 절차는 gh CLI + git log로 이슈↔커밋을 잇는다.
# 규약 3(테스트 변경이 수정 대상과 같은 파일 — 제외)이 후보를 크게 줄인다:
#   cfg(test) 포함 파일이 ripgrep crates/ 44개, fd src/ 13개 (실측)
```

각 레포에 대해 표를 만든다:

| 레포 | 닫힌 이슈 | 테스트 동반 수정 | 규약 1 후 | 규약 2 후 | 규약 3 후 | 규약 4 후 | 규약 5 후 | **최종 채택 가능** |
|---|---|---|---|---|---|---|---|---|

⚠ **규약 6(§3-4-3 지목됨 → 제외)은 여기서 못 적용한다** — 조달·`check` 실행이 필요하다. T21에서 걸러지므로 **최종 채택 가능 수는 상한이지 확정이 아니다.**

- [ ] **Step 4: 최소 표본·편중 상한과 대조한다**

T1이 동결한 값과 대조한다:
- **최소 표본 16** — 총합이 미달이면 §6-4-2의 처분 셋 중 하나. **최소 표본 자체를 재조정하는 형태는 금지**
- **편중 상한 60%** — 어느 단일 레포도 전체의 60%를 넘지 않는다. 못 채우면 같은 처분. **여기도 상한 자체를 재조정할 수 없다**(7R Minor 5)

근거: §1-1이 축의 실재를 논증한 것은 **대형 레포 조건**인데 공급량대로 배분하면 대부분이 한 레포에서 나올 수 있다.

⚠ **H5(심링크 처리)는 실사 결과와 무관하게 무조건 필요하다** — just에도 필요하다(§2-2 결정 8).

⚠ **규모 축의 실체는 LOC가 아니라 다중 크레이트다**: just 64.7K > ripgrep 54K이고(`pilot-tasks.md:36-37`) ripgrep이 고유하게 주는 것은 **워크스페이스 9크레이트**다.

- [ ] **Step 5: `supply-survey.md`를 쓰고 커밋한다**

담을 것: rope 캡처 원 로그 / 레포별 실사 표 / 최종 레포 선정과 근거 / 최소 표본·편중 상한 충족 여부 / 미달 시 채택한 처분.

```bash
git add docs/experiments/2026-07-20-m15-real-repo-baseline/supply-survey.md
git commit -m "docs(m15): 공급량 실사 + rope 캡처 — 레포 선정 (§6-4-1·§6-4-12)"
```

- [ ] **Step 6: 체크포인트 — 사용자에게 보고한다**

실사 결과와 레포 선정, 최소 표본 충족 여부를 보고한다. **미달이면 어느 처분을 택할지는 사용자 결정이다.**

---

### Task 21: 조달 · 착수 전 3항 감사 · 표본 동결

**Files:**
- Create: `tasks-real/<task>/` × N (`task.toml`, `procure.toml`, `fixture-overlay/`, `solution/`)
- Create: `docs/experiments/2026-07-20-m15-real-repo-baseline/audit/` (과제별 원 출력 + 판정)

**Interfaces:**
- Consumes: T17 조달 스크립트, T18 누설 감사기, T20의 레포 선정
- Produces: 동결된 `tasks-real/` — T22의 스모크 대상과 T24의 배치가 쓴다

**Consumers:** ① T22(스모크) ② T23 사전등록 항목 4·16 ③ T24(배치) ④ §9-A1a·A1b·A2·A3.

**픽스처 구성 — 성립하는 유일한 형태** (§3-3. 세 코드 경로가 다른 형태를 각각 거부한다: `task.rs:67-71` / `verify.rs:86-88` / `verify.rs:117`):

| | 내용 |
|---|---|
| 픽스처 | 부모 트리 **+ 그 커밋의 테스트 변경분 백포트** (조달이 `fixture-overlay/`로 얹는다) |
| `solution/` | 커밋 diff **− 테스트 변경분** |
| `protected` | 그 테스트 경로 |
| `check` | 그 테스트를 지목하는 명령 |

`check`의 필터가 아무것도 못 잡으면 exit 0 → 픽스처 PASS → 판별력 실패로 **M12형 0-테스트 거짓 초록불도 걷어낸다.**

- [ ] **Step 1: 과제 후보마다 `procure.toml`과 `task.toml`을 쓴다**

`tasks-real/<task>/procure.toml`:

```toml
repo = "ripgrep"
issue_url = "https://github.com/BurntSushi/ripgrep/issues/NNNN"
fix_sha = "<40자>"
parent_sha = "<40자>"
oracle_files = ["crates/core/flags/hiargs.rs"]
```

⚠ **`oracle_files`는 "정답 커밋의 비테스트 *소스* 파일"로 좁힌 명시 목록이다**(§5-4 제약 2). 실제 수정 커밋은 `CHANGELOG.md`·문서를 흔히 동반한다(ripgrep 관례) — **레포마다 관례가 다르므로 자동 규칙이 아니라 사람이 열거한다.**

`tasks-real/<task>/task.toml`:

```toml
prompt = "<이슈 본문 그대로. 손대지 않는다>"
check = "cargo test --test <name> <filter>"
context_tokens = 32768
command_timeout_secs = 180
check_timeout_secs = 300
timeout_secs = <T23 §6-4-8이 앵커에서 정한 값>
protected = ["tests/<the-test-file>.rs"]
```

⚠ **프롬프트 = 이슈 본문. 손대지 않는다**(§3-2). 그래서 **항해 거리 라벨은 이슈 본문 하나가 가른다**: 이슈 본문이 파일/함수를 명시 → "단축됨", 아니면 "단축 안 됨". ⚠ **기술 통계 전용이다** — 셀 비교는 과제 간 비교라 난이도와 완전 교락되고 검정력이 없다. **부분군 분석·층화 판정에 쓰지 않는다**(사전등록 공약).

⚠ `timeout_secs` 기본 300s는 llama.cpp 앵커 avg에 비해 좁아 **거의 모든 런이 Timeout으로 떨어질 수 있다** — T23이 앵커에 붙여 값을 정한다.

- [ ] **Step 2: 조달한다**

```bash
LOCO_REAL_REPOS=~/loco-real-repos \
LOCO_TASKS_REAL_CACHE=~/loco-tasks-real-cache \
  scripts/procure_real.sh --all tasks-real
```
Expected: 과제마다 `조달 완료` + 심링크 수. `target/` 가드나 `export-ignore` 의심에 걸리면 **그 과제를 고치거나 뺀다.**

- [ ] **Step 3: 착수 전 사람 감사 3항** (§3-4)

**이것이 M13 대비 유일한 실질 개선점**이므로 자동 게이트에 위임하면 개선이 사라진다. **`--verify`는 이 감사를 대신하지 못한다.**

**3-4-1. 이슈 ↔ 커밋 정합** — ① 이슈가 요구하는 것 중 커밋이 안 한 것 ② 커밋이 한 것 중 이슈가 요구하지 않은 것. 하나라도 있으면 제외하거나 라벨을 분리한다. M13의 F5(*"요청받은 일이 이미 되어 있었다"*)는 `--verify`를 그대로 통과하는 형태다.

**3-4-2. `.gitattributes` 감사** — `export-ignore`/`export-subst`가 있으면 `git archive` 산출물이 부모 트리와 조용히 달라진다. 3R 실측으로 현재 4레포는 사용 0건이지만 **커밋 시점마다 다시 본다**. ⚠ **`export-subst`는 파일 수를 안 바꾸므로 T17의 자동 가드로 안 잡힌다 — 이 사람 감사가 그 몫이다.**

```bash
git -C ~/loco-real-repos/<repo>.git show <parent_sha>:.gitattributes 2>/dev/null | grep -nE 'export-(ignore|subst)' || echo "사용 0건"
```

**3-4-3. 픽스처 누설 감사** — `--verify` 샌드박스 안에서 `check` 출력을 캡처하고 T18로 판정한다:

```bash
# RUST_BACKTRACE=0 + --test-threads=1 로 실행을 고정한다.
# ⚠ 픽스처 디렉터리에서 직접 돌리면 §3-5의 target/ 가드를 깬다 — 샌드박스 안에서
RUST_BACKTRACE=0 cargo run -- eval tasks-real --verify --filter <task> 2>&1 \
  | tee docs/experiments/2026-07-20-m15-real-repo-baseline/audit/<task>-check.txt
python3 scripts/leak_audit.py \
  docs/experiments/2026-07-20-m15-real-repo-baseline/audit/<task>-check.txt \
  --sandbox <샌드박스 절대 경로> --oracle <oracle_files의 각 항목>
```

**처분은 하나로 고정: 지목됨 → 제외**(§3-2 규약 6). *"제외하거나 라벨한다"*는 표본 수와 쿼터를 동시에 조작할 자유도가 된다.

⚠ **원 출력 그대로**와 **추출 스크립트**를 사전등록 산출물에 포함한다.

- [ ] **Step 4: `--verify` 전건 통과를 확인한다** (§9-A1a, 차단 기준)

```bash
cargo run -- eval tasks-real --verify 2>&1 | tail -5
```
Expected: `검증 N/N 통과`. **하나라도 실패하면 그 과제를 표본에서 뺀다.**

- [ ] **Step 5: `target/` 부재와 스테일 뮤테이션을 확인한다** (§9-A3, 차단 기준)

```bash
find tasks-real -maxdepth 3 -name target -print   # 0건이어야 한다
cargo test --lib eval::verify 2>&1 | tail -5      # H16 자동 테스트
```
그리고 `thresholds.md` §4의 **레포별 수동 절차**를 과제마다 한 번 수행한다(경로 표를 볼 것 — ripgrep은 루트에 `src/`가 없다).

- [ ] **Step 6: 캐시를 비운 재조달로 매니페스트 일치를 확인한다** (§9-A2, 차단 기준)

⚠ **캐시 웜 재실행은 `cargo test`의 결정성만 시험한다**(4R 측정 I2). 반드시 비운다:

```bash
mv ~/loco-tasks-real-cache ~/loco-tasks-real-cache.bak
LOCO_REAL_REPOS=~/loco-real-repos LOCO_TASKS_REAL_CACHE=~/loco-tasks-real-cache \
  scripts/procure_real.sh --all tasks-real
for d in ~/loco-tasks-real-cache/*/*/meta/; do
  rel=${d#~/loco-tasks-real-cache/}
  # ⚠ **매니페스트와 심링크 목록을 둘 다** 비교한다(2R 측정 m4) — 매니페스트는
  #    일반 파일만 담으므로 심링크 집합 변화를 혼자서는 못 본다
  diff "$d/manifest.tsv" "$HOME/loco-tasks-real-cache.bak/$rel/manifest.tsv" \
    && diff "$d/symlinks.txt" "$HOME/loco-tasks-real-cache.bak/$rel/symlinks.txt" \
    && echo "일치: $rel" || echo "불일치: $rel"
done
cargo run -- eval tasks-real --verify 2>&1 | tail -3
```
Expected: 전 항목 `일치`, `검증 N/N 통과`

- [ ] **Step 7: 표본을 동결한다**

`docs/experiments/2026-07-20-m15-real-repo-baseline/frozen-sample.md`에 과제마다:
(레포, 이슈 URL, fix SHA, parent SHA, `check`, `protected`, **§3-4-3 지목 판정 및 원 출력**, §3-2 항해 거리 라벨, §5-4 오라클 파일 목록).

**N 확정 후 §6-4-6의 실격 대역을 절대 개수로 환산해 못박는다** (`0.98·√N`).

- [ ] **Step 8: 커밋**

```bash
cargo test && cargo clippy --all-targets -- -D warnings
git add tasks-real docs/experiments/2026-07-20-m15-real-repo-baseline/
git commit -m "feat(tasks-real): N과제 조달·3항 감사·표본 동결 (§3-2·§3-4·§6-4-4)"
```

- [ ] **Step 9: 체크포인트 — 사용자에게 보고한다**

N, 레포 분포(편중 상한 대비), 규약 6으로 떨어진 과제 수, 실격 대역의 절대값.

---

### Task 22: 스모크 · 분기 확정

**Files:**
- Create: `docs/experiments/2026-07-20-m15-real-repo-baseline/smoke.md`

**Interfaces:**
- Consumes: T1의 `마진`(=1024), T16의 `--session` 모드, T20의 `n_ctx_train`, T21의 조달된 과제
- Produces: `r_obs`와 **확정된 서버 로드 ctx** — T23 사전등록 §6-4-8이 동결한다

**Consumers:** ① T23(사전등록 조건 고정) ② T24(배치 전 게이트 4③) ③ **분기 3을 타면 일곱 소비자 전수 갱신**(아래).

**왜 스모크가 필요한가:** 여유는 `(ctx−max_out)×0.1`, 즉 **어느 컨텍스트에서든 예산의 11.1%로 구조적 상수**이고 M13 앵커(35/36)·M14 게이트(36/36)가 정확히 같은 여유에서 돌았다(그 72런에서 **오버플로 0건**). **11.1%는 신규 위험이 아니라 loco의 표준 자세다.** 그럼에도 잔여 위험은 실재한다 — 추정기는 미검증이고 실레포는 코드 밀도가 높아 `len()/4`의 오차가 더 클 수 있으며 6~10시간 배치에서 대가가 크다.

**스모크 명세** (§4-1-1):
- **대상**: **조달된 `tasks-real` 1과제.** `r`의 위험 근거가 *"실레포는 코드 밀도가 높다"*이므로 `tasks/`에서 돌리면 틀린 `r`을 잰다
- **캐시**: `serve.sh`에 차단 플래그가 없고 추가하면 핀 변경이라 비교가능성에 걸린다. 대신 **세션 첫 턴은 정의상 캐시 미적중**이므로 그 턴의 `prompt_tokens`를 완전 프롬프트 기준으로 삼아 §5-5의 의미를 확정한다
- **도달 조건**: **`pack()`이 최소 1회 발동할 때까지** 돌린다 — 예산점(25,804)에 못 닿으면 §5-3 회귀가 3배 외삽이 된다
- **서버 로드**: 관측용 **40960**(배치 조건이 아님을 기록에 명시). **스모크 후 서버를 내린다**

⚠ **스모크 대상 1과제의 지위**(7R Minor 4): 조달·스모크가 사전등록보다 앞서므로 그 과제는 §6-4-4 동결 표본에 **포함되며**, §3-2 규약 6에 걸려 탈락하면 **다른 과제로 스모크를 재수행한다**(`r_obs`를 재측정한다).

- [ ] **Step 1: 관측용 서버를 띄운다**

```bash
pkill -f llama-server || true
LOCO_MODEL_GGUF=<gguf> LOCO_CTX=40960 scripts/serve.sh > docs/experiments/2026-07-20-m15-real-repo-baseline/smoke-server.log 2>&1 &
sleep 20
grep -E "n_ctx_slot" docs/experiments/2026-07-20-m15-real-repo-baseline/smoke-server.log
```

- [ ] **Step 2: 픽스처 **사본**에서 1세션을 돌린다**

⚠⚠ **`<task_dir>/fixture` 안에서 직접 돌리면 안 된다**(1R 측정 Critical 1). 에이전트의 `cargo test`가 거기에 `target/`을 만드는데, 스모크의 도달 조건이 *"`pack()`이 최소 1회 발동할 때까지"*라 **빌드가 사실상 확정**이다. 그러면:
- §3-5의 `target/` 가드가 깨진다 — `copy_tree`는 `.gitignore`를 안 보므로 배치에서 **60런 × 최대 1GB 복사**가 되고, `fs::copy`의 mtime 보존이 H6가 닫으려는 M6 스테일 벡터를 **픽스처 안에서** 되살린다
- **T21 Step 5가 이미 통과시킨 §9-A3(차단 기준)를 사후에 무효화한다.** `target/` 부재 검사와 캐시 비운 재조달이 **둘 다 T22 이전**이라 아무도 다시 안 본다
- `.loco/sessions/`도 픽스처에 남는다

**사본에서 돌린다:**

```bash
SMOKE=$(mktemp -d)/smoke
cp -R tasks-real/<task>/fixture "$SMOKE"
# 스모크 전용 config — 전역 .loco/config.toml은 건드리지 않는다 (아래 ⚠)
mkdir -p "$SMOKE/.loco"
printf 'context_tokens = 32768\nmax_output_tokens = 4096\n' > "$SMOKE/.loco/config.toml"
cd "$SMOKE"
cargo run --manifest-path <loco>/Cargo.toml -- -p "<이슈 본문>" --auto 2>&1 | tail -20
ls -t "$SMOKE/.loco/sessions"/*.jsonl | head -1
```

⚠ **전역 `.loco/config.toml`을 고치지 말 것**(1R 측정 I3). loco의 설정은 계층형이라 **작업 디렉터리의 `./.loco/config.toml`이 나중에 이겨**, 사본 안에 두면 전역을 안 건드리고도 스모크 조건이 적용된다. 전역을 32768로 바꾸고 되돌리는 방식은 `.loco/`가 git-ignored라 `git status`에도 안 떠서 **배치까지 살아남기 쉽고**, 그러면 배치의 `effective_config.context_tokens`가 32768이 되어 **§8 각주 3이 세 번째로 거짓**이 된다(초판·개정 2에서 이미 두 번 거짓이었다).

⚠ **M15 배치 중 전역 config의 `context_tokens`는 8192다.** 32768은 `TaskSpec`(H1)으로만 들어간다 — T23 항목 8이 이 구분을 명시해야 한다.

⚠ **`pack()`이 발동하지 않으면** 다음 단계가 경고를 찍는다. Step 3의 탈출구를 볼 것.

- [ ] **Step 3: `r_obs`를 뽑는다**

```bash
python3 scripts/exp_metrics.py --session <session.jsonl>
```
Expected: `r_obs=…` 한 줄 + `pack_fired=` ≥1.

**`# WARN pack 미발동`이 나오면 — 탈출구는 순서가 정해져 있다**(1R 측정 m3). 초판은 *"Step 2로 돌아간다 / 세션을 더 길게 돌린다"*로만 적었는데, `-p` 원샷 + *"프롬프트 = 이슈 본문, 손대지 않는다"*(§3-2) 조합에서 컨트롤러가 쥔 다이얼이 사실상 "과제를 바꾼다" 하나뿐이고 **그것은 `r_obs`를 — 따라서 확정 로드값을 — 데이터 의존으로 만든다.**

**허용 순서 (위에서부터, 앞 단계가 실패해야 다음으로 간다):**

1. **같은 과제·같은 프롬프트로 `max_turns`를 올려 재실행** — 프롬프트를 안 건드리므로 자유도가 없다. 이것이 기본 탈출구다
2. **같은 과제에서 프롬프트 뒤에 `--auto` 후속 요청 없이 재시도** (시드만 다르게) — 모델의 탐색 길이가 런마다 다르므로 몇 회는 시도할 가치가 있다. **시도 횟수를 미리 정하고(권장 3회) 기록한다**
3. **다른 과제로 교체** — ⚠ **여기까지 왔다면 교체 사실과 이유를 `smoke.md`에 적고, 교체 전 과제의 `r_obs`도 함께 기록한다.** 둘 중 큰 값을 쓴다(보수적 방향). "더 편한 `r_obs`가 나올 때까지 과제를 갈아 끼우는" 경로를 이것이 막는다

⚠ 어느 경우든 **시도 전부를 기록한다** — 버린 관측이 있는데 안 적으면 그것이 자유도다.

- [ ] **Step 4: `L_req`를 계산하고 분기를 정한다**

T1이 동결한 산식 그대로:

> **`L_req` = ⌈(32768 − 4096) · 0.9 · r_obs + 4096 + 1024⌉**

⚠ **전제 확인 먼저**: `n_ctx_train ≥ 32768`인가(T20 Step 1)? **미만이면 분기 2가 공집합이 되고 분기 1과 3이 겹치며 분기 1의 "로드 32768"이 분기 3 자신의 rope 근거와 모순된다 — 미만이면 분기 3 직행.**

| # | 조건 | 처분 |
|---|---|---|
| 1 | `L_req ≤ 32768` | 로드 **32768**. 4③을 어느 형태로도 통과 |
| 2 | `32768 < L_req ≤ n_ctx_train` | 4③의 동결값 등호로 로드를 **`L_req`로 동결**(≥ 부등식이 아니라 **단일 값**) |
| 3 | `L_req > n_ctx_train` | 분기 2 불가(rope 체제가 바뀌어 모든 프롬프트 길이에서 모델 동작이 달라진다). **운용점 하향** — `ctx ≤ mo + (L−mo)/(0.9·r_obs)` |

⚠ **조건을 `L_req` 기준으로 쓰는 것이 핵심이다.** `r_obs ≤ 1.05` 같은 비율 기준으로 쓰면 `r_obs ∈ (1.05, 1.1111]` 구간이 분기 2로 가면서 **32768보다 작은 로드를 동결하라고 지시한다**(예: `r_obs=1.10` → 32,480) — §4-1의 "동결값은 실효 운용점 이상" 제약과 충돌한다. `L_req` 단일 산식이 이 틈을 구조적으로 없앤다.

⚠ **`budget·r + mo ≤ ctx`는 필요조건이지 충분조건이 아니다**(6R M4) — `session.rs:151`의 `while … && self.messages.len() > 3` 바닥 때문에 `pack()`은 예산 준수를 보장하지 않는다(시스템+user+assistant 3개가 예산을 넘으면 그대로 나간다). 25,804 예산에서 현실화 가능성은 낮으나 로드 결정이 이 부등식 하나에 걸려 있으므로 기록한다.

- [ ] **Step 5: 분기 3이면 — 소비자 일곱을 전수 갱신한다**

**분기 3을 타면 스펙과 플랜의 다음 일곱을 전부 갱신한다:**

1. **§4 제목·§4-1 본문 자신**(*"축 E — 32K 운용점 채택"*, *"파일럿과 `tasks-real`만 32K"*, *"`tasks-real` 과제만 H1로 32768을 덮는다"*)
2. §2 축 E 서술
3. §2-2 결정 2
4. §6-4-8의 `context_tokens`
5. §8 각주 4(*"단일 운용점(32K)"*)
6. §4-5의 예산점 25,804(§4-1-1 스모크 도달 조건도 이 값을 쓴다) **및 +25% 비용 추정**
7. **§6-1의 "단일 조건(32K)"과 소요 추정** — 런당 +61~68%가 §6-4-11 예산 상한의 근거이므로 운용점 하향은 **예산 항까지 움직인다**

그리고 **플랜 쪽**: T21의 `task.toml` `context_tokens`, T23 사전등록 §6-4-8·§6-4-11.

⚠ **분기 3은 사용자 보고 대상이다** — §2-2 결정 2(32K 운용점)를 포기하는 것이므로 스스로 결정하지 말 것.

- [ ] **Step 6: 서버를 내리고, 픽스처 무오염을 확인하고, 기록한다**

```bash
pkill -f llama-server
# ⚠ **삭제 전에 세션 JSONL을 건져낸다**(2R 측정 A-4). §6-4-19⑥과 §5-5가
#    "`prompt_tokens` 의미 확정 결과와 **원자료**"를 요구하는데, 개정 2는 사본
#    실행으로 바꾸면서 이 기록 항목을 안 고쳐 **삭제된 임시 디렉터리를 가리켰다.**
#    `--session` 원 출력은 집계 결과이지 원자료가 아니다
mkdir -p docs/experiments/2026-07-20-m15-real-repo-baseline/smoke
cp "$SMOKE/.loco/sessions"/*.jsonl docs/experiments/2026-07-20-m15-real-repo-baseline/smoke/
rm -rf "$SMOKE"   # 스모크 사본 폐기
# C1 재확인 — 스모크가 픽스처를 건드리지 않았음을 증명한다
find tasks-real -maxdepth 3 -name target -print     # 0건이어야 한다
find tasks-real -maxdepth 3 -name .loco -print      # 0건이어야 한다
cargo run -- eval tasks-real --verify 2>&1 | tail -3
```
Expected: 두 `find` 모두 무출력, `검증 N/N 통과`.

⚠ **하나라도 걸리면 그 과제를 재조달한다**(`scripts/procure_real.sh`가 `rm -rf "$dst"`로 시작하므로 재실행 한 줄이다) — 그리고 **재조달 후 `--verify`를 다시 통과시킨 뒤에만** 다음으로 간다.

`smoke.md`에 담을 것: 대상 과제 / **레포에 복사한 세션 JSONL 경로**(`docs/experiments/…/smoke/*.jsonl` — §6-4-19⑥의 원자료) / `--session` 원 출력 / `r_obs` / **첫 턴 `prompt_tokens`와 §5-5 의미 확정 결과** / `pack_fired` / `L_req` 계산 / `n_ctx_train` / **채택 분기와 확정 로드값** / T1 동결 커밋 해시.

⚠ **달성 슬랙 = `n_ctx_slot − ((ctx−mo)·0.9·r_obs + mo)`는 사후 기록**이며 `마진`(입력항)과 **구분해 적는다**(7R I3).

- [ ] **Step 7: 커밋 + 체크포인트**

```bash
git add docs/experiments/2026-07-20-m15-real-repo-baseline/
git commit -m "docs(m15): 배치 전 스모크 — r_obs 측정과 서버 로드 분기 확정 (§4-1-1)"
```

사용자에게 `r_obs`·`L_req`·채택 분기를 보고한다.

---

### Task 23: 사전등록 — **사용자 승인 게이트에서 정지**

**Files:**
- Create: `docs/experiments/2026-07-20-m15-real-repo-baseline/pre-registration.md`

**Interfaces:**
- Consumes: T1·T20·T21·T22의 산출물 전부
- Produces: 승인된 사전등록 — **PROTOCOL 1이 요구하는 배치 전제조건**

**Consumers:** ① T24(배치 수행) ② T25(리포트의 판정 규칙) ③ `.claude/agents/loco-experiment-runner.md`(러너는 **사전등록 없는 배치를 수행하지 않는다**).

⚠ **PROTOCOL 1: 사전등록 없이는 배치를 돌리지 않는다.** 사전등록 = 가설·조건·표본·지표·판정 규칙·중단 규칙·시간 예산이 담긴 문서가 **사용자 승인을 받은 상태.**

⚠ **가설·판정 임계값은 "해당 없음"으로 명시한다**(효과 비교 부재). `tasks-real`은 신설 트랙이라 대조군이 없고 **M15에는 효과 입증 실험이 없다** — 첫 배치의 통과율은 판정이 아니라 베이스라인이다.

- [ ] **Step 1: §6-4의 19항목을 전부 채운다**

| # | 항목 | 출처 |
|---|---|---|
| 1 | 공급량 실사 결과 (전 이력) | T20 |
| 2 | 최소 표본(16)과 미달 처분 — **확정 커밋 해시 포함** | T1 |
| 3 | 레포 편중 상한(60%) | T1·T20 |
| 4 | 표본 동결 (과제별 좌표 + 지목 판정 **및 원 출력** + 항해 거리 라벨 + 오라클 목록) | T21 |
| 5 | 주 지표 — `passed` 1차, `passed_strict` 사전 지정 보조. **둘의 방향이 갈리면 리포트 헤드라인에 적는다**(M9가 이미 그 갈림을 냈다) | — |
| 6 | 실격 대역 — `0.98·√N` **절대 개수로 환산** | T1·T21 |
| 7 | 통과율 분석 계획 — 과제 수준 통과 비율의 평균, **과제 단위 복원추출 부트스트랩(재추출 횟수 명시)**, 95% CI. **"런 수준 구간은 어떤 형태로도 보고하지 않는다" 공약** | T15 |
| 8 | 조건 고정 (아래 상세) | T21·T22 |
| 9 | 환경 조건 캡처 — 배치 시작 시 `env \| grep -E '^CARGO'`. ⚠ `exec.rs`가 `.env()`를 안 불러 부모 환경을 상속하므로 `CARGO_NET_OFFLINE`·`CARGO_HOME`이 양쪽에 걸리는데 어디에도 안 남는다 | — |
| 10 | 자증 절차 — H9의 `RunRecord` 실효값 + 과제별 `task.toml`·`procure.toml` 해시 | T6·T8 |
| 11 | GPU 시간 예산 상한과 초과 시 처분. ⚠ **총 런 수(= 과제 수 × 반복 수)의 상한을 함께 못박는다** — §6-1이 반복 상향을 허용했으므로 예산이 60런에 고정돼 있지 않다(N=16·반복 4면 64런으로 "최악 10시간"을 넘는다) | — |
| 12 | 로그 캡처 경로 + **데몬화** (`PROTOCOL.md:24` 4④ 레시피) | — |
| 13 | 중단 규칙과 "배치 사망" 정의 (M13 §5 관례 승계) | — |
| 14 | 재측정 횟수 사전 공약 (M14 관례 승계) | — |
| 15 | 항해 거리 라벨 상한(80%) — ⚠ **하드 제약이 아니라 보고 의무다.** 이 축은 이슈 본문이 정하므로 선정 시점에 통제 불가이고 라벨은 이미 기술 통계 전용이다. **초과하면 리포트 헤드라인에 적고 베이스라인의 적용 범위 제한으로 명시.** 부분군 분석·층화 판정에 쓰지 않는다는 공약 함께 | T21 |
| 16 | §3-4의 3항 감사 판정 — 과제별, **원 출력과 추출 스크립트 포함** | T21 |
| 17 | PROTOCOL 4①·4③·항목 5 갱신 — **적용 시점 M15 이후 표시** | T19 |
| 18 | TMPDIR 여유 — 배치 전후 `ls ${TMPDIR}/loco-eval-*`. 빌드 후 `target/`이 zoxide 371M / fd 255M / ripgrep 459M / **just 998M**이고 `Sandbox::cleanup`은 best-effort다 | — |
| 19 | 축 C·§5-4 지표의 분석 계획 (아래 상세) | T15·T16 |

**항목 8 상세** — 전부 값을 적는다: **`context_tokens` — 전역과 과제별을 나눠 적는다**(전역 = **8192**, `tasks-real` 과제별 = 32768 또는 T22 분기 3의 하향값. §8 각주 3이 두 번 거짓이었던 지점이고 1R 측정 I3이 세 번째 위험을 지적했다) / **`max_output_tokens`(값과 근거)** / `max_turns` / `command_timeout_secs` / `check_timeout_secs` / **`timeout_secs`(과제별 — llama.cpp 앵커 `20260719T082030Z`의 런당 시간에 붙여 정한다)** / **`timeout_scale`** / **`base_seed`와 하위 배치별 `--seed`** / **`--repeats`(값과 근거 — 아래 ⚠)** / **서버 로드 ctx(T22 분기 결과)·`마진` 값과 확정 커밋 해시** / **달성 슬랙(사후 기록, `마진`과 구분)** / **부트스트랩 `--resamples`·`--seed`** / 모델·양자화.

⚠ **`--repeats`의 근거는 두 문장이 필요하다**(1R 측정 m1 — 초판은 §6-4-8 원문만 옮기고 §6-1의 판단 규칙을 빠뜨렸다):

1. §6-4-8: `base_seed+repeat` 규약상 **반복 수는 시드 집합도 바꾼다**
2. **§6-1**: 과제가 20 미만으로 확정되면 **통과율에 대해서는** 남는 예산을 반복 수로 돌리지 않는다(§6-2의 재추출 단위 선언상 반복 증가는 과제 수준 정밀도를 사 주지 않는다). ⚠ **다만 §5-4 지표는 다르다** — §6-4-19①의 층별 분모 하에서 3/3 통과 과제는 실패 층이 비어 항해 지표에서 **통째로 제외**되므로, 반복을 늘리면 제외 셀이 줄고 층내 해상도가 올라간다. 따라서 **반복 상향 여부는 §6-4-19의 제외 셀 예상과 함께 여기서 판단한다** — 근거 없이 금지만 남기지 않는다.

**즉 사전등록은 "예상 제외 셀 수"를 적고 그것을 근거로 `--repeats`를 정해야 한다.** T21의 N과 난이도 분포에서 추정한다.

**항목 19 상세** — ① 항해/수선의 **층별 분모**와 제외 규칙 ② 교집합 `≠ ∅` ③ **층화 비합산 공약** ④ **부트스트랩 재추출 단위 = 과제, 제외 후 남은 집합에서 재추출** ⑤ §5-3 절편/기울기 추정 방법(턴 단위 최소자승, `inline_system` 층화) ⑥ **§5-5 `prompt_tokens` 의미 확정 결과와 원자료**(T22의 첫 턴 기준).

- [ ] **Step 2: 배치 분할을 등록한다**

⚠ **`run_eval`은 `report.json`을 루프가 전부 끝난 뒤 단 한 번** 쓴다(`mod.rs:147-149`). **LLM 에러 1건이 하네스 전체를 죽이고**(`mod.rs:211-215` → `loop_result?`) **report.json이 안 써진다.** `chat` 재시도는 3회 200/400ms 백오프뿐이라 1초 넘는 히컵에 배치가 증발한다.

**따라서 `--filter`로 4~5개 하위 배치로 쪼개 등록한다.** 분할 자체는 코드 무변경으로 가능하다(`filter_tasks`는 정확 일치·반복 가능·불일치 시 `bail!`이고 호출마다 별도 스탬프+report.json이 생긴다). **집계는 T15의 `--pool`이 맡는다.**

각 하위 배치의 `--filter` 목록과 `--seed`를 명시한다.

- [ ] **Step 3: 데몬화 명령을 등록한다**

⚠ **하네스 백그라운드 60분 수명 상한** — 이 배치는 그 **6~10배**다. macOS에 `setsid`가 없으므로:

```bash
python3 -c "import os,sys; os.setsid(); os.execvp(sys.argv[1], sys.argv[1:])" \
  cargo run -- eval tasks-real --repeats 3 --filter <...> > <로그 경로> 2>&1 &
```

- [ ] **Step 4: 사용자 승인을 요청하고 정지한다**

**⛔ 이 태스크는 여기서 멈춘다.** 사용자 승인 없이 T24로 넘어가지 말 것 — PROTOCOL 1 위반이고 GPU 시간이 무효가 된다.

```bash
git add docs/experiments/2026-07-20-m15-real-repo-baseline/pre-registration.md
git commit -m "docs(m15): 베이스라인 배치 사전등록 — 승인 대기 (PROTOCOL 1)"
```

---

### Task 24: 배치 수행 · 지표 추출

**Files:**
- Create: `.loco/eval/<stamp>/` × 4~5 (git-ignored), `docs/experiments/2026-07-20-m15-real-repo-baseline/metrics/`

**Interfaces:**
- Consumes: T23의 **승인된** 사전등록
- Produces: 원 지표 — T25의 리포트가 인용한다

**Consumers:** ① T25(리포트·판정) ② `docs/baselines.md`의 M15 절.

⚠ **러너 위임 가능**: `.claude/agents/loco-experiment-runner.md`가 이 형태의 무인 수행을 위해 존재한다. **사전등록 없이는 수행하지 않는다**는 것이 그 에이전트의 계약이다.

- [ ] **Step 1: 배치 전 게이트** (PROTOCOL 4, T19가 개정한 형태)

```bash
# ⓪ 픽스처 무오염 (M15 신규 — C1 대응). A3는 T21에서 통과했지만 그 사이 T22 스모크가
#    돌았다. target/ 하나가 60런 × 최대 1GB 복사 + 스테일 mtime 벡터를 만든다
find tasks-real -maxdepth 3 -name target -print     # 0건이어야 한다
find tasks-real -maxdepth 3 -name .loco -print      # 0건이어야 한다
# 전역 config가 배치 조건인지 — ⚠ M15에서 전역 context_tokens는 **8192**다.
#    32768은 TaskSpec(H1)으로만 들어간다 (§8 각주 3)
grep -n "context_tokens" .loco/config.toml 2>/dev/null || echo "(전역 오버라이드 없음 = 코드 기본 8192)"
# ① 세 트리 --verify
cargo run -- eval tasks --verify 2>&1 | tail -2
cargo run -- eval tasks-large --verify 2>&1 | tail -2
cargo run -- eval tasks-real --verify 2>&1 | tail -2
# ② 서버 기동 (로그 리다이렉션 포함, 이전 서버는 먼저 내린다)
pkill -f llama-server || true
LOCO_MODEL_GGUF=<gguf> LOCO_CTX=<T22 확정 로드> scripts/serve.sh > <로그 경로> 2>&1 &
# ③ 배치 전 스모크 — 전건 통과해야 시작한다
#    - json_schema 요청 1건 HTTP 200 (PROTOCOL.md:27-38의 curl 그대로)
#    - n_ctx_slot == **사전등록에 동결된 서버 로드 ctx** (T19의 개정 형태)
#    - curl /v1/models 의 data[0].id == --alias 값
#    - .loco/config.toml이 이번 배치 조건인지 (직전 배치 잔재는 GPU 시간 전체를 무효화)
#    - ls ${TMPDIR}/.cargo — 존재하면 수동 제거
#    - ls ${TMPDIR}/loco-eval-* — 항목 18의 배치 전 캡처
# ④ 환경 조건 캡처 (항목 9)
env | grep -E '^CARGO' | tee docs/experiments/2026-07-20-m15-real-repo-baseline/metrics/env-cargo.txt
```

⚠ **④의 통과 로그를 배치 산출물에 첨부한다**(§9-A4, 7R Minor 1) — **게이트 형태를 M15가 직접 개정했으므로 그 게이트가 실제로 돌았다는 증거가 필요하다.**

- [ ] **Step 2: 하위 배치를 순차 수행한다** (데몬화)

⚠ **측정 중 `cargo build`/`test` 병행 금지**(PROTOCOL 2). ⚠ **하위 배치는 순차다** — 병행하면 CPU 경합으로 타이밍 판정이 흔들린다.

```bash
python3 -c "import os,sys; os.setsid(); os.execvp(sys.argv[1], sys.argv[1:])" \
  cargo run -- eval tasks-real --repeats <R> --seed <S> --filter <...> \
  > docs/experiments/2026-07-20-m15-real-repo-baseline/metrics/batch-1.log 2>&1 &
```

- [ ] **Step 3: 지표를 추출한다**

```bash
python3 scripts/exp_metrics.py --selftest \
  | tee docs/experiments/2026-07-20-m15-real-repo-baseline/metrics/selftest.txt
python3 scripts/exp_metrics.py --pool .loco/eval/<stamp1> .loco/eval/<stamp2> ... \
  | tee docs/experiments/2026-07-20-m15-real-repo-baseline/metrics/pooled.txt
```

⚠ **`--selftest` 출력을 배치 산출물에 첨부한다**(§6-3) — 마커 문자열은 Rust 상수에서 손으로 복사한 것이라 드리프트 위험이 있다.

- [ ] **Step 4: 마커 계수를 기회 분모와 함께 낸다** (§6-3·§9-B1)

| 장치 | 분모 |
|---|---|
| 파이프 장치 | 파이프 포함 `run_command` 호출 수 (M14 방식 전수 추출) |
| FINISH_NUDGE | 무장 조건 충족 런 수 |
| A-3 | 성공한 `edit_file`/`write_file` 수 → **절단률** |

⚠ **0회도 답이다** — 다만 **기회 분모와 함께 볼 때만** 답이다. A-3의 **효과는 이 설계로 측정할 수 없다**(§1-2 답 1) — 새로 얻는 것은 절단률뿐이다.

- [ ] **Step 5: 배치 후 확인**

```bash
ls ${TMPDIR}/loco-eval-* 2>/dev/null | head   # 항목 18 — Sandbox::cleanup은 best-effort
grep -c '"effective_context_tokens"' .loco/eval/<stamp>/report.json   # §9-A4 자증
python3 -c "import json;r=json.load(open('.loco/eval/<stamp>/report.json'));print(r['schema_fallback_count'])"
```
⚠ `schema_fallback_count` > 0이면 그 런들은 **스키마 강제 없이 돈 것이라 측정값으로 신뢰할 수 없다.**

- [ ] **Step 6: 커밋 (report.json은 git-ignored — 지표 산출물만)**

```bash
git add docs/experiments/2026-07-20-m15-real-repo-baseline/metrics/
git commit -m "docs(m15): 베이스라인 배치 원 지표 (스탬프 <...>)"
```

---

### Task 25: 유인 3건 · 리포트 · 독립 리뷰 · 병합

**Files:**
- Create: `docs/experiments/2026-07-20-m15-real-repo-baseline/report.md`
- Modify: `docs/baselines.md` (M15 절), `CLAUDE.md` (M15 결과 반영), `docs/m16-candidates.md`

**Interfaces:**
- Consumes: T24의 지표
- Produces: M15 판정 + M16 인수인계

**Consumers:** ① `docs/baselines.md`(M16의 대조군 인용처) ② `CLAUDE.md` ③ M16 설계.

- [ ] **Step 1: 유인 3건을 수행한다** (§7)

**격리 규약 5항:**
① `tasks-real` 후보 목록과 **배타**로 사전등록 ② **배치 완주 이후에 수행**(동시 실행은 `cargo test` CPU 경합으로 **PROTOCOL 2** 위반) ③ **관측 스키마를 착수 전 고정**, 사후 범주 추가 시 시점과 사유 기록 ④ **커밋/되돌림 정책을 사전에** ⑤ **실패 테스트가 없는 이슈로 고른다** — 그것이 §1-1이 승계로 미룬 축이다.

```bash
LOCO_BIN=<loco> PILOT_LEDGER=<레포 밖 경로> scripts/pilot.sh
```
⚠ **원장은 레포 밖에 둔다** — 사용자 코드의 diff를 담는다.

**한계 4항 — 리포트에 그대로 싣는다:**
1. **비율로 인용하지 않는다** — 사건 목록과 서술로만. M13은 *"통계가 아니다"*라고 말해서가 아니라 **작은 n의 비율을 인용해서** 집계가 붕괴했다(6/19 → 감사 후 4)
2. **8K 대조가 없으므로 어떤 관측도 컨텍스트에 귀속하지 않는다**
3. **비맹검·도구 제작자 자기 평가·`check` 부재**라는 M13의 한계를 그대로 상속한다 — §3-1 표의 개선은 **무인 배치에만** 적용된다
4. 말할 수 있는 것은 **"그 축에서 관측된 실패 양상의 목록"**뿐. 산출은 **M16의 관측 스키마 입력**이며 **표본 수 산정에 쓰지 않는다**

- [ ] **Step 2: 리포트를 쓴다**

**§9의 A/B를 나눠 적는다** — A는 반증 가능한 판정 기준, B는 인도물 확인이다. 10개를 한 목록으로 두면 전부 검정인 것처럼 읽힌다.

| # | 기준 | 차단 | 결과 |
|---|---|---|---|
| A1a | 동결된 전 과제가 `eval --verify` 통과 | 예 | |
| A1b | 최소 표본(16) **및** 편중 상한(60%) 둘 다 만족 | **아니오** | 미달이면 §6-4-2 처분 (iii): 실패로 보고하고 M16 대조군으로 인용하지 않되 병합은 막지 않는다 |
| A2 | **캐시를 비운 재조달** 후 `--verify` 통과 + 매니페스트 일치 | 예 | |
| A3 | 스테일 뮤테이션·실행 비트 테스트 통과 + `<task_dir>/fixture`에 `target/` 없음 | 예 | |
| A4 | 배치 완주 + H9 실효값 런별 기록 + **개정된 4③의 통과 로그 첨부** | 예 | |
| A5 | 결과가 §6-4-6 실격 대역 **밖** | **아니오** | 대역 안이면 "베이스라인 확보 실패"로 보고 |
| A6 | 축 C 일곱 항목이 `--selftest` 포함 동작 + **추정기 오차가 §5-3·§6-4-19대로 보고**(오차가 크든 작든 결과다) | 예 | |
| A7 | 기존 게이트 전건: `cargo test`, `clippy --all-targets -- -D warnings`, 세 트리 `--verify` | 예 | |
| B1 | M14 파이프 마커 발동 여부가 **기회 분모와 함께** (0회도 답) | — | |
| B2 | 유인 3건이 원장에 기록되고 루브릭이 못 본 것이 명시됨 | — | |
| B3 | **측정 리포트가 병합 전 독립 리뷰를 1회 이상** | — | Step 4 |

**비교가능성 각주를 반드시 싣는다** (§8):
1. **M13 파일럿과 비교 불가** — 채점자(기계 `check` vs 사용자 판정)·과제 출처·반복 수(3 vs 1)·픽스처 구성
2. **`tasks-real`은 신설 트랙이라 M13·M14 어느 수치와도 비교되지 않는다.** 단 **마커 계수는 기회 분모를 갖춘 경우 M14와 대조 가능**
3. **`tasks/`·`tasks-large/` 앵커는 8K로 불변** — 운용점은 H1로 먹이므로 `.loco/config.toml`을 건드리지 않는다. **초판의 "경로에서만 지정한다"도, 개정 2의 "`effective_config`로 자증한다"도 거짓이었다 — H9가 고쳤다**
4. **단일 모델(ornith-1.0-9b Q4_K_M)·단일 양자화·단일 운용점.** M13 한계 중 M15가 푼 것은 없다
5. M12 `sr_error`·M13 T7 `verify_*`·M14 `verify_allpass` 각주는 그대로 유효
6. **분기 2를 탔으면 `n_ctx_slot ≠ context_tokens`** — M13 이후 처음 나오는 형태다. 로드값과 운용값을 나란히 적고 `n_ctx_slot`이 운용점이 아니라 **동결 로드**를 증언한다는 것을 명시

**남은 편향을 명시한다**(§10-4): **컨트롤러가 과제를 고른다.** 전제는 외부인이 썼지만 *어느 이슈를 고를지*는 컨트롤러 판단이고 **§3-3의 테스트 백포트에서 컨트롤러 저작이 다시 들어온다.** 완화책이지 제거책이 아니다.

- [ ] **Step 3: `docs/baselines.md`에 M15 절을 더한다**

M15 수치와 스탬프, 그리고 위 각주 전부. **`exp_metrics.py`의 신규 컬럼 캐비엇도** M12 `sr_error`·M14 `verify_allpass` 각주와 같은 자리에 적는다.

- [ ] **Step 4: 독립 리뷰** (§9-B3)

M13의 마지막 한계(*"컨트롤러가 자기 분석을 자기가 검증할 때 무엇이 새는지"*)에 대응한다. 리포트를 **병합 전에** 독립 리뷰에 붙인다.

⚠ 리뷰 프롬프트에 **"0건이면 0건으로 보고하는 것이 정상적 종결"**을 반드시 명시할 것 — 없으면 라운드를 늘리려 발견을 만들어낸다.

⚠ **러너/리뷰어 보고를 그대로 옮기지 말 것** — `report.json`을 직접 대조한다(M12 실측 사례: 러너 보고를 옮겼더니 서사가 뒤집혔다).

- [ ] **Step 5: 병합**

```bash
cargo test && cargo clippy --all-targets -- -D warnings
cargo run -- eval tasks --verify 2>&1 | tail -2
cargo run -- eval tasks-large --verify 2>&1 | tail -2
cargo run -- eval tasks-real --verify 2>&1 | tail -2
git checkout main && git merge --no-ff m15/real-repo-track
```

⚠ **최종 판정·병합 결정은 사용자 리뷰를 거친다**(PROTOCOL 7). **푸시는 사용자가 지시할 때만.**

- [ ] **Step 6: M16 인수인계**

`docs/m16-candidates.md`에 담을 것:
- **M13의 0/7 축**(테스트도 없이 "이걸 해줘"만 주어지는 형태) — §1-1이 승계로 미룬 것. **필요 표본 수는 M16이 자기 사전등록에서 산정한다**(n=3에서 비율을 뽑아 표본 수 공식에 넣는 것은 §7 한계 1의 우회라 **금지**)
- 유인 3건이 낸 **실패 양상 목록** = M16이 사전에 범주로 잡아 둘 사건 유형
- 후보 A·B·D·질문 B(`docs/m15-candidates.md`)
- **베이스라인이 실격 대역에 들어갔으면 그 사실** — M16 대조군으로 못 쓴다

---

## Self-Review 결과 (개정 3)

**1. 2R 지적 반영 대조 — Critical 3 · Important 8 · Minor 13 전건:**

| 축 | 지적 | 반영 |
|---|---|---|
| 실현 C1 / 측정 C1 | T14가 `tok`에서 T15의 이름 넷 참조 → 단독 `NameError` | T14 Step 3에 선언 이동, T15는 채우기만 |
| 실현 C2 / 측정 I1 | `.files`가 자기를 셈 (2파일→3파일) | **T17 구조 변경** — `<sha>/tree/` + `<sha>/meta/` 분리로 이름 필터 자체를 없앰 |
| 실현 C3 | `tar --exclude`가 basename 매칭 → 동명 파일 소실 | 같은 구조 변경 — **제외할 것이 없어졌다**. 컨트롤러가 tar 동작 직접 실측 |
| 실현 I7 | export-ignore 오탐(루트 `manifest.tsv`) | 〃 (셋이 한 뿌리였다) |
| 실현 I4 / 측정 A-3 | H17 세 번째 사이트 = `tasks-real`이 타는 경로 | T2에 `sync_protected` 파일 분기 + 단일 파일 테스트 |
| 실현 I5 / 측정 m7 | T15 Step 4에 `assert` 0줄 | 실제 단언 11건 (축 C ⑥ 포함) |
| 실현 I6 | `continue` 3개가 미고정 | T15 Step 4b |
| 측정 I8 | 축 C ⑥ `--selftest` 단언 0건 → A6이 반증 불가 | T15 Step 4의 `touch_ev` 블록 |
| 측정 A-4 | T22가 §6-4-19⑥ 원자료를 삭제 | `rm -rf` 전 세션 JSONL을 레포로 복사 |
| 측정 A-5 | `grep` 근거 미정정 2곳 | T13 테스트 주석·단언 메시지 |
| 측정 A-6 | T6이 `agent/mod.rs`를 고치는데 Files·`git add`·구조표에 없음 | 세 곳 갱신 |
| 측정 A-8 / I6 | CLAUDE.md가 신규 마커·컬럼 0건 열거 | T19 Step 4b (마커 1 + 컬럼 16 전수) |
| 측정 I7 | `armed_runs`의 죽은 논리곱 + 근사 표시 없음 | 항 제거 + `APPROX` 라벨 |
| 측정 m2 | M13/M14 오귀속 (실제는 스펙 초판·개정 2) | T6 주석 |
| 측정 m3 | `COLS`만 늘리고 행 조립 미제시 | T15 Step 2 |
| 측정 m4 | 매니페스트가 심링크 집합을 못 봄 | §9-A2 대조에 `symlinks.txt` 추가 |
| 실현 m1 | `row["max_prompt"]`는 리스트 첨자 | `row[col[...]]` |
| 실현 m6 / 측정 m6 | `process()`가 여섯 번째 언팩 지점 | T14 Step 3b에 명시 |
| 실현 m6(T2) | "두 테스트"가 실제로는 셋 | Step 2·4 개수 정정 |

**2. 2R이 "확인했고 문제없던 것"으로 통과시킨 것** — 개정 3이 깨지 않았는지 확인이 필요한 항목: Consumers 25/25 · §6-4 19/19 · 항목 8 13/13 · §9 11/11 · §8 각주 6/6 · H 19/19 · `run_metrics` 인덱스 접근 15곳(최대 `[11]` < 12) · `COLS` 위치 의존 소비자 0건 · T14 셀프테스트 산술 6/6 · `disqualification()` = §6-4-6.

⚠ **개정 3이 `tok`을 12번 인덱스에 두는 것은 여전히 안전하다**(기존 최대 인덱스 `[11]`). T15 Step 4의 `run_metrics(touch_ev)[12]`가 그 계약을 쓴다.

**3. 개정 3이 새로 만들 수 있는 것 (3R이 볼 곳)**

정직하게 적는다 — 2R의 교훈이 "고치는 자리에서 새로 만든다"이므로:

- **T17의 `tree/`+`meta/` 분리는 구조 변경이다.** 소비자를 전수로 셌다고 믿지만(캐시 히트 마커·픽스처 실체화 `src`·A2 재조달 대조·Step 5 검증 명령), **`git worktree`처럼 경로를 가정하는 다른 자리가 남았을 수 있다**
- **T2의 세 번째 사이트 수정이 `meta` 변수명을 재사용한다**(`let meta = std::fs::symlink_metadata(&src)?;`) — 그 분기에 동명 변수가 이미 있는지 확인되지 않았다
- **T15 Step 4의 단언 문자열**(`"nav_hit[fail] tasks=1 excluded=1"`)은 합성 데이터에 맞춰 실제 값을 넣어야 한다. 형태만 제시했다
- **`--session`이 `run_metrics(...)[12]`를 쓰는데 T16이 여전히 13원소 언팩 형태로 적혀 있는지** 재확인 필요
