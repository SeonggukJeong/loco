#!/bin/sh
# loco 실사용 파일럿 세션 래퍼 (M13 스펙 §4-2).
# loco 프로덕션 코드는 건드리지 않는다 — 세션을 감싸기만 한다.
#
# 사용법 (대상 레포 안에서):
#   LOCO_BIN=/path/to/loco PILOT_LEDGER=/path/to/ledger.jsonl scripts/pilot.sh
set -eu

LOCO_BIN="${LOCO_BIN:-loco}"
PILOT_LEDGER="${PILOT_LEDGER:?PILOT_LEDGER (원장 JSONL 경로)를 지정하세요}"
REPO="$(pwd)"

command -v git >/dev/null || { echo "git이 필요합니다"; exit 1; }
git rev-parse --git-dir >/dev/null 2>&1 || { echo "git 레포 안에서 실행하세요"; exit 1; }

# 세션 시작(타이밍/리비전 캡처) 전에 확인 — 실패해도 사용자 비용이 0이어야 한다.
if echo "$LOCO_BIN" | grep -q /; then
  # 슬래시 포함 = 경로 직접 지정 — 실행 비트만으로는 부족하다(-x는 디렉토리에도 참)
  [ -f "$LOCO_BIN" ] && [ -x "$LOCO_BIN" ] || { echo "LOCO_BIN이 실행 가능한 파일이 아닙니다: $LOCO_BIN"; exit 1; }
else
  # 이름만 주어짐 = PATH 탐색 — command -v로 해석한 뒤 같은 검사를 한다.
  # 셸 빌트인(예: true)은 경로가 아닌 이름을 돌려주므로 여기서 거부된다 —
  # 빌트인이 loco일 수는 없으므로 의도된 동작이다.
  resolved_bin=$(command -v "$LOCO_BIN" 2>/dev/null) || { echo "LOCO_BIN이 실행 가능한 파일이 아닙니다: $LOCO_BIN"; exit 1; }
  [ -f "$resolved_bin" ] && [ -x "$resolved_bin" ] || { echo "LOCO_BIN이 실행 가능한 파일이 아닙니다: $LOCO_BIN"; exit 1; }
fi

# 원장 경로도 같은 이유로 여기서 확인한다 — 지금까지는 세션 전체가 끝난 뒤,
# 파이썬 헤레독 안에서야 실패했다. 원장 실패는 LOCO_BIN 실패보다 비용이 크다:
# 세션 시간 전부가 날아간다.
ledger_dir=$(dirname "$PILOT_LEDGER")
[ -d "$ledger_dir" ] || { echo "원장 디렉터리가 없습니다: $ledger_dir"; exit 1; }
if [ -e "$PILOT_LEDGER" ]; then
  [ -f "$PILOT_LEDGER" ] || { echo "원장 경로가 파일이 아닙니다: $PILOT_LEDGER"; exit 1; }
  [ -w "$PILOT_LEDGER" ] || { echo "원장 파일에 쓸 수 없습니다: $PILOT_LEDGER"; exit 1; }
else
  # 없으면 지금 만들어 본다 — 별도 프로브 파일 없이 실제 원장 파일 생성 자체를
  # 사전 점검으로 쓴다: 실패하면(권한 등) 아무 파일도 남지 않고, 성공하면
  # 그 파일이 곧 첫 줄을 받을 실제 원장이다(첫 줄은 원장이 스스로 만들 수 있어야 한다).
  ( : > "$PILOT_LEDGER" ) 2>/dev/null || { echo "원장 파일을 만들 수 없습니다: $PILOT_LEDGER"; exit 1; }
fi

if [ -n "$(git status --porcelain)" ]; then
  printf '워킹트리가 더럽습니다. 세션 diff가 오염됩니다. 계속할까요? [y/N] '
  read -r ans
  [ "$ans" = "y" ] || exit 1
fi

# --- 세션 전 수집: 결과를 알기 전에 받아야 분모로 쓸 수 있다 -----------------
printf '과제 유형 한 단어 (bugfix/feature/refactor/explore/test/other): '
read -r TASK_TYPE
printf '난이도 추정 (상/중/하) — 지금 추정해야 의미가 있습니다: '
read -r DIFFICULTY
printf '과제 한 줄: '
read -r TASK_DESC

SESSION_ID="$(date -u +%Y%m%dT%H%M%SZ)"
START_REV="$(git rev-parse HEAD)"
START_TS="$(date +%s)"

