# M16 설계 — 계층 레포 notes 온보딩 하네스

- 상태: **개정 2 — 2R Ready: Yes (Critical 0 · Important 0) · 플랜 작성 가능**
- 작성일: 2026-07-21
- 기준 커밋: `232e551` (main, M15 병합) · 설계 초안 `7f525bd`
- 리뷰:
  - 1R `…-review-1.md` — Ready: No · C0 · I8
  - 2R `…-review-2.md` — **Ready: Yes** · C0 · I0 · Minor 4 반영(개정 2)
- 선행 문서:
  - `docs/m16-candidates.md`
  - `docs/baselines.md` "M15" · `docs/experiments/2026-07-20-m15-real-repo-baseline/report.md`
  - M15 스펙 `docs/superpowers/specs/2026-07-20-m15-real-repo-track-design.md`
  - `docs/experiments/PROTOCOL.md`
- 스코핑: 2026-07-21 brainstorm (superpowers) — 사용자 결정 요약 §0

## 0. 스코핑 경위 (사용자 결정)

| # | 결정 | 내용 |
|---|---|---|
| 1 | 본선 형태 | **B2 hard gate** (전용 notes 툴 + mut 전 조건). B3/C/D/E 1차 제외 |
| 2 | 수명 | **프로젝트 디스크 영속**. eval 샌드박스는 fixture에 notes 없어 **런마다 cold start** |
| 3 | 구조 | **계층 notes**: root = 요약+라우팅; 하위 = 상세. 전 notes 매턴 주입 금지 |
| 4 | mut 게이트 | **root 스키마 OK ∧ (조상 notes 스키마 OK ≥1 ∨ 루트 파일 편집)** — §3-5 |
| 5 | 품질·갱신 | **형식 스키마** + **성공 코드 mut 후 dirty → finish 1회 거부**. thrifty **전문 템플릿은 거부 body만** (툴 `doc()`·SYSTEM 장문 금지) — §3-4 |
| 6 | 측정 | **feature flag on/off + control 재측정**. M15 0/51 스탬프 control 인용 금지 |
| 7 | 1차 예산 | 양 암 **max_turns=25 · timeout_secs=600** |

**사용자 문제 제기:** “있음/없음만 보면 갱신이 안 되고, 비었을 때 thrifty로 뭘 쓸지 지침이 없다.”  
→ §3 스키마·거부 템플릿·stale-finish.

---

## 1. 배경

### 1-1. M15가 고정한 바닥

`tasks-real` N=17 · ×3 · seed 0 · 운용 32768 · 로드 37632 · 이슈 본문 only:

| 지표 | 값 |
|---|---|
| 통과 | **0/51** (엄격 0 · ff 1) |
| 전패 | 17/17 → **실격** (양의 베이스라인 실패 · M16 대조 비인용) |
| outcome | max_turns 27 · timeout 18 · repetition_stop 5 · finished 1 |
| fail-층 nav_hit / fix_hit | ≈0.75 / ≈0.22 |

해석: `--verify` 결함 아님 · **cold start 수선·완주 실패**. `tasks/`와 나란히 비교 금지.

### 1-2. 제품 직관과 평가의 간극

실사용 루프는 **탐색 → 요약 지도 → 작업**. M15는 1단계를 강제하지 않는 가혹 조건.  
M16은 지도를 **하네스 계약**으로 만들고 동일 표본에서 들어 올림을 잰다.

### 1-3. 주장하지 않는 것

notes 의미 채점 · 이슈 풀어쓰기 · pack만으로 통과율 구원 · M15 0점 폐기.

---

## 2. 목표 · 성공 기준 · 비범위

### 2-1. 목표

1. 계층 영속 notes (`.loco/notes/`) · 세션 간 재사용  
2. thrifty: 층별 캡 · 전문 매턴 주입 금지 · 템플릿은 실패 경로  
3. mut 전 root+조상(또는 루트 파일) 스키마 · mut 후 stale-finish 1회  
4. flag on/off 재측정 · 기전 지표 계측  

