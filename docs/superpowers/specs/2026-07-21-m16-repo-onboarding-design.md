# M16 설계 — 계층 레포 notes 온보딩 하네스

- 상태: **초안 (brainstorm 합의 반영) — 사용자 스펙 리뷰 대기**
- 작성일: 2026-07-21
- 기준 커밋: `232e551` (main, M15 병합)
- 선행 문서:
  - `docs/m16-candidates.md` (후보 A–E · 열린 결정)
  - `docs/baselines.md` "M15" · `docs/experiments/2026-07-20-m15-real-repo-baseline/report.md`
  - M15 스펙 `docs/superpowers/specs/2026-07-20-m15-real-repo-track-design.md`
  - `docs/experiments/PROTOCOL.md`
- 스코핑: 2026-07-21 brainstorm (superpowers) — 사용자 결정 요약 §0

## 0. 스코핑 경위 (사용자 결정)

| # | 결정 | 내용 |
|---|---|---|
| 1 | 본선 형태 | **B2 hard gate** (전용 notes 툴 + mut 전 조건). B3 phased(초반 mut 툴 비활성)는 max_turns 재설계·Timeout 풍선으로 1차 제외. C 자동 주입·D REPO_MAP·E RAG는 비목표 또는 백로그 |
| 2 | 수명 | **프로젝트 디스크 영속** — 같은 레포 세션 간 재사용. eval 샌드박스는 fixture에 notes 없어 **런마다 cold start** |
| 3 | 구조 | **계층 notes**: root = 짧은 설명 + **라우팅**; 디렉/모듈로 갈수록 상세. 단일 거대 notes 통째 컨텍스트 주입 금지 (소형 모델 토큰 압박) |
| 4 | mut 게이트 세기 | **root 스키마 OK ∧ 편집 경로의 notes 조상 중 ≥1 스키마 OK** |
| 5 | 품질·갱신 | **형식 스키마**(의미 검증 없음) + **성공 코드 mut 후 dirty → finish 1회 거부**로 갱신 유도. thrifty 템플릿은 SYSTEM 장문이 아니라 **툴 description + 거부 body** |
| 6 | 측정 | **feature flag on/off + control 재측정**. M15 0/51 스탬프를 control로 인용 금지 |
| 7 | 1차 예산 | treatment·control 모두 **max_turns=25 · timeout_secs=600** (M15와 동일). 예산 상향은 주 주장에 섞지 않음 |

**사용자 문제 제기 (설계 핵심):** “notes 있음/없음만 보면 갱신이 안 되고, 비었을 때 무엇을 최소 토큰으로 쓸지 지침이 없다.”  
→ §3 스키마·템플릿·stale-finish가 그 답이다.

---

## 1. 배경

### 1-1. M15가 고정한 바닥

`tasks-real` N=17 · ×3 · seed 0 · 운용 32768 · 로드 37632 · 이슈 본문 only:

| 지표 | 값 |
|---|---|
| 통과 | **0/51** (엄격 0 · ff 1) |
| 전패 | 17/17 → **실격** (양의 베이스라인 확보 실패 · M16 대조 인용 비권고) |
| outcome | max_turns 27 · timeout 18 · repetition_stop 5 · finished 1 |
| fail-층 nav_hit / fix_hit | ≈0.75 / ≈0.22 |

해석: 테스트/`--verify` 결함이 아니라 **cold start에서 수선·완주 실패**.  
합성 `tasks/` 고득점과 나란히 비교하지 않는다.

### 1-2. 제품 직관과 평가의 간극

실사용(및 대형 모델 관행)은 낯선 레포에서 **탐색 → 요약 지도 → 작업**이다.  
M15 평가는 그 1단계를 허용·강제하지 않는 **가혹 조건**이다. M16은 지도를
**하네스 계약**으로 만들고, 동일 표본에서 들어 올림을 잰다.

### 1-3. 이 마일스톤이 주장하지 않는 것

