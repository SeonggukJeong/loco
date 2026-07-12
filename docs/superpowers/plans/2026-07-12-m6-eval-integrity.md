# M6 판정·평가 신뢰성 개편 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 평가 하네스의 판정을 신뢰 가능하게 만든다 — 메타테스트(`eval --verify`) + 판정기 수선(정규화 사다리) + 이중 리포트 + v2 기준선 재측정.

**Architecture:** 스펙 `docs/superpowers/specs/2026-07-12-m6-eval-integrity-design.md`. 신규 모듈 `src/eval/verify.rs`가 기존 샌드박스·exec를 재사용해 과제별 "픽스처에서 check 실패(변별성) + solution/ 오버레이 후 통과(해결가능성)"를 LLM 없이 검증한다. 판정기 수선은 픽스처의 판정 테스트 내부(하네스 아님). report.rs에 `_count` 집계 3종 추가.

**Tech Stack:** Rust edition 2024, tokio, clap(derive), serde/serde_json, anyhow. **신규 크레이트 금지.**

## Global Constraints

- **에이전트 코드 동결**: `src/agent/`, `src/tools/`(exec.rs 포함), SYSTEM_PROMPT, `src/llm/`, `src/session.rs`, `src/config.rs`는 이 마일스톤에서 **한 줄도 수정 금지** — v2 수치 변화가 순수 판정기 변경분임을 보장한다 (스펙 §2)
- 의존성 고정 — 새 크레이트 추가 금지 (스펙 하드 제약)
- 사용자-facing CLI 문자열은 한국어, 식별자·주석 규약은 기존 코드를 따름
- 매 태스크 종료 시 `cargo test` + `cargo clippy --all-targets -- -D warnings` 통과 필수
- `tasks/` 픽스처 크레이트는 워크스페이스 비멤버 — 루트 `cargo test`가 픽스처 테스트를 돌리지 않으므로 판정 테스트 검증은 `--verify` 실행으로 한다
- 커밋은 conventional commits (제목 한국어 가능)
- `.claude/` 훅이 픽스처 편집 시 확인을 요구할 수 있음 — 이 플랜의 픽스처 판정 테스트 수정(Task 4)과 solution/ 추가(Task 3)는 스펙이 요구하는 의도된 변경이다

## File Structure

| 파일 | 역할 |
|---|---|
| Create `src/eval/verify.rs` | verify 모드 전체(옵션·레코드·검증 루프·한국어 표) + 단위/통합 테스트 |
| Modify `src/eval/mod.rs` | `pub mod verify;` + 헬퍼 3종 `pub(crate)` 승격 |
| Modify `src/eval/sandbox.rs` | `overlay_tree`(read+write) 신설 + `sync_protected` mtime 벡터 수선 |
| Modify `src/main.rs` | `--verify` 플래그, Eval 분기 재배선(verify는 client/model 생략) |
| Modify `src/eval/report.rs` | `passed_count`·`passed_strict_count`·`false_finish_count` 집계 + 표 확장 |
| Create `tasks/*/solution/**` (12개) | 레퍼런스 솔루션 오버레이 |
| Modify `tasks/.gitattributes` | edit-crlf-file solution CRLF 바이트 고정 |
| Modify `tasks/find-definition/fixture/tests/check.rs` 등 3과제 판정 테스트 | 정규화 사다리 + 사다리 단위 테스트, 비변별 케이스 교체 |
| Modify `CLAUDE.md`, `docs/baselines.md` | 커맨드·v2 기준선 문서화 |

---

### Task 1: verify 코어 (`src/eval/verify.rs`)

**Files:**
- Create: `src/eval/verify.rs`
- Modify: `src/eval/mod.rs:4-6` (모듈 선언), `src/eval/mod.rs:271,280,309` (헬퍼 가시성), `src/eval/sandbox.rs` (`overlay_tree` 신설 + `sync_protected` 수선)
- Test: `src/eval/verify.rs`·`src/eval/sandbox.rs` 내 `#[cfg(test)]` 모듈

**Interfaces:**
- Consumes: `super::task::load_tasks(&Path) -> anyhow::Result<Vec<Task>>` (이름순 정렬 보장), `super::sandbox::{Sandbox, overlay_tree}`, `super::{cargo_tripwire, scaled_timeout, with_implicit_protected}`, `crate::tools::exec::{exec_shell, ExecEnd}` (`exec_shell(command: &str, cwd: &Path, timeout: Duration, cancel: &AtomicBool) -> std::io::Result<Exec>`)
- Produces: `pub(crate) fn overlay_tree(src: &Path, dst: &Path) -> anyhow::Result<()>` (sandbox.rs), `pub struct VerifyOptions { pub tasks_dir: PathBuf, pub timeout_scale: f64 }`, `pub struct VerifyRecord { pub name: String, pub discriminates: bool, pub solvable: bool, pub error: Option<String> }` + `impl VerifyRecord { pub fn ok(&self) -> bool }`, `pub async fn run_verify(opts: &VerifyOptions) -> anyhow::Result<Vec<VerifyRecord>>`, `pub fn render_verify_table(records: &[VerifyRecord]) -> String` — Task 2(main 배선)가 verify 쪽 넷을 사용

- [ ] **Step 1: mod.rs 가시성 준비**

`src/eval/mod.rs`의 모듈 선언에 `pub mod verify;`를 추가하고(4~6행의 `pub mod report;` 옆), 아래 3개 함수의 `fn`을 `pub(crate) fn`으로 바꾼다 (기존 본문 무변경):

- `fn scaled_timeout(secs: u64, scale: f64) -> Duration` (mod.rs:271)
- `fn with_implicit_protected(protected: &[String]) -> Vec<String>` (mod.rs:280)
- `fn cargo_tripwire(sandbox_root: &Path) -> anyhow::Result<()>` (mod.rs:309)

이 시점엔 verify.rs가 없어 컴파일이 깨지므로 Step 3과 함께 진행한다.

- [ ] **Step 2: sandbox.rs — `overlay_tree` 신설 + `sync_protected` mtime 벡터 수선**

**배경 (스펙 §4, 플랜 리뷰 C-2·I-2 실측):** macOS의 `std::fs::copy`는 원본 mtime을 보존한다(clonefile). 웜 샌드박스에 원본 mtime 그대로 소스를 덮으면 기존 빌드 산출물보다 과거가 되어 cargo가 재빌드를 건너뛰고 **스테일 테스트 바이너리로 판정**한다 — verify 오버레이(8/12 거짓 ✗ 실측)와 eval의 `sync_protected` 복원(변조된 protected로 빌드된 캐시 재사용) 둘 다 해당. read+write는 mtime을 쓰기 시각으로 갱신해 이 벡터를 없앤다.

`src/eval/sandbox.rs`에 추가 (`copy_tree` 아래 — `copy_tree`는 신선한 샌드박스 생성 전용으로 유지, 가시성 변경 없음):

```rust
/// src 트리를 dst에 덮어쓴다 — fs::copy 대신 read+write. macOS의 fs::copy는
/// 원본 mtime을 보존해(clonefile) 기존 빌드 산출물보다 과거가 되고, cargo가
/// 재빌드를 건너뛰어 스테일 테스트 바이너리로 판정하는 벡터가 된다 (M6 §4)
pub(crate) fn overlay_tree(src: &Path, dst: &Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let meta = std::fs::symlink_metadata(&from)?;
        if meta.is_symlink() {
            bail!("오버레이 원본에 심링크가 있음 (지원 안 함): {}", from.display());
        }
        if meta.is_dir() {
            std::fs::create_dir_all(&to)?;
            overlay_tree(&from, &to)?;
        } else {
            let bytes = std::fs::read(&from)?;
            std::fs::write(&to, bytes)
                .with_context(|| format!("오버레이 쓰기 실패: {}", to.display()))?;
        }
    }
    Ok(())
}
```

`sync_protected`(sandbox.rs:43-61)의 복원 두 곳을 같은 이유로 교체 — dir 브랜치의 `copy_tree(&src, &dst)?` → `overlay_tree(&src, &dst)?`, file 브랜치의 `std::fs::copy(&src, &dst)?` →

