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
| Batch 2 (20260712T063028Z) | 19/36 | 10 | 135 | 32 | 1 | F15/M7/R13/T1 | keep |

**Batch 2 판정 근거 (keep):** 통과 +4런(15→19). not found 76→32(§6.2 최근접 인용 32회 발동 — Batch 2 타깃 적중), missing field 27→10, 거짓 성공 finish 3→1. replace_all 실사용 3회(치환 발생; 트랜스크립트 "replace_all" 문자열 98회는 대부분 시스템 프롬프트 doc() 에코라 원자료로만 기록). 과제별: chain-edits 0/3→2/3, rename-function 0/3→1/3 — fix-compile-error(Batch 1)와 합쳐 공통 0% 6종 중 3종 탈출로 스펙 §2 성공 기준 1(≥2종) 조기 달성. count-usages 0/3→1/3(qwen 첫 통과). 관찰: multiline-string-edit 시드 2에서 첫 Timeout 1건(기준선·Batch 1은 Timeout 0) — 단발이라 지표 악화로 보지 않음, Batch 3 측정에서 재관찰. RepetitionStop 14→13, MaxTurns 8→7로 정체 — 루프 계열은 Batch 3 대상.
| Batch 3 (20260712T103250Z) | 18/36 | 50 | 100 | 50 | 2 | F16/M8/R12 | keep |

**Batch 3 판정 근거 (keep, 스펙 §3 ±1런 규칙):** 통과 -1런(19→18)은 keep 범위. 신규 메커니즘 발동 확인 — 검증 넛지 3회, 전략 교정 5회, Timeout 재발 0(Batch 2의 1건 소멸). count-usages 1/3→2/3. 악화 항목의 원인: missing field 10→50은 특정 런 편중(create-module 시드1 단독 12건 — 해당 과제는 3/3 통과, 통과 무관 원자료 노이즈), not found 32→50도 유사 분산. chain-edits 2/3→0/3이 -1런의 실체 — 시드0이 새 (호출,결과) 윈도의 RepetitionStop에 12턴에서 걸림(시드1·2는 MaxTurns 25턴 소진, Batch 2에서도 실패하던 시드 구성과 변동 혼재). 시드 단위 분산과 신규 정지의 구분은 Task 17 최종 측정에서 재관찰.

## M5 최종 결과 (2026-07-12)

측정 조건은 기준선과 동일(각 모델 단독 로드 ctx 8192, `max_output_tokens=4096`, seed 0, `--repeats 3`), 하네스는 feat/m5-scaffolding `70a9f1a`(구현 완료 시점 — qwen 최종치는 Batch 3 측정을 재사용, 이후 코드 무변경).

### 전체 통과율 (기준선 대비)

| 모델 | 기준선 | M5 최종 | report.json |
|---|---|---|---|
| google/gemma-4-e4b | 11.1% (4/36) | **66.7% (24/36)** | `.loco/eval/20260712T110845Z/report.json` |
| qwen/qwen3-vl-4b | 33.3% (12/36) | **50.0% (18/36)** | `.loco/eval/20260712T103250Z/report.json` |

### 과제별 (통과 / 평균 턴 / 평균 시간)

| 과제 | gemma-4-e4b | qwen3-vl-4b |
|---|---|---|
| add-function | **2/3** · 13.7턴 · 82.3s | 0/3 · 15.0턴 · 81.0s |
| chain-edits | **3/3** · 11.0턴 · 44.2s | 0/3 · 20.7턴 · 53.7s |
| count-usages | 1/3 · 4.3턴 · 10.7s | **2/3** · 11.7턴 · 22.6s |
| create-module | 2/3 · 14.7턴 · 30.7s | **3/3** · 13.7턴 · 17.6s |
| edit-crlf-file | **3/3** · 5.7턴 · 13.5s | **3/3** · 9.0턴 · 11.1s |
| find-definition | 0/3 · 3.0턴 · 5.7s | **3/3** · 5.3턴 · 5.7s |
| fix-compile-error | **3/3** · 14.7턴 · 77.5s | **3/3** · 8.7턴 · 54.3s |
| fix-failing-test | **3/3** · 12.7턴 · 74.9s | 0/3 · 17.3턴 · 139.3s |
| fix-off-by-one | **3/3** · 9.3턴 · 37.3s | **3/3** · 6.3턴 · 10.3s |
| implement-from-doc | 1/3 · 16.7턴 · 95.7s | 0/3 · 24.0턴 · 176.9s |
| multiline-string-edit | 0/3 · 25.0턴 · 158.8s | 0/3 · 15.3턴 · 43.5s |
| rename-function | **3/3** · 11.3턴 · 43.3s | 1/3 · 15.3턴 · 31.2s |

