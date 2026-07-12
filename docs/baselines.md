# M4 기준선 측정 결과 (4B급 소형 모델)

측정일: 2026-07-12 (UTC 스탬프 기준 2026-07-11). 스캐폴딩 개선(M5+)의 효과는 이 수치 대비로 판단한다.

## 측정 조건

- 하네스: feat/m4-eval `e7b2f1b` (픽스처 `.gitignore` 수정 포함 — 아래 각주)
- 커맨드: `./target/release/loco eval tasks --repeats 3` (시드 0부터, 반복 i의 시드 = i, timeout×1)
- 서버: LM Studio, 컨텍스트 길이 8192로 로드; 로컬 `./.loco/config.toml`에 `max_output_tokens = 4096` (그 외 기본값: max_turns=25, context_tokens=8192, command_timeout_secs=60)
- 판정: 샌드박스에서 실행 후 protected(tests/Cargo.toml) 원복 → `cargo test` 종료 코드

## 전체 통과율

| 모델 | 통과율 | 통과/전체 | report.json |
|---|---|---|---|
| google/gemma-4-e4b | **11.1%** | 4/36 | `.loco/eval/20260711T165322Z/report.json` |
| qwen/qwen3-vl-4b | **33.3%** | 12/36 | `.loco/eval/20260711T235558Z/report.json` |

## 과제별 (통과 / 평균 턴 / 평균 시간)

| 과제 | gemma-4-e4b | qwen3-vl-4b |
|---|---|---|
| add-function | 0/3 · 17.7턴 · 58.4s | 0/3 · 19.7턴 · 64.6s |
| chain-edits | 0/3 · 14.0턴 · 64.7s | 0/3 · 25.0턴 · 34.3s |
| count-usages | 0/3 · 4.7턴 · 15.0s | 0/3 · 10.0턴 · 9.5s |
| create-module | 1/3 · 13.7턴 · 34.0s | **3/3** · 25.0턴 · 30.6s |
| edit-crlf-file | 0/3 · 6.7턴 · 16.6s | **3/3** · 6.0턴 · 5.9s |
| find-definition | 0/3 · 6.7턴 · 18.8s | **3/3** · 9.3턴 · 9.0s |
| fix-compile-error | 0/3 · 12.3턴 · 30.8s | 0/3 · 8.3턴 · 7.1s |
| fix-failing-test | 1/3 · 20.3턴 · 79.3s | 0/3 · 25.0턴 · 55.2s |
| fix-off-by-one | **2/3** · 7.3턴 · 21.6s | **3/3** · 9.0턴 · 12.5s |
| implement-from-doc | 0/3 · 12.3턴 · 50.1s | 0/3 · 13.7턴 · 63.8s |
| multiline-string-edit | 0/3 · 20.0턴 · 90.8s | 0/3 · 25.0턴 · 60.6s |
| rename-function | 0/3 · 18.0턴 · 85.5s | 0/3 · 21.3턴 · 25.4s |

## outcome 분포 (36회 실행)

| 모델 | Finished | MaxTurns | RepetitionStop | Timeout |
|---|---|---|---|---|
| gemma-4-e4b | 23 | 7 | 6 | 0 |
| qwen3-vl-4b | 12 | 17 | 7 | 0 |

## 관찰

- **qwen은 이봉(bimodal)**: 4개 과제를 3/3으로 안정 통과, 나머지 8개는 0/3 — 과제별 능력 경계가 뚜렷하다. gemma는 분산형(1/3·2/3 혼재).
- **"통과인데 MaxTurns/RepetitionStop"**(qwen의 create-module 3건, edit-crlf-file 3건): 작업은 끝냈지만 `finish`를 부르지 못하고 소진된 경우 — "outcome과 무관하게 check 실행" 설계 결정이 없었다면 전부 오판됐을 것.
- **자신 있는 오답**: count-usages는 양쪽 모두 0/6 — 짧게 Finished하고 틀린 답 저장(정밀 카운팅 최약점). gemma는 find-definition·edit-crlf-file에서도 같은 패턴.
- **공통 난공불락**: add-function, chain-edits, implement-from-doc, multiline-string-edit, rename-function, fix-compile-error — 다중 편집·이스케이프·지구력 계열. 스캐폴딩 개선(M5)의 1차 타깃.
- gemma가 유일하게 앞선 과제는 fix-failing-test(1/3 vs 0/3).
- Timeout 0 — 과제 기본 타임아웃(300s)은 현 환경에서 여유.

## 각주: 측정 전 픽스처 수정 (e7b2f1b)

최초 스모크 2회가 fix-failing-test에서 LM Studio 400(n_keep > context length)으로 중단됐다. 원인: `cargo test`가 만든 `target/`을 인자 없는 `list_files`가 나열(픽스처에 `.gitignore` 부재) → 14KB 툴 결과가 마지막 메시지가 되어 `pack()` 축소 불가 → 프롬프트+max_tokens가 ctx 초과. 12개 픽스처에 `/target` `.gitignore`를 추가해 해결(실제 크레이트와 동일한 형태 — 측정 타당성 개선). 이 수정 이전의 부분 실행 기록은 `.loco/eval/20260711T{161615,161826,163141}Z`에 남아 있으며 기준선에 포함하지 않는다.
