# 실험 사전등록: M15 실레포 베이스라인 배치 (`tasks-real`)

- 날짜/디렉토리: `docs/experiments/2026-07-20-m15-real-repo-baseline/`
- 스펙 근거: `docs/superpowers/specs/2026-07-20-m15-real-repo-track-design.md`
  §4-1-1(분기)·§5(축 C)·§6(측정)·§6-4(사전등록 필수 19항목)·§8(비교가능성)·§9(성공 기준)
- 플랜: `docs/superpowers/plans/2026-07-20-m15-real-repo-track.md` Task 23
- 프로토콜: `docs/experiments/PROTOCOL.md` (M15 이후 개정 적용 — 항목 17)
- **상태: 승인됨 (2026-07-21).** 사용자가 승인했고, **승인 성립 커밋은 `66a3c7e`**다
  (전언 승인 불가 — M11·M12·M13·M14 전례). 승인 시점의 사전등록 본문은 초안 커밋
  `0b89dcb`에 고정. 표본 N=17 · r_obs=1.2587 · L_req=37601 · 실격≥13 · 51런 · 중단·재측정 공약 불변.
  T24 GPU 배치는 이 승인 이후에만 시작한다 (PROTOCOL 1).
- **개정 A (2026-07-21, T24 게이트 실측):** 4③ 동결 서버 로드를 **37632**로 정정.
  사유: `L_req=37601`은 산식 출력 그대로(재측정 없음)이나, llama-server b9960이
  `-c`를 **256 배수로 올림**해 `n_ctx_slot=37632`를 보고한다(`ceil(37601/256)*256`).
  배치 기동은 `LOCO_CTX=37632`로 하여 `-c == n_ctx_slot` 등호를 성립시킨다.
  운용점 32768·r_obs·표본·판정·예산은 불변. 잔재 로드(예: 40960)는 여전히 등호 실패.

## 0. 성격 — 효과 실험이 아니라 베이스라인 기술 통계

`tasks-real`은 신설 트랙이다. **대조군이 없고, M15에는 효과 입증 실험이 없다.**
첫 배치의 통과율은 개입 판정이 아니라 **M16이 상속할 베이스라인**이다
(스펙 §6-2·§1-2). 따라서:

| 항목 | 본 사전등록에서의 지위 |
|---|---|
| 가설 (효과 비교) | **해당 없음** — 반증할 암 대비가 없다 |
| 판정 임계값 (승자 암) | **해당 없음** — 효과 비교 부재 |
| 실격 대역 (§6-4-6 / §9-A5) | **있음** — 전승·전패 천장 붙음. 대역 안이면 "베이스라인 확보 실패"로
  보고하고 M16 대조군으로 인용하지 않되, **M15 병합은 막지 않는다** |
| A1b (최소 표본·편중) | 표본 동결 시점에 이미 충족 확인. 미달이었다면 처분 (i)/(ii)/(iii) 중 하나 |

## 1. 가설

**해당 없음** (효과 비교 부재). 이 배치가 내는 것은 기술 통계와 실격 여부다.

## 2. 조건 (암)

단일 암 — 개입 대비 없음.

| 항목 | 값 | 근거·출처 |
|---|---|---|
| 브랜치 | `m15/real-repo-track` | 플랜 Global Constraint — main 병합은 T25 판정 후 |
| 대상 코드 | 승인 시점 `git rev-parse HEAD` (인프라 T1–T20 완료, 측정 대상) | 배치 착수 전 `git diff <승인커밋>..HEAD -- src/ scripts/ tasks-real/` 가
  **문서 전용 변경만** 허용. `src/` 비어 있지 않으면 배치 중단·보고 |
