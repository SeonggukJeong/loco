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

## 측정의 한계 (해석 시 주의)

1. **채점이 의도적으로 관대하다**: outcome과 무관하게 check를 실행하므로 "작업은 됐지만 finish를 못 부른" 실행도 통과다. qwen의 통과 12건 중 6건이 이 경우(MaxTurns 3, RepetitionStop 3). "Finished만 인정" 기준으로 환산하면 qwen 16.7%(6/36), gemma 11.1%(4/36 — 전부 Finished)로 격차가 3배→1.5배로 줄어든다. 종료 규율을 따로 보려면 outcome 분포를 함께 읽을 것.
2. **행동 기반 판정**: `cargo test` 종료 코드가 유일한 기준 — 수정의 최소성·품질·"진짜 리네임인지(별칭 추가로도 통과 가능)"는 측정하지 않는다.
3. **모델+스캐폴딩 시스템 측정**: 순수 모델 능력이 아니다. 예: edit-crlf-file 통과에는 loco의 EOL 자동 보존(M3)이 기여. 스캐폴딩을 바꾸면 같은 모델도 수치가 변한다 — 그게 이 하네스의 목적이다.
4. **알려진 판정 우회(이론)**: protected가 `tests`/`Cargo.toml`만 커버 — `.cargo/config.toml`에 가짜 러너를 쓰면 대부분 과제가 가짜 통과 가능(M5 수정 예정). 협조적 4B 모델에선 실위험 근접 0이고, 실행별 트랜스크립트로 감사 가능. 이번 통과 건들은 해당 없음 확인.
5. **표본 3회, 신뢰구간 없음**: 과제당 1건 차이가 ±33%p. 시드 3개(0-2) 고정이라 재현은 되지만 분산 추정은 안 된다. 모델 간 비교는 전체 통과율(n=36)로만.
6. **환경 의존**: report.json이 유효 config(`max_output_tokens=4096` 등 로컬 오버라이드)를 스냅샷하지 않는다(M5 항목) — 다른 머신·설정과의 수치 비교는 이 문서의 측정 조건을 수동 대조할 것. `command_timeout_secs`(60s)는 `--timeout-scale`의 영향을 받지 않는다(스펙 의도).
7. **판정기 자체의 협소함**: find-definition은 정답 형식이 좁고(후행 슬래시·따옴표 불인정), fix-off-by-one의 zero 테스트는 버그 상태에서도 통과하는 비변별 케이스(3중 2개만 변별). 과제 개편은 기준선 무효화를 수반하므로 M5에서 일괄 검토.

## 각주: 측정 전 픽스처 수정 (e7b2f1b)

최초 스모크 2회가 fix-failing-test에서 LM Studio 400(n_keep > context length)으로 중단됐다. 원인: `cargo test`가 만든 `target/`을 인자 없는 `list_files`가 나열(픽스처에 `.gitignore` 부재) → 14KB 툴 결과가 마지막 메시지가 되어 `pack()` 축소 불가 → 프롬프트+max_tokens가 ctx 초과. 12개 픽스처에 `/target` `.gitignore`를 추가해 해결(실제 크레이트와 동일한 형태 — 측정 타당성 개선). 이 수정 이전의 부분 실행 기록은 `.loco/eval/20260711T{161615,161826,163141}Z`에 남아 있으며 기준선에 포함하지 않는다.

## M5 경과 (배치별 qwen 측정)

측정 조건은 기준선과 동일(qwen3-vl-4b 단독, ctx 8192, `max_output_tokens=4096`, seed 0, `--repeats 3`). 지표는 `run-*.jsonl` grep 집계(missing field / `exit code:` 발생 수 / search block not found)와 report.json의 outcome 분포·거짓 성공 finish(passed=false & finished) 수.

| 배치 | 통과 | missing field | run_command 실행 | not found | 거짓 성공 finish | outcome 분포 | 판정 |
|---|---|---|---|---|---|---|---|
| 기준선 (20260711T235558Z) | 12/36 | 80 | 12 | 73 | 6 | F12/M17/R7 | — |
| Batch 1 (20260712T054353Z) | 15/36 | 27 | 110 | 76 | 3 | F14/M8/R14 | keep |

**Batch 1 판정 근거 (keep):** 통과 +3런(스펙 §3의 -2런 기준 반대 방향), 타깃 지표 전부 개선 — missing field 80→27(스키마 에코·salvage 효과), run_command 실행 12→110(검증 규칙 효과), 거짓 성공 finish 6→3. salvage 노트 발동 10회(신규 메커니즘 작동 확인). 관찰: fix-compile-error 0/3→3/3(공통 0% 6종 중 첫 탈출, 스펙 §2 성공 기준 1의 절반); 안정 4종(create-module·edit-crlf-file·find-definition·fix-off-by-one) 전부 3/3 유지; not found는 76으로 정체(Batch 2 대상); RepetitionStop 7→14 — MaxTurns 소진(17→8) 대신 기존 5회 동일 반복 정지가 더 일찍 걸리는 쪽으로 이동(multiline-string-edit·rename-function이 9~12턴 조기 종료), Batch 3의 (호출,결과) 윈도 개편이 이 루프들의 교정 대상.
