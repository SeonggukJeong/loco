# M7 — 모델 세트 재편·속도 지표 마감·판정 무결성 부채 구현 플랜

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 스펙 `docs/superpowers/specs/2026-07-16-m7-set-reorg-integrity-design.md`(M7)의 구현 — report 최상위 평균 s/런(A3), cargo config 스냅샷 감지(C1), 테스트 보강 2건(C2), 모델 세트 재편·문서 정리(A1·C4).

**Architecture:** 판정기(tasks/)·스캐폴딩(src/agent·src/tools 프로덕션)은 **불변**이고, 코드 변경은 src/eval(하네스)에만 들어간다. C1은 새 파일 `src/eval/integrity.rs`(변화-감지 스냅샷, 존재-중단인 기존 트립와이어와 별개 메커니즘)로 격리하고, A3는 report.rs에 순수 추가한다. A1·C4는 문서 3종 편집이다.

**Tech Stack:** Rust edition 2024 (toolchain 1.97), 표준 라이브러리만 (신규 크레이트 금지 — tempfile은 기존 dev-dependency로 테스트에서만 사용).

## Global Constraints

- 의존성 추가 절대 금지 (스펙 고정 목록) — `std`만으로 구현, 테스트는 기존 dev-dep `tempfile` 사용 가능
- 게이트: `cargo test` 전부 통과 + `cargo clippy --all-targets -- -D warnings` 무경고 — **매 태스크 종료마다**
- 사용자 대면 메시지(stderr 알림·에러)는 한국어, 식별자·코드는 영어; CLAUDE.md는 **영문 유지**
- `src/agent`·`src/tools`는 `#[cfg(test)]` 모듈 내부만 변경 가능(C2), 프로덕션 경로 diff 0; `tasks/` 완전 불변
- report.json은 기존 키 이름·의미 불변 — 신규 키 추가만 (M6 §5 전례)
- 커밋: conventional commits, 제목 한국어 가능
- 기준 커밋: M7 시작 시점 = `6ab4f3b` (최종 게이트의 diff 검사 기준)
- 재측정·LLM 필요 작업 없음 — 전 태스크가 서버 없이 진행 가능

---

### Task 1: A3 — report 최상위 평균 s/런 (런 가중)

**Files:**
- Modify: `src/eval/report.rs` (Report 구조체·render_table·tests)
- Modify: `src/eval/mod.rs:111-133` (Report 생성부)

**Interfaces:**
- Consumes: 기존 `RunRecord.duration_secs`, `TaskReport.runs`, `Report::total_of` 패턴
- Produces: `Report.avg_duration_secs: f64` 필드, `Report::avg_duration_of(tasks: &[TaskReport]) -> f64` — 이후 태스크가 소비하지 않음(독립)

- [ ] **Step 1: 실패하는 테스트 작성**

`src/eval/report.rs`의 `#[cfg(test)] mod tests`에 추가 (기존 헬퍼 `run(passed, turns, secs)` 사용, `report.rs:148`):

```rust
    #[test]
    fn top_level_avg_duration_is_run_weighted() {
        // 런 가중 — 과제별 평균의 평균이 아님 (M7 §4): (10+20+60)/3 = 30, 평균의 평균이면 37.5
        let a = TaskReport::from_runs("a".into(), vec![run(true, 1, 10.0), run(true, 1, 20.0)]);
        let b = TaskReport::from_runs("b".into(), vec![run(false, 1, 60.0)]);
        assert_eq!(Report::avg_duration_of(&[a, b]), 30.0);
        assert_eq!(Report::avg_duration_of(&[]), 0.0, "빈 목록은 0 (0나눗셈 금지)");
    }

    #[test]
    fn table_shows_avg_duration_per_run() {
        // sample_report는 38.5s 런 1개 — 요약 라인에 평균 s/런 노출 (M7 §4)
        let table = sample_report().render_table();
        assert!(table.contains("평균 38.5s/런"), "{table}");
    }
```

그리고 기존 `report_json_has_design_schema_fields`(`report.rs:259`)의 키 배열에 `"avg_duration_secs"`를 추가:

```rust
        for key in ["model", "base_seed", "repeats", "timeout_scale", "started_at", "duration_secs", "interrupted", "tasks", "total_pass_rate", "effective_config", "avg_duration_secs"] {
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib eval::report`
Expected: FAIL — `avg_duration_of` 미정의 컴파일 에러 (E0599)

