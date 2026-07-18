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
    "status_note": "[status] files edited",
    "pipe_note": "the exit code reflects only the last command",
}
COLS = ["run", "outcome", "passed"] + list(MARKS) + [
    "sr_recovered", "sr_recovery_denom", "finish_missing_maxrun", "perturb_turns", "stop_cause",
    "first_mut_turn", "cargo_after_mut", "zero_mut_end", "status_in_args", "sr_files",
]


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
        # finish 인자누락 연속 스트릭(스펙 §7-3): tool_result가 끼면 리셋
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


def stop_cause(outcome, last_result):
    # last_result는 run_metrics의 last_body(정지 직전 마지막 비-assistant 이벤트
    # 본문) — 교정 노트가 같은 턴에 겹쳐 last_body를 덮으면 오분류될 수 있으나,
    # 교정 래치가 런당 1회라 정지 턴과 겹칠 조건 자체가 실질 도달 불가.
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
    marks = " ".join(f"{k}={totals[k]}" for k in MARKS)
    print(f"# summary {marks} recovered={total_rec}/{total_den} "
          f"stops sr={stops['sr']} finish={stops['finish']} other={stops['other']} "
          f"zero_mut max_turns={zero_mut['max_turns']} finished={zero_mut['finished']} "
          f"other={zero_mut['other']} cargo_after_mut={cargo_runs}/{mut_runs}")


def selftest():
    def ev(kind, content, tool=None, args=None):
        e = {"kind": kind, "content": content}
        if tool:
            e["tool"] = tool
        if args is not None:
            e["args"] = args
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
    (counts, rec, den, fin_max, perturb, _,
     _, _, _, _) = run_metrics(events)
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
    _, _, _, _, perturb2, _, _, _, _, _ = run_metrics(events2)
    assert perturb2 == 1, perturb2  # Denied 턴 1회만 섭동 하 — 리셋 후 재축적 전
    # finish 정지 분류 (리뷰 I-2): FINISH_ERR는 user 이벤트로 남는다
    fin = "Error: finish requires a string `summary` argument, e.g. ..."
    events3 = [ev("tool_result", sr, "edit_file"), ev("user", fin), ev("user", fin)]
    _, _, _, fin_max3, _, last3, _, _, _, _ = run_metrics(events3)
    assert fin_max3 == 2, fin_max3
    assert stop_cause("repetition_stop", last3) == "finish"
    assert stop_cause("repetition_stop", sr) == "sr"
    assert stop_cause("finished", sr) == "-"
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
    print("selftest ok")


if __name__ == "__main__":
    if len(sys.argv) >= 2 and sys.argv[1] == "--selftest":
        selftest()
    elif len(sys.argv) >= 2:
        for d in sys.argv[1:]:
            process(d)
    else:
        sys.exit(__doc__)  # 인자 없는 호출은 실패가 의도 — usage를 stderr로, exit 1