| 모델 | ornith-1.0-9b Q4_K_M, alias `ornith` | M13 이후 앵커 모델 |
| GGUF | `~/.lmstudio/models/deepreinforce-ai/Ornith-1.0-9B-GGUF/ornith-1.0-9b-Q4_K_M.gguf` | handoff / serve.sh |
| 서빙 | `scripts/serve.sh` 핀 (M13) | 핀 변경 = 비교가능성 무효 |
| **실효 운용점 `context_tokens`** | **과제별 32768** (`TaskSpec`, H1). **전역 config = 8192** (올리지 말 것) | §8 각주 3; 전역을 32768로 올리면 `tasks/` 앵커 조건이 오염된다 |
| **서버 로드 ctx** | **`LOCO_CTX=37632`** (4③ 동결값 **등호**, `≥` 아님) | L_req=37601 → 256-정렬 개정 A |
| `max_output_tokens` | **4096** | M13 사고 토큰 잠식 레버(§4-5·M13 §3-2). 전역 `.loco/config.toml` |
| `max_turns` | **25** (코드 기본; `task.toml` 오버라이드 없음) | 스모크의 max_turns 상향(40–100)은 pack 도달용이며 **배치 조건이 아님** |
| `temperature` | 0.1 (코드 기본) | config에 두지 않음 |
| `command_timeout_secs` | **과제별 180** (`task.toml`) | 실레포 툴 명령; 전역 60 잔재가 있어도 TaskSpec이 덮음 |
| `check_timeout_secs` | **과제별 300** | `task.toml` 전 과제 동일 |
| `timeout_secs` (에이전트 런 상한) | **과제별 600** | 아래 §8-1 |
| `timeout_scale` | **1.0** (CLI 미지정 = 기본) | 스케일 안 씀 |
| `base_url` | `http://localhost:8080/v1` | serve.sh 기본 포트 |
| `--repeats` | **3** | 아래 §8-2 |
| `base_seed` / 하위 배치 `--seed` | **0** (전 하위 배치 동일) | `base_seed+repeat` → 시드 {0,1,2}. 분할이 시드 집합을 바꾸지 않게 전 배치 동일 seed |
| 모델 서버 로드 명령 | `LOCO_MODEL_GGUF=<gguf> LOCO_CTX=37632 scripts/serve.sh > <로그> 2>&1` | 개정 A 등호 동결 |

### 2-1. T22 분기 결과 (재유도 금지 — 인용)

스모크·분기 원자료: `smoke.md`, 채택 세션
`smoke/attempt5-rg-1868-max100.jsonl`. **재측정하지 않는다.**

| 기호 | 동결값 |
|---|---|
| `r_obs` | **1.2587** (턴별 `prompt_tokens/estimate_tokens` **최댓값**) |
| 첫 턴 `prompt_tokens` | **2111** (정의상 캐시 미스, §5-5) |
| `pack_fired` | **9** (도달 조건 충족) |
| `마진` | **1024** (T1 `thresholds.md`, 커밋 **`d583ff8`**) |
| `L_req` | **37601** = ⌈(32768−4096)·0.9·1.2587 + 4096 + 1024⌉ |
| `n_ctx_train` | **262144** (T20 GGUF 직독, `supply-survey.md` §1) |
| 분기 | **2** (`32768 < 37601 ≤ 262144`) |
| 확정 서버 로드 (4③) | **37632** (개정 A — L_req 37601의 256-정렬 실현값) |
| 스모크 관측 로드 (배치 조건 아님) | 40960 |
| 사후 슬랙 (기록용, `마진`과 구분) | `n_ctx_slot_obs − ((ctx−mo)·0.9·r_obs + mo)` = 40960 − 36576.5 = **4383.5** (스모크 로드 기준). 배치에서는 `37632 − 36576.5 = 1055.5` |

⚠ 분기 2이므로 **`n_ctx_slot ≠ context_tokens`**. 리포트에 로드(37632)·L_req(37601)·운용점(32768)을
나란히 적고, `n_ctx_slot`이 증언하는 것은 **동결 로드**이지 실효 운용점이 아님을 명시한다
(§8 각주 6). 실효 운용점의 증인은 H9 `RunRecord.effective_context_tokens`다.

## 3. 표본

### 3-1. 공급량 실사 (항목 1)

원문: `supply-survey.md` (+ §6 재실사 추록). 전 이력 · 정정 `link_issues.py`.

**1차 4레포 (proxy / 문언):**

| 레포 | 닫힌 이슈 | 이슈연결 | 테스트동반 | 규약1·3 후(proxy) | 문언 |
|---|---|---|---|---|---|
| zoxide | 648 | 4 | 0 | **0** | 0 |
| fd | 847 | 124 | 31 | **22** | 25 |
| ripgrep | 1697 | 588 | 170 | **37** | 91 |
| just | 1283 | 14 | 1 | **0** | 0 |

zoxide·just는 squash-merge + `(#PR)` 관례로 커밋 메시지 연결 공급 0
(탐침 80/80 `commit_id: null`).

**재실사 추가 5레포 (proxy):** bat 22 · hyperfine 2 · **delta 3** · sd 1 · dust 0.

**선정 레포 (최종):** **fd + ripgrep + delta**.
(초판 선정 fd+ripgrep → 편중 위반 후 처분 1로 delta 추가. bat은 비ASCII 경로로 미채택.)

