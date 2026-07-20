#!/bin/sh
# loco 실사용 파일럿 세션 래퍼 (M13 스펙 §4-2).
# loco 프로덕션 코드는 건드리지 않는다 — 세션을 감싸기만 한다.
#
# 사용법 (대상 레포 안에서):
#   LOCO_BIN=/path/to/loco PILOT_LEDGER=/path/to/ledger.jsonl scripts/pilot.sh
# 선택 환경변수: PILOT_BUILD_TIMEOUT_SECS(기본 300), PILOT_TEST_TIMEOUT_SECS(기본 600)
#   — 세션 후 build/test 확인(T9 현장 수선 2)의 시간 상한.
set -eu

LOCO_BIN="${LOCO_BIN:-loco}"
PILOT_LEDGER="${PILOT_LEDGER:?PILOT_LEDGER (원장 JSONL 경로)를 지정하세요}"
# 세션 후 build/test 확인 시간 상한(초, T9 현장 수선 2) — 하드코딩하지 않고
# 레포별로 조정할 수 있게 환경변수로 뺀다: just의 테스트 스위트는 서브프로세스
# ~1,851개를 띄우고 ripgrep 전체 워크스페이스 빌드는 느리다 — 기본값을 넉넉히
# 잡아야 정상적으로 오래 걸리는 빌드/테스트를 시간초과로 오분류하지 않는다.
PILOT_BUILD_TIMEOUT_SECS="${PILOT_BUILD_TIMEOUT_SECS:-300}"
PILOT_TEST_TIMEOUT_SECS="${PILOT_TEST_TIMEOUT_SECS:-600}"
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

# 세션이 만든 변경 = 미커밋 워킹트리 diff + 세션 중 생긴 커밋 + **신규 파일**.
# `git add -N`으로 추적되지 않은 파일을 intent-to-add로 올려야 `git diff`가 본다 —
# 안 그러면 `write_file`로 새 파일만 만든 성공 세션이 "산출물 0줄"로 기록되고,
# 하류 생존율 집계에서 실패 세션과 구별되지 않는다(`write_file`은 6-tool set의
# 정식 멤버라 가설이 아니다). `.loco/`는 제외 — 우리 자신의 부산물이다.
git add -N -- . ':(exclude).loco' >/dev/null 2>&1 || true
if ! DIFF="$(git diff "$START_REV")"; then
  DIFF=""
  echo "경고: git diff 실패 — 원장의 diff가 비어 있으나 '변경 없음'을 뜻하지 않습니다." >&2
fi

# 이번 세션의 트랜스크립트. **mtime 하한이 필수다** — `ls -t | head -1`로 두면
# loco가 크래시해 트랜스크립트를 못 남겼을 때 **직전 세션 것이 이번 세션에
# 귀속**되고, 집계기가 그 마커를 세어 유령 범주를 만든다(침묵보다 나쁜, 적극적
# 오데이터). 이름 대조가 아니라 mtime인 이유: loco 스탬프가 이 스크립트보다
# 1초 늦게 찍히는 경우가 실제로 있다(파일럿 20건 중 3건).
TRANSCRIPT="$(find "$REPO/.loco/sessions" -name '*.jsonl' -newermt "@$START_TS" 2>/dev/null | sort | tail -1 || echo "")"
if [ -z "$TRANSCRIPT" ]; then
  echo "경고: 이번 세션의 트랜스크립트를 찾지 못했습니다 — 기계 판정 없이 기록됩니다." >&2
fi

# --- 세션 후 빌드/테스트 확인 (T9 현장 수선 2) -------------------------------
# 배경(실측): 세션 20260719T150304Z(fd, F4)는 그럴듯해 보이는 diff만 보고
# "수정해서 씀"으로 판정됐다. 실제로는 cargo build는 통과했지만 cargo test는
# 컴파일조차 안 됐고(중복 테스트 함수 + 존재하지 않는 fd_find 크레이트 참조 —
# fd는 바이너리 전용 크레이트다), fd --path-separator=@ 실행은 모델이 쓴
# 룩어라운드 정규식((?<![/])/(?![/]))이 Rust regex 크레이트에서 지원되지 않아
# 바로 그 변경이 건드린 코드 경로에서 panic했다. diff를 눈으로 읽는 것만으로는
# 이 중 무엇도 드러나지 않았다 — 판정 프롬프트 전에 실행 증거가 있어야 사람이
# 속지 않는다. 아래는 증거일 뿐 판정을 대신하지 않는다: 통과/실패를 자동으로
# 판정에 반영하거나 제안하지 않고, 사람이 보는 화면에 판정 프롬프트보다 먼저
# 찍어 두기만 한다.
#
# 프로젝트 종류는 하드코딩하지 않고 감지한다 — 지금 파일럿 4개 레포가 전부
# Rust라 해도 그 가정을 코드에 그냥 박아두지 않고 사람이 보는 출력 문구로
# 명시한다(다른 언어 레포에 이 스크립트를 그대로 붙여 써도 조용히 틀린 값을
# 내지 않게). 인식 가능한 프로젝트 파일이 없으면 확인을 건너뛰고 그 사실만
# 기록한다 — 이 확인은 부가 증거이지 게이트가 아니므로 세션 자체를 실패시키지
# 않는다.
BUILD_STATUS="skipped"
BUILD_DETAIL="Cargo.toml 없음 — 인식 가능한 프로젝트 파일 없음"
BUILD_SECS=0
TEST_STATUS="skipped"
TEST_DETAIL="Cargo.toml 없음 — 인식 가능한 프로젝트 파일 없음"
TEST_SECS=0

