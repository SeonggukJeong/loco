#!/usr/bin/env python3
"""M13 파일럿 원장 집계 (스펙 §4-3·§4-4). stdlib 전용.

  python3 scripts/pilot_tally.py <ledger.jsonl>

레포는 CLI 인자가 아니라 각 행 자신의 repo 필드에서 읽는다 — 파일럿이 4개
레포(zoxide/fd/ripgrep/just)에 걸치도록 확장되면서, 예전처럼 레포 경로 하나를
모든 행에 일괄 적용하면 다른 레포 소속 행은 git grep이 전부 미스 처리돼
생존율이 아무 표시 없이 체계적으로 과소 계상됐다(T10 리뷰 수선 2 — repo는
이미 REQUIRED_FIELDS라 누락 행은 여전히 즉시 중단되고, 생존율은 레포별로
나눠 낸다 + 전체 가중치도 별도 표시).

산출:
  1) 범주별 건수 — 주 산출물(스펙 §4-4). 다중 라벨이므로 합 != 세션 수
  2) 줄 생존율 — 기술 통계 보조 지표, 레포별 소계 + 전체 가중치. 대리 지표이며
     왜곡 5종이 알려져 있다(스펙 §4-3). 헤드라인으로 인용하지 말 것 — 아래 경고 참조

마커 판정 상수(오류 문자열 리터럴 등)는 이 파일에서 다시 선언하지 않고
scripts/exp_metrics.py에서 그대로 import한다. 그쪽이 검증된(러스트 소스와
교차 핀된) 유일한 정의이고, 여기서 별도 리터럴을 들면 드리프트하는 두 번째
분류기가 생긴다(M13 T10 결정, 플랜 e3c9264). import 실패는 조용히 넘어가지
않고 즉시 중단한다 — 침묵 폴백은 "이 마커는 이제부터 이 파일이 손으로
재정의한 걸 쓴다"는 뜻이 되어 버려서, 정확히 우리가 막으려는 드리프트다.
"""
import json
import os
import re
import subprocess
import sys
from collections import Counter

_SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
if _SCRIPT_DIR not in sys.path:
    sys.path.insert(0, _SCRIPT_DIR)
try:
    from exp_metrics import MARKS, BADARGS_KEY_PREFIX
except ImportError as exc:  # 조용한 폴백 금지 (사용자 결정) — 크게 죽는다
    sys.exit(
        "오류: scripts/exp_metrics.py에서 마커 상수(MARKS, BADARGS_KEY_PREFIX)를 "
        f"import하지 못했습니다. 폴백 없이 중단합니다: {exc}"
    )

# 스펙 §4-4 — 세션 1(T11 첫 실사용 파일럿) 이전에 확정된 범주.
# "loco 비정상 종료(exit≠0)"는 T10에서 loco_exit 필드가 원장 스키마에 추가된
# 것(T9 리뷰 수정)에 대응해 이 시점에 신설한다 — 역시 세션 1 이전이므로
# "신규 추가 시 추가 시점을 원장에 기록할 것"이라는 조항의 대상은 아니다
# (사후 추가가 아니라 최초 사전선언의 일부다). 아래 두 범주는 이름은
# 사전선언돼 있지만 **이 스크립트는 기계 판정을 시도하지 않는다** — 이유는
# classify()의 docstring과 아래 UNINSTRUMENTED에 적어 둔다. 항상 0으로
# 나오는 것은 버그가 아니라 알려진 계측 공백이며, 출력 표에서도 △로 표시해
# "측정해서 0건"과 구분한다(M13 T10 리뷰 Important 2):
#   - "컨텍스트 오버플로": agent/mod.rs의 재시도/포기 알림은 AgentEvent::Notice
#     로만 나가고 session.push()를 타지 않으므로 .loco/sessions/*.jsonl에
#     아무 흔적도 남지 않는다(직접 확인, src/agent/mod.rs 208-228행).
#   - "엉뚱한 파일 편집": "정답 파일이 무엇이었는가"를 원장 스키마 자체가
#     기록하지 않으므로 기계 판정 대상 밖이다.
# "length 루프"는 반대로 M13 T10 리뷰(Important 1)에서 계측을 신설했다:
# finish_reason=="length" 재시도 시 세션에 push되는 고정 문구
# (src/agent/mod.rs 234-239행)를 scripts/exp_metrics.py에 MARKS["length_retry"]로
# 신설해 참조한다(리터럴 복제 금지 결정 — 위 모듈 docstring 참조) — 이제
# S/R 루프·BadArgs와 동급의 기계 판정 범주다(classify() 참조).
#
# 아래 두 범주는 파일럿 진행 중 현장에서 추가됐다(스펙 §4-4가 허용하는
# "신규 범주는 발견 시 추가하되 추가 시점을 원장에 기록" 조항 — 사후 그리기가
# 아니라 이 시점에 실측으로 드러난 구분이라는 근거를 여기 남긴다):
#
#   - "턴 소진(finish 미호출)" — 세션 6/20(2026-07-19)에 추가. 그때까지
#     "뮤테이션 없는 탐색 루프" 하나로 뭉쳐 있던 두 세션이 실은 정반대
#     실패다: session 20260719T132833Z(zoxide, "Z1")는 툴 호출 **2회**만에
#     질문과 무관한 일반 요약으로 finish를 호출했다(너무 일찍 끝남). session
#     20260719T141458Z(fd, "F1")는 max_turns(25)를 전부 써서 src/walk.rs를
#     10줄씩 반복 재독(offset 1을 이벤트 14·50에서, offset 31을 16·52에서
#     재조회)하고 finish를 **한 번도 안 불렀다**(트랜스크립트 마지막 이벤트가
#     tool_result). M14가 필요로 할 개입이 정반대(전자는 "계속 진행", 후자는
#     "멈추고 답하라")라 하나로 합치면 그 구분이 사라진다.
#   - "반복 루프(교정 발동)" — 세션 7/20(2026-07-19)에 추가. session
#     20260719T142204Z(fd, "F2")는 사전 선언 범주 어디에도 안 걸려 "기타"로
#     떨어졌었다: src/cli.rs를 offset 960/968/970으로 맴돌다 loco의 반복 교정
#     (REPEAT_CORRECTION, exp_metrics.py의 MARKS["repeat_corr"] = "repeating
#     the same tool call")이 한 번 발동했는데도 offset 968을 세 번 더
#     반복했고, 결국 사람이 같은 프롬프트를 재입력하자 모델이 ~5턴 만에
#     바로 끝냈다. 신호 자체는 exp_metrics.py에 이미 있었는데
#     (MARKS["repeat_corr"]) pilot_tally.py가 import하지 않아서 못 잡던
#     것뿐이다 — length_retry를 신설했을 때와 같은 종류의 계측 누락.
#
# 둘 다 기계 판정이고(증거 출처 "machine"), UNINSTRUMENTED에는 없다(아래
# 참조) — 실제로 셀 수 있어서 추가한 것이지 계측 공백이 아니다.
CATEGORIES = [
    "실패 없음", "S/R 루프", "반복 루프(교정 발동)", "뮤테이션 0회 거짓 finish",
    "뮤테이션 없는 탐색 루프", "턴 소진(finish 미호출)",
    "컨텍스트 오버플로", "엉뚱한 파일 편집", "length 루프", "인자 누락(BadArgs)",
    "loco 비정상 종료(exit≠0)", "산출물 빌드/테스트 실패",
]