### 3-2. 최소 표본·편중 (항목 2·3) — 데이터 이전 동결

| 임계값 | 값 | 동결 커밋 |
|---|---|---|
| 최소 표본 하한 | **16** | **`d583ff8`** (`thresholds.md`) |
| 레포 편중 상한 | 단일 레포 **≤ 60%** | 동일 |
| 미달 허용 처분 | (i) 레포 추가 재실사 (ii) M15 연기 (iii) 확보 수로 진행·A1b 실패 보고·M16 비인용 | 동일 — **임계값 자체 재조정 금지** |

**동결 표본 대조:** N=17 ≥ 16, 최대 편중 ripgrep **58.8% ≤ 60%** → **A1b 충족**.
(초판 N=16·rg 62.5%는 편중 위반 → 처분 1 재실사 완료.)

### 3-3. 표본 동결 (항목 4) — N=17

원문 표·제외 사유: `frozen-sample.md`. 감사 원 출력: `audit/<task>-check.txt`.
추출 스크립트: `scripts/leak_audit.py` (항목 16).

| # | task | repo | issue | fix (짧은) | parent (짧은) | nav | 지목 |
|---|---|---|---|---|---|---|---|
| 1 | delta-1089-whole-file-commit | delta | 1089 | `bd54a51205be` | `e28e97de7aa0` | 단축 안 됨 | 지목되지 않음 |
| 2 | fd-1873-path-sep | fd | 1873 | `ed4766419152` | `90e73d72df25` | 단축 안 됨 | 지목되지 않음 |
| 3 | fd-404-min-exact-depth | fd | 404 | `d63c63be8cf8` | `47974b647959` | 단축 안 됨 | 지목되지 않음 |
| 4 | fd-535-prune | fd | 535 | `ec4cc981fcf4` | `06eb231fbd64` | 단축 안 됨 | 지목되지 않음 |
| 5 | fd-615-hidden-dot-pattern | fd | 615 | `cadaef3f076f` | `17bd256ae6e4` | 단축 안 됨 | 지목되지 않음 |
| 6 | fd-675-number-parse-error | fd | 675 | `e0adb45d082d` | `ec4cc981fcf4` | 단축 안 됨 | 지목되지 않음 |
| 7 | fd-898-strip-cwd-exec | fd | 898 | `4ffc34956f9a` | `5039d2db9914` | 단축 안 됨 | 지목되지 않음 |
| 8 | rg-1138-no-ignore-dot | ripgrep | 1138 | `12a6ca45f9da` | `9d703110cfe0` | 단축 안 됨 | 지목되지 않음 |
| 9 | rg-1159-exit-status | ripgrep | 1159 | `f3164f2615ce` | `31d3e241306f` | 단축 안 됨 | 지목되지 않음 |
| 10 | rg-1176-fixed-strings-file | ripgrep | 1176 | `0df71240ff19` | `f3164f2615ce` | **단축됨** | 지목되지 않음 |
| 11 | rg-1293-glob-case-insensitive | ripgrep | 1293 | `c2cb0a4de459` | `adb9332f52b8` | 단축 안 됨 | 지목되지 않음 |
| 12 | rg-1390-no-context-sep | ripgrep | 1390 | `e71eedf0eb80` | `88f46d12f1f3` | 단축 안 됨 | 지목되지 않음 |
| 13 | rg-1420-no-ignore-exclude | ripgrep | 1420 | `297b428c8c92` | `804b43ecd8bd` | **단축됨** | 지목되지 않음 |
| 14 | rg-1466-no-ignore-files | ripgrep | 1466 | `c4c43c733ee9` | `447506ebe02f` | 단축 안 됨 | 지목되지 않음 |
| 15 | rg-1868-passthru-context | ripgrep | 1868 | `a77b914e7ac9` | `2e2af50a4df0` | 단축 안 됨 | 지목되지 않음 |
| 16 | rg-568-leading-hyphen | ripgrep | 568 | `6dce04963d4e` | `d4b790fd8d97` | **단축됨** | 지목되지 않음 |
| 17 | rg-740-passthru | ripgrep | 740 | `58bdc366ec29` | `34c0b1bc709f` | 단축 안 됨 | 지목되지 않음 |

구성: **fd 6 (35.3%) · delta 1 (5.9%) · ripgrep 10 (58.8%)**. `--verify` **17/17**.

오라클·`check`·`protected` 전문은 `frozen-sample.md` 및 각 `procure.toml`/`task.toml`.
스모크 채택 과제 `rg-1868-passthru-context`는 본 동결 표본에 **포함** (규약 6 탈락 없음).

