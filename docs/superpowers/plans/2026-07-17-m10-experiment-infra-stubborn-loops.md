# M10 실험 인프라 + 완고 S/R 루프 개입 구현 계획

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 사전등록 실험 체계(프로토콜·지표 스크립트·무인 러너)를 깔고, 완고 S/R 루프 기계적 개입 2안(강제 전환 vs 디코딩 섭동)을 3암 표적 실험으로 비교해 승자를 main에 병합한다.

**Architecture:** 스펙 `docs/superpowers/specs/2026-07-17-m10-experiment-infra-stubborn-loops-design.md`(전문 리뷰 2R Ready=Yes). 인프라(--filter·스크립트·규약·러너)는 main에, 개입은 암 브랜치(`m10/arm-block`, `m10/arm-perturb`)에 각 1건씩 — 승자만 병합. 0단계 법의학이 개입 세부값(차단 임계 3회·섭동 0.7)을 확정한다.

**Tech Stack:** Rust(edition 2024, 신규 크레이트 금지), Python 3 표준 라이브러리(지표 스크립트), lms CLI(모델 교체), clap(기존 의존).

## Global Constraints

- 신규 크레이트 추가 금지(스펙 하드 제약) — python 스크립트도 표준 라이브러리만
- 모델-대면 텍스트(오류문·교정문) 영문, 사용자 CLI 메시지·Notice 한국어
- config 키 신설 금지 — 개입은 항상-켜짐, 암 구분은 브랜치 빌드
- report.json 스키마 변경 금지, `tasks/`·`tasks-large/` 픽스처 변경 금지
- 게이트(모든 커밋 전): `cargo test` + `cargo clippy --all-targets -- -D warnings`; tasks 트리를 건드리지 않아도 Task 2 이후 `cargo run -- eval tasks --verify`(12/12)·`cargo run -- eval tasks-large --verify`(3/3)는 암 브랜치 완성 시점(Task 6·7)과 실험 전 게이트에서 필수
- 측정 중 cargo build/test 병행 금지(CPU 경합) — 암 전환 빌드는 배치 사이에만
- 커밋은 conventional commits(제목 한국어 허용), push는 사용자 지시가 있을 때만
- Rust 파일 주석·문서는 이 저장소 관례(한국어 주석 + 스펙 절 참조)를 따른다

---

### Task 1: 0단계 트랜스크립트 법의학 (측정 비용 0, 코드 없음)

**Files:**
- Create: `docs/research/2026-07-17-m10-sr-loop-forensics.md`

**Interfaces:**
- Produces: 차단 임계(기본 3회)·섭동 온도(기본 0.7)의 확정 판정. **전제가 뒤집히면(반복이 문자 단위 복사가 아니면) 여기서 중단하고 스펙 개정·재리뷰로 회귀**(스펙 §3).

- [ ] **Step 1: 대상 15런 식별** — S/R 발생 런(스펙 §3: B:2·C:4·D:3·E:2·F:4):

```bash
for d in 20260717T015330Z 20260717T020632Z 20260717T022652Z 20260717T031126Z 20260717T032507Z 20260717T034527Z; do
  for f in .loco/eval/$d/run-*.jsonl; do
    n=$(python3 -c "
import json,sys
print(sum((json.loads(l).get('content') or '').count('search and replace are identical') for l in open('$f') if json.loads(l).get('kind')!='assistant'))")
    [ "$n" -gt 0 ] && echo "$d $(basename $f) SR=$n"
  done
done
```

Expected: 15개 런 나열. 배치 라벨(baselines.md "M9 행동 지표 비교" 표와 동일): A=015330Z(1단 gemma), B=020632Z(1단 ornith@8K), C=022652Z(1단 ornith@32K), D=031126Z(2단 gemma), E=032507Z(2단 ornith@8K), F=034527Z(2단 ornith@32K). A는 0건.

- [ ] **Step 2: 런별 정독·분류** — 각 런을 아래 덤프로 정독:

```bash
python3 - "$FILE" <<'EOF'
import json, sys
for i, l in enumerate(open(sys.argv[1])):
    e = json.loads(l)
    k, c = e.get("kind"), (e.get("content") or "")
    if k == "assistant":
        try:
            a = json.loads(c).get("action") or {}
            print(f"[{i}] ACT {a.get('tool')} {json.dumps(a.get('args'), ensure_ascii=False)[:160]}")
        except Exception:
            print(f"[{i}] ACT(unparsed) {c[:120]!r}")
    elif k in ("tool_result", "user"):
        print(f"[{i}] {k.upper()} {e.get('tool','')} {c[:180]!r}")
EOF
```

4개 축 기록: ⓐ 반복 호출이 직전 출력의 문자 단위 복사인가(E fm0의 "변주 후 회귀" 재현 여부 포함) ⓑ write_file 갈아타기 성공/실패와 실패 원인 ⓒ 대상 파일 전체 재작성이 8K에 들어가는 크기인가(`wc -l`로 fix-monthly `monthly.rs`=190줄, update-vat 대상 파일들 실측) ⓓ B/E uv1(뮤테이션 0회 cat 루프)·F fd1(write_file 반복 정지, passed=True) 판독 수록 — 리뷰 1R 결과의 근거 트랜스크립트 명기

- [ ] **Step 3: 노트 작성** — `docs/research/2026-07-17-m10-sr-loop-forensics.md`: 런별 표(배치/과제/S/R 횟수/복사 여부/갈아타기 결과) + 판정 절("차단 임계 3회 유지/변경, 섭동 0.7 유지/변경, 근거") + ⓓ 수록 절

- [ ] **Step 4: 커밋**

```bash
git add docs/research/2026-07-17-m10-sr-loop-forensics.md
git commit -m "docs: M10 0단계 — S/R 루프 법의학 15런, 개입 세부값 확정 (§3)"
```

---

### Task 2: `eval --filter` (main + eval/verify 공용)

**Files:**
- Modify: `src/main.rs` (Eval 서브커맨드 + 파싱 테스트)
- Modify: `src/eval/task.rs` (`filter_tasks` + 단위 테스트)
- Modify: `src/eval/mod.rs` (`EvalOptions.filters` + 적용)
- Modify: `src/eval/verify.rs` (`VerifyOptions.filters` + 적용)

**Interfaces:**
- Produces: `task::filter_tasks(tasks: Vec<Task>, filters: &[String]) -> anyhow::Result<Vec<Task>>`; `EvalOptions`/`VerifyOptions`에 `pub filters: Vec<String>` 필드. Task 9의 러너가 `cargo run -- eval tasks-large --filter fix-monthly-total --filter update-vat-rate --repeats 10 --seed 0`을 쓴다.

- [ ] **Step 1: 실패 테스트 작성** — `src/eval/task.rs`의 `#[cfg(test)] mod tests`에 추가(기존 테스트 관례 확인 후 동일 스타일):

```rust
#[test]
fn filter_selects_exact_names_and_rejects_any_unmatched_value() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["alpha", "beta"] {
        let d = dir.path().join(name);
        std::fs::create_dir_all(d.join("fixture")).unwrap();
        std::fs::write(d.join("fixture/keep.txt"), "x").unwrap();
        std::fs::write(
            d.join("task.toml"),
            "prompt = \"p\"\ncheck = \"true\"\nprotected = [\"keep.txt\"]\n",
        )
        .unwrap();
    }
    let tasks = load_tasks(dir.path()).unwrap();
    // 빈 필터 = 전체
    assert_eq!(filter_tasks(load_tasks(dir.path()).unwrap(), &[]).unwrap().len(), 2);
    // 정확 일치 선택
    let picked = filter_tasks(tasks, &["alpha".to_string()]).unwrap();
    assert_eq!(picked.len(), 1);
    assert_eq!(picked[0].name, "alpha");
    // 값별 비매치 → 오류 (오타 하나가 섞여도 전체 실패 — 침묵 축소 금지, 스펙 §7-1)
    let err = filter_tasks(
        load_tasks(dir.path()).unwrap(),
        &["alpha".to_string(), "betaa".to_string()],
    )
    .unwrap_err();
    assert!(err.to_string().contains("betaa"), "{err}");
}
```

