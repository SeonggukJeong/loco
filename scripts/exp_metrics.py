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
    "empty_test_note": "0 tests ran (",
    "verify_total": "verification: last cargo test: ",
    "verify_zero": "verification: last cargo test ran 0 tests",
    "verify_allpass": "verification: last cargo test: all ",
}
COLS = ["run", "outcome", "passed"] + list(MARKS) + [
    "sr_recovered", "sr_recovery_denom", "finish_missing_maxrun", "perturb_turns", "stop_cause",
    "first_mut_turn", "cargo_after_mut", "zero_mut_end", "status_in_args", "sr_files",
    "verify_failed", "sr_corr_total", "perturb_turns_ext",
]

# M12 §3-1 badargs_streak()이 세는 "missing field" BadArgs 접두 — tools/mod.rs
# 스키마 에코 경로와 agent/repetition.rs::BADARGS_KEY_PREFIX(모듈 비공개,
# pub 아님)를 텍스트로 재고정. 이 파일은 별도 카피를 유지한다(파이썬↔러스트
# 크로스 임포트 불가 — badargs_key_prefix_matches_actual_missing_field_errors_only
# 러스트 테스트가 원본 접두를 도구 오류문과 교차 핀한다)
BADARGS_KEY_PREFIX = "Error: invalid arguments: missing field"


