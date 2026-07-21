//! 판정기 메타테스트 (M6 스펙 §4) — LLM 없이 과제마다 두 성질을 검증한다:
//! 변별성(픽스처 원본에서 check 실패)과 해결가능성(solution/ 오버레이 후 check 통과).
//! 측정이 아니라 게이트 — report.json을 쓰지 않고 표와 종료 코드로만 보고한다.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Context;

use super::sandbox::{overlay_tree, Sandbox};
use super::task::{filter_tasks, load_tasks, Task};
use super::{cargo_tripwire, scaled_timeout, with_implicit_protected};
use crate::tools::exec::{exec_shell, ExecEnd};

pub struct VerifyOptions {
    pub tasks_dir: PathBuf,
    pub timeout_scale: f64,
    /// 과제 이름 정확 일치 필터 — 빈 벡터면 전체 실행 (M10 §7-1)
    pub filters: Vec<String>,
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
    let tasks = filter_tasks(load_tasks(&opts.tasks_dir)?, &opts.filters)?;
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
        VerifyOptions { tasks_dir: dir.to_path_buf(), timeout_scale: 1.0, filters: vec![] }
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
}
