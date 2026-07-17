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

- 기준선 한계 1~3·5·7항은 그대로 유효. 4항(.cargo 판정 우회)은 M5에서 샌드박스 내부 벡터가 차단됐다(암묵 protected + 상위 경로 트립와이어) — 단 `$CARGO_HOME`/홈 디렉터리 벡터는 미차단 잔여 한계, 그리고 트립와이어는 temp_dir 상위에 `.cargo`가 우연히 존재하는 환경에서 하네스를 중단시킨다(감지만 하고 정리하지 않음 — 에러 메시지의 경로를 수동 제거). (M7: `$CARGO_HOME`/홈·temp_dir 상위 조상 config 벡터는 스냅샷 감지로 승격 — "판정 무결성 갱신 (M7 §5)" 절 참고.)
- 6항(config 미스냅샷)은 해소 — report.json이 `effective_config`(base_url·temperature·context_tokens·max_output_tokens·max_turns·command_timeout_secs·loco_version)를 기록한다.

## v2 기준선 (M6 판정기 개편 후, 2026-07-16 측정)

M6(스펙 2026-07-12-m6-eval-integrity-design.md)이 판정기를 수선했으므로 **이 절의 수치는
v1 계열(기준선·M5)과 직접 비교할 수 없다**. M7+ 마일스톤은 이 절을 비교 기준으로 사용한다.

### 판정기 변경 목록 (v1 → v2)