### 2-2. 성공 기준 (데이터 전 고정 · 단일 1차 기준)

**정의 (N=17, repeats=3, 총 51런/암):**

| 기호 | 정의 |
|---|---|
| `passed_count_i` | 과제 i의 통과 런 수 (0..3) |
| `task_mean_pass` | `mean_i (passed_count_i / 3)` — M15 pool과 동일 계열 |
| `tasks_with_any_pass` | `|{ i : passed_count_i ≥ 1 }|` |

| 층 | 기준 (treatment 암) |
|---|---|
| 구현 게이트 | `cargo test` · clippy `-D warnings` · verify 12/12 · 3/3 · 17/17 |
| **1차 최소 들어 올림** | **`task_mean_pass ≥ ε` with `ε = 1/17`**  
| 2차 보고 (판정 아님) | `tasks_with_any_pass` · 엄격 · false_finish · control 대비 Δ |
| 기전 생존 | §5-3: treatment에서 **mechanism-alive** 조건 충족 (아니면 배치 해석 보류) |
| 실격 (암 독립) | N=17에서 **전패 과제 ≥ 13** 또는 **전승 과제 ≥ 13**  
  (M15 절대값: `0.98·√N` 휴리스틱과 동일한 사전등록 재기술). 실격 ≠ 인프라 실패 |

`ε = 1/17` ⇔ 대략 **총 통과 런 ≥ 3 / 51** (과제 균등 시).  
**“통과 과제 ≥ 1”을 1차 OR로 두지 않는다** — 1/3 한 방이 성공이 되는 약화를 막기 위함.  
`tasks_with_any_pass`는 기전 서사 보고용.

**ε와 실격은 독립 라벨이다.** `task_mean_pass ≥ ε`여도 전패 과제 ≥13이면 그 암은 여전히 실격로 보고한다 — ε만으로 DQ를 지우지 않는다 (사전등록에 동일 문장).

ε·실격 상향은 **사전등록 개정만** (사후 금지).

### 2-3. 비범위

RAG · 멀티에이전트 · 이슈 재작성 · 모델 교체 · 예산 단독 상향 주 암 · notes LLM judge · 신규 크레이트 · `tasks-real` 프롬프트/오라클 변경.

---

## 3. 설계

### 3-1. 저장 레이아웃

```text
.loco/notes/
  _root.md
  src.md
  src/walk.md
  tests.md
```

| 규칙 | 내용 |
|---|---|
| SSOT | 디스크. 세션 접지는 파생 |
| 재사용 | 실사용 세션 간 유지 · eval cold start |
| git | `.loco/` gitignore 관례 — 로컬 기본 |
| 게이트 후보 | **디렉 층** + `_root`. 파일 단위 notes 비필수 |

**조상 키 매핑** (코드 상대경로 `P`, 프로젝트 루트 기준):

1. `P`의 부모 디렉이 루트이면 → 디렉 조상 집합 **∅** (루트 파일 — §3-5 특례)  
2. 아니면 부모 디렉 `D0`, 그 상위 `D1`… 각각 notes 키 `D0`, `D1`… (`/` 유지, `.md` 없이 정규화 후 저장 시 `.md` 부여)

