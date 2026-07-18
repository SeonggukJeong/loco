# M11 진행 상태 접지 (하네스 상태선) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 하네스가 이미 추적하는 진행 상태(수정 파일·검증 여부·잔여 턴)를 조건부로 툴 결과에 접지(상태선)하고, 파이프 exit code 위장을 안내해, uv형 다지점 전파 과제의 탐색 루프·턴 소진 실패를 줄인다 — 사전등록 실험 2로 검증 후 승자만 main 병합.

**Architecture:** 스펙 `docs/superpowers/specs/2026-07-18-m11-progress-state-grounding-design.md`(리뷰 2R Ready=Yes, 1ddcb69). 상태선은 순수 상태기계 `src/agent/status_note.rs`(신설) + `session.remove_status_note()`(최신만 유지) + `run()` 배선으로 구성 — 기존 note 채널(`merge_note`→`push_tool_result`)에 마지막 순서로 병합되므로 반복 해시(body만)·오류 스트릭(body 첫 문장)·finish_nudge 파싱(body 첫 줄)과 자동 격리. 파이프 안내는 `run_command` 결과 body의 Done 분기에만 붙는다.

**Tech Stack:** Rust edition 2024(신규 크레이트 금지), Python 3 표준 라이브러리(exp_metrics), LM Studio + lms CLI(측정).

## Global Constraints

- 스펙 = `docs/superpowers/specs/2026-07-18-m11-progress-state-grounding-design.md` 커밋 `1ddcb69` — 모든 §번호는 이 문서 기준
- 게이트(매 태스크): `cargo test` 전건 통과 + `cargo clippy --all-targets -- -D warnings` 무경고. 픽스처를 만진 경우에만 `--verify` 추가 (이번 플랜은 픽스처 변경 없음 — 최종 게이트에서 두 트리 `--verify` 재확인만)
- 신규 모델-대면 텍스트 전부 영문, 사용자 CLI 메시지 한국어, config 토글 신설 금지, `tasks/`·`tasks-large/` 픽스처 변경 일절 금지
- **브랜치 규율(스펙 §6)**: Task 1~3은 main 직접 커밋(문서·소품·인프라 — 에이전트 행동 불변), Task 4~7은 `m11/status-note` 브랜치(개입 코드 — main 병합은 Task 10 판정 후에만). 브랜치는 Task 4 시작 시 main(Task 3 완료 시점)에서 생성
- **사전등록 없이 GPU 시간 금지**(PROTOCOL.md) — Task 8이 사용자 승인 게이트에서 정지, Task 9는 승인된 사전등록 문서로만 수행
- 측정 중 cargo build/test 병행 금지, 트립와이어 사전 점검 `ls ${TMPDIR}/.cargo`(존재 시 수동 제거)
- 커밋은 conventional commits(제목 한국어 가능), 각 태스크 끝에 커밋
- 상태선 마커 계약: 마커 줄 = `"[status] "` 접두, 연속 줄 = 9칸 공백 `"         "` 접두 — status_note.rs·session.rs·exp_metrics.py 세 곳이 공유(Task 3·5·6의 문자열이 일치해야 한다)

---

### Task 1: 법의학 공식화 (main, 측정 0)

**Files:**
- Create: `docs/research/2026-07-18-m11-uv-progress-forensics.md`

**Interfaces:**
- Consumes: `.loco/eval/20260717T152633Z/`(8K 승자 배치, uv+fm 각 10런) · `.loco/eval/20260717T164905Z/`(32K 검증 배치 20런) — git-ignored 로컬 데이터, `report.json` + `run-*.jsonl`
- Produces: 스펙 §2 기준 2의 대조 실측 수치 확정(①·②·풍선 가드), Task 8 사전등록의 판정 임계 근거. **전제 반전 시 이 플랜을 중단하고 스펙 개정·재리뷰**(스펙 §3)

- [ ] **Step 1: 분류 스크립트 작성 (스크래치패드 — 리포에 넣지 않는다)**

`/private/tmp/.../scratchpad/classify.py` (경로는 세션 스크래치패드 사용):

```python
#!/usr/bin/env python3
"""M11 §3 법의학: 런별 도구 시퀀스·성공 뮤테이션·cargo 검증·파이프 위장 분류."""
import glob, json, os, sys

def classify(stamp_dir):
    rep = json.load(open(os.path.join(stamp_dir, "report.json")))
    idx = {f"run-{t['name']}-{r['repeat']}": (r.get("outcome"), r.get("passed"))
           for t in rep.get("tasks", []) for r in t.get("runs", [])}
    print(f"# {stamp_dir}")
    print("run\toutcome\tpassed\tturns\tmut_ok\tcargo_bare\tcargo_piped\tseq")
    for path in sorted(glob.glob(os.path.join(stamp_dir, "run-*.jsonl"))):
        events = [json.loads(l) for l in open(path)]
        name = os.path.basename(path).removesuffix(".jsonl")
        seq, mut_ok, cargo_bare, cargo_piped = [], 0, 0, 0
        for e in events:
            if e.get("kind") != "tool_result":
                continue
            tool, content = e.get("tool") or "", e.get("content") or ""
            cmd = str((e.get("args") or {}).get("command", ""))
            ok = not (content.startswith("Error:") or content.startswith("Denied:"))
            if tool in ("edit_file", "write_file"):
                mut_ok += ok
                seq.append(("edit" if tool == "edit_file" else "WR") + ("" if ok else "!"))
            elif tool == "run_command" and "cargo" in cmd:
                piped = "|" in cmd.split("cargo", 1)[1]
                cargo_piped += piped
                cargo_bare += not piped
                seq.append("TEST|" if piped else "TEST")
            else:
                seq.append({"read_file": "rd", "grep": "gr",
                            "list_files": "ls", "run_command": "cmd"}.get(tool, tool))
        o, p = idx.get(name, ("?", None))
        print(f"{name}\t{o}\t{p}\t{len(seq)}\t{mut_ok}\t{cargo_bare}\t{cargo_piped}\t{' '.join(seq)}")

for d in sys.argv[1:]:
    classify(d)
```

- [ ] **Step 2: 세 배치에 적용해 표 산출**

Run: `python3 <scratchpad>/classify.py .loco/eval/20260717T152633Z .loco/eval/20260717T164905Z`
Expected: 152633Z uv 10런이 스펙 §1 표와 일치 — 조기 finish 0·5(mut_ok=0, outcome=finished) / 탐색 루프 1·2·7(mut_ok=0, max_turns) / 턴 소진 3·4·6·8 / 성공 9(cargo_bare≥2). 불일치 항목이 나오면 **성공 뮤테이션 기준**(스펙 §3 회귀 조건)으로 재검토 — 분류 자체가 뒤집히면 정지하고 스펙 회귀.

- [ ] **Step 3: 픽스처 표기 산포·파이프 위장 근거 재확인**

Run: `cd tasks-large/update-vat-rate && for f in inv-parse/src/defaults.rs inv-report/src/forecast.rs inv-report/src/invoice.rs inv-core/src/rules/pricing.rs; do diff fixture/$f solution/$f; done`
Expected: 4지점 diff가 `10`/`1.10`/`110 / 100`/`10 / 100` 표기 산포를 보인다(스펙 §1 반전 1).
Run(파이프 위장 전수, ⓓ):

```bash
python3 - <<'EOF'
import glob, json
for stamp in ["20260717T125544Z", "20260717T140556Z", "20260717T152633Z", "20260717T164905Z"]:
    for path in sorted(glob.glob(f".loco/eval/{stamp}/run-*.jsonl")):
        for line in open(path):
            e = json.loads(line)
            if e.get("kind") != "tool_result" or e.get("tool") != "run_command":
                continue
            cmd = str((e.get("args") or {}).get("command", ""))
            if "cargo" in cmd and "|" in cmd.split("cargo", 1)[1]:
                print(stamp, path.split("/")[-1], "|", cmd)
EOF
```

Expected: 152633Z uv run-3의 `cargo test 2>&1 | tail -50` 포함 — 건수·해당 런을 노트에 기록. (이 스캔은 naive `|` 포함 판정 — 따옴표 안 오탐이 섞이면 눈으로 걸러 기록한다.)

- [ ] **Step 4: 법의학 노트 작성**