### outcome 분포 (36회 실행)

| 모델 | Finished | MaxTurns | RepetitionStop | Timeout | 거짓 성공 finish |
|---|---|---|---|---|---|
| gemma-4-e4b (기준선) | 23 | 7 | 6 | 0 | — |
| gemma-4-e4b (M5) | 30 | 5 | 1 | 0 | 6 |
| qwen3-vl-4b (기준선) | 12 | 17 | 7 | 0 | 6 |
| qwen3-vl-4b (M5) | 16 | 8 | 12 | 0 | 2 |

### 성공 기준 판정 (M5 스펙 §2 — 3항 모두 충족)

1. **공통 0% 6종 중 ≥2종 탈출: ✓ (5종)** — add-function(gemma 2/3), chain-edits(gemma 3/3; qwen은 Batch 2에서 2/3 후 Batch 3에서 0/3 — 시드 분산), implement-from-doc(gemma 1/3), rename-function(gemma 3/3, qwen 1/3), fix-compile-error(양쪽 3/3). 잔존은 multiline-string-edit 1종(양쪽 0/3 — 이스케이프 심층, 후속 마일스톤 타깃).
2. **qwen 안정 4종 각각 ≥2/3: ✓** — create-module·edit-crlf-file·find-definition·fix-off-by-one 전부 3/3.
3. **모델별 전체 통과율 ≥ 기준선: ✓** — gemma 66.7% ≥ 11.1% (+55.6%p, 6.0배), qwen 50.0% ≥ 33.3% (+16.7%p).

### 관찰

- **gemma의 도약은 salvage 파싱이 주도**: gemma 트랜스크립트에서 salvage 노트 285회 발동(qwen 52회) — 기준선 gemma의 지배적 실패였던 "args 밖 필드" 형태 오류가 파싱 계층에서 구제되며 Finished 23→30, RepetitionStop 6→1로 이동. 모델 순위가 역전됐다(기준선 gemma:qwen = 1:3 → M5 4:3).
- **qwen -1런(Batch 2 19→Batch 3 18)은 keep 범위의 시드 분산**: chain-edits 시드0이 새 반복 윈도 정지에 12턴에서 걸린 것이 유일한 배치 간 후퇴. 나머지 지표는 전 배치 개선 유지(missing field 80→50, not found 73→50, 거짓 성공 6→2).
- **gemma 거짓 성공 finish 6건 잔존**: find-definition 0/3(평균 3턴 — 짧고 자신 있는 오답), count-usages 계열. 기준선의 "자신 있는 오답" 패턴은 스캐폴딩으로 안 잡힌다 — 판정기 변별력(기준선 한계 7항)과 함께 후속 과제.
- **검증 넛지·전략 교정은 저빈도 발동**(gemma 1·5회, qwen 3·5회) — 발동 시 finish 전 cargo test 실행을 유도한 트랜스크립트 확인. run_command 실행 수는 기준선 12(qwen)→100+로, 대부분 프롬프트의 검증 규칙 효과.
- Timeout 0 유지(Batch 2의 1건은 재발 없음).

### 잔여 한계 (기준선 "측정의 한계" 승계 + M5 추가)

- 기준선 한계 1~3·5·7항은 그대로 유효. 4항(.cargo 판정 우회)은 M5에서 샌드박스 내부 벡터가 차단됐다(암묵 protected + 상위 경로 트립와이어) — 단 `$CARGO_HOME`/홈 디렉터리 벡터는 미차단 잔여 한계, 그리고 트립와이어는 temp_dir 상위에 `.cargo`가 우연히 존재하는 환경에서 하네스를 중단시킨다(감지만 하고 정리하지 않음 — 에러 메시지의 경로를 수동 제거).
- 6항(config 미스냅샷)은 해소 — report.json이 `effective_config`(base_url·temperature·context_tokens·max_output_tokens·max_turns·command_timeout_secs·loco_version)를 기록한다.