### 3-4. 항해 거리 라벨 상한 (항목 15) — 보고 의무

| 셀 | 수 | 비율 |
|---|---|---|
| 단축 안 됨 | 14 | **82.4%** |
| 단축됨 | 3 | 17.6% |

**"단축 안 됨" 셀이 80%를 초과한다.** 하드 제약이 아니라 **보고 의무**: T25 리포트
헤드라인에 이 사실을 적고, 베이스라인 적용 범위를 *"이슈 본문이 파일/함수를 거의
지정하지 않는 과제에 편중"*으로 제한한다고 명시한다. **이 라벨로 부분군 분석·층화
판정을 하지 않는다**(스펙 §3-2).

### 3-5. 3항 감사 (항목 16)

- 지목 판정: 전 채택 과제 `지목되지 않음` (`leak_audit.py`, `audit/*-check.txt`)
- 이슈↔커밋 정합·제외: `audit/excluded.json`, `frozen-sample.md` 제외 기록
- `.gitattributes`: `audit/gitattributes-audit.json`
- 추출 스크립트 경로: **`scripts/leak_audit.py`** (사전등록 산출물에 경로로 포함)

### 3-6. 과제 좌표 해시 (항목 10 자증 입력)

작성 시점(HEAD `84a0916` 계열) SHA-256. 배치 전 재해시해 불일치 시 중단.

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

## 4. 지표

### 4-1. 주·보조 (항목 5)

| 역할 | 지표 | 추출 |
|---|---|---|
| **주** | 과제 수준 통과 비율의 평균 — 런 `passed`를 과제 내 평균 → 과제 간 평균 | `report.json` + `exp_metrics.py --pool` |
| **보조 (사전 지정)** | 동일 구조의 `passed_strict` (`passed ∧ outcome==finished`) | 동일 |
| 방향 갈림 | 주와 보조 방향이 갈리면 **리포트 헤드라인에 적는다** (M9 선례) | — |
| 기록 의무 | `false_finish_count`, `stop_cause` 분포, 마커·토큰·항해 열 | `exp_metrics.py` |

**효과 비교 임계값: 해당 없음.**

### 4-2. 실격 대역 (항목 6) — N=17 절대값

T1 공식: 실격 ⟺ `N − 전승 < 0.98·√N` (바닥 대칭: 전패).

```
0.98·√17 ≈ 4.0406
전승 ≥ 17 − 4 = 13  또는  전패 ≥ 13  → 실격
```

**동결 절대값: 전승 과제 수 ≥ 13 또는 전패 과제 수 ≥ 13 이면 실격**
(`frozen-sample.md`와 동일). 여기서 "전승/전패 과제" = 해당 과제의 3런이 모두
pass / 모두 fail.

⚠ 척도 캐비엇·부트스트랩 CI와 수치 일치 불필요 — `thresholds.md` §3 그대로.

§9-A5: 대역 **안**이면 "베이스라인 확보 실패"·M16 비인용, **M15 병합은 막지 않음**.

### 4-3. 통과율 분석 계획 (항목 7)

| 요소 | 값 |
|---|---|
| 요약 통계 | 과제 수준 통과 비율의 산술 평균 |
| 불확실성 | **과제 단위** 복원추출 부트스트랩 |
| 재추출 횟수 | **`--resamples 10000`** |
| 부트스트랩 seed | **`--seed 0`** |
| 보고 | 점추정 + **95% CI** |
| **공약** | **런 수준 구간은 어떤 형태로도 보고하지 않는다** |

구현: `python3 scripts/exp_metrics.py --pool <stamp1> ... --resamples 10000 --seed 0`.

### 4-4. 축 C·§5-4 분석 계획 (항목 19)

① **항해/수선 지표** (`nav_hit`/`fix_hit` 등): 분모 = 해당 과제의 **층별** 런 수
   (통과 층·실패 층 각각). 과제별 층내 비율 → 과제 수준 평균.
   **층 크기 0인 과제는 그 층 평균에서 제외**하고 제외 과제 수를 함께 보고.
   오라클 없는 과제는 `nav_hit`/`fix_hit` = `"-"` (0이 아님).
② 교집합 판정 = §3-4-3과 동일하게 **`≠ ∅`**.
③ **층화 비합산** — 통과 층과 실패 층 분모를 합치지 않는다.
④ 부트스트랩: 재추출 단위 = **과제**. **제외 후 남은 집합에서** 재추출
   (전체에서 뽑아 정의된 것만 집계하지 않음). resamples/seed = 항목 7과 동일.