`docs/research/2026-07-18-m11-uv-progress-forensics.md` — 절 구성:
1. 목적·데이터(스탬프 3종·리워드 픽스처 기준임을 명시). **좌표계 정의 한 줄**:
   이 노트와 exp_metrics의 "턴"은 **tool_result 이벤트 1-기준 순번**이다 —
   에이전트 `turns` 카운터(length 턴 등 포함)와 다른 좌표계이므로 스펙 §1
   표의 턴 수와 직접 비교하지 않는다
2. 152633Z uv 10런 분류표(Step 2 출력 + 3덩어리 귀속) — 스펙 §1 표의 공식 재검증
3. fm 10런 + 32K 20런 동일 분류(ⓑ — 3덩어리 일반화 여부 판정 서술)
4. 픽스처 표기 산포와 "재검색 푸터 무산" 판정 기록(ⓒ)
5. 파이프 위장 전수(ⓓ — 건수·해당 런)
6. **사전등록 입력**: §2 기준 2 수치 확정(①=?/10, ②=?/5, 뮤테이션 0회 finished=?런) — Step 2 실측으로 스펙 서술(3/10·3/5·2런)을 확정 또는 정정
7. 전제 반전 여부 판정 한 단락(반전 시: 플랜 중단·스펙 개정)

- [ ] **Step 5: Commit**

```bash
git add docs/research/2026-07-18-m11-uv-progress-forensics.md
git commit -m "docs: M11 0단계 법의학 — uv 진행상태 실패 3덩어리 공식화 (152633Z·164905Z)"
```

---

### Task 2: 소품 — 섭동 finish-재시도 오버라이드 테스트 (main)

**Files:**
- Modify: `src/agent/mod.rs` (tests 모듈 — 기존 섭동 테스트 4건 근처, mod.rs:1484-1569 부근)

**Interfaces:**
- Consumes: 기존 테스트 헬퍼 `Scripted`/`ok`/`turn`/`finish`/`make_guided_agent`/`new_session`/`run_quiet`(mod.rs:529-602), M10 §5 원복 핀("finish 시도 턴은 스트릭 불변 → 오버라이드 유지")
- Produces: 동작 변경 없음 — 회귀 테스트 1건 (스펙 §7)

- [ ] **Step 1: 실패 확인이 아닌 커버리지 추가임을 확인**

이 테스트는 기존 동작의 미커버 분기 고정이다 — 작성 후 바로 통과해야 정상. 통과하지 않으면 M10 병합 코드에 버그가 있다는 뜻이므로 정지·보고.

- [ ] **Step 2: 테스트 작성**

`src/agent/mod.rs` tests 모듈, 기존 섭동 테스트들 뒤에:

```rust
#[tokio::test]
async fn perturb_override_survives_finish_missing_summary_turn() {
    // M10 §5 원복 핀: finish 시도 턴은 S/R 스트릭 불변 → 오버라이드 유지 (M11 §7 이월 소품)
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.rs"), "fn a() {}\n").unwrap();
    let sr = turn(
        "edit_file",
        serde_json::json!({"path": "f.rs", "search": "fn a() {}", "replace": "fn a() {}"}),
    );
    let script = Scripted::new(vec![
        ok(&sr),                                    // S/R 오류 1
        ok(&sr),                                    // S/R 오류 2 → 스트릭 2
        ok(&turn("finish", serde_json::json!({}))), // summary 없는 finish — 스트릭 불변
        ok(&finish("done")),
    ]);
    let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    run_quiet(&mut agent, &mut session, "x").await.unwrap();
    let reqs = script.requests.lock().unwrap();
    assert!((reqs[2].temperature - 0.7).abs() < 1e-6, "스트릭 2 도달 후 요청은 섭동");
    assert!(
        (reqs[3].temperature - 0.7).abs() < 1e-6,
        "finish 인자누락 턴은 스트릭 불변 — 오버라이드 유지: {}",
        reqs[3].temperature
    );
}
```

- [ ] **Step 3: 테스트 실행**

Run: `cargo test perturb_override_survives_finish_missing_summary_turn`
Expected: PASS (1 passed)

- [ ] **Step 4: 게이트 + Commit**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전건 통과·무경고

```bash
git add src/agent/mod.rs
git commit -m "test(agent): 섭동 오버라이드 finish-재시도 턴 유지 회귀 테스트 (M10 §5 이월)"
```

---

### Task 3: exp_metrics.py 확장 — 상태선·검증·파일 귀속 지표 (main)

**Files:**
- Modify: `scripts/exp_metrics.py` (188줄 — MARKS·COLS·run_metrics·process·selftest)

**Interfaces:**
- Consumes: 트랜스크립트 스키마(`kind`/`content`/`tool`/`args`), 상태선 마커 `"[status] files edited"`(Task 5의 STATUS_MARKER + 본문 첫 필드와 일치), 파이프 노트 `"the exit code reflects only the last command"`(Task 4 문구의 부분 문자열)
- Produces: 런별 컬럼 `first_mut_turn`/`cargo_after_mut`/`zero_mut_end`/`status_in_args`/`sr_files`, 요약의 zero_mut 분해 — Task 9·10의 판정 입력. `first_mut_turn`의 "턴"은 tool_result 이벤트 1-기준 순번(에이전트 turns 카운터와 별개 좌표계 — Task 1 노트의 정의와 동일, 리포트에 명기)

- [ ] **Step 1: MARKS·COLS 확장**

```python
MARKS = {
    "sr_error": "search and replace are identical",
    "sr_correction": "Write the MODIFIED code in `replace`",
    "sr_block": "edit_file is disabled for this file",
    "repeat_corr": "repeating the same tool call",
    "finish_missing": "finish requires a string `summary`",
    "finish_args_corr": "Do not call finish with empty args again",
    "finish_nudge": "do not re-verify what you have already confirmed",
    "status_note": "[status] files edited",
    "pipe_note": "the exit code reflects only the last command",
}
COLS = ["run", "outcome", "passed"] + list(MARKS) + [
    "sr_recovered", "sr_recovery_denom", "finish_missing_maxrun", "perturb_turns", "stop_cause",
    "first_mut_turn", "cargo_after_mut", "zero_mut_end", "status_in_args", "sr_files",
]
```

- [ ] **Step 2: run_metrics 확장**

함수 전체를 다음으로 교체(기존 로직 보존 + 추가분 — 주석의 기존 설명 유지):

```python
def run_metrics(events):
    counts = dict.fromkeys(MARKS, 0)
    streak_key, streak, perturb_turns = None, 0, 0
    # (마커 발견 후 남은 edit/write 시도 기회) 목록 — "2시도 내 회복" 판정
    pending, recovered, denom = [], 0, 0
    fin_run, fin_max = 0, 0
    last_body = ""
    tool_turn = 0        # tool_result 이벤트 1-기준 순번
    first_mut_turn = 0   # 첫 성공 뮤테이션의 tool_turn (0 = 없음)
    cargo_after_mut = 0  # 첫 성공 뮤테이션 이후 cargo run_command 유무 (§2 기준 2 ②)
    status_in_args = 0   # edit/write args 내 "[status]" 복사 오염 (§9 관측)
    sr_files = {}        # S/R 오류의 파일 귀속 (M10 §9 기준 3② 각주 해소)
    for e in events:
        kind, content = e.get("kind"), e.get("content") or ""
        if kind == "assistant":
            continue
        # 마커는 모든 비-assistant 이벤트에서 센다 — 교정 노트는 tool_result가
        # 아니라 별도 user 이벤트로 남는다 (baselines.md 추출 레시피)
        for k, m in MARKS.items():
            counts[k] += content.count(m)
        last_body = content
        if MARKS["finish_missing"] in content:
            fin_run += 1
            fin_max = max(fin_max, fin_run)
        elif kind == "tool_result":
            fin_run = 0
        if kind != "tool_result":
            continue
        tool_turn += 1
        tool = e.get("tool") or ""
        args = e.get("args") or {}
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
            if ok and not first_mut_turn:
                first_mut_turn = tool_turn
            if any("[status]" in str(args.get(f, "")) for f in ("search", "replace", "content")):
                status_in_args += 1
            still = []
            for tries in pending:
                if ok:
                    recovered += 1
                elif tries - 1 > 0:
                    still.append(tries - 1)
            pending = still
        if tool == "run_command" and first_mut_turn and "cargo" in str(args.get("command", "")):
            cargo_after_mut = 1
        if MARKS["sr_error"] in content:
            denom += 1
            pending.append(2)
            p = str(args.get("path") or "?")
            sr_files[p] = sr_files.get(p, 0) + 1
    return (counts, recovered, denom, fin_max, perturb_turns, last_body,
            first_mut_turn, cargo_after_mut, status_in_args, sr_files)
```

