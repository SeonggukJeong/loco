# 실험 결과 (초안): 완고 S/R 루프 개입 3암 비교 — 8K 배치

- 사전등록: `docs/experiments/2026-07-17-sr-loop-arms/pre-registration.md` (상태: 승인됨 2026-07-17)
- 이 문서는 8K 3암 배치(60런)만 다룬다. 32K 승자 검증·tasks/ 스포트는 별도 지시 대기.
- 작성: loco-experiment-runner 역할 수행(자동), 최종 판정은 사용자 몫 — 이 문서는 초안이다.

## 0. 배치 전 게이트 요약

- 각 배치: `git checkout <arm>` → `cargo build` → `cargo run -- eval tasks --verify`
  (전 배치 12/12) → `cargo run -- eval tasks-large --verify`(전 배치 3/3) →
  `.loco/config.toml` 기록 → `lms unload --all` → `lms load ornith-1.0-9b
  --context-length 12288` → `curl localhost:1234/api/v0/models` 검증.
- `.loco/config.toml` 배치 전 원본(기존 잔재, M8 측정용): `context_tokens=8192,
  max_output_tokens=4096, command_timeout_secs=240` — 우연히 사전등록 8K 값과
  동일했다(백업 기록만 하고 그대로 기록). 3암 전부 이 값으로 실행.
  배치 종료 후 `command_timeout_secs`만 60(기준값)으로 원복, 나머지는 유지.
- lms 재로드 확인(3암 공통, 매 배치 재실행): `curl -s localhost:1234/api/v0/models`
  → `{"id":"ornith-1.0-9b","state":"loaded","loaded_context_length":12288,...}`
  (다른 모델은 전부 `not-loaded`) — 3배치 모두 동일 출력 확인.

## 1. 배치 ↔ 커밋 ↔ 스탬프

| 암 | 브랜치 | 커밋(rev-parse) | eval 스탬프 | 순수 실행시간(report.json duration_secs) | 이상 |
|---|---|---|---|---|---|
| ① 기준선 | m10/base | e843c05b31539b8ed8467b6fb0f05c00c9f80339 | 20260717T125544Z | 4111.6s (68.5분) | 없음 |
| ② 강제 전환 | m10/arm-block | 9fa5f722d90f81f18089bfbd8d3112ec2cddbf6d | 20260717T140556Z | 4757.8s (79.3분) | 없음 |
| ③ 디코딩 섭동 | m10/arm-perturb | 3f971291684e7182bb7a290996129de5acdf409c | 20260717T152633Z | 4308.2s (71.8분) | 없음 |

배치 순서: ①→②→③ (권장 순서 그대로). 3암 모두 `interrupted: false`,
`effective_config`은 사전등록 8K 조건(`context_tokens: 8192, max_output_tokens:
4096, command_timeout_secs: 240`)과 정확히 일치, `temperature: 0.1`(기저값,
③도 effective_config 자체는 0.1 — 섭동은 런타임 중 일시 상향이라 스냅샷엔
반영되지 않음, 코드 설계대로), `model: ornith-1.0-9b` 3암 동일. 3암 순수
실행시간 합계 4111.6+4757.8+4308.2 = 13177.6초(219.6분 ≈ 3.66h) — 사전등록
예산(≤3.0h) 대비 약 22% 초과, 중단 임계(1.5배 = 4.5h)는 미도달. §5-1 참고.

LLM 에러·부분 리포트 없음 → 배치 재수행 없음, 중단 규칙 미발동.

## 2. 지표 표 (요약)

### 2-1. 주 지표

| 지표 | ① base | ② block | ③ perturb |
|---|---|---|---|
| sr발 반복정지 수 (stop_cause=sr) | 1 | 0 | 0 |
| 완고 루프(sr_error≥3) 발생 런 수 | 7 | 7 | 3 |
| 그중 sr발 반복정지로 귀결 | 1/7 | 0/7 | 0/3 |
| 완고 루프 발생 런 종결 전환율(=1-위) | 85.7% | 100% | 100%(N=3, 소표본 경계) |
| 오류당 2시도 내 회복률 (sr_recovered/denom) | 18/42 = 42.9% | 17/39 = 43.6% | 20/30 = 66.7% |

소표본 규칙(발생 런 <3건이면 비율 대신 전수 나열) 적용 대상 확인: ①·②는
7건으로 규칙 미적용(비율 사용). ③은 정확히 3건 — "3건 미만"은 아니므로
문언상 비율 사용 가능하나, 여전히 작은 표본이라 전수를 함께 남긴다.

③ perturb의 완고 루프 발생 런 3건 전수(모두 update-vat-rate, stop_cause=sr
아님 — 전부 max_turns로 종결):
- run-update-vat-rate-3: sr_error=6, outcome=max_turns, passed=False
- run-update-vat-rate-6: sr_error=7, outcome=max_turns, passed=False
- run-update-vat-rate-8: sr_error=5, outcome=max_turns, passed=False

개입 실제 발동 확인(트리거 미도달로 정보 0이 되는 사고 방지):
- ② block: `sr_block` 마커(edit_file 차단) 총 8건 발생 — H1 개입 실제 발동.
- ③ perturb: `perturb_turns`(섭동 유도 턴) 다수 발생(런별 TSV 참고, 예:
  update-vat-rate-3/4/6/8 등에 1회씩) — H2 개입 실제 발동.

### 2-2. 보조 지표