# "산출물 빌드/테스트 실패"는 세션 14/20(R4) 관측에서 신설했다. 추가 시점을
# 남기는 이유는 ad2a5e9(범주 2종 현장 추가)와 같다 — 사후에 "언제부터 세기
# 시작했나"를 복원할 수 있어야 한다.
#
# 신설 사유는 범주가 없어서가 아니라 **신호를 안 읽고 있어서**였다. 2e22f41이
# 원장에 build_status/test_status를 넣었는데 이 스크립트가 그 필드를 한 번도
# 참조하지 않았다. R4는 컴파일되지 않는 트리를 내놓았는데 범주표에는
# "인자 누락(BadArgs) 1건"으로만 잡혀 거의 깨끗한 세션처럼 보였다.
# 이 누락은 세 번째 재발이다(F3의 length_retry, 세션 7의 repeat_corr) — 전부
# "신호는 데이터에 있는데 집계기가 참조를 안 한다"는 같은 형태다.
#
# 적용 범위 주의: build_status는 2e22f41 이후 세션(원장 10행 F5부터)에만 있다.
# 1~9행은 필드 자체가 없어 None이며, 이 범주는 그 행들에 대해 침묵한다
# (없음을 "통과"로 읽지 않는다). 몇 행이 미계측인지는 표 아래 각주로 찍는다.

# 위 주석의 계측 공백 목록 — 값은 출력 표 각주에 그대로 쓰는 사유 한 줄.
# classify()는 이 두 범주에 절대 라벨을 붙이지 않는다: 항상 0이 "발생 안 함"이
# 아니라 "잴 방법이 없음"임을 표에서 시각적으로 구분한다(M13 T10 리뷰
# Important 2 — 그렇지 않으면 측정한 0과 못 잰 0이 인쇄에서 똑같아 보인다).
UNINSTRUMENTED = {
    "컨텍스트 오버플로": "AgentEvent::Notice 전용 알림 — session.push()를 안 타 트랜스크립트에 안 남음",
    "엉뚱한 파일 편집": "정답 파일 오라클이 원장 스키마에 없음",
}

# T9 리뷰가 loco_exit을 추가한 이유(브리핑 참조): "$LOCO_BIN || true"로는
# "loco가 크래시함"과 "loco가 애초에 안 돎"을 구분할 수 없어, 스키마상
# 유효하지만 정상적인 무-diff 세션과 구별 불가능한 행이 생겼다. 이 스크립트가
# 지키는 계약: loco_exit != 0인 행은 무슨 일이 있었든 "실패 없음"으로
# 조용히 흡수되지 않는다(아래 classify() 참조) — 원인(컨텍스트 오버플로인지
# 서버 다운인지 진짜 크래시인지)은 원장이 stderr 텍스트를 담지 않아 이
# 스크립트만으로는 알 수 없다. 사람이 reason 필드를 읽고 필요하면 사후에
# 더 구체적인 범주로 재분류해야 한다 — 그래서 "미분류 세부" 절을 출력한다.
CODE_CHANGE_TASK_TYPES = {"bugfix", "feature", "refactor", "test"}

# pilot.sh가 세션 전 수집 프롬프트에서 사용자에게 제시하는 안내 어휘
# (scripts/pilot.sh 35행: "bugfix/feature/refactor/explore/test/other") — 그러나
# 실제 입력은 read -r로 받는 무제약 자유 텍스트라 이 목록 밖의 값(오타·동의어)이
# 원장에 그대로 들어갈 수 있다. CODE_CHANGE_TASK_TYPES(위)는 그중 "코드 변경이
# 기대되는" 부분집합이고, 이 집합은 classify()가 아는 전체 어휘다 — 여기 없는
# 값은 조용히 "실패 없음"/"기타"로 흡수되지 않도록 별도 진단 절에서 표면화한다
# (M13 T10 리뷰 Important 3): 정확히 이 방식의 조용한 흡수가 §4-4 범주 분리가
# 막으려는 실패이므로, 정규화하거나 추측해서 재분류하지 않고 사람이 읽게 한다.
KNOWN_TASK_TYPES = CODE_CHANGE_TASK_TYPES | {"explore", "other"}

