# loco 실험 프로토콜 (M10 §7-4)

GPU 시간(측정 배치)을 쓰는 모든 실험에 적용된다.

1. **사전등록 없이는 배치를 돌리지 않는다.** 사전등록 = 가설·조건(암)·표본·
   지표·판정 규칙·중단 규칙·시간 예산이 담긴 `pre-registration.md`가 사용자
   승인을 받은 상태. 판정 규칙은 데이터를 보기 전에 확정한다.
2. **측정 중 cargo build/test 병행 금지**(CPU 경합이 타이밍 판정을 흔든다 —
   CLAUDE.md). 암 전환에 필요한 체크아웃·빌드는 배치 사이에만.
3. **소표본 규칙**(M9 스펙 §2 승계): 관심 현상 발생 런이 배치당 3런 미만이면
   비율 대신 발생 런 전수를 나열하고 방향으로 판정한다.
4. **배치 전 게이트**: ① 두 tasks 트리 `--verify` 통과(12/12·3/3)
   ② 모델 서버 기동(**로그 리다이렉션 포함** — 구체 캡처 경로는 항목 5가
   가리키는 각 배치의 사전등록 문서가 못박는다):
   `LOCO_MODEL_GGUF=<gguf> LOCO_CTX=<ctx> scripts/serve.sh > <로그 경로> 2>&1`
   (이전 서버가 떠 있으면 먼저 내린다 — `pkill -f llama-server`)
   ③ 배치 전 스모크 (전건 통과해야 배치를 시작한다):
   - json_schema 요청 1건이 **HTTP 200** — 실패하면 배치를 시작하지 말 것.
     이 검사가 M12→M13 전환에서 발견된 조용한 전면 실패(스펙 §3-3-1)를 막는다
   - 서버 기동 로그의 `n_ctx_slot` == config의 `context_tokens`
   - `curl -s localhost:<port>/v1/models` 의 `data[0].id` == `--alias` 값
   - `.loco/config.toml` 이 이번 배치 조건인지 (직전 배치 잔재는 GPU 시간 전체를 무효화)
   - `ls ${TMPDIR}/.cargo` — 존재하면 수동 제거
   ④ 데몬화: macOS에 `setsid`가 없다.
   `python3 -c "import os,sys; os.setsid(); os.execvp(sys.argv[1], sys.argv[1:])" <cmd>...`

   배치 전 json_schema 스모크 구체 명령:
   ```bash
   curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/v1/chat/completions \
     -H 'Content-Type: application/json' -d '{
     "model":"ornith","messages":[{"role":"user","content":"hi"}],
     "temperature":0.1,"max_tokens":64,"stream":false,
     "response_format":{"type":"json_schema","json_schema":{"name":"agent_turn","schema":{
     "type":"object","properties":{"thought":{"type":"string"},
     "action":{"type":"object","properties":{"tool":{"type":"string","enum":["finish"]},
     "args":{"type":"object"}},"required":["tool","args"]}},
     "required":["thought","action"]}}}}'
   ```
   Expected: `200`
5. **재현 가능성 기록**: 배치마다 eval 스탬프 ↔ `git rev-parse HEAD` 쌍,
   서버 기동 로그의 `n_ctx_slot`과 `curl /v1/models`의 `data[0].id` 확인
   출력(항목 4 ③ 스모크와 동일 근거 — llama.cpp에는 `lms` 같은 확인 CLI가
   없으므로, 이 두 출력이 "어떤 모델·컨텍스트로 돌았는가"를 증언하는
   대체 증거다), 사용한 config 값을 report.md에 기재. **서버 기동 로그
   자체를 파일로 남기는 구체 캡처 명령(리다이렉션 경로 포함)은 이 프로토콜이
   못박지 않는다 — 각 배치의 사전등록 문서가 등록하고, 여기서는 그 문서를
   가리키기만 한다**(예: `docs/experiments/2026-07-19-llamacpp-anchor/pre-registration.md`
   §2-2). 표준 위치 없이 매 배치가 로그 캡처를 임의로 하면, 배치 후
   `n_ctx_slot` 재확인(항목 4 ③)이 근거 로그 없이 통과 처리될 위험이 있다.
   report.json은 암을 자증하지 못한다(loco_version이 전 브랜치 동일).
6. **중단 규칙 준수**: 사전등록에 적힌 그대로. Ctrl+C 부분 리포트는 폐기하고
   해당 배치 재수행.
7. **판정은 사람이**: 러너는 report.md 초안(지표 표 + 사전등록 판정 규칙의
   기계적 적용)까지만. 최종 판정·병합 결정은 사용자 리뷰를 거친다.