- notes 내용의 **의미적 정확성** 자동 채점
- 이슈 프롬프트를 사람이 풀어 쓰는 방식의 통과율 개선
- pack·토큰 회계만으로 통과율 구원
- M15 0점을 “실패한 실험”으로 폐기 (바닥 좌표로 유지)

---

## 2. 목표 · 성공 기준 · 비범위

### 2-1. 목표

1. **계층 영속 notes** (`.loco/notes/`) — root 라우팅 + 하위 상세, 세션 간 재사용.
2. **소형 모델 thrifty** — 전 트리 덤프 금지, 층별 캡, 전문 매턴 주입 금지.
3. **강제 루프** — mut 전 root+조상 스키마; mut 후 dirty notes면 finish 1회 거부.
4. **지침 위치** — “무엇을 최소로 쓸지”를 거부/툴 설명에 템플릿으로 실어, 빈 notes에서도 행동이 정의되게.
5. **측정** — flag on vs off 재측정으로 장치 효과 분리; 기전 지표 계측.

### 2-2. 성공 기준 (데이터 전 고정)

| 층 | 기준 |
|---|---|
| 구현 게이트 | `cargo test` · clippy `-D warnings` · `eval --verify` tasks 12/12 · tasks-large 3/3 · tasks-real 17/17 |
| 기전 | treatment에서 `notes_mut_gate` / `notes_schema_reject` / `notes_stale_finish` / `notes_updates`가 **관측 가능**(전부 0이면 장치 미작동 → 배치 해석 보류) |
| 최소 들어 올림 | 과제 수준 통과 평균 ≥ **ε = 1/17** **또는** 통과 과제 수 ≥ 1 (treatment, 사전등록과 동일 정의) |
| 비교 | control(flag off) 대비 treatment 방향성 + CI (PROTOCOL 소표본 규칙 준수) |
| 실격 | 전패 과제 수 ≥ M15와 동일 공식 대역(사전등록에 N=17 기준 재기술). **양 암 실격**이면 “온보딩으로 바닥 탈출 실패”로 보고하되 인프라 병합은 별개 |

ε는 **최소 기전 들어 올림**이지 “제품 완성” 임계가 아니다. 더 센 “유용 베이스라인” 대역이 필요하면 사전등록 개정으로만 추가한다 (사후 상향 금지).

### 2-3. 비범위

- RAG / 임베딩 / 외부 인덱서
- 멀티에이전트
- 이슈 본문 재작성 · 경로·해법 힌트 주입
- 모델·양자화 교체
- Timeout 또는 max_turns **단독** 상향을 주 개입 암으로 사용
- notes 본문 LLM-as-judge
- 신규 크레이트 (금지 유지; 표준 라이브러리·기존 의존성만)
- `tasks-real` 이슈 프롬프트·오라클 변경 (표본 동결 유지)

---

## 3. 설계

### 3-1. 저장 레이아웃

프로젝트 루트:

```text
.loco/notes/
  _root.md              # root 층 (게이트 필수)
  src.md                # 디렉 "src"
  src/walk.md           # 더 깊은 디렉 층 (선택)
  tests.md
```

| 규칙 | 내용 |
|---|---|
| SSOT | **디스크 파일**. 세션 메모리는 캐시/접지일 뿐 진실원이 아님 |
| 재사용 | 실사용: 다음 세션이 기존 파일을 읽음. eval: 샌드박스에 파일 없음 → cold start |
| git | `.loco/` 기존 gitignore 관례 유지 → notes는 **로컬 기본**(커밋 공유는 사용자 선택, 하네스 강제 아님) |
| 1차 규약 | 게이트 조상 후보는 **디렉 층** + `_root`. 파일 단위 notes는 허용하되 1차 조상 탐색 단순화를 위해 **필수는 아님** |

**조상 매핑:** 코드 경로 `a/b/c.rs` → notes 키 후보 (구체→상위):