- [ ] **Step 3: 최소 구현**

`src/eval/report.rs`:

(a) `Report` 구조체(`:79-94`)에 필드 추가 — `false_finish_count` 다음 줄. 기존 `duration_secs`(`:85`)에도 의미 구분 주석을 단다:

```rust
    /// 하네스 벽시계 총합 — check 실행·샌드박스 준비 오버헤드 포함 (M7 §4 의미 구분)
    pub duration_secs: f64,
```

```rust
    /// 런당 에이전트 실행 시간의 **런 가중** 평균 — per-run `duration_secs`(agent.run만
    /// 계측, check 제외) 정의 승계라 벽시계 `duration_secs`/총런수와 일치하지 않는다 (M7 §4)
    pub avg_duration_secs: f64,
```

(b) `RunRecord.duration_secs`(`:38`)에 주석:

```rust
    /// 에이전트 실행 시간(agent.run)만 — 판정 check·샌드박스 준비 제외 (M7 §4)
    pub duration_secs: f64,
```

(c) `impl Report`에 `total_of` 바로 아래 추가:

```rust
    /// 런 가중 평균 s/런 — total_of와 같은 정의 철학 (반복 수가 달라도 왜곡 없음, M7 §4)
    pub fn avg_duration_of(tasks: &[TaskReport]) -> f64 {
        let total: usize = tasks.iter().map(|t| t.runs.len()).sum();
        if total == 0 {
            return 0.0;
        }
        let sum: f64 = tasks.iter().flat_map(|t| t.runs.iter().map(|r| r.duration_secs)).sum();
        sum / total as f64
    }
```

(d) `render_table`(`:129-139`)의 요약 라인에 평균 추가 — 거짓 성공 finish 뒤, 시드 앞:

```rust
        out.push_str(&format!(
            "전체 통과율 {:.1}% ({}/{total}) · 엄격 {:.1}% ({}/{total}) · 거짓 성공 finish {} · 평균 {:.1}s/런 (시드 {}부터, timeout×{}){}\n",
            self.total_pass_rate * 100.0,
            self.passed_count,
            strict_rate * 100.0,
            self.passed_strict_count,
            self.false_finish_count,
            self.avg_duration_secs,
            self.base_seed,
            self.timeout_scale,
            if self.interrupted { " — 중단됨(부분 결과)" } else { "" }
        ));
```

(e) 테스트 헬퍼 `sample_report()`(`:231-256`)에 필드 추가 — `total_pass_rate` 계산 줄 방식 답습:

```rust
            total_pass_rate: Report::total_of(&tasks),
            avg_duration_secs: Report::avg_duration_of(&tasks),
```

(f) `src/eval/mod.rs`의 `Report` 생성부(`:111-133`) — `false_finish_count` 줄 다음에 추가 (`tasks: task_reports`가 소유권을 가져가기 **전**이어야 함):

```rust
        avg_duration_secs: Report::avg_duration_of(&task_reports),
```

- [ ] **Step 4: 통과 확인**

Run: `cargo test --lib eval` 후 `cargo test`
Expected: 전부 PASS (신규 2 + 기존 전건)

- [ ] **Step 5: clippy + 커밋**

```bash
cargo clippy --all-targets -- -D warnings
git add src/eval/report.rs src/eval/mod.rs
git commit -m "feat(eval): report 최상위 평균 s/런 — 런 가중 집계·요약 라인 (M7 §4)"
```

---

### Task 2: C1 — cargo config 변조 스냅샷 감지

**Files:**
- Create: `src/eval/integrity.rs`
- Modify: `src/eval/mod.rs` (모듈 선언, `run_eval` 스냅샷 채취, `run_once`/`judge` 파라미터 배선, 트립와이어 주석)

**Interfaces:**
- Consumes: 없음 (std만)
- Produces: `integrity::resolve_cargo_home() -> Option<PathBuf>`, `integrity::CargoConfigSnapshot::take(cargo_home: Option<&Path>, temp_dir: &Path) -> CargoConfigSnapshot`, `CargoConfigSnapshot::verify_unchanged(&self) -> anyhow::Result<()>` — 같은 태스크의 배선만 소비

- [ ] **Step 1: 모듈 작성 (단위 테스트 동거)**

`src/eval/integrity.rs` 생성 (테스트 포함 전문):

