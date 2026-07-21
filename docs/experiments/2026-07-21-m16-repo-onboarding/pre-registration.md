# 실험 사전등록: M16 계층 레포 notes 온보딩 (control vs treatment)

- 날짜/디렉토리: `docs/experiments/2026-07-21-m16-repo-onboarding/`
- 스펙 근거: `docs/superpowers/specs/2026-07-21-m16-repo-onboarding-design.md`
  §2-2(성공 기준)·§3(장치)·§5(측정 프로토콜)·§7(Key Decisions)
- 플랜: `docs/superpowers/plans/2026-07-21-m16-repo-onboarding.md` (T0–T7 구현 완료)
- 프로토콜: `docs/experiments/PROTOCOL.md` (M15 이후 형태)
- 표본 원천: M15 동결 `docs/experiments/2026-07-20-m15-real-repo-baseline/frozen-sample.md` (N=17)
- **상태: 승인됨 (2026-07-21) · 개정 B (2026-07-21, 최소 스모크).**  
  초판 102런(양 암)은 **보류**. 당장 수행하는 것은 아래 **§0-B treatment 최소 스모크**만.
  PROTOCOL 1: 개정 B 승인 = 본 문서 개정 커밋.

---

## 0-B. 개정 B — treatment 최소 스모크 (지금 돌릴 것)

| 항목 | 값 |
|---|---|
| 목적 | 하네스 **on** 경로가 사는지 · 기전 마커가 찍히는지 · 1런 완주 가능한지. **ε 판정·Δ 주장 아님** |
| 암 | **treatment only** (`repo_notes=true`). control 재측정 **안 함** |
| 과제 | **`fd-1873-path-sep` 1개** (동결 표본 안 · 소형 fd) |
| 반복 | **`--repeats 1 --seed 0`** → **총 1런** |
| 1차 판정 (이 개정) | **없음** (스펙 ε=1/17 적용 **안 함**) |
| 성공 관찰 (보고만) | (1) 하네스 오류 없이 완주 (2) `effective_config.repo_notes==true` (3) mechanism-alive 여부 기록 |
| 명시적 비주장 | M15 0/51 대비 “들어 올림” 통계 주장 금지 · N=17 실격 라벨 금지 |
| 중단 시 | 부분 산출 폐기 · 동일 1런 재시도 1회까지 |
| 이후 | 스모크 결과에 따라 전량(51 또는 개정 표본) 재사전등록 |

**초판 §0–§13의 102런 계약은 효력 정지(보류).** 전량 측정 시 초판 또는 새 개정으로 재승인.

### 0-B 명령

```bash
# .loco/config.toml: repo_notes = true (+ 전역 8192/4096/8080)
cargo run -- eval tasks-real --repeats 1 --seed 0 --filter fd-1873-path-sep
```

---

## 0. 성격 — (초판, 보류) 효과 실험 (재측정 control)

| 항목 | 초판 값 (보류) |
|---|---|
| 가설 | notes 온보딩이 cold-start 통과를 들어 올린다 |
| 암 | control false · treatment true **재측정** |
| 1차 판정 | treatment `task_mean_pass ≥ 1/17` |
| 실격 | 암 독립 · 전패/전승 ≥13 |
| M15 0/51 | control 인용 금지 (전량 실험 시) |
| 총 런 | 102 (17×3×2) |

**ε와 실격은 독립.** 사후 ε 상향 금지 — 개정만.

---

## 1. 가설

**H1 (1차):** treatment 암에서  
`task_mean_pass = mean_i (passed_count_i / 3) ≥ 1/17`  
이 성립한다.  
(대략 총 통과 런 ≥ 3/51 when evenly distributed; **“통과 과제 ≥ 1”을 1차 OR로 두지 않는다**.)

**H2 (기전, 판정 아님):** treatment에서 **mechanism-alive** —  
`notes_updates > 0` **또는** `(notes_mut_gate + notes_schema_reject + notes_stale_finish) > 0`.  
전부 0이고 certified 사용 흔적이 없으면 **장치 미작동 → 해석 보류** (스펙 §5-3).

**H3 (보고, 판정 아님):** control 대비 treatment의 `task_mean_pass` Δ, `tasks_with_any_pass`, 엄격 통과, false_finish, first_mut_turn, nav_hit/fix_hit 층별.

---

## 2. 조건 (암)