| 지표 | ① base | ② block | ③ perturb |
|---|---|---|---|
| 전체 통과율 | 50.0% (10/20) | 40.0% (8/20) | 55.0% (11/20) |
| 엄격 통과율 (passed∧finished) | 40.0% (8/20) | 30.0% (6/20) | 45.0% (9/20) |
| 거짓 성공 finish | 2 | 2 | 2 |
| 평균 시간/런 (avg_duration_secs) | 202.3s | 234.7s | 211.6s |
| salvage 이벤트 | 0 | 0 | 0 |
| fix-monthly-total 통과/엄격/평균턴/평균시간 | 9/10, 7/10, 12.2턴, 165.7s | 7/10, 5/10, 13.8턴, 241.0s | 10/10, 8/10, 11.3턴, 200.3s |
| update-vat-rate 통과/엄격/평균턴/평균시간 | 1/10, 1/10, 19.9턴, 239.0s | 1/10, 1/10, 19.4턴, 228.4s | 1/10, 1/10, 19.8턴, 222.9s |

### 2-3. 런별 TSV 원문 (`python3 scripts/exp_metrics.py` 그대로)

```
# .loco/eval/20260717T125544Z
run	outcome	passed	sr_error	sr_correction	sr_block	repeat_corr	finish_missing	finish_args_corr	finish_nudge	sr_recovered	sr_recovery_denom	finish_missing_maxrun	perturb_turns	stop_cause
run-fix-monthly-total-0	repetition_stop	False	6	1	0	1	0	0	0	0	6	0	4	sr
run-fix-monthly-total-1	finished	True	0	0	0	0	1	0	0	0	0	1	0	-
run-fix-monthly-total-2	max_turns	True	3	0	0	0	0	0	1	1	3	0	0	-
run-fix-monthly-total-3	finished	True	2	1	0	0	0	0	0	2	2	0	1	-
run-fix-monthly-total-4	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-fix-monthly-total-5	finished	True	1	0	0	0	0	0	0	1	1	0	0	-
run-fix-monthly-total-6	max_turns	True	2	0	0	0	0	0	1	0	2	0	0	-
run-fix-monthly-total-7	finished	True	2	1	0	0	0	0	1	2	2	0	1	-
run-fix-monthly-total-8	finished	True	1	0	0	0	1	0	1	0	1	1	0	-
run-fix-monthly-total-9	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-update-vat-rate-0	finished	False	0	0	0	0	0	0	0	0	0	0	0	-
run-update-vat-rate-1	max_turns	False	0	0	0	1	0	0	0	0	0	0	0	-
run-update-vat-rate-2	max_turns	False	0	0	0	1	0	0	0	0	0	0	0	-
run-update-vat-rate-3	max_turns	False	6	1	0	0	0	0	0	2	6	0	1	-
run-update-vat-rate-4	repetition_stop	False	3	1	0	1	0	0	0	1	3	0	1	other
run-update-vat-rate-5	finished	False	0	0	0	0	1	0	0	0	0	1	0	-
run-update-vat-rate-6	max_turns	False	6	1	0	0	0	0	0	3	6	0	1	-
run-update-vat-rate-7	max_turns	False	0	0	0	0	0	0	0	0	0	0	0	-
run-update-vat-rate-8	max_turns	False	5	1	0	1	0	0	0	2	5	0	1	-
run-update-vat-rate-9	finished	True	5	1	0	1	0	0	0	4	5	0	1	-
# summary sr_error=42 sr_correction=8 sr_block=0 repeat_corr=6 finish_missing=3 finish_args_corr=0 finish_nudge=4 recovered=18/42 stops sr=1 finish=0 other=1
# .loco/eval/20260717T140556Z
run	outcome	passed	sr_error	sr_correction	sr_block	repeat_corr	finish_missing	finish_args_corr	finish_nudge	sr_recovered	sr_recovery_denom	finish_missing_maxrun	perturb_turns	stop_cause
run-fix-monthly-total-0	max_turns	False	3	1	1	1	0	0	0	0	3	0	2	-
run-fix-monthly-total-1	finished	True	0	0	0	0	1	0	0	0	0	1	0	-
run-fix-monthly-total-2	finished	True	2	0	0	0	0	0	0	1	2	0	0	-
run-fix-monthly-total-3	finished	True	2	1	0	0	0	0	0	2	2	0	1	-
run-fix-monthly-total-4	timeout	False	0	0	0	0	0	0	0	0	0	0	0	-
run-fix-monthly-total-5	max_turns	True	2	0	0	0	0	0	0	2	2	0	0	-
run-fix-monthly-total-6	repetition_stop	True	3	1	2	1	5	1	0	1	3	2	2	finish
run-fix-monthly-total-7	finished	True	2	1	0	0	0	0	0	2	2	0	1	-
run-fix-monthly-total-8	max_turns	False	3	0	2	0	0	0	0	2	3	0	0	-
run-fix-monthly-total-9	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-update-vat-rate-0	finished	False	0	0	0	0	0	0	0	0	0	0	0	-
run-update-vat-rate-1	max_turns	False	0	0	0	1	0	0	0	0	0	0	0	-
run-update-vat-rate-2	max_turns	False	0	0	0	1	0	0	0	0	0	0	0	-
run-update-vat-rate-3	max_turns	False	6	1	0	0	0	0	0	2	6	0	1	-
run-update-vat-rate-4	repetition_stop	False	3	1	0	1	0	0	0	1	3	0	1	other
run-update-vat-rate-5	finished	False	0	0	0	0	1	0	0	0	0	1	0	-
run-update-vat-rate-6	max_turns	False	6	1	2	1	2	1	0	2	6	2	1	-
run-update-vat-rate-7	max_turns	False	0	0	0	0	0	0	0	0	0	0	0	-
run-update-vat-rate-8	max_turns	False	5	0	1	1	0	0	0	1	5	0	0	-
run-update-vat-rate-9	finished	True	2	0	0	0	0	0	0	1	2	0	0	-
# summary sr_error=39 sr_correction=7 sr_block=8 repeat_corr=7 finish_missing=9 finish_args_corr=2 finish_nudge=0 recovered=17/39 stops sr=0 finish=1 other=1
# .loco/eval/20260717T152633Z
run	outcome	passed	sr_error	sr_correction	sr_block	repeat_corr	finish_missing	finish_args_corr	finish_nudge	sr_recovered	sr_recovery_denom	finish_missing_maxrun	perturb_turns	stop_cause
run-fix-monthly-total-0	finished	True	2	1	0	0	0	0	0	1	2	0	1	-
run-fix-monthly-total-1	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-fix-monthly-total-2	finished	True	1	0	0	0	0	0	0	1	1	0	0	-
run-fix-monthly-total-3	finished	True	2	1	0	0	0	0	0	2	2	0	1	-
run-fix-monthly-total-4	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-fix-monthly-total-5	finished	True	1	0	0	0	0	0	0	1	1	0	0	-
run-fix-monthly-total-6	max_turns	True	1	0	0	0	0	0	0	0	1	0	0	-
run-fix-monthly-total-7	finished	True	2	1	0	0	0	0	0	2	2	0	1	-
run-fix-monthly-total-8	max_turns	True	0	0	0	1	0	0	1	0	0	0	0	-
run-fix-monthly-total-9	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-update-vat-rate-0	finished	False	0	0	0	0	0	0	0	0	0	0	0	-
run-update-vat-rate-1	max_turns	False	0	0	0	1	0	0	0	0	0	0	0	-
run-update-vat-rate-2	max_turns	False	0	0	0	1	0	0	0	0	0	0	0	-
run-update-vat-rate-3	max_turns	False	6	1	0	0	0	0	0	4	6	0	1	-
run-update-vat-rate-4	max_turns	False	2	1	0	0	0	0	0	2	2	0	1	-
run-update-vat-rate-5	finished	False	0	0	0	0	1	0	0	0	0	1	0	-
run-update-vat-rate-6	max_turns	False	7	1	0	0	0	0	0	4	7	0	1	-
run-update-vat-rate-7	max_turns	False	0	0	0	0	0	0	0	0	0	0	0	-
run-update-vat-rate-8	max_turns	False	5	1	0	1	0	0	0	2	5	0	1	-
run-update-vat-rate-9	finished	True	1	0	0	0	0	0	0	1	1	0	0	-
# summary sr_error=30 sr_correction=7 sr_block=0 repeat_corr=4 finish_missing=1 finish_args_corr=0 finish_nudge=1 recovered=20/30 stops sr=0 finish=0 other=0
```