⑤ §5-3 추정기: 턴 단위 최소자승 회귀, `inline_system` 여부로 층화
   (`exp_metrics.py --session` / pool 출력의 slope·intercept).
⑥ **§5-5 `prompt_tokens` 의미 (T22 동결, 재측정 없음):**
   - 정의: 서버가 보고한 프롬프트 토큰.
   - **첫 턴 = 정의상 캐시 미스** → 완전 프롬프트 기준. 원자료: 첫 턴 **2111**.
   - `r_obs`는 턴별 비의 **최댓값**(평균 아님). 동결 **1.2587**.

### 4-5. 마커·기회 분모 (§6-3, 판정 아님)

| 장치 | 계수 | 분모 |
|---|---|---|
| 파이프 가드 | `pipe_unreleased` 등 | 파이프 포함 `run_command` (proxy `pipe_note`; known under-count) |
| FINISH_NUDGE | `finish_nudge_total` | 무장 조건 충족 런 — **`armed_runs~`는 근사**, 정확 분모로 인용 금지 |
| A-3 | `model_diff` / `model_diff_trunc` | 성공 edit/write → **절단률** = trunc/diff |

0회도 답이다 — **분모와 함께**일 때만. `exp_metrics.py --selftest` 출력을 배치
산출물에 첨부(마커 수동 미러 드리프트 가드).

## 5. 판정 규칙 (데이터 보기 전)

1. **효과 승자: 해당 없음.**
2. **실격 (§6-4-6):** 전승 ≥13 또는 전패 ≥13 → 베이스라인 확보 실패, M16 대조 비인용.
   대역 밖 → M16 대조군 적격 후보(최종 인용은 T25 리뷰).
3. **소표본 규칙 (PROTOCOL 3):** 관심 현상 발생 런이 배치당 3 미만이면 비율 대신
   발생 런 전수 나열·방향 판정. 베이스라인 통과율 자체(N=17)에는 해당 없음.
4. **최종 판정은 사람**(PROTOCOL 7). 러너는 지표 표 + 본 규칙의 기계 적용 초안까지.

## 6. 하위 배치 분할 (내구성)

`run_eval`은 루프 종료 후 `report.json` 1회 기록. LLM 에러 1건이 하네스 전체를 죽인다.
**5개 하위 배치**로 쪼개 각각 별도 스탬프·report.json. 집계는 `--pool`.

| 하위 | `--filter` (정확 일치, 반복) | 과제 수 | 런 수 | `--seed` |
|---|---|---|---|---|
| B1 | `fd-1873-path-sep` `fd-404-min-exact-depth` `fd-535-prune` `fd-615-hidden-dot-pattern` | 4 | 12 | 0 |
| B2 | `fd-675-number-parse-error` `fd-898-strip-cwd-exec` `delta-1089-whole-file-commit` | 3 | 9 | 0 |
| B3 | `rg-1138-no-ignore-dot` `rg-1159-exit-status` `rg-1176-fixed-strings-file` `rg-1293-glob-case-insensitive` | 4 | 12 | 0 |
| B4 | `rg-1390-no-context-sep` `rg-1420-no-ignore-exclude` `rg-1466-no-ignore-files` | 3 | 9 | 0 |
| B5 | `rg-1868-passthru-context` `rg-568-leading-hyphen` `rg-740-passthru` | 3 | 9 | 0 |

**합계: 17과제 × 3반복 = 51런.** 하위 배치는 **순차** (병행 금지 — PROTOCOL 2).

명령 템플릿 (각 Bi):

```bash
cargo run -- eval tasks-real --repeats 3 --seed 0 \
  --filter <…> --filter <…>
```

## 7. 중단 규칙 (항목 13) · 재측정 (항목 14)

### 7-1. 배치 사망 정의

**배치 사망** ⟺ 다음 중 하나로 해당 하위 배치의 `report.json`이 **정상 완주 산출이
아닌** 경우:

- 하네스 오류(서버 다운, 과제 정의 오류, LLM 전파 에러로 루프 중단)
- Ctrl+C / 강제 종료로 `interrupted: true` 또는 report 미생성
- 데몬화 실패로 프로세스가 시작되지 않음

**정상 종료(`interrupted: false`)한 낮은 통과 수는 사망이 아니다** — 폐기·재해석
금지 (M12/M14 교훈).

### 7-2. 중단 시 행동

1. 죽은 하위 배치의 부분 산출은 **폐기**.
2. 원인 제거(서버 재기동, 설정 잔재 제거 등) 후 **그 하위 배치만** 처음부터 재수행.
3. 이미 정상 완주한 형제 하위 배치는 **재수행하지 않는다**.

