#!/bin/sh
# loco 측정·배포용 llama-server 기동 — 조건을 핀으로 고정한다 (M13 스펙 §3-3).
# 이 스크립트가 곧 배포 산출물이자 실험 조건 기록이다. 값을 바꾸면 그 배치는
# 이전 배치와 비교 불가능해진다 — 반드시 사전등록 문서에 반영할 것.
#
# 사용법:
#   LOCO_MODEL_GGUF=/path/to/model.gguf scripts/serve.sh
#   LOCO_CTX=32768 LOCO_MODEL_GGUF=... scripts/serve.sh
set -eu

: "${LOCO_MODEL_GGUF:?LOCO_MODEL_GGUF (GGUF 경로)를 지정하세요}"
LOCO_CTX="${LOCO_CTX:-8192}"
LOCO_PORT="${LOCO_PORT:-8080}"
LOCO_ALIAS="${LOCO_ALIAS:-ornith}"
LLAMA_SERVER="${LLAMA_SERVER:-llama-server}"

# --- 핀 1: -np 1 -------------------------------------------------------------
# llama.cpp에서 -c는 병렬 슬롯이 나눠 쓰는 총량이다. -np 1로 n_ctx_slot == -c 를
# 결정론적으로 만든다. (기본 -np -1(auto)은 n_slots=4이지만 kv_unified=true라
# 분할하지 않는다 — 그 동작에 의존하지 않는다. 진짜 함정은 반대 방향이다:
# "안전하게" -np 4를 주면 슬롯당 컨텍스트가 1/4로 조용히 줄어든다.)
#
# --- 핀 2: 샘플러 4종 --------------------------------------------------------
# loco는 temperature만 보내고 top-k/top-p/min-p/repeat-penalty는 안 보낸다
# (src/llm/types.rs:22-35). 서버 기본값에 좌우되므로 명시 고정한다.
# 조사 근거(Step 1 실측, llama-server 9960/a935fbffe, 2026-07-19,
# `llama-server --help`의 실효 기본값 그대로 인용):
#   --top-k 40           (default: 40, 0 = disabled)
#   --top-p 0.95          (default: 0.95, 1.0 = disabled)
#   --min-p 0.05          (default: 0.05, 0.0 = disabled)
#   --repeat-penalty 1.00 (default: 1.00, 1.0 = disabled)
# 위 4개는 아래 실제 인자값과 전부 일치 — 실측이 이겼고 바꿀 것이 없었다.
# LM Studio 쪽 실효 기본값은 확보하지 않았다("확보 실패"): LM Studio는 더 이상
# 대상 스택이 아니고 CLI로 샘플러 기본값을 조회할 방법도 없다 — 스펙 §3-3의
# 목적은 LM Studio를 흉내 내는 것이 아니라 "우리가 무엇을 돌리는지 아는 것"이므로
# llama-server 쪽 실측만으로 충분하다.
#
# --- 핀 3: reasoning 처리 = 기본값(auto, reasoning_content로 분리) -----------
# --reasoning-format none 을 절대 쓰지 말 것: response_format: json_schema 와
# 병용 시 b9960에서 전 요청이 400 "Failed to initialize samplers: std::exception"
# 이 된다. 본문에 "context"가 없어 오버플로 감지를 비켜가고 json_schema 폴백이
# 영구 발동해, 배치는 정상 종료하면서 매 턴 파싱이 실패한다(M13 스펙 §3-3-1).
# 주의: 이 핀은 사고 토큰의 예산 잠식을 해결하지 않는다. --reasoning-format은
# 토큰을 어디에 "보고"할지만 정하고 생성을 막지 않는다. 실효 레버는
# .loco/config.toml 의 max_output_tokens 상향뿐이다(스펙 §3-2).
#
# --- 핀 4: --alias -----------------------------------------------------------
# alias가 없으면 /v1/models 의 id가 GGUF 전체 경로가 되고, 그 문자열이
# report.json 최상위 model 필드에 그대로 박힌다.
exec "$LLAMA_SERVER" \
  -m "$LOCO_MODEL_GGUF" \
  -c "$LOCO_CTX" \
  -np 1 \
  --alias "$LOCO_ALIAS" \
  --host 127.0.0.1 \
  --port "$LOCO_PORT" \
  --top-k 40 \
  --top-p 0.95 \
  --min-p 0.05 \
  --repeat-penalty 1.0