양 암 **동일**: 코드 HEAD · 모델 · 컨텍스트 · 시드 · 과제 집합 · 타임아웃 · 서빙 핀.  
**유일한 개입:** `.loco/config.toml` 의 `repo_notes` (true/false).  
eval은 basename `tasks-real` 에 한해 이 값을 유지하고, 동일 값이 `EffectiveConfig.repo_notes`에 스냅샷된다.

| 항목 | 값 | 근거 |
|---|---|---|
| 브랜치 / 코드 | **main** · 측정 착수 시 `git rev-parse HEAD` (M16 머지 포함, ≥ `2c87fdb`) | 배치 전 `git diff <승인커밋>..HEAD -- src/ scripts/ tasks-real/` 는 문서·본 실험 산출물만 허용. `src/` 변경 시 **중단·사전등록 개정** |
| 모델 | ornith-1.0-9b Q4_K_M, alias `ornith` | M13+ 앵커 |
| GGUF | `~/.lmstudio/models/deepreinforce-ai/Ornith-1.0-9B-GGUF/ornith-1.0-9b-Q4_K_M.gguf` | M15와 동일 |
| 서빙 | `scripts/serve.sh` 핀 (M13) | 핀 변경 = 비교가능성 무효 |
| 실효 운용점 | 과제별 **32768** (`TaskSpec`) · 전역 config **8192** | M15 §2 |
| 서버 로드 ctx | **`LOCO_CTX=37632`** (4③ 등호) | M15 개정 A |
| `max_output_tokens` | **4096** | M15 |
| `max_turns` | **25** | 스펙 §0·§5-1 (양 암 동일) |
| `temperature` | 0.1 | 코드 기본 |
| `command_timeout_secs` | 과제별 **180** | task.toml |
| `check_timeout_secs` | 과제별 **300** | task.toml |
| `timeout_secs` (에이전트) | 과제별 **600** | 스펙 §0 · M15 §8-1 |
| `timeout_scale` | **1.0** | CLI 미지정 |
| `base_url` | `http://localhost:8080/v1` | serve.sh |
| `--repeats` | **3** | 스펙 §5-1 |
| `--seed` | **0** (전 하위 배치) | `base_seed+repeat` → {0,1,2} |
| control config | `repo_notes = false` | 스펙 §5-2 |
| treatment config | `repo_notes = true` | 스펙 §5-2 · 제품 기본과 동일 |

### 2-1. 서버/L_req 동결 (M15 인용 — 재측정 없음)

| 기호 | 값 |
|---|---|
| `r_obs` | **1.2587** (M15 T22) |
| `L_req` | **37601** |
| 4③ 서버 로드 | **37632** |
| 운용점 | **32768** |
| 분기 | **2** (`n_ctx_slot ≠ context_tokens`) |

리포트에 로드·L_req·운용점·`effective_context_tokens`를 병기한다.

### 2-2. 암별 `.loco/config.toml` (배치 시 디스크)

**공통 골격** (암마다 `repo_notes` 한 줄만 교체):

```toml
context_tokens = 8192
max_output_tokens = 4096
command_timeout_secs = 60
base_url = "http://localhost:8080/v1"
repo_notes = false   # control 암
# repo_notes = true  # treatment 암 — control 블록과 동시에 두지 말 것
```

- 전역 `context_tokens=8192` 유지 (tasks-real TaskSpec이 32768 덮음).
- 암 전환 시 config를 바꾼 뒤 **첫 하위 배치 전** `EffectiveConfig` 스냅샷 기대값 확인  
  (control: `repo_notes==false` · treatment: `true`).
- M12 잔재 `command_timeout_secs=240` 등 있으면 배치 전 제거.

---

## 3. 표본

### 3-1. 동결 N=17 (M15 frozen-sample — 재선정 없음)

| # | task |
|---|---|
| 1 | delta-1089-whole-file-commit |
| 2 | fd-1873-path-sep |
| 3 | fd-404-min-exact-depth |
| 4 | fd-535-prune |
| 5 | fd-615-hidden-dot-pattern |
| 6 | fd-675-number-parse-error |
| 7 | fd-898-strip-cwd-exec |
| 8 | rg-1138-no-ignore-dot |
| 9 | rg-1159-exit-status |
| 10 | rg-1176-fixed-strings-file |
| 11 | rg-1293-glob-case-insensitive |
| 12 | rg-1390-no-context-sep |
| 13 | rg-1420-no-ignore-exclude |
| 14 | rg-1466-no-ignore-files |
| 15 | rg-1868-passthru-context |
| 16 | rg-568-leading-hyphen |
| 17 | rg-740-passthru |

