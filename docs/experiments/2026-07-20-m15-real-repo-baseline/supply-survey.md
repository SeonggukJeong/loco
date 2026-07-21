# M15 공급량 실사 + rope 캡처 — 레포 선정 (§6-4-1·§6-4-12)

이 문서는 T1이 동결한 임계값(`thresholds.md`, 커밋 `d583ff8`: 최소 표본 16 · 편중
상한 60%)과 대조되는 **데이터**다. 임계값은 이 실사보다 먼저 얼어붙었다.

측정 일자: 2026-07-21. 도구: `gh`(인증 SeonggukJeong), 로컬 bare 클론,
`scripts/link_issues.py`(정정된 정규식·NUL 파서, 커밋 `9b7c8a8`).

---

## 1. rope 파라미터 (축 E — §4-1-1 분기 전제)

⚠ **방법 주의**: 브리프 Step 1은 `serve.sh`를 재기동해 서버 로그를 grep하도록
지시했으나, 사용자의 실행 중 llama-server(ornith, `-c 8192`)를 끊지 않기 위해
**GGUF 메타데이터를 직접 읽었다**. `n_ctx_train`은 모델 속성이므로 두 방법은
등가다(llama.cpp의 `n_ctx_train`은 GGUF `*.context_length` 키에서 온다). 원 로그
줄이 필요하면 별도 기동으로 재확인 가능하다.

GGUF: `~/.lmstudio/models/deepreinforce-ai/Ornith-1.0-9B-GGUF/ornith-1.0-9b-Q4_K_M.gguf`

```
qwen35.context_length      = 262144      # = n_ctx_train
qwen35.rope.freq_base      = 10000000.0  # = 1e7
qwen35.rope.dimension_count= 64
(rope.scaling.* 키 없음 → freq_scale = 1.0, 선형 스케일링 미적용)
```

| 값 | 측정 |
|---|---|
| `n_ctx_train` | **262144** |
| `freq_base` | 1e7 |
| `freq_scale` | 1.0 (스케일링 키 부재) |

> **결론: `n_ctx_train` = 262144 ≥ 32768 → §4-1-1의 분기 전제 충족.**
> 32768 운용점은 모델의 훈련 컨텍스트 안이므로 **분기 3 직행 조건이 아니다.**
> 분기 1/2 판정은 T22가 스모크 `r_obs`로 확정한다.

---

## 2. 전 이력 공급량 실사 (§6-4-1)

⚠ **정규식이 계약이다.** 5R 리뷰가 쓰던 `(fix|close|resolve)[sd]?`는 `Fixes`·`Fixed`를
못 물어(가장 흔한 두 키워드) 후보를 대폭 과소계상했다(ripgrep 11 vs 정정 588).
정정된 `link_issues.py`(`\b(clos(?:e|es|ed)|fix(?:|es|ed)|resolv(?:e|es|ed)) +#(\d+)`)로
전 이력을 NUL 레코드 파싱해 다시 셌다 — 이것이 §6-4-1의 공식 기록이다.

full bare clone (총 32MB): zoxide 627 · fd 1992 · ripgrep 2315 · just 2283 커밋.

### 2-1. 표 (proxy = 주 분석)

| 레포 | 닫힌 이슈 | 이슈언급커밋 | 테스트동반 | 규약1·3 후(proxy) | 규약1·3 후(literal) |
|---|---|---|---|---|---|
| zoxide | 648 | 4 | 0 | **0** | 0 |
| fd | 847 | 124 | 31 | **22** | 25 |
| ripgrep | 1697 | 588 | 170 | **37** | 91 |
| just | 1283 | 14 | 1 | **0** | 0 |
| **합** | | | | **59** | **116** |

⚠ **규약 2·4·5·6은 아직 미적용** — 2/5(의존성·`.cargo` 동반)와 4(백포트 컴파일)는
T21에서 사람·컴파일로, 6(§3-4-3 지목됨)은 T21의 조달+`check`+`leak_audit.py`로
걸린다. **따라서 위 수는 상한이지 확정이 아니다.**

### 2-2. squash-merge 레포의 구조적 0 (§6-4-3 편중 계산에 직접 영향)

zoxide·just는 커밋 메시지 연결로 사실상 0이다. **squash-merge + `(#PR)` 관례**라
이슈가 PR 본문으로 닫히고 커밋 메시지에 흔적이 없다. GitHub `timeline` API로
소표본 탐침(각 40 이슈):

```
zoxide: probed=40  closed-event commit_id 발견=0  테스트동반=0
just  : probed=40  closed-event commit_id 발견=0  테스트동반=0
```

**80/80 전부 `commit_id: null`** — `.commit_id` 경로로는 회복 불가. 더 깊은
`timeline→PR→merge_commit_sha` 경로는 이슈당 다중 요청(zoxide 648 + just 1283 =
~1931 gh 호출)이 들고, 탐침이 시사하듯 구조적으로 테스트 동반 후보를 낼
가능성이 낮다. **연결 방식 차이(커밋 메시지 vs 미회복)를 사전등록에 명기한다.**

---

## 3. 임계값 대조 (§6-4-2·§6-4-3)