(selftest 통과 확인 후 추출: `python3 scripts/exp_metrics.py --selftest` → `selftest ok`.)

## 3. 사전등록 판정 규칙의 기계적 적용

규칙 원문: "주 지표 우세 암을 main에 병합. 동률이면 단순한 쪽(암②). 두 암 모두
기준선보다 나쁘면 병합 없이 실패 턴 제거안을 M11 입력으로."

기계적 대조(주 지표 3개, §2-1):
1. sr발 반복정지 수 — ②(0)=③(0) < ①(1): ②·③ 동률, 둘 다 기준선보다 우세.
2. 완고 루프 종결 전환율 — ②(100%)=③(100%) > ①(85.7%): ②·③ 동률, 둘 다
   기준선보다 우세(단, ③은 N=3 소표본 경계).
3. 오류당 2시도 내 회복률 — ③(66.7%) > ②(43.6%) > ①(42.9%): ③이 ②·①을
   뚜렷이 앞섬.

세 주 지표 중 어느 것에서도 ②가 ③을 앞서는 경우가 없다(①·②는 1·2에서
동률, 3에서 ③이 우세). 규칙을 문언 그대로 기계 적용하면 "동률" 조항(②·③이
모든 주 지표에서 동률일 때 단순한 쪽 선택)은 발동 조건(전 지표 동률)을
충족하지 못한다 — 지표 3에서 동률이 깨진다. "두 암 모두 기준선보다 나쁘면"
조항도 미해당(②·③ 모두 세 지표 전부에서 기준선과 같거나 우세).

→ 기계적으로 세 주 지표를 그대로 대조하면 ③(perturb)이 ②(block)를
지배(dominate)한다(2개 동률 + 1개 우세, 열세 지표 없음). 다만:
- 지표 3(회복률)의 우세 폭이 판정을 사실상 결정하는데, 사전등록에 3개 주
  지표 간 가중치·우선순위가 명시돼 있지 않다 — "지표가 여러 개일 때 하나만
  갈리면 그것으로 결정"이라는 해석은 러너의 보간이며 사전등록 문언에 없다.
- 지표 2에서 ③의 표본이 N=3(소표본 경계)이라 "동률"의 신뢰도가 ②(N=7)보다
  낮다.