### 7-3. 재측정 횟수 사전 공약

| 상황 | 공약 |
|---|---|
| 하위 배치 사망 | 해당 하위 배치당 **재수행 1회**까지. 재수행도 사망 → **정지·사용자 보고**. 추가 시도는 사전등록 개정 필요 |
| 정상 완주 후 통과율 불만 | **재측정 0회** — 베이스라인이므로 "마음에 안 드는 숫자"로 그물을 넓히지 않는다 |
| 실격 대역 안 | 재측정 없음. §9-A5 처분만 |

## 8. 조건 고정 상세 (항목 8)

### 8-1. `timeout_secs=600` 근거

- llama.cpp 앵커 `20260719T082030Z`: 36런, `avg_duration_secs ≈ 67.1`, max 단일 런 ≈ 305s
  (8K·합성 `tasks/`).
- 32K 비용 배수(M9 근거, §4-2): 런당 **+61~68%** → 에이전트 시간 대략 110s 전후,
  꼬리는 더 김.
- 실레포 `check`(fd/rg 통합 테스트) 수 초~수십 초 추가.
- 기본 300s는 앵커 max(305s)에도 못 미치고 32K 꼬리에서 **Timeout 폭주** 위험
  (스펙 §6-4-8 경고).
- **600s** = 앵커 max의 약 2× + 실레포 여유. 전 `tasks-real` `task.toml`에 이미 기록.

`command_timeout_secs=180`: 단일 툴 명령(빌드/테스트 조각). `check_timeout_secs=300`:
채점 `check` 전용.

### 8-2. `--repeats=3` 근거 (두 문장 의무)

1. **§6-4-8:** `base_seed+repeat` 규약상 반복 수는 시드 집합을 바꾼다 → 반복을 올리면
   시드 {0,1,2}가 아닌 집합이 되어 비교·재현 좌표가 바뀐다. 기본 3·seed 0을 동결한다.
2. **§6-1:** N=17 < 20 이므로 **통과율에 대해** 남는 예산을 반복으로 돌리지 않는다
   (재추출 단위 = 과제 → 반복 증가는 과제 수준 정밀도를 사지 못함).

**§5-4 / 제외 셀 예상 (반복 상향 판단):**

- M13 실레포 파일럿 채택 ~4/19 ≈ 20%대. 같은 난이도 가정이면 과제당 Bin(3, 0.2)에서
  3/3 전승 확률 ≈ 0.008 → 기대 전승 과제 ≪ 1.
- 따라서 **실패 층**은 거의 모든 과제에서 비어 있지 않고, **통과 층** 제외 셀은 많을 수
  있다. 통과 층 해상도를 위해 반복을 4–5로 올리면 총 런 68–85로 **§6-4-11 최악 10h 추정과
  총 런 상한(아래)을 위협**한다.
- **판단: repeats=3 유지.** 통과 층 제외가 사후 크면 그 사실을 리포트에 적고, 반복
  상향은 M16 사전등록의 몫으로 남긴다.

### 8-3. `.loco/config.toml` (배치 시 디스크 상태)

```toml
context_tokens = 8192
max_output_tokens = 4096
command_timeout_secs = 60
base_url = "http://localhost:8080/v1"
```

- `context_tokens=8192`는 **전역**. 실효 32768은 `tasks-real/*/task.toml`의
  `context_tokens = 32768`만.
- `command_timeout_secs=60` 전역은 TaskSpec 180이 과제별로 덮는다.
- M12 잔재 `command_timeout_secs=240` 등이 있으면 **배치 전 제거**.

## 9. 환경 조건 (항목 9)

배치 시작 시 캡처해 산출물에 첨부:

```bash
env | grep -E '^CARGO' | tee docs/experiments/2026-07-20-m15-real-repo-baseline/metrics/env-cargo.txt
```

`exec.rs`가 `.env()`를 안 불러 부모 환경을 상속 → `CARGO_NET_OFFLINE`·`CARGO_HOME`이
양 런에 걸려도 report.json에 안 남는다. **이 캡처가 유일 기록.**

## 10. 자증 절차 (항목 10)

배치 후 / 리포트 시:

1. 각 런 `report.json` → `effective_context_tokens` (H9) == **32768**
   (`grep -c '"effective_context_tokens"'` 및 값 검증).