```rust
                let bytes = std::fs::read(&src)?;
                std::fs::write(&dst, bytes)?;
```

sandbox.rs 테스트 모듈에 mtime 회귀 테스트 2건 추가 (`File::set_modified`는 std, 1.75+):

```rust
    fn age_file(p: &Path) -> std::time::SystemTime {
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        let f = std::fs::File::options().write(true).open(p).unwrap();
        f.set_modified(old).unwrap();
        old
    }

    #[test]
    fn overlay_tree_refreshes_mtime() {
        // fs::copy였다면 macOS에서 원본 mtime(1시간 전)이 보존돼 실패한다
        let src = fixture_with(&[("a.rs", "new")]);
        let dst = fixture_with(&[("a.rs", "stale")]);
        let old = age_file(&src.path().join("a.rs"));
        overlay_tree(src.path(), dst.path()).unwrap();
        let copied = std::fs::metadata(dst.path().join("a.rs")).unwrap().modified().unwrap();
        assert!(copied > old + std::time::Duration::from_secs(1800), "오버레이는 mtime을 갱신해야 함 — 스테일 빌드 캐시 방지 (M6 §4)");
        assert_eq!(std::fs::read_to_string(dst.path().join("a.rs")).unwrap(), "new");
    }

    #[test]
    fn sync_protected_refreshes_mtime() {
        // 에이전트가 protected를 변조·빌드해 둔 뒤 복원돼도 check가 재빌드하게
        let fx = fixture_with(&[("tests/t.rs", "ORIGINAL")]);
        let old = age_file(&fx.path().join("tests/t.rs"));
        let sb = Sandbox::create(fx.path()).unwrap();
        std::fs::write(sb.root.join("tests/t.rs"), "HACKED").unwrap();
        sb.sync_protected(fx.path(), &["tests".to_string()]).unwrap();
        let restored = std::fs::metadata(sb.root.join("tests/t.rs")).unwrap().modified().unwrap();
        assert!(restored > old + std::time::Duration::from_secs(1800), "protected 복원도 mtime 갱신 (M6 §4)");
        assert_eq!(std::fs::read_to_string(sb.root.join("tests/t.rs")).unwrap(), "ORIGINAL");
        sb.cleanup();
    }
```

Run: `cargo test --lib eval::sandbox`
Expected: 기존 8개 + 신규 2개 전부 PASS

- [ ] **Step 3: verify.rs 뼈대 + 실패하는 테스트 작성**

`src/eval/verify.rs`를 아래 전체 내용으로 생성한다. 테스트가 먼저 의미를 고정하고, 구현은 Step 4에서 채운다 — 뼈대의 `todo!()`로 컴파일은 되게 한다:

```rust
//! 판정기 메타테스트 (M6 스펙 §4) — LLM 없이 과제마다 두 성질을 검증한다:
//! 변별성(픽스처 원본에서 check 실패)과 해결가능성(solution/ 오버레이 후 check 통과).
//! 측정이 아니라 게이트 — report.json을 쓰지 않고 표와 종료 코드로만 보고한다.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Context;

use super::sandbox::{overlay_tree, Sandbox};
use super::task::{load_tasks, Task};
use super::{cargo_tripwire, scaled_timeout, with_implicit_protected};
use crate::tools::exec::{exec_shell, ExecEnd};

pub struct VerifyOptions {
    pub tasks_dir: PathBuf,
    pub timeout_scale: f64,
}

#[derive(Debug)]
pub struct VerifyRecord {
    pub name: String,
    /// 1단계 — 픽스처 원본에서 check가 실패했는가 (버그 상태를 변별)
    pub discriminates: bool,
    /// 2단계 — solution/ 오버레이 후 check가 통과했는가
    pub solvable: bool,
    /// 검증을 실행할 수 없었던 사유 (solution/ 부재, protected 겹침)
    pub error: Option<String>,
}

impl VerifyRecord {
    pub fn ok(&self) -> bool {
        self.error.is_none() && self.discriminates && self.solvable
    }
}

/// 전 과제 검증. Ctrl+C는 하네스 에러로 전파한다 — 게이트는 부분 결과가 무의미
pub async fn run_verify(opts: &VerifyOptions) -> anyhow::Result<Vec<VerifyRecord>> {
    todo!()
}

/// stdout용 한국어 표 — 마지막 줄이 `검증 n/m 통과` 요약
pub fn render_verify_table(records: &[VerifyRecord]) -> String {
    todo!()
}
```

이어서 같은 파일 하단에 테스트 모듈 2개 — 표 렌더링은 셸 의존이 없어 플랫폼 무관 모듈로, check 실행 테스트는 `sh -c` 의존이라 기존 eval 테스트와 동일하게 unix 게이트:

```rust
#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn table_marks_and_summary() {
        let recs = vec![
            VerifyRecord { name: "a".into(), discriminates: true, solvable: true, error: None },
            VerifyRecord { name: "b".into(), discriminates: false, solvable: true, error: None },
            VerifyRecord { name: "c".into(), discriminates: false, solvable: false, error: Some("solution/ 없음".into()) },
        ];
        let table = render_verify_table(&recs);
        assert!(table.contains("검증 1/3"), "{table}");
        assert!(table.contains("게이트 실패"));
        assert!(table.contains("solution/ 없음"), "error 사유가 표에 보여야 함");
        let all_ok = vec![VerifyRecord { name: "a".into(), discriminates: true, solvable: true, error: None }];
        assert!(!render_verify_table(&all_ok).contains("게이트 실패"));
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;

    fn write_task(
        root: &Path,
        name: &str,
        toml: &str,
        fixture: &[(&str, &str)],
        solution: Option<&[(&str, &str)]>,
    ) {
        let dir = root.join(name);
        std::fs::create_dir_all(dir.join("fixture")).unwrap();
        std::fs::write(dir.join("task.toml"), toml).unwrap();
        for (rel, content) in fixture {
            let p = dir.join("fixture").join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
        if let Some(files) = solution {
            std::fs::create_dir_all(dir.join("solution")).unwrap();
            for (rel, content) in files {
                let p = dir.join("solution").join(rel);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(p, content).unwrap();
            }
        }
    }

    const TOML: &str = "prompt = \"p\"\ncheck = \"test -f solved.txt\"\nprotected = [\"keep.txt\"]\n";

    fn opts(dir: &Path) -> VerifyOptions {
        VerifyOptions { tasks_dir: dir.to_path_buf(), timeout_scale: 1.0 }
    }

    #[tokio::test]
    async fn both_gates_pass() {
        let dir = tempfile::tempdir().unwrap();
        write_task(dir.path(), "good", TOML, &[("keep.txt", "k")], Some(&[("solved.txt", "s")]));
        let recs = run_verify(&opts(dir.path())).await.unwrap();
        assert_eq!(recs.len(), 1);
        assert!(recs[0].discriminates && recs[0].solvable, "{recs:?}");
        assert!(recs[0].ok());
    }

    #[tokio::test]
    async fn non_discriminating_check_fails_gate() {
        // check가 픽스처 원본에서 이미 통과 → 변별성 ✗ (비변별 판정기 검출)
        let dir = tempfile::tempdir().unwrap();
        let toml = "prompt = \"p\"\ncheck = \"true\"\nprotected = [\"keep.txt\"]\n";
        write_task(dir.path(), "lax", toml, &[("keep.txt", "k")], Some(&[("solved.txt", "s")]));
        let recs = run_verify(&opts(dir.path())).await.unwrap();
        assert!(!recs[0].discriminates && recs[0].solvable);
        assert!(!recs[0].ok());
    }

    #[tokio::test]
    async fn unsolvable_solution_fails_gate() {
        let dir = tempfile::tempdir().unwrap();
        write_task(dir.path(), "broken", TOML, &[("keep.txt", "k")], Some(&[("wrong.txt", "s")]));
        let recs = run_verify(&opts(dir.path())).await.unwrap();
        assert!(recs[0].discriminates && !recs[0].solvable);
        assert!(!recs[0].ok());
    }

    #[tokio::test]
    async fn missing_solution_dir_fails_gate() {
        let dir = tempfile::tempdir().unwrap();
        write_task(dir.path(), "nosol", TOML, &[("keep.txt", "k")], None);
        let recs = run_verify(&opts(dir.path())).await.unwrap();
        assert!(recs[0].error.as_deref().unwrap().contains("solution/"), "{recs:?}");
        assert!(!recs[0].ok());
    }

    #[tokio::test]
    async fn solution_overwriting_protected_fails_gate() {
        // 저작 실수 방어: solution이 판정 자산을 덮으면 "판정기를 바꾼 해결가능성 증명"이 된다 (스펙 §4)
        let dir = tempfile::tempdir().unwrap();
        write_task(
            dir.path(),
            "tamper",
            TOML,
            &[("keep.txt", "k")],
            Some(&[("keep.txt", "HACK"), ("solved.txt", "s")]),
        );
        let recs = run_verify(&opts(dir.path())).await.unwrap();
        let err = recs[0].error.as_deref().unwrap();
        assert!(err.contains("protected"), "{err}");
    }

    #[tokio::test]
    async fn implicit_dot_cargo_overlap_fails_gate() {
        let dir = tempfile::tempdir().unwrap();
        write_task(
            dir.path(),
            "cargo-hack",
            TOML,
            &[("keep.txt", "k")],
            Some(&[(".cargo/config.toml", "[target]"), ("solved.txt", "s")]),
        );
        let recs = run_verify(&opts(dir.path())).await.unwrap();
        assert!(recs[0].error.as_deref().unwrap().contains(".cargo"), "{recs:?}");
    }
}
```