- [ ] **Step 3: process() 갱신**

언패킹·행·요약을 새 반환에 맞춘다. 기존 for 루프 본문을 다음으로 교체:

```python
    zero_mut = {"max_turns": 0, "finished": 0, "other": 0}
    mut_runs, cargo_runs = 0, 0
    for path in sorted(glob.glob(os.path.join(stamp_dir, "run-*.jsonl"))):
        events = [json.loads(l) for l in open(path)]
        (counts, rec, den, fin_max, perturb, last,
         first_mut, cargo_mut, st_args, sr_files) = run_metrics(events)
        name = os.path.basename(path).removesuffix(".jsonl")
        outcome, passed = idx.get(name, ("?", None))
        cause = stop_cause(outcome, last)
        if cause != "-":
            stops[cause] += 1
        if first_mut == 0:
            zero_mut[outcome if outcome in ("max_turns", "finished") else "other"] += 1
            zme = outcome
        else:
            zme = "-"
            mut_runs += 1
            cargo_runs += cargo_mut
        for k in MARKS:
            totals[k] += counts[k]
        total_rec += rec
        total_den += den
        files_col = ",".join(f"{os.path.basename(p)}:{n}" for p, n in sorted(sr_files.items())) or "-"
        row = [name, outcome, str(passed)] + [str(counts[k]) for k in MARKS]
        row += [str(rec), str(den), str(fin_max), str(perturb), cause,
                str(first_mut), str(cargo_mut), zme, str(st_args), files_col]
        print("\t".join(row))
```

요약 print를 다음으로 교체:

```python
    marks = " ".join(f"{k}={totals[k]}" for k in MARKS)
    print(f"# summary {marks} recovered={total_rec}/{total_den} "
          f"stops sr={stops['sr']} finish={stops['finish']} other={stops['other']} "
          f"zero_mut max_turns={zero_mut['max_turns']} finished={zero_mut['finished']} "
          f"other={zero_mut['other']} cargo_after_mut={cargo_runs}/{mut_runs}")
```

(`zero_mut`/`mut_runs`/`cargo_runs` 초기화는 위 교체 블록 첫 두 줄 — 기존 `stops = ...` 다음에 위치.)

- [ ] **Step 4: selftest 확장**

`ev` 헬퍼에 `args` 파라미터 추가 + 기존 언패킹 갱신 + 신규 케이스:

```python
    def ev(kind, content, tool=None, args=None):
        e = {"kind": kind, "content": content}
        if tool:
            e["tool"] = tool
        if args is not None:
            e["args"] = args
        return e
```

기존 세 언패킹을 10-튜플로 갱신(`counts, rec, den, fin_max, perturb, _ , *_ = run_metrics(...)` 형태 금지 — 명시 언패킹으로). 신규 assert 블록을 selftest 끝(`print("selftest ok")` 앞)에 추가:

```python
    # M11: 상태선·파이프 노트·검증·파일 귀속·복사 오염
    events4 = [
        ev("tool_result", sr, "edit_file", {"path": "src/a.rs"}),
        ev("user", "[status] files edited: none yet | turns: 5 of 25 used"),
        ev("tool_result", "wrote", "write_file", {"path": "./src/a.rs", "content": "x"}),
        ev("tool_result", "exit code: 0\nok\nnote: this command is a pipeline - "
           "the exit code reflects only the last command in the pipe",
           "run_command", {"command": "cargo test | tail -5"}),
        ev("tool_result", "Error: edit failed: something else", "edit_file",
           {"path": "b.rs", "search": "[status] files edited", "replace": "y"}),
    ]
    (c4, _, _, _, _, _, fm4, cm4, sa4, sf4) = run_metrics(events4)
    assert c4["status_note"] == 1, c4
    assert c4["pipe_note"] == 1, c4
    assert fm4 == 2, fm4          # 첫 성공 뮤테이션은 두 번째 tool_result(write_file)
    assert cm4 == 1, cm4          # 뮤테이션 후 cargo run_command 실행됨
    assert sa4 == 1, sa4          # args 내 [status] 복사 오염 1건
    assert sf4 == {"src/a.rs": 1}, sf4  # S/R 오류 파일 귀속
    # 뮤테이션 0회 런: first_mut_turn == 0 (zero_mut 분류는 process가 outcome으로)
    (_, _, _, _, _, _, fm5, cm5, _, _) = run_metrics([ev("tool_result", "x", "read_file")])
    assert (fm5, cm5) == (0, 0), (fm5, cm5)
```

- [ ] **Step 5: selftest 실행**

Run: `python3 scripts/exp_metrics.py --selftest`
Expected: `selftest ok`
Run(회귀 — 기존 배치에 적용): `python3 scripts/exp_metrics.py .loco/eval/20260717T152633Z | tail -3`
Expected: 신규 컬럼 포함 표가 에러 없이 출력, `status_note=0 pipe_note=0`(개입 전 배치), `zero_mut` 요약에 uv 조기 finish 2런이 `finished`로 잡힘

- [ ] **Step 6: Commit**

```bash
git add scripts/exp_metrics.py
git commit -m "feat(scripts): exp_metrics M11 확장 — 상태선·cargo 검증·zero-mut 분해·S/R 파일 귀속"
```

---

### Task 4: 브랜치 생성 + 파이프 exit code 안내 (§5)

**Files:**
- Modify: `src/tools/run_command.rs`

**Interfaces:**
- Consumes: `exec_shell`/`ExecEnd`(tools/exec.rs — 변경 없음), Done 분기의 `exit code: {code}\n{body}` 계약
- Produces: `has_unquoted_pipe(cmd: &str) -> bool`(모듈 내부 fn), Done 분기 노트 문구 `"note: this command is a pipeline - the exit code reflects only the last command in the pipe"` — Task 3 마커의 원본

- [ ] **Step 1: 브랜치 생성**

```bash
git checkout -b m11/status-note
```

- [ ] **Step 2: 실패 테스트 작성**

`src/tools/run_command.rs` tests의 unix 모듈에 추가:

```rust
#[test]
fn pipeline_gets_exit_code_provenance_note() {
    let (_d, ctx) = ctx();
    let out = RunCommand
        .run(&serde_json::json!({"command": "false | cat"}), &ctx)
        .unwrap();
    assert!(out.starts_with("exit code: 0"), "파이프 exit는 마지막 명령의 것: {out}");
    assert!(out.contains("note: this command is a pipeline"), "{out}");
}

#[test]
fn non_pipeline_commands_get_no_note() {
    let (_d, ctx) = ctx();
    for cmd in ["echo hi", "grep -c 'a\\|b' /dev/null || true", "true || false"] {
        let out = RunCommand.run(&serde_json::json!({"command": cmd}), &ctx).unwrap();
        assert!(!out.contains("note: this command"), "{cmd} → {out}");
    }
}

#[test]
fn timed_out_command_gets_no_pipeline_note() {
    let (_d, mut c) = ctx();
    c.command_timeout = Duration::from_millis(300);
    let out = RunCommand
        .run(&serde_json::json!({"command": "sleep 30 | cat"}), &c)
        .unwrap();
    assert!(out.contains("timed out") && !out.contains("note: this command"), "{out}");
}
```

유닛 판정 테스트(모듈 하단, unix 게이트 밖):

```rust
#[test]
fn unquoted_pipe_detection() {
    assert!(super::has_unquoted_pipe("cargo test 2>&1 | tail -50"));
    assert!(!super::has_unquoted_pipe(r#"grep "a\|b" f.rs"#));
    assert!(!super::has_unquoted_pipe("grep 'x|y' f.rs"));
    assert!(!super::has_unquoted_pipe(r"grep a\|b f.rs"), "따옴표 밖 백슬래시 이스케이프");
    assert!(
        super::has_unquoted_pipe(r"echo 'a\' | cat"),
        "단일따옴표 안 백슬래시는 리터럴 — 따옴표가 닫히고 파이프는 실파이프"
    );
    assert!(!super::has_unquoted_pipe("a || b"), "OR 연산자는 파이프가 아님");
    assert!(super::has_unquoted_pipe("a | b || c"), "혼합 — 실파이프가 있으면 참");
    assert!(!super::has_unquoted_pipe("echo x"));
}
```