2. `effective_max_turns` == **25**.
3. 서버 로그 `n_ctx_slot == 37632` (4③ 등호, 개정 A) — 통과 로그를 배치 산출물에 **첨부** (§9-A4).
4. `curl /v1/models` → `data[0].id == ornith`.
5. §3-6 해시 표와 배치 직전 `task.toml`/`procure.toml` 재해시 일치.
6. `schema_fallback_count == 0` (0이 아니면 해당 런 측정 신뢰 불가 — 보고).

## 11. GPU 시간 예산 (항목 11)

| 항목 | 값 |
|---|---|
| 총 런 수 상한 | **51** (= 17 × 3). 반복 상향·표본 추가로 51 초과 금지(사전등록 개정 없이) |
| 추정 | llama.cpp 8K 앵커 ~67s/런 × 1.65(32K) ≈ 110s 에이전트 + check → 대략 런당 2–4분.
  51런 → **중앙 5–8h, 최악 10h** (스펙 §6-1 N=20·60런 추정을 N=17·51런으로 하향) |
| **상한** | **벽시계 12h** (여유 + 하위 배치 재시작 1회분). 초과 시 **진행 중 하위 배치 완주 후 정지**,
  미시작 하위 배치는 수행하지 않고 사용자 보고 |
| 측정 중 | `cargo build`/`test` **병행 금지** (PROTOCOL 2) |
| 빌드 | **debug** (`--release` 금지 — report.json에 프로파일이 안 남아 사후 발견 불가) |

## 12. 로그 캡처 · 데몬화 (항목 12)

### 12-1. 경로

| 산출 | 경로 |
|---|---|
| 서버 기동 로그 | `docs/experiments/2026-07-20-m15-real-repo-baseline/metrics/serve-37601.log` |
| 하위 배치 stdout/err | `.../metrics/batch-B{1..5}.log` |
| CARGO env | `.../metrics/env-cargo.txt` |
| 4③·models 스모크 | `.../metrics/preflight.txt` |
| exp_metrics selftest | `.../metrics/selftest.txt` |
| pool 요약 | `.../metrics/pooled.txt` |
| eval 스탬프 | `.loco/eval/<stamp>/` (git-ignored) × 5 |

### 12-2. 서버 기동

```bash
# 이전 서버: pkill -f 대신 바이너리 정확 매치 (래퍼 self-match 회피)
pgrep -x llama-server | xargs -r kill
# macOS xargs -r 없으면:
# pgrep -x llama-server | xargs kill 2>/dev/null || true

LOCO_MODEL_GGUF=~/.lmstudio/models/deepreinforce-ai/Ornith-1.0-9B-GGUF/ornith-1.0-9b-Q4_K_M.gguf \
LOCO_CTX=37632 \
  scripts/serve.sh \
  > docs/experiments/2026-07-20-m15-real-repo-baseline/metrics/serve-37601.log 2>&1 &
```

### 12-3. 데몬화 (fork-then-setsid — PROTOCOL 4④ M15 형태)

대화형 셸에서 배경 프로세스는 이미 그룹 리더 → 단독 `os.setsid()`는 `PermissionError`.
**fork 후 setsid:**

```bash
python3 -c "
import os,sys
if os.fork(): os._exit(0)
os.setsid()
os.execvp(sys.argv[1], sys.argv[1:])
" cargo run -- eval tasks-real --repeats 3 --seed 0 \
  --filter <…> \
  > docs/experiments/2026-07-20-m15-real-repo-baseline/metrics/batch-B1.log 2>&1
```

확인:

```bash
ps -o pid,ppid,pgid,sid,command -p $(pgrep -f 'eval tasks-real' | head -1)
```

Expected: `PID == SID`.

## 13. PROTOCOL 적용 시점 (항목 17)

| 조항 | M15 이후 형태 | 본 배치 |
|---|---|---|
| 4① | 세 트리 `--verify` (`tasks` 12/12 · `tasks-large` 3/3 · `tasks-real` **17/17**) | 준수 |
| 4③ | `n_ctx_slot ==` **동결 서버 로드 37632** (등호, 개정 A; ≥ 실효 운용점 32768) | 준수 |
| 항목 5 | 실효 운용 증인 = H9 `effective_context_tokens`; `n_ctx_slot`은 로드 증인 | 준수 |
| 4④ | fork-then-setsid | 준수 |

M13·M14 앵커 리포트는 소급 재해석하지 않는다 (`n_ctx_slot == 8192 == context_tokens`).

## 14. TMPDIR (항목 18)

