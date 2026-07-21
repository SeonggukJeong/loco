# M16 레포 온보딩 (계층 notes) — 실험 자리

| | |
|---|---|
| **한 줄** | `repo_notes` on/off 재측정으로 cold-start `tasks-real` 통과율을 들어 올릴 수 있는지 본다 |
| **성격** | 효과 실험 (control vs treatment) · **사전등록 필수** · M15 0/51 스탬프를 control로 **재사용 금지** |
| **상태** | **코드 main 머지** · **사전등록 승인됨 (2026-07-21)** · **GPU 배치 진행 가능** · 숫자/판정은 측정 후 |

---

## 스펙 근거

- 설계: [`docs/superpowers/specs/2026-07-21-m16-repo-onboarding-design.md`](../../superpowers/specs/2026-07-21-m16-repo-onboarding-design.md) — **§5 측정 프로토콜** (암·지표·mechanism-alive·회귀 `repo_notes=false`)
- 플랜: `docs/superpowers/plans/2026-07-21-m16-repo-onboarding.md`
- 프로토콜: [`docs/experiments/PROTOCOL.md`](../PROTOCOL.md) · 템플릿: [`docs/experiments/TEMPLATE.md`](../TEMPLATE.md)
- 배경 바닥: M15 실레포 베이스라인 **0/51 DQ** (`docs/experiments/2026-07-20-m15-real-repo-baseline/`) — 인용 가능한 control **아님**

---

## Flag 정책 (요약)

| 표면 | `repo_notes` |
|---|---|
| 제품 REPL / config default | **true** |
| `eval` · `tasks` / `tasks-large` / 기타 non-`tasks-real` | 하네스가 **false 강제** |
| `eval` · `tasks-real` 실험 암 | 러너가 암별 config로 **true/false 고정** · `EffectiveConfig` 스냅샷 |

툴 이름: **`update_repo_notes`**. flag off → 툴 미등록 · SYSTEM 포인터 없음 · mut-gate/stale off (silent no-op 없음).

---

## TODO (GPU 전)

1. [x] `pre-registration.md` 초안 — §5 / §2-2 / mechanism-alive / DQ / 102런
2. [x] **사용자 승인 커밋** (상태 `승인됨`, 2026-07-21)
3. [ ] control (`repo_notes=false`) → treatment (`true`) 재측정 (각 17×3)
4. [ ] `report.md` · `docs/baselines.md` M16 절 — **측정 후에만** 숫자 기록

이 디렉터리에 통과율·스탬프·가짜 결과를 넣지 말 것.
