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
- **keep/revert**: n=36·p≈⅓에서 통과 수의 σ≈2.8런이므로 통과 수 단독으로는 revert
  근거가 부족하다. revert 조건: 직전 측정 대비 **2런 이상 감소 + 겨냥 메커니즘 지표
  악화 동반**, 또는 확인 재측정 1회에서 감소 재현. ±1런은 노이즈로 간주해 keep.
  스캐폴딩 변경은 궤적 전체를 바꾸므로 배치 간 비교는 비페어 비교임을 전제한다
- **알려진 한계 (수용)**: 배치 측정이 qwen 단독이므로 gemma 회귀는 최종 측정에서야
  드러나고 배치 귀속이 불가능하다 — 모델 교체 비용을 고려한 의도적 트레이드오프.
  또한 Batch 1의 프롬프트 검증 규칙과 Batch 3의 검증 넛지는 같은 메커니즘 ②를
  겨냥하므로 누적 측정에서 Batch 3의 한계 효과가 축소되어 보일 수 있다
- **메커니즘 지표**: 통과율과 별개로 배치마다 트랜스크립트에서 집계 —
  `missing field` 횟수(①), cargo 실행 횟수·거짓 성공 finish 수(②),
  `search block not found`·multi-match 횟수(③), max_turns/RepetitionStop 분포(④⑤),
  replace_all 오용 징후(단일 위치 의도인데 다중 치환된 케이스 — Batch 2 회귀 관찰용).
  통과율이 안 움직여도 겨냥 메커니즘의 소멸 여부로 항목 효과를 판단한다
- **기록**: 배치별 결과와 최종 판정을 `docs/baselines.md`에 M5 절로 추가

## 4. Batch 0 — 하네스 무결성 (측정 없음)

1. **`.cargo` 판정 우회 차단 (샌드박스 내부 벡터)**: `sync_protected`가 매 과제에서
   `.cargo`를 **암묵 protected 경로**로 항상 포함한다(task.toml의 protected 목록과 합집합).
   fixture에 없으면 에이전트가 만든 `.cargo/`를 check 전에 삭제, 있으면 원복 — 기존
   "fixture 원본과 정확히 일치" 의미론 재사용. 가짜 러너(`.cargo/config.toml`의 runner
   바꿔치기)에 의한 가짜 통과를 **샌드박스 내부에서** 차단한다.
   cargo의 config 탐색은 cwd에서 루트까지 상향이므로 샌드박스 **밖** 벡터(temp_dir의
   `.cargo/`, `$CARGO_HOME`)는 남는다 — 저비용 트립와이어로 샌드박스 루트의 상위
   경로들(temp_dir까지)에 `.cargo`가 존재하면 하네스 에러로 중단하고, `$CARGO_HOME`/
   홈 디렉터리 벡터는 미차단 잔여 한계로 `docs/baselines.md` 한계 절에 승계 기록한다
   (협조적 4B 모델 전제에서 실위험 근접 0, 트랜스크립트로 감사 가능 — 기존 서술 유지)
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
   - action 레벨의 오브젝트 미지 키(`args_2` 등)를 args에 병합. 충돌 시 우선순위는
     **키 이름 오름차순으로 병합하며 나중 병합이 이김** (serde_json 기본 Map은 BTreeMap이라
     문서 내 키 순서를 보존하지 않음 — preserve_order는 indexmap 의존이라 금지).
     관측된 분열형(`args` 잔재 + `args_2` 최신)에서 이 규칙은 "최신 의도 우선"과 일치한다
   - 턴 최상위의 미지 키도 동일 승격. `thought`/`action`/`tool`/`args*` 예약어 제외
   - **범위 제외**: action 래퍼가 없는 플랫 턴(`{"thought","tool","args"}`)은 36런 어디에도
     관측되지 않아 다루지 않는다 — M5 측정 트랜스크립트에서 관측되면 그때 추가
   - salvage 발동 시 툴 결과에 한 줄 노트 부가:
     `note: fields outside "args" were accepted this time - put them inside "args".`
   - Batch 3의 반복 감지 키는 **salvage 정규화 후의 args**로 계산한다 (정규화 전 키를 쓰면
     args 밖 필드만 다른 호출들이 같은 키로 합쳐지는 기존 잠재 버그가 존속 — 실사례:
     qwen fix-compile-error-0의 `read_file|{}` 5연속 RepetitionStop)