주의: `task.toml`의 실제 필수 필드는 파일 상단 `TaskSpec` 정의를 읽고 맞출 것(위는 prompt/check/protected 가정 — 다르면 기존 테스트 픽스처 생성 헬퍼를 재사용).

- [ ] **Step 2: 실패 확인**

Run: `cargo test filter_selects -- --nocapture`
Expected: FAIL — `filter_tasks` 미정의(컴파일 에러)

- [ ] **Step 3: 구현** — `src/eval/task.rs`의 `load_tasks` 아래:

```rust
/// --filter 적용 (M10 §7-1): 전체 로드·검증 **후** 이름 정확 일치로 선별.
/// 각 필터 값이 최소 1과제와 일치해야 한다 — 오타 필터가 배치를 조용히
/// 축소한 채 완주하는 침묵 실패는 사전등록 규율과 정면 충돌
pub fn filter_tasks(tasks: Vec<Task>, filters: &[String]) -> anyhow::Result<Vec<Task>> {
    if filters.is_empty() {
        return Ok(tasks);
    }
    for f in filters {
        if !tasks.iter().any(|t| t.name == *f) {
            bail!("--filter '{f}'와 일치하는 과제가 없음");
        }
    }
    Ok(tasks.into_iter().filter(|t| filters.contains(&t.name)).collect())
}
```

- [ ] **Step 4: 배선** — ① `src/eval/mod.rs`: `EvalOptions`에 `pub filters: Vec<String>,` 추가, `run_eval` 첫 줄을 `let tasks = task::filter_tasks(task::load_tasks(&opts.tasks_dir)?, &opts.filters)?;`로. ② `src/eval/verify.rs`: `VerifyOptions`에 `pub filters: Vec<String>,` 추가, `run_verify`의 `load_tasks` 호출을 같은 모양으로 감쌈. ③ `src/main.rs` Eval에 `/// 과제 이름 정확 일치 필터 (반복 지정 가능, 값별 비매치는 오류)` doc과 함께 `#[arg(long)] filter: Vec<String>,` 추가, `VerifyOptions { tasks_dir, timeout_scale, filters: filter.clone() }`·`EvalOptions { .., filters: filter }`로 전달. ④ 기존 `EvalOptions {`/`VerifyOptions {` 생성자 전부에 `filters: vec![]` 추가:

```bash
grep -rn "EvalOptions {\|VerifyOptions {" src/
```

- [ ] **Step 5: clap 파싱 테스트** — `src/main.rs` 기존 `verify_conflicts_...` 옆에:

```rust
#[test]
fn filter_flag_repeats_and_combines_with_verify() {
    assert!(Cli::try_parse_from(["loco", "eval", "tasks", "--filter", "a", "--filter", "b"]).is_ok());
    assert!(Cli::try_parse_from(["loco", "eval", "tasks", "--verify", "--filter", "a"]).is_ok(), "표적 검증 조합 (§7-1)");
}
```

- [ ] **Step 6: eval 하네스 통합 테스트** — `src/eval/mod.rs`의 기존 `#[cfg(unix)]` 통합 테스트 블록(mod.rs:433 부근의 scripted client·`opts()` 헬퍼)에 추가. 픽스처 2과제 생성은 Step 1의 tempdir 패턴, 나머지는 기존 테스트 관례를 그대로 재사용:

```rust
#[cfg(unix)]
#[tokio::test]
async fn filter_runs_selected_task_only_but_validates_all_definitions() {
    // 케이스 1: 정상 과제 alpha·beta + opts.filters=["alpha"] → run_eval Ok,
    //   report.tasks.len()==1 ∧ report.tasks[0].name=="alpha" (§9 "필터 일치 과제만 수행")
    // 케이스 2: beta/task.toml을 `protected = []`로 깨뜨린 뒤 같은 filters →
    //   run_eval Err (로드 후 필터 — 비선택 과제의 정의 오류도 검출, §7-1)
}
```

- [ ] **Step 7: 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전부 PASS·경고 0

- [ ] **Step 8: 스모크** — 실제 트리에서:

```bash
cargo run -- eval tasks-large --verify --filter fix-monthly-total   # 1과제만 검증, exit 0
cargo run -- eval tasks-large --verify --filter no-such-task; echo "exit=$?"   # exit=1, 오류문에 no-such-task
```

- [ ] **Step 9: 커밋**

```bash
git add src/main.rs src/eval/task.rs src/eval/mod.rs src/eval/verify.rs
git commit -m "feat(eval): --filter 과제 필터 — 값별 비매치 오류, verify 조합 (M10 §7-1)"
```

---

### Task 3: `scripts/exp_metrics.py` (지표 추출기 + selftest)

**Files:**
- Create: `scripts/exp_metrics.py`

**Interfaces:**
- Produces: `python3 scripts/exp_metrics.py <stamp-dir>...` — 런별 TSV + 스탬프별 요약; `--selftest` exit 0. Task 9 러너·Task 10 판정이 소비.
- Consumes: 트랜스크립트 이벤트 스키마 `{"kind": system|user|assistant|tool_result, "tool": str?, "content": str}`(kind는 실제 M9 트랜스크립트로 검증할 것 — baselines.md "행동 지표 추출 레시피"의 스키마), report.json의 `tasks[].runs[]`(passed/outcome).

- [ ] **Step 1: 스크립트 작성** — 전체 내용:

```python
#!/usr/bin/env python3
"""loco 실험 지표 추출기 (M10 §7-3).

usage:
  python3 scripts/exp_metrics.py .loco/eval/<stamp> [...]   # 런별 TSV + 요약
  python3 scripts/exp_metrics.py --selftest                  # 내장 샘플 자기검증

assistant 이벤트는 마커 카운트에서 제외(모델이 교정문을 인용할 수 있음).
섭동 유도 규칙은 스펙 §5 원복 핀과 동일해야 한다: 스트릭은 액션 결과 턴
(tool_result)에서만 갱신, 'Error:' 비접두 본문(Denied: 포함)은 리셋.
표준 라이브러리만 사용(폐쇄망 개발 도구).
"""
import glob
import json
import os
import sys

SR_KEY = "Error: edit failed: search and replace are identical - no change would be made"
MARKS = {
    "sr_error": "search and replace are identical",
    "sr_correction": "Write the MODIFIED code in `replace`",
    "sr_block": "edit_file is disabled for this file",
    "repeat_corr": "repeating the same tool call",
    "finish_missing": "finish requires a string `summary`",
    "finish_args_corr": "Do not call finish with empty args again",
    "finish_nudge": "do not re-verify what you have already confirmed",
}
COLS = ["run", "outcome", "passed"] + list(MARKS) + [
    "sr_recovered", "sr_recovery_denom", "finish_missing_maxrun", "perturb_turns", "stop_cause",
]


def run_metrics(events):
    counts = dict.fromkeys(MARKS, 0)
    streak_key, streak, perturb_turns = None, 0, 0
    # (마커 발견 후 남은 edit/write 시도 기회) 목록 — "2시도 내 회복" 판정
    pending, recovered, denom = [], 0, 0
    fin_run, fin_max = 0, 0
    last_body = ""
    for e in events:
        kind, content = e.get("kind"), e.get("content") or ""
        if kind == "assistant":
            continue
        # 마커는 모든 비-assistant 이벤트에서 센다 — 교정 노트는 tool_result가
        # 아니라 별도 user 이벤트로 남는다 (baselines.md 추출 레시피)
        for k, m in MARKS.items():
            counts[k] += content.count(m)
        # 정지 원인 분류는 마지막 비-assistant 본문으로 — 인자누락 finish의
        # FINISH_ERR는 tool_result가 아니라 user 이벤트로 남는다 (리뷰 I-2)
        last_body = content
        # 인자누락 finish 최장 연속 (스펙 §7-3) — 다른 액션 결과(tool_result)가 끊는다
        if MARKS["finish_missing"] in content:
            fin_run += 1
            fin_max = max(fin_max, fin_run)
        elif kind == "tool_result":
            fin_run = 0
        if kind != "tool_result":
            continue
        tool = e.get("tool") or ""
        # 섭동 유도 (§5 핀): 이 결과 턴 직전 요청이 스트릭>=2 상태였으면 섭동 턴
        if streak_key == SR_KEY and streak >= 2:
            perturb_turns += 1
        if content.startswith("Error:"):
            key = content.split(".")[0]
            streak = streak + 1 if key == streak_key else 1
            streak_key = key
        else:
            streak_key, streak = None, 0
        # 회복 판정: S/R 오류 후 다음 2번의 edit/write 시도 안에 성공
        if tool in ("edit_file", "write_file"):
            ok = not (content.startswith("Error:") or content.startswith("Denied:"))
            still = []
            for tries in pending:
                if ok:
                    recovered += 1
                elif tries - 1 > 0:
                    still.append(tries - 1)
            pending = still
        if MARKS["sr_error"] in content:
            denom += 1
            pending.append(2)
    return counts, recovered, denom, fin_max, perturb_turns, last_body


def stop_cause(outcome, last_result):
    if outcome != "repetition_stop":
        return "-"
    if MARKS["sr_error"] in last_result or MARKS["sr_block"] in last_result:
        return "sr"
    if MARKS["finish_missing"] in last_result:
        return "finish"
    return "other"


def report_index(stamp_dir):
    idx = {}
    path = os.path.join(stamp_dir, "report.json")
    if not os.path.exists(path):
        return idx
    rep = json.load(open(path))
    for t in rep.get("tasks", []):
        for r in t.get("runs", []):
            idx[f"run-{t['name']}-{r['repeat']}"] = (r.get("outcome", "?"), r.get("passed"))
    return idx


def process(stamp_dir):
    idx = report_index(stamp_dir)
    print(f"# {stamp_dir}")
    print("\t".join(COLS))
    totals = dict.fromkeys(MARKS, 0)
    total_rec, total_den = 0, 0
    stops = {"sr": 0, "finish": 0, "other": 0}
    for path in sorted(glob.glob(os.path.join(stamp_dir, "run-*.jsonl"))):
        events = [json.loads(l) for l in open(path)]
        counts, rec, den, fin_max, perturb, last = run_metrics(events)
        name = os.path.basename(path).removesuffix(".jsonl")
        outcome, passed = idx.get(name, ("?", None))
        cause = stop_cause(outcome, last)
        if cause != "-":
            stops[cause] += 1
        for k in MARKS:
            totals[k] += counts[k]
        total_rec += rec
        total_den += den
        row = [name, outcome, str(passed)] + [str(counts[k]) for k in MARKS]
        row += [str(rec), str(den), str(fin_max), str(perturb), cause]
        print("\t".join(row))
    marks = " ".join(f"{k}={totals[k]}" for k in MARKS)
    print(f"# summary {marks} recovered={total_rec}/{total_den} "
          f"stops sr={stops['sr']} finish={stops['finish']} other={stops['other']}")


def selftest():
    def ev(kind, content, tool=None):
        e = {"kind": kind, "content": content}
        if tool:
            e["tool"] = tool
        return e

    sr = SR_KEY + ". Put the code as it is NOW in `search`, and the code AFTER your change in `replace`."
    # S/R 2회(스트릭 2 도달) → 섭동 하 1턴 → write_file 성공(회복·스트릭 해제)
    events = [
        ev("assistant", "따라 말한 교정문: Write the MODIFIED code in `replace`"),  # 제외 확인
        ev("tool_result", sr, "edit_file"),
        ev("tool_result", sr, "edit_file"),
        ev("tool_result", "wrote file", "write_file"),
        ev("tool_result", "exit code: 0\nok", "run_command"),
    ]
    counts, rec, den, fin_max, perturb, _ = run_metrics(events)
    assert counts["sr_error"] == 2, counts
    assert (rec, den) == (2, 2), (rec, den)  # 두 오류 모두 다음 시도(성공)로 회복
    assert fin_max == 0, fin_max
    assert perturb == 1, perturb  # 스트릭 2 이후의 결과 턴은 write_file 1턴뿐
    # Denied: 는 스트릭 리셋 — 섭동 비유지
    events2 = [
        ev("tool_result", sr, "edit_file"),
        ev("tool_result", sr, "edit_file"),
        ev("tool_result", "Denied: policy", "edit_file"),
        ev("tool_result", sr, "edit_file"),
    ]
    _, _, _, _, perturb2, _ = run_metrics(events2)
    assert perturb2 == 1, perturb2  # Denied 턴 1회만 섭동 하 — 리셋 후 재축적 전
    # finish 정지 분류 (리뷰 I-2): FINISH_ERR는 user 이벤트로 남는다
    fin = "Error: finish requires a string `summary` argument, e.g. ..."
    events3 = [ev("tool_result", sr, "edit_file"), ev("user", fin), ev("user", fin)]
    _, _, _, fin_max3, _, last3 = run_metrics(events3)
    assert fin_max3 == 2, fin_max3
    assert stop_cause("repetition_stop", last3) == "finish"
    assert stop_cause("repetition_stop", sr) == "sr"
    assert stop_cause("finished", sr) == "-"
    print("selftest ok")


if __name__ == "__main__":
    if len(sys.argv) >= 2 and sys.argv[1] == "--selftest":
        selftest()
    elif len(sys.argv) >= 2:
        for d in sys.argv[1:]:
            process(d)
    else:
        sys.exit(__doc__)  # 인자 없는 호출은 실패가 의도 — usage를 stderr로, exit 1
```

- [ ] **Step 2: selftest 통과 확인**

Run: `python3 scripts/exp_metrics.py --selftest`
Expected: `selftest ok`, exit 0

- [ ] **Step 3: 실데이터 스모크 + 스키마 검증** — M9 스탬프로 실행하고, E fm0 행이 이 대화에서 확정된 실측(sr_error=6, sr발 정지)과 일치하는지 확인. **불일치하면 트랜스크립트 실스키마(kind·tool 필드명)를 정독해 스크립트를 맞출 것**:

```bash
python3 scripts/exp_metrics.py .loco/eval/20260717T032507Z | head -15
```

Expected: `run-fix-monthly-total-0` 행에 sr_error=6·stop_cause=sr, `run-update-vat-rate-1` 행에 sr_error=0·stop_cause=other, 말미에 `# summary` 행. 1단 ornith@32K 배치(022652Z)의 `run-find-definition-large-1`에서 finish_missing_maxrun=6도 교차 확인(baselines.md "최장 6회 — C fd1")

- [ ] **Step 4: 커밋**

```bash
git add scripts/exp_metrics.py
git commit -m "feat(scripts): exp_metrics.py — 행동 지표 추출기 + selftest (M10 §7-3)"
```

---

### Task 4: 실험 규약 문서 (`docs/experiments/`)

**Files:**
- Create: `docs/experiments/PROTOCOL.md`
- Create: `docs/experiments/TEMPLATE.md`

**Interfaces:**
- Produces: 사전등록 양식과 수행 규칙 — Task 8(사전등록)·Task 9(러너)가 그대로 따른다.

- [ ] **Step 1: PROTOCOL.md 작성** — 전체 내용:

```markdown
# loco 실험 프로토콜 (M10 §7-4)

GPU 시간(측정 배치)을 쓰는 모든 실험에 적용된다.

1. **사전등록 없이는 배치를 돌리지 않는다.** 사전등록 = 가설·조건(암)·표본·
   지표·판정 규칙·중단 규칙·시간 예산이 담긴 `pre-registration.md`가 사용자
   승인을 받은 상태. 판정 규칙은 데이터를 보기 전에 확정한다.
2. **측정 중 cargo build/test 병행 금지**(CPU 경합이 타이밍 판정을 흔든다 —
   CLAUDE.md). 암 전환에 필요한 체크아웃·빌드는 배치 사이에만.
3. **소표본 규칙**(M9 스펙 §2 승계): 관심 현상 발생 런이 배치당 3런 미만이면
   비율 대신 발생 런 전수를 나열하고 방향으로 판정한다.
4. **배치 전 게이트**: ① 두 tasks 트리 `--verify` 통과(12/12·3/3) ② 대상
   모델 로드·언로드(`lms unload --all` → `lms load <model> --context-length
   <N>`) ③ `curl -s localhost:1234/api/v0/models`로 로드 상태·컨텍스트 길이
   검증 — `model = ""` 자동 선택은 로드된 첫 모델을 잡으므로 언로드가 필수.
5. **재현 가능성 기록**: 배치마다 eval 스탬프 ↔ `git rev-parse HEAD` 쌍,
   lms 확인 출력, 사용한 config 값을 report.md에 기재. report.json은 암을
   자증하지 못한다(loco_version이 전 브랜치 동일).
6. **중단 규칙 준수**: 사전등록에 적힌 그대로. Ctrl+C 부분 리포트는 폐기하고
   해당 배치 재수행.
7. **판정은 사람이**: 러너는 report.md 초안(지표 표 + 사전등록 판정 규칙의
   기계적 적용)까지만. 최종 판정·병합 결정은 사용자 리뷰를 거친다.
```

- [ ] **Step 2: TEMPLATE.md 작성** — 전체 내용:

```markdown
# 실험 사전등록: <제목>

- 날짜/디렉토리: docs/experiments/YYYY-MM-DD-<slug>/
- 스펙 근거: <스펙 경로·절>
- 상태: 초안 | 승인됨(승인일) | 수행됨(report.md)

## 가설
H1: ... (반증 가능한 형태로)

## 조건 (암)
| 암 | 브랜치/커밋 | 내용 |
|---|---|---|

## 표본
모델·컨텍스트·과제(--filter)·반복(--repeats/--seed)·총 런 수.
제외가 있으면 근거를 적는다.

## 지표
주 지표(판정을 결정)·보조 지표(부작용 감시). 추출 방법(exp_metrics.py 열 이름).

## 판정 규칙 (데이터 보기 전 확정)
승자 규칙·동률 규칙·전패 시 처분. 소표본 규칙 적용 기준.

## 중단 규칙
하네스 오류·부분 리포트·시간 초과 시 행동.

## 시간 예산 (상한치)
배치별 상한과 총합. 산정 근거(실측 평균 × 여유분).
```

- [ ] **Step 3: 커밋**

```bash
git add docs/experiments/PROTOCOL.md docs/experiments/TEMPLATE.md
git commit -m "docs: 실험 프로토콜·사전등록 템플릿 (M10 §7-4)"
```

---

### Task 5: 무인 수행 러너 에이전트 정의

**Files:**
- Create: `.claude/agents/loco-experiment-runner.md`

**Interfaces:**
- Produces: Claude Code 서브에이전트 정의 — Task 9에서 `Agent(subagent_type: "loco-experiment-runner", prompt: "<사전등록 경로> 수행")`으로 호출.

- [ ] **Step 1: 에이전트 정의 작성** — 전체 내용:

```markdown
---
name: loco-experiment-runner
description: 승인된 사전등록 문서에 따라 loco 측정 배치를 무인 수행한다 — lms 모델 교체·배치 전 게이트·순차 수행·지표 추출·report.md 초안까지. 사전등록 없는 배치는 수행하지 않는다.
tools: Bash, Read, Write, Glob, Grep
---

당신은 loco 실험 수행자다. 입력 프롬프트에 사전등록 문서 경로가 온다.

## 절차 (docs/experiments/PROTOCOL.md 준수)
1. 사전등록 문서를 읽는다. 상태가 "승인됨"이 아니면 즉시 중단·보고.
2. 배치 전 게이트: 해당 암 브랜치 체크아웃(`git checkout <branch>`) →
   `cargo build` → `cargo run -- eval tasks --verify`(12/12)와
   `cargo run -- eval tasks-large --verify`(3/3) → 사전등록의 배치별 config
   값(context_tokens·max_output_tokens·command_timeout_secs)을
   `./.loco/config.toml`에 기록 → `lms unload --all` →
   `lms load <모델> --context-length <로드값>` →
   `curl -s localhost:1234/api/v0/models`로 로드·컨텍스트 검증.
   배치 후 report.json `effective_config`가 사전등록 값과 일치하는지 대조 —
   직전 배치의 config 잔재는 GPU 시간 전체를 무효화한다.
3. 사전등록의 배치 명령을 그대로 실행(예: `cargo run -- eval tasks-large
   --filter fix-monthly-total --filter update-vat-rate --repeats 10 --seed 0`).
   실행 직후 `git rev-parse HEAD`와 eval 스탬프 경로를 기록.
4. 전 배치 종료 후 `python3 scripts/exp_metrics.py <스탬프>...`를 돌려
   실험 디렉토리에 `report.md` 초안 작성: 배치↔커밋↔스탬프 표, 지표 표
   (런별 TSV 원문 포함), 사전등록 판정 규칙의 기계적 적용 결과, 이상 징후.
5. 중단 규칙: LLM 에러·부분 리포트 시 해당 배치 1회 재수행, 재실패면 전체
   중단하고 원인(마지막 오류 출력 포함)을 report.md에 기록 후 종료.

## 금지
- 제품 코드·픽스처·사전등록 문서 수정. 커밋 생성. git push.
- 측정 중 cargo build/test 병행(체크아웃·빌드는 배치 사이에만).
- 사전등록에 없는 배치·조건 추가, 판정 규칙 변경.
- 최종 판정 선언 — 초안까지만, 판정은 사용자 몫.
```

- [ ] **Step 2: 커밋**

```bash
git add .claude/agents/loco-experiment-runner.md
git commit -m "feat(agents): loco-experiment-runner — 사전등록 기반 무인 측정 수행자 (M10 §7-5)"
```

---

### Task 6: 암② S/R 강제 전환 (`m10/base` 포인터 + `m10/arm-block` 브랜치)

**Files:**
- Create: `src/agent/sr_block.rs`
- Modify: `src/agent/mod.rs` (모듈 선언·run() 배선·Scripted 테스트)

**Interfaces:**
- Consumes: `repetition::SR_KEY`(오류 첫 문장 상수), run() 루프의 게이트 거부 처리 패턴(mod.rs:310-332)
- Produces: `SrBlock::new()`, `SrBlock::is_blocked(&self, args: &serde_json::Value) -> bool`, `SrBlock::record_sr_error(&mut self, args: &serde_json::Value)`, `sr_block::SR_BLOCK_ERR: &str`, `sr_block::SR_BLOCK_THRESHOLD: usize = 3`(Task 1이 임계를 바꾸면 이 상수만 변경)

- [ ] **Step 1: 브랜치 준비** — main 최신 커밋(Task 5까지 포함)에 base 포인터를 만들고 암 브랜치 생성:

```bash
git branch m10/base && git checkout -b m10/arm-block m10/base
```

