# loco 실험 프로토콜 (M10 §7-4)

GPU 시간(측정 배치)을 쓰는 모든 실험에 적용된다.

1. **사전등록 없이는 배치를 돌리지 않는다.** 사전등록 = 가설·조건(암)·표본·
   지표·판정 규칙·중단 규칙·시간 예산이 담긴 `pre-registration.md`가 사용자
   승인을 받은 상태. 판정 규칙은 데이터를 보기 전에 확정한다.
2. **측정 중 cargo build/test 병행 금지**(CPU 경합이 타이밍 판정을 흔든다 —
   CLAUDE.md). 암 전환에 필요한 체크아웃·빌드는 배치 사이에만.
3. **소표본 규칙**(M9 스펙 §2 승계): 관심 현상 발생 런이 배치당 3런 미만이면
   비율 대신 발생 런 전수를 나열하고 방향으로 판정한다.
4. **배치 전 게이트**: ① 세 tasks 트리 `--verify` 통과
   (`tasks` 12/12 · `tasks-large` 3/3 · `tasks-real` N/N).
   ⚠ **M15 이후** — `tasks-real`은 조달이 만든 상태에 의존한다(스펙 §3-7).
   `--verify` 자체가 네트워크를 타지는 않지만(조달은 `scripts/procure_real.sh`로
   분리된 명시 단계다) **픽스처가 없으면 명확히 실패한다**. N은 그 배치의
   사전등록이 동결한 과제 수다
   ② 모델 서버 기동(**로그 리다이렉션 포함** — 구체 캡처 경로는 항목 5가
   가리키는 각 배치의 사전등록 문서가 못박는다):
   `LOCO_MODEL_GGUF=<gguf> LOCO_CTX=<ctx> scripts/serve.sh > <로그 경로> 2>&1`
   (이전 서버가 떠 있으면 먼저 내린다 — `pkill -f llama-server`)
   ③ 배치 전 스모크 (전건 통과해야 배치를 시작한다):
   - json_schema 요청 1건이 **HTTP 200** — 실패하면 배치를 시작하지 말 것.
     이 검사가 M12→M13 전환에서 발견된 조용한 전면 실패(스펙 §3-3-1)를 막는다
   - 서버 기동 로그의 `n_ctx_slot` == **사전등록에 동결된 서버 로드 ctx**
     (동결값은 그 배치의 **실효 운용점** 이상이어야 한다)
     ⚠ **M15 이후 형태다.** M15 이전 배치(M13 앵커·M14 게이트)는 원문
     *"config의 `context_tokens`"*로 읽으며, 둘 다 `n_ctx_slot == 8192 ==
     context_tokens`라 어느 형태로도 통과하므로 **소급 재해석되지 않는다**.
     ⚠ 하한이 "config의 `context_tokens`"가 아니라 **실효 운용점**인 이유:
     M15 배치 중 전역 config는 8192이고 `tasks-real` 과제만 `TaskSpec.context_tokens`로
     32768을 덮는다 — 원문 그대로 읽으면 하한이 8192가 되어 무의미하다.
     ⚠ **단순 `>=`로 바꾸면 안 된다**: 스모크 서버를 안 내린 채 배치를 시작하면
     `n_ctx_slot=40960 >= 32768`로 통과해, 아래 *"직전 배치 잔재는 GPU 시간
     전체를 무효화"*가 경고하는 실패 모드가 열린다. **동결값 등호는 과잉
     제약(운용점 = 로드 강제)만 없애고 축소 탐지·잔재 탐지·자증을 전부 보존한다**
   - `curl -s localhost:<port>/v1/models` 의 `data[0].id` == `--alias` 값
   - `.loco/config.toml` 이 이번 배치 조건인지 (직전 배치 잔재는 GPU 시간 전체를 무효화)
   - `ls ${TMPDIR}/.cargo` — 존재하면 수동 제거
   ④ 데몬화: macOS에 `setsid`가 없다. **M15 이후** — fork-then-setsid 형태를 쓴다.
   이전 형태(`os.setsid()`를 배경 프로세스에서 바로 호출)는 **대화형 셸에서
   `PermissionError`로 즉사한다** — 배경 프로세스가 이미 프로세스 그룹 리더라
   `setsid()`가 EPERM이기 때문이다. M13·M14 배치가 구 레시피로 돌았다면 그것은
   비대화형 경로였다는 뜻이며, 소급 재해석하지 않는다:
   ```bash
   python3 -c "
   import os,sys
   if os.fork(): os._exit(0)
   os.setsid(); os.execvp(sys.argv[1], sys.argv[1:])
   " <cmd>...
   ```

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
   대체 증거다), ⚠ **M15 이후: `n_ctx_slot`이 증언하는 것은 "동결된 서버
   로드"이지 "실효 운용점"이 아니다.** 분기 2를 타면 `n_ctx_slot ≠ context_tokens`가
   되고(스펙 §8 각주 6), 분기 1에서도 4③은 **동결값 ≥ 실효 운용점**만 요구하므로
   여전히 상한이다. **실효 운용점의 실제 증인은 `RunRecord.effective_context_tokens`
   (M15 H9)이며, 배치 리포트는 그 값을 함께 적어야 한다** — `n_ctx_slot` 단독으로는
   "어떤 컨텍스트로 돌았는가"에 답하지 못한다 (M15 §6-4-17), 사용한 config 값을 report.md에 기재. **서버 기동 로그
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