2. **스키마 에코 에러** (`Registry::dispatch`): 툴이 `BadArgs`를 반환하면 dispatch가 일괄로
   기대 시그니처(해당 툴의 `doc()`)와 실제 수신 키 목록을 덧붙인다:
   `invalid arguments: missing field 'search'. edit_file(path, search, replace): ... You sent keys: [pattern, path].`
   finish의 summary 누락 에러에도 형태 예시 `{"tool":"finish","args":{"summary":"..."}}` 포함
3. **날 것 에러 번역**: read_file이 디렉터리를 받으면
   `path is a directory, not a file - use list_files for directories`.
   grep 정규식 파싱 실패 시 **리터럴 검색으로 자동 폴백**. 0매치가 "그 코드는 없다"로
   오독되지 않도록 폴백 시 매치 유무와 무관하게 헤더에 원인을 병기한다:
   `invalid regex (<사유>); searched for the literal text instead - N matches`
4. **시스템 프롬프트 개정** (`agent/prompt.rs`): few-shot 예시를 grep 1개 → grep +
   edit_file(여러 줄 search) + run_command 3개로(grep 스키마 고착 해소). 규칙 2줄 추가:
   "After changing files, verify with run_command (e.g. `cargo test`) before finish" /
   "Copy `search` text exactly from the latest read_file output". 추가 토큰 ~150개

## 6. Batch 2 — edit_file 개선 (메커니즘 ③)

툴 형태(`path, search, replace`)는 유지하고 피드백과 옵션만 추가한다.

**크기 상한 원칙**: 이 배치가 추가하는 모든 모델 대상 텍스트는 상한을 가진다. 측정
조건(`max_output_tokens=4096`)에서 입력 예산은 (8192−4096)×0.9 ≈ 3686토큰으로 기본값의
2/3이고, `pack()`은 마지막 메시지를 줄이지 못하므로 상한 없는 툴 결과는 컨텍스트 400 →
eval 하네스 전체 중단으로 이어질 수 있다(픽스처 `.gitignore` 사건과 동일 기전, 스펙 §6
예산 원칙 준수).

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
   블록 길이만큼, **최대 10줄까지** 인용한다:
   ```
   search block not found. Closest match at lines 6-8:
   <실제 텍스트>
   Copy this text exactly into `search` if this is the location you meant.
   ```
   JSON 이스케이프 깊이 불일치(multiline-string-edit의 사인)에서 "복사만 하면 되는" 재료 제공
3. **multi-match 에러에 위치 나열**: `matches 3 locations (lines 4, 9, 17)` + 각 매치의
   첫 줄 인용 + "add surrounding lines to pick one, or set \"replace_all\": true if you
   intend to change every occurrence" 안내. 나열은 **최대 5개 위치**, 초과분은 `and N more`
4. **`replace_all` 옵션 인자**: `edit_file(path, search, replace, replace_all?)`. true면
   매칭 사다리의 해당 단계에서 발견된 모든 위치를 일괄 치환, `replaced N occurrences` +
   **첫 매치의 결과 컨텍스트만** 보고(크기 상한 원칙). rename-function(양 모델 0/6)의
   구조적 해결책. 의미론: 매치는 **비중첩 탐욕**(2·3단계 라인 윈도 포함), 3단계
   indent-shift는 위치마다 자기 indent를 적용. 1단계는 부분문자열 치환이므로 식별자
   내부 매치(`old_fn`이 `old_fn_helper`의 접두)까지 바꾼다 — 이 한계를 doc()에 명시한다
5. **무변경 편집 에러화**: `search == replace`는
   `search and replace are identical - no change would be made` 에러 (성공 오인 차단)