- [ ] **Step 3: 실패 확인**

Run: `cargo test --lib tools::run_command`
Expected: FAIL — `has_unquoted_pipe` 미정의(컴파일 에러)

- [ ] **Step 4: 구현**

`run_command.rs`에 추가(§5 — 따옴표·백슬래시 인지, `||` 스킵):

```rust
/// M11 §5: 따옴표 밖 파이프 존재 판정 — `||`(OR)는 파이프가 아니고, 따옴표
/// 안·백슬래시 이스케이프된 `|`는 무시한다(grep 패턴 상용 — 오발 방지).
/// 잔여 이스케이프 엣지 케이스의 오발은 허용 오차(정보 한 줄, 무해)
fn has_unquoted_pipe(cmd: &str) -> bool {
    let bytes = cmd.as_bytes();
    let (mut in_single, mut in_double) = (false, false);
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if !in_single => {
                i += 2; // 다음 문자는 이스케이프됨 (single quote 안에서만 리터럴)
                continue;
            }
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b'|' if !in_single && !in_double => {
                if bytes.get(i + 1) == Some(&b'|') {
                    i += 2; // `||` — OR 연산자
                    continue;
                }
                return true;
            }
            _ => {}
        }
        i += 1;
    }
    false
}
```

`run()`의 Done 분기를 교체:

```rust
            ExecEnd::Done(status) => {
                let code = status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "(terminated by signal)".to_string());
                let mut out = format!("exit code: {code}\n{}", exec.body);
                if has_unquoted_pipe(&args.command) {
                    out.push_str(
                        "\nnote: this command is a pipeline - the exit code reflects only the last command in the pipe",
                    );
                }
                out
            }
```

- [ ] **Step 5: 통과 확인 + 게이트 + Commit**

Run: `cargo test --lib tools::run_command && cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 전건 PASS·무경고

```bash
git add src/tools/run_command.rs
git commit -m "feat(tools): run_command 파이프 exit code 출처 안내 — 따옴표·이스케이프 인지 (M11 §5)"
```

---

### Task 5: status_note.rs 코어 — 상태기계·렌더 (branch)

**Files:**
- Create: `src/agent/status_note.rs`
- Modify: `src/agent/mod.rs:1-6` (`pub mod status_note;` 선언 추가)

**Interfaces:**
- Consumes: 없음(순수 로직 — serde_json::Value만)
- Produces(Task 6·7이 사용):
  - `pub const STATUS_MARKER: &str = "[status] "` / `pub const CONT_INDENT: &str = "         "`
  - `pub struct StatusNote` — `new()`, `record_mutation(&mut self, args: &serde_json::Value)`, `record_command_exit(&mut self, body: &str)`, `on_turn(&mut self, ctx: &TurnCtx) -> Option<String>`
  - `pub struct TurnCtx { pub turn: usize, pub max_turns: usize, pub mutation_ok: bool, pub has_note_channel: bool, pub mutated_since_verify: bool }` — `turn`은 이번 액션 턴의 1-기준 순번(= run()의 `turns + 1`, **도달 판정 시점 핀** — 스펙 §4가 플랜에 위임한 것)

- [ ] **Step 1: 실패 테스트 작성**

`src/agent/status_note.rs` (파일 하단 tests 모듈로 함께 작성):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(turn: usize, mutation_ok: bool, channel: bool, msv: bool) -> TurnCtx {
        TurnCtx { turn, max_turns: 25, mutation_ok, has_note_channel: channel, mutated_since_verify: msv }
    }

    #[test]
    fn zero_mutation_cadence_fires_at_5_10_15_20_only() {
        let mut s = StatusNote::new();
        for t in 1..=25 {
            let got = s.on_turn(&ctx(t, false, true, false)).is_some();
            let want = matches!(t, 5 | 10 | 15 | 20);
            assert_eq!(got, want, "turn {t}");
        }
    }

    #[test]
    fn mutation_turn_fires_immediately_and_cadence_stops_after_mutation() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "src/a.rs"}));
        let note = s.on_turn(&ctx(2, true, true, true)).expect("조건 1");
        assert!(note.contains("files edited: 1 (src/a.rs)"), "{note}");
        assert!(note.contains("verification: none since your last edit"), "{note}");
        assert!(note.contains("turns: 2 of 25 used"), "{note}");
        // 뮤테이션 이후 케이던스(조건 2)는 침묵 — 15·20(조건 3)만 남는다
        assert!(s.on_turn(&ctx(5, false, true, true)).is_none());
        assert!(s.on_turn(&ctx(10, false, true, true)).is_none());
        assert!(s.on_turn(&ctx(15, false, true, true)).is_some(), "조건 3");
    }

    #[test]
    fn zero_mutation_render_is_single_line() {
        let mut s = StatusNote::new();
        let note = s.on_turn(&ctx(5, false, true, false)).unwrap();
        assert_eq!(note, "[status] files edited: none yet | turns: 5 of 25 used");
    }

    #[test]
    fn verified_state_shows_last_exit_and_overwrite_pins() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "a.rs"}));
        s.record_command_exit("exit code: 101\nfailed");
        let note = s.on_turn(&ctx(15, false, true, false)).unwrap();
        assert!(note.contains("verification: last command exited 101"), "{note}");
        // 덮어쓰기 핀: exit 줄 없는 Ok(타임아웃 본문)는 None으로 덮는다 (§4)
        s.record_command_exit("command timed out after 240s and was killed\n");
        let note = s.on_turn(&ctx(20, false, true, false)).unwrap();
        assert!(note.contains("verification: last command gave no exit code"), "{note}");
    }

    #[test]
    fn path_list_caps_at_five_with_lexical_dedup() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "./src/a.rs"}));
        s.record_mutation(&serde_json::json!({"path": "src/a.rs"})); // 표기 변형 — 합산
        for p in ["b.rs", "c.rs", "d.rs", "e.rs", "f.rs", "g.rs"] {
            s.record_mutation(&serde_json::json!({"path": p}));
        }
        let note = s.on_turn(&ctx(9, true, true, true)).unwrap();
        assert!(note.contains("files edited: 7 (src/a.rs, b.rs, c.rs, d.rs, e.rs and 2 more)"), "{note}");
    }

    #[test]
    fn threshold_on_channelless_turn_carries_over_once() {
        let mut s = StatusNote::new();
        assert!(s.on_turn(&ctx(5, false, false, false)).is_none(), "채널 없음 — 이월");
        let note = s.on_turn(&ctx(6, false, true, false)).expect("이월분 1회 주입");
        assert!(note.contains("turns: 6 of 25"), "{note}");
        assert!(s.on_turn(&ctx(7, false, true, false)).is_none(), "이월은 1회로 소진");
    }

    #[test]
    fn continuation_lines_use_nine_space_indent() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({"path": "a.rs"}));
        let note = s.on_turn(&ctx(3, true, true, true)).unwrap();
        for line in note.lines().skip(1) {
            assert!(line.starts_with(CONT_INDENT), "블록 경계 구조 계약: {line:?}");
        }
    }

    #[test]
    fn missing_or_non_string_path_is_skipped() {
        let mut s = StatusNote::new();
        s.record_mutation(&serde_json::json!({}));
        s.record_mutation(&serde_json::json!({"path": 42}));
        assert!(s.on_turn(&ctx(5, false, true, false)).unwrap().contains("none yet"));
    }
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib agent::status_note`
Expected: FAIL — 모듈 미존재(컴파일 에러)

- [ ] **Step 3: 구현**

`src/agent/status_note.rs`:

```rust
//! M11 §4 — 진행 상태 접지(상태선). 하네스가 이미 아는 진행 상태(수정 파일·
//! 검증 여부·잔여 턴)를 조건부로 툴 결과 note에 접지한다. 유도형 — 차단 없음.
//! 채널 격리: note는 body와 분리되므로 반복 해시·오류 스트릭·exit 파싱 무영향.

use std::path::{Component, Path};

/// 상태선 마커 — session::remove_status_note·scripts/exp_metrics.py와 공유 계약
pub const STATUS_MARKER: &str = "[status] ";
/// 연속 줄 들여쓰기(9칸 = 마커 길이) — 블록 경계 판정 구조 (§4 블록 경계 핀)
pub const CONT_INDENT: &str = "         ";

/// 뮤테이션 0회 케이던스 (조건 2 — 탐색 루프 겨냥)
const ZERO_MUT_CADENCE: [usize; 4] = [5, 10, 15, 20];
/// 무조건 페이싱 임계 (조건 3 — 턴 소진 겨냥)
const PACING: [usize; 2] = [15, 20];

/// run()이 turns를 증가시키는 매 턴 종료 지점에서 넘기는 이번 턴의 사실
pub struct TurnCtx {
    /// 이번 액션 턴의 1-기준 순번 = run()의 `turns + 1` (도달 판정 시점 핀)
    pub turn: usize,
    pub max_turns: usize,
    /// 이번 턴이 성공 뮤테이션(edit_file/write_file 디스패치 Ok)인가 (조건 1)
    pub mutation_ok: bool,
    /// push_tool_result 경로인가 — false(length·finish 오류·VERIFY_NUDGE 반려)면
    /// 임계는 pending 이월 (§4 이월 핀)
    pub has_note_channel: bool,
    /// run()의 mutated_since_verify (VERIFY_NUDGE 의미론 상속 — §4)
    pub mutated_since_verify: bool,
}

pub struct StatusNote {
    mutated_paths: Vec<String>,
    last_cmd_exit: Option<String>,
    pending: bool,
}

impl Default for StatusNote {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusNote {
    pub fn new() -> Self {
        Self { mutated_paths: Vec::new(), last_cmd_exit: None, pending: false }
    }

    /// 성공 뮤테이션의 path 수집 — 렉시컬 정규화·중복 제거·삽입 순서 유지.
    /// path 부재/비문자열은 생략 (§4)
    pub fn record_mutation(&mut self, args: &serde_json::Value) {
        let Some(p) = args.get("path").and_then(|v| v.as_str()) else { return };
        let n = normalize(p);
        if !self.mutated_paths.contains(&n) {
            self.mutated_paths.push(n);
        }
    }

    /// run_command Ok의 body 첫 줄에서 exit 값을 접두 파싱 — 줄이 없으면 None으로
    /// **덮어쓴다**(스테일 방지 핀 §4: 이전 성공의 0이 거짓 접지되지 않게)
    pub fn record_command_exit(&mut self, body: &str) {
        self.last_cmd_exit = body
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("exit code: "))
            .map(str::to_string);
    }

    /// 턴 종료 지점 판정 — 발동이면 렌더된 상태선(턴당 최대 1회는 호출 지점이
    /// 턴당 하나라는 배선 계약으로 성립). 채널 없는 턴의 임계는 pending 이월
    pub fn on_turn(&mut self, ctx: &TurnCtx) -> Option<String> {
        let cadence = self.mutated_paths.is_empty() && ZERO_MUT_CADENCE.contains(&ctx.turn);
        let pacing = PACING.contains(&ctx.turn);
        if !(ctx.mutation_ok || cadence || pacing || self.pending) {
            return None;
        }
        if !ctx.has_note_channel {
            self.pending = true;
            return None;
        }
        self.pending = false;
        Some(self.render(ctx))
    }

    fn render(&self, ctx: &TurnCtx) -> String {
        let turns_line = format!("turns: {} of {} used", ctx.turn, ctx.max_turns);
        if self.mutated_paths.is_empty() {
            return format!("{STATUS_MARKER}files edited: none yet | {turns_line}");
        }
        let shown = self.mutated_paths.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
        let extra = self.mutated_paths.len().saturating_sub(5);
        let files = if extra > 0 {
            format!("{} ({shown} and {extra} more)", self.mutated_paths.len())
        } else {
            format!("{} ({shown})", self.mutated_paths.len())
        };
        let verification = if ctx.mutated_since_verify {
            "verification: none since your last edit".to_string()
        } else {
            match &self.last_cmd_exit {
                Some(code) => format!("verification: last command exited {code}"),
                None => "verification: last command gave no exit code".to_string(),
            }
        };
        format!("{STATUS_MARKER}files edited: {files}\n{CONT_INDENT}{verification}\n{CONT_INDENT}{turns_line}")
    }
}

/// 렉시컬 정규화 — CurDir 제거·ParentDir 팝, 파일시스템 조회 없음
/// (m10/arm-block:src/agent/sr_block.rs에서 포팅 — M10 스펙 §4)
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
```

`src/agent/mod.rs` 모듈 선언(1-6행)에 추가:

```rust
pub mod status_note;
```

- [ ] **Step 4: 통과 확인 + 게이트 + Commit**

Run: `cargo test --lib agent::status_note && cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 신규 8건 포함 전건 PASS·무경고

```bash
git add src/agent/status_note.rs src/agent/mod.rs
git commit -m "feat(agent): status_note 상태기계 — 발동 3조건·이월·최신 상태 렌더 (M11 §4)"
```

---

### Task 6: session.remove_status_note — 최신만 유지 (branch)

**Files:**
- Modify: `src/session.rs` (Session impl — `pack()` 아래)

**Interfaces:**
- Consumes: `status_note::{STATUS_MARKER, CONT_INDENT}`(Task 5)
- Produces: `pub fn remove_status_note(&mut self)` — Task 7의 run() 배선이 새 상태선 병합 직전에 호출

- [ ] **Step 1: 실패 테스트 작성**

`src/session.rs` tests에 추가:

```rust
#[test]
fn remove_status_note_strips_only_the_status_block() {
    let mut s = sess(vec![ChatMessage::system("sys")]);
    s.push_tool_result(
        "edit_file",
        &serde_json::json!({}),
        "Edited a.rs",
        Some("note: fix your args.\n\n[status] files edited: 1 (a.rs)\n         verification: none since your last edit\n         turns: 3 of 25 used"),
    );
    s.remove_status_note();
    let last = s.messages().last().unwrap();
    assert!(!last.content.contains("[status]"), "{}", last.content);
    assert!(last.content.contains("note: fix your args."), "교정 노트 보존: {}", last.content);
    assert!(last.content.contains("Edited a.rs"), "body 보존: {}", last.content);
}

#[test]
fn remove_status_note_ignores_marker_inside_tool_body() {
    // loco 자기 소스 grep 도그푸딩 — body 안의 가짜 마커는 제거 대상이 아니다 (§4 블록 경계 핀)
    let mut s = sess(vec![ChatMessage::system("sys")]);
    s.push_tool_result("grep", &serde_json::json!({}), "src/x.rs:1:[status] files edited: ...", None);
    let before = s.messages().last().unwrap().content.clone();
    s.remove_status_note();
    assert_eq!(s.messages().last().unwrap().content, before, "tool_result 구조 불변");
}

#[test]
fn remove_status_note_preserves_merged_user_request_after_block() {
    // MaxTurns 후 후속 요청이 상태선 블록 뒤에 병합된 경우 (§4 — truncate-to-end 금지)
    let mut s = sess(vec![ChatMessage::system("sys")]);
    s.push_tool_result(
        "read_file",
        &serde_json::json!({}),
        "body",
        Some("[status] files edited: none yet | turns: 25 of 25 used"),
    );
    s.push_user_request("이어서 이것도 해줘");
    s.remove_status_note();
    let last = s.messages().last().unwrap();
    assert!(!last.content.contains("[status]"), "{}", last.content);
    assert!(last.content.contains("이어서 이것도 해줘"), "병합 요청 보존: {}", last.content);
}

