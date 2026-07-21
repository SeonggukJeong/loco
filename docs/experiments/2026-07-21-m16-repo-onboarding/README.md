# M16 레포 온보딩 (계층 notes) — 실험 자리

| | |
|---|---|
| **한 줄** | `repo_notes=true` treatment 경로를 스모크로 본 뒤, **길이/경로/검증 하네스 패치**까지 연쇄 시도 |
| **성격** | 개정 B 스모크 + **진단 패치 루프** · ε/Δ·전량 102런 **아님** · 통계 인용 금지 |
| **상태** | **세션 종료 (2026-07-21)** · `fd-1873-path-sep` 반복 실패 · 코드 패치는 main에 있음 · 다음 세션은 **다른 축** 권장 |
| **스택** | llama.cpp · ornith-1.0-9b · `context=8192` · `max_output=4096` · `max_turns=50` · `timeout-scale 3` · port 8080 |
| **최종 HEAD** | `84ed9a9` (push 시점 기준 main tip — 아래 커밋 표 참고) |

---

## 스펙 근거

- 설계: `docs/superpowers/specs/2026-07-21-m16-repo-onboarding-design.md` §5
- 사전등록: [`pre-registration.md`](./pre-registration.md) — **§0-B** (treatment 최소 스모크)
- 배경 바닥: M15 0/51 DQ (스모크 수치를 **배치 통과율처럼 인용하지 말 것**)

---

## 한 페이지 요약 (다른 사람용)

### 무엇을 하려 했나

1. M16 notes 온보딩 하네스가 `repo_notes=true`에서 **죽지 않고** 도는지 (mechanism-alive).
2. 실레포 스모크 과제 `tasks-real/fd-1873-path-sep` 에서 모델이 버그를 고치는지 (보너스, 게이트 아님).

### 실제로 관측된 실패 사다리 (자주 이 순서로 막힘)

```
탐색 중 JSON stutter (같은 턴 × N → length 4096)
    → (패치 후) salvage / max_tokens 캡으로 벽시계는 줄음
notes path 오인 (.loco/notes → 중첩 파일) 또는 mut-gate / notes thrash
    → (패치 후) `_root` 매핑·thrifty 교정으로 일부 완화
검증: cargo test | tail 파이프 + 실패 테스트 이름 무시 + 같은 offset 재read
    → B1/B2 노트는 발화 가능하나, 그 전에 notes/rep_stop으로 끝나는 런이 많음
```

**핵심 교훈:** notes 온보딩 “품질”을 재기 전에, 이 모델+스택에서는 **출력 stutter · notes 경로/게이트 thrash · 파이프 검증**이 먼저 런을 죽인다. 스모크 0/1 연속은 “notes가 쓸모없다”의 증거가 아니라 **앞단 하네스/디코딩 병목** 증거에 가깝다.

### 무엇을 고쳤나 (코드 — main)

| commit | 한 줄 |
|---|---|
| `3853a33` | `path=.loco/notes/_root` 접두 strip |
| `b3e3af1` | thrifty schema reject + 2-streak notes 교정 |
| `ccce7c1` | thrifty mut-gate + brevity SYSTEM + notes length recovery |
| `e4c3b54` | B1/B2: 실패 테스트 grep 항해 · 파이프 검증 힌트 (**cargo/libtest 이름 없이**) |
| `d12756d` | length 시 **첫 complete JSON salvage** |
| `14c57c3` | 산문 length → `max_tokens≤1024` + JSON-only 에스컬레이션 |
| `a4494a3` | `path=.loco/notes`(디렉터리만) → **`_root`** (중첩 `.loco/notes/.loco/notes.md` 방지) |
| `84ed9a9` | JSON **stutter salvage 후에도** max_tokens 캡 유지 + stutter 노트 |

### 스모크 결과 한 표 (`fd-1873-path-sep` · treatment · seed 0)

로컬 스탬프 디렉터리: `.loco/eval/<stamp>/` (gitignore — 재현은 트랜스크립트가 있는 머신에서).  
표의 숫자는 **n=1 진단**이지 효과 추정량이 아니다.

| stamp (UTC) | HEAD (대표) | outcome | turns | 벽시계 | 무엇이 보였나 |
|---|---|---|---|---|---|
| `131902Z` | 초기 treatment | max_turns | 24 | ~6m | 파이프 검증 다수 · notes dual-path 이슈 단서 |
| `133940Z` | max_turns=50 | timeout | 8 | — | 파이프 test |
| `135116Z` | t×3 | rep_stop | 18 | — | 전부 piped test |
| `140558Z` | thrifty | timeout | 6 | ~30m | length 루프 |
| `145615Z` | brevity | **rep_stop** | 29 | ~5m | **B1 원형**: `tests.rs@2790` ×5 · `cargo test \| tail` |
| `151524Z` | B1/B2 | timeout | 4 | **1805s** | verify 0 · length×14 (산문/stutter) |
| `161025Z` | length cap | rep_stop | 20 | **373s** | B1/B2 발화 1회 · notes thrash · 파이프 1 |
| `162606Z` | stutter cap | **rep_stop** | 14 | **201s** | stutter 1회 후 completion≤503 · `_root` 쓰기 정상 · **run_command 0** · notes×5 |

스탬프 원장: [`metrics/stamps-smoke.txt`](./metrics/stamps-smoke.txt)  
검증 도달 런 포렌식: [`metrics/forensic-verify-nav.md`](./metrics/forensic-verify-nav.md)

### 고친 뒤에도 “또 똑같다”고 느낀 이유