- [ ] **Step 2: SrBlock 단위 테스트 작성** — 먼저 `src/agent/mod.rs`의 기존 모듈 선언부(`mod repetition;` 근처)에 `mod sr_block;`을 추가한다(미선언 파일은 컴파일 대상이 아니라 실패 확인이 안 된다 — 리뷰 M-1). 그 다음 `src/agent/sr_block.rs` 신규 파일 하단에:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn args(path: &str) -> serde_json::Value {
        serde_json::json!({"path": path, "search": "a", "replace": "a"})
    }

    #[test]
    fn blocks_at_threshold_and_merges_path_spellings() {
        let mut b = SrBlock::new();
        b.record_sr_error(&args("./src/x.rs"));
        b.record_sr_error(&args("src/x.rs"));
        assert!(!b.is_blocked(&args("src/x.rs")), "2회는 비차단");
        b.record_sr_error(&args("src/./x.rs"));
        assert!(b.is_blocked(&args("src/x.rs")), "표기 변형 3회 합산 → 차단");
        assert!(b.is_blocked(&args("./src/x.rs")), "차단 조회도 정규화 — 철자 우회 불가");
        assert!(!b.is_blocked(&args("src/y.rs")), "다른 파일 독립");
    }

    #[test]
    fn parent_dir_components_normalize_lexically() {
        let mut b = SrBlock::new();
        for _ in 0..SR_BLOCK_THRESHOLD {
            b.record_sr_error(&args("src/sub/../x.rs"));
        }
        assert!(b.is_blocked(&args("src/x.rs")));
    }

    #[test]
    fn missing_or_non_string_path_is_ignored() {
        let mut b = SrBlock::new();
        for _ in 0..SR_BLOCK_THRESHOLD {
            b.record_sr_error(&serde_json::json!({}));
            b.record_sr_error(&serde_json::json!({"path": 3}));
        }
        assert!(!b.is_blocked(&serde_json::json!({})), "BadArgs는 카운트·차단 제외 (§4)");
    }

    #[test]
    fn block_error_first_sentence_is_streak_key_stable() {
        // 오류 스트릭 키 = 첫 '.'까지 — 내부 마침표가 생기면 키가 흔들린다 (M10 §4)
        assert_eq!(
            SR_BLOCK_ERR.split('.').next().unwrap(),
            "Error: edit_file is disabled for this file after repeated identical search/replace failures"
        );
    }
}
```

- [ ] **Step 3: 실패 확인**

Run: `cargo test sr_block`
Expected: FAIL — 모듈 미존재(컴파일 에러)

- [ ] **Step 4: SrBlock 구현** — `src/agent/sr_block.rs` 상단:

```rust
//! edit_file S/R 강제 전환 (M10 §4) — 파일별 S/R 오류 누적 3회 시 그 파일의
//! edit_file 디스패치를 차단하고 write_file 재작성을 지시한다. 텍스트 교정
//! 3층(도구 처방→SR_CORRECTION→REPEAT_CORRECTION) 위의 4번째 단.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path};

/// 차단 임계 — 1회차 도구 처방·2회차 SR_CORRECTION이 소진된 시점 (§4, 0단계 확정값)
pub const SR_BLOCK_THRESHOLD: usize = 3;

/// 모델 대상 — 영어. 첫 문장(첫 '.'까지)은 오류 스트릭 키로 안정 유지할 것
pub const SR_BLOCK_ERR: &str = "Error: edit_file is disabled for this file after repeated identical search/replace failures. \
Rewrite the complete file with write_file, applying your fix.";

pub struct SrBlock {
    counts: HashMap<String, usize>,
    blocked: HashSet<String>,
}

impl SrBlock {
    pub fn new() -> Self {
        Self { counts: HashMap::new(), blocked: HashSet::new() }
    }

    /// 렉시컬 정규화 (§4): CurDir 제거·ParentDir 팝 — 표기 변형의 카운트 분산과
    /// 차단 철자 우회를 막는다. 파일시스템 조회는 하지 않는다
    fn normalize(path: &str) -> String {
        let mut parts: Vec<String> = Vec::new();
        for c in Path::new(path).components() {
            match c {
                Component::CurDir => {}
                Component::ParentDir => {
                    parts.pop();
                }
                other => parts.push(other.as_os_str().to_string_lossy().into_owned()),
            }
        }
        parts.join("/")
    }

    fn key(args: &serde_json::Value) -> Option<String> {
        args.get("path").and_then(|v| v.as_str()).map(Self::normalize)
    }

    /// path 부재·비문자열(BadArgs)은 판정 제외 (§4)
    pub fn is_blocked(&self, args: &serde_json::Value) -> bool {
        Self::key(args).is_some_and(|k| self.blocked.contains(&k))
    }

    pub fn record_sr_error(&mut self, args: &serde_json::Value) {
        let Some(k) = Self::key(args) else { return };
        let n = self.counts.entry(k.clone()).or_insert(0);
        *n += 1;
        if *n >= SR_BLOCK_THRESHOLD {
            self.blocked.insert(k);
        }
    }
}
```

(모듈 선언은 Step 2에서 이미 추가됨.)

- [ ] **Step 5: 단위 테스트 통과 확인**

Run: `cargo test sr_block`
Expected: 4개 PASS

- [ ] **Step 6: run() 배선** — `src/agent/mod.rs`. ① 지역 상태(run() 상단, `let mut finish_nudge = ...` 옆):

```rust
let mut sr_block = sr_block::SrBlock::new();
```

② **차단 검사 — 게이트·preview보다 앞**(§4·리뷰 I3). `on_event(AgentEvent::Action { .. })` 직후, `let gate_preview = ...` 앞에 삽입:

```rust
// M10 §4: 차단된 파일의 edit_file은 preview·게이트·디스패치 없이 거부.
// 차단 결과도 게이트 거부와 같은 경로로 계수·주입된다 (윈도 백스톱은
// 동일 인자 반복에만 성립 — 변주 시 상한은 max_turns, §4 상호작용)
if turn.action.tool == "edit_file" && sr_block.is_blocked(&turn.action.args) {
    on_event(AgentEvent::Notice("(edit_file 반복 실패 — write_file 전환 강제)".to_string()));
    let body = sr_block::SR_BLOCK_ERR;
    finish_missing_streak = 0; // 툴 결과를 낸 액션 턴 — 게이트 거부와 동급 (§4)
    let (mut note, stop) = self.track_and_note(&mut tracker, &turn, body, on_event);
    if !stop && let Some(nudge) = finish_nudge.on_turn(finish_nudge::TurnEvent::MutationAttempt) {
        on_event(AgentEvent::Notice("(검증 완료 후 재확인 반복 — finish 유도 주입)".to_string()));
        note = merge_note(note, nudge);
    }
    session.push_tool_result(&turn.action.tool, &turn.action.args, body, note.as_deref());
    if stop {
        return Ok(AgentOutcome::RepetitionStop);
    }
    turns += 1;
    continue;
}
```

③ **카운트 — 디스패치 결과 확정 직후**(`let body = match dispatched {...};` 뒤):

```rust
// M10 §4: S/R 오류(첫 문장 = SR_KEY)를 파일별 누적 — 연속 불요
if turn.action.tool == "edit_file" && body.starts_with(repetition::SR_KEY) {
    sr_block.record_sr_error(&turn.action.args);
}
```

- [ ] **Step 7: Scripted 통합 테스트** — `src/agent/mod.rs` tests 모듈에 추가(기존 헬퍼 `make_guided_agent`/`turn`/`finish`/`session_contains`/`run_quiet` 재사용):

```rust
#[tokio::test]
async fn fourth_edit_after_three_sr_errors_is_blocked_without_dispatch() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.rs"), "x\n").unwrap();
    let sr = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "x"}));
    // 표기 변형 3회 → 4번째는 인자가 유효(성공 가능)해도 차단 — 디스패치 안 됨을 파일 불변으로 증명
    let sr2 = turn("edit_file", serde_json::json!({"path": "./f.rs", "search": "x", "replace": "x"}));
    let fix = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "y"}));
    let script = Scripted::new(vec![ok(&sr), ok(&sr2), ok(&sr), ok(&fix), ok(&finish("done"))]);
    let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    // PanicApprover(기존 테스트 관례, mod.rs:1153 부근)로 실행 — S/R 동일 3회는 preview
    // Err라 게이트 미도달, 4번째(유효 인자)는 차단이 preview·게이트보다 앞임을 증명:
    // 게이트 뒤에 잘못 배선하면 approver가 호출돼 패닉한다 (§9 "preview·게이트도 미실행")
    let outcome = agent.run(&mut session, "x", &mut PanicApprover, &mut |_| {}).await.unwrap();
    assert!(matches!(outcome, AgentOutcome::Finished(_)), "{outcome:?}");
    assert!(session_contains(&session, "edit_file is disabled for this file"));
    assert_eq!(std::fs::read_to_string(dir.path().join("f.rs")).unwrap(), "x\n", "차단 — 편집 미실행");
}

