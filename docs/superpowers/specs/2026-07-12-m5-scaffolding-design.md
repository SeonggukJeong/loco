# M5 — 스캐폴딩 개선 (기준선 대비 측정) 설계

승인일: 2026-07-12. 본 문서는 loco 설계 스펙(2026-07-02)의 M5 마일스톤을 구체화한다.
기준선: `docs/baselines.md` (gemma-4-e4b 11.1%, qwen3-vl-4b 33.3%, `eval tasks --repeats 3`, seed 0, ctx 8192).

## 1. 배경 — 기준선 실패 분석

공통 0% 과제 6종(add-function, chain-edits, implement-from-doc, multiline-string-edit,
rename-function, fix-compile-error)의 실패 트랜스크립트 36런(2모델 × 6과제 × 3반복)을
전수 분석한 결과, 실패는 5개 메커니즘으로 수렴한다:

| # | 메커니즘 | gemma | qwen |
|---|---|---|---|
| ① | 인자 스키마 붕괴 — `args_2` 분열, grep 스키마 고착, args 밖 필드 배치 | 턴의 54% 소모, 13/18런 무수정 종료 | fix-compile-error 3런 전멸 |
| ② | 검증 0회 + 거짓 성공 finish | cargo 성공 0/18 | cargo 미실행 17/18 |
| ③ | edit_file 파괴·불일치 — mid-line splice, 이스케이프 늪, 성공 시 무피드백, multi-match 정보 부재, replace_all 부재 | 이스케이프 늪·모호 매치 | 11/18런 파일 파괴 |
| ④ | 주기 2~3 루프(read↔edit 왕복 등)의 반복 감지 회피 | edit↔read 왕복 | max_turns 11런 전부 |
| ⑤ | summary 없는 finish 반복 (감지 면제, 스펙 §3 사각지대) | 3런 29턴 낭비 | 경미 |

핵심 관찰: **정답이 이미 모델 안에 있었던 런이 다수다.** gemma add-function-2는 거부당한
`args_2` 안에 거의 완전한 정답 구현이 있었고, rename-function은 양 모델 모두 3파일 중
2파일까지 도달했다. 스캐폴딩이 받아주기만 했어도 통과했을 런이 실재한다.

분석 원본: `.loco/eval/20260711T165322Z`(gemma), `.loco/eval/20260711T235558Z`(qwen)의
per-run 트랜스크립트.

## 2. 목표·성공 기준·범위

**목표**: 도구 인터페이스·에러 피드백·루프 구조 개선으로 위 5개 메커니즘을 제거하고,
효과를 기준선 대비 eval 통과율로 측정한다.

**성공 기준** (최종 판정, 두 모델 전체 측정):
1. 공통 0% 6종 중 **최소 2종**이 한 모델에서라도 0 탈출
2. qwen 안정 통과 4종(create-module, edit-crlf-file, find-definition, fix-off-by-one)이
   각각 ≥2/3 유지 (무회귀)
3. 모델별 전체 통과율(n=36)이 기준선 이상

**범위 결정** (브레인스토밍 합의):
- 하네스 무결성 수정 포함 (Batch 0)
- 과제 변별력 개편(find-definition 정답 형식, fix-off-by-one 비변별 케이스 등)은 **M6으로
  연기** — 픽스처 변경은 기준선을 무효화하므로, M5는 기존 기준선을 그대로 사용한다
- **신규 크레이트 없음** — 프롬프트·툴 인터페이스·에러 피드백·루프 구조 개선만.
  tree-sitter repo map, 자동 검증 실행(루프가 스스로 cargo check 주입), 줄번호 편집 툴
  (`replace_lines`)은 M6+ 후보로 보류
- 라인 경계 강제 매칭 보류 — 정당한 단일 토큰 치환(qwen이 성공적으로 쓰던 패턴)까지
  차단해 회귀 위험. Batch 2의 결과 컨텍스트 반환이 splice 노출을 대신한다

## 3. 측정 프로토콜

- **배치 단위 측정**: Batch 1~3 각각 구현 후 qwen3-vl-4b로 `eval tasks --repeats 3` 1회
  (기준선과 동일 조건: LM Studio ctx 8192, 로컬 config `max_output_tokens = 4096`, seed 0).
  Batch 0은 에이전트 행동 불변이므로 측정하지 않는다
- **최종 판정**: 전 배치 완료 후 gemma·qwen 두 모델 전체 측정 (모델 교체는 사용자 협조)
- **keep/revert**: 배치 측정에서 전체 통과 수가 직전 측정 대비 2런 이상 감소하면 원인
  분석 후 해당 배치 항목을 선별 revert. ±1런은 노이즈로 간주해 keep