# T9의 원장 스키마(scripts/pilot.sh 참조) — 이 중 하나라도 없는 줄은 손상된
# 원장으로 간주해 즉시 중단한다(자기검토 요구사항: 손상된 줄이 조용히
# 스킵돼 분모가 틀어지면 안 된다).
REQUIRED_FIELDS = [
    "session_id", "repo", "start_rev", "end_rev", "task_type", "difficulty",
    "task", "transcript", "diff", "duration_secs", "loco_exit", "verdict", "reason",
]


def load_ledger(path):
    """원장 JSONL을 읽는다. 손상되거나 스키마가 불완전한 줄은 큰 소리로 중단한다.

    (skip해서 넘어가면 그 줄이 조용히 분모에서 빠져 세션 수·범주 건수가
    둘 다 틀어진다 — 원장이 이 마일스톤의 유일 결과물이므로 여기서만큼은
    관대한 파싱이 오히려 해롭다.)
    """
    rows = []
    try:
        f = open(path, encoding="utf-8")
    except OSError as exc:
        sys.exit(f"오류: 원장을 열 수 없습니다: {exc}")
    with f:
        for lineno, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as exc:
                sys.exit(f"오류: {path}:{lineno}줄 — JSON 파싱 실패(원장 손상): {exc}")
            if not isinstance(row, dict):
                sys.exit(f"오류: {path}:{lineno}줄 — JSON 객체가 아닙니다")
            missing = [k for k in REQUIRED_FIELDS if k not in row]
            if missing:
                sys.exit(
                    f"오류: {path}:{lineno}줄 — 필수 필드 누락: {', '.join(missing)} "
                    "(scripts/pilot.sh 스키마와 다른 원장이거나 손상된 줄)"
                )
            if not isinstance(row["loco_exit"], int) or isinstance(row["loco_exit"], bool):
                sys.exit(
                    f"오류: {path}:{lineno}줄 — loco_exit이 정수가 아닙니다: "
                    f"{row['loco_exit']!r} (비정상 종료 판정에 쓰이는 필드라 타입이 어긋나면 안 됩니다)"
                )
            rows.append(row)
    if not rows:
        sys.exit(f"오류: {path}에 유효한 세션 행이 없습니다")
    return rows


def added_lines_by_file(diff):
    """diff에서 유의미한 추가 줄만, 그 줄이 실제로 속한 파일 경로와 짝지어 뽑는다.

    반환: [(path, body), ...] — path는 `+++ b/<path>` 헤더에서 읽은 대상 경로.
    공백/괄호/짧은 줄은 이전과 동일하게 생존 판정 노이즈로 걸러진다.

    (T10 현장 수선 6 — 파일 귀속이 없던 예전 버전은 diff 전체에서 "+"로 시작하는
    줄만 모아 레포 전역을 git grep했다. 실제 파일럿 원장으로 발견한 사례
    (가설이 아니라 실측): session 20260719T134645Z(zoxide)가 src/util.rs에 추가한
    `#[cfg(test)] mod tests { ... }` 블록 10줄 중 세션이 실제로 쓴 내용은 HEAD에
    하나도 없었는데, `#[cfg(test)]`/`mod tests {`/`    use super::*;`/
    `    #[test]` 네 줄이 같은 레포의 src/db/mod.rs·src/db/stream.rs에 있던 기존
    테스트 상용구와 글자 그대로 일치해 "30.0%(10줄) 생존"으로 찍혔다 — 세션이
    실제로 만든 파일과 무관한 매치라 이 파일 귀속으로 바로잡는다.)

    `+++ /dev/null`(파일 삭제)은 이 hunk에 귀속되는 파일이 없다는 뜻이라
    이후 "+" 줄을 아예 수집하지 않는다(삭제 diff는 애초에 "+" 줄을 내지 않지만,
    방어적으로 처리) — path=None으로 두고 최종 결과에서 자동으로 빠진다.
    """
    out = []  # [(path, body), ...]
    current = None  # 지금 보고 있는 hunk가 귀속된 파일 경로 (헤더 못 만나면 None)
    for line in diff.splitlines():
        if line.startswith("+++ "):
            target = line[4:].strip()
            if target == "/dev/null":
                current = None
            elif target.startswith("b/"):
                current = target[2:]
            else:
                # --no-prefix 등 낯선 형식 — pilot.sh는 git diff 기본 프리픽스만
                # 쓰므로(스크립트 확인) 실사용에서는 도달하지 않지만 방어적으로
                # 헤더 그대로를 경로로 취급한다(크래시보다 낫다).
                current = target
            continue
        if not line.startswith("+"):
            continue
        if current is None:
            continue
        body = line[1:].strip()
        if len(body) > 10 and body not in ("{", "}", "*/"):
            out.append((current, body))
    return out


def _repo_problem(repo):
    """repo 경로가 없거나 git 레포가 아니면 사유 문자열, 정상이면 None.

    레포가 이동·삭제된 경우를 "그 안 코드가 삭제됨"(생존율 0%)과 절대
    혼동하면 안 된다(T10 리뷰 수선 2) — survival()이 git grep을 실행하기
    전에 이 확인을 먼저 통과시켜, 두 경우를 반환값에서부터 분리한다.
    """
    if not repo or not os.path.isdir(repo):
        return f"경로가 디렉터리로 존재하지 않음(이동/삭제됐을 수 있음): {repo!r}"
    r = subprocess.run(
        ["git", "-C", repo, "rev-parse", "--git-dir"], capture_output=True
    )
    if r.returncode != 0:
        err = r.stderr.decode(errors="replace").strip()
        return f"git 레포가 아님: {repo!r}" + (f" ({err})" if err else "")
    return None