#[tokio::test]
async fn two_sr_errors_do_not_block_and_other_file_stays_editable() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.rs"), "x\n").unwrap();
    std::fs::write(dir.path().join("g.rs"), "x\n").unwrap();
    let sr = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "x"}));
    // f.rs 2회 + read_file 개입(비연속 확인) 후 f.rs 3번째 시도는 아직 디스패치됨(성공),
    // g.rs는 카운트 3 미달로 정상 편집
    let read = turn("read_file", serde_json::json!({"path": "g.rs"}));
    let fix_f = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "y"}));
    let fix_g = turn("edit_file", serde_json::json!({"path": "g.rs", "search": "x", "replace": "z"}));
    // finish 2개: 편집 성공으로 mutated_since_verify=true → 1차 finish는 VERIFY_NUDGE 반려 (기존 테스트 관례)
    let script = Scripted::new(vec![ok(&sr), ok(&read), ok(&sr), ok(&fix_f), ok(&fix_g), ok(&finish("d")), ok(&finish("d"))]);
    let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
    assert!(matches!(outcome, AgentOutcome::Finished(_)), "{outcome:?}");
    assert_eq!(std::fs::read_to_string(dir.path().join("f.rs")).unwrap(), "y\n", "2회는 차단 아님");
    assert_eq!(std::fs::read_to_string(dir.path().join("g.rs")).unwrap(), "z\n", "타 파일 독립");
}

#[tokio::test]
async fn blocked_identical_calls_reach_repetition_stop_backstop() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.rs"), "x\n").unwrap();
    let sr = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "x"}));
    // 동일 인자: SR 오류 3회(윈도 (키,SR해시) 3히트 — 3회째 REPEAT_CORRECTION) 후
    // 차단 본문으로 결과가 바뀌어 (키,차단해시) 새 항목 — 5히트째(8번째 호출)에 정지 (§4 백스톱)
    let script = Scripted::new((0..8).map(|_| ok(&sr)).collect());
    let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
    assert!(matches!(outcome, AgentOutcome::RepetitionStop), "{outcome:?}");
}
```

- [ ] **Step 8: 게이트 + 검증 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo run -- eval tasks --verify && cargo run -- eval tasks-large --verify`
Expected: 전부 PASS, 12/12·3/3

- [ ] **Step 9: 커밋 (arm-block 브랜치)**

```bash
git add src/agent/sr_block.rs src/agent/mod.rs
git commit -m "feat(agent): S/R 3누적 시 edit_file 차단 — write_file 강제 전환 (M10 §4, 암②)"
```

---

### Task 7: 암③ 디코딩 섭동 (`m10/arm-perturb` 브랜치)

**Files:**
- Modify: `src/agent/repetition.rs` (`sr_streak` 접근자 + 테스트)
- Modify: `src/agent/mod.rs` (`temperature_override` 필드·배선·Scripted 테스트)

**Interfaces:**
- Consumes: `RepetitionTracker.error_correction`의 스트릭 상태(last_error_key/error_streak), `build_request`(mod.rs:131-150), Scripted의 요청 캡처(`script.requests`)
- Produces: `RepetitionTracker::sr_streak(&self) -> usize`; `Agent.temperature_override: Option<f32>`(run() 진입 시 리셋); `SR_PERTURB_TEMPERATURE: f32 = 0.7`(Task 1이 값을 바꾸면 이 상수만 변경)

- [ ] **Step 1: 브랜치 준비** — 암②와 독립적으로 base에서 분기:

```bash
git checkout -b m10/arm-perturb m10/base
```

- [ ] **Step 2: sr_streak 단위 테스트** — `src/agent/repetition.rs` tests에:

```rust
#[test]
fn sr_streak_tracks_consecutive_sr_errors_only() {
    let mut t = RepetitionTracker::new();
    let sr = format!("{SR_KEY}. x.");
    assert_eq!(t.sr_streak(), 0);
    t.error_correction("edit_file", &sr);
    assert_eq!(t.sr_streak(), 1);
    t.error_correction("edit_file", &sr);
    assert_eq!(t.sr_streak(), 2, "SR_CORRECTION 래치와 무관하게 스트릭은 계속 노출 (M10 §5 배선 주의)");
    t.error_correction("edit_file", &sr);
    assert_eq!(t.sr_streak(), 3);
    t.error_correction("edit_file", "ok result");
    assert_eq!(t.sr_streak(), 0, "비-S/R 결과로 리셋");
    t.error_correction("edit_file", "Error: edit failed: search block not found. y");
    assert_eq!(t.sr_streak(), 0, "다른 오류 키는 S/R 스트릭이 아님");
}
```

- [ ] **Step 3: 실패 확인**

Run: `cargo test sr_streak`
Expected: FAIL — `sr_streak` 미정의

- [ ] **Step 4: 구현** — ① `src/agent/repetition.rs`, `seen_key` 아래:

```rust
/// S/R 오류 연속 길이 (M10 §5) — 디코딩 섭동의 트리거 술어.
/// error_correction()의 Some(SR_CORRECTION) 반환에 걸지 말 것 — 그쪽은 런당
/// 1회 래치라 "스트릭 재도달 시 재활성"이 깨진다. §5 트리거의 tool==edit_file
/// 술어는 생략 — SR_KEY 본문은 edit_file만 낸다(도구 층 오류문 교차 핀)
pub fn sr_streak(&self) -> usize {
    if self.last_error_key.as_deref() == Some(SR_KEY) { self.error_streak } else { 0 }
}
```

② `src/agent/mod.rs`: 상수(FINISH_ARGS_CORRECTION 근처):

```rust
/// S/R 스트릭 2연속 시 다음 요청의 temperature (M10 §5 — 저온 복사 어트랙터
/// 가설의 개입값, 0단계 확정). 스트릭이 끊기면 즉시 원복, 래치 없음
const SR_PERTURB_TEMPERATURE: f32 = 0.7;
```

`Agent` 구조체에 필드 추가 + `new()`에서 `temperature_override: None,` 초기화:

```rust
/// S/R 스트릭 중 일시 temperature 상향 (M10 §5). run() 지역 수명 —
/// 진입 시 리셋해 REPL의 다음 런으로 새지 않는다 (리뷰 2R M-1)
temperature_override: Option<f32>,
```

`build_request`의 temperature 줄 교체:

```rust
temperature: self.temperature_override.unwrap_or(self.temperature),
```

`run()` 진입부(`session.push_user_request(request);` 직후):

```rust
self.temperature_override = None; // M10 §5 — run() 지역 수명
```

헬퍼(track_and_note 아래):

