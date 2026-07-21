# 다음 세션 인수인계 (2026-07-21 종료)

## 한 줄

M16 treatment 스모크 + 하네스 패치 루프 **여기서 끊음**. `fd-1873` n=1 연속 실패는 여전하고, 남은 병목은 **notes thrash / 검증 미도달 / (서버) repeat-penalty 0** 쪽. **같은 줄기 소형 패치 반복 금지.**

## 읽기 순서

1. [`README.md`](./README.md) — 전체 그림·스모크 표·다음 방향 A–E  
2. [`pre-registration.md`](./pre-registration.md) §0-B — 개정 B 범위  
3. [`metrics/stamps-smoke.txt`](./metrics/stamps-smoke.txt)  
4. [`metrics/forensic-verify-nav.md`](./metrics/forensic-verify-nav.md)  
5. 로컬에 있으면 `.loco/eval/20260721T162606Z/` (stutter-cap 최종 스모크)

## main tip (이 문서 작성 시점)

`84ed9a9` — stutter salvage 후 max_tokens 캡. 그 앞 path/length/B1B2 커밋은 README 표.

## 권장 첫 질문 (다음 세션)

- “notes thrash를 줄일 제품 변경 vs serve repeat-penalty 파일럿 vs notes off 대조 1런” 중 **하나만** 고른다.
- 고른 뒤에만 스모크 또는 재사전등록.

## 하지 말 것

- `fd-1873` seed 0에 또 한 줄 SYSTEM / salvage 패치 얹고 스모크만 반복.