# macOS에는 timeout(1)이 없다 — 이 프로젝트에서 이미 한 번 "있다"고 잘못
# 가정해 명령이 조용히 no-op 난 전례가 있으므로, timeout을 자체 구현하고
# 아래 검증 단계에서 실측으로 동작을 확인한다(침묵 no-op이 아니라 실제로
# 프로세스를 죽이는지). 프로세스 그룹(kill -PGID) 대신 pgrep -P로 자손을
# 재귀적으로 찾아 죽인다 — 비대화형 sh는 job control이 꺼져 있어 백그라운드
# 잡이 스크립트 자신과 같은 프로세스 그룹에 남을 수 있고, 그러면 -PGID kill이
# 스크립트 자신까지 죽여 버린다. pgrep -P 순회는 job control 유무에 기대지
# 않는다.
kill_tree() {
  # $1=pid $2=시그널 이름(TERM/KILL)
  for _kt_child in $(pgrep -P "$1" 2>/dev/null || true); do
    kill_tree "$_kt_child" "$2"
  done
  kill "-$2" "$1" 2>/dev/null || true
}

# 명령을 백그라운드로 돌리고 timeout초 안에 안 끝나면 프로세스 트리를 통째로
# 죽인다. 결과는 전역변수 _RWT_STATUS(정상 종료 시 그 종료코드, 시간초과 시
# 관례값 124)/_RWT_TIMEDOUT(0|1)으로 돌려준다 — 함수 자신의 반환값은 항상
# 0으로 고정한다: 만약 여기서 실패를 그대로 반환하면 그 함수 호출 자체가
# set -e에 걸려 스크립트 전체가 중단된다. cargo build/test가 실패하는 것은
# 정상적으로 있을 수 있는 결과이지 이 스크립트의 오류가 아니므로, 그 실패가
# 스크립트를 죽이면 안 된다(원장 기록까지 못 가고 세션 전체가 유실된다).
run_with_timeout() {
  _rwt_timeout="$1"; _rwt_out="$2"; shift 2
  rm -f "$_rwt_out.timedout"
  "$@" >"$_rwt_out" 2>&1 &
  _rwt_pid=$!
  (
    sleep "$_rwt_timeout"
    if kill -0 "$_rwt_pid" 2>/dev/null; then
      : > "$_rwt_out.timedout"
      kill_tree "$_rwt_pid" TERM
      sleep 1
      kill_tree "$_rwt_pid" KILL
    fi
  ) &
  _rwt_watcher=$!
  _rwt_status=0
  wait "$_rwt_pid" 2>/dev/null || _rwt_status=$?
  # 제시간에 끝났다면 감시자를 정리한다 — 안 그러면 timeout초가 다 지날 때까지
  # 백그라운드에 남아 다음 확인(cargo test)의 감시자와 뒤섞일 수 있다.
  #
  # 반드시 kill_tree로 죽인다. 감시자는 서브셸이고 그 안의 `sleep`은 별도
  # 프로세스라, 서브셸만 kill하면 sleep이 PPID 1로 고아가 되어 살아남는다
  # (실측 확인: 감시자 kill 후 `ps`에 `sleep 120`이 PPID 1로 잔존). 고아 sleep은
  # 스크립트의 stdout을 물려받은 채 타임아웃 전체를 버티므로, 파이프로 읽는
  # 호출자(스크립트를 파이프에 물린 자동화·터미널 래퍼)는 세션이 끝난 뒤에도
  # 타임아웃초만큼 매달린다 — 실제로 빌드/테스트가 1초에 끝난 스모크가 600초를
  # 넘겼고 그 원인이 이것이었다.
  kill_tree "$_rwt_watcher" TERM
  wait "$_rwt_watcher" 2>/dev/null || true
  if [ -f "$_rwt_out.timedout" ]; then
    _RWT_TIMEDOUT=1
    _RWT_STATUS=124
  else
    _RWT_TIMEDOUT=0
    _RWT_STATUS="$_rwt_status"
  fi
  return 0
}