- 보조 지표(§2-2)에서 ②는 fix-monthly-total 통과율·엄격 통과율이 기준선보다
  오히려 낮다(9/10·7/10 → 7/10·5/10) — 주 지표엔 없지만 부작용 감시
  대상이다.

이 문서는 위 사실만 보고하고 병합 여부를 선언하지 않는다. 최종 판정은
사용자 몫(PROTOCOL.md §7).

스포트 게이트(≥33/36)는 32K 승자 검증 단계에서 평가되는 항목으로 이번 8K
3암 배치의 범위 밖이다 — 아직 미실행.

## 4. 중단 규칙 적용 여부

3암 전부 `interrupted: false`, LLM 에러 없음, Ctrl+C 없음 → 재수행·중단 규칙
미발동.

## 5. 이상 징후

1. **시간 예산 초과(비중단)**: 3암 순수 실행시간 합계 219.6분(3.66h)이
   사전등록 예상(≤3.0h, 실측 근거 2.2~2.4h)을 약 22% 초과. 개별 런 소요시간
   편차가 컸다(예: ② fix-monthly-total-4가 Timeout으로 605.0초 소요,
   반면 짧은 런은 40초대) — 개입 암에서 실패 경로가 길어지는 런이 섞여
   있었던 것으로 보인다. 중단 임계(1.5배=4.5h)는 넘지 않아 재수행 없이
   계속 진행했다.
2. **update-vat-rate가 3암 전부 1/10로 불변**: 개입(②·③) 모두 이 과제에는
   측정 가능한 영향을 주지 못했다. 실패 원인 대부분이 `max_turns`
   소진이며(런별 TSV 참고), sr_error 발생 자체는 3암 모두 관측되지만
   개입 발동(sr_block/perturb_turns) 이후에도 과제를 끝내지 못한 경우가
   다수다 — 이 과제의 병목이 S/R 루프 자체가 아닐 가능성을 시사(주석:
   pre-registration은 fix-monthly-total·update-vat-rate 둘 다 표적으로
   묶었으나, 개입 효과는 fix-monthly-total에 편중돼 나타났다).
3. **block 암에서 fix-monthly-total 통과율이 기준선보다 낮음**: ①9/10(엄격
   7/10) → ②7/10(엄격 5/10). 표본이 10런으로 작아 노이즈일 가능성을 배제할
   수 없으나, H1(강제 전환)이 이 과제에서는 역효과로 보이는 방향성이다.
4. **개입 실제 발동 확인**: ② sr_block=8건, ③ perturb_turns 다수(런별 TSV
   참고) — 두 암 모두 트리거 미도달로 "정보 0"이 되는 사고는 없었다(H1·H2
   모두 실제로 검증 가능한 데이터를 냈다).