- **최소 표본 16**: proxy 합 59 ≫ 16 — **여유롭게 충족.** literal도 116.
  규약 2/4/5/6이 줄여도 fd+ripgrep만으로 16 이상 남을 여지가 크다.
- **편중 상한 60%**: 후보 **풀**에서는 ripgrep이 62.7%(proxy) / 78.4%(literal)로
  초과한다. 그러나 §6-4-3의 상한은 **선정 표본**에 걸린다 — ripgrep 선택 수를
  캡하면 충족된다(예: fd 22 + ripgrep 33 = 55에서 ripgrep 정확히 60%).

### 무결성 기록 — 조작적 정의의 순서 (§6-4-3)

규약 3에는 두 조작적 정의가 있다: **proxy**(커밋이 건드린 비테스트 `.rs`가
`#[cfg(test)]`를 품는가)와 **문언**(이번 커밋이 그 파일의 테스트 코드를 함께
고쳤는가). 5R 리뷰가 두 정의의 수치를 **먼저 드러냈고**, 이 선택은 그 뒤에
내려졌다. 그래서:

- **임계값 자체(16·60%)는 T1에서 데이터 이전에 동결됐다**(`d583ff8`). 조정한 것은
  임계값이 아니라 **측정 방법의 조작적 정의**다.
- **사전등록(T23)은 두 정의의 수치를 나란히 적는다** — proxy(주 분석) / 문언(민감도):
  - proxy: fd 22 + ripgrep 37 = 59, ripgrep 편중 62.7%
  - 문언: fd 25 + ripgrep 91 = 116, ripgrep 편중 78.4%

---

## 4. 레포 선정 (사용자 결정, 2026-07-21)

> **선정: fd + ripgrep 2레포. ripgrep 선택을 60%로 캡해 표본을 구성한다.**

근거와 한계:
- 두 레포만으로 동결 임계값(최소 16·편중 ≤60%)을 **선정 단계에서** 만족할 수 있다.
- ripgrep이 **9-크레이트 워크스페이스**를 고유하게 주므로 §1-1의 다중 크레이트
  항해 축은 살아 있다.
- ⚠ **한계**: just(64.7K LOC 단일 크레이트)·zoxide의 다양성을 잃는다. squash-merge
  레포의 구조적 공급 0이 원인이며, 이 2레포 한계를 **사전등록에 명기한다**(§9-A1b가
  이 제약을 함께 본다).

## 5. 남는 작업 (T21로)

- 선정 표본의 **최종 N 동결**(ripgrep ≤60% 제약 하에) — T21 표본 동결.
- 규약 2/4/5/6 적용으로 proxy 59에서 확정 수로 좁힘.
- 각 과제 조달(`procure_real.sh`) + `leak_audit.py` 지목 감사(규약 6).
- H5(심링크)는 선정과 무관하게 필수였으나 fd+ripgrep 조합에서 실제 심링크는
  ripgrep `HomebrewFormula` 하나다(§2-2 결정 8).

---

## 부록: 산출물

- 레포별 닫힌 이슈: `survey/<repo>-issues.json`
- 이슈↔커밋 연결(정정 정규식): `survey/<repo>-fixcommits.tsv`
- 테스트 동반: `survey/<repo>-with-tests.tsv`
- 규약1·3 후보(proxy): `survey/<repo>-candidates.tsv`
- `*-records.bin`(git log NUL 덤프)은 재생성 가능하므로 커밋에서 제외(`.gitignore`).


---

## 6. 재실사 추록 (2026-07-21, 처분 1)

T21 표본이 ripgrep 62.5%로 편중 상한을 넘기자 사용자가 **레포 추가 후 재실사**를 택했다.

### 6-1. 추가 bare 클론

`~/loco-real-repos/{bat,hyperfine,delta,sd,dust}.git` (기존 4 + 5 = 9레포).

### 6-2. 공급 (proxy, 정정 `link_issues.py`)

| 레포 | 닫힌 이슈 | 이슈연결 | 테스트동반 | 규약1·3 후(proxy) |
|---|---|---|---|---|
| bat | 1298 | 194 | 23 | **22** |
| hyperfine | 227 | 56 | 4 | **2** |
| delta | 622 | 175 | 23 | **3** |
| sd | 134 | 17 | 4 | **1** |
| dust | 238 | 1 | 0 | **0** |

### 6-3. 선정 갱신

- **추가 채택: `dandavison/delta`** (1과제 `delta-1089-whole-file-commit`가 전 게이트 통과)
- bat: 조달 가드 gitlink 수정 후에도 비ASCII 경로 인코딩 차로 export-ignore 오탐 → 이번 표본에서 미채택
- hyperfine·sd: 채택 가능 상한이 낮고 이번 라운드에서 게이트 통과 과제 0
- **최종 표본 N=17**: fd 6 + delta 1 + ripgrep 10 → 최대 편중 ripgrep **58.8% ≤ 60%**, 최소 16 충족

### 6-4. 조달 스크립트 수정

`scripts/procure_real.sh`: ls-tree 경로 집합에서 **mode 160000 gitlink 제외**.
`git archive`는 서브모듈을 펼치지 않으므로 gitlink를 export-ignore로 취급하면 안 된다.