#[test]
fn remove_status_note_survives_elided_messages() {
    // pack() 생략 후에도(접미 보존 — session.rs 실의미론) 옛 상태선을 제거할 수 있다
    let big = "x".repeat(4_000);
    let mut s = sess(vec![ChatMessage::system("sys")]);
    s.push_tool_result("read_file", &serde_json::json!({}), &big,
        Some("[status] files edited: none yet | turns: 5 of 25 used"));
    s.push(ChatMessage::assistant("t"));
    s.push_tool_result("read_file", &serde_json::json!({}), &big, None);
    // 예산 1200: 생략 후 총 ≈1041토큰 ≤ 1200이라 쌍 제거 단계 미진입 — 검증 대상
    // 메시지가 살아남는다 (800이면 drain(1..=2)이 메시지째 지워 테스트 전제가 깨짐).
    // 기존 pack_elides_oldest_tool_results_first와 같은 검증된 상수
    s.pack(1_200); // 첫 tool_result 본문 생략 — 접미(상태선)는 pack이 보존한다
    assert!(s.messages().iter().any(|m| m.content.contains(ELIDED) && m.content.contains("[status]")));
    s.remove_status_note();
    assert!(!s.messages().iter().any(|m| m.content.contains("[status]")), "생략 메시지의 접미도 제거");
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib session`
Expected: FAIL — `remove_status_note` 미정의(컴파일 에러)

- [ ] **Step 3: 구현**

`src/session.rs`의 Session impl, `pack()` 아래에 추가:

```rust
    /// M11 §4 최신만 유지 — 저장 히스토리에서 기존 상태선 블록을 제거한다.
    /// pack()의 생략 단계는 `</tool_result>` 뒤 접미(병합 노트)를 보존하므로 옛
    /// 상태선은 축약으로 회수되지 않는다 — 새 주입 직전에 이 메서드로 걷어내
    /// 문맥에 상태선이 항상 최대 1개이게 한다. 탐색은 각 user 메시지의 마지막
    /// `</tool_result>` 이후 접미로 한정(툴 body 안의 가짜 마커 보호), 블록 =
    /// 마커 줄 + 이어지는 CONT_INDENT 들여쓴 줄, 블록 뒤 텍스트(병합된 후속
    /// 요청)는 보존. 트랜스크립트는 원본 유지(pack()과 같은 제자리 변형)
    pub fn remove_status_note(&mut self) {
        use crate::agent::status_note::{CONT_INDENT, STATUS_MARKER};
        for m in &mut self.messages {
            if m.role != "user" {
                continue;
            }
            let Some(close) = m.content.rfind("</tool_result>") else { continue };
            let split_at = close + "</tool_result>".len();
            if !m.content[split_at..].contains(STATUS_MARKER) {
                continue;
            }
            let (head, suffix) = m.content.split_at(split_at);
            let mut in_block = false;
            let kept: Vec<&str> = suffix
                .lines()
                .filter(|line| {
                    if line.starts_with(STATUS_MARKER) {
                        in_block = true;
                        false
                    } else if in_block && line.starts_with(CONT_INDENT) {
                        false
                    } else {
                        in_block = false;
                        true
                    }
                })
                .collect();
            let mut new_suffix = kept.join("\n");
            while new_suffix.ends_with('\n') {
                new_suffix.pop();
            }
            m.content = format!("{head}{new_suffix}");
        }
    }
```

(주의: `suffix.lines()`의 첫 요소는 `</tool_result>` 직후의 잔여 — 병합 형식이 `\n\n`이므로 첫 줄은 빈 문자열이고 `kept`에 남는다. 결과 문자열의 후행 개행만 정리한다. 병합 요청이 블록 뒤에 있던 케이스는 제거 후 3중 개행이 남을 수 있다 — 콘텐츠 무손실이므로 허용 오차, 압축하지 않는다.)

- [ ] **Step 4: 통과 확인 + 게이트 + Commit**

Run: `cargo test --lib session && cargo test && cargo clippy --all-targets -- -D warnings`
Expected: 신규 4건 포함 전건 PASS·무경고

```bash
git add src/session.rs
git commit -m "feat(session): remove_status_note — 상태선 최신만 유지, 블록 경계·병합 요청 보존 (M11 §4)"
```

---

### Task 7: run() 배선 + 통합 테스트 + 브랜치 게이트 (branch)

**Files:**
- Modify: `src/agent/mod.rs` — `run()`(160-406행 영역) + tests

**Interfaces:**
- Consumes: `StatusNote`/`TurnCtx`(Task 5), `Session::remove_status_note`(Task 6), 기존 `merge_note`/`track_and_note`/`finish_nudge`
- Produces: 최종 동작 — 상태선이 note 채널 마지막 순서로 주입, RepetitionStop 턴 제외, 채널 없는 턴 이월

- [ ] **Step 1: 실패 통합 테스트 작성**

`src/agent/mod.rs` tests에 추가:

```rust
#[tokio::test]
async fn status_note_cadence_fires_at_turn_5_when_nothing_edited() {
    let dir = tempfile::tempdir().unwrap();
    for n in ["a", "b", "c", "d", "e"] {
        std::fs::write(dir.path().join(format!("{n}.txt")), "x").unwrap();
    }
    let reads: Vec<_> = ["a", "b", "c", "d", "e"]
        .iter()
        .map(|n| ok(&turn("read_file", serde_json::json!({"path": format!("{n}.txt")}))))
        .collect();
    let mut script_vec = reads;
    script_vec.push(ok(&finish("done")));
    let script = Scripted::new(script_vec);
    let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    run_quiet(&mut agent, &mut session, "x").await.unwrap();
    let with_status: Vec<_> = session
        .messages()
        .iter()
        .filter(|m| m.content.contains("[status]"))
        .collect();
    assert_eq!(with_status.len(), 1, "턴 5에서 정확히 1회");
    assert!(
        with_status[0].content.contains("[status] files edited: none yet | turns: 5 of 25 used"),
        "{}",
        with_status[0].content
    );
}

#[tokio::test]
async fn status_note_fires_on_mutation_and_keeps_only_latest() {
    let dir = tempfile::tempdir().unwrap();
    let w = |p: &str| turn("write_file", serde_json::json!({"path": p, "content": "x"}));
    // 무검증 finish는 VERIFY_NUDGE가 1회 반려한다 — 두 번째 finish로 종결 (M5 §7.1)
    let script = Scripted::new(vec![
        ok(&w("a.rs")),
        ok(&w("b.rs")),
        ok(&finish("done")),
        ok(&finish("done")),
    ]);
    let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
    assert!(matches!(outcome, AgentOutcome::Finished(_)));
    let with_status: Vec<_> =
        session.messages().iter().filter(|m| m.content.contains("[status]")).collect();
    assert_eq!(with_status.len(), 1, "최신만 유지 — 히스토리에 상태선 1개");
    let c = &with_status.last().unwrap().content;
    assert!(c.contains("files edited: 2 (a.rs, b.rs)"), "{c}");
    assert!(c.contains("verification: none since your last edit"), "{c}");
}

#[tokio::test]
async fn status_note_merges_after_existing_correction_notes() {
    // 스펙 §8 "기존 교정문과 병합 순서" — salvage 노트가 있는 뮤테이션 턴에서
    // 상태선은 같은 메시지의 마지막에 온다
    let dir = tempfile::tempdir().unwrap();
    let bad_shape =
        r#"{"thought": "w", "action": {"tool": "write_file", "args": {"path": "a.rs"}, "content": "x"}}"#;
    let script = Scripted::new(vec![ok(bad_shape), ok(&finish("done")), ok(&finish("done"))]);
    let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    run_quiet(&mut agent, &mut session, "x").await.unwrap();
    let msg = session
        .messages()
        .iter()
        .find(|m| m.content.contains("[status]"))
        .expect("salvage된 write_file 뮤테이션 턴에 상태선");
    let salvage_pos = msg.content.find("fields outside").expect("salvage 노트 공존");
    let status_pos = msg.content.find("[status]").unwrap();
    assert!(status_pos > salvage_pos, "상태선은 마지막 병합: {}", msg.content);
}

#[tokio::test]
async fn status_note_threshold_on_length_turn_carries_to_next_tool_turn() {
    let dir = tempfile::tempdir().unwrap();
    for n in ["a", "b", "c", "d", "e"] {
        std::fs::write(dir.path().join(format!("{n}.txt")), "x").unwrap();
    }
    let rd = |n: &str| ok(&turn("read_file", serde_json::json!({"path": format!("{n}.txt")})));
    let script = Scripted::new(vec![
        rd("a"), rd("b"), rd("c"), rd("d"),
        ok_with_reason("truncated…", "length"), // 턴 5 — 채널 없음, 이월
        rd("e"),                                 // 턴 6 — 이월분 주입
        ok(&finish("done")),
    ]);
    let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    run_quiet(&mut agent, &mut session, "x").await.unwrap();
    let with_status: Vec<_> =
        session.messages().iter().filter(|m| m.content.contains("[status]")).collect();
    assert_eq!(with_status.len(), 1);
    assert!(with_status[0].content.contains("turns: 6 of 25"), "{}", with_status[0].content);
}

#[tokio::test]
async fn repetition_stop_still_fires_with_status_note_active() {
    // 정지 우선순위: 동일 호출 5회 정지 턴(턴 5 = 케이던스 임계)에는 상태선을
    // 주입하지 않는다 (!stop 가드) — 히스토리에 [status] 0개인 채 정지
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "x").unwrap();
    let same = turn("read_file", serde_json::json!({"path": "a.txt"}));
    let script = Scripted::new(vec![ok(&same), ok(&same), ok(&same), ok(&same), ok(&same)]);
    let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
    assert!(matches!(outcome, AgentOutcome::RepetitionStop));
    assert!(!session_contains(&session, "[status]"), "정지 턴 주입 억제");
}