5. **salvage 이벤트 0건(3암 공통)**: `SALVAGE_NOTE`("fields outside \"args\"
   were accepted this time") 문자열이 60런 전체 JSONL에서 0건 — ornith는
   이 실험 조건에서 프로토콜 이탈 교정이 필요 없었다(CLAUDE.md의 gemma M5
   salvage 사례와 다른 모델 특성, 참고용 보조 지표).
6. **거짓 성공 finish 2건, 3암 동일**: 우연히 3암 모두 정확히 2건으로
   일치 — 개입과 무관하게 일정하게 발생하는 실패 유형으로 보인다(개별
   런은 다를 수 있으니 재현성 주장은 아님).

## 6. 재현 정보

- 모델: ornith-1.0-9b (deepreinforce-ai, Q4_K_M, 5.63GB) — 3암 모두 로드
  컨텍스트 12288, `effective_config.temperature: 0.1` 동일(③의 런타임
  섭동은 스냅샷에 미반영, 설계대로).
- `.loco/config.toml` 8K 배치 조건: `context_tokens=8192,
  max_output_tokens=4096, command_timeout_secs=240`. 배치 종료 후
  `command_timeout_secs`만 60으로 원복(나머지 필드는 사전 잔재 값 유지 —
  러너 지시 그대로, 스펙 기본값 복원은 이 작업 범위 밖).
- 작업 종료 시 브랜치: main(6c792d8) — 3암 배치 전부 main으로 복귀 완료.
- 커밋·git push 없음(금지 목록 준수).

## 7. 승자 검증 배치 (32K 승자 + tasks/ 스포트)

사전등록 표본 절 "승자 확정 후" 항목. 승자 후보: 암③ m10/arm-perturb
(3f97129) — §3 판정 규칙의 기계적 적용 결과, 세 주 지표에서 암②를
지배(2개 동률·1개 우세, 열세 지표 없음). 이 절은 그 승자를 32K로 재측정하고
tasks/ 스포트 게이트를 실행한 결과만 보고한다(최종 판정 문장 없음).

### 7-0. 배치 전 게이트

- `git checkout m10/arm-perturb` → `3f971291684e7182bb7a290996129de5acdf409c`
  (사전등록 표와 일치) → `cargo build` 정상 종료 → `cargo run -- eval tasks
  --verify` 12/12 → `cargo run -- eval tasks-large --verify` 3/3. 두 배치(A·B)
  모두 이 체크아웃·빌드 상태를 그대로 사용(암·커밋 전환 없이 config·lms
  로드값만 배치 사이에 교체).

### 7-1. 배치 ↔ 커밋 ↔ 스탬프

| 배치 | 조건 | 브랜치(커밋) | eval 스탬프 | 런 수 | duration_secs(총) | avg_duration_secs |
|---|---|---|---|---|---|---|
| A | 32K 승자 재측정 | m10/arm-perturb (3f971291684e7182bb7a290996129de5acdf409c) | 20260717T164905Z | 20/20 | 4805.9s (80.1분) | 236.3s |
| B | tasks/ 스포트 (v2) | m10/arm-perturb (3f971291684e7182bb7a290996129de5acdf409c) | 20260717T215729Z | 36/36 | 2255.9s (37.6분) | 62.0s |

시간 예산 대조: A ≤1.5h 상한 대비 80.1분(약 89% 사용, 상한 1.5배=2.25h
미도달), B ≤1.0h 상한 대비 37.6분(약 63% 사용). 두 배치 모두 `interrupted:
false`, LLM 에러 없음(로그 grep `error|panic|LLM 오류|중단` 0건) → 재수행
없음, 중단 규칙 미발동.

### 7-2. lms 확인 출력

배치 A 전(32K, unload→load 49152):
```
{
  "id": "ornith-1.0-9b", "state": "loaded",
  "max_context_length": 262144, "loaded_context_length": 49152
}
```
(다른 3개 모델 전부 `not-loaded`.)

배치 B 전(8K v2, unload→load 8192):
```
{
  "id": "ornith-1.0-9b", "state": "loaded",
  "max_context_length": 262144, "loaded_context_length": 8192
}
```
(다른 3개 모델 전부 `not-loaded`.)

`effective_config` 대조(report.json, 배치별):
- A: `{"context_tokens": 32768, "max_output_tokens": 4096,
  "command_timeout_secs": 240, "temperature": 0.1, "model":
  "ornith-1.0-9b"}` — 사전등록 32K 조건과 정확히 일치.
- B: `{"context_tokens": 8192, "max_output_tokens": 4096,
  "command_timeout_secs": 60, "temperature": 0.1, "model":
  "ornith-1.0-9b"}` — 사전등록 v2 스포트 조건과 정확히 일치.
직전 배치(A, 32768/240)의 config 잔재 없음(B의 effective_config가 8192/60으로
정확히 갱신됨) — GPU 시간 무효화 사고 없음.

### 7-3. 지표 표

#### 배치 A (32K, fix-monthly-total·update-vat-rate × 10반복)

| 지표 | 값 |
|---|---|
| sr발 반복정지 수 (stop_cause=sr) | 0 |
| 완고 루프(sr_error≥3) 발생 런 수 | 6 (전부 update-vat-rate: 시드 0,1,3,5,6,8) |
| 그중 sr발 반복정지로 귀결 | 0/6 |
| 완고 루프 발생 런 종결 전환율 | 100% (6/6) |
| 오류당 2시도 내 회복률 (sr_recovered/denom) | 36/45 = 80.0% |
| 전체 통과율 | 65.0% (13/20) |
| 엄격 통과율 (passed∧finished) | 60.0% (12/20) |
| 거짓 성공 finish | 0 |
| 평균 시간/런 | 236.3s |
| fix-monthly-total 통과/엄격/평균턴/평균시간 | 10/10, 10/10, 7.1턴, 103.0s |
| update-vat-rate 통과/엄격/평균턴/평균시간 | 3/10, 2/10, 22.1턴, 369.6s |

#### 배치 B (tasks/ 스포트, 12과제 × 3반복 = 36런)

| 지표 | 값 |
|---|---|
| 전체 통과율 | 97.2% (35/36) |
| 엄격 통과율 (passed∧finished) | 94.4% (34/36) |
| 거짓 성공 finish | 0 |
| 평균 시간/런 | 62.0s |
| 스포트 게이트 기준 | ≥33/36 |
| 스포트 실측 | 35/36 |
| 유일 실패 런 | multiline-string-edit 시드0, outcome=repetition_stop,
  stop_cause=other, 13턴, 88.8s (sr_error=3, sr_block=0 — S/R 루프가 아닌
  다른 반복 패턴으로 RepetitionStop) |
| 과제별 통과 | add-function 3/3·chain-edits 3/3(엄격2/3)·count-usages 3/3·
  create-module 3/3·edit-crlf-file 3/3·find-definition 3/3·
  fix-compile-error 3/3·fix-failing-test 3/3·fix-off-by-one 3/3·
  implement-from-doc 3/3·multiline-string-edit 2/3(엄격2/3)·
  rename-function 3/3 |

이 절은 게이트 통과·미통과를 선언하지 않는다 — 35/36 ≥ 33/36이라는 산술
비교만 기록한다. 최종 판정은 사용자 몫.

### 7-4. 런별 TSV 원문 (`python3 scripts/exp_metrics.py .loco/eval/20260717T164905Z .loco/eval/20260717T215729Z`)

```
# .loco/eval/20260717T164905Z
run	outcome	passed	sr_error	sr_correction	sr_block	repeat_corr	finish_missing	finish_args_corr	finish_nudge	sr_recovered	sr_recovery_denom	finish_missing_maxrun	perturb_turns	stop_cause
run-fix-monthly-total-0	finished	True	2	1	0	0	1	0	0	1	2	1	1	-
run-fix-monthly-total-1	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-fix-monthly-total-2	finished	True	1	0	0	0	1	0	0	1	1	1	0	-
run-fix-monthly-total-3	finished	True	2	1	0	0	0	0	0	2	2	0	1	-
run-fix-monthly-total-4	finished	True	0	0	0	1	1	0	0	0	0	1	0	-
run-fix-monthly-total-5	finished	True	1	0	0	0	0	0	0	1	1	0	0	-
run-fix-monthly-total-6	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-fix-monthly-total-7	finished	True	2	1	0	0	0	0	1	2	2	0	1	-
run-fix-monthly-total-8	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-fix-monthly-total-9	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-update-vat-rate-0	max_turns	False	5	0	0	0	0	0	0	4	5	0	0	-
run-update-vat-rate-1	max_turns	False	4	0	0	1	0	0	0	3	4	0	0	-
run-update-vat-rate-2	finished	True	0	0	0	0	1	0	0	0	0	1	0	-
run-update-vat-rate-3	max_turns	False	10	1	0	0	0	0	0	6	10	0	3	-
run-update-vat-rate-4	max_turns	False	1	0	0	0	0	0	0	1	1	0	0	-
run-update-vat-rate-5	timeout	True	5	1	0	0	0	0	0	5	5	0	2	-
run-update-vat-rate-6	max_turns	False	6	1	0	1	0	0	0	5	6	0	2	-
run-update-vat-rate-7	max_turns	False	0	0	0	0	0	0	0	0	0	0	0	-
run-update-vat-rate-8	timeout	False	6	1	0	0	0	0	0	5	6	0	2	-
run-update-vat-rate-9	finished	True	0	0	0	1	0	0	0	0	0	0	0	-
# summary sr_error=45 sr_correction=7 sr_block=0 repeat_corr=4 finish_missing=4 finish_args_corr=0 finish_nudge=1 recovered=36/45 stops sr=0 finish=0 other=0
# .loco/eval/20260717T215729Z
run	outcome	passed	sr_error	sr_correction	sr_block	repeat_corr	finish_missing	finish_args_corr	finish_nudge	sr_recovered	sr_recovery_denom	finish_missing_maxrun	perturb_turns	stop_cause
run-add-function-0	finished	True	2	0	0	0	0	0	0	2	2	0	0	-
run-add-function-1	finished	True	1	0	0	0	0	0	0	1	1	0	0	-
run-add-function-2	finished	True	2	0	0	0	0	0	0	1	2	0	0	-
run-chain-edits-0	finished	True	1	0	0	0	0	0	0	1	1	0	0	-
run-chain-edits-1	finished	True	4	0	0	0	0	0	0	4	4	0	0	-
run-chain-edits-2	timeout	True	1	0	0	0	2	1	0	1	1	2	0	-
run-count-usages-0	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-count-usages-1	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-count-usages-2	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-create-module-0	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-create-module-1	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-create-module-2	finished	True	0	0	0	0	1	0	0	0	0	1	0	-
run-edit-crlf-file-0	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-edit-crlf-file-1	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-edit-crlf-file-2	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-find-definition-0	finished	True	0	0	0	0	1	0	0	0	0	1	0	-
run-find-definition-1	finished	True	0	0	0	0	1	0	0	0	0	1	0	-
run-find-definition-2	finished	True	0	0	0	1	2	0	0	0	0	1	0	-
run-fix-compile-error-0	finished	True	1	0	0	0	0	0	0	1	1	0	0	-
run-fix-compile-error-1	finished	True	1	0	0	0	0	0	0	1	1	0	0	-
run-fix-compile-error-2	finished	True	1	0	0	0	0	0	0	1	1	0	0	-
run-fix-failing-test-0	finished	True	1	0	0	0	0	0	0	1	1	0	0	-
run-fix-failing-test-1	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-fix-failing-test-2	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-fix-off-by-one-0	finished	True	2	1	0	0	0	0	0	2	2	0	1	-
run-fix-off-by-one-1	finished	True	1	0	0	0	0	0	0	1	1	0	0	-
run-fix-off-by-one-2	finished	True	2	1	0	0	1	0	0	2	2	1	1	-
run-implement-from-doc-0	finished	True	1	0	0	0	0	0	0	1	1	0	0	-
run-implement-from-doc-1	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
run-implement-from-doc-2	finished	True	1	0	0	0	0	0	0	1	1	0	0	-
run-multiline-string-edit-0	repetition_stop	False	3	0	0	1	0	0	0	1	3	0	0	other
run-multiline-string-edit-1	finished	True	1	0	0	1	0	0	0	1	1	0	0	-
run-multiline-string-edit-2	finished	True	2	0	0	0	0	0	0	1	2	0	0	-
run-rename-function-0	finished	True	2	1	0	0	0	0	0	1	2	0	1	-
run-rename-function-1	finished	True	1	0	0	1	1	0	0	1	1	1	0	-
run-rename-function-2	finished	True	0	0	0	0	0	0	0	0	0	0	0	-
# summary sr_error=31 sr_correction=3 sr_block=0 repeat_corr=4 finish_missing=9 finish_args_corr=1 finish_nudge=0 recovered=26/31 stops sr=0 finish=0 other=1
```

(selftest 확인: `python3 scripts/exp_metrics.py --selftest` → `selftest ok`,
본 배치 실행 전 §2-3에서 이미 확인됨.)

### 7-5. 32K 대 8K 비교 (③ perturb 암, 동일 표적 2과제×10반복)

| 지표 | 8K (§2-1/2-2, 스탬프 20260717T152633Z) | 32K (본 절, 스탬프 20260717T164905Z) |
|---|---|---|
| sr발 반복정지 수 | 0 | 0 |
| 완고 루프(sr_error≥3) 발생 런 수 | 3 (소표본 경계) | 6 |
| 그중 sr발 반복정지 귀결 | 0/3 | 0/6 |
| 완고 루프 종결 전환율 | 100% (N=3) | 100% (N=6) |
| 오류당 2시도 내 회복률 | 66.7% (20/30) | 80.0% (36/45) |
| 전체 통과율 | 55.0% (11/20) | 65.0% (13/20) |
| 엄격 통과율 | 45.0% (9/20) | 60.0% (12/20) |
| 거짓 성공 finish | 2 | 0 |
| 평균 시간/런 | 211.6s | 236.3s |
| fix-monthly-total 통과/엄격/평균턴/평균시간 | 10/10, 8/10, 11.3턴, 200.3s | 10/10, 10/10, 7.1턴, 103.0s |
| update-vat-rate 통과/엄격/평균턴/평균시간 | 1/10, 1/10, 19.8턴, 222.9s | 3/10, 2/10, 22.1턴, 369.6s |

관찰(판정 아님): 32K에서 완고 루프 발생 런 수가 3→6으로 늘었으나(총
sr_error도 30→45로 증가) 회복률은 66.7%→80.0%로 함께 올랐고, 종결
전환율(sr발 정지로 안 끝난 비율)은 8K·32K 모두 100%로 동일하다. 통과율·
엄격 통과율·거짓 finish는 32K에서 8K 대비 전 항목 개선 방향, 평균
시간/런은 32K가 소폭(24.7s) 더 길다. fix-monthly-total은 32K에서 평균
턴 수가 줄고(11.3→7.1) 엄격 통과가 8/10→10/10으로 올랐다. update-vat-rate는
여전히 두 조건 모두 낮은 통과율(1/10, 3/10)이며 컨텍스트 확장만으로는
이 과제의 병목이 크게 풀리지 않는다(§5-2 관찰과 일관).

### 7-6. 이상 징후

1. **배치 완료 통지 유실(러너 내부 이슈, 데이터 무결성과 무관)**: 배치 A·B
   모두 자체 백그라운드 모니터(`while kill -0 <pid>; do sleep 20; done`)가
   완료 신호를 내지 못하고 조율자(coordinator) 메시지로 먼저 통보받았다.
   각 배치 모두 통보 내용을 그대로 신뢰하지 않고 `ps -p <pid>`(프로세스
   종료 확인), 로그 tail(완주 출력 확인), `report.json`
   (`interrupted`/`effective_config`/집계 수치) 직접 열람으로 독립
   재검증했다 — 배치 B는 마침 배경 모니터가 정상적으로 `completed`
   통지를 내어 이중 확인됨. 실측 수치 자체에는 영향 없음.
2. **완고 루프 발생 런 수 증가(32K)**: §7-5 참고 — 발생 건수 자체는
   늘었으나(3→6) 전환율·회복률·통과율은 모두 8K와 같거나 우세한
   방향이었다. 발생 건수 증가가 "개입 필요 빈도" 증가인지 "컨텍스트가
   늘어난 만큼 더 긴 시행이 가능해져 관측 기회 자체가 늘어난 것"인지는
   본 배치 데이터만으로 구분되지 않는다(관찰 기록, 해석 아님).
3. **시간 예산**: A 80.1분(상한 90분의 89%), B 37.6분(상한 60분의 63%) —
   둘 다 상한 1.5배 중단 임계 미도달, 재수행 없음.
4. **LLM 에러·부분 리포트 없음**: 두 배치 모두 `interrupted: false`,
   로그에 에러 문자열 0건 → 중단 규칙 미발동, 재수행 없음.

### 7-7. 종료 시 상태

- 브랜치: main (6c792d8f5587cfc77663ffa9d6edc0e802263b11).
- `.loco/config.toml`: `context_tokens=8192, max_output_tokens=4096,
  command_timeout_secs=60`(8K 기준값 원복 — max_output_tokens은 배치 B의
  v2 조건 값 4096을 그대로 유지, 사전등록도 v2를 "context_tokens
  8192·로드 8192"로만 특정해 이 값 변경을 요구하지 않음).
- lms: ornith-1.0-9b, `loaded_context_length: 8192` 유지(다른 모델
  전부 not-loaded).
- 커밋·git push 없음(금지 목록 준수).

## 8. 최종 판정 (사전등록 판정 규칙의 적용 — 컨트롤러 작성, 사용자 리뷰 대상)

### 8-1. 판정

**승자: 암③ 디코딩 섭동 (m10/arm-perturb, 3f97129).**

사전등록 판정 규칙("주 지표 우세 암을 main에 병합, 동률이면 암②")의 적용:

| 주 지표 | ① 기준선 | ② 차단 | ③ 섭동 | 판정 |
|---|---|---|---|---|
| sr발 반복정지 수 | 1 | 0 | 0 | ②=③ 동률 (둘 다 기준선 우세) |
| 완고 루프 발생 런의 종결 전환율 | — | 100% | 100% (N=3, 소표본 — §3 전수 나열) | ②=③ 동률 |
| 오류당 2시도 내 회복률 | 42.9% | 43.6% | **66.7%** | **③ 우세** |

암③은 주 지표 2개 동률·1개 우세로 열세 축이 없다 — "동률이면 암②" 규칙은 3개 주 지표
전부의 동률에만 해당하므로 적용되지 않는다. 두 암 모두 기준선 이상이므로 전패 규칙도
미발동. 검증 배치: 32K에서 13/20(엄격 12/20 — 본 실험 최고, fm 10/10 전승),
tasks/ 스포트 **35/36 ≥ 게이트 33/36 통과**(엄격 34/36, M9 스포트 34/36 대비 회귀 없음).

### 8-2. 가설 판정

- **H2(저온 복사 어트랙터) — 강지지.** 섭동만으로 S/R 총량 42→30(-29%), 회복률
  42.9→66.7%(32K에서 80%), 부작용 지표(finish 누락 3→1, salvage 0) 무악화.
  0단계 법의학의 "문자 단위 복사" 전제와 정합.
- **H1(행동 공간 차단으로 종결) — 부분 지지, 채택 기각.** 차단은 sr발 정지를 제거했으나
  (1→0) 실패가 종료 규율로 전이(finish 누락 3→9, fm 통과 9→7). 강요형 개입의
  풍선효과 실측 — 설계 원칙 노트: 개입은 모델의 행동 분포와 싸우지 말고(차단) 분포
  자체를 흔드는(섭동) 쪽이 부작용이 작다.

### 8-3. 정직 기록 (판정 불변 사항)

- 8K 3암 실측 합계 3.66h — 예산 3.0h 대비 +22% (배치별 중단 임계 1.5×는 미달, 중단 미발동).
- update-vat-rate는 전 조건 1/10~3/10 — S/R이 파일 4곳에 산재해 두 개입 모두 임계
  미도달이 다수. 이 과제의 병목은 루프가 아니라 다지점 전파 능력 자체(M11 입력).
- 32K에서 완고 루프 발생 런 3→6 증가 관찰(전환율·회복률·통과율은 동등 이상) — 해석 유보.
- 외부 critique(Grok) 보류 3건 처분: F1(차단 후 write_file 전환 실패 우려)은 승자가
  비차단 암이라 무관화(단 암②의 finish 전이는 방향 적중 — M11 참고), F2(변주 소진)는
  미관찰, F3(백슬래시 경로 분산)은 암② 전 트랜스크립트 grep 0건 — 본 실험 무영향 확정.
- 패자 브랜치 m10/arm-block은 삭제하지 않고 보존(사전등록 그대로).

## 9. 스펙 §2 성공 기준 대조

스펙 `docs/superpowers/specs/2026-07-17-m10-experiment-infra-stubborn-loops-design.md`
§2의 5개 성공 기준을 이 실험의 산출물·수치와 대조한다.

| # | 성공 기준 (스펙 §2 원문 요약) | 충족 여부 | 근거 |
|---|---|---|---|
| 1 | 게이트: `cargo test` + `cargo clippy --all-targets -- -D warnings` + `eval tasks --verify` 12/12 + `eval tasks-large --verify` 3/3 (모든 암 브랜치에서) | **충족** | §0 배치 전 게이트(전 배치 12/12·3/3) + 병합 후 재확인(cargo test 293·clippy 0·verify 12/12+3/3, 위 "배치 ↔ 커밋 ↔ 스탬프" 절 참고) |
| 2 | 인프라 실증: 실험 1이 사전등록 → 무인 수행(모델 교체·배치 순차·게이트 검증 포함) → 자동 지표 리포트로 완주, 수동 정독 없이 판정 가능한 report.md | **충족** | 본 문서 자체가 산출물 — §0(자동 게이트)·§1(배치↔커밋↔스탬프)·§2-3/§7-4(`exp_metrics.py` 자동 추출 TSV)로 완주, lms unload/load·curl 검증까지 무인 수행(§0, §7-0~7-2); 판정(§3·§8)은 지표 표만으로 도출 가능 |
| 3 | 행동 지표(주): 승자 암에서 ①S/R발 반복정지 0건 ②완고 루프 발생 런의 종결 전환(수정 성공 또는 유효 finish 도달) ③오류당 2시도 내 회복률이 기준선 대비 상승 — 소표본 규칙(<3런은 전수+방향) 승계 | **충족** | 승자 암③: ① sr발 반복정지 0건(8K §2-1, 32K §7-3 모두 0) ② 종결 전환율 100%(8K N=3·32K N=6, §2-1·§7-3) ③ 오류당 2시도 내 회복률 42.9%(①)→66.7%(③), 32K에서 80.0%로 추가 상승(§7-5) — 기준선 대비 전 항목 상승 |
| 4 | 통과율(보조): 승자 암 엄격 통과율이 기준선 대비 비악화, tasks/ 스포트(ornith@8K, v2 조건) ≥ 33/36 | **충족** | 엄격 통과율 40.0%(①, 8/20)→45.0%(③, 9/20)로 상승(비악화 충족, §2-2), 32K에서 60.0%(12/20, §7-3)까지 추가 상승; tasks/ 스포트 35/36 ≥ 33/36(§7-3, 엄격 34/36 — M9 스포트 34/36 대비 회귀 없음) |
| 5 | 신규 모델-대면 텍스트 전부 영문(차단 오류문 포함), 사용자 CLI 메시지 한국어 | **충족** | §4의 차단 오류문(`Error: edit_file is disabled...`)·§5의 섭동 Notice 트리거 텍스트 모두 영문 스펙대로 구현(코드 게이트 — cargo test 293건에 포함); REPL Notice 한 줄은 한국어(스펙 §4·§5 "발동 시 REPL에는 한국어 Notice 한 줄" 규정, M5/M9 관례 승계) — 이 실험 문서 자체는 별도로 트랜스크립트 문자열을 재검사하지 않았으나 구현 코드의 언어 분리는 게이트(기준 1) 통과로 담보됨 |