구성: fd 6 (35.3%) · delta 1 (5.9%) · ripgrep 10 (58.8%) · 편중 ≤60% ·  
상세 좌표·오라클: `…/2026-07-20-m15-real-repo-baseline/frozen-sample.md`.

**총 런:** 17 × 3 × **2암** = **102런**.

### 3-2. 과제 좌표 해시 (배치 전 재해시 · 불일치 시 중단)

작성 시점(main `2c87fdb` 계열) SHA-256 — M15 표와 바이트 동일 확인:

| task | task.toml | procure.toml |
|---|---|---|
| delta-1089-whole-file-commit | `00e882dcd0d88af796c3043d79889755b87d059e397db23792e5dad31f75f297` | `33fbdd087ba29d93ceb012799df786b2e43a2c8c5b6f127f4fb0170999a69065` |
| fd-1873-path-sep | `47cae6b5d9b0ea52ed4cb480e09828b34d7b108bd254734db02560e4592d3a52` | `3cf9f4195db80e01198e6d8fbdad1784b167d521490f60ad48e39525967eff1d` |
| fd-404-min-exact-depth | `790e9d5a34d5cac6cf537fd74014c5efe796eb58c42612873388fca30a03377b` | `63354b359c86c9b671394562824940c16937dcfc24c65291378a687d0173bc72` |
| fd-535-prune | `1fdefb9ff9efb5bca693aa400cfb0c15674affe959eb3f59369a594367168b5a` | `47c96a0f3c864b09610cfff556c449c6faa8ad785a78ea79665694ec1d345e98` |
| fd-615-hidden-dot-pattern | `3add06c3931ff495e197dbd69abe469d980d1bab096d81b62e1fbd6082460339` | `73b60e6679d43ee5dd415af23f187a5953286d8cbd4ed6bbfeab0fb5d8c54342` |
| fd-675-number-parse-error | `af1d2bb7fbb3ff5a520bb6c89c890673976b65948d95a02ceb65071ad21eb311` | `c599e8f80c831bb957456d26de0604819d00ebcdc6eb3d0ffb678e72e586e66c` |
| fd-898-strip-cwd-exec | `322132a923f435ea26e5973f52d7192665e1426ca1c67dfe461ba7ce92fd3453` | `2c26977bbb7651ee8ec9049a77fec8df3d8f2482dde444a00cb3f00d33aec344` |
| rg-1138-no-ignore-dot | `01e7ee4f5d8d00d303a8d1392836c488f3be02297c330f102e501aa05f097d1f` | `165d7402a4d48501b9cf1ee8c95988ff4243c5071e109766883f6797954ad01d` |
| rg-1159-exit-status | `2eeb75c7584e3505635ae4fd1703555f2cf692dcabc7d417a60a6b312e2a906f` | `87a05a05b7a1a530a74027524cae30e9c4bdd7ac573799f69422df7588a54591` |
| rg-1176-fixed-strings-file | `82771fe1ae7cbc43c852621ba6f0e7cffd3e0a0e5be7133ec451f4be9ca85c01` | `fc897aad9ac7412b683388c32d3b5edc0a2e3853f2490ce7a53ba2f5a307afc0` |
| rg-1293-glob-case-insensitive | `ef0822ff0d545bcb10fbb8a07622c08e928091c6235291b6f82b419bcd64b255` | `c9a684047ac2ebedba2165c2c8049932b59ff73540260366c6db1ff160c9664a` |
| rg-1390-no-context-sep | `52adf1e6e49b424d439fb6227c0a7590b9aae4e1af369be6ac77a0ca724b13e9` | `2a60c8feec1fed51cbd8337369da71858f8efc73d97b8d2a20d1992a6b8c454e` |
| rg-1420-no-ignore-exclude | `4eba4cb8ff150052d4c739eb125b4677d33b90a7e1aa517672706a00773c63c5` | `f46c2352e85045480b55f2916084fbc4ae0a7afcbd4b3a1dbbe2196cb1e5f6b8` |
| rg-1466-no-ignore-files | `fb05d97ff7fb810709893fde5568e6d3e1866c201a82f898673a883c02764bbe` | `72a726ac3de8d0cbc84b828e7cb6ce6ff06b2f32afa12d3d708d1d427dbb88f8` |
| rg-1868-passthru-context | `17c5e20586245e92395d2dc7d9fcc1e3437dd40b1a7d65ee021ee7d4fb1a5492` | `876b28d2c9da7977ed77fde0799d70e26d122498baf18d5f81c7c1bd3665f9b0` |
| rg-568-leading-hyphen | `6e720637d2757c4768a4164591c0eec5747ba86a7d1c152dc71f857686f9a7e2` | `c4e4dfdd2d877c91388b6ce0d7b52cf34791b67db3056df85b2e2bd0dd142206` |
| rg-740-passthru | `aa11054472c98d89d9d1ba081c57c8bc0275dcb6848197e436c47d67f9b2808e` | `24b7c4c7a05097f0e10df445f573e86c35432bbd897ad2882e1a59693be0699a` |