**정규화 거부:** `.` / `..` 세그먼트, notes 루트 밖 escape, NUL.  
**허용 정규화:** `//` 축약, 선두 `./` 제거, Windows `\` → `/`, 키 끝 `.md` strip, `_root` ≡ `root` 거부하고 **`_root`만** root 키.

**매핑 벡터 (PR1 필수 테스트):**

| 코드 경로 | 게이트 조상 키 (구체→상위) | dirty 키 | root 필수 |
|---|---|---|---|
| `Cargo.toml` | ∅ → **root-only 특례** | `_root` | yes |
| `build.rs` | ∅ → root-only | `_root` | yes |
| `src/main.rs` | `src` | `src` | yes |
| `src/exec/job.rs` | `src/exec`, `src` | `src/exec` | yes |
| `crates/core/app.rs` | `crates/core`, `crates` | `crates/core` | yes |

### 3-2. 형식 스키마 (의미 검증 없음)

라인 스캔. **1차 soft-reject 포함 (고정, O3 닫힘).**

#### `_root.md` 합격 (전부)

| 항목 | 규칙 |
|---|---|
| `## summary` | 다음 `##` 전까지 비공백 줄 **1–3** |
| `## routes` | `- <path> → <role>` bullet **≥ 1** |
| 크기 | **≤ 1200 bytes** — 초과 시 **거부** (조용한 truncate 금지) |
| soft-reject | fenced code(```) **≥ 1** 또는 비공백 줄 **≥ 40** → 거부 |

#### 디렉 층 합격

| 항목 | 규칙 |
|---|---|
| `## role` | 비공백 ≥ 1줄 |
| 본문 | `## entrypoints` bullet ≥ 1 **또는** `## notes` bullet ≥ 1 |
| 크기 | **≤ 800 bytes** |
| soft-reject | fence ≥ 1 또는 비공백 줄 ≥ 40 |

**추가 `##` 섹션** (예: `## do_not`)은 크기·soft-reject만 통과하면 **허용**.

스키마 실패 응답 끝에 §3-4 템플릿 전문.

### 3-3. 툴 `update_repo_notes`

| | |
|---|---|
| args | `path` (notes 키) · `content` (전체 교체) |
| 동작 | 정규화 → 스키마 → `.loco/notes/<key>.md` 기록 · **certified set**에 키 추가 (§3-5) |
| 성공 body (계측 고정) | 접두 **`repo notes updated:`** + 경로 + bytes  
  상수명 예: `NOTES_UPDATE_OK_PREFIX` |
| 실패 | 사유 + thrifty 템플릿 |
| `is_mutating()` | **`true`** — TtyApprover 확인 · AutoApprover 통과 |
| 검증 상태 | **`mutated_since_verify` / StatusNote mutation / FINISH_NUDGE mutation 이벤트는 `edit_file`\|`write_file` 화이트리스트만**.  
  notes 성공 디스패치는 VERIFY를 **재무장하지 않음** (I1). 단위 테스트 의무 |
| 코드 mut 게이트 | notes 툴 자체에는 **미적용** |
| 레지스트리 | `repo_notes=true`일 때만 `Registry::guided(&cfg)` (또는 동등)에 등록 |

**금지:** `edit_file` / `write_file` 대상 경로가 `.loco/notes/**` → 즉시 에러, `update_repo_notes` 안내 (옵션 A 확정).

**off-tool 잔여:** `run_command`로 파일을 쓰는 것은 막지 않음. 게이트는 **certified set**만 신뢰 (§3-5).  
메트릭: `notes_offtool` 라벨 — 디스크에 스키마 OK notes가 있으나 런 중 `notes_updates==0` (휴리스틱, 분석 층).

`list_repo_notes` 전용 툴 **없음** (YAGNI).

### 3-4. Thrifty 지침 표면 (flag 행렬 · I2)

| 표면 | `repo_notes=true` | `repo_notes=false` (control) |
|---|---|---|
| SYSTEM 포인터 (2–3문장) | **예** | **아니오** (control 순수) |
| 툴 등록 + 스키마 enum | **예** | **아니오** |
| tool `doc()` | **짧은 시그니처 ≤ ~2줄** · 템플릿 전문 **금지** | n/a |
| root/모듈 템플릿 전문 | 스키마 실패 · mut-gate 거부 body **만** | n/a |
| mut gate / NOTES_STALE | on | **off** |
| `[repo_notes]` 접지 | on (PR4, 효과 주장 밖) | off |

SYSTEM 포인터 예시 (flag on only):

> Maintain hierarchical repo notes under `.loco/notes/` via `update_repo_notes`. Keep root short (summary + routes); deeper dirs hold role/entrypoints. Do not paste file bodies, test logs, or issue text.

**Root / 모듈 템플릿** — 거부 body 전용 (SYSTEM·doc에 넣지 않음):

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

```markdown
## role
(one line: what this directory owns)

## entrypoints
- SymbolOrFile — one-line why (max 5)

## notes
- optional convention bullets (max 3)
```

### 3-5. Mut 게이트 (코드 편집) · certified set (I8)

**대상:** `edit_file` / `write_file` and path ∉ `.loco/notes/**`.

**런 스코프 `certified: BTreeSet<NotesKey>`:**

1. **세션/런 시작:** 디스크 `.loco/notes/**`를 스캔, 스키마 OK인 키를 certified에 넣음 (실사용 재사용 · eval은 보통 공집합).  
2. **성공 `update_repo_notes`:** 키 insert.  
3. 게이트는 **디스크 재파싱만으로 통과시키지 않음** — 반드시 `key ∈ certified`.

**게이트 조건 (승인/preview 전 거부 권고):**

1. `_root` ∈ certified (스키마는 cert 시점·갱신 시 이미 OK).  
2. **디렉 조상:** 조상 키 중 certified ≥ 1  
   **또는 루트 파일 특례:** 편집 경로의 부모가 프로젝트 루트이면 조건 2를 `_root`만으로 충족 (I6).  
3. 실패 시 거부 문구 접두 **`repo notes mut gate:`** + 템플릿  
   상수: `NOTES_MUT_GATE_MARK`

조건 불충족 시 **매번** 거부 (once-latch 아님).

**cert 재검증:** 1차는 start-scan + tool 성공 시점 파서만. mid-run `run_command`가 certified 파일을 덮어써도 키는 남는다 — shell로 *신규 cert를 얻는* 것은 불가. mtime 재파싱은 M16 1차 밖 (§6).

### 3-6. Mut 후 갱신 유도 · finish 순서 (I4)

| 필드 | 의미 |
|---|---|
| `notes_dirty: BTreeSet<NotesKey>` | 성공 코드 mut 시 dirty 키 insert |
| `notes_stale_nudged: bool` | NOTES_STALE once-latch |

**dirty 키 규칙:** 편집 경로의 **가장 구체적 디렉 조상 키**. 루트 파일이면 **`_root`**.  
파일이 아직 없어도 키를 넣음 (갱신 유도).

**clear:** 성공 `update_repo_notes` on **그 exact 키** → remove.  
(더 얕은 조상 갱신만으로는 clear 안 됨 — 의도적 단순화. once-latch로 불완전 갱신 허용.)

**NOTES_STALE_NUDGE 본문 (상수, 전문 첫 줄 고정):**

```text
repo notes stale: you edited code but did not update notes for: {keys}. Call update_repo_notes on each listed key, then finish.
```

`exp_metrics` 매칭 접두: **`repo notes stale:`** (상수 `NOTES_STALE_MARK` / 본문 상수 `NOTES_STALE_NUDGE`).

**summary 있는 `finish` 처리 순서 (고정):**

1. 기존 RepetitionStop 등 루프 상단 종료  
2. **VERIFY_NUDGE / VERIFY_NUDGE_PIPE** (코드 미검증 · once) — `passed` 직결  
3. **NOTES_STALE_NUDGE** (`notes_dirty` 비어 있지 않고 `!notes_stale_nudged` · once)  
4. finish 수락  

**통합 테스트 고정 시나리오:**

- **A (미검증+dirty):** code mut → finish → VERIFY → finish(검증 없이) → STALE → finish → accept  
- **B (검증 후 dirty만):** code mut → run_command 성공(verify clear) → finish → STALE only → finish → accept  

notes dirty와 VERIFY는 **직교** (I1 화이트리스트 전제).

### 3-7. 접지 · pack

| 규칙 | 내용 |
|---|---|
| 매 턴 전문 | 금지 |
| 접지 | 선택(PR4) · 효과 주장 밖 · 갱신/거부 직후 짧은 블록 |
| 마커 | `[repo_notes] ` · keep-latest strip |
| pack | 디스크 SSOT · 과제 메시지 삭제 금지 |

### 3-8. Feature flag · eval 계약 (I5)

| 항목 | 계약 |
|---|---|
| 키 | `repo_notes: bool` (`deny_unknown_fields`) |
| REPL 기본 | **`true`** (O1 닫힘) |
| flag off | 툴 **미등록** · SYSTEM 포인터 **없음** · 게이트/stale **off** · **silent no-op 모드 없음** |
| `tasks/` · `tasks-large` 회귀 | 반드시 **`repo_notes=false`** (CLAUDE.md + 사전등록 명시) |
| `tasks-real` 실험 | 러너가 암별 `.loco/config.toml` (또는 동등)로 고정 · **EffectiveConfig에 `repo_notes` 스냅샷** |
| 등록 API | `Registry::guided`가 config를 받거나 Agent 생성 시 툴셋 분기 — **툴 상륙 PR에 flag 배선 동봉** (PR 합침 §9) |

control = 동일 바이너리 · `repo_notes=false` · **재측정**.

---

## 4. 기존 장치 연동

| 장치 | 연동 |
|---|---|
| `StatusNote` | 별 마커 · 검증 줄 비병합 |
| VERIFY / FINISH_NUDGE | §3-6 순서 · notes는 mutation whitelist 밖 |
| `RepetitionTracker` | notes 툴 포함 |
| **protected / H7** | **`.loco/notes`를 task `protected` 및 암시적 protected 집합에 넣지 않는다.**  
  `sync_protected`는 fixture에 없는 notes를 **복원·삭제하지 않음**(목록에 없어서).  
  H7은 protected 경로만 세므로 notes 쓰기는 **자동 제외** — 별도 H7 예외 코드 불필요. notes는 `passed`에 영향 없음 (I7) |
| `AutoApprover` | notes `is_mutating` true → 자동 승인 |
| 이슈/leak | 불변 · 하네스 notes 시드 금지 |

### 4-1. 누설

notes ∩ oracle basename → `notes_oracle_overlap` 분석 라벨 · **자동 실격 아님**.

---

## 5. 측정 프로토콜

### 5-1. 고정 조건

표본 N=17 · ×3 seed 0 · ornith · 32768/37632 · max_turns 25 · timeout 600 · 이슈 only.

### 5-2. 암

1. control — `repo_notes=false`  
2. treatment — `repo_notes=true`  

사전등록 필수 · 재측정 공약.

### 5-3. 지표 · 마커 문자열 계약 (I10)

Rust 상수와 `exp_metrics.MARKS` **문자 일치** · `--selftest` 의무.

| 컬럼 | 매칭 문자열 (초안, 구현 시 상수화) |
|---|---|
| `notes_schema_reject` | `repo notes schema:` |
| `notes_mut_gate` | `repo notes mut gate:` |
| `notes_stale_finish` | `repo notes stale:` |
| `notes_updates` | `repo notes updated:` |
| `notes_bytes_max` | **필수** 컬럼 (flag off 런은 `-`). 출처: 성공 `update_repo_notes` 및 start-scan 시 `session` transcript **extra**  
  `notes_bytes_total` (런 중 합 또는 max — 구현은 **max over keys of file len after last cert write**, 컬럼명 `notes_bytes_max`)를 기록하고 exp_metrics가 extras/툴 성공 줄에서 산출 |
| `notes_offtool` | process() 휴리스틱 라벨 (선택 출력) |

**mechanism-alive (treatment):**  
`notes_updates > 0` **또는** (`notes_mut_gate + notes_schema_reject + notes_stale_finish > 0`).  
전부 0이고 cert도 공집합에 가깝면 **장치 미작동 → 해석 보류**.  
(디스크만 있고 updates=0이면 `notes_offtool` 경고.)

기존: first_mut_turn (edit/write only 유지), nav_hit, fix_hit, pack_*, outcome, protected_edits.

### 5-4. 판정

§2-2. 암별 실격 독립. 사후 ε 변경 금지.

### 5-5. 회귀

합성·large 배치는 **`repo_notes=false` 고정**. treatment 코드 경로와 분리.

---

## 6. 리스크 · 풍선

| 리스크 | 완화 |
|---|---|
| VERIFY 재무장 (notes mut) | whitelist (I1) |
| control SYSTEM 오염 | flag 행렬 (I2) |
| 스키마 쓰레기 | soft-reject 고정 |
| Timeout↑ | 예산 고정으로 정직 측정 |
| shell off-tool | certified + offtool 라벨; mid-run overwrite는 cert 유지(1차 잔여) |
| 깊은 dirty 미갱신 | exact-key clear + once-latch |
| tasks/ 비교 붕괴 | false 강제 · CLAUDE.md; (선택) non-tasks-real + true 시 stderr 한 줄 경고 |

---

## 7. Key Decisions

1. 계층 디스크 notes · thrifty  
2. 스키마 = 형식 · soft-reject 1차 포함  
3. certified set 게이트 · root-file 특례  
4. `is_mutating` true + VERIFY whitelist  
5. finish 순서: VERIFY → NOTES_STALE  
6. 템플릿 = 거부 body only · control에 SYSTEM 포인터 없음  
7. 1차 성공 = `task_mean_pass ≥ 1/17` only  
8. flag: silent no-op 없음 · 회귀 false  
9. protected에 notes 넣지 않음  
10. 예산 고정 · M15 스탬프 control 금지  

---

## 8. Open Questions

1R로 **O1(true)·O2(.loco/notes)·O3(soft-reject 포함)·O4(ε=1/17)** 기본을 스펙 본문에 고정했다.  
잔여 사용자 덮어쓰기만 가능 — 별도 미결 표 없음.

---

## 9. PR Plan

| PR | 제목 | 내용 | 의존 |
|---|---|---|---|
| PR1 | `feat(notes): schema + path map + templates + tests` | 파서·매핑 벡터·캡·soft-reject·템플릿 상수 | — |
| PR2 | `feat(notes): tool + config flag + registry wiring` | `update_repo_notes` · `repo_notes` · guided(&cfg) · EffectiveConfig · **default true / eval false 문서** | PR1 |
| PR3 | `feat(agent): certified gate + dirty + finish order` | mut gate · stale · VERIFY whitelist · 통합 테스트 | PR2 |
| PR4 | `feat(session): optional [repo_notes] grounding` | keep-latest · **효과 주장 밖** | PR3 |
| PR5 | `feat(metrics): notes columns + selftest` | 마커 문자 계약 · notes_bytes_max | PR3 |
| PR6 | `docs(m16): pre-registration + CLAUDE.md` | 실험·회귀 false | PR2–5 |
| PR7 | `chore(m16): control/treatment GPU batch` | 사전등록 후 | PR6 |

(초안 8PR → 툴+flag 합쳐 7PR. grounding은 선택·비주장.)

---

## 10. 구현 착수 전 체크

- [ ] 1R 재심사 Ready: Yes  
- [ ] 사용자 스펙 승인  
- [ ] writing-plans  
- [ ] 사전등록 → GPU  

---

## 11. 변경 이력

| 날짜 | 내용 |
|---|---|
| 2026-07-21 | brainstorm 초안 |
| 2026-07-21 | **개정 1** — 1R I1–I14: VERIFY whitelist·flag 행렬·ε 단일화·finish 순서·eval false 전용·root-file·protected 부정·certified set·soft-reject 고정·마커 문자열·PR 합침 |
| 2026-07-21 | **개정 2** — 2R Ready: Yes. STALE 본문·notes_bytes_max 출처·ε⊥DQ·cert mid-run 잔여·테스트 A/B 고정 |