- [ ] **Step 4: 테스트가 실패(패닉)하는지 확인**

Run: `cargo test --lib eval::verify 2>&1 | tail -20`
Expected: `todo!()` 패닉으로 unix 모듈 6개 FAIL, 표 테스트만 통과 여부 무관 (컴파일은 성공해야 한다 — 컴파일 에러면 가시성 승격 누락). 이 단계의 미사용 import·파라미터 경고 다수는 정상 — Step 5 구현이 전부 소비한다

- [ ] **Step 5: 구현 채우기**

`run_verify`/`render_verify_table`의 `todo!()`를 아래 구현으로 교체하고 보조 함수를 추가한다:

```rust
/// 전 과제 검증. Ctrl+C는 하네스 에러로 전파한다 — 게이트는 부분 결과가 무의미
pub async fn run_verify(opts: &VerifyOptions) -> anyhow::Result<Vec<VerifyRecord>> {
    let tasks = load_tasks(&opts.tasks_dir)?;
    // run_eval과 같은 장수 SIGINT 리스너 — check 실행 중 Ctrl+C가 프로세스 그룹을 죽인다
    let interrupt = Arc::new(AtomicBool::new(false));
    let listener = tokio::spawn({
        let flag = interrupt.clone();
        async move {
            while tokio::signal::ctrl_c().await.is_ok() {
                flag.store(true, Ordering::SeqCst);
            }
        }
    });
    let result = verify_all(&tasks, opts.timeout_scale, &interrupt).await;
    listener.abort();
    result
}

async fn verify_all(
    tasks: &[Task],
    scale: f64,
    interrupt: &Arc<AtomicBool>,
) -> anyhow::Result<Vec<VerifyRecord>> {
    let mut records = Vec::new();
    for t in tasks {
        if interrupt.load(Ordering::SeqCst) {
            anyhow::bail!("중단됨 — 검증은 부분 결과를 기록하지 않습니다");
        }
        eprintln!("[{}] 검증 중…", t.name);
        records.push(verify_one(t, scale, interrupt).await?);
    }
    Ok(records)
}

async fn verify_one(t: &Task, scale: f64, interrupt: &Arc<AtomicBool>) -> anyhow::Result<VerifyRecord> {
    let fail = |why: String| VerifyRecord {
        name: t.name.clone(),
        discriminates: false,
        solvable: false,
        error: Some(why),
    };
    let solution = t.fixture.parent().expect("fixture는 과제 디렉터리 바로 아래").join("solution");
    if !solution.is_dir() {
        return Ok(fail("solution/ 없음 — 전 과제가 레퍼런스 솔루션을 가져야 합니다 (M6 §4)".into()));
    }
    let protected = with_implicit_protected(&t.spec.protected);
    if let Some(bad) = first_protected_overlap(&solution, &protected)? {
        return Ok(fail(format!("solution/이 protected 경로를 덮음: {bad}")));
    }
    let sb = Sandbox::create(&t.fixture)?;
    // 에러 경로에서도 샌드박스를 정리한 뒤 전파 (judge와 동일 패턴)
    let res = check_both(t, &sb, &solution, scale, interrupt).await;
    sb.cleanup();
    let (discriminates, solvable) = res?;
    Ok(VerifyRecord { name: t.name.clone(), discriminates, solvable, error: None })
}

/// (1단계 check 실패 여부 → 변별성, 2단계 check 통과 여부 → 해결가능성)
async fn check_both(
    t: &Task,
    sb: &Sandbox,
    solution: &Path,
    scale: f64,
    interrupt: &Arc<AtomicBool>,
) -> anyhow::Result<(bool, bool)> {
    // 잔류 .cargo 환경이면 게이트 자체가 오염 — 하네스 중단 (스펙 §4: verify에서도 실행)
    cargo_tripwire(&sb.root)?;
    let timeout = scaled_timeout(t.spec.check_timeout_secs, scale);
    let step1 = run_check(&t.spec.check, &sb.root, timeout, interrupt)
        .await
        .with_context(|| format!("과제 {}: 1단계(변별성) check 실행 실패", t.name))?;
    // read+write 오버레이 — fs::copy는 macOS에서 mtime을 보존해 스테일 판정 (스펙 §4)
    overlay_tree(solution, &sb.root)
        .with_context(|| format!("과제 {}: solution/ 오버레이 실패", t.name))?;
    let step2 = run_check(&t.spec.check, &sb.root, timeout, interrupt)
        .await
        .with_context(|| format!("과제 {}: 2단계(해결가능성) check 실행 실패", t.name))?;
    Ok((!step1, step2))
}

/// check를 워커 스레드에서 실행해 통과 여부를 돌려준다. 취소·타임아웃은 에러
/// (타임아웃을 "실패"로 읽으면 1단계에서 거짓 변별성이 되므로 판정 불능으로 처리)
async fn run_check(
    check: &str,
    root: &Path,
    timeout: std::time::Duration,
    interrupt: &Arc<AtomicBool>,
) -> anyhow::Result<bool> {
    let check = check.to_string();
    let root = root.to_path_buf();
    let cancel = interrupt.clone();
    let exec = tokio::task::spawn_blocking(move || exec_shell(&check, &root, timeout, &cancel))
        .await
        .context("check 실행 태스크가 패닉")?
        .context("check 명령 실행 실패")?;
    match exec.end {
        ExecEnd::Cancelled => anyhow::bail!("중단됨"),
        ExecEnd::TimedOut => anyhow::bail!("check 타임아웃 — 판정 불가 (check_timeout_secs 또는 --timeout-scale 조정)"),
        ExecEnd::Done(s) => Ok(s.success()),
    }
}

/// solution/ 안 상대 경로 중 protected 경로(또는 그 하위)를 덮는 첫 항목.
/// Path::starts_with는 컴포넌트 단위라 "tests"가 "tests-extra"에 오매치되지 않는다
fn first_protected_overlap(solution: &Path, protected: &[String]) -> anyhow::Result<Option<String>> {
    fn walk(base: &Path, dir: &Path, protected: &[String]) -> anyhow::Result<Option<String>> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let rel = path.strip_prefix(base).expect("base 하위만 순회");
            if protected.iter().any(|p| rel.starts_with(p)) {
                return Ok(Some(rel.display().to_string()));
            }
            // let-chain 병합 — clippy 1.97(edition 2024)의 collapsible_if가 -D warnings에서 에러
            if entry.file_type()?.is_dir()
                && let Some(hit) = walk(base, &path, protected)?
            {
                return Ok(Some(hit));
            }
        }
        Ok(None)
    }
    walk(solution, solution, protected)
}

/// stdout용 한국어 표 — 마지막 줄이 `검증 n/m 통과` 요약
pub fn render_verify_table(records: &[VerifyRecord]) -> String {
    let mut out = String::new();
    out.push_str(&format!("{:<28} {:>7} {:>11}\n", "과제", "변별성", "해결가능성"));
    for r in records {
        let (d, s) = match &r.error {
            Some(_) => ("—", "—"),
            None => (mark(r.discriminates), mark(r.solvable)),
        };
        out.push_str(&format!("{:<28} {:>7} {:>11}", r.name, d, s));
        if let Some(e) = &r.error {
            out.push_str(&format!("  ({e})"));
        }
        out.push('\n');
    }
    let ok = records.iter().filter(|r| r.ok()).count();
    out.push_str(&format!(
        "검증 {ok}/{} 통과{}\n",
        records.len(),
        if ok == records.len() { "" } else { " — 게이트 실패" }
    ));
    out
}

fn mark(b: bool) -> &'static str {
    if b { "✓" } else { "✗" }
}
```