```bash
# 배치 전
ls ${TMPDIR}/.cargo 2>/dev/null && echo "REMOVE MANUALLY"   # 존재 시 수동 제거
ls ${TMPDIR}/loco-eval-* 2>/dev/null | head
df -h ${TMPDIR}

# 배치 후
ls ${TMPDIR}/loco-eval-* 2>/dev/null | head
```

`Sandbox::cleanup`은 best-effort. 빌드 후 target 규모 참고: fd ~255M · ripgrep ~459M
(just ~998M은 본 표본 미포함). 잔여 샌드박스가 디스크를 잠식하면 수동 정리 후 다음
하위 배치.

## 15. 배치 전 게이트 체크리스트 (T24 Step 1)

승인 후·배치 직전. 전건 통과 전 GPU 루프 금지.

0. `find tasks-real -maxdepth 3 -name target` → 0; `-name .loco` → 0
1. `grep context_tokens .loco/config.toml` → **8192** (또는 전역 오버라이드 없음)
2. `cargo run -- eval tasks --verify` → 12/12
3. `cargo run -- eval tasks-large --verify` → 3/3
4. `cargo run -- eval tasks-real --verify` → **17/17**
5. 서버 `LOCO_CTX=37632` 기동 + 로그 캡처 (개정 A)
6. json_schema curl → **HTTP 200** (PROTOCOL 본문 명령)
7. 로그 `n_ctx_slot` **== 37632**
8. `/v1/models` `data[0].id` **== ornith**
9. `ls ${TMPDIR}/.cargo` 없음
10. `env | grep -E '^CARGO'` 캡처
11. `python3 scripts/exp_metrics.py --selftest` 통과 + 첨부
12. 스테일 뮤테이션: `audit/stale-mutation.json` 표본 확인 + 배치 전 전 과제
    `thresholds.md` §4 절차 재확인(미완 시 완주 후 첨부). 실패 시 배치 금지
13. `--release` 미사용 · 측정 중 병행 빌드 없음

## 16. 비교가능성 각주 (스펙 §8 승계)

1. M13 파일럿과 **비교 불가** (채점자·출처·반복·픽스처).
2. `tasks-real` 통과율은 M13·M14 수치와 **비교하지 않는다.** 마커는 기회 분모 있을 때만
   M14와 대조 가능.
3. `tasks/`·`tasks-large` 앵커는 8K 불변 — 전역 config 오염 금지.
4. 단일 모델·단일 양자화·단일 운용점(32K).
5. M12 `sr_error` · M13 T7 `verify_*` · M14 `verify_allpass` 각주 유효.
6. **분기 2:** `n_ctx_slot(37632) ≠ context_tokens(32768)` (L_req=37601) — 리포트에 병기.

## 17. 성공 기준 매핑 (§9, 배치 후 T25)

| ID | 기준 | 본 문서 위치 |
|---|---|---|
| A1a | 동결 전 과제 `--verify` | §15.4 |
| A1b | N≥16 · 편중≤60% | §3-2 (충족) |
| A2 | 캐시 비움 재조달 매니페스트 일치 | T25 / 배치 전 선택 게이트 |
| A3 | 스테일·실행비트·fixture `target/` 없음 | §15.0 · §15.12 |
| A4 | H9 기록 + 4③ 통과 로그 첨부 | §10 · §12 |
| A5 | 실격 대역 밖 | §4-2 · §5 |
| A6 | 축 C 일곱 항목 + selftest | §4-4 · §15.11 |
| A7 | cargo test / clippy / 세 verify | 인프라 게이트 (배치 전 재확인) |

## 18. 유인 3건 (§7) — 배치 후, 본 사전등록 범위 밖 스케줄

배치 **완주 후** `scripts/pilot.sh` 3세션. `tasks-real` 후보와 **배타**, 관측 스키마
착수 전 고정, 비율 인용 금지. 상세는 T25. **본 GPU 배치 예산·런 상한에 포함하지 않음.**

---

## 승인 서명

- [x] 위 조건·표본(N=17)·L_req **37601**·4③ 로드 **37632**(개정 A)·`r_obs` **1.2587**·실격 **≥13**·예산 **51런/12h**·
      재측정 공약을 데이터 없이 확정한다
- [x] 상태 행을 `승인됨(날짜)`로 바꾸는 커밋이 승인의 성립 근거다
- 승인자 / 날짜: 사용자 / 2026-07-21
- 개정 A: T24 게이트 실측(llama.cpp 256-정렬). L_req·r_obs 재측정 없음.

**승인됨 — T24 진행 중 (PROTOCOL 1). 4③ 로드 = 37632 (개정 A).**