---

## 4. 지표

### 4-1. 주·보조

| 역할 | 지표 | 추출 |
|---|---|---|
| **1차 주 (treatment)** | `task_mean_pass` = mean over tasks of (`passed_count`/3) | `report.json` + `exp_metrics.py --pool` |
| 2차 보고 | `tasks_with_any_pass`, 엄격 통과율, false_finish, control 대비 Δ | 동일 |
| 기전 | `notes_updates`, `notes_mut_gate`, `notes_schema_reject`, `notes_stale_finish`, `notes_bytes_max` | `exp_metrics.py` MARKS + COLS |
| 기존 보조 | first_mut_turn, nav_hit, fix_hit, pack_*, stop_cause, protected_edits | 동일 |

### 4-2. 마커 문자열 (Rust 상수와 문자 일치)

| 컬럼 | 매칭 문자열 |
|---|---|
| `notes_schema_reject` | `repo notes schema:` |
| `notes_mut_gate` | `repo notes mut gate:` |
| `notes_stale_finish` | `repo notes stale:` |
| `notes_updates` | `repo notes updated:` |
| `notes_bytes_max` | transcript extra `notes_bytes_max` (flag-off → `-`) |

`notes_offtool` 휴리스틱은 **1차 판정 밖** (선택 보고).

### 4-3. mechanism-alive (treatment만)

다음 중 하나면 **살아 있음**:

1. 전 treatment 런 합 `notes_updates > 0`, 또는  
2. 전 treatment 런 합 `notes_mut_gate + notes_schema_reject + notes_stale_finish > 0`.

둘 다 0이면 **해석 보류** — ε 충족 여부와 무관하게 “장치가 안 돌았다”로 헤드라인.

### 4-4. 실격 대역 (암 독립, N=17)

```
전승 과제 수 ≥ 13  또는  전패 과제 수 ≥ 13  → 그 암 실격
```

(M15와 동일 절대값: `0.98·√17` 휴리스틱 재기술. 스펙 §2-2.)

### 4-5. 통과율 분석 (pool)

| 요소 | 값 |
|---|---|
| 요약 | 과제 수준 통과 비율의 산술 평균 (`task_mean_pass`) |
| 불확실성 | 과제 단위 부트스트랩 · `--resamples 10000` · `--seed 0` |
| 보고 | 점추정 + 95% CI (런 수준 구간 보고 금지) |
| 명령 | `python3 scripts/exp_metrics.py --pool <control stamps…>` 및 treatment 각각 |

항해/수선 층화·비합산은 M15 §4-4와 동일 (`--pool` 기본 계획).

### 4-6. 소표본 규칙 (PROTOCOL 3)

기전 마커 등 **관심 현상 발생 런이 암당 3 미만**이면 비율 대신 발생 런 전수 나열·방향 판정.  
1차 `task_mean_pass` (N=17 과제) 자체에는 해당 없음.

---

## 5. 판정 규칙 (데이터 보기 전)