- [ ] **Step 6: 테스트 통과 확인**

Run: `cargo test --lib eval::verify`
Expected: 7개 테스트 전부 PASS (unix 6 + 표 1)

- [ ] **Step 7: 전체 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전부 통과 (기존 238+ 테스트 무회귀)

- [ ] **Step 8: Commit**

```bash
git add src/eval/verify.rs src/eval/mod.rs src/eval/sandbox.rs
git commit -m "feat(eval): 판정기 메타테스트 코어 + read+write 오버레이 (M6 §4, mtime 벡터 수선)"
```

---

### Task 2: CLI `--verify` 배선 (`src/main.rs`)

**Files:**
- Modify: `src/main.rs:29-45` (Command::Eval 정의), `src/main.rs:65-98` (run 재배선)

**Interfaces:**
- Consumes: Task 1의 `loco::eval::verify::{run_verify, render_verify_table, VerifyOptions}`
- Produces: `cargo run -- eval <tasks-dir> --verify [--timeout-scale F]` — 전 과제 양방향 ✓면 exit 0, 아니면 exit 1. `--repeats`/`--seed`와 상호 배타. Task 3이 이 커맨드로 solution을 검증한다

- [ ] **Step 1: Eval 서브커맨드에 플래그 추가**

`src/main.rs`의 `Command::Eval`에 필드 추가 (`timeout_scale` 아래):

```rust
        /// 판정기 메타테스트 — LLM 없이 과제별 변별성·해결가능성만 검증 (M6)
        #[arg(long, conflicts_with_all = ["repeats", "seed"])]
        verify: bool,
```

clap derive의 `conflicts_with_all`은 default_value_t가 있는 인자에 대해 **사용자가 명시 전달한 경우에만** 충돌을 발화한다 — 기본값과는 충돌하지 않으므로 `eval tasks/ --verify` 단독 호출이 유효하다.

- [ ] **Step 2: run() 재배선**

`run()`의 Eval 처리(65~98행)를 다음 구조로 바꾼다 — **verify 경로는 `OpenAiClient` 생성과 `resolve_model`을 건드리지 않는다** (스펙 §4: LLM·서버 불필요; 현행 코드는 eval 분기 앞 68행에서 `/v1/models`를 호출하므로 순서 이동이 핵심):

```rust
async fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    let config = Config::load_default()?;
    if let Some(Command::Eval { tasks_dir, repeats, seed, timeout_scale, verify }) = cli.command {
        // Duration::from_secs_f64는 음수/비유한 값뿐 아니라 u64::MAX초 초과에도
        // 패닉 — 하네스 에러(exit 1)로 선검증. 상한 1e6이면 300초 과제가 ~9.5년
        if !(timeout_scale.is_finite() && timeout_scale > 0.0 && timeout_scale <= 1_000_000.0) {
            anyhow::bail!("--timeout-scale은 0보다 크고 1000000 이하여야 합니다 (받은 값: {timeout_scale})");
        }
        if verify {
            // 메타테스트는 게이트 — LLM·서버 없이 동작해야 하므로 client를 만들지 않는다 (M6 §4)
            let opts = loco::eval::verify::VerifyOptions { tasks_dir, timeout_scale };
            let records = loco::eval::verify::run_verify(&opts).await?;
            print!("{}", loco::eval::verify::render_verify_table(&records));
            let all_ok = records.iter().all(|r| r.ok());
            return Ok(if all_ok { ExitCode::SUCCESS } else { ExitCode::from(1) });
        }
        if repeats == 0 {
            anyhow::bail!("--repeats는 1 이상이어야 합니다");
        }
        let client = OpenAiClient::new(&config.base_url, config.api_key.clone());
        let model = resolve_model(&client, &config).await?;
        let opts = loco::eval::EvalOptions {
            tasks_dir,
            repeats,
            base_seed: seed,
            timeout_scale,
            cancel_grace: std::time::Duration::from_secs(5),
        };
        let root = std::env::current_dir()?;
        let run = loco::eval::run_eval(&client, &config, &model, &opts, &root).await?;
        println!("{}", run.report.render_table());
        println!("리포트: {}", run.report_path.display());
        return Ok(if run.report.interrupted { ExitCode::from(1) } else { ExitCode::SUCCESS });
    }
    let client = OpenAiClient::new(&config.base_url, config.api_key.clone());
    let model = resolve_model(&client, &config).await?;
    match cli.prompt {
        Some(prompt) => run_oneshot(&client, &config, &model, &prompt, cli.auto).await,
        None => {
            run_repl(&client, &config, &model, cli.auto).await?;
            Ok(ExitCode::SUCCESS)
        }
    }
}
```

(빈 tasks_dir는 `load_tasks`가 에러를 내므로 `records`가 비는 경우는 없다 — `all(|r| r.ok())`의 공진리 걱정 불요.)

- [ ] **Step 3: 상호 배타 자동 테스트 (스펙 §7)**

`src/main.rs` 말미에 추가:

```rust
#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn verify_conflicts_with_repeats_and_seed() {
        assert!(Cli::try_parse_from(["loco", "eval", "tasks", "--verify", "--repeats", "2"]).is_err());
        assert!(Cli::try_parse_from(["loco", "eval", "tasks", "--verify", "--seed", "1"]).is_err());
        assert!(Cli::try_parse_from(["loco", "eval", "tasks", "--verify"]).is_ok(), "단독 --verify는 유효 (기본값과는 비충돌)");
        assert!(
            Cli::try_parse_from(["loco", "eval", "tasks", "--verify", "--timeout-scale", "2.0"]).is_ok(),
            "--timeout-scale은 verify와 병용 가능 (check 실행 시간에 관여)"
        );
    }
}
```

(`use clap::Parser;`는 파일 상단에 이미 있어 `use super::*;`로 trait이 함께 들어온다.)

Run: `cargo test --bin loco`
Expected: `verify_conflicts_with_repeats_and_seed` PASS

- [ ] **Step 4: 서버 불필요 스모크 (수동 확인)**

Run: LM Studio가 꺼진 상태(또는 무관하게)에서 `cargo run -- eval tasks/ --verify; echo "exit=$?"`
Expected: 서버 접속 시도 없이 12과제 검증 실행. **이 시점엔 solution/이 없으므로 12과제 전부 `(solution/ 없음 …)` 표기 + `검증 0/12 통과 — 게이트 실패`, exit=1** — 이것이 배선·게이트 방향의 정확성 확인이다