def survival(repo, diff, session_id="?"):
    """추가 줄 중 현재 HEAD 트리에 남아 있는 비율.

    반환값은 (rate, n, reason) 세 가지 경우:
      (None, 0, None)   — 판정 대상 없음 (diff에 유의미한 추가 줄이 없음)
      (None, 0, reason) — 판정 불가 (레포 경로 문제) — 0%와 혼동 금지, reason에 사유 문자열
      (rate, n, None)   — 정상 판정, n줄 중 rate 비율 생존

    git grep -F 고정 문자열 대조라 diff 줄에 정규식 메타문자(., *, [ 등)가
    있어도 문자 그대로 비교된다 — 정규식으로 오인돼 오매칭/에러가 나는
    일은 없다.

    T10 현장 수선 6(실측, 가설 아님): 예전 버전은 `git grep HEAD`를 pathspec
    없이 돌려 레포 전체를 뒤졌다 — 그러면 세션이 만든 줄이 아니라 그 글자와
    "우연히 같은" 다른 파일의 기존 코드가 매치돼도 "생존"으로 잡힌다
    (added_lines_by_file의 docstring에 있는 zoxide 세션 실측 사례 참조).
    지금은 각 줄을 diff가 귀속한 파일(`git grep ... -- <path>`)로 한정해서만
    찾는다. 그 파일 자체가 HEAD에 없으면(이동/삭제) grep이 못 찾는 이유가
    "그 줄이 지워짐"이 아니라 "찾을 파일이 없음"이라 서로 다른 사유다 — 아래
    루프에서 cat-file로 파일 존재를 먼저 갈라 이 둘을 구분해서 다루되(파일
    없음 쪽은 stderr 한 줄로 알림), 생존 판정 결과 자체(미생존 0)는 둘 다 같다.
    """
    reason = _repo_problem(repo)
    if reason is not None:
        return None, 0, reason
    lines = added_lines_by_file(diff)
    if not lines:
        return None, 0, None
    alive = 0
    file_exists = {}  # path -> HEAD에 그 파일이 있는지 (파일당 1회만 확인)
    for path, body in lines:
        if path not in file_exists:
            e = subprocess.run(
                ["git", "-C", repo, "cat-file", "-e", f"HEAD:{path}"],
                capture_output=True,
            )
            file_exists[path] = e.returncode == 0
            if not file_exists[path]:
                print(
                    f"  경고: 대상 파일이 HEAD에 없음(session {session_id}, {path}) "
                    "— 이동/삭제됐을 수 있음. 이 파일 소속 줄은 '그 줄이 지워짐'이",
                    file=sys.stderr,
                )
                print(
                    "  아니라 '찾을 파일 자체가 없음'으로 미생존 처리한다",
                    file=sys.stderr,
                )
        if not file_exists[path]:
            continue  # 파일이 없으니 그 안 줄도 당연히 미생존 — grep 돌릴 필요 없음
        r = subprocess.run(
            ["git", "-C", repo, "grep", "-qF", body, "HEAD", "--", path],
            capture_output=True,
        )
        if r.returncode == 0:
            alive += 1
        elif r.returncode > 1:
            # 0=매치, 1=미매치, 2+=git 자체 오류(예: HEAD 없음) — 마지막 경우를
            # "미매치"로 조용히 뭉개면 생존율이 이유 없이 낮게 잡힌다
            err = r.stderr.decode(errors="replace").strip()
            print(f"  경고: git grep 실패(session {session_id}): {err}", file=sys.stderr)
    return alive / len(lines), len(lines), None


# status_note.rs가 렌더하는 상태선의 고정 형식(직접 확인, src/agent/status_note.rs
# 100행 근처: `format!("turns: {} of {} used", ctx.turn, ctx.max_turns)`). 이 정규식
# 자체는 세 기존 마커(sr_error/length_retry/BADARGS_KEY_PREFIX)의 복제가 아니라
# 별개 목적(턴 상한 M 회수)의 새 패턴이라 exp_metrics.py에 대응 상수가 없다.
_TURNS_LINE_RE = re.compile(r"turns: \d+ of (\d+) used")


def _max_turns_from_status(events):
    """이 세션 자신의 [status] 줄에서 max_turns(M)를 그대로 읽어온다 — 25를

    하드코딩하지 않는 이유: max_turns는 `.loco/config.toml`로 레포마다 덮어쓸
    수 있는 값이라(파일럿 4개 레포 모두 기본값 25를 쓰지만, 이 스크립트가
    그 사실에 의존하면 설정이 바뀌는 순간 조용히 틀린다), 세션이 실제로 겪은
    상한을 트랜스크립트 자신에게 직접 묻는다. status_note.rs는
    "turns: {turn} of {max_turns} used"를 문자 그대로 렌더하므로 M은 추정이
    아니라 그 세션의 실측값이다.

    상태선이 하나도 없으면(has_note_channel==false인 채널 없는 턴만 있었거나,
    세션이 turn 3 전에 끝났거나) None — 상한을 모른다는 뜻이고, 호출부는
    None에서 절대 발화하지 않는다(불확실성에는 발화 금지). 상태선이 여럿인데
    M 값이 서로 다르면(정상 경로에서는 있을 수 없다 — max_turns는 run() 동안
    고정이다; 트랜스크립트가 손상/이어붙여진 신호) 어느 쪽이 맞는지 추측하지
    않고 역시 None으로 판정 불가 처리한다.
    """
    seen = set()
    for e in events:
        if e.get("kind") != "user":
            continue
        content = e.get("content") or ""
        if MARKS["status_note"] not in content:
            continue
        m = _TURNS_LINE_RE.search(content)
        if m:
            seen.add(int(m.group(1)))
    if len(seen) == 1:
        return seen.pop()
    return None