```rust
//! 판정 무결성 — 샌드박스 밖 cargo config 변조 감지 (M7 스펙 §5).
//! 하네스 시작 시 감시 대상 파일 상태를 스냅샷하고 매 런 check 직전에 비교한다.
//! 존재-중단(cargo_tripwire)이 아니라 **변화-감지** — 사전 존재하는 정당 config는
//! 수용하고, 측정 중의 상태 전이만 변조로 본다. 정리는 하지 않는다.

use std::path::{Path, PathBuf};

/// 파일 상태 3종 — 상태 전이 일체(내용↔부재↔읽기불가)가 변조다 (M7 §5)
#[derive(Debug, Clone, PartialEq)]
enum FileState {
    Absent,
    Unreadable,
    Content(Vec<u8>),
}

fn state_of(path: &Path) -> FileState {
    match std::fs::read(path) {
        Ok(bytes) => FileState::Content(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => FileState::Absent,
        Err(_) => FileState::Unreadable,
    }
}

/// env CARGO_HOME → 없으면 홈 밑 `.cargo`. 둘 다 불가면 None (호출부가 감시 생략을 알림)
pub fn resolve_cargo_home() -> Option<PathBuf> {
    if let Some(v) = std::env::var_os("CARGO_HOME") {
        if !v.is_empty() {
            return Some(PathBuf::from(v));
        }
    }
    std::env::home_dir().map(|h| h.join(".cargo"))
}

#[derive(Debug)]
pub struct CargoConfigSnapshot {
    entries: Vec<(PathBuf, FileState)>,
}

impl CargoConfigSnapshot {
    /// 감시 대상: ① cargo_home의 config.toml·config(레거시명), ② temp_dir의 **상위**
    /// 조상(루트까지) 각각의 .cargo/config.toml·.cargo/config. temp_dir 자체는 기존
    /// 트립와이어(존재-중단) 관할이라 제외. 조상 열거는 canonicalize 기준 — cargo의
    /// 상향 걷기는 심링크 해소된 cwd 기준이다 (macOS /var→/private/var, M7 §5)
    pub fn take(cargo_home: Option<&Path>, temp_dir: &Path) -> Self {
        let mut paths = Vec::new();
        if let Some(home) = cargo_home {
            paths.push(home.join("config.toml"));
            paths.push(home.join("config"));
        }
        let canon = temp_dir.canonicalize().unwrap_or_else(|_| temp_dir.to_path_buf());
        let mut cur = canon.parent();
        while let Some(dir) = cur {
            let dot_cargo = dir.join(".cargo");
            paths.push(dot_cargo.join("config.toml"));
            paths.push(dot_cargo.join("config"));
            cur = dir.parent();
        }
        let entries = paths
            .into_iter()
            .map(|p| {
                let s = state_of(&p);
                (p, s)
            })
            .collect();
        Self { entries }
    }

    /// 스냅샷 대비 상태 전이가 있으면 변조로 판단해 에러 (하네스 중단 — exit 1)
    pub fn verify_unchanged(&self) -> anyhow::Result<()> {
        for (path, then) in &self.entries {
            if state_of(path) != *then {
                anyhow::bail!(
                    "판정 무결성 경고: 측정 시작 후 cargo 설정 파일이 변경되었습니다 ({}) — check가 오염된 설정을 읽을 수 있어 중단합니다",
                    path.display()
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// cargo_home 주입용 임시 구조: <tmp>/cargo-home + 조상 열거용 <tmp>/a/b/T
    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let cargo_home = dir.path().join("cargo-home");
        std::fs::create_dir_all(&cargo_home).unwrap();
        let deep_temp = dir.path().join("a/b/T");
        std::fs::create_dir_all(&deep_temp).unwrap();
        (dir, cargo_home, deep_temp)
    }

    #[test]
    fn unchanged_passes() {
        let (_d, ch, t) = setup();
        std::fs::write(ch.join("config.toml"), "[build]\n").unwrap();
        let snap = CargoConfigSnapshot::take(Some(&ch), &t);
        assert!(snap.verify_unchanged().is_ok(), "사전 존재 config는 수용 (변화-감지)");
    }

    #[test]
    fn content_change_is_detected() {
        let (_d, ch, t) = setup();
        std::fs::write(ch.join("config.toml"), "[build]\n").unwrap();
        let snap = CargoConfigSnapshot::take(Some(&ch), &t);
        std::fs::write(ch.join("config.toml"), "[target.'cfg(all())']\nrunner = \"evil\"\n").unwrap();
        let err = snap.verify_unchanged().unwrap_err();
        assert!(err.to_string().contains("config.toml"), "{err}");
    }

    #[test]
    fn creation_is_detected() {
        let (_d, ch, t) = setup();
        let snap = CargoConfigSnapshot::take(Some(&ch), &t);
        std::fs::write(ch.join("config"), "poison\n").unwrap();
        assert!(snap.verify_unchanged().is_err(), "부재→존재 전이도 변조 (레거시명 포함)");
    }

    #[test]
    fn deletion_is_detected() {
        let (_d, ch, t) = setup();
        std::fs::write(ch.join("config.toml"), "[build]\n").unwrap();
        let snap = CargoConfigSnapshot::take(Some(&ch), &t);
        std::fs::remove_file(ch.join("config.toml")).unwrap();
        assert!(snap.verify_unchanged().is_err(), "존재→부재 전이도 변조");
    }

    #[test]
    fn ancestors_above_temp_dir_are_watched() {
        // temp_dir(<tmp>/a/b/T)의 상위 조상 <tmp>/a에 config를 심으면 감지 (M7 §5 ②)
        let (d, _ch, t) = setup();
        let snap = CargoConfigSnapshot::take(None, &t);
        std::fs::create_dir_all(d.path().join("a/.cargo")).unwrap();
        std::fs::write(d.path().join("a/.cargo/config.toml"), "runner poison\n").unwrap();
        let err = snap.verify_unchanged().unwrap_err();
        assert!(err.to_string().contains(".cargo"), "{err}");
    }
}
```