1. `a/b.md` (디렉 `a/b`)
2. `a.md` (디렉 `a`)

`_root`는 조상 체인과 **별도로 항상** 요구된다.  
정규화: `//` 제거, `.`/`..` 거부, notes 루트 밖 escape 거부 (`path::confine` 계열).

### 3-2. 형식 스키마 (의미 검증 없음)

파서는 라인 스캔 상태기계. 합격/불합격만.

#### `_root.md` 합격 조건 (전부)

| 항목 | 규칙 |
|---|---|
| `## summary` | 헤더 존재 · 다음 헤더 전까지 비공백 줄 **1–3** |
| `## routes` | 헤더 존재 · `- <path> → <role>` 형태 bullet **≥ 1** |
| 크기 | **≤ 1200 bytes** (UTF-8). 초과 시 툴이 거부 또는 truncate+재검증(구현: **거부 권고** — 조용한 절단은 지도 손실) |
| soft-reject 휴리스틱 (선택, 1차 권고) | fenced code block ≥ 1 또는 비공백 줄 ≥ 40 → 거부 + “do not dump bodies” |

#### 디렉 층 (`src.md` 등) 합격 조건

| 항목 | 규칙 |
|---|---|
| `## role` | 비공백 ≥ 1줄 |
| 본문 | `## entrypoints` bullet ≥ 1 **또는** `## notes` bullet ≥ 1 |
| 크기 | **≤ 800 bytes** |

스키마 실패 시 툴/게이트 응답 끝에 **§3-4 템플릿**을 붙인다.

### 3-3. 툴

#### `update_repo_notes`

| | |
|---|---|
| args | `path`: notes 키 (`_root` 또는 `src`, `src/walk` … — `.md` 유무 정규화) · `content`: 전체 교체 본문 |
| 동작 | 정규화 → 스키마 검사 → `.loco/notes/<key>.md` 기록 (부모 dir 생성) |
| 성공 | 짧은 OK + 기록 경로 + byte 수. (선택) dirty clear if key matched |
| 실패 | 스키마/경로/캡 사유 + thrifty 템플릿 |
| mut 분류 | **notes 쓰기** — 코드 mut 게이트 **적용 안 함**. 승인 게이트: eval `AutoApprover` 통과. 코드 `edit_file`/`write_file` 게이트와 분리 |
| 레지스트리 | `Registry::guided()`에 추가 (finish는 기존처럼 루프 전용) |

**의도적 제외 (1차):** `list_repo_notes` 전용 툴 — `list_files` on `.loco/notes`로 대체 (YAGNI).

**우회 방지:** 코드 mut 게이트 통과 조건의 “스키마 OK notes”는 **`update_repo_notes` 성공으로 기록된 파일** 또는 디스크 파일을 **동일 파서로 재검증**한 결과.  
`edit_file`/`write_file`로 `.loco/notes/**`를 직접 고치면: (A) 금지하고 notes 툴로 안내, 또는 (B) 기록 후 동일 스키마 검사 실패 시 에러. **권고 (A)** — 스키마 경로 단일화.

### 3-4. Thrifty 템플릿 (지침의 거처)

SYSTEM_PROMPT에는 **2–3문장 포인터만**:

> Maintain hierarchical repo notes under `.loco/notes/` via `update_repo_notes`. Root holds a short summary and routes; deeper dirs hold role/entrypoints. Do not paste file bodies, test logs, or issue text. Fill notes before editing code; update notes after successful edits.

**Root 템플릿** (스키마 실패·mut 게이트 거부 body에 그대로):

```markdown
## summary
(1-3 lines: what this repo/binary is)

## routes
- src/cli.rs → CLI flags
- src/walk.rs → directory walk
(only high-traffic paths; one line each)

## do_not
- paste issue text, test names, diffs, or full file bodies
```

**모듈 템플릿:**