#[tokio::test]
async fn status_note_on_a_repeated_result_does_not_break_repetition_hash() {
    // 채널 격리 실증 (스펙 §8): 턴 5에서 상태선이 병합된 결과가 이후 동일 반복돼도
    // 해시는 body만 보므로 5회째에 RepetitionStop 도달
    let dir = tempfile::tempdir().unwrap();
    for n in ["a", "b", "c", "d", "e"] {
        std::fs::write(dir.path().join(format!("{n}.txt")), "x").unwrap();
    }
    let rd = |n: &str| ok(&turn("read_file", serde_json::json!({"path": format!("{n}.txt")})));
    // 턴 1-4 상이 read, 턴 5 = e.txt 1회차(케이던스 상태선 병합), 턴 6-9 = e.txt 반복
    let script = Scripted::new(vec![
        rd("a"), rd("b"), rd("c"), rd("d"),
        rd("e"), rd("e"), rd("e"), rd("e"), rd("e"),
    ]);
    let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
    assert!(matches!(outcome, AgentOutcome::RepetitionStop), "e.txt 5회째(턴 9) 정지");
    assert!(session_contains(&session, "[status]"), "턴 5의 상태선이 반복 결과에 병합돼 있었음");
}

#[tokio::test]
async fn status_note_threshold_on_finish_error_turn_carries_over() {
    // 이월 핀 경로 ③(finish 오류 턴 — session.push 경로) 통합 검증 (스펙 §8)
    let dir = tempfile::tempdir().unwrap();
    for n in ["a", "b", "c", "d", "e"] {
        std::fs::write(dir.path().join(format!("{n}.txt")), "x").unwrap();
    }
    let rd = |n: &str| ok(&turn("read_file", serde_json::json!({"path": format!("{n}.txt")})));
    let script = Scripted::new(vec![
        rd("a"), rd("b"), rd("c"), rd("d"),
        ok(&turn("finish", serde_json::json!({}))), // 턴 5 — summary 없음, 채널 없음 → 이월
        rd("e"),                                     // 턴 6 — 이월분 주입
        ok(&finish("done")),
    ]);
    let mut agent = make_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    run_quiet(&mut agent, &mut session, "x").await.unwrap();
    let with_status: Vec<_> =
        session.messages().iter().filter(|m| m.content.contains("[status]")).collect();
    assert_eq!(with_status.len(), 1);
    assert!(with_status[0].content.contains("turns: 6 of 25"), "{}", with_status[0].content);
}

#[tokio::test]
async fn status_note_threshold_on_verify_nudge_turn_carries_over() {
    // 이월 핀 경로 ②(VERIFY_NUDGE 반려 턴) 통합 검증 (스펙 §8) — 뮤테이션으로
    // 케이던스가 꺼진 뒤 pacing 15를 반려 턴이 밟는 시나리오
    let dir = tempfile::tempdir().unwrap();
    let mut script_vec =
        vec![ok(&turn("write_file", serde_json::json!({"path": "a.rs", "content": "x"})))]; // 턴 1 — 뮤테이션(상태선 #1)
    for i in 0..13 {
        let name = format!("f{i}.txt");
        std::fs::write(dir.path().join(&name), "x").unwrap();
        script_vec.push(ok(&turn("read_file", serde_json::json!({"path": name})))); // 턴 2-14
    }
    script_vec.push(ok(&finish("done"))); // 턴 15 — VERIFY_NUDGE 반려(채널 없음) + pacing 15 → 이월
    script_vec.push(ok(&turn("read_file", serde_json::json!({"path": "f0.txt"})))); // 턴 16 — 이월분 주입
    script_vec.push(ok(&finish("done"))); // 종결 (VERIFY_NUDGE는 런당 1회)
    let script = Scripted::new(script_vec);
    let mut agent = make_guided_agent(&script, dir.path().to_path_buf(), 25);
    let mut session = new_session(&agent);
    let outcome = run_quiet(&mut agent, &mut session, "x").await.unwrap();
    assert!(matches!(outcome, AgentOutcome::Finished(_)));
    let with_status: Vec<_> =
        session.messages().iter().filter(|m| m.content.contains("[status]")).collect();
    assert_eq!(with_status.len(), 1, "최신만 유지");
    let c = &with_status[0].content;
    assert!(c.contains("turns: 16 of 25"), "이월분이 턴 16에 주입: {c}");
    assert!(c.contains("verification: none since your last edit"), "{c}");
}
```

- [ ] **Step 2: 실패 확인**

Run: `cargo test --lib agent::tests::status_note`
Expected: FAIL — 상태선이 아직 미배선(`[status]` 0개 매치로 assert 실패; `repetition_stop_still_fires...` 하나는 미배선에서도 통과해도 무방)

- [ ] **Step 3: run() 배선**

`run()` 상단(기존 지역 상태 선언부, `finish_args_corrected` 아래)에:

```rust
        let mut status = status_note::StatusNote::new();
```

**배선 지점 5곳** — turns를 증가시키는 모든 턴 종료 지점(스펙 §4 발동·이월 핀):

① length 턴(`turns += 1; continue;` 직전, mod.rs:220 부근):

```rust
                let _ = status.on_turn(&status_note::TurnCtx {
                    turn: turns + 1,
                    max_turns: self.max_turns,
                    mutation_ok: false,
                    has_note_channel: false, // session.push 경로 — 이월
                    mutated_since_verify,
                });
```

② finish summary-있는 VERIFY_NUDGE 반려 턴(`turns += 1; continue;` 직전, mod.rs:269 부근): ①과 동일 블록.

③ finish summary-없는 턴(`session.push(tool_result_message("finish", &body));` 직전, mod.rs:296 부근): ①과 동일 블록.

④ 게이트 거부 턴 — 기존 nudge 병합 뒤·`push_tool_result` 앞(mod.rs:332-336 부근)을 다음으로 교체:

```rust
                    if !stop && let Some(nudge) = finish_nudge.on_turn(ev) {
                        on_event(AgentEvent::Notice("(검증 완료 후 재확인 반복 — finish 유도 주입)".to_string()));
                        note = merge_note(note, nudge);
                    }
                    if !stop
                        && let Some(s) = status.on_turn(&status_note::TurnCtx {
                            turn: turns + 1,
                            max_turns: self.max_turns,
                            mutation_ok: false, // 거부 — 뮤테이션 아님
                            has_note_channel: true,
                            mutated_since_verify,
                        })
                    {
                        session.remove_status_note();
                        note = merge_note(note, &s);
                    }
                    session.push_tool_result(&turn.action.tool, &turn.action.args, &body, note.as_deref());
```

⑤ 디스패치 턴 — 기록 2종은 `dispatch_ok` 분기(기존 mutated_since_verify 갱신 블록, mod.rs:364-370)에 추가:

```rust
            if dispatch_ok {
                if turn.action.tool == "run_command" {
                    mutated_since_verify = false; // 검증 실행으로 인정 — 종료 코드 무관 (M5 §7.1)
                    status.record_command_exit(&body);
                } else if self.registry.get(&turn.action.tool).is_some_and(|t| t.is_mutating()) {
                    mutated_since_verify = true;
                }
                if matches!(turn.action.tool.as_str(), "edit_file" | "write_file") {
                    status.record_mutation(&turn.action.args);
                }
            }
```

주입은 기존 nudge 병합 뒤·`push_tool_result` 앞(mod.rs:395-399)을 ④와 같은 패턴으로 교체 — 단 `mutation_ok`는:

```rust
                    mutation_ok: dispatch_ok
                        && matches!(turn.action.tool.as_str(), "edit_file" | "write_file"),