```rust
/// M10 §5: 스트릭 상태를 오버라이드에 반영 — track_and_note(error_correction
/// 경유) 직후에만 호출한다. 무액션·finish 턴은 호출 지점에 닿지 않아 유지된다
fn update_perturb(
    &mut self,
    tracker: &repetition::RepetitionTracker,
    on_event: &mut dyn FnMut(AgentEvent<'_>),
) {
    let want = (tracker.sr_streak() >= 2).then_some(SR_PERTURB_TEMPERATURE);
    if want.is_some() && self.temperature_override.is_none() {
        on_event(AgentEvent::Notice("(S/R 반복 감지 — temperature 일시 상향)".to_string()));
    }
    self.temperature_override = want;
}
```

호출 배선 — `track_and_note`를 부르는 **두 지점**(게이트 거부 경로 mod.rs:320, 디스패치 경로 mod.rs:382) 각각의 직후에:

```rust
self.update_perturb(&tracker, on_event);
```

(게이트 거부 경로는 `Denied:` 본문이 스트릭을 리셋하므로 자연 해제 — §5 핀.)

- [ ] **Step 5: Scripted 통합 테스트** — `src/agent/mod.rs` tests에:

```rust
#[tokio::test]
async fn sr_streak_of_two_raises_temperature_until_streak_breaks() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.rs"), "x\n").unwrap();
    std::fs::write(dir.path().join("g.rs"), "y\n").unwrap();
    let sr = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "x"}));
    let read = turn("read_file", serde_json::json!({"path": "g.rs"}));
    let script = Scripted::new(vec![ok(&sr), ok(&sr), ok(&read), ok(&finish("d"))]);
    let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    run_quiet(&mut agent, &mut session, "x").await.unwrap();
    let temps: Vec<f32> = script.requests.lock().unwrap().iter().map(|r| r.temperature).collect();
    // 요청0(첫 턴)·요청1(SR 1회 후) 기본값, 요청2(SR 2연속 후) 0.7, 요청3(read 성공 후) 원복
    assert_eq!(temps, vec![0.1, 0.1, 0.7, 0.1], "{temps:?}");
}

#[tokio::test]
async fn perturb_reactivates_without_latch_and_resets_per_run() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.rs"), "x\n").unwrap();
    std::fs::write(dir.path().join("g.rs"), "y\n").unwrap();
    let sr = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "x"}));
    let read = turn("read_file", serde_json::json!({"path": "g.rs"}));
    // 1런: SR×2 → read → SR×2 (재활성 확인 — SR_CORRECTION 래치와 무관) → finish
    let script = Scripted::new(vec![
        ok(&sr), ok(&sr), ok(&read), ok(&sr), ok(&sr), ok(&finish("d")),
        // 2런: 활성 상태로 끝난 뒤에도 진입 리셋 확인
        ok(&finish("d2")),
    ]);
    let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    run_quiet(&mut agent, &mut session, "x").await.unwrap();
    {
        let reqs = script.requests.lock().unwrap();
        let temps: Vec<f32> = reqs.iter().map(|r| r.temperature).collect();
        // 요청4는 두 번째 스트릭의 1회차 직후(아직 1연속)라 기본값, 요청5가 재활성
        // — SR_CORRECTION 래치(1런 1회)와 무관하게 스트릭 재도달만으로 켜진다
        assert_eq!(temps, vec![0.1, 0.1, 0.7, 0.1, 0.1, 0.7], "{temps:?}");
    }
    let mut session2 = new_session(&agent);
    run_quiet(&mut agent, &mut session2, "y").await.unwrap();
    let reqs = script.requests.lock().unwrap();
    assert_eq!(reqs.last().unwrap().temperature, 0.1, "활성 상태로 끝난 뒤에도 run() 진입 리셋 (리뷰 2R M-1)");
}

#[tokio::test]
async fn gate_denied_edit_clears_perturb_override() {
    // §5 핀: Denied: 본문은 스트릭 리셋 → 오버라이드 해제. 주의: SR 오류 2회는
    // preview(dry_run)가 같은 사다리를 타므로 Err → 게이트 생략 → 디스패치가 SR
    // 오류를 되먹여 스트릭이 쌓인다. 유효한 3번째 편집만 preview Ok → 거부 경로.
    // (approval.rs의 실제 Approver 트레이트 시그니처에 맞출 것; preview가 S/R
    // 동일에서 Err가 아니면 이 전제를 재확인하고 테스트를 조정한다)
    struct DenyEdits;
    impl crate::agent::approval::Approver for DenyEdits {
        fn approve(&mut self, _req: &ApprovalRequest) -> Decision {
            Decision::Deny { reason: "테스트 거부".into() }
        }
    }
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.rs"), "x\n").unwrap();
    let sr = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "x"}));
    let valid = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "y"}));
    let script = Scripted::new(vec![ok(&sr), ok(&sr), ok(&valid), ok(&finish("d"))]);
    let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    agent.run(&mut session, "x", &mut DenyEdits, &mut |_| {}).await.unwrap();
    let temps: Vec<f32> = script.requests.lock().unwrap().iter().map(|r| r.temperature).collect();
    // 요청2까지 SR 2연속으로 0.7, 유효 편집이 게이트 거부(Denied:)되며 리셋 → 요청3 원복
    assert_eq!(temps, vec![0.1, 0.1, 0.7, 0.1], "{temps:?}");
}
```

스펙 §9의 "무액션 턴·finish 턴 유지" 케이스:

```rust
#[tokio::test]
async fn no_action_turns_preserve_perturb_override() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.rs"), "x\n").unwrap();
    let sr = turn("edit_file", serde_json::json!({"path": "f.rs", "search": "x", "replace": "x"}));
    let script = Scripted::new(vec![
        ok(&sr),
        ok(&sr),
        ok_with_reason("cut off", "length"), // 무액션 턴 — 스트릭·오버라이드 불변 (§5 핀)
        ok(&finish("d")),
    ]);
    let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    run_quiet(&mut agent, &mut session, "x").await.unwrap();
    let temps: Vec<f32> = script.requests.lock().unwrap().iter().map(|r| r.temperature).collect();
    // length-cut 턴이 오버라이드를 건드리지 않아 그다음 요청도 0.7 유지
    assert_eq!(temps, vec![0.1, 0.1, 0.7, 0.7], "{temps:?}");
}
```

주의: 첫 테스트에서 read 뒤 finish 요청(요청3)의 온도가 원복인 이유 — read_file 성공이 스트릭을 리셋. `Config::default().temperature` 기본값 정의는 config.rs:26, 보증 테스트는 config.rs:139.

- [ ] **Step 6: 게이트 + 검증 게이트**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo run -- eval tasks --verify && cargo run -- eval tasks-large --verify`
Expected: 전부 PASS, 12/12·3/3

- [ ] **Step 7: 커밋 (arm-perturb 브랜치) 후 main 복귀**

```bash
git add src/agent/repetition.rs src/agent/mod.rs
git commit -m "feat(agent): S/R 2연속 시 temperature 0.7 일시 상향 — 디코딩 섭동 (M10 §5, 암③)"
git checkout main
```

---

### Task 8: 실험 1 사전등록 문서 (사용자 승인 게이트)

**Files:**
- Create: `docs/experiments/2026-07-17-sr-loop-arms/pre-registration.md`

**Interfaces:**
- Consumes: TEMPLATE.md 양식, Task 1의 확정값, Task 6·7의 브랜치·커밋 해시
- Produces: 승인된 사전등록 — Task 9 러너의 유일한 입력

- [ ] **Step 1: 사전등록 작성** — TEMPLATE.md 양식으로, 스펙 §8 요강을 옮긴다. 핵심 내용(그대로 사용, 브랜치 해시만 실값 기입):

```markdown
# 실험 사전등록: 완고 S/R 루프 개입 3암 비교

- 날짜/디렉토리: docs/experiments/2026-07-17-sr-loop-arms/
- 스펙 근거: docs/superpowers/specs/2026-07-17-m10-experiment-infra-stubborn-loops-design.md §8
- 상태: 초안