def _parse_assistant_action(text):
    """assistant 원문(코드펜스 가능)에서 최선 노력으로 "action" 객체를 뽑는다.

    scripts/exp_metrics.py::parse_fail_first의 최소 판별 사다리(코드펜스 제거 →
    첫 "{" ~ 마지막 "}" 슬라이스 → json.loads)를 그대로 따라간다 — 그 함수를
    직접 import하지 않는 이유는 그건 pass/fail 비트만 돌려주고 파싱된 값
    자체를 버리기 때문이다(여기선 action.tool 값이 필요). 실패하면 항상
    None(추측하지 않음) — 아래 _finish_ever_emitted가 "모르면 False로 접어라"
    원칙을 지키는 전제가 이 함수의 보수성이다.
    """
    if "{" not in text:
        return None
    if "```" in text:
        for p in text.split("```"):
            p = p.lstrip()
            if p.startswith("json"):
                p = p[4:]
            if p.lstrip().startswith("{"):
                text = p
                break
    start = text.find("{")
    end = text.rfind("}")
    if start < 0 or end <= start:
        return None
    try:
        obj = json.loads(text[start:end + 1])
    except (ValueError, TypeError):
        return None
    if not isinstance(obj, dict):
        return None
    action = obj.get("action")
    return action if isinstance(action, dict) else None


def _finish_ever_emitted(events):
    """모델이 한 번이라도 action.tool == "finish"를 냈으면 True(수락/거부 불문).

    agent/mod.rs 295행의 실제 분기(`turn.action.tool == "finish"`)와 같은
    필드만 본다 — args 안에 우연히 끼어든 tool 키(M12 §3-2 정규화 대상)는
    다른 관심사이므로 여기서 참조하지 않는다. 파싱 안 되는(malformed) turn은
    finish였는지 알 수 없으므로 건너뛴다(추측 금지) — 이 함수가 False를
    돌려준다고 해서 "finish가 없었다"가 증명되는 게 아니라 "찾지 못했다"에
    가깝다는 뜻이라, 아래 턴 소진 판정에서 이 비대칭이 어느 쪽으로도
    안전한지는 호출부 docstring에 적어 둔다.
    """
    for e in events:
        if e.get("kind") != "assistant":
            continue
        action = _parse_assistant_action(e.get("content") or "")
        if action and action.get("tool") == "finish":
            return True
    return False


def _turns_used_no_finish(events):
    """agent/mod.rs의 실제 `turns` 카운터를 재구성한다 — finish가 한 번도 안
    나온 세션에 한해서만 유효하다(호출부가 _finish_ever_emitted로 먼저 거른
    뒤에만 쓸 것: finish 인자 누락·VERIFY_NUDGE 반려 경로는 각자 다른
    turns += 1 지점을 열기 때문에 이 함수가 재현하지 않는다).

    실제 turns 증가 지점은 딱 두 갈래뿐이다(finish 미호출 한정):
      - 정상 디스패치(mod.rs 424행 근처): assistant 다음에 tool_result가 온다.
      - 출력 잘림 재시도(finish_reason=="length", mod.rs 248행): assistant
        다음에 MARKS["length_retry"]를 담은 user가 온다.
    순수 JSON 파싱 실패 재시도(mod.rs 257-283행, PARSE_ATTEMPTS)도 assistant를
    하나 더 남기지만, 코드 주석대로 max_turns에 계상되지 않고, 그 다음
    이벤트가 위 두 모양 어느 쪽도 아니므로 별도 마커 없이 자연히 제외된다.

    트랜스크립트가 매달린 assistant(다음 이벤트가 로그에 없음 — 턴 도중
    중단)로 끝나면 그 마지막 턴은 과소계상된다 — 의도된 안전한 방향이다
    (과다계상만이 거짓 "상한 도달"을 만들어낼 수 있으므로).
    """
    n = 0
    for i, e in enumerate(events):
        if e.get("kind") != "assistant":
            continue
        if i + 1 >= len(events):
            continue
        nxt = events[i + 1]
        if nxt.get("kind") == "tool_result":
            n += 1
        elif nxt.get("kind") == "user" and MARKS["length_retry"] in (nxt.get("content") or ""):
            n += 1
    return n