- **메커니즘 지표**: 통과율과 별개로 배치마다 트랜스크립트에서 집계 —
  `missing field` 횟수(①), cargo 실행 횟수·거짓 성공 finish 수(②),
  `search block not found`·multi-match 횟수(③), max_turns/RepetitionStop 분포(④⑤).
  통과율이 안 움직여도 겨냥 메커니즘의 소멸 여부로 항목 효과를 판단한다
- **기록**: 배치별 결과와 최종 판정을 `docs/baselines.md`에 M5 절로 추가

## 4. Batch 0 — 하네스 무결성 (측정 없음)

1. **`.cargo` 판정 우회 차단**: `sync_protected`가 매 과제에서 `.cargo`를 **암묵 protected
   경로**로 항상 포함한다(task.toml의 protected 목록과 합집합). fixture에 없으면 에이전트가
   만든 `.cargo/`를 check 전에 삭제, 있으면 원복 — 기존 "fixture 원본과 정확히 일치"
   의미론 재사용. 가짜 러너(`.cargo/config.toml`의 runner 바꿔치기)에 의한 가짜 통과 차단
2. **timeout 상한**: `timeout_secs`/`check_timeout_secs` × `--timeout-scale` 계산을 포화
   연산으로 하고 상한 3600초로 클램프 — 거대 값의 Duration 패닉 차단
3. **report.json 유효 config 스냅샷**: 해석 완료된 설정(model, temperature, max_turns,
   max_output_tokens, context_tokens, command_timeout_secs, base_url)과 loco 버전
   (`CARGO_PKG_VERSION`)을 report에 기록 — 측정 조건의 수동 대조 문제 해소

## 5. Batch 1 — 프로토콜 관용·에러 피드백·프롬프트 (메커니즘 ①⑤)

1. **Salvage 파싱** (`agent/protocol.rs`): `parse_turn`을 `serde_json::Value` 경유로 바꾸고
   정규화 단계를 추가한다:
   - action 레벨의 스칼라 미지 키를 args로 승격 —
     `{"tool": "run_command", "args": {}, "command": "..."}` → `args.command`
   - action 레벨의 오브젝트 미지 키(`args_2` 등)를 args에 병합, **충돌 시 나중 키 우선**
     (모델의 최신 의도 우선)
   - 턴 최상위의 미지 키도 동일 승격. `thought`/`action`/`tool`/`args*` 예약어 제외
   - salvage 발동 시 툴 결과에 한 줄 노트 부가:
     `note: fields outside "args" were accepted this time - put them inside "args".`
2. **스키마 에코 에러** (`Registry::dispatch`): 툴이 `BadArgs`를 반환하면 dispatch가 일괄로
   기대 시그니처(해당 툴의 `doc()`)와 실제 수신 키 목록을 덧붙인다:
   `invalid arguments: missing field 'search'. edit_file(path, search, replace): ... You sent keys: [pattern, path].`
   finish의 summary 누락 에러에도 형태 예시 `{"tool":"finish","args":{"summary":"..."}}` 포함
3. **날 것 에러 번역**: read_file이 디렉터리를 받으면
   `path is a directory, not a file - use list_files for directories`.
   grep 정규식 파싱 실패 시 **리터럴 검색으로 자동 폴백**, 결과에 `(literal match)` 표기
4. **시스템 프롬프트 개정** (`agent/prompt.rs`): few-shot 예시를 grep 1개 → grep +
   edit_file(여러 줄 search) + run_command 3개로(grep 스키마 고착 해소). 규칙 2줄 추가:
   "After changing files, verify with run_command (e.g. `cargo test`) before finish" /
   "Copy `search` text exactly from the latest read_file output". 추가 토큰 ~150개

## 6. Batch 2 — edit_file 개선 (메커니즘 ③)

툴 형태(`path, search, replace`)는 유지하고 피드백과 옵션만 추가한다.

1. **성공 시 결과 컨텍스트 반환**: 성공 메시지에 변경 부위 ±3줄(편집 후 상태)을 덧붙인다.
   줄번호는 헤더에만 표기하고 본문은 원문 그대로(줄번호 접두가 다음 search에 복사될 위험
   차단). preview(확인 게이트)는 기존 diff 유지
   ```
   Edited src/lib.rs (matched exact). Context after edit (lines 4-12):
   <구간 원문>
   Verify this is what you intended.
   ```
2. **not-found 에러에 최근접 실제 텍스트 인용**: search 첫 줄이 부분 매치되는 라인을 찾고
   (없으면 문자 bigram 중첩 최대 라인 — 무의존 구현), 그 위치의 실제 파일 내용을 search
   블록 길이만큼 인용한다:
   ```
   search block not found. Closest match at lines 6-8:
   <실제 텍스트>
   Copy this text exactly into `search` if this is the location you meant.
   ```
   JSON 이스케이프 깊이 불일치(multiline-string-edit의 사인)에서 "복사만 하면 되는" 재료 제공
3. **multi-match 에러에 위치 나열**: `matches 3 locations (lines 4, 9, 17)` + 각 매치의
   첫 줄 인용 + "add surrounding lines to pick one, or set \"replace_all\": true" 안내