- find-definition: 정규화 사다리 확장 — 감싼 따옴표쌍(`"` `'` `` ` ``)·후행 슬래시·후행
  마침표 허용 (여러 줄·산문은 계속 거부). 사다리 단위 테스트가 판정 테스트 파일에 동거
- count-usages: trim·따옴표쌍 제거 후 정수 파싱·수치 비교 (산문 거부). 사다리 단위 테스트 동거
- fix-off-by-one: 비변별 `zero` 테스트를 변별 케이스(`two`)로 교체
- 하네스: verify 오버레이·protected 복원을 read+write로 — macOS `fs::copy`의 mtime
  보존이 스테일 테스트 바이너리 판정을 유발하는 벡터 수선 (판정기 자체는 아니나
  판정 경로 무결성 수정)
- Task 3 솔루션 감사(전 12과제 `--verify` 12/12, ✗ 0건): 솔루션 저작 오류 **0건**
- Task 4 전수 변별성 감사(기지 3건 외): 추가 교체 **0건**. fix-failing-test의
  `max_single`·`max_empty`는 버그 함수(`max_csv`, min↔max 오류)를 겨냥하지만 단일·빈
  입력이라 구조적으로 그 버그를 변별할 수 없음 — 정당한 엣지(None 초기화·빈 목록)
  커버리지로 유지, 버그는 `max_of_list`가 변별한다(교체 불필요)

### 측정 조건

기준선과 동일: 모델 단독 로드 ctx 8192, 로컬 config `max_output_tokens = 4096`, seed 0,
`--repeats 3`, 측정 중 병행 빌드 금지. 하네스: 커밋 `4cb7325`(브랜치 m6-eval-integrity,
측정 내내 워킹트리 클린 — 세 모델 동일 바이너리). `effective_config`: ctx 8192,
max_output_tokens 4096, max_turns 25, temperature 0.1, loco 0.1.0. report.json(로컬,
`.loco/eval`는 git-ignored): gemma `20260716T051407Z`, qwen `20260716T055019Z`,
ornith `20260716T062853Z`.

### 전체 통과율

| 모델 | 통과 | 엄격(Finished∧통과) | 거짓 성공 finish |
|---|---|---|---|
| google/gemma-4-e4b | 72.2% (26/36) | 69.4% (25/36) | 4 |
| qwen/qwen3-vl-4b | 50.0% (18/36) | 33.3% (12/36) | 3 |

### 과제별 (통과, 엄격이 다르면 괄호)

| 과제 | gemma | qwen | ornith*(측정 당시 탐색) |
|---|---|---|---|
| add-function | 3/3 | 0/3 | 3/3 |
| chain-edits | 3/3 | 3/3 | 3/3 |
| count-usages | 0/3 | 0/3 | 3/3 |
| create-module | 1/3 | 3/3(엄격 0/3) | 3/3 |
| edit-crlf-file | 3/3 | 3/3 | 3/3 |
| find-definition | 3/3 | 3/3 | 3/3 |
| fix-compile-error | 3/3(엄격 2/3) | 3/3(엄격 1/3) | 3/3 |
| fix-failing-test | 3/3 | 0/3 | 3/3 |
| fix-off-by-one | 2/3 | 2/3 | 3/3 |
| implement-from-doc | 2/3 | 0/3 | 2/3 |
| multiline-string-edit | 0/3 | 0/3 | 3/3 |
| rename-function | 3/3 | 1/3(엄격 0/3) | 2/3 |

### 관찰

- **엄격 vs 관대 격차 = 종료 규율 지표(M6 §5 이중 리포트가 노린 신호)**: gemma 2.8pp(69.4 vs
  72.2), **qwen 16.7pp**(33.3 vs 50.0), ornith 0pp. qwen은 `RepetitionStop` 14 / `Finished` 15로,
  관대>엄격 6건(create-module 3/3→엄격0, fix-compile-error 3/3→1, rename-function 1/3→0)이 전부
  "과제는 풀었으나 깔끔히 `finish` 못 하고 반복 정지로 끝나 `check`가 뒤늦게 통과"한 케이스다.
- **count-usages: 양 4B 모델 공통 0/3 + 거짓 성공 finish**(gemma 3·qwen 3). 정규화 사다리가
  형식은 받아주지만 두 모델 모두 개수 자체를 자신 있게 틀린다 — 판정기 협소가 아니라 모델 한계
  (v1의 "자신 있는 오답" 패턴이 사다리로도 안 잡힘을 재확인). 반면 v1의 또 다른 형식-거짓성공
  후보였던 **find-definition은 양 모델 3/3(엄격)** — 사다리가 형식 변형을 흡수했다.
- **multiline-string-edit: 양 4B 0/3**(M5 "마지막 공통 0%" 지속 — 이스케이프 심층, gemma 평균 25턴·
  201s의 length 루프). M7+ 잔여 난관.
- 모델별 강점 분리: gemma는 add-function·fix-failing-test에서 3/3인데 qwen은 0/3, 반대로
  create-module은 qwen이 관대 3/3(엄격 0). 4B 두 모델의 프로필이 다르다.

### 9B 측정 (측정 당시 탐색 — M7에서 기준선 승격)

프로젝트 방향 전환(README 2026-07-13 — "저사양 하드웨어에서 대형 모델 구동")의 첫 실측. **측정
당시에는 기준선이 아니었다**: 모델 세트(gemma·qwen 4B) 밖이고, 4B→9B 크기 + 파인튜닝 차이가 섞여
위 표와 대등 비교가 아니라 "같은 하네스에서 더 큰 같은-계열(Qwen3) 모델이 어떻게 하나"의
탐색이었다. **M7 세트 재편으로 기준선에 승격** — 경위는 아래 "모델 세트 재편 (M7)" 절 참고.

| 모델 | 통과 | 엄격 | 거짓 성공 finish | 총 소요(평균/런) |
|---|---|---|---|---|
| ornith-1.0-9b (Qwen3 계열 9B, GGUF Q4_K_M) | 94.4% (34/36) | 94.4% (34/36) | 0 | 40.4분 (67.3s) |

- **엄격 = 관대**(전 통과 34건이 `Finished`, `RepetitionStop`은 실패한 2런뿐) — qwen의 종료 규율
  약점이 9B에서 해소. 실패는 implement-from-doc 2/3·rename-function 2/3 둘뿐.
- **4B 공통 난관 둘 다 돌파**: count-usages 3/3(양 4B 0/3), multiline-string-edit 3/3(양 4B 0/3).
- **속도**: 총 40.4분(평균 67.3s/런)으로 4B와 대등 — 9B 지연 우려는 이 과제·이 하드웨어에선
  미발생(방향 전환의 청신호). 단 chain-edits 132s·multiline 125s·rename 130s처럼 긴 과제도 있음.
- M7+ 검토: qwen3-vl-4b 은퇴 + Ornith를 "Qwen3 계열 대표"로 이관 후보(계열은 같아도 크기·변종이
  달라 동일 모델 대체는 아님 — 의도적 세트 재편으로 기록).

## 모델 세트 재편 (M7, 2026-07-16)

기준선 모델 세트를 **google/gemma-4-e4b(4B 대표) + ornith-1.0-9b(Qwen3 계열 대표)**로
재편한다 (M7 스펙 §3).

- **qwen3-vl-4b 은퇴**: 같은 계열(Qwen3) 9B가 4B급 속도로 대폭 상회(50.0%→94.4%)하고
  종료 규율 약점(엄격 격차 16.7pp)도 9B에서 해소(0pp)됐다. v2 수치는 위 절에 역사
  기록으로 유지하며, 이후 마일스톤의 측정 대상에서 제외한다.
- **ornith-1.0-9b 승격**: 탐색 측정(`20260716T062853Z`, 하네스 `4cb7325`)을 기준선으로
  **재지정** — v2 기준선과 동일 프로토콜·동일 하네스로 이미 측정됐고 이후 main은 문서
  커밋뿐이라 재측정하지 않는다. 한계: report.json은 하네스 커밋을 자증하지 못한다
  (`loco_version`은 전 커밋 0.1.0) — 커밋 해시 스냅샷은 백로그.
- **대체가 아닌 재편**: 계열은 같아도 크기(4B→9B)·변종(vl vs Ornith 파인튜닝)이 달라
  동일 모델 대체가 아니다 — qwen→ornith 수치를 시계열로 잇는 해석을 금지한다.
- **속도 병기** (M7 §4): 이후 모델 비교는 평균 s/런을 1급 정보로 병기한다.

| 모델 | 통과 | 엄격 | 평균 s/런 | 세트 지위 |
|---|---|---|---|---|
| google/gemma-4-e4b | 72.2% | 69.4% | 52.3s | 기준선 (4B 대표) |
| qwen/qwen3-vl-4b | 50.0% | 33.3% | 59.7s | **은퇴 (M7)** |
| ornith-1.0-9b | 94.4% | 94.4% | 67.3s | **기준선 (M7 승격, Qwen3 계열 대표)** |

### 판정 무결성 갱신 (M7 §5)

- `$CARGO_HOME`/홈 config와 temp_dir **상위 조상**의 `.cargo/config*` 변조는 이제
  스냅샷 감지(하네스 시작 1회 기록 → 매 런 check 전 비교, 상태 전이=중단)로 잡는다
  (`src/eval/integrity.rs`). 측정 중 사용자가 해당 config를 직접 편집해도 중단된다
  (오탐 수용 — 병행 작업 금지 프로토콜과 일관).
- 잔여(백로그): 시작 전 사전 오염 config(CARGO_HOME 격리가 닫을 대상), cargo
  **바이너리 교체**(`$CARGO_HOME/bin`·`~/.rustup` — PATH 고정/절대경로/해시 계열).

## M8 측정 조건·ornith 실측 사양표 (2026-07-17)

> **주의(2026-07-17 리워드)**: 아래 20260716T* 3개 배치(Task 11/12/13)는 전부
> 리워드 전 픽스처로 측정됐다. 이 커밋에서 tasks-large 베이스 테스트의 doc
> 주석을 인다월드 어휘로 리워드했다(grep으로 노출되던 평가 메타 어휘 제거 —
> 27런 중 10런에서 노출 확인, 상세는
> `docs/research/2026-07-17-m8-failure-analysis.md` §5). 아래 수치는 모두
> **리워드 전 픽스처 기준**이며, M9 측정부터는 리워드된 픽스처를 써야 한다.

### 측정 조건 (Task 10~12 공통)

- 로컬 config (`./.loco/config.toml`): `context_tokens = 8192`(Task 12만 32768),
  `max_output_tokens = 4096`, `command_timeout_secs = 240`
- 머신: macOS 48GB 통합 메모리, LM Studio llama.cpp Metal 백엔드 2.25.2, `PARALLEL=4`·`kv_unified=true`
- 프리필 방법(스펙 §5): 고정 프롬프트 = 베이스 픽스처 `rules/mod.rs` 앞 **540줄**(usage 실측
  **6090 프롬프트 토큰** — 6K±10% 충족; 플랜의 400~450줄 추정은 실측 보정으로 540줄 확정),
  스트리밍 첫 토큰 지연(TTFT) 측정, 로드점당 3반복. ornith은 reasoning 채널로 먼저 출력하므로
  TTFT는 채널 무관 첫 생성 델타 기준

### ornith-1.0-9b 사양표

| 로드 ctx | TTFT 중앙값 | 프리필 tok/s | 디코드 tok/s | 비고 |
|---|---|---|---|---|
| 8192 | 18.263s | ~333 | ~34 | 3반복 편차 <1% |
| 16384 | 18.207s | ~334 | ~34 | 〃 |
| 32768 | 18.238s | ~334 | ~34 | 〃 |

(디코드는 9반복 전체 33.8~34.2 tok/s 범위 — 점별 축약 없이 ~34로 통일)

**속도는 로드 ctx와 무관**(연산은 실토큰 수에만 비례) — 6K 프롬프트 기준 TTFT ~18.2s.
eval 관점: 컨텍스트가 6K 차면 턴당 프리필만 ~18s (프롬프트 캐시 미적중 시).

### 메모리 예산 (폐쇄망 RAM-only 배포 기준)

ornith은 **하이브리드 아치**다: GGUF 메타데이터 `qwen35.full_attention_interval = 4` +
`qwen35.ssm.*` — 32층 중 **8층만 풀 어텐션 KV**, 나머지 24층은 선형 어텐션(O(1) 상태).
토큰당 KV = 8층 × KV헤드 4 × (K256+V256) × 2B(F16) = **32 KiB/토큰**.
실측 교차검증: 6090토큰 프로브 후 와이어드 메모리 +0.16GB — 예측 190MiB에 ±20% 내 부합
(macOS 와이어드 계측 노이즈 감안; 풀 어텐션 가정이었다면 +0.78GB여야 했다).
(주의: 표준 풀 어텐션 가정(128 KiB/토큰) 대비 1/4 — 사양표를 타 모델에 이식하지 말 것)

| 로드 ctx | KV 할당 | 가중치(Q4_K_M) + KV | 
|---|---|---|
| 8192 | 0.25 GiB | ~5.5 GiB |
| 12288 | 0.38 GiB | ~5.6 GiB |
| 16384 | 0.5 GiB | ~5.7 GiB |
| 32768 | 1.0 GiB | ~6.2 GiB |
| 40960 | 1.25 GiB | ~6.5 GiB |
| 49152 | 1.5 GiB | ~6.7 GiB |

- 가중치 5.24 GiB(로드 로그) + KV + SSM 상태·컴퓨트 버퍼(수백 MB급) → **32K급 운용도 총 ~7 GiB**.
  내장 그래픽 전용(전용 VRAM 없음) 사내 머신에서 시스템 RAM만으로 수용 가능한 규모
  (16GB RAM 머신에서도 OS 여유 포함 성립). CPU 백엔드는 KV를 로드 시 전량 할당하므로
  위 표의 할당치가 곧 상주 예산이다
- 이 머신(Metal) 실측: llama-server 프로세스 RSS/풋프린트는 가중치가 와이어드 GPU
  메모리에 있어 과소 표시(풋프린트 ~1GB) — 시스템 모니터로 판독 시 와이어드 메모리 **델타**를
  볼 것: 언로드 3.02GB → 49152 로드 8.22GB, 즉 모델 귀속분 ≈ 델타 5.2GB(절대치는 시스템
  전체 와이어드라 위 표의 ~6.7GiB와 직접 비교 불가; Metal은 KV를 터치 시점에 커밋)

### 32K 로드값 확정 (Task 12용)

**49152 채택.** 근거: KV 40960(1.25GiB) vs 49152(1.5GiB)의 차이 0.25GiB는 48GB 머신에서
무의미한 반면, LM Studio는 로드 ctx가 부족하면 측정 중 400("n_keep > context length")으로
하네스를 중단시키므로 여유가 큰 쪽이 재시작 리스크를 줄인다. 49152 직접 로드 확인 완료
(와이어드 8.22GB; 속도는 8192→32768 평탄 패턴에서 추론 — 49152 자체 TTFT는 별도 미측정).
각주(스펙 §5): **실운용 로드는 여유분 포함(32K 운용 = 로드 40960~49152)**.

### M8 8K 베이스라인 (Task 11, tasks-large 3과제 × 3반복, 시드 0)

로드 ctx 12288(운용 여유), `context_tokens = 8192`. 하네스 855b566 기준.

| 모델 | 통과 | 엄격 | 거짓 finish | 평균 s/런 | find-def | fix-monthly | update-vat |
|---|---|---|---|---|---|---|---|
| gemma-4-e4b | 44.4% (4/9) | 44.4% | 3 | 80.5s | 2/3 (5.0턴) | 0/3 (17.0턴) | 2/3 (24.3턴) |
| ornith-1.0-9b | 55.6% (5/9) | 44.4% | 0 | 156.8s | 3/3 (12.0턴, 엄격 2/3) | 2/3 (8.7턴) | 0/3 (25턴 전멸) |

- 리포트: gemma `.loco/eval/20260716T163308Z`, ornith `.loco/eval/20260716T164620Z` (각 timeout×1)
- **과제별 강약 역전**: gemma는 fix-monthly 전패(증상→원인 추적 실패)·update-vat 2/3,
  ornith은 fix-monthly 2/3·update-vat 전패(3런 모두 MaxTurns 25턴 ~313s — 산포 4지점
  탐색이 8K 컨텍스트에서 턴 예산을 소진). 소형 저장소 세트(tasks/) 수치와의 격차:
  gemma 72.2%→44.4%, ornith 94.4%→55.6% — 대형 저장소 트랙의 존재 이유를 수치로 확인
- 상세 실패 분류는 Task 13 분석 노트로

### M8 32K 민감도 (Task 12, ornith 단독)

`context_tokens = 32768`(로드 49152 — 위 확정값), 그 외 조건 동일. 리포트
`.loco/eval/20260716T171133Z` (effective_config.context_tokens=32768 자증 확인, timeout×1).

| 조건 | 통과 | 엄격 | 거짓 finish | 평균 s/런 | find-def | fix-monthly | update-vat |
|---|---|---|---|---|---|---|---|
| ornith 8K (재게) | 55.6% | 44.4% | 0 | 156.8s | 3/3 | 2/3 | 0/3 |
| ornith 32K | **88.9% (8/9)** | 44.4% | 0 | 217.8s | 3/3 (엄격 1/3) | 2/3 | **3/3** (엄격 1/3) |

- **32K가 update-vat 전멸을 구제**(0/3→3/3): 8K에서 턴 예산을 소진시키던 산포 4지점
  탐색이 컨텍스트 여유로 완주됨 — 관대 +33.3pp
- **엄격 불변(44.4%)**: 구제된 통과가 대부분 MaxTurns 통과(작업 완수 후 finish 미호출) —
  컨텍스트는 탐색 병목을 풀지만 종료 규율은 별개 병목으로 잔존. M9 후보 판단 시
  "컨텍스트 확대"와 "종료 규율 스캐폴딩"을 분리 평가할 것
- 평균 217.8s/런(8K 대비 +39%) — 커진 컨텍스트의 턴당 프리필 비용(사양표의 ~334 tok/s로
  일관 설명). 상세 분류는 Task 13 분석 노트

### M8 최종 (Task 13 실패 분류)

27런(3배치×3과제×3반복) 전수 트랜스크립트 정독 결과, 함정 대장 11종 중 실제로
모델을 오도한 것은 #8·#9뿐(각 1건) — #2·#7도 여러 런에서 조우했지만 #2는
전부 저항했고 #7은 `edit_file`의 closest-match 힌트로 구제됐다. 나머지(특히
#10 갓파일·#11 재수출 사슬)는 `grep`의 정밀한 질의(`fn <함수명>` 패턴)에 9런
전부 무력화됐다 — find-definition-large의 실패는 함정이 아니라 종료 규율
(finish 인자 누락·강박적 재검증)에서 나왔다. 가장 크고 새로운 발견은 스펙 §8
백로그에 없던 `edit_file` 자기-버그(`search`와 `replace`에 동일한 수정 전
텍스트를 넣는 실행 결함) — ornith 전용 패턴으로 27런 중 9런·37회 발생하며,
8K에서는 직접적 실패 원인(반복정지·파일 손상)으로, 32K에서는
`write_file`/`sed`/`python3`
우회에 턴 예산을 다 써 "정답은 만들지만 finish 못 함"(관대 통과·엄격 실패)으로
이어진다 — 32K 민감도의 "엄격 불변" 관찰(위 절)을 턴 단위로 설명하는 기제다.
상세 런별 분류표·함정 발동 근거·M9 요구사항 후보 우선순위는
`docs/research/2026-07-17-m8-failure-analysis.md` 참고.

## M9 1단 재베이스라인 (리워드 픽스처, 스캐폴딩 전, 2026-07-17)

리워드된 픽스처(58aab75 이후)로 M8과 동일 조건 재측정 — M8 수치와의 차이 =
리워드(누출 제거) 효과. **이후 M9 2단(스캐폴딩 후) 비교의 기준선.**
하네스 커밋: 7d06da0.

| 모델 | 통과 | 엄격 | 거짓 finish | 평균 s/런 | report |
|---|---|---|---|---|---|
| gemma-4-e4b @8K | 6/9 | 4/9 | 1 | 81.3s | `20260717T015330Z` |
| ornith-1.0-9b @8K | 5/9 | 5/9 | 1 | 127.0s | `20260717T020632Z` |
| ornith-1.0-9b @32K | 6/9 | 5/9 | 0 | 213.4s | `20260717T022652Z` |

리워드가 수치를 양방향으로 움직였다: gemma 관대 +22.3pp(44.4→66.7%)·거짓 finish 3→1,
ornith@32K 관대 −22.2pp(88.9→66.7%) — M8 32K의 높은 관대 통과에 누출이 기여했다는 신호.
엄격은 ornith 두 배치에서 44.4→55.6%로 소폭 상승, update-vat는 8K에서 여전히 0/3(M8과 동일 병목).

## M9 2단 (스캐폴딩 후, 2026-07-17)

스캐폴딩(edit_file S/R 처방+전용 교정, finish 인자누락 교정, FINISH_NUDGE — 커밋
43020fd~9ad3804) 적용 후 재측정. 1단 대비 차이 = 스캐폴딩 효과. 스포트 배치만
v2 조건(timeout 60s·로드 8192), 나머지는 1단과 동일 조건.

| 배치 | 통과 | 엄격 | 거짓 finish | 평균 s/런 | report |
|---|---|---|---|---|---|
| gemma-4-e4b @8K | 6/9 | 5/9 | 0 | 84.7s | `20260717T031126Z` |
| ornith-1.0-9b @8K | 5/9 | 4/9 | 1 | 127.0s | `20260717T032507Z` |
| ornith-1.0-9b @32K | 7/9 | 5/9 | 0 | 204.4s | `20260717T034527Z` |
| ornith @8K, tasks/ 스포트 | 34/36 | 33/36 | 0 | 84.6s | `20260717T041725Z` |

### 행동 지표 비교 (tasks-large, 1단 → 2단; §2 소표본 규칙에 따라 발생 런 전수)

**S/R 오류 발생 런** ("2시도 내 회복" = 오류 후 다음 2번의 edit/write 시도 안에 성공):

| 단계 | 발생 런 (오류수, 회복) | 오류 합 | 회복 합 | S/R발 반복정지 |
|---|---|---|---|---|
| 1단 | B:fm0(7,1)·fm2(2,1) / C:fm0(1,1)·fm2(1,1)·uv0(5,3)·uv1(3,2) | 19 | 9 | **1** (B fm0) |
| 2단 | D:fm1(2,1)·fm2(1,1)·uv0(1,0) / E:fm0(6,0)·fm2(1,1) / F:fm0(3,0)·fm2(1,1)·uv0(5,3)·uv1(3,0) | 23 | 7 | **1** (E fm0) |

- SR_CORRECTION 발동: tasks-large 4런(E fm0, F fm0·uv0·uv1) 중 다음 시도 내 회복 1런(F uv0).
  **tasks/ 스포트에서는 발동 5런 전부 다음 시도 내 회복** (add-function-0,
  fix-compile-error-0, fix-off-by-one-0·2, rename-function-0) — 교정 실효가 과제
  난이도에 갈린다.
- E fm0 정독: 첫 편집부터 동일 S/R 호출 6연속 — 도구 처방(1회차)→SR_CORRECTION(2회차)→
  REPEAT_CORRECTION(3회째) 3층을 전부 무시하고 5회째 반복정지. 텍스트 교정의 한계
  케이스(스펙 §7 리스크 실증).

**finish 규율**:

| 지표 | 1단 | 2단 |
|---|---|---|
| 인자누락발 반복정지 | 0 | 0 (비악화 — 대체 판정: 단위 테스트 게이트 통과) |
| 인자누락 finish 시도 (런/회) | 8런 17회 (최장 6회 — C fd1, max_turns 미종결) | 10런 24회 (최장 5회) |
| FINISH_ARGS_CORRECTION 발동 | — | 2런 (D fd2, E fd1) — **둘 다 이후 유효 finish 도달** |
| 검증 성공 런 중 finished 종결 | 14/18 | 14/18 (동률) |
| FINISH_NUDGE 발동 | — | 2건: E fd0(7액션 뒤 finish), tasks/ rename-1(즉시 finish) — 모두 종결 |
| 거짓 성공 finish | 2 | 1 |

순수 동일-명령 재검증 루프(B/E uv1 — `cargo test` exit 0 반복 5회)는 설계대로
반복정지가 선점해 FINISH_NUDGE 도달 전에 정지 — 1·2단 동일 패턴 각 1건.

### 성공 기준 판정 (스펙 §2 전건 대조)

1. **게이트 4종: 충족** — cargo test 286 통과·clippy 0·verify 12/12·3/3
2. **행동 지표 ①(S/R): 미충족** — S/R발 반복정지 1건 잔존(1단과 동수, 동일 과제
   fix-monthly-total), 2시도 내 회복 9/19→7/23(오류당)로 상승 아님. 단 소형
   세트에선 SR_CORRECTION 5/5 즉시 회복 — 개입 자체는 동작하나 대형 저장소의
   완고한 루프(ornith fm0 패턴)를 못 끊는다.
   **행동 지표 ②(finish): 부분 충족** — 인자누락발 정지 0 유지(대체 판정 충족),
   ARGS_CORRECTION 발동 2런 모두 종결 도달(방향성 긍정), 검증 후 finish 도달률은
   14/18 동률(상승 아님).
3. **통과율(보조): 부분 충족** — tasks-large 엄격: gemma 4/9→5/9 ↑,
   ornith@32K 5/9 =, **ornith@8K 5/9→4/9 ↓1런(비악화 미달, 소표본 노이즈 범위)**.
   tasks/ 스포트 34/36 ≥ 33/36 충족(엄격 33/36, 거짓 finish 0).
4. **신규 모델-대면 텍스트 영문: 충족** — S/R 처방·SR_CORRECTION·
   FINISH_ARGS_CORRECTION·FINISH_NUDGE 전부 영문.

### 행동 지표 추출 레시피

각 배치 디렉토리에 대해 (트랜스크립트 kind: system/user/assistant/tool_result;
교정 노트는 별도 user 이벤트, tool_result의 도구명 필드는 `tool`):

```python
import json, sys, glob, os
# usage: python3 m9_metrics.py .loco/eval/<stamp>
MARKS = {
    "sr_error": "search and replace are identical",
    "sr_correction": "Write the MODIFIED code in `replace`",
    "finish_missing": "finish requires a string `summary`",
    "finish_args_corr": "Do not call finish with empty args again",
    "finish_nudge": "do not re-verify what you have already confirmed",
    "repeat_corr": "repeating the same tool call",
}
for path in sorted(glob.glob(os.path.join(sys.argv[1], "run-*.jsonl"))):
    counts = dict.fromkeys(MARKS, 0)
    with open(path) as f:
        for line in f:
            e = json.loads(line)
            if e.get("kind") == "assistant":
                continue  # 모델이 인용한 문구는 세지 않는다
            c = e.get("content", "") or ""
            for k, m in MARKS.items():
                counts[k] += c.count(m)
    print(os.path.basename(path), counts)
```