def normalize_path(path):
    """agent/status_note.rs::normalize의 파이썬 재구현(§4-1 파일별 누적 키와
    동일 정규화라야 sr_by_file 카운터가 표기 변형을 합산한다). CurDir 제거·
    ParentDir 팝, 파일시스템 조회 없음 — POSIX 구분자만 가정(모델 산출물은
    슬래시 경로가 절대다수)."""
    absolute = path.startswith("/")
    parts = []
    for seg in path.split("/"):
        if seg in ("", "."):
            continue
        if seg == "..":
            if parts:
                parts.pop()
            continue
        parts.append(seg)
    joined = "/".join(parts)
    return f"/{joined}" if absolute else joined


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
    # M12 §4-1 확대 트리거 재구성 — agent/repetition.rs::RepetitionTracker의
    # sr_by_file(파일별 누적)·badargs_streak를 텍스트만으로 재추적한다.
    # 온도 오버라이드 자체는 전송 파라미터라 트랜스크립트에 남지 않으므로
    # (기존 perturb_turns과 동일 사정) 반드시 상태기계 재현이 필요 — 마커
    # 존재 여부만 세는 나머지 신규 컬럼과 달리 시뮬레이션 기반이다.
    sr_cum = {}          # normalize(path) -> 누적 S/R 오류 수
    last_sr_file = None  # 가장 최근 S/R 오류의 normalize(path)
    badargs_streak = 0
    perturb_ext = 0
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
        # M12 §4-1 확대 판정 — 위와 같은 자리, 같은 "직전 요청" 시점(이번 턴
        # 자신의 갱신 전 상태)에서 파일별 누적·badargs 스트릭도 함께 본다
        sr_file_val = sr_cum.get(last_sr_file, 0) if last_sr_file else 0
        if (streak_key == SR_KEY and streak >= 2) or sr_file_val >= 2 or badargs_streak >= 2:
            perturb_ext += 1
        if content.startswith("Error:"):
            key = content.split(".")[0]
            streak = streak + 1 if key == streak_key else 1
            streak_key = key
            if tool == "edit_file" and key == SR_KEY:
                pathn = normalize_path(str(args.get("path") or ""))
                sr_cum[pathn] = sr_cum.get(pathn, 0) + 1
                last_sr_file = pathn
            else:
                last_sr_file = None
        else:
            streak_key, streak = None, 0
            last_sr_file = None
        badargs_streak = badargs_streak + 1 if content.startswith(BADARGS_KEY_PREFIX) else 0
        # 회복 판정: S/R 오류 후 다음 2번의 edit/write 시도 안에 성공
        if tool in ("edit_file", "write_file"):
            ok = not (content.startswith("Error:") or content.startswith("Denied:"))
            if ok and not first_mut_turn:
                first_mut_turn = tool_turn
            if any("[status]" in str(args.get(f, "")) for f in ("search", "replace", "content")):
                status_in_args += 1
            if ok:
                # record_mutation_ok 상당 — 성공 뮤테이션은 그 파일의 누적을 지운다
                sr_cum.pop(normalize_path(str(args.get("path") or "")), None)
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
            first_mut_turn, cargo_after_mut, status_in_args, sr_files, perturb_ext)


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
         first_mut, cargo_mut, st_args, sr_files, perturb_ext) = run_metrics(events)
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
        # M12 §6 파생 컬럼: verify_failed(규칙 2)는 verify_total(규칙 2·4 합)에서
        # verify_allpass(규칙 4)를 뺀 나머지 — 실패 개수가 가변이라 고정
        # 부분문자열로 못 잡으므로 뺄셈으로 도출한다(브리프 Step 1).
        # sr_corr_total은 SR_CORRECTION 총 발화 수 — 파일별/연속별 두 트리거가
        # 겹칠 때 트랜스크립트만으로 원인 경로를 구분할 수 없어(브리프 Step 1
        # "구분할 수 없으면 구분하지 말 것") 총수로 둔다(기존 sr_correction
        # 마커와 같은 텍스트를 세므로 값은 일치 — 별도 컬럼명은 이 태스크의
        # 파일별 조사 맥락에서 의도를 분명히 하기 위함)
        verify_failed = counts["verify_total"] - counts["verify_allpass"]
        sr_corr_total = counts["sr_correction"]
        row = [name, outcome, str(passed)] + [str(counts[k]) for k in MARKS]
        row += [str(rec), str(den), str(fin_max), str(perturb), cause,
                str(first_mut), str(cargo_mut), zme, str(st_args), files_col,
                str(verify_failed), str(sr_corr_total), str(perturb_ext)]
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
     _, _, _, _, _) = run_metrics(events)
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
    _, _, _, _, perturb2, _, _, _, _, _, _ = run_metrics(events2)
    assert perturb2 == 1, perturb2  # Denied 턴 1회만 섭동 하 — 리셋 후 재축적 전
    # finish 정지 분류 (리뷰 I-2): FINISH_ERR는 user 이벤트로 남는다
    fin = "Error: finish requires a string `summary` argument, e.g. ..."
    events3 = [ev("tool_result", sr, "edit_file"), ev("user", fin), ev("user", fin)]
    _, _, _, fin_max3, _, last3, _, _, _, _, _ = run_metrics(events3)
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
    (c4, _, _, _, _, _, fm4, cm4, sa4, sf4, _) = run_metrics(events4)
    assert c4["status_note"] == 1, c4
    assert c4["pipe_note"] == 1, c4
    assert fm4 == 2, fm4          # 첫 성공 뮤테이션은 두 번째 tool_result(write_file)
    assert cm4 == 1, cm4          # 뮤테이션 후 cargo run_command 실행됨
    assert sa4 == 1, sa4          # args 내 [status] 복사 오염 1건
    assert sf4 == {"src/a.rs": 1}, sf4  # S/R 오류 파일 귀속
    # 뮤테이션 0회 런: first_mut_turn == 0 (zero_mut 분류는 process가 outcome으로)
    (_, _, _, _, _, _, fm5, cm5, _, _, _) = run_metrics([ev("tool_result", "x", "read_file")])
    assert (fm5, cm5) == (0, 0), (fm5, cm5)

    # M12 §4-1: perturb_turns_ext 확대 — 파일별 비연속 누적(cum>=2)이 섭동을
    # 유도하는데, 기존 perturb_turns(연속 전용 재구성)는 이 경로를 놓친다.
    # a.rs SR → 무관 read(연속 스트릭 리셋, 누적은 보존) → a.rs SR 재발(cum=2)
    # → 다음 결과 턴에서 perturb_ext만 발화(연속 스트릭은 1에 불과)
    events5 = [
        ev("tool_result", sr, "edit_file", {"path": "a.rs"}),
        ev("tool_result", "ok read", "read_file", {"path": "a.rs"}),
        ev("tool_result", sr, "edit_file", {"path": "a.rs"}),
        ev("tool_result", "ok read again", "read_file", {"path": "a.rs"}),
    ]
    r5 = run_metrics(events5)
    perturb5, perturb_ext5 = r5[4], r5[10]
    assert perturb5 == 0, perturb5          # 연속 전용 재구성은 이 비연속 재발을 못 잡는다
    assert perturb_ext5 == 1, perturb_ext5  # 파일별 누적 확대 트리거는 잡는다

    # M12 §3-1: missing-field 연속 2도 섭동 확대 트리거 — S/R과 무관한 경로
    badargs = "Error: invalid arguments: missing field `content`. Expected: write_file(path, content)."
    events6 = [
        ev("tool_result", badargs, "write_file"),
        ev("tool_result", badargs, "write_file"),
        ev("tool_result", "wrote", "write_file", {"path": "x.rs", "content": "y"}),
    ]
    r6 = run_metrics(events6)
    perturb6, perturb_ext6 = r6[4], r6[10]
    assert perturb6 == 0, perturb6          # SR 전용 재구성은 badargs 스트릭을 안 봄
    assert perturb_ext6 == 1, perturb_ext6  # badargs_streak>=2도 확대 트리거에 포함

    # M12 §2-2·§2-3: 0-테스트 무효화 노트 + 상태선 verification 5규칙 중
    # 규칙 2·3·4의 렌더 분포. verify_total은 규칙 2·4 양쪽에 매치되므로
    # verify_failed는 뺄셈으로 파생한다(브리프 Step 1) — (1,1,1)과 파생값 1.
    events7 = [
        ev("tool_result", "exit code: 0\nnote: 0 tests ran (13 filtered out) - cargo test filters "
           "by test NAME, not file name; this exit 0 did not verify anything", "run_command"),
        ev("user", "[status] files edited: 1 (a.rs)\n"
           "         verification: last cargo test ran 0 tests (filter matched nothing)\n"
           "         turns: 5 of 25 used"),
        ev("user", "[status] files edited: 1 (a.rs)\n"
           "         verification: last cargo test: all 5 passed\n"
           "         turns: 10 of 25 used"),
        ev("user", "[status] files edited: 1 (a.rs)\n"
           "         verification: last cargo test: 3 failed (alpha, beta and 1 more)\n"
           "         turns: 15 of 25 used"),
    ]
    counts7 = run_metrics(events7)[0]
    assert counts7["empty_test_note"] == 1, counts7   # run_command 노트(§2-2) — 상태선 규칙 3 문구와 겹치지 않음
    assert counts7["verify_zero"] == 1, counts7        # 규칙 3
    assert counts7["verify_allpass"] == 1, counts7     # 규칙 4
    assert counts7["verify_total"] == 2, counts7        # 규칙 2·4 렌더 합(규칙 3 미포함)
    verify_failed7 = counts7["verify_total"] - counts7["verify_allpass"]
    assert verify_failed7 == 1, verify_failed7          # 규칙 2 렌더 수(파생)

    # M12 §4-1: SR_CORRECTION 발화 총수 — 파일별/연속 두 트리거가 겹치면
    # 트랜스크립트만으로 원인 경로를 구분할 수 없어(브리프 "구분할 수 없으면
    # 구분하지 말 것") sr_corr_total은 기존 sr_correction 마커 총수와 같다.
    events8 = [
        ev("tool_result", sr, "edit_file", {"path": "c.rs"}),
        ev("tool_result", "Your `replace` is identical to `search`. Write the MODIFIED code in "
           "`replace`. If you cannot produce a different `replace`, rewrite the whole file with "
           "write_file, applying the fix.", "edit_file", {"path": "c.rs"}),
    ]
    counts8 = run_metrics(events8)[0]
    assert counts8["sr_correction"] == 1, counts8
    sr_corr_total8 = counts8["sr_correction"]
    assert sr_corr_total8 == 1, sr_corr_total8

    print("selftest ok")


if __name__ == "__main__":
    if len(sys.argv) >= 2 and sys.argv[1] == "--selftest":
        selftest()
    elif len(sys.argv) >= 2:
        for d in sys.argv[1:]:
            process(d)
    else:
        sys.exit(__doc__)  # 인자 없는 호출은 실패가 의도 — usage를 stderr로, exit 1