```markdown
## role
(one line: what this directory owns)

## entrypoints
- SymbolOrFile — one-line why (max 5)

## notes
- optional convention bullets (max 3)
```

토큰 전략: 매 턴 장문 SYSTEM 대신 **실패 시에만** 템플릿 전문을 실어, “비었을 때 무엇을 쓸지”가 정의되게 한다.

### 3-5. Mut 게이트 (코드 편집)

대상: `edit_file` / `write_file` whose path is **not** under `.loco/notes/` (notes 직접 write는 §3-3에서 별도 처리·권고 금지).

디스패치 전(승인 게이트/preview 전후 위치는 구현 시 기존 salvage reverse-rule 패턴에 맞춤; **승인 전에 거부**해 불필요한 preview 방지 권고):

1. `_root.md` 존재 ∧ 스키마 OK? → 아니면 거부 + root 템플릿  
2. 편집 경로의 조상 notes 키 중 **스키마 OK ≥ 1**? → 아니면 거부 + 모듈 템플릿 + “write notes for one of: …”  
3. 통과 시 기존 승인·dispatch

Once-latch 아님 — 조건 불충족 시 **매번** 거부 (쓰레기 root 방치 후 통과 방지).  
단, 동일 턴 스팸 방지용 로그 카운터는 계측만.

### 3-6. Mut 후 갱신 유도 (stale finish)

런 스코프 상태:

| 필드 | 의미 |
|---|---|
| `notes_dirty: BTreeSet<NotesKey>` | 성공 코드 mut 후, 그 경로의 **가장 구체적 조상 키**(없으면 가장 가까운 상위 후보 키를 dirty 표시 — 파일이 없어도 키를 넣어 갱신 유도) |
| `notes_stale_nudged: bool` | finish 거부 1회 latch |

**성공 코드 mut 시:** dirty에 조상 키 insert.  
**성공 `update_repo_notes` on key (스키마 OK):** 해당 키 remove.  
**`finish` with summary:** `notes_dirty` 비어 있지 않고 `!notes_stale_nudged` → finish 거절, `NOTES_STALE_NUDGE` 1회, latch true. dirty여도 두 번째 finish는 통과 (무한 루프 방지; VERIFY_NUDGE와 동일 철학).

우선순위 (같은 턴 충돌 시 위에서 이김):  
`RepetitionStop` > 기존 finish/verify latches 문서화 순서에 **NOTES_STALE을 VERIFY_NUDGE와 병기** — 구현 시 `agent/mod.rs` finish 분기에 명시적 순서 테이블을 코드 주석+테스트로 고정.

### 3-7. 접지 · pack · 토큰

| 규칙 | 내용 |
|---|---|
| 매 턴 전문 주입 | **금지** |
| 접지 시점 | notes 갱신 성공 직후 및/또는 mut 게이트 거부 직후; 선택적으로 status cadence에 root summary **1줄** + routes 개수 |
| 마커 | `[repo_notes] ` 접두 (status의 `[status] `와 분리). keep-latest strip (`session` 패턴 재사용) |
| pack | 디스크 SSOT. 접지 블록은 tool_result 접미사 strip 대상. **과제 user 메시지 삭제 금지**(M13 교훈) |
| 컨텍스트 | 1차 암 예산 고정; notes 캡이 1차 토큰 방어선 |

### 3-8. Feature flag

| 항목 | 내용 |
|---|---|
| config 키 | `repo_notes` (bool). `deny_unknown_fields` 유지 → 키 추가 시 config 스키마·문서 동시 갱신 |
| 기본값 | **제품 REPL: true 권고**; 구현 PR에서 확정. **`tasks/`·`tasks-large` eval: 게이트 no-op 또는 false** 로 기존 게이트 비교 유지 |
| tasks-real 실험 | 암별로 설정 스냅샷을 `report.json` / effective_config에 기록 (M15 H9 규율) |
| control | 동일 바이너리 · `repo_notes=false` · **재측정** |