```

(상태선은 note 병합의 **마지막** — salvage·교정·nudge 뒤. RepetitionStop 턴은 `!stop` 가드로 제외 — 스펙 §4.)

- [ ] **Step 4: 통과 확인**

Run: `cargo test --lib agent`
Expected: 신규 8건 포함 전건 PASS. 기존 테스트 중 5턴 이상 무뮤테이션 시나리오(예: finish_nudge·repetition 계열)가 상태선 주입으로 어서션이 흔들리면, **기대값을 상태선 포함으로 갱신하는 것이 맞는지 스펙 §4 발동 조건과 대조 후** 갱신한다(무단 완화 금지 — 흔들린 테스트 목록을 커밋 메시지에 남긴다).

- [ ] **Step 5: 브랜치 최종 게이트 + Commit**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo run -- eval tasks --verify && cargo run -- eval tasks-large --verify`
Expected: 전건 통과·무경고·12/12·3/3

```bash
git add src/agent/mod.rs
git commit -m "feat(agent): 상태선 run() 배선 — note 채널 마지막 병합·이월·최신만 유지 (M11 §4)"
```

---

### Task 8: 실험 2 사전등록 — 사용자 승인 게이트에서 정지

**Files:**
- Create: `docs/experiments/2026-07-18-progress-grounding/pre-registration.md`

**Interfaces:**
- Consumes: `docs/experiments/TEMPLATE.md`(양식)·PROTOCOL.md(규율), Task 1 노트의 확정 수치, 스펙 §6 요강
- Produces: 승인된 사전등록 문서 — Task 9의 유일한 수행 근거

- [ ] **Step 1: 사전등록 작성 (main에 커밋)**

TEMPLATE.md 양식을 따르되 다음을 반드시 포함:
- 가설 H1(케이던스 접지 → 탐색 루프 감소)·H2(검증 접지 → 뮤테이션 런 cargo 검증률 상승)
- 암: 대조 = **재사용** 스탬프 `20260717T152633Z`(코드 동일성 근거: 스펙 리뷰 1R의 `git diff m10/arm-perturb main -- src/ tasks/ tasks-large/` 공집합 — 문서에 인용) / 개입 = `m11/status-note` 최종 커밋 해시 명기
- 표본: ornith-1.0-9b@8K(로드 12288) × uv+fm × `--repeats 10 --seed 0` + tasks/ 스포트 36런(v2 조건: timeout 60·로드 8192)
- 판정 규칙(Task 1 수치로 임계 고정): 주 = ① zero_mut max_turns 감소(대조 uv 3/10) ② cargo_after_mut 상승(대조 uv 3/5) / **열세 축 = zero_mut finished 비증가(대조 uv 2/10 — 풍선효과 가드)** / 보조 = uv 통과율(1/10)·fm 비악화·스포트 ≥33/36·평균 턴·오버플로·salvage. 소표본 규칙(3런 미만 전수 나열) 명시
- 조건 검증: effective_config + 최상위 model 필드 + 스탬프↔`git rev-parse HEAD` 쌍, 대조군 재사용 한계 각주
- 32K 검증 배치 포함 여부(스펙 §6 — 주지표 충족 시 선택), 시간 상한 ≈2.5h(+22% 전례 반영), 중단 규칙(LLM 에러 1회 재수행·재실패 중단, Ctrl+C 폐기 후 재수행)

```bash
git checkout main
git add docs/experiments/2026-07-18-progress-grounding/pre-registration.md
git commit -m "docs: 실험 2 사전등록 초안 — 진행 상태 접지 (승인 대기)"
```

- [ ] **Step 2: 정지 — 사용자 승인 게이트**

사전등록 요지(가설·표본·판정 임계·시간 상한)를 사용자에게 보고하고 **승인을 명시적으로 받을 때까지 Task 9로 진행하지 않는다**(PROTOCOL: 사전등록 없이 GPU 시간 금지). 수정 요청 시 반영 후 재승인.

---

### Task 9: 실험 2 무인 수행 (러너)

**Files:**
- Create: `docs/experiments/2026-07-18-progress-grounding/report.md` (러너 초안)

**Interfaces:**
- Consumes: 승인된 pre-registration.md, `.claude/agents/loco-experiment-runner.md`, `lms` CLI, `scripts/exp_metrics.py`(Task 3)
- Produces: 개입군 스탬프 2종(uv+fm@8K, tasks/ 스포트) + exp_metrics 출력 + report.md 초안

- [ ] **Step 1: 수행 전 점검**

- `AskUserQuestion`으로 lms 모델 교체 대행 승인(세션 관례 — [[loco-lms-cli-model-switching]] 전례)
- Run: `ls ${TMPDIR}/.cargo` → 존재하면 수동 제거 후 진행
- Run: `git -C . rev-parse m11/status-note` → 사전등록의 해시와 일치 확인

- [ ] **Step 2: 러너 디스패치**

loco-experiment-runner 에이전트에 사전등록 경로를 입력으로 위임하되, 지시문에 다음을 명시(M10 운영 교훈):
- "배치 감시는 통지에 의존하지 말 것 — 각 `cargo run -- eval` 프로세스의 **exit code와 스탬프 디렉토리 생성**으로 종료를 직접 확인"
- 배치 순서: `m11/status-note` 체크아웃·빌드 → uv+fm@8K(`--filter update-vat-rate --filter fix-monthly-total --repeats 10 --seed 0`) → tasks/ 스포트(v2 조건 — config 전환 절차는 러너 문서) → 배치별 스탬프↔rev-parse 쌍 기록
- 측정 중 빌드/테스트 병행 금지, 사전등록에 없는 배치 수행 금지

- [ ] **Step 3: 지표 추출·초안 확인**

Run: `python3 scripts/exp_metrics.py .loco/eval/<개입군-스탬프> .loco/eval/20260717T152633Z`
Expected: 두 배치의 신규 컬럼 비교표 — report.md 초안에 대조/개입 표와 조건 검증(effective_config·model·해시 쌍) 수록

---

### Task 10: 판정·병합·문서

**Files:**
- Modify: `docs/experiments/2026-07-18-progress-grounding/report.md`(판정 확정), `docs/baselines.md`(M11 절 신설), `README.md`, `CLAUDE.md`(Architecture·Commands M11 반영)

**Interfaces:**
- Consumes: Task 9 산출 + 사전등록의 판정 규칙
- Produces: 병합(주지표 충족 시) 또는 비병합 기록, 문서 일체

- [ ] **Step 1: 사전등록 규칙으로 판정**

주지표 ①·② 우세 + 열세 축(zero_mut finished 비증가·스포트 ≥33/36·fm 비악화) 무저촉이면 승. 경계선이면 대조군 재측정을 사전등록 개정으로 제안(스펙 §9). 판정 근거를 report.md에 전수 기재(소표본은 런 나열).

- [ ] **Step 2-a: 승 → 병합**

```bash
git checkout main
git merge --no-ff m11/status-note -m "feat(agent): M11 병합 — 진행 상태 접지 상태선+파이프 안내 (실험 2 판정)"
cargo test && cargo clippy --all-targets -- -D warnings
cargo run -- eval tasks --verify && cargo run -- eval tasks-large --verify
```

Expected: 전건 통과. 승자 브랜치는 병합 후 삭제 가능(병합 커밋이 이력을 담당) — 보존 의무는 **패자** 브랜치에만 있다(M10 관례: m10/arm-block 보존 전례).

- [ ] **Step 2-b: 패 → 비병합 기록**

`m11/status-note` 보존(비병합), report.md에 기각 근거·실패 전이 분석(어느 지표가 어디로 샜나), 백로그 갱신.

- [ ] **Step 3: 문서 갱신 + Commit**

- `docs/baselines.md`: "M11 실험 2" 절 — 스탬프·수치·판정, 대조군 재사용 각주
- `README.md`: M11 요지 1-2문장
- `CLAUDE.md`: 헤더 M1-M11, Architecture에 상태선·파이프 안내 요약(영문), 실험 2 스탬프 포인터

```bash
git add docs/baselines.md README.md CLAUDE.md docs/experiments/2026-07-18-progress-grounding/report.md
git commit -m "docs: M11 판정·기준선·상태 갱신 — 실험 2 결과 반영"
```
