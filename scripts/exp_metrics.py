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
import contextlib
import glob
import io
import json
import os
import sys
import tempfile

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
    # M12 T9 수정(리뷰 Item 4·컨트롤러 결정): agent/mod.rs의 salvage 역방향 규칙
    # 노트 2종. 부분문자열은 Rust 리터럴에서 문자 그대로 복사(백틱·따옴표 포함).
    "args_tool_key": "the `tool` key inside \"args\" is not a parameter",
    "args_tool_switch": "\"args\" named a different tool, so this call was dispatched as that tool instead",
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

# M12 T9 수정(리뷰 Item 1) — agent/repetition.rs::MAX_SR_CORRECTIONS과 값 재고정.
# 런당 SR_CORRECTION 총 발화 상한(파일별 래치 완화의 풍선효과 방지선, 스펙 §4-1/§7).
MAX_SR_CORRECTIONS = 3


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
    # M12 T9 수정(리뷰 Item 1) — SR_CORRECTION 실 발화를 repetition.rs::error_correction()과
    # 동일한 술어(연속 스트릭>=2 ∨ 파일별 누적>=2, 파일별 래치, 런당 상한
    # MAX_SR_CORRECTIONS)로 재현한다. sr_corr_total은 그중 "파일별 누적 단독으로
    # 도달"(streak<2 ∧ cum>=2)한 발화만 센다 — 연속 2 경로는 파일별 완화가 아니어도
    # 이미 발화했을 경로이므로 풍선효과 감시 대상이 아니다(스펙 §4-1·§7 watchdog).
    sr_latched = set()      # 이미 교정을 발화한 파일 (파일별 래치)
    sr_correction_count = 0  # 런당 총 발화 수
    sr_corr_total = 0        # 파일별 누적 단독 귀속 발화 수 (신규 컬럼)
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
                # error_correction()의 SR 발화 판정과 같은 자리(이번 오류를 반영한
                # 갱신 직후) — reached는 연속 스트릭 ∨ 파일별 누적, 래치는 파일별,
                # 상한은 런당 총 MAX_SR_CORRECTIONS
                reached = streak >= 2 or sr_cum[pathn] >= 2
                if reached and pathn not in sr_latched and sr_correction_count < MAX_SR_CORRECTIONS:
                    sr_latched.add(pathn)
                    sr_correction_count += 1
                    if streak < 2:
                        sr_corr_total += 1  # 연속 경로가 아니라 파일별 누적 단독 귀속
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
                # record_mutation_ok 상당 — 성공 뮤테이션은 그 파일의 누적과 래치를
                # 함께 지운다(M12 §4-1: 재발한 루프는 별개 사건이라 다시 교정받는다)
                mutated_path = normalize_path(str(args.get("path") or ""))
                sr_cum.pop(mutated_path, None)
                sr_latched.discard(mutated_path)
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
            first_mut_turn, cargo_after_mut, status_in_args, sr_files, perturb_ext,
            sr_corr_total)


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
         first_mut, cargo_mut, st_args, sr_files, perturb_ext,
         sr_corr_total) = run_metrics(events)
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
        # sr_corr_total(M12 T9 수정 — 리뷰 Item 1): §6이 요구하는 "파일별 S/R
        # 트리거 발동 카운트"·§7 풍선효과 watchdog는 기존 sr_correction 마커의
        # 단순 재노출로는 무의미(파일별 신호가 전혀 없음 — 최초 구현의 결함).
        # run_metrics()가 repetition.rs::error_correction()의 실 발화 술어(연속
        # 스트릭>=2 ∨ 파일별 누적>=2, 파일별 래치, 런당 상한 3)를 재현해 그중
        # "파일별 누적 단독 귀속"(연속 경로가 아닌) 발화만 반환 — 더 이상
        # sr_correction과 항상 같지 않다(예: 연속 2로 발화한 경우 sr_correction은
        # 1이어도 sr_corr_total은 0).
        verify_failed = counts["verify_total"] - counts["verify_allpass"]
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
     _, _, _, _, _, _) = run_metrics(events)
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
    _, _, _, _, perturb2, _, _, _, _, _, _, _ = run_metrics(events2)
    assert perturb2 == 1, perturb2  # Denied 턴 1회만 섭동 하 — 리셋 후 재축적 전
    # finish 정지 분류 (리뷰 I-2): FINISH_ERR는 user 이벤트로 남는다
    fin = "Error: finish requires a string `summary` argument, e.g. ..."
    events3 = [ev("tool_result", sr, "edit_file"), ev("user", fin), ev("user", fin)]
    _, _, _, fin_max3, _, last3, _, _, _, _, _, _ = run_metrics(events3)
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
    (c4, _, _, _, _, _, fm4, cm4, sa4, sf4, _, _) = run_metrics(events4)
    assert c4["status_note"] == 1, c4
    assert c4["pipe_note"] == 1, c4
    assert fm4 == 2, fm4          # 첫 성공 뮤테이션은 두 번째 tool_result(write_file)
    assert cm4 == 1, cm4          # 뮤테이션 후 cargo run_command 실행됨
    assert sa4 == 1, sa4          # args 내 [status] 복사 오염 1건
    assert sf4 == {"src/a.rs": 1}, sf4  # S/R 오류 파일 귀속
    # 뮤테이션 0회 런: first_mut_turn == 0 (zero_mut 분류는 process가 outcome으로)
    (_, _, _, _, _, _, fm5, cm5, _, _, _, _) = run_metrics([ev("tool_result", "x", "read_file")])
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

    # M12 T9 수정(리뷰 Item 1): sr_corr_total은 이제 기존 sr_correction 마커의
    # 단순 복제가 아니다 — 연속 스트릭 2로 발화(레거시 경로)한 경우 SR_CORRECTION
    # 텍스트 자체는 나가도(sr_correction==1) 파일별 누적 단독 귀속은 아니므로
    # sr_corr_total==0이어야 한다. 교정 노트는 실제 트랜스크립트처럼 별도
    # user 이벤트로 남는다(session.rs::push_tool_result → record("user", note)).
    events8 = [
        ev("tool_result", sr, "edit_file", {"path": "c.rs"}),
        ev("tool_result", sr, "edit_file", {"path": "c.rs"}),
        ev("user", "Your `replace` is identical to `search`. Write the MODIFIED code in "
           "`replace`. If you cannot produce a different `replace`, rewrite the whole file with "
           "write_file, applying the fix."),
    ]
    r8 = run_metrics(events8)
    counts8, sr_corr_total8 = r8[0], r8[11]
    assert counts8["sr_correction"] == 1, counts8    # 연속 2로 실제 발화(레거시 경로)
    assert sr_corr_total8 == 0, sr_corr_total8       # 그러나 파일별 단독 귀속은 아님 — 더 이상 항등이 아니다

    # 대조군: 비연속 재발로 파일별 누적만 2에 도달(연속 스트릭은 1) — 이번엔
    # 파일별 단독 귀속이라 sr_corr_total==1. c.rs와 사건이 겹치지 않도록 d.rs 사용.
    events8b = [
        ev("tool_result", sr, "edit_file", {"path": "d.rs"}),
        ev("tool_result", "ok read", "read_file", {"path": "d.rs"}),  # 연속 스트릭 리셋, 누적은 보존
        ev("tool_result", sr, "edit_file", {"path": "d.rs"}),
    ]
    sr_corr_total8b = run_metrics(events8b)[11]
    assert sr_corr_total8b == 1, sr_corr_total8b

    # --- M12 T9 수정(리뷰 Item 3): perturb_turns_ext/sr_corr_total 재현부의
    # 최소핀 3종 — Rust repetition.rs의 T7 가드(459-471행)와 대응하는 뮤턴트를
    # 각각 킬한다. 세 경우 모두 손상 시 정상 시나리오에서 값이 달라진다.

    # (a) record_mutation_ok 상당(sr_cum.pop) 삭제 킬: 성공 뮤테이션 후 같은
    # 파일 누적이 지워지지 않으면, 재발 1회만으로도(실제로는 2회째) 조기 발화한다.
    events9_pop = [
        ev("tool_result", sr, "edit_file", {"path": "a.rs"}),               # cum[a.rs]=1
        ev("tool_result", "wrote", "write_file", {"path": "a.rs", "content": "x"}),  # 성공 — pop 있어야 cum 리셋
        ev("tool_result", sr, "edit_file", {"path": "a.rs"}),               # pop 정상: cum=1(미도달). 삭제되면: cum=2(조기 발화)
    ]
    sr_corr_total_pop = run_metrics(events9_pop)[11]
    assert sr_corr_total_pop == 0, sr_corr_total_pop  # pop 삭제 뮤턴트에서는 1이 된다 — 킬

    # (b) normalize_path를 항등함수로 바꾸는 뮤턴트 킬: "./a.rs"와 "a.rs"가
    # 같은 파일로 합산되지 않으면 누적이 갈라져 2에 도달하지 못한다.
    events9_normalize = [
        ev("tool_result", sr, "edit_file", {"path": "./a.rs"}),  # normalize 정상: 키 "a.rs", cum=1
        ev("tool_result", "ok read", "read_file", {"path": "a.rs"}),
        ev("tool_result", sr, "edit_file", {"path": "a.rs"}),    # 같은 키로 합산돼야 cum=2
    ]
    sr_corr_total_norm = run_metrics(events9_normalize)[11]
    assert sr_corr_total_norm == 1, sr_corr_total_norm  # 항등함수 뮤턴트에서는 0이 된다 — 킬

    # (c) 성공(비-오류) 분기의 last_sr_file = None 삭제 뮤턴트 킬(T7 가드와 동일
    # 대상, repetition.rs 459-471행). a.rs SR 2연속으로 파일 누적을 2까지 올린
    # 뒤, 무관 파일(b.rs) 성공 뮤테이션 턴을 거쳐, 그다음 턴에서 last_sr_file이
    # 제대로 풀렸는지를 perturb_turns_ext로 관찰한다.
    events9_reset = [
        ev("tool_result", sr, "edit_file", {"path": "a.rs"}),  # cum[a.rs]=1, streak=1
        ev("tool_result", sr, "edit_file", {"path": "a.rs"}),  # cum[a.rs]=2, streak=2, last_sr_file=a.rs
        ev("tool_result", "wrote b.rs", "write_file", {"path": "b.rs", "content": "y"}),
        # ↑ 이 턴 자신은 이전 상태(streak=2)로 이미 섭동에 잡힌다 — 관찰 대상은 다음 턴
        ev("tool_result", "exit code: 0\nok", "run_command", {"command": "true"}),
        # ↑ last_sr_file이 정상 해제됐다면 여기서 파일별 누적으로 섭동에 안 잡혀야 한다
    ]
    perturb_ext_reset = run_metrics(events9_reset)[10]
    assert perturb_ext_reset == 1, perturb_ext_reset  # 리셋 삭제 뮤턴트에서는 2가 된다(마지막 턴이 스테일 a.rs 누적으로 오발화) — 킬

    # M12 T9 수정(리뷰 Item 4·컨트롤러 결정): agent/mod.rs::ARGS_TOOL_KEY_NOTE·
    # ARGS_TOOL_SWITCH_NOTE 마커 카운트. 문구는 Rust 리터럴에서 그대로 복사.
    events_salvage_reverse = [
        ev("user", "note: the `tool` key inside \"args\" is not a parameter - it was removed. "
           "Put only the tool's own parameters inside \"args\"."),
        ev("user", "note: \"args\" named a different tool, so this call was dispatched as that tool "
           "instead. Put the tool name only in \"action\".\"tool\"."),
    ]
    counts_notes = run_metrics(events_salvage_reverse)[0]
    assert counts_notes["args_tool_key"] == 1, counts_notes
    assert counts_notes["args_tool_switch"] == 1, counts_notes

    # M12 T9 수정(리뷰 Item 2): verify_failed·sr_corr_total은 이제 process()를
    # 실제로 호출해 출력 테이블 값으로 검증한다(로컬 재계산이 아니라 실 코드
    # 경로) — process() 내부의 두 파생식을 각각 0으로 바꾸는 뮤턴트가 살아남던
    # 결함(리뷰 확인)을 여기서 잡는다. verify는 위 events7과 동일 시나리오,
    # sr_corr_total은 위 events8b(파일별 단독 귀속)와 동일 시나리오를 재사용.
    with tempfile.TemporaryDirectory() as stamp_dir:
        report = {"tasks": [{"name": "demo", "runs": [{"repeat": 0, "outcome": "finished", "passed": True}]}]}
        with open(os.path.join(stamp_dir, "report.json"), "w") as f:
            json.dump(report, f)
        with open(os.path.join(stamp_dir, "run-demo-0.jsonl"), "w") as f:
            for e in events7 + events8b:
                f.write(json.dumps(e) + "\n")
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            process(stamp_dir)
        out_lines = buf.getvalue().rstrip("\n").split("\n")
        header = out_lines[1].split("\t")
        row = out_lines[2].split("\t")
        col = {name: i for i, name in enumerate(header)}
        assert row[col["verify_failed"]] == "1", row       # process()의 뺄셈식이 실제로 실행됐는지
        assert row[col["sr_corr_total"]] == "1", row        # process()가 run_metrics의 실 반환값을 그대로 쓰는지

    print("selftest ok")


if __name__ == "__main__":
    if len(sys.argv) >= 2 and sys.argv[1] == "--selftest":
        selftest()
    elif len(sys.argv) >= 2:
        for d in sys.argv[1:]:
            process(d)
    else:
        sys.exit(__doc__)  # 인자 없는 호출은 실패가 의도 — usage를 stderr로, exit 1