| 체감 | 실제 |
|---|---|
| 또 length | **같은 JSON을 max_tokens까지 복붙** (stutter). salvage는 턴은 살리지만 **생성 비용**은 한 번 냄. cap 후 벽시계는 1805→201s로 줄었음. |
| 또 notes | mut-gate / 스키마 / 같은 `_root` 재갱신 → **RepetitionStop**. path 중첩은 `a4494a3`로 막혔음. |
| 또 실패 | 과제 **pass는 한 번도 없음** (이 세션 n=1 줄기). 패치는 “안 죽게 / 덜 비싸게”이지 “고치게”가 아님. |

서버 배경: `scripts/serve.sh` **`--repeat-penalty 1.0` (비활성)** — loco는 temperature만 보냄. M13 핀과 동일; stutter의 디코딩 쪽 원인 후보.

---

## 개정 B (원래 계획)

| | |
|---|---|
| flag | `repo_notes=true` |
| 1차 filter | `fd-1873-path-sep` (대부분의 진단 런) |
| 확장 (부분 수행) | `delta-1089-whole-file-commit` 스모크 1회 기록 있음 (`metrics/smoke-delta-1089-…`) |
| `rg-740` | 이 세션에서 깊게 못 감 |
| 판정 | 완주 · EffectiveConfig · mechanism-alive **기록만** — **ε/Δ 없음** |

공통 로컬 config 골격 (git 무시 `.loco/config.toml`):

```toml
context_tokens = 8192
max_output_tokens = 4096
command_timeout_secs = 60
base_url = "http://localhost:8080/v1"
repo_notes = true
max_turns = 50
```

```bash
cargo run --release -- eval tasks-real --repeats 1 --seed 0 --timeout-scale 3 \
  --filter fd-1873-path-sep
```

---

## 시도 타임라인 (세션 내)

1. **개정 B 스모크** — treatment 경로 살아 있음 / pass 아님.
2. **path prefix** — `.loco/notes/_root` dual-write 수정 (`3853a33`).
3. **thrifty notes** — 거절 템플릿 붙여넣기·과대 notes JSON 완화 (`b3e3af1`, `ccce7c1`).
4. **brevity SYSTEM** — thought 짧게; length 완전 해결은 못 함.
5. **B1/B2** — 언어 중립 검증·항해 노트 (`e4c3b54`). 검증 도달 런에서만 의미 있음.
6. **length salvage** — stutter 첫 JSON 회수 (`d12756d`).
7. **prose length cap + escalate** (`14c57c3`).
8. **`.loco/notes` → `_root`** (`a4494a3`) — mut-gate가 영원히 안 풀리던 dead cert 경로.
9. **stutter 후에도 max_tokens 유지** (`84ed9a9`).
10. **세션 종료** — pass 미달 · 다음 방향은 아래.

상세 포렌식(파이프/재read/stutter 분류): `metrics/forensic-verify-nav.md`.

---

## 다음 세션용 (다른 방향 권장)

이 세션은 **같은 과제 n=1 패치 루프**에 수렴했다. 이어서 같은 줄기만 더 돌리기보다:

### A. 디코딩 / 서빙 (하네스 문구가 아닌 쪽)

- `serve.sh` **repeat-penalty > 1.0** 파일럿 (M13 앵커와 **비비교** 명시).
- 또는 요청에 frequency/presence penalty를 넣을 API 여지 조사 (현 `ChatRequest`엔 없음).
- `max_output_tokens` 기본을 4096→2048 등으로 낮춘 **의도적** operating point (길이 예산 vs edit 크기 트레이드오프).

### B. notes 제품 동작 ( thrash 줄이기 )

- mut-gate 통과 후 **연속 `update_repo_notes` 상한** / 동일 key 재쓰기 쿨다운.
- notes를 **orientation only**로 더 강하게 (버그·패치 본문 금지 — SYSTEM에 이미 일부 있음, 강제력 약함).
- mut-gate 자체를 “실레포 스모크에서 켜기” vs “측정 배치에서만” 재검토 (게이트가 수정 턴을 잡아먹음).

### C. 검증 축 (B1/B2는 이미 심음)

- 파이프 `run_command` **성공 판정을 더 세게** (예: 파이프면 VERIFY 미해제 — 이미 M14 쪽 존재; 모델이 파이프만 쓰는 습관 자체는 남음).
- “실패 테스트 이름 → 자동 grep 제안”을 status_note에 넣기 (모델 무시 가능).
- **check 문자열 vs 모델 검증 명령** 불일치 과제는 task 설계 이슈로 분리.

### D. 실험 설계

- `fd-1873` n=1 중단. **control vs treatment** 를 notes thrash가 덜한 과제 또는 `tasks/` 소형으로 mechanism-alive만.
- 또는 notes **off** 로 동일 과제 베이스라인 1런 — “notes가 원인인지 stutter/모델 한계인지” 분리.
- 전량 102런은 **위 A/B 중 하나가 스모크에서 안정**된 뒤에만.

### E. 하지 말 것 (이 세션 기준)

- 같은 seed·같은 과제에 소형 프롬프트 패치만 연속 커밋하며 “이제 고쳤다” 반복.
- 스모크 0/1을 M16 효과의 기각 근거로 baselines에 올리기.

---

## TODO

1. [x] 초판 사전등록 · 승인
2. [x] 개정 B (treatment 최소 스모크)
3. [x] `fd-1873` 다중 진단 스모크 · 로그/스탬프 (`metrics/stamps-smoke.txt`)
4. [x] length/path/B1B2 하네스 패치 (main 커밋 표)
5. [x] 세션 종료 기록 (이 README)
6. [ ] (다음) A~D 중 방향 선택 후 **새 사전등록 또는 명시적 진단 메모** — 전량 102 보류

숫자/판정은 측정 배치에서만 “결과”로 쓴다. 위 표는 진단 원장이다.
