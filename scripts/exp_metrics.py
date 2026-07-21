#!/usr/bin/env python3
"""loco 실험 지표 추출기 (M10 §7-3).

usage:
  python3 scripts/exp_metrics.py .loco/eval/<stamp> [...]   # 런별 TSV + 요약
  python3 scripts/exp_metrics.py --selftest                  # 내장 샘플 자기검증
  python3 scripts/exp_metrics.py --pool <stamp> [<stamp> ...] [--resamples N] [--seed N]
                                                               # M15 H15 후반부 — §6-1의
                                                               # --filter 분할을 배치 수준
                                                               # 하나로 되돌리는 풀링 집계
  python3 scripts/exp_metrics.py --session <session.jsonl>   # M15 H19 — 스모크 1세션
                                                               # 트랜스크립트에서 r_obs 산출
                                                               # (report.json 없이)

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
    # ⚠ M14 비교가능성: §3-4-2의 규칙 4 → 규칙 5 폴백이 파이프 실행의 allpass 렌더를
    # 규칙 5 문자열로 옮긴다. 모델이 파이프를 쓰는 만큼 verify_allpass·verify_total이
    # **내려간다** — 하락은 회귀가 아니라 폴백이 작동한 증거일 수 있다.
    # 파생 verify_failed(= total - allpass)는 규칙 2가 불변이고 두 원지표가 같은 양만큼
    # 줄어 보존된다. M14 전후 배치의 이 두 지표를 나란히 인용하지 말 것.
    # 선례: M12 sr_error(검사 순서), M13 T7 verify_*(무뮤테이션 렌더로 상향)
    "verify_total": "verification: last cargo test: ",
    "verify_zero": "verification: last cargo test ran 0 tests",
    "verify_allpass": "verification: last cargo test: all ",
    # M12 T9 수정(리뷰 Item 4·컨트롤러 결정): agent/mod.rs의 salvage 역방향 규칙
    # 노트 2종. 부분문자열은 Rust 리터럴에서 문자 그대로 복사(백틱·따옴표 포함).
    "args_tool_key": "the `tool` key inside \"args\" is not a parameter",
    "args_tool_switch": "\"args\" named a different tool, so this call was dispatched as that tool instead",
    # M13 T10 리뷰 수정(Important 1) — agent/mod.rs가 finish_reason=="length"일
    # 때 session.push(ChatMessage::user(...))로 남기는 고정 재시도 문구(직접
    # 확인, src/agent/mod.rs 234-239행). 러스트 리터럴에서 문자 그대로 복사.
    # 이 파일이 손으로 미러링하는 상수라 드리프트 감시 대상이다 — 위
    # BADARGS_KEY_PREFIX·args_tool_key/switch와 같은 사정(크로스임포트 불가,
    # 자동 드리프트 감지 없음): 러스트 쪽 문구가 바뀌면 이 리터럴도 사람이
    # 함께 고쳐야 한다.
    "length_retry": "cut off by the output token limit",
    # M14 T10 — A-1(파이프 가드가 해제 술어에만 걸림)·A-2(상태선 규칙 5 파이프
    # 한정자)·A-3(모델용 diff 헤더) 소비자 신호. 문자열은 Rust 상수에서 문자
    # 그대로 복사(드리프트 감시 대상 — 위 args_tool_key/switch·length_retry와
    # 같은 사정, 자동 검출 없음):
    #   verify_nudge_pipe   → agent/mod.rs::VERIFY_NUDGE_PIPE
    #   finish_nudge_pipe   → agent/finish_nudge.rs::FINISH_NUDGE_PIPE
    #   status_pipe_qual/status_no_summary → agent/status_note.rs 규칙 5 한정자
    #   model_diff          → tools/diff.rs::render_diff_for_model 헤더(정상·절단 두 형태 모두 매치)
    "verify_nudge_pipe": "but it was a shell pipeline",
    "finish_nudge_pipe": "was a shell pipeline, so it did not establish",
    "status_pipe_qual": "(via pipe",
    "status_no_summary": "no test summary in output",
    "model_diff": " lines, +",
    # M15 — A-3 절단률(§6-3)의 분자. model_diff는 절단·비절단 양쪽 헤더에 매치해
    # **분모** 역할이다(CLAUDE.md 명시). 문자열은 tools/diff.rs에서 문자 그대로
    # 복사(수동 미러 — 자동 검출 없음, 위 length_retry·args_tool_key/switch와 같은 사정)
    "model_diff_trunc": "[diff truncated]",
    # M16 T5 — repo notes 장치 마커. 문자열은 Rust 상수에서 문자 그대로 복사
    # (드리프트 감시 대상 — 위 args_tool_key/switch·length_retry와 같은 사정):
    #   notes_schema_reject → tools/update_repo_notes.rs::NOTES_SCHEMA_REJECT_PREFIX
    #   notes_mut_gate      → notes/state.rs::NOTES_MUT_GATE_MARK
    #   notes_stale_finish  → notes/state.rs::NOTES_STALE_MARK
    #   notes_updates       → tools/update_repo_notes.rs::NOTES_UPDATE_OK_PREFIX
    "notes_schema_reject": "repo notes schema:",
    "notes_mut_gate": "repo notes mut gate:",
    "notes_stale_finish": "repo notes stale:",
    "notes_updates": "repo notes updated:",
}
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
    # M15 H15 후반부(§5-4·§6-4-19②) — 항해/수선 원자료. nav_hit/fix_hit은
    # "1"|"0"|"-"("-"=오라클 없는 과제, 0이 아니다). reads는 **집합 크기**
    # (고유 파일 수 — "몇 개를 열었나"), greps/lists는 **호출 수**
    # ("몇 번 훑었나") — 단위가 다르다. 배치 집계는 reads/greps/lists 모두 합
    # (est_ratio_max만 최대다)
    "nav_hit", "fix_hit", "reads", "greps", "lists",
    # M16 T5 — 인증 notes 파일 길이의 런 중 최댓값(§5-3). 출처 우선순위:
    # transcript extra kind `notes_bytes_max`(session.record_extra) → 성공
    # `repo notes updated: … (N bytes)` 줄 파싱 max → flag-off/부재 시 "-"
    "notes_bytes_max",
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


def parse_fail_first(events):
    """첫 assistant 메시지가 유효한 에이전트 턴으로 파싱되지 않으면 1.

    M13 스펙 §3-6-1의 기계 검사 — C1형 조용한 전면 실패(json_schema 폴백이
    영구 발동해 매 턴 파싱이 실패하는데 배치는 정상 종료)를 배치 후에
    기계적으로 잡는다.

    Rust의 protocol.rs::parse_turn을 완전 재현하지 않는다. 판별력 있는 최소
    검사만 한다: 코드펜스를 벗기고 JSON 객체를 찾은 뒤, thought가 있고
    action이 tool 키를 가진 객체인지 본다.

    "{" 전무는 예외적으로 확정 판정(1)이다 — 판정 불가가 아니라 증명된
    실패다(T4 리뷰 Finding 1). Rust parse_turn(직접 확인, protocol.rs
    21-46행)의 세 사다리(그대로 파싱 → 펜스 제거 후 파싱 →
    first_json_object 스캔) 중 마지막 관문 first_json_object는
    `text.find('{')?`로 시작한다 — 앞 두 사다리가 전부 폴스루해도 텍스트에
    "{"가 하나도 없으면 이 관문에서 즉시 실패해 "Your reply contained no
    JSON object" 에러로 귀결된다. 즉 세 사다리 전부가 실패하는 게 보장되는
    유일한 무-JSON 형태이며, 이 형태(모델이 순수 산문만 출력)가 바로 이
    컬럼이 잡으려는 실패의 가장 흔한 모양이다. 나머지 미판정 케이스
    ("{"는 있으나 닫는 "}"를 못 찾음/파싱 실패, assistant 자체가 없음)는
    여전히 거짓 양성 금지 원칙에 따라 0으로 둔다.

    알려진 과소검출(T4 리뷰 Finding 2 — 의도적으로 미수선): 산문 속 디코이
    "{...}"가 실제(펜스로 감싼) JSON 턴보다 앞에 오는 메시지에서는, Rust의
    first_json_object가 그 디코이의 첫 균형 중괄호에 커밋해 파싱에
    실패하는 반면, 이 파이썬 검사는 펜스 분리 단계에서 디코이를 건너뛰고
    뒤쪽의 진짜 JSON을 찾아내 0(파싱 성공)을 반환한다 — 두 판정이 갈린다.
    제대로 고치려면 Rust의 문자열 인지 중괄호 스캐너를 그대로 재현해야
    하는데, 이는 브리프가 의도한 "판별력 있는 최소 검사" 설계와 충돌한다.
    비용 대비 이 형태의 실측 사례가 아직 없어 고치지 않고 이 사례만
    기록해 둔다 — 이 검사가 1을 반환하면 그 판정은 (위 no-"{" 사유로)
    믿을 수 있지만, 0을 반환했다고 해서 Rust가 반드시 파싱에 성공했다는
    보장은 아니다.
    """
    for ev in events:
        if ev.get("kind") != "assistant":
            continue
        text = ev.get("content") or ""
        if "{" not in text:
            return 1  # 위 독스트링 참조 — 증명된 실패, 판정 불가 아님
        # 코드펜스 제거
        if "```" in text:
            parts = text.split("```")
            for p in parts:
                p = p.lstrip()
                if p.startswith("json"):
                    p = p[4:]
                if p.lstrip().startswith("{"):
                    text = p
                    break
        start = text.find("{")
        end = text.rfind("}")
        if start < 0 or end <= start:
            return 0  # "{"는 있었으나(위 통과) 닫는 "}"를 못 찾음 — 판정 불가
        try:
            obj = json.loads(text[start:end + 1])
        except (ValueError, TypeError):
            return 0  # 파싱 불가 — 판정 불가
        if not isinstance(obj, dict):
            return 0  # 방어적 belt-and-suspenders: 위 슬라이스는 "{"로 시작·"}"로
            # 끝나도록 구성되므로 json.loads가 성공하면 dict 아닌 결과는 도달 불가
        action = obj.get("action")
        if not isinstance(action, dict) or "tool" not in action:
            return 1  # action이 객체가 아니거나 tool이 없다 = 스키마 미강제 형태
        if "thought" not in obj:
            return 1
        return 0
    return 0  # assistant 없음 — 판정 불가


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
    # ⚠ **아래 넷은 T15가 채우지만 선언은 여기다**. `tok`이 이들을 참조하므로
    # T14만 적용한 상태에서도 정의돼 있어야 한다 — 선언을 T15로 미루고 참조만
    # 여기 두면 T14 단독 적용 시 모든 run_metrics 호출이
    # `NameError: name 'read_set' is not defined`로 죽는다(1R Critical 5와
    # 같은 형태 — 튜플을 바꾸고 소비자를 전수로 안 갱신한 것 — 가 그 Critical을
    # 고치는 자리에서 재발하지 않도록 여기서 함께 선언한다)
    read_set, edit_set = set(), set()   # T15 Step 1이 touch 이벤트로 채운다
    grep_calls, list_calls = 0, 0
    # M16 T5 — notes_bytes_max. extra 가 있으면 **마지막** 유효 값(Rust가
    # scan/write 때마다 현재 bytes_max()를 다시 기록하므로 last == 런 종료 시점
    # 값). 없으면 성공 줄 `(N bytes)` 파싱값의 max. 둘 다 없으면 "-" (flag off).
    notes_bytes_from_extra = None   # int | None — 마지막 유효 extra
    notes_bytes_from_success = None  # int | None — success 줄 max
    for e in events:
        kind, content = e.get("kind"), e.get("content") or ""
        if kind == "assistant":
            continue
        # 마커는 모든 비-assistant 이벤트에서 센다 — 교정 노트는 tool_result가
        # 아니라 별도 user 이벤트로 남는다 (baselines.md 추출 레시피)
        for k, m in MARKS.items():
            counts[k] += content.count(m)
        # 성공 update_repo_notes 본문에서 바이트 수 폴백 파싱
        # (형식: "repo notes updated: .loco/notes/_root.md (420 bytes)")
        if content.startswith(MARKS["notes_updates"]) and content.endswith(" bytes)"):
            open_paren = content.rfind("(")
            if open_paren >= 0:
                num = content[open_paren + 1: -len(" bytes)")]
                try:
                    n = int(num)
                except ValueError:
                    n = None
                if n is not None:
                    notes_bytes_from_success = (
                        n if notes_bytes_from_success is None
                        else max(notes_bytes_from_success, n)
                    )
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
        if kind == "touch":
            t = json.loads(content)
            tool, p = t.get("tool"), t.get("path")
            # ⚠ normalize_path를 여기서도 건다(§6-4-19②). oracle_index()가 정규화한
            # 경로를 쓰므로, 원문 경로를 그대로 넣으면 `./src/x.rs`와 `src/x.rs`가
            # 어긋나 진짜 히트가 nav_hit=0으로 조용히 누락된다. Rust 쪽(agent/mod.rs)이
            # status_note::normalize를 이미 걸지만 두 정규화가 같다는 보장을 코드로
            # 강제할 수단이 없어 여기서도 건다 — 멱등이다
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
        # M16 T5 — kind 는 notes/state.rs::NOTES_BYTES_MAX_KIND ("notes_bytes_max")
        # 와 문자 일치. last_body 오염 방지를 위해 반드시 continue (usage/pack 선례).
        if kind == "notes_bytes_max":
            try:
                notes_bytes_from_extra = int(content.strip())
            except ValueError:
                pass
            continue
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
    # M16 T5: extra 우선, 없으면 성공 줄 max, 둘 다 없으면 "-" (flag off / 미기록)
    if notes_bytes_from_extra is not None:
        notes_bytes_max = str(notes_bytes_from_extra)
    elif notes_bytes_from_success is not None:
        notes_bytes_max = str(notes_bytes_from_success)
    else:
        notes_bytes_max = "-"
    tok = {
        "max_prompt": max_prompt, "max_est": max_est,
        "est_ratio_max": est_ratio_max, "budget_ratio_max": budget_ratio_max,
        "pack_turns": pack_turns, "pack_elided": pack_elided, "pack_dropped": pack_dropped,
        "overflow_shrink": overflow_shrink, "overflow_giveup": overflow_giveup,
        "inline_sys_turns": inline_sys_turns,
        "usage_rows": usage_rows,          # §5-3 회귀 입력
        "read_set": read_set, "edit_set": edit_set,   # T15가 채운다
        "grep_calls": grep_calls, "list_calls": list_calls,
        "notes_bytes_max": notes_bytes_max,  # M16 T5 — 튜플 arity 안 건드림
    }
    return (counts, recovered, denom, fin_max, perturb_turns, last_body,
            first_mut_turn, cargo_after_mut, status_in_args, sr_files, perturb_ext,
            sr_corr_total, tok)


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


def oracle_index(stamp_dir):
    """과제 이름 → 오라클 소스 파일 목록 (M15 H15 후반부·§5-4 입력 계약).

    report.json에 **동결**된 것을 읽는다 — 별도 파일에서 읽으면 사후 변경이
    가능해진다. 경로는 normalize_path로 정규화해 트랜스크립트의 표기 변형
    (`./src/x.rs` vs `src/x.rs`)과 합산되게 한다. procure.toml이 없는 과제
    (기존 두 트리 — tasks/, tasks-large/)는 procure 자체가 null이라 빈 목록을
    준다 — 오라클 없음은 §6-4-19①이 "해당 없음"으로 다루는 그 상태다."""
    out = {}
    path = os.path.join(stamp_dir, "report.json")
    if not os.path.exists(path):
        return out
    for t in json.load(open(path)).get("tasks", []):
        pr = t.get("procure") or {}
        out[t["name"]] = [normalize_path(f) for f in pr.get("oracle_files", [])]
    return out


def process(stamp_dir):
    idx = report_index(stamp_dir)
    oracle_by_task = oracle_index(stamp_dir)
    rows = []
    print(f"# {stamp_dir}")
    print("\t".join(COLS))
    totals = dict.fromkeys(MARKS, 0)
    total_rec, total_den = 0, 0
    stops = {"sr": 0, "finish": 0, "other": 0}
    zero_mut = {"max_turns": 0, "finished": 0, "other": 0}
    mut_runs, cargo_runs = 0, 0
    parse_fail_total = 0
    # M15 축 C 배치 수준 누산기. est_ratio_max/max_prompt는 런별 값의 최댓값
    # (§4-1-1의 r_obs 정의 — 평균이 아니다), 나머지는 배치 총합이다
    batch_max_prompt = 0
    batch_ratio = 0.0
    batch_pack, batch_shrink, batch_giveup, batch_prot = 0, 0, 0, 0
    for path in sorted(glob.glob(os.path.join(stamp_dir, "run-*.jsonl"))):
        events = [json.loads(l) for l in open(path)]
        (counts, rec, den, fin_max, perturb, last,
         first_mut, cargo_mut, st_args, sr_files, perturb_ext,
         sr_corr_total, tok) = run_metrics(events)
        name = os.path.basename(path).removesuffix(".jsonl")
        outcome, passed, protected_edits, task_name = idx.get(name, ("?", None, 0, "?"))
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
        pff = parse_fail_first(events)
        parse_fail_total += pff
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
        # M14 T10 파생 컬럼 2종 — 신규 마커 카운트만으로는 직접 못 얻는 §8-4 관측
        # 항목 2건의 해소.
        # finish_nudge_total: agent/finish_nudge.rs::FINISH_NUDGE가 발동하는 순간
        # 파이프가 원인(unreleased_due_to_pipe)이면 FINISH_NUDGE_PIPE로 치환되어
        # 나간다(agent/mod.rs 508·634행) — 기본 문구(finish_nudge 마커)와 파이프
        # 문구(finish_nudge_pipe 마커)는 상호배타적 발동이라 단순 합이 "FINISH_NUDGE
        # 발동 총 횟수"다(스펙 §8-4).
        # pipe_unreleased: `unreleased_due_to_pipe`는 렌더되는 문자열이 아니라 하네스
        # 내부 상태라 직접 셀 마커가 없다. 그런데 agent/mod.rs 595행의 대입
        # (`unreleased_due_to_pipe = !released && is_piped`)은 run_command가
        # ExecEnd::Done에 도달한 파이프 명령마다 참이 되고, 이는 tools/run_command.rs가
        # 같은 has_unquoted_pipe 판정으로 붙이는 M11 pipe_note와 발동 조건이 정확히
        # 같다 — 그래서 새 마커를 만들지 않고 pipe_note를 그대로 재사용한다.
        # 알려진 과소검출(고의로 안 고침): 파이프 명령이 타임아웃·취소로 끝나면
        # run_command 쪽 ExecEnd::Done 분기를 안 타 pipe_note가 안 붙지만,
        # agent/mod.rs의 dispatch_ok는 그 경우도 참(타임아웃도 도구 결과는 Ok)이라
        # 플래그 자체는 여전히 참이 된다 — "파이프이면서 타임아웃"이라는 좁은
        # 교집합만 이 프록시가 놓친다.
        finish_nudge_total = counts["finish_nudge"] + counts["finish_nudge_pipe"]
        pipe_unreleased = counts["pipe_note"]
        # §6-4-19②: 교집합 판정은 §3-4-3과 **동일하게** `≠ ∅`다. 오라클이 없는
        # 과제(기존 두 트리)는 "-" — 0이 아니다. 0으로 찍으면 "항해 실패"로
        # 읽히는데 사실은 "해당 없음"이다(§6-4-19①이 같은 이유로 층 크기 0인
        # 과제를 평균에서 제외한다)
        oracle = set(oracle_by_task.get(task_name, []))
        nav_hit = "-" if not oracle else ("1" if tok["read_set"] & oracle else "0")
        fix_hit = "-" if not oracle else ("1" if tok["edit_set"] & oracle else "0")
        row = [name, outcome, str(passed)] + [str(counts[k]) for k in MARKS]
        row += [str(rec), str(den), str(fin_max), str(perturb), cause,
                str(first_mut), str(cargo_mut), zme, str(st_args), files_col,
                str(verify_failed), str(sr_corr_total), str(perturb_ext), str(pff),
                str(finish_nudge_total), str(pipe_unreleased)]
        row += [str(tok["max_prompt"]), str(tok["max_est"]),
                f"{tok['est_ratio_max']:.4f}", f"{tok['budget_ratio_max']:.4f}",
                str(tok["pack_turns"]), str(tok["pack_elided"]), str(tok["pack_dropped"]),
                str(tok["overflow_shrink"]), str(tok["overflow_giveup"]),
                str(tok["inline_sys_turns"]), str(protected_edits)]
        # reads=집합 크기(고유 파일 수), greps/lists=호출 수 — 단위가 다르다(브리프 Step 2)
        row += [nav_hit, fix_hit, str(len(tok["read_set"])),
                str(tok["grep_calls"]), str(tok["list_calls"]),
                # M16 T5 — 이미 str ("-" | "N"); 튜플 폭 변경 없이 tok 경유
                tok["notes_bytes_max"]]
        print("\t".join(row))
        batch_max_prompt = max(batch_max_prompt, tok["max_prompt"])
        batch_ratio = max(batch_ratio, tok["est_ratio_max"])
        batch_pack += tok["pack_turns"]
        batch_shrink += tok["overflow_shrink"]
        batch_giveup += tok["overflow_giveup"]
        batch_prot += protected_edits
        # ⚠ 풀링 모드 계약(1R 실현 I5) — 타입이 살아 있는 딕셔너리라야 한다.
        # dict(zip(COLS, row))로 만들면 passed 셀이 str(passed)라 "True"가 되고,
        # stratified_rate의 r["passed"] is want가 예외 없이 전부 거짓이 되어
        # 모든 과제가 excluded로 빠지고 mean=nan이 된다. passed는 여기서 bool
        # 그대로(report_index가 json.load로 준 원본 — str()로 감싸지 말 것)
        rows.append({
            "run": name,
            "task": task_name,
            "passed": passed,
            "outcome": outcome,
            "nav_hit": nav_hit,
            "fix_hit": fix_hit,
            "tok": tok,
            "counts": counts,
        })
    marks = " ".join(f"{k}={totals[k]}" for k in MARKS)
    # M14 T10 Step 1 — 이전엔 "parse_fail_first(총): N  <- 0이 아니면 ..." 형태의
    # 한글 키+서술문이었다. 다른 전 필드처럼 key=value 한 항목으로: 의미(0이 아니면
    # 그 배치는 앵커/게이트로 쓸 수 없다는 판정 기준)는 안 바뀌었고, 그 설명은
    # parse_fail_first()의 독스트링에 이미 있다 — grep 레시피가 이 줄을 다른
    # key=value 필드와 동일하게 파싱할 수 있도록 프로즈만 뺐다.
    print(f"# summary {marks} recovered={total_rec}/{total_den} "
          f"stops sr={stops['sr']} finish={stops['finish']} other={stops['other']} "
          f"zero_mut max_turns={zero_mut['max_turns']} finished={zero_mut['finished']} "
          f"other={zero_mut['other']} cargo_after_mut={cargo_runs}/{mut_runs} "
          f"parse_fail_first={parse_fail_total}")
    # M15 축 C 배치 요약. ⚠ est_ratio_max는 런별 값의 최댓값이다(평균이
    # 아니다) — §4-1-1이 r_obs를 그렇게 정의했고 T22의 분기 판정이 이 숫자를
    # 그대로 쓴다
    print(f"# tokens max_prompt={batch_max_prompt} est_ratio_max={batch_ratio:.4f} "
          f"pack_turns={batch_pack} overflow_shrink={batch_shrink} "
          f"overflow_giveup={batch_giveup} protected_edits={batch_prot}")
    return rows


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
    (counts, rec, den, fin_max, perturb, *_) = run_metrics(events)
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
    _, _, _, _, perturb2, *_ = run_metrics(events2)
    assert perturb2 == 1, perturb2  # Denied 턴 1회만 섭동 하 — 리셋 후 재축적 전
    # finish 정지 분류 (리뷰 I-2): FINISH_ERR는 user 이벤트로 남는다
    fin = "Error: finish requires a string `summary` argument, e.g. ..."
    events3 = [ev("tool_result", sr, "edit_file"), ev("user", fin), ev("user", fin)]
    _, _, _, fin_max3, _, last3, *_ = run_metrics(events3)
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
    (c4, _, _, _, _, _, fm4, cm4, sa4, sf4, *_) = run_metrics(events4)
    assert c4["status_note"] == 1, c4
    assert c4["pipe_note"] == 1, c4
    assert fm4 == 2, fm4          # 첫 성공 뮤테이션은 두 번째 tool_result(write_file)
    assert cm4 == 1, cm4          # 뮤테이션 후 cargo run_command 실행됨
    assert sa4 == 1, sa4          # args 내 [status] 복사 오염 1건
    assert sf4 == {"src/a.rs": 1}, sf4  # S/R 오류 파일 귀속
    # 뮤테이션 0회 런: first_mut_turn == 0 (zero_mut 분류는 process가 outcome으로)
    (_, _, _, _, _, _, fm5, cm5, *_) = run_metrics([ev("tool_result", "x", "read_file")])
    assert (fm5, cm5) == (0, 0), (fm5, cm5)

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

    # T9 마무리 2R 수정(리뷰 Minor) — badargs_streak의 리셋(157행: 비-badargs
    # 결과 턴에서 0으로 되돌림)은 이 6번 픽스처의 증가분만으로는 핀되지 않는다.
    # `else: 0`을 `else: badargs_streak`(단조 누적)로 바꿔도 위 두 단언은 그대로
    # 통과한다 — 3번째 턴("wrote", 비-badargs) 자체는 이미 리셋 이전 상태(streak=2)로
    # 확대 트리거에 잡혀 perturb_ext6==1은 불변이기 때문. 리셋이 실제로 걸렸는지는
    # 그 "다음" 결과 턴에서만 관측 가능하므로, 비-badargs 턴(3번, 기존)에 이어
    # 아무 결과 턴이나 하나 더("ok read", 무관 read_file) 추가해 그 턴에서
    # perturb_ext가 더는 늘지 않는지 단언한다.
    events6b = events6 + [ev("tool_result", "ok read", "read_file", {"path": "x.rs"})]
    r6b = run_metrics(events6b)
    perturb_ext6b = r6b[10]
    assert perturb_ext6b == 1, perturb_ext6b  # 리셋 정상: 4번째 턴은 badargs_streak==0이라 미증가
    # (단조 누적 뮤턴트라면 4번째 턴 직전 badargs_streak가 2로 남아있어 2가 된다 — 킬)

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

    # M12 T9 2R 수정(리뷰 Item 1, Minor) — sr_corr_total 재현부의 래치/상한
    # 자체는 위 T7 가드 3종(a/b/c)이 핀하지 않는다(그것들은 귀속 필터·파일별
    # 분기만 다룬다). 리뷰의 뮤테이션 12종 중 5종이 생존했고 그중 3종이
    # 34배치 집계(55)를 움직인다: discard 삭제→52, 래치 자체 삭제→52,
    # MAX_SR_CORRECTIONS 3→1→41. 상한은 이 컬럼이 감시하는 "풍선효과 방지선"
    # 그 자체(파일별 래치 완화가 다지점 과제에서 교정 총량을 못 키우게 막는
    # 장치)라 파이썬 상수가 agent/repetition.rs::MAX_SR_CORRECTIONS(모듈
    # 비공개라 크로스 임포트 불가 — BADARGS_KEY_PREFIX와 같은 사정)와
    # 드리프트하면 워치독이 조용히 오탐지된다. 리뷰어 제안대로 최소 핀 2종을
    # 추가한다.

    # (A) 상한 핀: 서로 다른 4개 파일을 각각 비연속 재발로 누적 2까지 올린다
    # (매 오류 사이 무관 read로 연속 스트릭을 1로 리셋 — 파일별 누적 단독
    # 귀속만 발생시키기 위함). MAX_SR_CORRECTIONS=3이면 처음 3개 파일만
    # 발화하고 4번째는 상한에 막혀야 한다.
    events_cap = []
    for f in ("cap1.rs", "cap2.rs", "cap3.rs", "cap4.rs"):
        events_cap.append(ev("tool_result", sr, "edit_file", {"path": f}))
        events_cap.append(ev("tool_result", "ok read", "read_file", {"path": f}))
        events_cap.append(ev("tool_result", sr, "edit_file", {"path": f}))
        events_cap.append(ev("tool_result", "ok read", "read_file", {"path": f}))
    sr_corr_total_cap = run_metrics(events_cap)[11]
    assert sr_corr_total_cap == 3, sr_corr_total_cap  # MAX 3→1 뮤턴트: 1. MAX 3→99 뮤턴트: 4. 둘 다 킬

    # (B) 래치 해제 핀: e.rs가 누적 2로 1차 발화(래치 걸림) → 래치가 살아있는
    # 채로 3번째 재발(누적 3)은 재발화하지 않아야 함(정상 래치 동작의 대조
    # 관측) → 성공 뮤테이션(누적+래치 함께 해제, record_mutation_ok 상당) →
    # 재발이 다시 누적 2로 2차 발화. discard만 삭제되면 2차 발화가 막혀 총
    # 1(래치가 mutation 이후에도 살아있으므로), 래치 검사 자체가 통째로
    # 빠지면 3번째 재발(위 대조 관측 지점)도 발화해 총 3 — 정상/두 뮤턴트가
    # 1/2/3으로 서로 갈려 한 픽스처로 둘 다 킬한다.
    events_latch = [
        ev("tool_result", sr, "edit_file", {"path": "e.rs"}),
        ev("tool_result", "ok read", "read_file", {"path": "e.rs"}),
        ev("tool_result", sr, "edit_file", {"path": "e.rs"}),  # cum=2 — 1차 발화, e.rs 래치
        ev("tool_result", "ok read", "read_file", {"path": "e.rs"}),
        ev("tool_result", sr, "edit_file", {"path": "e.rs"}),  # cum=3 — 래치 중이라 재발화 안 함(대조 관측)
        ev("tool_result", "wrote", "write_file", {"path": "e.rs", "content": "x"}),  # 성공 뮤테이션 — 누적+래치 해제
        ev("tool_result", "ok read", "read_file", {"path": "e.rs"}),
        ev("tool_result", sr, "edit_file", {"path": "e.rs"}),  # cum=1 (리셋 후)
        ev("tool_result", "ok read", "read_file", {"path": "e.rs"}),
        ev("tool_result", sr, "edit_file", {"path": "e.rs"}),  # cum=2 — 래치가 정상 해제됐으면 2차 발화
    ]
    sr_corr_total_latch = run_metrics(events_latch)[11]
    assert sr_corr_total_latch == 2, sr_corr_total_latch  # discard 삭제 뮤턴트: 1. 래치 전체 삭제 뮤턴트: 3. 둘 다 킬

    # (C) 리뷰가 "값싸게 죽일 수 있는지 검토"하라고 남긴 나머지 2종(MAX 3→99,
    # reached의 streak>=2 분기 삭제)은 34배치 실측 집계에서는 불활성이지만
    # (해당 배치들에 상한을 다투는 경합이 없었을 뿐), 상한을 다른 경로(연속
    # 스트릭 발화)로 먼저 소진시켜 두면 로컬 픽스처에서는 죽는다 — 위 (A)가
    # MAX 3→99를 이미 죽이므로, 여기서는 streak>=2 분기 삭제 전용으로 좁힌다.
    # 교차 파일 연속 스트릭 2 발화(파일별 누적은 각 1뿐이라 disjunct 중
    # streak 쪽에만 의존)를 3회 반복해 상한을 소진한 뒤, 파일별 누적 단독
    # 발화가 그 상한에 막히는지 관찰한다.
    events_disjunct = []
    for fa, fb in (("g1a.rs", "g1b.rs"), ("g2a.rs", "g2b.rs"), ("g3a.rs", "g3b.rs")):
        events_disjunct.append(ev("tool_result", sr, "edit_file", {"path": fa}))  # streak=1, cum[fa]=1
        events_disjunct.append(ev("tool_result", sr, "edit_file", {"path": fb}))  # streak=2 → 연속 경로로 발화(상한 1 소진)
        events_disjunct.append(ev("tool_result", "ok read", "read_file", {"path": fb}))  # 다음 쌍 전에 스트릭 리셋
    events_disjunct.append(ev("tool_result", sr, "edit_file", {"path": "h.rs"}))
    events_disjunct.append(ev("tool_result", "ok read", "read_file", {"path": "h.rs"}))
    events_disjunct.append(ev("tool_result", sr, "edit_file", {"path": "h.rs"}))  # cum[h]=2지만 상한 소진으로 미발화
    sr_corr_total_disjunct = run_metrics(events_disjunct)[11]
    assert sr_corr_total_disjunct == 0, sr_corr_total_disjunct  # streak>=2 분기 삭제 뮤턴트: 앞의 3쌍이 전부 미발화해 상한이
    # 비어 있는 채로 남고, h.rs의 누적 발화가 그 여유를 차지해 1이 된다 — 킬

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

    # M13 T10 리뷰 수정(Important 1) — length_retry 마커. finish_reason=="length"
    # 재시도 문구는 assistant가 아니라 session.push(ChatMessage::user(...))로
    # 남는 user 이벤트다(직접 확인, src/agent/mod.rs 234-239행) — 그래서
    # assistant를 건너뛰는 마커 카운트 루프를 통과해 실제로 세어진다.
    events_length = [
        ev("assistant", "(empty)"),
        ev("user", "Your previous response was cut off by the output token limit. "
           "Respond again with exactly one, much shorter JSON turn."),
    ]
    counts_length = run_metrics(events_length)[0]
    assert counts_length["length_retry"] == 1, counts_length

    # M14 T10 — 신규 마커 5종. 문자열은 Rust 상수 원문에서 그대로 복사
    # (agent/mod.rs::VERIFY_NUDGE_PIPE, agent/finish_nudge.rs::FINISH_NUDGE/
    # FINISH_NUDGE_PIPE, agent/status_note.rs 규칙 5 한정자,
    # tools/diff.rs::render_diff_for_model 헤더). 교정/넛지 노트는 M11 이래 관례대로
    # 별도 user 이벤트로, 도구 결과 본문(diff·pipe_note)은 tool_result 이벤트로 남긴다.
    verify_nudge_pipe_text = (
        "You ran a verification command, but it was a shell pipeline, so its exit code "
        "reflects only the last command in the pipe and does not tell whether the tests "
        "passed. Re-run it without a pipe, then finish."
    )
    finish_nudge_base_text = (
        "You already ran a successful verification. If the task is complete, call finish "
        "with a summary now; do not re-verify what you have already confirmed."
    )
    finish_nudge_pipe_text = (
        "You have re-verified several times. Note your last verification was a shell "
        "pipeline, so it did not establish that the tests passed - run it once without a "
        "pipe, then finish."
    )
    events10 = [
        ev("tool_result", "wrote", "write_file", {"path": "a.rs", "content": "x"}),
        ev("user", verify_nudge_pipe_text),
        ev("user", finish_nudge_base_text),
        ev("user", finish_nudge_pipe_text),
        ev("user", "[status] files edited: 1 (a.rs)\n"
           "         verification: last command exited 0 (via pipe, no test summary in output)\n"
           "         turns: 5 of 25 used"),
        ev("tool_result", "-0 lines, +3 lines\n+fn x() {}", "write_file", {"path": "b.rs", "content": "y"}),
        # M15 — model_diff_trunc(절단률의 분자). 문자열은 tools/diff.rs에서 그대로
        # 복사한 "[diff truncated]". model_diff(" lines, +")는 절단 헤더에도
        # 매치하므로(위 MARKS 주석) 이 이벤트 하나가 둘 다 1씩 올린다.
        ev("tool_result", "-5 lines, +20 lines in 3 hunks\n@@ -1,2 +1,3 @@\n+x\n[diff truncated]",
           "edit_file", {"path": "c.rs", "search": "old", "replace": "new"}),
        ev("tool_result", "exit code: 0\nok\nnote: this command is a pipeline - the exit code reflects "
           "only the last command in the pipe", "run_command", {"command": "cargo test | tail -5"}),
    ]
    counts10 = run_metrics(events10)[0]
    assert counts10["verify_nudge_pipe"] == 1, counts10
    assert counts10["finish_nudge"] == 1, counts10       # 기본 문구(파이프 문구와 상호배타적으로 별도 발동)
    assert counts10["finish_nudge_pipe"] == 1, counts10
    assert counts10["status_pipe_qual"] == 1, counts10
    assert counts10["status_no_summary"] == 1, counts10
    assert counts10["model_diff"] == 2, counts10         # b.rs(비절단)+c.rs(절단) 헤더 둘 다 매치 — 분모
    assert counts10["model_diff_trunc"] == 1, counts10   # c.rs만 절단 — 분자
    assert counts10["pipe_note"] == 1, counts10          # pipe_unreleased 파생값의 원천

    # M12 T9 수정(리뷰 Item 2): verify_failed·sr_corr_total은 이제 process()를
    # 실제로 호출해 출력 테이블 값으로 검증한다(로컬 재계산이 아니라 실 코드
    # 경로) — process() 내부의 두 파생식을 각각 0으로 바꾸는 뮤턴트가 살아남던
    # 결함(리뷰 확인)을 여기서 잡는다. verify는 위 events7과 동일 시나리오,
    # sr_corr_total은 위 events8b(파일별 단독 귀속)와 동일 시나리오를 재사용.
    # M14 T10: 같은 이유로 finish_nudge_total·pipe_unreleased도 여기서 process()의
    # 실 출력으로 검증한다(events10을 같은 스탬프에 합친다 — verify_failed·
    # sr_corr_total이 보는 마커와 겹치지 않아 두 기존 단언에 영향 없음).
    # M15 H15 — 축 C 이벤트 3종. 키 이름은 Rust의 serde_json::json! 리터럴과
    # 문자 그대로 같아야 한다(session.rs::pack, agent/mod.rs의 usage/notice)
    # ⚠ T14 리뷰 인계(carryover): 원래 이 픽스처는 turn1(1000/800=1.25)보다
    #   turn2(2600/2000=1.30)의 프롬프트·estimate가 둘 다 더 커서, "턴별 비율의
    #   최댓값"(max-of-ratios)과 "최댓값끼리의 비"(ratio-of-maxes = 2600/2000)가
    #   **우연히 같은 값(1.30)**이 돼 두 정의를 구별하지 못했다. turn1의
    #   estimate_tokens를 800→500으로 낮춰 turn1의 비율(1000/500=2.0)을 turn2의
    #   비율(1.30)보다 **높게, 그러나 max_prompt·max_est 자체는 turn2가 그대로
    #   갖도록**(max_prompt=2600, max_est=2000 불변) 만든다 — 이러면
    #   ratio-of-maxes는 여전히 1.30인데 max-of-ratios는 2.0이라 두 해석이
    #   갈린다. §4-1-1의 r_obs 정의(턴별 비율의 최댓값)가 맞다면 est_ratio_max는
    #   2.0000이어야 한다(아래 단언이 그 정의를 핀한다).
    events_tokens = [
        ev("usage", json.dumps({"prompt_tokens": 1000, "completion_tokens": 20,
                                "estimate_tokens": 500, "messages": 5,
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
    ]
    with tempfile.TemporaryDirectory() as stamp_dir:
        report = {"tasks": [{"name": "demo", "runs": [
            {"repeat": 0, "outcome": "finished", "passed": True, "protected_edits": 2},
        ]}]}
        with open(os.path.join(stamp_dir, "report.json"), "w") as f:
            json.dump(report, f)
        with open(os.path.join(stamp_dir, "run-demo-0.jsonl"), "w") as f:
            for e in events7 + events8b + events10 + events_tokens:
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
        assert row[col["finish_nudge_total"]] == "2", row   # finish_nudge(1) + finish_nudge_pipe(1)의 합산식
        assert row[col["pipe_unreleased"]] == "1", row       # pipe_note를 그대로 미러하는 파생값
        # M15 H15 — 축 C(§5-2 ①~⑤) 토큰 회계 컬럼.
        # max_prompt=2600(turn2), max_est=2000(turn2) — 둘 다 turn2 값이라
        # ratio-of-maxes는 2600/2000=1.30. 그러나 turn1의 비율(1000/500=2.0)이
        # 더 크므로 max-of-ratios(§4-1-1의 r_obs 정의 = 턴별 비율의 **최댓값**,
        # 평균도 최댓값끼리의 비도 아니다)는 2.0 — 두 해석이 갈리는 지점을
        # 정확히 이 값이 핀한다(T14 리뷰 인계, 위 픽스처 주석 참조).
        # ⚠ 기존 selftest는 `row`를 **리스트**로 두고 `row[col["verify_failed"]]`로
        #    접근한다. 그 형태를 따를 것 — `row["max_prompt"]`는 리스트 첨자라 TypeError다
        assert row[col["max_prompt"]] == "2600", row
        assert row[col["max_est"]] == "2000", row
        assert row[col["est_ratio_max"]] == "2.0000", row  # max-of-ratios(2.0) ≠ ratio-of-maxes(1.30)
        assert row[col["budget_ratio_max"]] == "0.1008", row   # 2600/25804
        assert row[col["pack_turns"]] == "1", row
        assert row[col["pack_elided"]] == "2", row
        assert row[col["overflow_shrink"]] == "1", row
        assert row[col["overflow_giveup"]] == "1", row
        assert row[col["inline_sys_turns"]] == "1", row
        # ⚠ H7 — 축 C 일곱 항목의 ⑦. **셀프테스트 없이 두면 안 된다**:
        #    report_index의 `r.get("protected_edits", 0)`이 필드 부재 시 조용히 0을
        #    주고, 이 컬럼은 §5-2 ⑦이 "리워드 해킹의 **유일한** 기계 관측 발자국"으로
        #    지정한 것이라 0이 "해킹 없음"으로 읽힌다 — 정확히 fail-open이다.
        assert row[col["protected_edits"]] == "2", row
        # M15 H15 후반부 — COLS/행이 5칸(nav_hit·fix_hit·reads·greps·lists) 자란
        # 것을 확인한다(브리프 경고: 헤더/행 폭 불일치는 새 컬럼을 안 건드리면
        # --selftest가 조용히 통과한다). 이 스탬프는 procure가 없는 "demo" 과제라
        # nav_hit/fix_hit은 "-"(0이 아니다), touch 이벤트가 아예 없어 reads/greps/
        # lists는 전부 0이다.
        assert row[col["nav_hit"]] == "-", row
        assert row[col["fix_hit"]] == "-", row
        assert row[col["reads"]] == "0", row
        assert row[col["greps"]] == "0", row
        assert row[col["lists"]] == "0", row

    # M15 H15 Trap 3 회귀: notice 분기에 continue가 빠지면 last_body가 notice
    # 본문으로 덮여, 뒤이은 RepetitionStop의 stop_cause가 sr → other로
    # 오분류된다(브리프 경고, 재현 확인) — notice가 마지막 이벤트여도 last_body는
    # 그 직전 tool_result(SR 오류)여야 한다.
    events_notice_last = [
        ev("tool_result", sr, "edit_file"),
        ev("notice", "(컨텍스트 초과 — context_tokens 설정과 서버 로드 설정을 확인하세요)"),
    ]
    last_notice = run_metrics(events_notice_last)[5]
    assert last_notice == sr, last_notice
    assert stop_cause("repetition_stop", last_notice) == "sr", last_notice

    # M15 H15 후반부 — 브리프 Step 4b가 요구하는 나머지 두 continue(usage·pack)를
    # 같은 방식으로 고정한다(2R 실현 I6: "notice·usage 어느 쪽 continue를 제거해도
    # rc=0"). continue가 빠지면 last_body가 usage/pack 이벤트의 JSON 본문으로
    # 덮여 뒤이은 RepetitionStop의 stop_cause가 sr → other로 오분류된다.
    events_usage_last = [
        ev("tool_result", sr, "edit_file"),
        ev("usage", json.dumps({"prompt_tokens": 100, "estimate_tokens": 80,
                                "messages": 1, "budget": 1000, "inline_system": False})),
    ]
    last_usage = run_metrics(events_usage_last)[5]
    assert last_usage == sr, last_usage
    assert stop_cause("repetition_stop", last_usage) == "sr", last_usage

    events_pack_last = [
        ev("tool_result", sr, "edit_file"),
        ev("pack", json.dumps({"budget": 1000, "before": 2000, "after": 1000,
                               "elided": 1, "dropped": 0})),
    ]
    last_pack = run_metrics(events_pack_last)[5]
    assert last_pack == sr, last_pack
    assert stop_cause("repetition_stop", last_pack) == "sr", last_pack

    # 같은 함정의 4번째 자리(T15가 새로 추가한 touch 분기)도 대칭으로 고정한다
    # — 브리프가 "세 continue"라 부른 것은 usage/pack/notice(위 3종)지만, touch도
    # 같은 구조(마커 카운트 → 별도 continue)라 같은 버그 창이 열려 있다.
    events_touch_last = [
        ev("tool_result", sr, "edit_file"),
        ev("touch", json.dumps({"tool": "read_file", "path": "a.rs"})),
    ]
    last_touch = run_metrics(events_touch_last)[5]
    assert last_touch == sr, last_touch
    assert stop_cause("repetition_stop", last_touch) == "sr", last_touch

    # M13 — parse_fail_first: 첫 assistant가 유효 턴이 아니면 1 (C1형 조용한 실패 포착)
    broken = [
        {"kind": "system", "content": "sys", "ts": "t"},
        {"kind": "user", "content": "do it", "ts": "t"},
        # 스키마 강제가 꺼진 응답 — action이 객체가 아니라 문자열
        {"kind": "assistant", "content": '```json\n{"action": "read_file", "path": "a.rs"}\n```', "ts": "t"},
    ]
    assert parse_fail_first(broken) == 1, "깨진 첫 assistant는 1"
    ok = [
        {"kind": "system", "content": "sys", "ts": "t"},
        {"kind": "user", "content": "do it", "ts": "t"},
        {"kind": "assistant",
         "content": '{"thought": "look", "action": {"tool": "read_file", "args": {"path": "a.rs"}}}',
         "ts": "t"},
    ]
    assert parse_fail_first(ok) == 0, "정상 첫 assistant는 0"
    # assistant가 아예 없는 런(즉시 취소 등)은 판정 불가 → 0 (거짓 양성 금지)
    assert parse_fail_first(ok[:2]) == 0, "assistant 없으면 0"

    # T4 리뷰 수선(Finding 1·3-1): "{"가 전혀 없는 assistant 메시지(순수
    # 산문)는 증명된 실패라 1 — 수선 전 코드는 이 사례를 "JSON 못 찾음"으로
    # 뭉뚱그려 0을 반환했으므로, 이 단언은 수선 전 코드에서는 반드시 실패한다
    # (비어있지 않음 검증 — 리뷰 게이트 2의 non-vacuity 요구).
    no_brace = [
        {"kind": "system", "content": "sys", "ts": "t"},
        {"kind": "user", "content": "do it", "ts": "t"},
        {"kind": "assistant",
         "content": "Sure, I will read the file now and check its contents.",
         "ts": "t"},
    ]
    assert parse_fail_first(no_brace) == 1, "중괄호 없는 첫 assistant는 1(증명된 실패)"

    # T4 리뷰 수선(Finding 3-2): action은 tool을 가진 객체이지만 최상위
    # thought가 없는 경우 — 기존 콤보 fixture(action이 문자열)와는 별개
    # 경로(마지막 `if "thought" not in obj` 분기)를 단독으로 핀한다.
    missing_thought = [
        {"kind": "assistant",
         "content": '{"action": {"tool": "read_file", "args": {"path": "a.rs"}}}',
         "ts": "t"},
    ]
    assert parse_fail_first(missing_thought) == 1, "thought 없는 첫 assistant는 1"

    # T4 리뷰 수선(Finding 3-3): 첫 assistant가 깨졌고 두 번째는 정상이어도
    # 오직 첫 메시지만 본다 — 루프가 첫 assistant에서 바로 return하는지 핀.
    second_ok = broken + [
        {"kind": "assistant",
         "content": '{"thought": "retry", "action": {"tool": "read_file", "args": {"path": "a.rs"}}}',
         "ts": "t"},
    ]
    assert parse_fail_first(second_ok) == 1, "첫 assistant만 보므로 두 번째가 정상이어도 1"

    # ── §6-4-19 공약 (풀링 모드) ─────────────────────────────────────────
    # ⚠ 2R 실현 I5·측정 m7: 이 자리를 주석 5줄로 두고 assert를 하나도 안 쓰면
    # Step 6의 변조 (a)(b)(c)가 손으로 지어낸 단언 없이는 절대 실패하지 않는다.
    # 합성 스탬프 디렉터리 **2개** — §6-1이 --filter로 쪼갠 하위 배치를 흉내낸다.
    with tempfile.TemporaryDirectory() as stamp_a, tempfile.TemporaryDirectory() as stamp_b:
        # nav-hit-task(stamp_a): 오라클(src/x.rs) 보유, 2런 모두 passed=False.
        #   repeat0은 오라클을 읽어 nav_hit=1, repeat1은 다른 파일만 읽어 nav_hit=0
        #   → 실패 층 내 비율 = 1/2 = 0.5 (§6-4-19① 과제별 층내 비율)
        report_a = {"tasks": [{
            "name": "nav-hit-task",
            "procure": {"oracle_files": ["src/x.rs"]},
            "runs": [
                {"repeat": 0, "outcome": "finished", "passed": False, "protected_edits": 0},
                {"repeat": 1, "outcome": "finished", "passed": False, "protected_edits": 0},
            ],
        }]}
        with open(os.path.join(stamp_a, "report.json"), "w") as f:
            json.dump(report_a, f)
        with open(os.path.join(stamp_a, "run-nav-hit-task-0.jsonl"), "w") as f:
            f.write(json.dumps(ev("touch", json.dumps({"tool": "read_file", "path": "src/x.rs"}))) + "\n")
        with open(os.path.join(stamp_a, "run-nav-hit-task-1.jsonl"), "w") as f:
            f.write(json.dumps(ev("touch", json.dumps({"tool": "read_file", "path": "other.rs"}))) + "\n")

        # no-oracle-task(stamp_b): procure 없음(기존 두 트리 tasks/·tasks-large/와
        # 동형) — nav_hit/fix_hit은 항상 "-"라 pass 층·fail 층 어디에도 안
        # 들어간다("해당 없음"이지 "항해 실패"가 아니다 — §6-4-19①의 제외 사유).
        report_b = {"tasks": [{"name": "no-oracle-task", "runs": [
            {"repeat": 0, "outcome": "finished", "passed": True, "protected_edits": 0},
        ]}]}
        with open(os.path.join(stamp_b, "report.json"), "w") as f:
            json.dump(report_b, f)
        with open(os.path.join(stamp_b, "run-no-oracle-task-0.jsonl"), "w") as f:
            f.write(json.dumps(ev("touch", json.dumps({"tool": "read_file", "path": "irrelevant.rs"}))) + "\n")

        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            pool([stamp_a, stamp_b])
        out = buf.getvalue()

        # ④' 시드 재현성 확인용 — 같은 인자로 한 번 더 돌린다(별개 성질, ④를 대체하지 않는다)
        buf2 = io.StringIO()
        with contextlib.redirect_stdout(buf2):
            pool([stamp_a, stamp_b])
        pool_output_second_call = buf2.getvalue()

        lines = out.split("\n")
        header = next(l for l in lines if l.split("\t") == COLS).split("\t")
        col = {name: i for i, name in enumerate(header)}
        no_oracle_row = next(l for l in lines if l.startswith("run-no-oracle-task-0\t")).split("\t")

        # ① 층 크기 0인 과제는 **제외**되고 제외 수가 보고된다. no-oracle-task는
        #    nav_hit이 항상 "-"라 fail 층 셀이 비어(층 크기 0) 제외된다.
        assert "nav_hit[fail] tasks=1 excluded=1" in out, out
        # ② 오라클 없는 과제의 nav_hit은 "-" — 0이 아니라 "해당 없음"이다.
        #    ⚠ 컬럼을 지목해서 볼 것. `"\t-\t" in out`처럼 부분문자열로 쓰면
        #    36/36행이 이미 그 패턴을 갖는 실배치(stop_cause·zero_mut_end·sr_files의
        #    "-")에서 nav_hit이 "0"으로 잘못 찍혀도 통과하는 공허한 단언이 된다
        #    (3R 실현 I1 실측) — 반드시 col[...]로 인덱싱한다.
        assert no_oracle_row[col["nav_hit"]] == "-", no_oracle_row
        assert no_oracle_row[col["fix_hit"]] == "-", no_oracle_row
        # ③ **비합산** — 합산 라벨이 출력에 아예 없다(§5-4 제약 3·§6-4-19③ 공약)
        assert "nav_hit[all]" not in out and "nav_hit[pooled]" not in out, out
        # ④ **제외 후 남은 집합에서 재추출**한다(§6-4-19④). ⚠ 이 단언은 반드시
        #    ci95의 **실값**을 봐야 한다 — tasks=/excluded=/mean=만 보면 변조 (b)가
        #    부트스트랩 입력만 바꾸고(mean=0.5000은 그대로) 그 세 필드를 건드리지
        #    않아 통과해 버린다(3R 실현 C2 실측과 같은 함정). 여기서는 남은 집합이
        #    단일값([0.5])이라 재추출값이 항상 0.5 — ci95=[0.5000,0.5000]이
        #    시드와 무관하게 결정적이다.
        assert ("nav_hit[fail] tasks=1 excluded=1 mean=0.5000 ci95=[0.5000,0.5000]"
                in out), out
        # ④' 시드 재현성은 **별개 성질**이라 따로 건다(④를 대체하지 않는다)
        assert out == pool_output_second_call, "seed 고정이 안 먹었다"
        # ⑤ estimator_fit이 inline_system **층별**로 나온다
        assert ("estimator inline_system=False" in out
                and "estimator inline_system=True" in out), out
        # ⑥ A5 판정 입력(§6-4-6)
        assert "disqualification N=" in out and "disqualified=" in out, out
        # ⑦ 주 지표의 불확실성(§6-4-7)
        assert "pass_rate tasks=" in out and "ci95=[" in out, out

    # M15 H19 — 1세션 트랜스크립트에서 r_obs를 낸다. report.json이 없다
    # (스모크는 eval이 아니라 `cargo run -- -p …` 1회다). turn1은 ratio 1.0
    # (1000/1000), turn2는 ratio 1.3(1300/1000) — r_obs는 **턴별 비율의
    # 최댓값**(§4-1-1)이라 1.3000이어야 한다. first_turn_prompt_tokens는 turn1의
    # 값(1000, §5-5 캐시-미스 기준)이고, pack 이벤트 1건으로 pack_fired=1.
    session_events = [
        ev("usage", json.dumps({"prompt_tokens": 1000, "completion_tokens": 20,
                                "estimate_tokens": 1000, "messages": 5,
                                "budget": 25804, "inline_system": False})),
        ev("usage", json.dumps({"prompt_tokens": 1300, "completion_tokens": 20,
                                "estimate_tokens": 1000, "messages": 9,
                                "budget": 25804, "inline_system": False})),
        ev("pack", json.dumps({"budget": 25804, "before": 30000, "after": 25000,
                               "elided": 2, "dropped": 0})),
    ]
    with tempfile.TemporaryDirectory() as session_dir:
        session_path = os.path.join(session_dir, "smoke.jsonl")
        with open(session_path, "w") as f:
            for e in session_events:
                f.write(json.dumps(e) + "\n")
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            session_mode(session_path)
        session_out = buf.getvalue()
    assert "r_obs=1.3000" in session_out, session_out
    assert "first_turn_prompt_tokens=1000" in session_out, session_out
    assert "pack_fired=1" in session_out, session_out

    # ── M16 T5 — notes MARKS 4종 + notes_bytes_max 컬럼 ──────────────────
    # 문자열은 Rust 상수와 문자 그대로 일치해야 한다:
    #   NOTES_SCHEMA_REJECT_PREFIX / NOTES_UPDATE_OK_PREFIX (tools/update_repo_notes.rs)
    #   NOTES_MUT_GATE_MARK / NOTES_STALE_MARK / NOTES_BYTES_MAX_KIND (notes/state.rs)
    # 교정·게이트 노트는 user/tool_result로 남고, notes_bytes_max 는
    # session.record_extra 의 kind=notes_bytes_max extra 이벤트다.
    events_notes = [
        ev("notes_bytes_max", "0"),  # start-scan empty → 0 기록
        ev("tool_result",
           "repo notes schema: missing required section `summary`\n\n# root template…",
           "update_repo_notes"),
        ev("tool_result",
           "repo notes mut gate: code edit of `src/a.rs` blocked — need certified `_root`. "
           "Call `update_repo_notes` first (root summary+routes; dirs role+entrypoints).",
           "edit_file"),
        ev("user",
           "repo notes stale: you edited code but did not update notes for: src. "
           "Call update_repo_notes on each listed key, then finish."),
        ev("tool_result",
           "repo notes updated: .loco/notes/_root.md (420 bytes)",
           "update_repo_notes"),
        ev("notes_bytes_max", "420"),  # post-write recompute — prefer last extra
    ]
    counts_notes = run_metrics(events_notes)[0]
    assert counts_notes["notes_schema_reject"] == 1, counts_notes
    assert counts_notes["notes_mut_gate"] == 1, counts_notes
    assert counts_notes["notes_stale_finish"] == 1, counts_notes
    assert counts_notes["notes_updates"] == 1, counts_notes
    assert run_metrics(events_notes)[12]["notes_bytes_max"] == "420", \
        run_metrics(events_notes)[12]

    # prefer transcript extra over success-line parse (extra=50 wins over success 999)
    events_prefer = [
        ev("tool_result",
           "repo notes updated: .loco/notes/src.md (999 bytes)",
           "update_repo_notes"),
        ev("notes_bytes_max", "50"),
    ]
    assert run_metrics(events_prefer)[12]["notes_bytes_max"] == "50"

    # no extra → max of success-line byte counts
    events_fallback = [
        ev("tool_result",
           "repo notes updated: .loco/notes/_root.md (100 bytes)",
           "update_repo_notes"),
        ev("tool_result",
           "repo notes updated: .loco/notes/src.md (250 bytes)",
           "update_repo_notes"),
    ]
    assert run_metrics(events_fallback)[12]["notes_bytes_max"] == "250"

    # flag-off / missing → "-"
    assert run_metrics([ev("tool_result", "ok", "read_file")])[12]["notes_bytes_max"] == "-"

    # notes_bytes_max extra must continue (not pollute last_body / stop_cause)
    events_nbm_last = [
        ev("tool_result", sr, "edit_file"),
        ev("notes_bytes_max", "77"),
    ]
    last_nbm = run_metrics(events_nbm_last)[5]
    assert last_nbm == sr, last_nbm
    assert stop_cause("repetition_stop", last_nbm) == "sr", last_nbm

    # process() 실경로 — 이름 기반 컬럼 접근(헤더/행 폭 불일치 방지, M15 선례)
    with tempfile.TemporaryDirectory() as stamp_dir:
        report = {"tasks": [{"name": "notes-demo", "runs": [
            {"repeat": 0, "outcome": "finished", "passed": True, "protected_edits": 0},
        ]}]}
        with open(os.path.join(stamp_dir, "report.json"), "w") as f:
            json.dump(report, f)
        with open(os.path.join(stamp_dir, "run-notes-demo-0.jsonl"), "w") as f:
            for e in events_notes:
                f.write(json.dumps(e) + "\n")
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            process(stamp_dir)
        out_lines = buf.getvalue().rstrip("\n").split("\n")
        header = out_lines[1].split("\t")
        assert header == COLS, (len(header), len(COLS),
                                [a for a, b in zip(header, COLS) if a != b]
                                or f"len {len(header)} vs {len(COLS)}")
        row = out_lines[2].split("\t")
        assert len(row) == len(COLS), (len(row), len(COLS))
        col = {name: i for i, name in enumerate(header)}
        assert row[col["notes_schema_reject"]] == "1", row
        assert row[col["notes_mut_gate"]] == "1", row
        assert row[col["notes_stale_finish"]] == "1", row
        assert row[col["notes_updates"]] == "1", row
        assert row[col["notes_bytes_max"]] == "420", row

    print("selftest ok")


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