참고: `resolve_cargo_home`은 env·홈 의존이라 단위 테스트를 두지 않는다(프로세스 전역 env 오염 회피 — 스냅샷 로직은 전부 경로 주입식으로 검증).

- [ ] **Step 2: 모듈 단위 테스트 그린 확인**

`src/eval/mod.rs:4`의 모듈 선언 블록에 추가(알파벳순 — `report` 앞):

```rust
pub mod integrity;
```

Run: `cargo test --lib eval::integrity`
Expected: PASS 5건. 신규 모듈이라 즉시 그린이 정상 — 하나라도 FAIL이면 구현 오류이므로 배선(Step 3) 전에 수정한다.

- [ ] **Step 3: 하네스 배선**

`src/eval/mod.rs`:

(a) `run_eval`(`:41`)에서 `let report_dir = ...`(`:51`) 다음에 채취:

```rust
    // M7 §5: 샌드박스 밖 cargo config 변조 감지 — 시작 시 1회 스냅샷, 매 런 check 전 비교
    let cargo_home = integrity::resolve_cargo_home();
    if cargo_home.is_none() {
        eprintln!("(CARGO_HOME을 해석할 수 없어 해당 감시를 생략합니다 — env 미설정·홈 없음)");
    }
    let cargo_snapshot =
        integrity::CargoConfigSnapshot::take(cargo_home.as_deref(), &std::env::temp_dir());
```

(b) `run_once` 호출(`:86`)에 인자 추가:

```rust
                match run_once(client, config, model, t, seed, repeat, opts, &report_dir, &interrupt, &cargo_snapshot).await? {
```

(c) `run_once` 시그니처(`:142-152`, `#[allow(clippy::too_many_arguments)]` 기존 유지)에 파라미터 추가:

```rust
    interrupt: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    cargo_snapshot: &integrity::CargoConfigSnapshot,
```

(d) `run_once` 내부의 `judge` 호출 **2곳**(`:201` TimedOut 경로, `:214` 정상 경로)에 `cargo_snapshot` 인자 추가. 예 (`:214`):

```rust
    let rec = judge(&sb, t, opts, kind, turns, elapsed, seed, repeat, interrupt, cargo_snapshot).await;
```

(e) `judge` 시그니처(`:223-233`, allow 기존 유지)에 파라미터 추가하고, `cargo_tripwire` 직후·check 실행 전(`:235` 다음)에 비교:

```rust
    cargo_snapshot: &integrity::CargoConfigSnapshot,
```

```rust
    sb.sync_protected(&t.fixture, &with_implicit_protected(&t.spec.protected))?;
    cargo_tripwire(&sb.root)?;
    cargo_snapshot.verify_unchanged()?;
```