def classify(row):
    """세션 1건 -> [(범주, 증거출처), ...]. 다중 라벨 허용.

    증거 출처는 "행 전체"가 아니라 "범주마다" 매긴다. 행 전체에 단일
    source를 매기면(브리프 초안이 그랬다) 예컨대 "뮤테이션 0회인데
    verdict=성공"처럼 기계 신호(뮤테이션 카운트)와 사용자 신호(verdict)가
    한 판정 안에서 섞이는 경우를 뭉갠다 — 스펙 §4-4가 지키려는 축 그 자체다.

    - "loco 비정상 종료": transcript 유무와 무관하게 항상 기계 판정이다
      (loco_exit은 셸 종료 상태 그대로 기록된 필드라 해석의 여지가 없다).
    - "S/R 루프"/"인자 누락(BadArgs)"/"length 루프"/"반복 루프(교정 발동)":
      transcript 본문에서 검색한 고정 마커(모두 exp_metrics.py에서 import,
      아래 참조) — 기계 판정.
      "length 루프"의 마커(MARKS["length_retry"])는 finish_reason=="length"일
      때 session.push(ChatMessage::user(...))로 남는 고정 재시도 문구다
      (src/agent/mod.rs 234-239행) — kind가 "user"라 bodies에 포함된다.
      "반복 루프(교정 발동)"의 마커(MARKS["repeat_corr"])는 agent/repetition.rs의
      8턴 윈도 반복 감지가 3번째 동일 호출에서 주입하는 REPEAT_CORRECTION
      고정 문구다(src/agent/mod.rs의 상수 정의, 555행 근처 주입 지점) — 역시
      kind가 "user"라 bodies에 포함된다. 세션 6/20 관측(CATEGORIES 옆 주석
      참조)에서 신설.
    - "뮤테이션 0회 거짓 finish"/"뮤테이션 없는 탐색 루프": 뮤테이션
      성공 횟수(기계, transcript 파싱)를 verdict(사용자 자기보고)로 가른다.
      가르는 축인 뮤테이션 카운트 자체가 transcript 파싱 결과이므로
      machine으로 표기하지만, 사용자 verdict 없이는 두 범주 중 어느 쪽인지
      정해지지 않는다는 점은 알아 둘 것.
      task_type이 코드 변경 과제(bugfix/feature/refactor/test)가 아니면
      "거짓 finish" 판정을 하지 않는다 — explore/other처럼 애초에 코드를
      안 고쳐도 되는 과제에서 뮤테이션 0회는 정상이지 의심 신호가 아니다.
    - "턴 소진(finish 미호출)": _finish_ever_emitted(events)가 False이고,
      _max_turns_from_status(events)로 회수한 상한 M에 _turns_used_no_finish
      (events)가 도달했을 때만 — 기계 판정. M을 회수 못 하면(상태선 자체가
      없는 트랜스크립트) 절대 발화하지 않는다(세 헬퍼 모두 docstring에
      각자의 보수적 방향을 적어 뒀다 — 전부 "모르면 0/False/None" 쪽으로
      기운다). 세션 6/20 관측(CATEGORIES 옆 주석 참조)에서 신설 — "뮤테이션
      없는 탐색 루프"와 다중 라벨로 함께 붙을 수 있다(같은 세션이 무뮤테이션
      *이면서* 턴도 다 썼을 수 있다).
    - "산출물 빌드/테스트 실패": 원장의 build_status/test_status가 "fail"일 때 —
      기계 판정(pilot.sh가 세션 후 직접 cargo를 돌려 기록한 값). transcript가
      아니라 원장 필드를 읽는 유일한 범주다. 필드가 없는 행(2e22f41 이전)에는
      침묵한다 — None을 "pass"로 읽으면 못 잰 것이 통과로 둔갑한다. 세션
      14/20(R4) 관측에서 신설, 사유는 CATEGORIES 옆 주석 참조.
    - "실패 없음": 위 어떤 범주도 안 걸렸고 verdict가 성공일 때만 — 순수
      사용자 자기보고(반증하는 기계 신호가 없다는 뜻일 뿐, 기계가 "성공"을
      확인해 준 게 아니다).
    - "기타": 위 어느 것도 아님 — 아래 미분류 세부에서 reason을 사람이 읽어야 한다.

    "컨텍스트 오버플로"/"엉뚱한 파일 편집"(UNINSTRUMENTED의 두 키)은 이 함수가
    절대 붙이지 않는다 — 이유는 위 CATEGORIES 옆 주석 참조(계측 공백, 버그 아님).
    """
    cats = []  # [(category, source)]

    if row["loco_exit"] != 0:
        cats.append(("loco 비정상 종료(exit≠0)", "machine"))

    tpath = row.get("transcript") or ""
    if tpath and os.path.exists(tpath):
        try:
            with open(tpath, encoding="utf-8") as tf:
                events = [json.loads(ln) for ln in tf if ln.strip()]
        except (ValueError, OSError):
            # 트랜스크립트는 스펙상 best-effort 부산물이다(CLAUDE.md) — 원장
            # 본문과 달리 이거 하나 못 읽는다고 전체 집계를 멈추지 않는다.
            # 다만 이 행의 기계 판정은 이 이하로 전부 미판정(신호 없음) 취급된다.
            events = []
        # assistant 이벤트는 마커 카운트에서 제외한다(exp_metrics.py와 동일 원칙
        # — 모델이 교정문을 그대로 인용할 수 있어 오탐 소지가 있다)
        bodies = " ".join(
            (e.get("content") or "") for e in events if e.get("kind") != "assistant"
        )
        muts = sum(
            1
            for e in events
            if e.get("kind") == "tool_result"
            and e.get("tool") in ("edit_file", "write_file")
            and not (e.get("content") or "").startswith(("Error:", "Denied:"))
        )
        if MARKS["sr_error"] in bodies:
            cats.append(("S/R 루프", "machine"))
        if BADARGS_KEY_PREFIX in bodies:
            cats.append(("인자 누락(BadArgs)", "machine"))
        if MARKS["length_retry"] in bodies:
            cats.append(("length 루프", "machine"))
        if MARKS["repeat_corr"] in bodies:
            cats.append(("반복 루프(교정 발동)", "machine"))
        if muts == 0:
            if row.get("verdict") == "성공" and row.get("task_type") in CODE_CHANGE_TASK_TYPES:
                cats.append(("뮤테이션 0회 거짓 finish", "machine"))
            elif row.get("verdict") != "성공":
                cats.append(("뮤테이션 없는 탐색 루프", "machine"))
            # else: 코드 변경이 필요 없는 과제(explore/other 등)에서의 무뮤테이션
            # 성공은 정상 — 범주를 붙이지 않고 "실패 없음"으로 흘러가게 둔다
        # "턴 소진(finish 미호출)" — finish가 한 번도 안 나왔고(수락이든 거부든),
        # 상태선에서 회수한 상한 M을 실제 사용 턴 수가 채웠을 때만 발화한다.
        # M을 못 구하면(상태선 없음) 아예 판정하지 않는다 — 불확실성에는
        # 발화 금지(요구사항 그대로, 거짓 양성이 실패 건수를 부풀리므로).
        if not _finish_ever_emitted(events):
            ceiling = _max_turns_from_status(events)
            if ceiling is not None and _turns_used_no_finish(events) >= ceiling:
                cats.append(("턴 소진(finish 미호출)", "machine"))

    # "산출물 빌드/테스트 실패" — transcript가 아니라 원장 필드에서 읽는다.
    # pilot.sh가 세션 후 직접 cargo를 돌려 기록한 값이라 기계 판정이다.
    # 필드가 없는 행(2e22f41 이전)에는 침묵한다 — None을 "pass"로 읽지 않는다.
    if row.get("build_status") == "fail" or row.get("test_status") == "fail":
        cats.append(("산출물 빌드/테스트 실패", "machine"))

    if not cats and row.get("verdict") == "성공":
        cats.append(("실패 없음", "user"))
    if not cats:
        cats.append(("기타", "user"))
    return cats