flag off 시: 툴은 레지스트리에 **없을 수도 / no-op 안내** 중 하나를 스펙에 하나로 고정.  
**권고:** flag off면 툴 미등록 + 게이트·stale 비활성 (모델이 유령 툴을 부르지 않게).

---

## 4. 기존 장치 연동

| 장치 | 연동 |
|---|---|
| `StatusNote` | 별 마커; status 검증 줄과 병합하지 않음 |
| `VERIFY_NUDGE` / `FINISH_NUDGE` | 독립. 코드 검증 신호와 notes stale은 직교 |
| `RepetitionTracker` | notes 툴 반복도 window에 포함 (기존 규율) |
| protected / `sync_protected` | `.loco/notes`를 protected로 넣어 지우지 말 것. 보상 해킹 카운터(H7)에 notes 쓰기 포함 여부: **제외** (지도 작성을 해킹으로 세지 않음) |
| `AutoApprover` | notes 툴 자동 승인 |
| leak audit / 이슈 프롬프트 | **불변**. notes에 오라클을 하네스가 넣지 않음 |

### 4-1. 누설 정책

배치 후(또는 exp_metrics): notes 텍스트와 `oracle_files` basename 교집합 →  
`notes_oracle_overlap` 라벨. **런 자동 실격 아님** — 분석 층.  
사전등록에 “라벨 정의”만 고정.

---

## 5. 측정 프로토콜

### 5-1. 고정 조건

| 항목 | 값 |
|---|---|
| 표본 | M15 동결 N=17 (`frozen-sample.md`) |
| 반복 | ×3 seed 0 → 51런/암 |
| 모델 | ornith-1.0-9b Q4_K_M |
| 운용 / 로드 | 32768 / 37632 (개정 A 정렬) |
| max_turns / timeout | 25 / 600 |
| 프롬프트 | 이슈 본문 only (M15와 동일) |

### 5-2. 암

1. **control** — `repo_notes=false`  
2. **treatment** — `repo_notes=true` (본 설계 전체)

GPU 배치 전 **PROTOCOL 사전등록 필수**. 재측정 횟수 공약 명시.

### 5-3. 지표 (exp_metrics)

신규 마커/컬럼 (이름은 구현 시 Rust 상수와 **문자 일치**, selftest 의무):

| 이름 | 출처 |
|---|---|
| `notes_schema_reject` | 스키마 실패 툴 결과 부분문자열 |
| `notes_mut_gate` | mut 게이트 거부 문구 |
| `notes_stale_finish` | `NOTES_STALE_NUDGE` |
| `notes_updates` | 성공 update 횟수 |
| `notes_bytes_max` | 런 중 최대 notes 총 bytes (선택) |
| 기존 | first_mut_turn, nav_hit, fix_hit, pack_*, outcome, protected_edits |

### 5-4. 판정

§2-2. 사후 ε 변경 금지.  
control 실격·treatment 실격 각각의 처분을 사전등록 표로 고정 (M15 A5 정신: 실격 ≠ 인프라 실패).

### 5-5. 회귀

`tasks/` spot 또는 full 36 및 `tasks-large` — flag off 경로에서 M14/M15 관측과 **비교 가능한** 설정.  
온보딩 코드가 공용 루프를 건드리므로 control 재측정이 기본이다.

---

## 6. 리스크 · 풍선 · 대안

| 리스크 | 완화 |
|---|---|
| 스키마 우회 (최소 bullet 쓰레기) | soft-reject 줄 수/펜스; 기전 지표로 “형식만 통과” 관측; 의미 검증은 비범위 |
| Timeout 증가 (notes 작성 턴) | 1차 예산 고정으로 **장치 비용이 통과율에 미치는 영향**을 정직히 측정; T+는 별 암 |
| finish 1회 후 dirty 방치 | once-latch 의도적; 갱신률 지표로 감시 |
| notes 컨텍스트 팽창 | 층별 캡 + 전문 비주입 |
| tasks/ 회귀 | flag off / 툴 미등록 |
| 조상 매핑 애매 | 스펙 §3-1 규칙 + 유닛 테스트 고정 벡터 |