## 7. Batch 3 — 검증 넛지·반복 윈도 (메커니즘 ②④⑤)

모두 `agent/mod.rs` 루프 내부 변경.

1. **검증 넛지 (finish 1회 반려)**: 마지막 성공한 mutating 툴 이후 run_command 실행 여부를
   추적한다. 편집이 있었는데 이후 run_command가 없으면 summary 있는 finish를 **실행당
   1회에 한해** 반려하고 되먹인다:
   `You modified files but never ran a verification command. Run the project's tests (e.g. cargo test) with run_command, then finish.`
   두 번째 finish는 무조건 통과(무한 반려 없음). 편집 없는 실행은 넛지 없음.
   "run_command 실행"의 판정: **디스패치가 Ok를 반환**한 경우만 인정(명령이 실제로
   돌았다는 뜻 — 종료 코드는 무관, `exit code: N`도 Ok). 거부(Denied)·BadArgs는 미인정.
   엣지 수용: 마지막 턴 직전의 넛지 반려는 Finished를 MaxTurns로 바꿀 수 있다 —
   eval 판정은 동일(check는 outcome 무관 실행)하고 REPL에선 요약이 유실되나 드묾
2. **반복 감지 윈도 확장**: 현행 "직전 (tool, args) 연속 일치"를 **(tool, args, 결과 해시)
   단위의 최근 8턴 윈도**로 일반화한다. 윈도 항목은 디스패치된 툴 호출(및 summary 없는
   finish — 결과는 상수 에러 메시지)이며, 파싱 실패·length 교정 턴은 계수하지 않는다:
   - 계수는 전부 **디스패치 후**(결과 해시 확보 시점)에 한다 — 결과를 예단하는 디스패치 전
     정지는 두지 않는다("4회 동일 읽기 → 편집 성공 → 달라진 재읽기"를 거짓 정지시키는
     구멍). 따라서 5회째 정지는 현행 규칙보다 로컬 툴 실행 1회만큼 늦게 발화한다(LLM
     호출 수는 동일) — 정확성을 위한 수용
   - 윈도 내 동일 (호출, 결과) 3회째 → 교정 메시지 1회 주입
   - 윈도 내 동일 (호출, 결과) 5회째 → `RepetitionStop`. 8턴 윈도에서 이 정지는 사실상
     **연속 반복(주기 1)에서만** 도달한다 — 엄격한 주기 2 교대는 윈도 내 같은 항목이
     최대 4회, 주기 3(예: chain-edits의 A→B→C 순환)은 최대 3회라 교정(3회째)과
     max_turns가 상한이 된다 (윈도를 넓히면 "서로 다른 편집 사이 동일한 실패 테스트
     결과" 같은 정상 진행을 오정지할 위험이 생겨 채택하지 않음)
   - 결과 해시가 키에 포함되므로 정당한 재읽기(편집으로 내용이 바뀐 파일)는 걸리지 않음
   - **동일 에러 3연속**(호출이 달라도) → 전략 전환 교정 주입. 동일성 판정은 에러
     **첫 문장**(첫 마침표까지 — Batch 1·2가 첫 줄 **안에** 가변 내용을 붙인다: 스키마
     에코의 수신 키 목록, not-found의 최근접 위치 `lines A-B` — 첫 줄 전체나 전문
     비교는 무력화됨). 교정문은 에러 계열별: 파일 편집 에러면
     `re-read the file, then rewrite it completely with write_file` (write_file 성공
     1/36런 — gemma mse-1 유일, qwen 0/18 — 의 탈출로 개방), 그 외는 일반 전략 전환 문구
   - 교정 메시지는 실행당 종류별 1회
3. **summary 없는 finish의 감지 편입**: 현행 의도적 면제(스펙 §3 사각지대)를 폐지하고 위
   윈도 계수에 포함, 5회째 `RepetitionStop`. 폴백 요약 조작(thought를 답변으로 승격)은
   하지 않는다 — 거짓 성공 요약 위험

**스펙 개정**: 이 배치는 스펙 §3의 명시된 v1 사각지대 2건(교대 반복, finish 반복)을
해소하는 의미 변경이다. 구현 시 본선 스펙의 개정 이력에 추가한다.

## 8. 테스트 전략

- **Batch 0**: sandbox 테스트에 `.cargo` 암묵 protected 케이스(에이전트가 추가한
  `.cargo/config.toml` 삭제, fixture 보유 시 원복) + 상위 경로 트립와이어(부모에 `.cargo`
  존재 시 하네스 에러) 추가. timeout 클램프 단위 테스트.
  report 스냅샷은 eval 픽스처 테스트에서 필드 존재 단언
- **Batch 1**: salvage 케이스는 실제 트랜스크립트의 원문 형태(`args_2` 분열, action 레벨
  필드, grep 스키마 고착, 빈 args)를 픽스처로 사용. 기존 프로토콜 테스트 전부 무변경 통과
  (관용은 실패하던 입력만 살린다). 스키마 에코는 Registry 단위 테스트. grep 리터럴 폴백은
  `{user_name}` 실사례로
- **Batch 2**: 기존 edit_file 테스트 8개 유지(성공 메시지 형식 단언만 갱신) + 신규:
  컨텍스트 반환 경계(파일 첫/끝), 최근접 인용, multi-match 줄번호, replace_all(각 사다리
  단계), no-op 에러, CRLF 파일 replace_all
- **Batch 3**: 전부 `Scripted` 가짜 클라이언트 — 넛지 발동/1회 한정/편집 없는 실행 비발동,
  A↔B 교대 3회째 교정, 5회째 정지, **4회 동일 읽기 → 편집 성공 → 달라진 재읽기 비정지**,
  동일 에러 3연속 교정(에러 계열별 교정문 포함), summary 없는 finish 5회 종식,
  반복 키가 salvage 정규화 후 args 기준임을 단언. 기존 반복 감지 테스트는 LLM 호출 수
  관점의 발화 시점 유지를 단언
- 게이트: 배치마다 `cargo test` + `cargo clippy --all-targets -- -D warnings`

## 9. 구현 순서와 의존성

Batch 0 → 1 → 2 → 3 순서. 배치 간 코드 의존은 없으나(독립 모듈), 측정 순서가 누적
비교이므로 순서를 지킨다. 각 배치는 측정 후 다음 배치 착수 (revert 판단을 위해).

## 개정 이력

- 2026-07-12: 최초 승인본 (브레인스토밍 섹션별 승인 완료)
- 2026-07-12: 독립 리뷰 14건 반영 (Ready=Yes, Critical 0) — 반복 계수의 디스패치 후 일원화
  (예단 정지 구멍 제거), salvage 병합 순서의 키 이름 결정론 재정의(preserve_order 의존
  회피), 신규 툴 텍스트 크기 상한(§6 예산 정합), keep/revert 규칙 강화(σ≈2.8런 근거)와
  gemma 무신호·프롬프트/넛지 중첩 한계 인정, .cargo 차단 범위 정정(샌드박스 내부)+
  트립와이어, grep 리터럴 폴백 원인 병기, 동일 에러 판정 첫 줄 기준+교정문 계열화,
  주기 3 정지 불가 명시(윈도 8 유지 근거), replace_all 의미론 3건(비중첩·자기 indent·
  부분문자열 한계), write_file 수치 정정(성공 1/36), 넛지 판정 기준(Ok 디스패치)·
  마지막 턴 엣지 수용, 플랫 턴 범위 제외 명시, 반복 키의 salvage 후 계산 명시
- 2026-07-12: 플랜 리뷰 반영 — 동일 에러 판정을 첫 줄→**첫 문장** 기준으로 정정(개선된
  에러 메시지가 첫 줄 안에 가변 내용을 포함), 윈도 8의 정지 도달 가능성 서술 정정
  (주기 2 교대는 최대 4회라 정지 불가 — 교정+max_turns 상한)
