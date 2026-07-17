# 실험 사전등록: 완고 S/R 루프 개입 3암 비교

- 날짜/디렉토리: docs/experiments/2026-07-17-sr-loop-arms/
- 스펙 근거: docs/superpowers/specs/2026-07-17-m10-experiment-infra-stubborn-loops-design.md §8
- 상태: 승인됨(2026-07-17)

## 가설
H1: 완고 S/R 루프는 행동 공간 차단(암②)으로 종결된다.
H2: 루프의 원인은 저온(0.1) 복사 어트랙터이며 디코딩 섭동(암③)으로 회복된다.

## 조건 (암)
| 암 | 브랜치 (커밋) | 내용 |
|---|---|---|
| ① 기준선 | m10/base (e843c05) | --filter + 인프라만, 에이전트 행동 불변 |
| ② 강제 전환 | m10/arm-block (9fa5f72) | S/R 3누적 → edit_file 차단·write_file 강제 |
| ③ 디코딩 섭동 | m10/arm-perturb (3f97129) | S/R 2연속 → temperature 0.7 일시 상향 |

## 표본
ornith-1.0-9b@8K(context_tokens 8192, 로드 12288), 표적 2과제
(fix-monthly-total·update-vat-rate) × 10반복(--repeats 10 --seed 0) × 3암 = 60런.
배치별 `./.loco/config.toml`(러너가 배치 전 기록·배치 후 effective_config 대조):
8K = context_tokens 8192·max_output_tokens 4096·command_timeout_secs 240,
32K = context_tokens 32768·나머지 동일, 스포트 = context_tokens 8192·
max_output_tokens 4096·command_timeout_secs 60 (v2 조건).
gemma 제외 — M9 데이터에서 S/R 4회 전부 개입 임계(2연속·3누적) 미도달, 정보 0.
승자 확정 후: 승자 암 @32K(32768/49152) 2과제 × 10반복 + tasks/ 스포트 36런
(v2 조건: command_timeout_secs 60, 로드 8192).

## 지표 (exp_metrics.py 열)
주: sr발 반복정지 수(stop_cause=sr), 완고 루프(파일별 S/R 3회+) 발생 런의 종결
전환율, 오류당 2시도 내 회복률(sr_recovered/sr_recovery_denom).
보조: 엄격 통과율(passed ∧ outcome=finished)·거짓 finish·평균 턴/시간·salvage.

## 판정 규칙 (데이터 보기 전 확정)
주 지표 우세 암을 main에 병합. 동률이면 단순한 쪽(암②). 두 암 모두 기준선보다
나쁘면 병합 없이 실패 턴 제거안을 M11 입력으로. 발생 런이 배치당 3런 미만인
지표는 전수 나열 + 방향 판정(소표본 규칙). 스포트 게이트: ≥33/36.

## 중단 규칙
LLM 에러·부분 리포트 → 해당 배치 1회 재수행, 재실패면 실험 중단·원인 조사.
Ctrl+C 부분 리포트는 폐기 후 해당 배치 재수행.

## 시간 예산 (상한치)
8K 3암 ≤ 3.0h(실측 2.2~2.4h + 개입 암 완주 전환 여유), 32K 승자 ≤ 1.5h,
스포트 ≤ 1.0h — 총 ≤ 5.5h + 모델 교체·게이트. 배치가 상한 1.5배를 넘으면 중단.