(f) 트립와이어 문서 주석(`:292-294`)을 실효 범위에 맞게 교체:

```rust
/// 샌드박스 상위 경로(base까지)에 .cargo가 있으면 판정 무결성 훼손으로 하네스 중단.
/// 실효 검사는 temp_dir/.cargo 하나다 — 샌드박스 부모가 곧 temp_dir이고 base에서
/// 중단하므로. temp_dir 상위 조상과 $CARGO_HOME/홈 config는 M7 스냅샷 감지
/// (integrity.rs)가 맡고, cargo 바이너리 교체 벡터는 백로그 (M7 스펙 §5)
```

- [ ] **Step 4: 전체 테스트 + clippy**

Run: `cargo test` 그리고 `cargo clippy --all-targets -- -D warnings`
Expected: 전부 PASS·무경고. mod.rs의 `#[cfg(unix)]` 통합 테스트는 실제 머신의 CARGO_HOME·temp 조상을 스냅샷하지만 테스트 중 그 파일들이 바뀌지 않으므로 통과해야 정상 — 여기서 실패하면 배선 위치(check 전/후)나 조상 열거 버그다.

- [ ] **Step 5: 커밋**

```bash
git add src/eval/integrity.rs src/eval/mod.rs
git commit -m "feat(eval): CARGO_HOME·temp_dir 상위 조상 cargo config 스냅샷 감지 (M7 §5)"
```

---

### Task 3: C2 — 테스트 보강 2건 (프로덕션 코드 불변)

**Files:**
- Modify: `src/tools/grep.rs` (`#[cfg(test)] mod tests`만)
- Modify: `src/agent/repetition.rs` (`#[cfg(test)] mod tests`만)

**Interfaces:**
- Consumes: `Grep::run` 기존 동작(`grep.rs:97-110` — 폴백 헤더가 캡된 `matches` 값 보고 + `[more matches truncated at 50]` 마커), `RepetitionTracker::record`의 pop-front-before-count 시맨틱(`repetition.rs:45-49`, WINDOW=8)
- Produces: 없음 (특성 고정 테스트)

**주의: 이 태스크의 테스트는 기존 동작의 특성 고정(characterization)이다 — 작성 즉시 통과해야 정상.** 실패하면 프로덕션 코드를 고치지 말고(동결) 중단·보고한다: 스펙 가정과 실동작이 다르다는 뜻이다.

- [ ] **Step 1: grep 폴백+절단 결합 테스트 작성**

`src/tools/grep.rs`의 tests 모듈에 추가 (`invalid_regex_falls_back_to_literal_search` 아래):

```rust
    #[test]
    fn literal_fallback_and_truncation_combine() {
        // M7 §6.1 — 폴백 헤더(캡 값 보고)와 절단 마커가 공존하는 결합 경로 고정.
        // 각각은 invalid_regex_*·caps_matches_at_50이 커버하지만 결합은 비어 있었다
        let dir = tempfile::tempdir().unwrap();
        let body: String = (1..=60).map(|i| format!("val {{hit}} {i}\n")).collect();
        std::fs::write(dir.path().join("many.txt"), body).unwrap();
        let ctx = ToolCtx::new(dir.path().to_path_buf());
        let out = Grep.run(&serde_json::json!({"pattern": "{hit}"}), &ctx).unwrap();
        assert!(out.starts_with("invalid regex"), "{out}");
        assert!(
            out.contains(&format!("literal text instead - {MAX_MATCHES} matches")),
            "헤더는 캡된 매치 수를 보고: {out}"
        );
        assert_eq!(out.matches("many.txt:").count(), MAX_MATCHES, "{out}");
        assert!(out.contains("[more matches truncated at 50]"), "{out}");
    }
```

- [ ] **Step 2: 통과 확인**

Run: `cargo test --lib tools::grep`
Expected: PASS (전 케이스). `literal_fallback_and_truncation_combine` FAIL 시 중단·보고.

- [ ] **Step 3: RepetitionTracker 부분 축출 경계 쌍 작성**

`src/agent/repetition.rs`의 tests 모듈에 추가 (`window_caps_at_eight_entries` 아래). 산술 근거: `record`는 만석(8)이면 **pop_front를 카운트 전에** 수행한다(`repetition.rs:45-49`) — 히트2+패딩6=만석에서 3번째 히트 푸시는 최고령 히트를 밀어내 윈도 내 동일 항목이 2가 되고, 패딩이 5면 축출 없이 3이 성립한다:

```rust
    #[test]
    fn partial_eviction_third_hit_evicts_oldest_and_stays_ok() {
        // M7 §6.2 — 완전 축출(window_caps_at_eight_entries)과 구별되는 오프바이원 경계:
        // 히트2+패딩6=만석 → 3번째 히트 푸시가 스스로 최고령 히트를 축출 → 카운트 2 → Ok
        let mut t = RepetitionTracker::new();
        t.record("a|1", "r");
        t.record("a|1", "r");
        for i in 0..6 {
            t.record(&format!("pad|{i}"), "x");
        }
        assert!(matches!(t.record("a|1", "r"), RepetitionVerdict::Ok), "축출로 3회 미달");
    }

    #[test]
    fn partial_eviction_one_fewer_pad_still_corrects() {
        // 위 케이스의 쌍 — 패딩 하나 적으면(2+5=7, 축출 없음) 3회가 성립해 교정 주입
        let mut t = RepetitionTracker::new();
        t.record("a|1", "r");
        t.record("a|1", "r");
        for i in 0..5 {
            t.record(&format!("pad|{i}"), "x");
        }
        assert!(matches!(t.record("a|1", "r"), RepetitionVerdict::InjectCorrection));
    }
```

- [ ] **Step 4: 통과 확인 + 전체 게이트**

Run: `cargo test --lib agent::repetition` 후 `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전부 PASS·무경고. 두 신규 케이스 중 하나라도 FAIL이면 중단·보고 (프로덕션 수정 금지).

- [ ] **Step 5: 커밋**

```bash
git add src/tools/grep.rs src/agent/repetition.rs
git commit -m "test: grep 폴백+절단 결합·RepetitionTracker 부분 축출 경계 고정 (M7 §6)"
```

---

### Task 4: A1 + C4 — 모델 세트 재편·문서 정리

**Files:**
- Modify: `docs/baselines.md` (재편 절 신설, 탐색 절 승격 표시, M5 잔여 한계 포인터)
- Modify: `README.md` (상태·요지 표)
- Modify: `CLAUDE.md` (영문 — remote 노트, eval 불릿, 상태 라인)

**Interfaces:**
- Consumes: Task 1·2의 결과 서술(avg_duration_secs, integrity.rs), 로컬 report.json 3종(`.loco/eval/20260716T{051407,055019,062853}Z/report.json` — git-ignored, 없으면 아래 폴백)
- Produces: 없음 (문서)

- [ ] **Step 1: 모델별 평균 s/런 산출**

```bash
python3 - <<'EOF'
import json
for stamp in ["20260716T051407Z", "20260716T055019Z", "20260716T062853Z"]:
    p = f".loco/eval/{stamp}/report.json"
    try:
        r = json.load(open(p))
    except FileNotFoundError:
        print(stamp, "없음 — 폴백 사용"); continue
    runs = [x["duration_secs"] for t in r["tasks"] for x in t["runs"]]
    print(stamp, r["model"], f"{sum(runs)/len(runs):.1f}s/런")
EOF
```

Expected: 세 줄 (gemma·qwen·ornith 순, ornith은 ~67.3s). **폴백**(report.json 유실 시): `docs/baselines.md` v2 "과제별" 표의 평균 시간 열 12값의 단순 평균 — repeats=3 균일이라 런 가중과 일치한다.

- [ ] **Step 2: baselines.md 편집**

(a) 파일 끝("### 탐색: 9B" 절 뒤)에 신설:

```markdown
## 모델 세트 재편 (M7, 2026-07-16)

기준선 모델 세트를 **google/gemma-4-e4b(4B 대표) + ornith-1.0-9b(Qwen3 계열 대표)**로
재편한다 (M7 스펙 §3).

- **qwen3-vl-4b 은퇴**: 같은 계열(Qwen3) 9B가 4B급 속도로 대폭 상회(50.0%→94.4%)하고
  종료 규율 약점(엄격 격차 16.7pp)도 9B에서 해소(0pp)됐다. v2 수치는 위 절에 역사
  기록으로 유지하며, 이후 마일스톤의 측정 대상에서 제외한다.