# 출력 파일에서 첫 error 줄을 뽑아 나중에 훑어볼 최소 단서로 남긴다. cargo는
# 출력이 tty가 아니면 색을 끄므로 "^error"로 줄 시작만 앵커링해도 된다(카펫된
# 로그에 우연히 등장한 "error"에 스푸핑되지 않도록 줄 시작 고정).
first_error_line() {
  _fel_line="$(grep -m1 '^error' "$1" 2>/dev/null || true)"
  if [ -z "$_fel_line" ]; then
    _fel_line="$(grep -m1 '[^[:space:]]' "$1" 2>/dev/null || true)"
  fi
  printf '%s' "$_fel_line" | cut -c1-200
}

# 확인 하나를 실행하고 RESULT_STATUS/RESULT_DETAIL/RESULT_SECS에 결과를 남긴다.
# 상태값은 pass/fail/skipped 세 가지 제안을 넘어 timeout을 네 번째 값으로 둔다
# — "시간초과"를 "실패"에 뭉쳐 넣으면 나중에 "빌드가 실제로 깨졌다"와 "빌드
# 확인이 인프라 사정(느린 레포·행)으로 못 끝났다"를 구분할 수 없어진다(요구
# 사항: 시간초과는 실패와 distinct하게 기록). scripts/pilot_tally.py는 이
# 필드들을 아직 소비하지 않지만(기존 9개 행에는 없음, REQUIRED_FIELDS에 없어
# 그래도 로드된다) 이름은 고정값으로 취급한다 — 나중에 그 스크립트가 이 필드를
# 읽게 될 것이므로 여기서 이름을 바꾸면 안 된다.
run_check() {
  _rc_label="$1"; _rc_timeout="$2"; _rc_out="$3"; shift 3
  _rc_t0=$(date +%s)
  run_with_timeout "$_rc_timeout" "$_rc_out" "$@"
  _rc_t1=$(date +%s)
  RESULT_SECS=$((_rc_t1 - _rc_t0))
  if [ "$_RWT_TIMEDOUT" -eq 1 ]; then
    RESULT_STATUS="timeout"
    RESULT_DETAIL="${_rc_timeout}초 제한 초과로 강제 종료"
  elif [ "$_RWT_STATUS" -eq 0 ]; then
    RESULT_STATUS="pass"
    RESULT_DETAIL=""
  else
    RESULT_STATUS="fail"
    RESULT_DETAIL="$(first_error_line "$_rc_out")"
  fi
  # 판정 프롬프트가 뜨기 전에 눈에 보이는 자리에 찍는다 — 대기 시간이 어디서
  # 들었는지 보이도록 소요 시간도 함께 보여준다.
  echo "  $_rc_label: $RESULT_STATUS (${RESULT_SECS}초)"
  if [ -n "$RESULT_DETAIL" ]; then
    echo "    -> $RESULT_DETAIL"
  fi
}

echo "--- 세션 후 빌드/테스트 확인 ---"
if [ -f "$REPO/Cargo.toml" ]; then
  echo "Cargo.toml 발견 — cargo 프로젝트로 가정하고 build/test를 확인합니다."
  if BUILD_OUT=$(mktemp) && TEST_OUT=$(mktemp); then
    run_check "cargo build" "$PILOT_BUILD_TIMEOUT_SECS" "$BUILD_OUT" cargo build
    BUILD_STATUS="$RESULT_STATUS"; BUILD_DETAIL="$RESULT_DETAIL"; BUILD_SECS="$RESULT_SECS"

    run_check "cargo test" "$PILOT_TEST_TIMEOUT_SECS" "$TEST_OUT" cargo test
    TEST_STATUS="$RESULT_STATUS"; TEST_DETAIL="$RESULT_DETAIL"; TEST_SECS="$RESULT_SECS"

    rm -f "$BUILD_OUT" "$BUILD_OUT.timedout" "$TEST_OUT" "$TEST_OUT.timedout"
  else
    echo "임시 파일을 만들 수 없어 build/test 확인을 건너뜁니다." >&2
    BUILD_STATUS="skipped"; BUILD_DETAIL="임시 파일 생성 실패"
    TEST_STATUS="skipped"; TEST_DETAIL="임시 파일 생성 실패"
  fi