def main():
    if len(sys.argv) != 2:
        print(f"오류: 인자 개수가 잘못됐습니다 (주어진 {len(sys.argv)-1}개, 필요 1개 — 원장 경로만)")
        print("새로운 형식: python3 scripts/pilot_tally.py <ledger.jsonl>")
        print("참고: 레포는 ledger 행의 repo 필드에서 읽습니다 (더 이상 CLI 인자가 아닙니다)")
        sys.exit(1)
    ledger_path = sys.argv[1]
    rows = load_ledger(ledger_path)
    print(f"# 파일럿 집계 — 세션 {len(rows)}개")
    print("※ 헤드라인은 아래 '범주별 건수'다. 그 다음 절들(판정 분포·난이도·줄 생존율)은")
    print("  전부 보조 통계다 — 특히 줄 생존율은 대리 지표이니 그 자체로 인용하지 말 것.\n")

    classified = [(r, classify(r)) for r in rows]

    cat_counts = Counter()
    cat_src_counts = Counter()  # (category, source) -> count
    for _, cats in classified:
        for cat, src in cats:
            cat_counts[cat] += 1
            cat_src_counts[(cat, src)] += 1

    print("## 범주별 건수 (주 산출물, 스펙 §4-4)")
    print("다중 라벨 — 합계는 세션 수와 같지 않다. 괄호는 증거 출처(기계 판정/사용자 사유) 내역")
    print("△ 표시 행은 계측 공백이다 — 항상 0이며, 아래 각주 참조\n")
    extra = [c for c in cat_counts if c not in CATEGORIES]
    for c in CATEGORIES + extra:
        m = cat_src_counts[(c, "machine")]
        u = cat_src_counts[(c, "user")]
        flag = "△" if c in UNINSTRUMENTED else " "
        print(f"{flag} {c:<28} {cat_counts[c]:>3}   (기계 {m} / 사용자 {u})")
    if any(c in UNINSTRUMENTED for c in CATEGORIES + extra):
        print("\n  △ = 계측 불가(이 스크립트가 절대 세지 않아 항상 0) — '측정해서 0건'이")
        print("  아니라 '잴 방법이 없음'이다. 실제 발생 여부는 이 표에서 알 수 없다:")
        for c in CATEGORIES:
            if c in UNINSTRUMENTED:
                print(f"    - {c}: {UNINSTRUMENTED[c]}")

    # 부분 계측 각주 — build_status가 없는 행이 있으면 몇 행인지 밝힌다.
    # UNINSTRUMENTED(항상 0)와 달리 이건 "일부 행만 잴 수 있다"라서 △가 아니다.
    no_build = [r for r in rows if r.get("build_status") is None]
    if no_build:
        print(f"\n  ※ '산출물 빌드/테스트 실패'는 {len(rows)}행 중 {len(no_build)}행을 못 잰다 —")
        print("  build_status/test_status 필드가 2e22f41 이후 세션에만 있다. 못 잰 행은")
        print("  이 범주에서 침묵하므로, 이 수치는 하한이다(실제 발생은 이보다 많을 수 있다).")

    build_failed = [
        r for r in rows
        if r.get("build_status") == "fail" or r.get("test_status") == "fail"
    ]
    if build_failed:
        print("\n## 산출물 빌드/테스트 실패 세부 — 판정이 '성공'이어도 트리가 깨졌을 수 있다")
        for r in build_failed:
            detail = (r.get("build_detail") or r.get("test_detail") or "(상세 미기재)").strip()
            print(f"  {r['session_id']}  build={r.get('build_status')} test={r.get('test_status')}  판정={r.get('verdict')}")
            print(f"    {detail[:160]}")

    crashed = [r for r in rows if r["loco_exit"] != 0]
    if crashed:
        print("\n## loco 비정상 종료 세부 — 원장은 원인을 모른다(stderr 미기록), reason을 사람이 읽을 것")
        for r in crashed:
            reason = r.get("reason") or "(사유 미기재)"
            print(f"  {r['session_id']}  exit={r['loco_exit']}  판정={r.get('verdict')} — {reason}")

    others = [r for r, cats in classified if cats == [("기타", "user")]]
    if others:
        print("\n## 미분류(\"기타\") 세부 — 기계 판정이 못 잡은 패턴은 사유를 읽고 수동 재분류할 것")
        for r in others:
            reason = r.get("reason") or "(사유 미기재)"
            print(f"  {r['session_id']}  [{r.get('task_type', '?')}] {r.get('verdict')} — {reason}")

    unknown_types = {}  # task_type 값 -> [session_id, ...]
    for r in rows:
        tt = r.get("task_type")
        if tt not in KNOWN_TASK_TYPES:
            unknown_types.setdefault(tt, []).append(r["session_id"])
    if unknown_types:
        print("\n## 미확인 task_type 세부 — pilot.sh 안내 어휘"
              f"({', '.join(sorted(KNOWN_TASK_TYPES))}) 밖의 값이다.")
        print("오타·동의어일 수 있으나 자동 정규화·재분류하지 않는다 — 아래 세션의 분류 결과를")
        print("사람이 직접 확인할 것(코드 변경 과제인데 어휘가 어긋나 '실패 없음'으로 조용히")
        print("흡수됐을 위험이 가장 크다):")
        for tt in sorted(unknown_types, key=str):
            ids = unknown_types[tt]
            print(f"  task_type={tt!r}  {len(ids)}건 — {', '.join(ids)}")

    print("\n## 판정 분포 (기술 통계)")
    for v, n in Counter(r.get("verdict") for r in rows).most_common():
        print(f"  {v:<16} {n}")

    print("\n## 난이도 × 판정 (분모 — 세션 전 수집)")
    for d in ("상", "중", "하"):
        sub = [r for r in rows if r.get("difficulty") == d]
        if sub:
            ok = sum(1 for r in sub if r.get("verdict") == "성공")
            print(f"  난이도 {d}: {len(sub)}세션, 성공 {ok}")

    print("\n## 줄 생존율 (보조 대리 지표 — 왜곡 5종 알려짐, 스펙 §4-3. 주 산출물 아님)")
    print("레포별로 나눈다 — 레포 규모가 20배 이상 차이 나 pooled 수치 하나면 큰 레포가")
    print("작은 레포를 가려버린다(T10 리뷰 수선 2). 아래는 보조 지표 안에서의 세부일")
    print("뿐, 위 범주별 건수(주 산출물)를 밀어내는 것은 아니다.")
    print("여섯째 왜곡(발견·수선 완료): 예전 버전은 생존 검사를 레포 전체에 돌려")
    print("세션이 만든 적 없는 다른 파일의 상용구까지 '생존'으로 셌다 — 실제 파일럿")
    print("원장에서 실측(session 20260719T134645Z, 가설 아님): 진짜 세션 산출물 0줄인데")
    print("30.0%(10줄)로 찍혔다. 지금은 diff가 귀속한 파일 안에서만 찾도록 닫았다.")

    by_repo = {}
    for r in rows:
        by_repo.setdefault(r["repo"], []).append(r)

    overall_alive, overall_lines, overall_judged = 0.0, 0, 0
    unjudgeable = []  # [(session_id, repo, reason), ...] — 레포 경로 문제로 아예 판정 못 한 행
    for repo in sorted(by_repo):
        sub = by_repo[repo]
        print(f"\n### {repo}")
        repo_alive, repo_lines, repo_judged = 0.0, 0, 0
        repo_unjudgeable_count = 0  # 이 repo에서 판정 불가(경로 문제)인 행 수
        for r in sub:
            rate, n, reason = survival(repo, r.get("diff") or "", r["session_id"])
            if reason is not None:
                unjudgeable.append((r["session_id"], repo, reason))
                repo_unjudgeable_count += 1
                continue
            if rate is None:
                continue
            repo_judged += 1
            repo_alive += rate * n
            repo_lines += n
            print(f"  {r['session_id']}  {rate:5.1%} ({n}줄)  {r.get('verdict')}")
        if repo_lines:
            print(f"  레포 소계: {repo_alive / repo_lines:.1%} ({repo_judged}세션, {repo_lines}줄)")
            overall_alive += repo_alive
            overall_lines += repo_lines
            overall_judged += repo_judged
        else:
            if repo_unjudgeable_count == len(sub):
                print("  판정 가능한 세션 없음 (모든 세션이 레포 경로 문제로 판정 불가)")
            else:
                print("  판정 가능한 세션 없음 (모든 diff가 공백이거나 유의미한 추가 줄이 없음)")

    if unjudgeable:
        print("\n## 판정 불가 — 레포 경로 문제 (0%로 세지 않음, 스펙 §4-3 왜곡 5종과는 별개)")
        print("레포가 이동/삭제된 경우를 '그 안 코드가 삭제됨'(생존율 0%)과 혼동하면 안 된다.")
        print("아래 행은 생존율 집계에서 완전히 제외됐다 — 원인을 사람이 확인할 것:")
        for sid, r_path, reason in unjudgeable:
            print(f"  {sid}  repo={r_path} — {reason}")
        print(f"\n  총 {len(unjudgeable)}건 판정 제외")

    print()
    if overall_lines:
        print(
            f"  전체 가중 생존율: {overall_alive / overall_lines:.1%} "
            f"({overall_judged}세션, {overall_lines}줄, {len(by_repo)}개 레포)"
        )
    else:
        # 레포별 메시지(위)와 문구가 같으면 레포가 하나일 때 같은 말이 두 번 찍혀
        # 중복 출력처럼 보인다 — 이 줄은 전체 합계임을 스스로 밝힌다.
        print("  전체 가중 생존율: 판정 가능한 세션 없음 (전 레포 통틀어 판정 대상 0)")
    print("\n  경고: 생존율은 채택의 대리 지표일 뿐이다. 삭제가 가치였던 세션·진단만")
    print("  내놓은 세션은 0으로 잡히고, 무관한 리팩터링에 휩쓸린 채택은 미채택으로")
    print("  잡힌다. 반드시 위 범주별 건수·판정 분포와 교차해서 읽을 것 — 이 수치")
    print("  하나를 이 파일럿의 결론으로 인용하지 말 것.")


if __name__ == "__main__":
    main()