- **ornith-1.0-9b 승격**: 탐색 측정(`20260716T062853Z`, 하네스 `4cb7325`)을 기준선으로
  **재지정** — v2 기준선과 동일 프로토콜·동일 하네스로 이미 측정됐고 이후 main은 문서
  커밋뿐이라 재측정하지 않는다. 한계: report.json은 하네스 커밋을 자증하지 못한다
  (`loco_version`은 전 커밋 0.1.0) — 커밋 해시 스냅샷은 백로그.
- **대체가 아닌 재편**: 계열은 같아도 크기(4B→9B)·변종(vl vs Ornith 파인튜닝)이 달라
  동일 모델 대체가 아니다 — qwen→ornith 수치를 시계열로 잇는 해석을 금지한다.
- **속도 병기** (M7 §4): 이후 모델 비교는 평균 s/런을 1급 정보로 병기한다.

| 모델 | 통과 | 엄격 | 평균 s/런 | 세트 지위 |
|---|---|---|---|---|
| google/gemma-4-e4b | 72.2% | 69.4% | {Step 1 값}s | 기준선 (4B 대표) |
| qwen/qwen3-vl-4b | 50.0% | 33.3% | {Step 1 값}s | **은퇴 (M7)** |
| ornith-1.0-9b | 94.4% | 94.4% | 67.3s | **기준선 (M7 승격, Qwen3 계열 대표)** |

### 판정 무결성 갱신 (M7 §5)

- `$CARGO_HOME`/홈 config와 temp_dir **상위 조상**의 `.cargo/config*` 변조는 이제
  스냅샷 감지(하네스 시작 1회 기록 → 매 런 check 전 비교, 상태 전이=중단)로 잡는다
  (`src/eval/integrity.rs`). 측정 중 사용자가 해당 config를 직접 편집해도 중단된다
  (오탐 수용 — 병행 작업 금지 프로토콜과 일관).
- 잔여(백로그): 시작 전 사전 오염 config(CARGO_HOME 격리가 닫을 대상), cargo
  **바이너리 교체**(`$CARGO_HOME/bin`·`~/.rustup` — PATH 고정/절대경로/해시 계열).
```

`{Step 1 값}`은 Step 1 산출값으로 치환 (플레이스홀더를 남기지 말 것).

(b) "### 탐색: 9B (저사양 대형모델 방향, 기준선 아님)" 헤딩 바로 아래에 한 줄 추가:

```markdown
> **M7 갱신**: 이 측정은 기준선으로 승격되었다 — 아래 "모델 세트 재편 (M7)" 절 참고.
```

(c) M5 "잔여 한계" 절의 `.cargo` 항목(`docs/baselines.md:136`) 끝에 문장 추가:

```markdown
(M7: `$CARGO_HOME`/홈·temp_dir 상위 조상 config 벡터는 스냅샷 감지로 승격 — "판정 무결성 갱신 (M7 §5)" 절 참고.)
```

- [ ] **Step 3: README.md 편집**

(a) 상태 헤딩·문단(`## 프로젝트 상태: M6 완료 · 저사양 대형모델 방향 탐색 (2026-07-16)` 및 이어지는 두 문단)을 교체:

```markdown
## 프로젝트 상태: M7 완료 · 모델 세트 재편 (2026-07-16)

M7은 측정 체계 마무리다 — 모델 세트 재편(qwen3-vl-4b 은퇴, Ornith 9B를 Qwen3 계열
대표 기준선으로 승격), 평균 s/런의 리포트 1급 지표화, 판정 무결성 보강(cargo config
스냅샷 감지), 테스트·문서 부채 정리. 판정기·에이전트 코드는 불변이라 v2 수치는 그대로
비교 가능하다. 상세는 `docs/baselines.md` "모델 세트 재편 (M7)" 절.
```

(b) "### v2 기준선 요지" 표를 교체 (Step 1 값 사용):

```markdown
| 모델 | 통과 | 엄격(Finished∧통과) | 거짓 성공 finish | 평균 s/런 | 세트 지위 |
|---|---|---|---|---|---|
| google/gemma-4-e4b | 72.2% (26/36) | 69.4% (25/36) | 4 | {값}s | 기준선 (4B 대표) |
| qwen/qwen3-vl-4b | 50.0% (18/36) | 33.3% (12/36) | 3 | {값}s | 은퇴 (M7) |
| ornith-1.0-9b | 94.4% (34/36) | 94.4% (34/36) | 0 | 67.3s | 기준선 (M7 승격) |
```

표 아래 측정 조건 문장은 유지하되 끝에 추가: `모델 세트 재편 경위는 docs/baselines.md 참고.`