## 가설
H1: 완고 S/R 루프는 행동 공간 차단(암②)으로 종결된다.
H2: 루프의 원인은 저온(0.1) 복사 어트랙터이며 디코딩 섭동(암③)으로 회복된다.

## 조건 (암)
| 암 | 브랜치 (커밋) | 내용 |
|---|---|---|
| ① 기준선 | m10/base (<해시>) | --filter + 인프라만, 에이전트 행동 불변 |
| ② 강제 전환 | m10/arm-block (<해시>) | S/R 3누적 → edit_file 차단·write_file 강제 |
| ③ 디코딩 섭동 | m10/arm-perturb (<해시>) | S/R 2연속 → temperature 0.7 일시 상향 |

## 표본
ornith-1.0-9b@8K(context_tokens 8192, 로드 12288), 표적 2과제
(fix-monthly-total·update-vat-rate) × 10반복(--repeats 10 --seed 0) × 3암 = 60런.
배치별 `./.loco/config.toml`(러너가 배치 전 기록·배치 후 effective_config 대조):
8K = context_tokens 8192·max_output_tokens 4096·command_timeout_secs 240,
32K = context_tokens 32768·나머지 동일, 스포트 = context_tokens 8192·
max_output_tokens 4096·command_timeout_secs 60 (v2 조건).
gemma 제외 — M9 데이터에서 S/R 4회 전부 개입 임계(2연속·3누적) 미도달, 정보 0.
승자 확정 후: 승자 암 @32K(32768/49152) 2과제 × 10반복 + tasks/ 스포트 36런
(v2 조건: command_timeout_secs 60, 로드 8192).

## 지표 (exp_metrics.py 열)
주: sr발 반복정지 수(stop_cause=sr), 완고 루프(파일별 S/R 3회+) 발생 런의 종결
전환율, 오류당 2시도 내 회복률(sr_recovered/sr_recovery_denom).
보조: 엄격 통과율(passed ∧ outcome=finished)·거짓 finish·평균 턴/시간·salvage.

## 판정 규칙 (데이터 보기 전 확정)
주 지표 우세 암을 main에 병합. 동률이면 단순한 쪽(암②). 두 암 모두 기준선보다
나쁘면 병합 없이 실패 턴 제거안을 M11 입력으로. 발생 런이 배치당 3런 미만인
지표는 전수 나열 + 방향 판정(소표본 규칙). 스포트 게이트: ≥33/36.

## 중단 규칙
LLM 에러·부분 리포트 → 해당 배치 1회 재수행, 재실패면 실험 중단·원인 조사.
Ctrl+C 부분 리포트는 폐기 후 해당 배치 재수행.

## 시간 예산 (상한치)
8K 3암 ≤ 3.0h(실측 2.2~2.4h + 개입 암 완주 전환 여유), 32K 승자 ≤ 1.5h,
스포트 ≤ 1.0h — 총 ≤ 5.5h + 모델 교체·게이트. 배치가 상한 1.5배를 넘으면 중단.
```

- [ ] **Step 2: 커밋**

```bash
git add docs/experiments/2026-07-17-sr-loop-arms/pre-registration.md
git commit -m "docs: 실험 1 사전등록 초안 — S/R 루프 3암 비교 (M10 §8)"
```

- [ ] **Step 3: 사용자 승인 요청** — **여기서 멈춘다.** 사용자에게 사전등록 리뷰를 요청하고, 승인 시 상태를 `승인됨(날짜)`으로 바꿔 커밋한 뒤에만 Task 9 진행(PROTOCOL.md 규칙 1).

---

### Task 9: 무인 실험 수행 (밤샘 배치)

**Files:**
- Create: `docs/experiments/2026-07-17-sr-loop-arms/report.md` (러너 초안)

**Interfaces:**
- Consumes: 승인된 pre-registration.md, loco-experiment-runner 에이전트(Task 5), lms CLI
- Produces: 8K 3암 배치의 eval 스탬프 + report.md 초안(배치↔커밋↔스탬프 표, 지표 표, 판정 규칙 기계 적용)

- [ ] **Step 1: 러너 디스패치** — Claude Code에서:

`Agent(subagent_type: "loco-experiment-runner", prompt: "docs/experiments/2026-07-17-sr-loop-arms/pre-registration.md의 8K 3암 배치를 수행하라")`

러너가 수행할 배치 명령(사전등록 그대로 — 암마다 체크아웃·빌드·게이트·lms 검증 후):

```bash
cargo run -- eval tasks-large --filter fix-monthly-total --filter update-vat-rate --repeats 10 --seed 0
```

- [ ] **Step 2: 완료 확인** — report.md 초안에 3암 × (스탬프, git hash, lms 확인 출력, 지표 표)가 전부 있는지, 중단 규칙 발동이 없었는지 확인. 부분 리포트가 있으면 사전등록 중단 규칙대로 처리 결과를 확인

- [ ] **Step 3: 승자 검증 배치** — 8K 결과에서 판정 규칙을 기계 적용해 승자 후보를 정하고(전패면 Task 10으로 — 병합 없음), 러너로 승자 암 @32K + tasks/ 스포트 배치 수행. 배치별 config 세팅·검증은 러너 절차 2가 담당(사전등록의 배치별 config 표 그대로 — 스포트는 v2 조건)

- [ ] **Step 4: 러너 산출물 커밋**

```bash
git add docs/experiments/2026-07-17-sr-loop-arms/report.md
git commit -m "docs: 실험 1 수행 결과 초안 — 3암 8K + 승자 검증 배치 (M10 §8)"
```

---

### Task 10: 판정·승자 병합·문서 갱신

**Files:**
- Modify: `docs/experiments/2026-07-17-sr-loop-arms/report.md` (최종 판정 절)
- Modify: `docs/baselines.md` (실험 1 결과 절)
- Modify: `README.md` (프로젝트 상태 절)
- Modify: `CLAUDE.md` (M10 요약 라인)

**Interfaces:**
- Consumes: report.md 초안, 사전등록 판정 규칙

- [ ] **Step 1: 판정** — 사전등록 판정 규칙만으로 승자 결정(주 지표 우세 → 동률 시 암② → 전패 시 병합 없음). 소표본 지표는 발생 런 전수 나열. report.md에 최종 판정 절 추가. **사용자 리뷰를 받은 뒤** 다음 단계 진행(PROTOCOL.md 규칙 7)

- [ ] **Step 2: 승자 병합**

```bash
git checkout main && git merge --no-ff <승자 브랜치> -m "feat(agent): M10 승자 암 병합 — <암 이름> (실험 1 판정)"
cargo test && cargo clippy --all-targets -- -D warnings
cargo run -- eval tasks --verify && cargo run -- eval tasks-large --verify
```

Expected: 게이트 전부 PASS (패자 브랜치는 삭제하지 않고 보존)

- [ ] **Step 3: 문서 갱신** — ① baselines.md: "M10 실험 1" 절(3암 표·행동 지표·판정·스탬프) ② README: 상태 절을 M10으로 ③ CLAUDE.md: M1-M9 라인을 M10 포함으로, --filter·scripts/exp_metrics.py·docs/experiments/ 규약·러너 에이전트·승자 개입 한 줄씩(영문) ④ 스펙 §2 성공 기준 5항 대조표를 report.md에 남긴다 ⑤ 스펙 목표 4의 README·CLAUDE.md "재검증 루프" 재특성화는 스펙 커밋(ff16ca4)에서 선반영됨 — M10 문구와 충돌 없는지 확인만

- [ ] **Step 4: 최종 커밋**

```bash
git add docs/ README.md CLAUDE.md
git commit -m "docs: M10 판정·기준선·상태 갱신 — 실험 1 결과 반영"
```

- [ ] **Step 5: 마일스톤 마감 확인** — 성공 기준 5항(스펙 §2) 전건 대조를 사용자에게 보고. push는 사용자 지시가 있을 때만.