# loco에는 위 "과제 한 줄"이 자동으로 전달되지 않는다 — 사용자가 loco> 프롬프트에서
# 직접 다시 입력해야 한다. 안내가 없으면 응답 후 돌아온 loco> 프롬프트를 멈춤/종료로
# 오인한다(실측: 세션은 살아 있었고 판정 프롬프트가 뒤에서 기다리고 있었을 뿐이었다).
echo ""
echo "방금 입력한 과제: $TASK_DESC"
echo "loco> 프롬프트가 뜨면 위 문장을 붙여넣으세요 (자동으로 전달되지 않습니다)."
echo "세션 종료는 loco> 프롬프트에서 /quit 또는 Ctrl+D 입니다 — 그 후에 판정/사유를 묻고 원장에 기록합니다."
echo ""

# --- 세션 ---------------------------------------------------------------------
LOCO_EXIT=0
"$LOCO_BIN" || LOCO_EXIT=$?   # 비정상 종료도 기록 대상이다

END_TS="$(date +%s)"
END_REV="$(git rev-parse HEAD)"
DURATION=$((END_TS - START_TS))

# 세션이 만든 변경 = 미커밋 워킹트리 diff + 세션 중 생긴 커밋
DIFF="$(git diff "$START_REV" 2>/dev/null || true)"

# 가장 최근 loco 세션 트랜스크립트
TRANSCRIPT="$(ls -t "$REPO"/.loco/sessions/*.jsonl 2>/dev/null | head -1 || echo "")"

# --- 세션 후 판정 -------------------------------------------------------------
printf '판정 (1=성공 2=수정해서 씀 3=버림): '
read -r V
case "$V" in
  1) VERDICT="성공" ;;
  2) VERDICT="수정해서 씀" ;;
  3) VERDICT="버림" ;;
  *) VERDICT="미기재" ;;
esac
printf '사유 한 줄: '
read -r REASON

# 값은 반드시 환경변수로 넘긴다 — 셸 변수를 파이썬 소스에 보간하면 안 된다.
# 이유(실측 확인): 파이썬 삼중따옴표는 백슬래시 이스케이프를 해석하므로
#   diff의  \"  ->  "        (백슬래시 소실)
#   diff의  \n  ->  실제 개행 (줄 구조 파괴)
#   diff의  \t  ->  탭
# 이 되고, diff에 """ 가 들어 있으면 아예 SyntaxError로 세션이 통째로 유실된다.
# 더 나쁜 것은 조용한 쪽이다: 손상된 diff도 유효한 JSON이고 길이가 0이 아니라
# "검증 통과"로 보인다. 그리고 T10의 survival()은 git grep -F 고정 문자열
# 대조라 손상된 줄이 전부 불일치 처리되어 생존율이 체계적으로 과소 계상된다.
if DIFF="$DIFF" REPO="$REPO" TASK_TYPE="$TASK_TYPE" DIFFICULTY="$DIFFICULTY" \
TASK_DESC="$TASK_DESC" TRANSCRIPT="$TRANSCRIPT" VERDICT="$VERDICT" \
REASON="$REASON" SESSION_ID="$SESSION_ID" START_REV="$START_REV" \
END_REV="$END_REV" DURATION="$DURATION" LOCO_EXIT="$LOCO_EXIT" \
python3 - "$PILOT_LEDGER" <<'PYEOF'
import json, os, sys
row = {
    "session_id": os.environ["SESSION_ID"],
    "repo": os.environ["REPO"],
    "start_rev": os.environ["START_REV"],
    "end_rev": os.environ["END_REV"],
    "task_type": os.environ["TASK_TYPE"],
    "difficulty": os.environ["DIFFICULTY"],
    "task": os.environ["TASK_DESC"],
    "transcript": os.environ["TRANSCRIPT"],
    "diff": os.environ["DIFF"],
    "duration_secs": int(os.environ["DURATION"]),
    "loco_exit": int(os.environ["LOCO_EXIT"]),
    "verdict": os.environ["VERDICT"],
    "reason": os.environ["REASON"],
}
with open(sys.argv[1], "a") as f:
    f.write(json.dumps(row, ensure_ascii=False) + "\n")
print(f"원장에 기록: {row['session_id']} ({row['verdict']})")
PYEOF
then
  :
else
  py_status=$?
  # 위 파이썬 트레이스백을 지우지 않는다(원인 확인용) — 그 위에 사용자용 한국어
  # 요약을 더한다. set -e에 걸리지 않도록 if/else로 상태를 그대로 보존해 종료한다.
  echo "원장 기록에 실패했습니다: $PILOT_LEDGER — 이 회차는 원장에 남지 않았습니다. 위 오류를 확인해 수동으로 기록하거나 다시 시도하세요." >&2
  exit "$py_status"
fi