1. **1차 성공:** treatment `task_mean_pass ≥ 1/17`.  
2. **실격 라벨:** 각 암 독립. treatment가 ε를 넘겨도 전패≥13이면 treatment 실격 보고 (ε를 지우지 않음).  
3. **control 실격:** 보고만 — 1차 성공 판정은 treatment ε. control이 전승≥13이면 “바닥이 사라진 조건”으로 헤드라인에 적고 해석 주의.  
4. **mechanism-alive 실패:** 해석 보류 (성공/실패 헤드라인 위에 경고).  
5. **control 대비 Δ:** 보고·서사. **1차 승패를 Δ 부호로 대체하지 않는다.**  
6. **정상 완주 후 통과율 불만 재측정: 0회.**  
7. **최종 판정은 사람** (PROTOCOL 7). 러너는 표 + 본 규칙 기계 적용 초안까지.

---

## 6. 하위 배치 분할

암마다 M15와 동일 5 하위 배치. **순차** (암 내부·암 간 병행 금지).

| 하위 | `--filter` | 과제 | 런/암 |
|---|---|---|---|
| B1 | `fd-1873-path-sep` `fd-404-min-exact-depth` `fd-535-prune` `fd-615-hidden-dot-pattern` | 4 | 12 |
| B2 | `fd-675-number-parse-error` `fd-898-strip-cwd-exec` `delta-1089-whole-file-commit` | 3 | 9 |
| B3 | `rg-1138-no-ignore-dot` `rg-1159-exit-status` `rg-1176-fixed-strings-file` `rg-1293-glob-case-insensitive` | 4 | 12 |
| B4 | `rg-1390-no-context-sep` `rg-1420-no-ignore-exclude` `rg-1466-no-ignore-files` | 3 | 9 |
| B5 | `rg-1868-passthru-context` `rg-568-leading-hyphen` `rg-740-passthru` | 3 | 9 |

**권장 순서:** control B1→B5 완주 → config를 treatment로 교체 → treatment B1→B5.  
(한 암의 부분 실패가 다른 암 스탬프와 섞이지 않게 스탬프 디렉터리·로그 접두를 암별로 분리.)

명령 템플릿:

```bash
# control: .loco/config.toml 에 repo_notes = false
cargo run -- eval tasks-real --repeats 3 --seed 0 \
  --filter <…> --filter <…>
```

스탬프는 `.loco/eval/<stamp>/`. 로그: §12.

---

## 7. 중단 규칙 · 재측정

### 7-1. 배치 사망

`report.json` 정상 완주가 아닌 경우 (하네스 오류, `interrupted: true`, 미시작).  
**낮은 통과는 사망이 아니다.**

### 7-2. 행동

1. 죽은 하위 배치 부분 산출 **폐기**.  
2. 원인 제거 후 **그 하위 배치만** 재수행.  
3. 정상 완주 형제 하위 배치 **재수행 금지**.

### 7-3. 재측정 횟수 공약

| 상황 | 공약 |
|---|---|
| 하위 배치 사망 | 해당 하위 배치당 재수행 **1회**. 재사망 → 정지·사용자 보고 |
| 정상 완주 후 숫자 불만 | **0회** |
| ε 미달 / 실격 | 재측정 없음 — 보고만 (ε 개정 금지) |

---

## 8. GPU 시간 예산

| 항목 | 값 |
|---|---|
| 총 런 상한 | **102** (= 17 × 3 × 2). 상향 시 사전등록 개정 필요 |
| 추정 | M15 51런 중앙 5–8h · 최악 ~10h 스케일 → **양 암 중앙 10–16h, 최악 ~20h** |
| **벽시계 상한** | **24h** (여유 + 하위 배치 재시작 1회분). 초과 시 진행 중 하위 배치 완주 후 정지 |
| 측정 중 | `cargo build`/`test` **병행 금지** |
| 빌드 | **debug** (`--release` 금지) |

---

## 9. 로그 · 데몬화

### 9-1. 경로 (본 실험 디렉터리 기준)

| 산출 | 경로 |
|---|---|
| 서버 로그 | `docs/experiments/2026-07-21-m16-repo-onboarding/metrics/serve-37632.log` |
| 하위 배치 로그 | `.../metrics/control-B{1..5}.log` · `.../metrics/treatment-B{1..5}.log` |
| CARGO env | `.../metrics/env-cargo.txt` |
| preflight | `.../metrics/preflight-control.txt` · `preflight-treatment.txt` |
| selftest | `.../metrics/selftest.txt` |
| pool | `.../metrics/pooled-control.txt` · `pooled-treatment.txt` |
| 스탬프 | `.loco/eval/<stamp>/` (gitignored) |