4. **`replace_all` 옵션 인자**: `edit_file(path, search, replace, replace_all?)`. true면
   매칭 사다리의 해당 단계에서 발견된 모든 위치를 일괄 치환, `replaced N occurrences` 보고.
   rename-function(양 모델 0/6)의 구조적 해결책
5. **무변경 편집 에러화**: `search == replace`는
   `search and replace are identical - no change would be made` 에러 (성공 오인 차단)

## 7. Batch 3 — 검증 넛지·반복 윈도 (메커니즘 ②④⑤)

모두 `agent/mod.rs` 루프 내부 변경.

1. **검증 넛지 (finish 1회 반려)**: 마지막 성공한 mutating 툴 이후 run_command 실행 여부를
   추적한다. 편집이 있었는데 이후 run_command가 없으면 summary 있는 finish를 **실행당
   1회에 한해** 반려하고 되먹인다:
   `You modified files but never ran a verification command. Run the project's tests (e.g. cargo test) with run_command, then finish.`
   두 번째 finish는 무조건 통과(무한 반려 없음). 편집 없는 실행은 넛지 없음
2. **반복 감지 윈도 확장**: 현행 "직전 (tool, args) 연속 일치"를 **(tool, args, 결과 해시)
   단위의 최근 8턴 윈도**로 일반화한다:
   - 윈도 내 동일 (호출, 결과) 3회째 → 교정 메시지 1회 주입 (주기 2~3 사이클 포착)
   - 동일 (호출, 결과) 5회째 → `RepetitionStop`. 발화 시점: 들어온 호출이 윈도 내 동일
     결과 4회를 이미 가진 키와 일치하면 **디스패치 전에** 정지 — 연속 반복 시나리오에서
     기존 규칙과 같은 턴에 발화한다
   - 결과 해시가 키에 포함되므로 정당한 재읽기(편집으로 내용이 바뀐 파일)는 걸리지 않음
   - **동일 에러 텍스트 3연속**(호출이 달라도) → 전략 전환 교정 주입:
     `The same error keeps occurring. Change strategy: re-read the file, then rewrite it completely with write_file.`
     (write_file 사용 0/36런의 탈출로 개방)
   - 교정 메시지는 실행당 종류별 1회
3. **summary 없는 finish의 감지 편입**: 현행 의도적 면제(스펙 §3 사각지대)를 폐지하고 위
   윈도 계수에 포함, 5회째 `RepetitionStop`. 폴백 요약 조작(thought를 답변으로 승격)은
   하지 않는다 — 거짓 성공 요약 위험

**스펙 개정**: 이 배치는 스펙 §3의 명시된 v1 사각지대 2건(교대 반복, finish 반복)을
해소하는 의미 변경이다. 구현 시 본선 스펙의 개정 이력에 추가한다.

## 8. 테스트 전략

- **Batch 0**: sandbox 테스트에 `.cargo` 암묵 protected 케이스(에이전트가 추가한
  `.cargo/config.toml` 삭제, fixture 보유 시 원복) 추가. timeout 클램프 단위 테스트.
  report 스냅샷은 eval 픽스처 테스트에서 필드 존재 단언
- **Batch 1**: salvage 케이스는 실제 트랜스크립트의 원문 형태(`args_2` 분열, action 레벨
  필드, grep 스키마 고착, 빈 args)를 픽스처로 사용. 기존 프로토콜 테스트 전부 무변경 통과
  (관용은 실패하던 입력만 살린다). 스키마 에코는 Registry 단위 테스트. grep 리터럴 폴백은
  `{user_name}` 실사례로
- **Batch 2**: 기존 edit_file 테스트 8개 유지(성공 메시지 형식 단언만 갱신) + 신규:
  컨텍스트 반환 경계(파일 첫/끝), 최근접 인용, multi-match 줄번호, replace_all(각 사다리
  단계), no-op 에러, CRLF 파일 replace_all
- **Batch 3**: 전부 `Scripted` 가짜 클라이언트 — 넛지 발동/1회 한정/편집 없는 실행 비발동,
  A↔B 교대 3회째 교정, 5회째 정지, 내용 바뀐 재읽기 비발동, 동일 에러 3연속 교정,
  summary 없는 finish 5회 종식. 기존 반복 감지 테스트는 발화 시점 동일성 단언 유지
- 게이트: 배치마다 `cargo test` + `cargo clippy --all-targets -- -D warnings`

## 9. 구현 순서와 의존성

Batch 0 → 1 → 2 → 3 순서. 배치 간 코드 의존은 없으나(독립 모듈), 측정 순서가 누적
비교이므로 순서를 지킨다. 각 배치는 측정 후 다음 배치 착수 (revert 판단을 위해).

## 개정 이력

- 2026-07-12: 최초 승인본 (브레인스토밍 섹션별 승인 완료)