**기각 대안 요약:** B3 phased, 단일 파일 섹션, 세션-only, presence-only 게이트, M15 스탬프 control, C 자동 온보딩(1차).

---

## 7. Key Decisions

1. **계층 디스크 notes** — 재활용·thrifty 동시 만족; 단일 블롭 컨텍스트 기각.  
2. **스키마 = 형식 계약** — 의미 채점 없이 우회·지침 공백을 줄임.  
3. **root + 조상 게이트** — root-only 우회와 full-tree 강제 사이의 절충.  
4. **stale finish 1회** — “있고 끝” 방지; 무한 finish 루프 방지.  
5. **지침은 거부 경로** — 빈 notes 행동 정의를 고정비 없이.  
6. **flag + 재측정** — M15 스탬프 control 오염 방지.  
7. **예산 고정 1차 암** — 장치 vs 예산 교락 금지.

---

## 8. Open Questions (스펙 리뷰에서 닫을 잔여)

구현 전 사용자 확인이 있으면 좋은 항목 (기본 권고 있음):

| # | 질문 | 기본 권고 |
|---|---|---|
| O1 | REPL 기본 `repo_notes` true? | **true** |
| O2 | notes 경로를 `.loco/notes` 대신 저장소 루트 `NOTES/` 등 공개 경로? | **`.loco/notes`** (gitignore·제품 로컬) |
| O3 | soft-reject(긴 덤프) 1차에 포함? | **포함** (줄≥40 또는 fence) |
| O4 | ε를 1/17이 아니라 더 높게? | **1/17 유지** (최소 들어 올림); 상향은 사전등록 개정 |

이 네 항목은 기본 권고로 구현 가능. 사용자가 덮어쓰면 개정 한 줄.

---

## 9. PR Plan

| PR | 제목 | 내용 | 의존 |
|---|---|---|---|
| PR1 | `feat(notes): schema parser + path mapping + unit tests` | 순수 파서·조상 키·캡·템플릿 상수. 에이전트 루프 미연결 | — |
| PR2 | `feat(tools): update_repo_notes tool` | 툴 등록, 디스크 I/O, confine, flag off 시 미등록 | PR1 |
| PR3 | `feat(agent): mut gate + notes dirty + NOTES_STALE_NUDGE` | 게이트·finish 연동·우선순위 테스트 | PR2 |
| PR4 | `feat(session): optional repo_notes grounding strip` | 짧은 접지 + keep-latest | PR3 |
| PR5 | `feat(config): repo_notes flag + effective_config snapshot` | TOML · report 기록 | PR2–3 |
| PR6 | `feat(metrics): exp_metrics notes columns + selftest` | 마커/컬럼 | PR3 |
| PR7 | `docs(m16): pre-registration + CLAUDE.md` | 실험 사전등록·커맨드 문서 | PR5–6 |
| PR8 | `chore(m16): control/treatment batch` (GPU) | 사전등록 승인 후 러너 | PR7 |

각 PR은 `cargo test` · clippy · 해당 verify를 독립 통과 가능하게 유지.

---

## 10. 구현 착수 전 체크

- [ ] 사용자 스펙 리뷰 승인 (본 문서)
- [ ] Open Questions O1–O4 묵시 수락 또는 개정
- [ ] writing-plans → 구현 플랜
- [ ] 구현 후 PROTOCOL 사전등록 → GPU 배치

---

## 11. 변경 이력

| 날짜 | 내용 |
|---|---|
| 2026-07-21 | brainstorm 합의 초안: 계층 notes, 스키마+조상 게이트, stale finish, thrifty 템플릿, flag 재측정 |
