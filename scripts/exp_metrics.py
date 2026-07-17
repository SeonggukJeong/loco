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
        # 가정: 교정 노트(별도 user 이벤트)가 정지 턴에 겹치면 last_body가 교정
        # 노트 쪽으로 덮여 오분류될 수 있으나, 교정 래치가 런당 1회라 정지 턴과
        # 겹칠 조건 자체가 현재 코드에서는 실질 도달 불가.
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