- [ ] **Step 5: 전체 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전부 통과

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat(cli): eval --verify 플래그 — LLM 없이 판정기 게이트 실행 (M6 §4)"
```

---

### Task 3: 레퍼런스 솔루션 12개 (`tasks/*/solution/`)

**Files:**
- Create: `tasks/<과제>/solution/…` — 12과제, 아래 스텝의 파일 목록
- Modify: `tasks/.gitattributes` (edit-crlf-file solution CRLF 핀)

**Interfaces:**
- Consumes: Task 2의 `cargo run -- eval tasks/ --verify` (감사 도구)
- Produces: 12과제 전부 `solution/` 보유 — Task 4(수선)와 성공 기준 1의 전제

아래 파일 내용은 임시 샌드박스에서 12과제 전부 `cargo test` 실측 검증을 거쳤다(2026-07-12 인벤토리). **오버레이는 파일 단위 덮어쓰기 — 각 파일은 부분 diff가 아니라 전문이다.** protected(`tests/`, `Cargo.toml`)를 건드리는 솔루션은 없고, 파일 삭제·이름변경이 필요한 과제도 없다. 신규 디렉터리 생성은 `mkdir -p` 후 파일 작성.

- [ ] **Step 1: 소스 수정 계열 7과제**

`tasks/add-function/solution/src/lib.rs`:

```rust
/// 정수 슬라이스의 중앙값. 짝수 길이는 가운데 두 값의 평균.
/// 입력은 비어 있지 않다고 가정한다.
pub fn median(xs: &[i64]) -> f64 {
    let mut sorted = xs.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2] as f64
    } else {
        (sorted[n / 2 - 1] as f64 + sorted[n / 2] as f64) / 2.0
    }
}
```

`tasks/chain-edits/solution/src/lib.rs`:

```rust
/// 재시도 상한
pub const MAX_RETRIES: u32 = 5;

/// 인사말
pub fn greeting() -> &'static str {
    "안녕하세요"
}

/// 재시도 대기시간(ms)
pub fn backoff_ms(attempt: u32) -> u64 {
    100 * 2u64.pow(attempt)
}
```

`tasks/fix-compile-error/solution/src/lib.rs`:

```rust
/// 단어들을 대문자로 바꿔 공백 하나로 잇는다
pub fn join_upper(words: &[&str]) -> String {
    let mut result = String::new();
    for w in words {
        result.push_str(&w.to_uppercase());
        result.push(' ');
    }
    result.trim_end().to_string()
}
```

`tasks/fix-failing-test/solution/src/lib.rs`:

```rust
/// 쉼표로 구분된 정수 목록의 합계. 공백 허용, 빈 문자열은 0.
pub fn sum_csv(input: &str) -> i64 {
    input.split(',').map(|p| p.trim().parse::<i64>().unwrap_or(0)).sum()
}

/// 목록의 최댓값. 파싱 불가 항목은 무시, 빈 목록이면 None.
pub fn max_csv(input: &str) -> Option<i64> {
    let mut best: Option<i64> = None;
    for part in input.split(',') {
        let Ok(v) = part.trim().parse::<i64>() else { continue };
        if best.is_none() || v > best.unwrap() {
            best = Some(v);
        }
    }
    best
}
```

`tasks/fix-off-by-one/solution/src/lib.rs`:

```rust
/// 1부터 n까지(포함) 정수의 합
pub fn sum_upto(n: u32) -> u32 {
    (1..=n).sum()
}
```

`tasks/implement-from-doc/solution/src/lib.rs`:

```rust
/// 런랭스 인코딩(RLE).
/// 연속으로 반복되는 문자를 `문자 + 반복횟수`로 축약한다.
/// 반복이 1회인 문자에도 횟수 1을 붙인다.
/// 예: "aaabbc" -> "a3b2c1", "" -> "".
/// 유니코드 문자 단위(char)로 처리한다.
pub fn rle(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    let Some(mut current) = chars.next() else {
        return out;
    };
    let mut count: usize = 1;
    for c in chars {
        if c == current {
            count += 1;
        } else {
            out.push(current);
            out.push_str(&count.to_string());
            current = c;
            count = 1;
        }
    }
    out.push(current);
    out.push_str(&count.to_string());
    out
}
```

`tasks/multiline-string-edit/solution/src/lib.rs` — **주의: fixture 원문에서 정확히 두 곳만 다르다** (`{user_name}`→`{username}`, `score: {score}\n` 줄 삽입). 이스케이프(`\\\"`, `C:\\data\\logs`)를 한 글자라도 흘리면 판정 테스트의 정확 일치 비교에서 실패하므로, 작성 후 fixture 원본과 diff로 두 곳만 바뀌었는지 확인할 것:

```rust
/// 사용자 요약 리포트 템플릿 (그대로 출력됨 — 이스케이프에 주의)
pub fn report_template() -> String {
    let mut t = String::new();
    t.push_str("== \"weekly\" report ==\n");
    t.push_str("user: {username}\n");
    t.push_str("said: \"hello, \\\"world\\\"\"\n");
    t.push_str("score: {score}\n");
    t.push_str("path: C:\\data\\logs\n");
    t.push_str("-- end of \"weekly\" report --\n");
    t
}
```

- [ ] **Step 2: answer 계열 2과제 + 다중·신규 파일 2과제**

`tasks/count-usages/solution/answer.txt` (신규 — 정답 4 = title 1 + slug 1 + compare 2; `pub use util::normalize;`는 선언이라 제외):

```
4
```

`tasks/find-definition/solution/answer.txt` (신규 — 정규형):

```
src/geometry.rs
```

`tasks/create-module/solution/src/lib.rs`:

```rust
pub mod shapes;
```

`tasks/create-module/solution/src/shapes.rs` (fixture에 없는 신규 파일 — 오버레이가 추가한다):

```rust
/// 직사각형 둘레 = 2*(w+h)
pub fn perimeter(w: u32, h: u32) -> u32 {
    2 * (w + h)
}
```

`tasks/rename-function/solution/src/cart.rs`:

```rust
/// 장바구니 합계 (수량 × 단가의 총합)
pub fn price_total(items: &[(u32, u32)]) -> u32 {
    items.iter().map(|(qty, price)| qty * price).sum()
}
```

`tasks/rename-function/solution/src/lib.rs`:

```rust
pub mod cart;
pub mod receipt;

pub use cart::price_total;
```

`tasks/rename-function/solution/src/receipt.rs`:

```rust
use crate::cart::price_total;

/// 영수증 한 줄 요약
pub fn summary(items: &[(u32, u32)]) -> String {
    format!("total: {}", price_total(items))
}

/// 배송비 포함 합계 (5000 미만이면 배송비 500)
pub fn with_shipping(items: &[(u32, u32)]) -> u32 {
    let t = price_total(items);
    if t < 5000 { t + 500 } else { t }
}
```

- [ ] **Step 3: edit-crlf-file — CRLF 바이트 정확 생성 + .gitattributes 핀**

에디터가 아니라 셸로 생성해 바이트를 보장한다 (`world`→`loco` 치환, 전 줄 CRLF, 파일 끝도 CRLF — 총 26바이트):

```bash
mkdir -p tasks/edit-crlf-file/solution/data
printf 'hello loco\r\ngoodbye moon\r\n' > tasks/edit-crlf-file/solution/data/greeting.txt
xxd tasks/edit-crlf-file/solution/data/greeting.txt
```

Expected: `0d0a`가 두 곳(각 줄 끝), 총 26바이트.

`tasks/.gitattributes`에 솔루션 경로 핀을 추가한다 (현재 fixture 한 줄뿐 — 이 항목이 없으면 checkout 시 git이 개행을 정규화해 바이트가 깨질 수 있다):

```
edit-crlf-file/fixture/data/greeting.txt -text
edit-crlf-file/solution/data/greeting.txt -text
```

- [ ] **Step 4: verify 실행 — §8 1단계 감사**

Run: `cargo run -- eval tasks/ --verify; echo "exit=$?"`
Expected: `검증 12/12 통과`, exit=0 — **정규형 솔루션은 수선 전 현행 판정기로도 통과가 기대치다** (스펙 §8 1단계). ✗ 항목이 나오면 그것이 감사 발견(솔루션 저작 오류 또는 변별성 회귀)이므로 해당 항목을 수정하고 재실행한다. 발견 내역(0건이면 0건)은 Task 6이 만드는 baselines.md v2 절 "판정기 변경 목록"에 기록한다 (스펙 §3).

- [ ] **Step 5: 루트 스위트 무회귀 확인**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전부 통과 (`tasks/`는 워크스페이스 비멤버 — solution 추가가 루트 빌드에 영향 없어야 정상)

- [ ] **Step 6: Commit**

```bash
git add tasks/
git commit -m "feat(tasks): 12과제 레퍼런스 솔루션 오버레이 — verify 12/12 (M6 §8-1)"
```

---

### Task 4: 판정기 수선 — 정규화 사다리 + 비변별 테스트 교체

**Files:**
- Modify: `tasks/find-definition/fixture/tests/check.rs` (전면 교체)
- Modify: `tasks/count-usages/fixture/tests/check.rs` (전면 교체)
- Modify: `tasks/fix-off-by-one/fixture/tests/sums.rs` (zero → two 교체)
- Test: 각 판정 테스트 파일 안의 사다리 단위 테스트 (verify 2단계가 실행)

**Interfaces:**
- Consumes: Task 3의 solution/ (수선이 변별성·해결가능성을 깨지 않는지 verify로 회귀 확인)
- Produces: v2 판정기 — 성공 기준 1의 "사다리 단위 테스트 포함" 요건 충족. Task 6이 변경 목록을 문서화

수선 목록은 스펙 §8 1단계 원칙대로 기준선 한계 7항 + M5 거짓 성공 분석에서 온 것이다(감사가 아니라 기지 결함). 픽스처의 `tests/`는 protected 판정 자산 — **이 수정이 바로 판정기 버전업(v1→v2)이며, 이후 v1 계열 수치와 비교 불가가 확정된다.** 사다리는 이 파일들 안에 구현한다(하네스 아님 — M6 §3); `#[test]`끼리는 헬퍼를 공유할 수 없는 통합 테스트 파일이므로 각 파일에 자체 사본을 둔다.

- [ ] **Step 1: find-definition 판정 테스트 교체**

`tasks/find-definition/fixture/tests/check.rs` 전체를 다음으로 교체:

```rust
/// answer.txt 정규화 사다리 (M6 §3): trim → 감싼 따옴표쌍 제거 → 경로 정규화.
/// 흔한 형식 변형(따옴표·후행 슬래시·후행 마침표·역슬래시·./ 접두)은 맞는 답으로
/// 인정하고, 여러 줄·산문은 정규화하지 않는다 — "한 줄로 저장" 지시 불이행은
/// 판정기 협소가 아니라 모델 실패다
fn normalize_path_answer(raw: &str) -> String {
    let s = raw.trim();
    if s.lines().count() > 1 {
        return s.to_string(); // 여러 줄은 그대로 두어 불일치로 실패시킨다
    }
    let s = strip_matched_quotes(s);
    let s = s.replace('\\', "/");
    let s = s.trim_start_matches("./");
    let s = s.trim_end_matches('/');
    let s = s.trim_end_matches('.');
    s.to_string()
}

/// 같은 따옴표(" ' `)로 감싼 경우에만 한 겹 벗긴다
fn strip_matched_quotes(s: &str) -> &str {
    for q in ['"', '\'', '`'] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            return &s[1..s.len() - 1];
        }
    }
    s
}

#[test]
fn answer_names_the_defining_file() {
    let answer = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/answer.txt"))
        .expect("answer.txt가 없습니다");
    assert_eq!(normalize_path_answer(&answer), "src/geometry.rs");
}

// --- 사다리 자체의 단위 테스트 (M6 §3·§7) — 에이전트 산출물과 무관한 고정 케이스.
// check 실행 시 항상 함께 돌아 eval·verify 양쪽에서 사다리를 검증한다.
// 메타테스트는 정규형 솔루션만 보므로, 변형 허용·거부는 이 테스트만이 담보한다

#[test]
fn ladder_accepts_common_variants() {
    for raw in [
        "src/geometry.rs",
        "  src/geometry.rs\n",
        "\"src/geometry.rs\"",
        "'src/geometry.rs'",
        "`src/geometry.rs`",
        "./src/geometry.rs",
        "src\\geometry.rs",
        "src/geometry.rs/",
        "src/geometry.rs.",
    ] {
        assert_eq!(normalize_path_answer(raw), "src/geometry.rs", "입력: {raw:?}");
    }
}

#[test]
fn ladder_rejects_prose_multiline_and_wrong_path() {
    for raw in [
        "정답은 src/geometry.rs 입니다",
        "src/geometry.rs\n(area 함수가 여기 있음)",
        "src/text.rs",
    ] {
        assert_ne!(normalize_path_answer(raw), "src/geometry.rs", "입력: {raw:?}");
    }
}
```

- [ ] **Step 2: count-usages 판정 테스트 교체**

`tasks/count-usages/fixture/tests/check.rs` 전체를 다음으로 교체:

```rust
/// answer.txt 정규화 사다리 (M6 §3): trim → 감싼 따옴표쌍 제거 → 정수 파싱·수치 비교.
/// 산문("4회")·여러 줄은 파싱 실패(None)로 남긴다 — 지시 불이행은 모델 실패다
fn parse_int_answer(raw: &str) -> Option<i64> {
    let s = raw.trim();
    if s.lines().count() > 1 {
        return None;
    }
    strip_matched_quotes(s).parse().ok()
}

/// 같은 따옴표(" ' `)로 감싼 경우에만 한 겹 벗긴다
fn strip_matched_quotes(s: &str) -> &str {
    for q in ['"', '\'', '`'] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            return &s[1..s.len() - 1];
        }
    }
    s
}

#[test]
fn answer_counts_call_sites() {
    let answer = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/answer.txt"))
        .expect("answer.txt가 없습니다");
    // title 1 + slug 1 + compare 2 (pub use 선언은 호출이 아님)
    assert_eq!(parse_int_answer(&answer), Some(4));
}

// --- 사다리 자체의 단위 테스트 (M6 §3·§7) — find-definition의 것과 같은 취지

#[test]
fn ladder_accepts_common_variants() {
    for raw in ["4", " 4\n", "\"4\"", "'4'", "`4`", "04"] {
        assert_eq!(parse_int_answer(raw), Some(4), "입력: {raw:?}");
    }
}

#[test]
fn ladder_rejects_prose_and_multiline() {
    for raw in ["4회", "호출은 4번", "4\n(설명)", ""] {
        assert_eq!(parse_int_answer(raw), None, "입력: {raw:?}");
    }
}
```

- [ ] **Step 3: fix-off-by-one 비변별 케이스 교체**

`tasks/fix-off-by-one/fixture/tests/sums.rs` 전체를 다음으로 교체 (`zero`는 버그 상태 `(1..n)`에서 `sum_upto(0)==0`으로 통과하는 비변별 케이스 — 기준선 한계 7항):

```rust
use fix_off_by_one::sum_upto;

#[test]
fn sums_inclusive() {
    assert_eq!(sum_upto(5), 15);
}

#[test]
fn one() {
    assert_eq!(sum_upto(1), 1);
}

// M6 §3: 기존 zero(0→0)는 배타 범위 버그((1..n))에서도 통과하는 비변별 케이스라
// 교체 — two는 버그 상태에서 1을 반환해 실패한다 (변별)
#[test]
fn two() {
    assert_eq!(sum_upto(2), 3);
}
```

- [ ] **Step 4: 12과제 개별 테스트 변별성 전수 감사 (스펙 §3 2항)**

기지 3건 외의 비변별 개별 테스트를 육안 감사한다. 각 과제의 fixture 버그 상태를 기준으로 `tests/*.rs`의 개별 `#[test]`마다 "이 케이스가 버그 상태에서도 통과하는가"를 판정하되, **교체 대상은 버그 대상 함수를 겨냥하면서 버그를 못 잡는 케이스만이다** — 버그와 무관한 함수를 검증하는 테스트(예: fix-failing-test의 `sum_csv` 계열 — 버그는 `max_csv`에 있음)는 커버리지 자산이므로 교체하지 않는다. 발견 시 fix-off-by-one `zero`→`two`와 같은 방식으로 변별 케이스로 교체하고, 결과(발견 0건이면 "추가 발견 0건")를 Task 6의 baselines.md v2 절 "판정기 변경 목록"에 기록한다.

- [ ] **Step 5: verify 회귀 게이트**

Run: `cargo run -- eval tasks/ --verify; echo "exit=$?"`
Expected: `검증 12/12 통과`, exit=0 — 수선이 세 과제의 변별성(픽스처에서 여전히 실패)·해결가능성(솔루션으로 여전히 통과)을 깨지 않았고, **사다리 단위 테스트가 2단계 check에서 함께 실행·통과**했다는 뜻 (성공 기준 1)

- [ ] **Step 6: 루트 스위트 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전부 통과

- [ ] **Step 7: Commit**

```bash
git add tasks/
git commit -m "feat(tasks): 판정기 v2 — answer 정규화 사다리(자체 테스트 동거)·비변별 케이스 교체 (M6 §3)"
```

(전수 감사에서 추가 교체가 나왔으면 해당 파일도 포함되도록 `tasks/` 전체를 스테이징한다.)

---

### Task 5: 이중 리포트 (`src/eval/report.rs`)

**Files:**
- Modify: `src/eval/report.rs` (TaskReport·Report 구조체, from_runs, render_table, 테스트), `src/eval/mod.rs:110-129` (Report 조립부)

**Interfaces:**
- Consumes: 기존 `RunRecord { passed: bool, outcome: RunOutcome, .. }`
- Produces: `TaskReport`·`Report`에 `passed_count: usize`, `passed_strict_count: usize`, `false_finish_count: usize` — report.json 신규 키(기존 키 불변), 표에 "엄격" 열. Task 7의 v2 측정·baselines.md가 이 필드를 읽는다

- [ ] **Step 1: 실패하는 테스트 작성**

`src/eval/report.rs`의 테스트 모듈에 추가 (기존 `fn run` 헬퍼 옆에 outcome 지정 헬퍼가 필요):

```rust
    fn run_with(passed: bool, outcome: RunOutcome) -> RunRecord {
        RunRecord { repeat: 0, seed: 0, passed, outcome, turns: 1, duration_secs: 1.0 }
    }

    #[test]
    fn strict_and_false_finish_counts() {
        let t = TaskReport::from_runs(
            "t".into(),
            vec![
                run_with(true, RunOutcome::Finished),   // passed + strict
                run_with(true, RunOutcome::MaxTurns),   // passed, 비엄격 (관대 채점의 대상)
                run_with(false, RunOutcome::Finished),  // 거짓 성공 finish
                run_with(false, RunOutcome::Timeout),   // 그냥 실패
            ],
        );
        assert_eq!(t.passed_count, 2);
        assert_eq!(t.passed_strict_count, 1, "Finished이면서 passed만");
        assert_eq!(t.false_finish_count, 1, "Finished인데 !passed만");
    }

    #[test]
    fn report_json_adds_count_fields_keeps_old_ones() {
        let v = serde_json::to_value(sample_report()).unwrap();
        // 신규 집계 — 과제별 + 최상위 (M6 §5, _count 접미사로 기존 passed/pass_rate와 충돌 회피)
        for key in ["passed_count", "passed_strict_count", "false_finish_count"] {
            assert!(v["tasks"][0].get(key).is_some(), "TaskReport에 {key}");
            assert!(v.get(key).is_some(), "Report 최상위에 {key}");
        }
        // 하위 호환 — 기존 키 이름·의미 불변
        assert!(v["tasks"][0].get("pass_rate").is_some());
        assert!(v.get("total_pass_rate").is_some());
        assert_eq!(v["tasks"][0]["runs"][0]["passed"], true);
    }

    #[test]
    fn table_shows_strict_column_and_false_finish_summary() {
        let tasks = vec![TaskReport::from_runs(
            "demo".into(),
            vec![run_with(true, RunOutcome::MaxTurns), run_with(false, RunOutcome::Finished)],
        )];
        let mut r = sample_report();
        r.total_pass_rate = Report::total_of(&tasks);
        r.passed_count = tasks.iter().map(|t| t.passed_count).sum();
        r.passed_strict_count = tasks.iter().map(|t| t.passed_strict_count).sum();
        r.false_finish_count = tasks.iter().map(|t| t.false_finish_count).sum();
        r.tasks = tasks;
        let table = r.render_table();
        assert!(table.contains("엄격"), "{table}");
        assert!(table.contains("거짓 성공 finish 1"), "{table}");
    }
```

- [ ] **Step 2: 컴파일 실패 확인**

Run: `cargo test --lib eval::report 2>&1 | head -20`
Expected: `passed_count` 필드 부재로 컴파일 에러

- [ ] **Step 3: 구현**

`TaskReport`에 필드 3개 추가(`pass_rate` 아래) + `from_runs` 계산 + `Report`에 동일 3필드(`total_pass_rate` 아래) 추가:

```rust
#[derive(Debug, Serialize)]
pub struct TaskReport {
    pub name: String,
    pub pass_rate: f64,
    /// check 통과 실행 수 (주 지표 — per-run passed의 합, M6 §5)
    pub passed_count: usize,
    /// outcome==finished 이면서 passed — 종료 규율 지표 (M6 §5)
    pub passed_strict_count: usize,
    /// outcome==finished 인데 !passed — "자신 있는 오답" 지표 (M6 §5)
    pub false_finish_count: usize,
    pub avg_turns: f64,
    pub avg_duration_secs: f64,
    pub runs: Vec<RunRecord>,
}

impl TaskReport {
    pub fn from_runs(name: String, runs: Vec<RunRecord>) -> TaskReport {
        let n = runs.len().max(1) as f64;
        TaskReport {
            pass_rate: runs.iter().filter(|r| r.passed).count() as f64 / n,
            passed_count: runs.iter().filter(|r| r.passed).count(),
            passed_strict_count: runs
                .iter()
                .filter(|r| r.passed && r.outcome == RunOutcome::Finished)
                .count(),
            false_finish_count: runs
                .iter()
                .filter(|r| !r.passed && r.outcome == RunOutcome::Finished)
                .count(),
            avg_turns: runs.iter().map(|r| r.turns as f64).sum::<f64>() / n,
            avg_duration_secs: runs.iter().map(|r| r.duration_secs).sum::<f64>() / n,
            name,
            runs,
        }
    }
}
```

`Report` 구조체 (`total_pass_rate` 아래에 삽입):

```rust
    pub total_pass_rate: f64,
    pub passed_count: usize,
    pub passed_strict_count: usize,
    pub false_finish_count: usize,
```

`render_table`을 다음으로 교체 (엄격 열 + 요약 줄 확장):

```rust
    pub fn render_table(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "{:<28} {:>7} {:>7} {:>9} {:>10}\n",
            "과제", "통과", "엄격", "평균 턴", "평균 시간"
        ));
        for t in &self.tasks {
            let n = t.runs.len();
            out.push_str(&format!(
                "{:<28} {:>7} {:>7} {:>9.1} {:>9.1}s\n",
                t.name,
                format!("{}/{n}", t.passed_count),
                format!("{}/{n}", t.passed_strict_count),
                t.avg_turns,
                t.avg_duration_secs
            ));
        }
        let total: usize = self.tasks.iter().map(|t| t.runs.len()).sum();
        let strict_rate = if total == 0 { 0.0 } else { self.passed_strict_count as f64 / total as f64 };
        out.push_str(&format!(
            "전체 통과율 {:.1}% ({}/{total}) · 엄격 {:.1}% ({}/{total}) · 거짓 성공 finish {} (시드 {}부터, timeout×{}){}\n",
            self.total_pass_rate * 100.0,
            self.passed_count,
            strict_rate * 100.0,
            self.passed_strict_count,
            self.false_finish_count,
            self.base_seed,
            self.timeout_scale,
            if self.interrupted { " — 중단됨(부분 결과)" } else { "" }
        ));
        out
    }
```

`src/eval/mod.rs`의 Report 조립부(110~129행)에 집계 추가 (`total_pass_rate` 줄 아래):

```rust
        passed_count: task_reports.iter().map(|t| t.passed_count).sum(),
        passed_strict_count: task_reports.iter().map(|t| t.passed_strict_count).sum(),
        false_finish_count: task_reports.iter().map(|t| t.false_finish_count).sum(),
```

report.rs 테스트의 `sample_report()`에도 같은 3필드를 추가해야 컴파일된다 (`total_pass_rate: Report::total_of(&tasks)` 옆에 `passed_count: 1, passed_strict_count: 1, false_finish_count: 0,`). 기존 `table_mentions_tasks_and_total` 테스트의 `"1/1"` 단언은 통과·엄격 두 열 모두에 매치되므로 그대로 둔다.

- [ ] **Step 4: 테스트 통과 확인**

Run: `cargo test --lib eval`
Expected: 신규 3개 포함 전부 PASS (mod.rs 통합 테스트의 표 검증 포함)

- [ ] **Step 5: 전체 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전부 통과

- [ ] **Step 6: Commit**

```bash
git add src/eval/report.rs src/eval/mod.rs
git commit -m "feat(eval): 이중 리포트 — passed/strict/false-finish 집계와 엄격 열 (M6 §5)"
```

---

### Task 6: 문서화 (CLAUDE.md · baselines.md 골격)

**Files:**
- Modify: `CLAUDE.md` (Commands 절, Architecture eval 항목, tasks/ 항목)
- Modify: `docs/baselines.md` (v2 기준선 절 골격 신설)

**Interfaces:**
- Consumes: Task 1~5의 실제 동작 (문서는 구현과 일치해야 함)
- Produces: Task 7(측정)이 채울 baselines.md v2 절 골격

- [ ] **Step 1: CLAUDE.md 갱신 (영문 유지)**

Commands 절에 추가:

```markdown
- `cargo run -- eval <tasks-dir> --verify` — judge meta-test gate (no LLM/server): per task, check must FAIL on the pristine fixture (discriminability) and PASS after overlaying `solution/` (solvability); exit 0 only when all tasks pass both. Mutually exclusive with `--repeats`/`--seed`; `--timeout-scale` still applies. Run after ANY change to `tasks/`.
```

Architecture의 `eval` 항목에 요지 반영: solution/ 오버레이 레이아웃(`task.toml` + `fixture/` + `solution/`, 변경·추가 파일만·삭제 표현 불가), verify가 protected(+implicit `.cargo`) 겹침을 거부, 트립와이어는 verify에서도 실행, report.json의 `passed_count`/`passed_strict_count`/`false_finish_count` 집계(기존 키 불변). `tasks/` 항목에 "every task carries a `solution/` reference overlay; answer-format judges normalize (trim → strip matched quotes → path/int canonicalize) and self-test that ladder in-file" 요지 추가.

- [ ] **Step 2: baselines.md v2 절 골격 신설**

`docs/baselines.md` 말미에 추가 — 수치는 Task 7이 채운다 (측정 전 골격이므로 "측정 예정" 상태를 명시해 두는 것이 정확하다; 이 시점의 커밋에는 빈 표가 들어간다):

```markdown
## v2 기준선 (M6 판정기 개편 후 — 측정 대기)

M6(스펙 2026-07-12-m6-eval-integrity-design.md)이 판정기를 수선했으므로 **이 절의 수치는
v1 계열(기준선·M5)과 직접 비교할 수 없다**. M7+ 마일스톤은 이 절을 비교 기준으로 사용한다.

### 판정기 변경 목록 (v1 → v2)

- find-definition: 정규화 사다리 확장 — 감싼 따옴표쌍(`"` `'` `` ` ``)·후행 슬래시·후행
  마침표 허용 (여러 줄·산문은 계속 거부). 사다리 단위 테스트가 판정 테스트 파일에 동거
- count-usages: trim·따옴표쌍 제거 후 정수 파싱·수치 비교 (산문 거부). 사다리 단위 테스트 동거
- fix-off-by-one: 비변별 `zero` 테스트를 변별 케이스(`two`)로 교체
- 하네스: verify 오버레이·protected 복원을 read+write로 — macOS `fs::copy`의 mtime
  보존이 스테일 테스트 바이너리 판정을 유발하는 벡터 수선 (판정기 자체는 아니나
  판정 경로 무결성 수정)
- (Task 3 솔루션 감사·Task 4 전수 감사 결과를 여기 기록 — 발견 0건이면 0건이라 명기)

### 측정 조건

기준선과 동일: 모델 단독 로드 ctx 8192, 로컬 config `max_output_tokens = 4096`, seed 0,
`--repeats 3`, 측정 중 병행 빌드 금지. 하네스: (측정 시점 커밋 해시 기입).

### 전체 통과율

| 모델 | 통과 | 엄격(Finished∧통과) | 거짓 성공 finish | report.json |
|---|---|---|---|---|
| google/gemma-4-e4b | (측정 예정) | | | |
| qwen/qwen3-vl-4b | (측정 예정) | | | |

### 과제별·관찰

(측정 후 기입)
```

- [ ] **Step 3: 문서-구현 일치 확인**

Run: `cargo run -- eval tasks/ --verify; echo "exit=$?"`
Expected: `검증 12/12 통과`, exit=0 (Task 3·4 완료 상태이므로) — CLAUDE.md의 커맨드 설명과 동작 일치 확인

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md docs/baselines.md
git commit -m "docs: M6 verify 커맨드·이중 리포트·v2 기준선 골격 문서화"
```

---

### Task 7: v2 기준선 재측정 (사용자 협조 필요)

**Files:**
- Modify: `docs/baselines.md` (v2 절 수치 확정)

**Interfaces:**
- Consumes: Task 1~6 완료 상태의 하네스 + LM Studio(사용자가 모델 로드·교체)
- Produces: v2 기준선 — M7+의 비교 기준

**주의: 이 태스크는 서브에이전트에 위임하지 말 것.** 모델 로드·교체는 사용자 협조가 필요하고, 측정 ~2시간 동안 cargo build/test를 병행하면 안 된다 (CLAUDE.md 측정 프로토콜).

- [ ] **Step 1: 측정 전 점검**

Run: `ls "${TMPDIR}/.cargo" 2>/dev/null; git status --short; cargo run -- eval tasks/ --verify | tail -1`
Expected: `.cargo` 없음(있으면 수동 제거), 워킹트리 클린, `검증 12/12 통과`

Run: `curl -s localhost:1234/api/v0/models | head -40`
Expected: gemma-4-e4b 단독 로드, context length ≥ 8192 — 아니면 사용자에게 로드 요청. 로컬 `./.loco/config.toml`의 `max_output_tokens = 4096` 확인

- [ ] **Step 2: gemma 측정**

Run: `cargo run -- eval tasks/ --repeats 3` (약 45~75분, 병행 작업 금지)
Expected: exit 0, `.loco/eval/<stamp>/report.json` 생성 — stamp 기록

- [ ] **Step 3: qwen 교체 후 측정**

사용자에게 gemma 언로드 + qwen3-vl-4b 로드(ctx 8192)를 요청한 뒤 (`model = ""`는 `/v1/models` 첫 항목 자동 선택 — 이전 모델을 언로드해야 새 모델이 잡힌다):

Run: `cargo run -- eval tasks/ --repeats 3`
Expected: exit 0, 두 번째 report.json — stamp 기록

- [ ] **Step 4: baselines.md v2 절 수치 확정**

두 report.json에서 `total_pass_rate`·`passed_count`·`passed_strict_count`·`false_finish_count`·과제별 `tasks[].passed_count`를 읽어 Task 6이 만든 골격 표를 채운다. 측정 시점 커밋 해시(`git rev-parse --short HEAD`)와 report.json 경로를 기입하고, 관찰 절에 최소한 다음을 기록: 거짓 성공 finish의 과제 분포(v1의 6건 대비 — find-definition·count-usages 계열이 사다리로 흡수됐는지, implement-from-doc 계열이 남았는지), 엄격 통과율과 관대 통과율의 격차(종료 규율 신호).

- [ ] **Step 5: Commit**

```bash
git add docs/baselines.md
git commit -m "docs: v2 기준선 측정 결과 (M6 판정기 개편 후, 양 모델 --repeats 3)"
```

---

## 완료 판정 (스펙 §2 성공 기준 대조)

1. `cargo run -- eval tasks/ --verify` 12/12 양방향 ✓ + answer 계열 사다리 단위 테스트가 2단계에서 실행·통과 — Task 3·4
2. 이중 리포트 반영 + 기존 필드 하위 호환 — Task 5
3. v2 기준선 양 모델 측정·문서화 — Task 6·7
4. 에이전트 코드 diff 0 (`git diff <M6 시작 커밋> --stat -- src/agent src/tools src/llm src/session.rs src/config.rs`가 비어야 함) + `cargo test`·`cargo clippy --all-targets -- -D warnings` 통과 — 매 태스크