else
  echo "인식 가능한 프로젝트 파일이 없습니다(Cargo.toml 기준) — build/test 확인을 건너뜁니다."
fi
echo "--------------------------------"
echo ""

# --- 세션 후 판정 -------------------------------------------------------------
# ★ EOF 내성이 필수다. `set -e` 하에서 `read`는 EOF에 non-zero를 반환하므로
# 그냥 `read -r V`로 두면 스크립트가 **아무 메시지 없이** 죽고 세션이 통째로
# 유실된다(실측 재현: 원장 0줄·에러 0줄·종료상태도 파이프에 먹힘). 이 경로는
# 스크립트 자신이 유도한다 — 위 안내가 "세션 종료는 Ctrl+D"라고 사용자를
# 길들여 놓기 때문에 여기서 Ctrl+D가 한 번 더 오기 쉽다.
# 원칙: 입력이 끊겨도 **죽지 말고 아는 만큼 기록한다.** 세션 시간(GPU+사람)은
# 되돌릴 수 없고, 판정은 나중에 원장을 고쳐 채울 수 있다.
printf '판정 (1=성공 2=수정해서 씀 3=버림): '
if ! read -r V; then
  V=""
  echo ""
  echo "입력이 EOF로 끊겼습니다 — 판정을 '미기재'로 두고 나머지는 그대로 기록합니다." >&2
fi
case "$V" in
  1) VERDICT="성공" ;;
  2) VERDICT="수정해서 씀" ;;
  3) VERDICT="버림" ;;
  *) VERDICT="미기재" ;;
esac
printf '사유 한 줄: '
if ! read -r REASON; then
  REASON=""
  echo "" >&2
fi

# 값은 반드시 환경변수로 넘긴다 — 셸 변수를 파이썬 소스에 보간하면 안 된다.
# 이유(실측 확인): 파이썬 삼중따옴표는 백슬래시 이스케이프를 해석하므로
#   diff의  \"  ->  "        (백슬래시 소실)
#   diff의  \n  ->  실제 개행 (줄 구조 파괴)
#   diff의  \t  ->  탭
# 이 되고, diff에 """ 가 들어 있으면 아예 SyntaxError로 세션이 통째로 유실된다.
# 더 나쁜 것은 조용한 쪽이다: 손상된 diff도 유효한 JSON이고 길이가 0이 아니라
# "검증 통과"로 보인다. 그리고 T10의 survival()은 git grep -F 고정 문자열
# 대조라 손상된 줄이 전부 불일치 처리되어 생존율이 체계적으로 과소 계상된다.
# BUILD_DETAIL/TEST_DETAIL(컴파일러 에러 첫 줄)도 같은 위험군이다 — 따옴표나
# 백슬래시가 그대로 들어올 수 있으므로 위와 동일하게 환경변수로만 넘긴다.
if DIFF="$DIFF" REPO="$REPO" TASK_TYPE="$TASK_TYPE" DIFFICULTY="$DIFFICULTY" \
TASK_DESC="$TASK_DESC" TRANSCRIPT="$TRANSCRIPT" VERDICT="$VERDICT" \
REASON="$REASON" SESSION_ID="$SESSION_ID" START_REV="$START_REV" \
END_REV="$END_REV" DURATION="$DURATION" LOCO_EXIT="$LOCO_EXIT" \
BUILD_STATUS="$BUILD_STATUS" BUILD_DETAIL="$BUILD_DETAIL" BUILD_SECS="$BUILD_SECS" \
TEST_STATUS="$TEST_STATUS" TEST_DETAIL="$TEST_DETAIL" TEST_SECS="$TEST_SECS" \
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
    # T9 현장 수선 2: 판정 프롬프트보다 먼저 화면에 보이는 빌드/테스트 확인
    # 결과 — 값은 pass/fail/skipped/timeout 중 하나. *_detail은 fail의 첫 error
    # 줄, skipped/timeout의 사유 문구, pass는 빈 문자열. 판정을 대신하지 않는
    # 증거 필드이며 사람이 여전히 verdict를 정한다.
    "build_status": os.environ["BUILD_STATUS"],
    "build_detail": os.environ["BUILD_DETAIL"],
    "build_secs": int(os.environ["BUILD_SECS"]),
    "test_status": os.environ["TEST_STATUS"],
    "test_detail": os.environ["TEST_DETAIL"],
    "test_secs": int(os.environ["TEST_SECS"]),
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