- [ ] **Step 4: CLAUDE.md 편집 (영문 유지)**

(a) `- No git remote — local-only repo` 줄 교체:

```markdown
- Remote: `origin` = github.com/SeonggukJeong/loco.git — push only when the user asks
```

(b) 서두 상태 문장(`M1-M5 done ... docs/superpowers/specs/2026-07-12-m5-scaffolding-design.md`.) 교체:

```markdown
M1-M7 done. v2 baselines (M6 judge overhaul — NOT comparable to v1/M5 numbers): gemma-4-e4b 72.2% (strict 69.4%), ornith-1.0-9b 94.4% (strict 94.4%, promoted to baseline in M7's set reorg; qwen3-vl-4b retired) — details and history in `docs/baselines.md`. M7 spec: `docs/superpowers/specs/2026-07-16-m7-set-reorg-integrity-design.md`.
```

(c) eval 불릿의 트립와이어 구절 교체 — old:

```
`.cargo` is implicitly protected, and a `.cargo` anywhere above the sandbox aborts the harness — tripwire detects but does not clean up, so a leftover `${TMPDIR}/.cargo` must be removed manually; `$CARGO_HOME`/home-dir vectors remain unblocked
```

new:

```
`.cargo` is implicitly protected; the tripwire's effective check is `temp_dir/.cargo` only (the sandbox's parent IS temp_dir — detection only, no cleanup: remove a leftover `${TMPDIR}/.cargo` manually). M7 adds change-detection snapshots (`eval/integrity.rs`): `$CARGO_HOME` config.toml/config plus `.cargo/config*` in every ancestor above canonicalized temp_dir are recorded at harness start and re-compared before each check — any state transition aborts the harness. Still out of scope (backlog): pre-existing poisoned configs (CARGO_HOME isolation) and cargo-binary replacement (`$CARGO_HOME/bin`, `~/.rustup` — PATH pinning/hashing)
```

(d) 같은 불릿의 `_count`-suffixed additions 문장 끝에 추가:

```
, plus top-level `avg_duration_secs` (run-weighted mean of per-run agent durations — excludes check/sandbox overhead, so it differs from wall-clock `duration_secs`/runs; M7 §4)
```

- [ ] **Step 5: 커밋**

```bash
git add docs/baselines.md README.md CLAUDE.md
git commit -m "docs: M7 모델 세트 재편(qwen 은퇴·Ornith 승격)·속도 병기·무결성 갱신·스테일 정리 (A1·C4)"
```

---

### Task 5: 최종 게이트 (스펙 §2 성공 기준 전건 확인)

**Files:** 없음 (검증만 — 실패 시에만 수정 커밋)

**Interfaces:**
- Consumes: Task 1-4 전체
- Produces: 성공 기준 5항 판정

- [ ] **Step 1: 테스트·클리피**

Run: `cargo test 2>&1 | tail -5` 그리고 `cargo clippy --all-targets -- -D warnings`
Expected: 전부 PASS (기존 250 + 신규 9±), 무경고

- [ ] **Step 2: 판정기 회귀 게이트**

Run: `cargo run -- eval tasks --verify`
Expected: 12과제 모두 변별성·해결가능성 통과, exit 0 (tasks/ 불변이므로 순수 회귀 확인 — 수 분 소요, LLM 불필요)

- [ ] **Step 3: diff 규율 검사 (성공 기준 4)**

```bash
git diff 6ab4f3b..HEAD --stat -- tasks/          # 출력 없어야 함
git diff 6ab4f3b..HEAD -- src/agent src/tools    # #[cfg(test)] mod tests 내부 추가만이어야 함 — 눈으로 확인
```

Expected: tasks/ 무변경; src/agent·src/tools diff는 테스트 모듈 내부의 순수 추가뿐(시그니처·프로덕션 경로 변경 0)

- [ ] **Step 4: report.json 스키마 확인 (성공 기준 3)**

Run: `cargo test --lib eval::report`
Expected: PASS — `report_json_adds_count_fields_keeps_old_ones`(기존 키 보존)와 `report_json_has_design_schema_fields`(신규 키 포함 목록) 동시 그린

- [ ] **Step 5: 결과 요약 보고**

성공 기준 5항 각각에 대해 증거(명령 출력)와 함께 통과/실패를 보고한다. 실패 항목이 있으면 커밋하지 말고 중단·보고.
