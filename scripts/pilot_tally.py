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
CATEGORIES = [
    "실패 없음", "S/R 루프", "뮤테이션 0회 거짓 finish", "뮤테이션 없는 탐색 루프",
    "컨텍스트 오버플로", "엉뚱한 파일 편집", "length 루프", "인자 누락(BadArgs)",
    "loco 비정상 종료(exit≠0)",
]

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


def added_lines(diff):
    """diff에서 유의미한 추가 줄만 — 공백/괄호/짧은 줄은 생존 판정 노이즈."""
    out = []
    for line in diff.splitlines():
        if not line.startswith("+") or line.startswith("+++"):
            continue
        body = line[1:].strip()
        if len(body) > 10 and body not in ("{", "}", "*/"):
            out.append(body)
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
    """
    reason = _repo_problem(repo)
    if reason is not None:
        return None, 0, reason
    lines = added_lines(diff)
    if not lines:
        return None, 0, None
    alive = 0
    for body in lines:
        r = subprocess.run(
            ["git", "-C", repo, "grep", "-qF", body, "HEAD"], capture_output=True
        )
        if r.returncode == 0:
            alive += 1
        elif r.returncode > 1:
            # 0=매치, 1=미매치, 2+=git 자체 오류(예: HEAD 없음) — 마지막 경우를
            # "미매치"로 조용히 뭉개면 생존율이 이유 없이 낮게 잡힌다
            err = r.stderr.decode(errors="replace").strip()
            print(f"  경고: git grep 실패(session {session_id}): {err}", file=sys.stderr)
    return alive / len(lines), len(lines), None


def classify(row):
    """세션 1건 -> [(범주, 증거출처), ...]. 다중 라벨 허용.

    증거 출처는 "행 전체"가 아니라 "범주마다" 매긴다. 행 전체에 단일
    source를 매기면(브리프 초안이 그랬다) 예컨대 "뮤테이션 0회인데
    verdict=성공"처럼 기계 신호(뮤테이션 카운트)와 사용자 신호(verdict)가
    한 판정 안에서 섞이는 경우를 뭉갠다 — 스펙 §4-4가 지키려는 축 그 자체다.

    - "loco 비정상 종료": transcript 유무와 무관하게 항상 기계 판정이다
      (loco_exit은 셸 종료 상태 그대로 기록된 필드라 해석의 여지가 없다).
    - "S/R 루프"/"인자 누락(BadArgs)"/"length 루프": transcript 본문에서 검색한
      고정 마커(모두 exp_metrics.py에서 import, 아래 참조) — 기계 판정.
      "length 루프"의 마커(MARKS["length_retry"])는 finish_reason=="length"일
      때 session.push(ChatMessage::user(...))로 남는 고정 재시도 문구다
      (src/agent/mod.rs 234-239행) — kind가 "user"라 bodies에 포함된다.
    - "뮤테이션 0회 거짓 finish"/"뮤테이션 없는 탐색 루프": 뮤테이션
      성공 횟수(기계, transcript 파싱)를 verdict(사용자 자기보고)로 가른다.
      가르는 축인 뮤테이션 카운트 자체가 transcript 파싱 결과이므로
      machine으로 표기하지만, 사용자 verdict 없이는 두 범주 중 어느 쪽인지
      정해지지 않는다는 점은 알아 둘 것.
      task_type이 코드 변경 과제(bugfix/feature/refactor/test)가 아니면
      "거짓 finish" 판정을 하지 않는다 — explore/other처럼 애초에 코드를
      안 고쳐도 되는 과제에서 뮤테이션 0회는 정상이지 의심 신호가 아니다.
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
        if muts == 0:
            if row.get("verdict") == "성공" and row.get("task_type") in CODE_CHANGE_TASK_TYPES:
                cats.append(("뮤테이션 0회 거짓 finish", "machine"))
            elif row.get("verdict") != "성공":
                cats.append(("뮤테이션 없는 탐색 루프", "machine"))
            # else: 코드 변경이 필요 없는 과제(explore/other 등)에서의 무뮤테이션
            # 성공은 정상 — 범주를 붙이지 않고 "실패 없음"으로 흘러가게 둔다

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
        print("  판정 가능한 세션 없음 (모든 diff가 공백이거나 유의미한 추가 줄이 없음)")
    print("\n  경고: 생존율은 채택의 대리 지표일 뿐이다. 삭제가 가치였던 세션·진단만")
    print("  내놓은 세션은 0으로 잡히고, 무관한 리팩터링에 휩쓸린 채택은 미채택으로")
    print("  잡힌다. 반드시 위 범주별 건수·판정 분포와 교차해서 읽을 것 — 이 수치")
    print("  하나를 이 파일럿의 결론으로 인용하지 말 것.")


if __name__ == "__main__":
    main()