`metrics/` 는 배치 시 생성. 이 사전등록 커밋에 빈 숫자 파일을 넣지 않는다.

### 9-2. 서버

```bash
pgrep -x llama-server | xargs kill 2>/dev/null || true

LOCO_MODEL_GGUF=~/.lmstudio/models/deepreinforce-ai/Ornith-1.0-9B-GGUF/ornith-1.0-9b-Q4_K_M.gguf \
LOCO_CTX=37632 \
  scripts/serve.sh \
  > docs/experiments/2026-07-21-m16-repo-onboarding/metrics/serve-37632.log 2>&1 &
```

### 9-3. 데몬화 (fork-then-setsid)

```bash
python3 -c "
import os,sys
if os.fork(): os._exit(0)
os.setsid()
os.execvp(sys.argv[1], sys.argv[1:])
" cargo run -- eval tasks-real --repeats 3 --seed 0 \
  --filter <…> \
  > docs/experiments/2026-07-21-m16-repo-onboarding/metrics/control-B1.log 2>&1
```

---

## 10. 자증 (배치 후)

1. 각 런 `effective_context_tokens == 32768`, `effective_max_turns == 25`.  
2. `report.json` → `effective_config.repo_notes` 가 암과 일치 (control false / treatment true).  
3. 서버 로그 `n_ctx_slot == 37632`.  
4. `/v1/models` → `data[0].id == ornith`.  
5. §3-2 해시 재확인.  
6. `schema_fallback_count == 0`.  
7. treatment pool에서 mechanism-alive 조건 평가.  
8. `python3 scripts/exp_metrics.py --selftest` 첨부.

---

## 11. 배치 전 게이트 체크리스트

0. `find tasks-real -maxdepth 3 -name target` → 0; fixture 내 `.loco` 없음  
1. `.loco/config.toml` 암 조건 (`repo_notes` + 전역 8192/4096)  
2. `cargo test` · `cargo clippy --all-targets -- -D warnings`  
3. `cargo run -- eval tasks --verify` → 12/12  
4. `cargo run -- eval tasks-large --verify` → 3/3  
5. `cargo run -- eval tasks-real --verify` → **17/17**  
6. 서버 `LOCO_CTX=37632` + 로그 캡처  
7. json_schema curl → **HTTP 200** (PROTOCOL 본문)  
8. `n_ctx_slot == 37632`  
9. models id == `ornith`  
10. `ls ${TMPDIR}/.cargo` 없음  
11. `env | grep -E '^CARGO'` 캡처  
12. `python3 scripts/exp_metrics.py --selftest`  
13. 측정 중 병행 빌드 없음 · debug 빌드  

---

## 12. 비교가능성 각주

1. **M15 0/51 스탬프를 control 숫자로 인용하지 않는다** — 동일 표본이어도 바이너리·flag·날짜가 다름.  
2. `tasks/` · `tasks-large` 통과율과 나란히 비교하지 않는다.  
3. 분기 2: `n_ctx_slot(37632) ≠ context_tokens(32768)`.  
4. M12 `sr_error` · M13/M14 `verify_*` 각주 유효.  
5. 단일 모델·양자화·운용점·예산(max_turns=25, timeout=600).

---

## 13. 성공 기준 매핑 (스펙 §2-2)

| 층 | 기준 |
|---|---|
| 구현 게이트 | cargo test · clippy · verify 12/12 · 3/3 · 17/17 (이미 T7; 배치 전 재확인) |
| **1차 최소 들어 올림** | treatment **`task_mean_pass ≥ 1/17`** |
| 2차 보고 | `tasks_with_any_pass` · 엄격 · false_finish · control Δ |
| 기전 생존 | §4-3 mechanism-alive (아니면 해석 보류) |
| 실격 | 암별 전패/전승 ≥13 |

---

## 승인 서명

- [x] 위 조건·표본(N=17)·ε=**1/17**·실격 **≥13**·양 암 재측정·총 **102런**/벽시계 **24h**·재측정 공약을 **데이터 없이** 확정한다  
- [x] M15 0/51 스탬프를 control로 쓰지 않는다  
- [x] 상태 행을 `승인됨(날짜)`로 바꾸는 커밋이 승인 성립 근거다  
- 승인자 / 날짜: 사용자 / 2026-07-21  

**승인됨 — control → treatment 순 측정 진행 (PROTOCOL 1).**
