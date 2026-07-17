# loco — 폐쇄망 소형모델 코딩 CLI

로컬에서 서빙되는 소형 LLM(OpenAI 호환 API)으로 코딩을 지원하는 CLI.
설계 문서: `docs/superpowers/specs/2026-07-02-loco-design.md`

## 프로젝트 상태: M9 완료 · 실행·종료 스캐폴딩 (2026-07-17)

M9는 M8 실패 데이터가 지목한 두 루프를 스캐폴딩으로 겨냥했다 — edit_file S/R
자기-버그(처방 오류문 + 전용 2연속 교정)와 finish 종료 실패(인자누락 2연속 교정 +
검증완료 후 FINISH_NUDGE). 소형 세트 교정 실효는 확실(SR_CORRECTION 발동 5런
전부 즉시 회복, 스포트 34/36 회귀 없음)하나 대형 저장소의 완고한 S/R 루프 1건과
미완성 탐색 루프(당초 "재검증 루프"로 오특성화 — M10 리뷰에서 정정, 뮤테이션
0회의 `cat` 반복이라 finish 유도 대상이 아님)는 잔존해 스펙 §2 행동 지표는
부분 미충족(S/R발 반복정지 1건·오류당 회복률 하락) — 완고 루프의 원인 분석 vs
기계적 개입이 M10 스코핑의 입력이 됐다. 판정 상세는 `docs/baselines.md` M9 절.

### M9 요지 (tasks-large 3과제 × 3반복, 리워드 픽스처 — M8 표와 직접 비교 불가)

| 조건 | 1단 (스캐폴딩 전) | 2단 (스캐폴딩 후) |
|---|---|---|
| gemma-4-e4b @8K | 6/9 · 엄격 4/9 · 거짓 1 | 6/9 · 엄격 5/9 · 거짓 0 |
| ornith-1.0-9b @8K | 5/9 · 엄격 5/9 · 거짓 1 | 5/9 · 엄격 4/9 · 거짓 1 |
| ornith-1.0-9b @32K | 6/9 · 엄격 5/9 · 거짓 0 | 7/9 · 엄격 5/9 · 거짓 0 |

1단(재베이스라인) 자체가 발견이다: 픽스처 리워드(누출 제거)만으로 M8 대비 gemma
관대 +22pp, ornith@32K 관대 −22pp — M8의 32K 88.9%는 누출이 부풀린 수치였다.

## M8 · 대형 저장소 트랙 (2026-07-17)

M8은 실사용 조건 측정이다 — 수만 라인급 사내 코드베이스 항해가 실제 병목이라는
북극성(폐쇄망 동료 배포)에 맞춰, 5크레이트 ~11.6K LOC 재고/물류 워크스페이스 픽스처
(`tasks-large/`, 검색 오염 함정 11종 내장)와 과제 3개를 신설하고 gemma·ornith 8K
베이스라인 + ornith 32K 민감도를 측정했다. 하네스 코드 변경 0. 결과: 소형 세트에서
94.4%였던 ornith이 대형 저장소 8K에서 55.6%로 떨어지고, 32K가 관대 통과를 88.9%로
구제하지만 엄격(종료 규율)은 44.4%로 불변 — 컨텍스트와 종료 규율이 별개 병목임을
확인했다. 27런 실패 분류가 M9 우선순위를 확정(`docs/research/2026-07-17-m8-failure-analysis.md`):
repo-map은 강등(모델들이 트리를 안 보고 grep 직행), 최우선은 edit_file 자기-버그 완화.

### M8 대형 저장소 트랙 요지 (상세 `docs/baselines.md` M8 절)

| 조건 | 통과 | 엄격(Finished∧통과) | 평균 s/런 |
|---|---|---|---|
| gemma-4-e4b @8K | 44.4% (4/9) | 44.4% | 80.5s |
| ornith-1.0-9b @8K | 55.6% (5/9) | 44.4% | 156.8s |
| ornith-1.0-9b @32K | 88.9% (8/9) | 44.4% | 217.8s |

과제 3 × 3반복 = 9런, seed 0. ornith 실측 사양표(프리필 ~334 tok/s·KV 32 KiB/토큰 —
하이브리드 아치라 RAM-only 폐쇄망에서 32K 운용 총 ~7 GiB)도 같은 절에 있다.

### v2 기준선 요지 — 소형 세트 `tasks/` (상세 `docs/baselines.md`)

| 모델 | 통과 | 엄격(Finished∧통과) | 거짓 성공 finish | 평균 s/런 | 세트 지위 |
|---|---|---|---|---|---|
| google/gemma-4-e4b | 72.2% (26/36) | 69.4% (25/36) | 4 | 52.3s | 기준선 (4B 대표) |
| qwen/qwen3-vl-4b | 50.0% (18/36) | 33.3% (12/36) | 3 | 59.7s | 은퇴 (M7) |
| ornith-1.0-9b | 94.4% (34/36) | 94.4% (34/36) | 0 | 67.3s | 기준선 (M7 승격) |

측정 조건: 모델 단독 로드 ctx 8192, `max_output_tokens = 4096`, seed 0, `--repeats 3`
(과제 12 × 3반복 = 36런), 하네스 커밋 `4cb7325`. 모델 세트 재편 경위는 docs/baselines.md 참고.

## 시작하기

1. LM Studio(또는 Ollama, llama.cpp server 등)에서 모델을 로드하고 서버 시작
   - LM Studio 기본 주소 `http://localhost:1234/v1` 는 설정 없이 바로 동작
2. 실행:

   ```
   cargo run                 # 대화형 에이전트 REPL
   cargo run -- -p "질문"    # 단발 실행 (답변만 stdout, 종료코드 0/1/2)
   cargo run -- --auto       # 확인 게이트를 자동 승인 (REPL/-p 모두 사용 가능)
   ```

## 사용법

REPL에 입력한 내용은 에이전트가 처리한다 — 모델이 read_file/list_files/grep로
프로젝트를 조사하고 write_file/edit_file/run_command로 직접 수정·실행까지 한다.
답은 마지막에 한 번에 출력된다 (`finish`). 진행 중 Ctrl+C 로 취소할 수 있다.

파일 수정이나 명령 실행처럼 상태를 바꾸는 동작은 실행 전에 미리보기(diff 또는
명령어)를 보여주고 `적용할까요? [y/N]` 확인을 거친다. `--auto`를 주면 이 확인을
전부 건너뛰고 자동 승인하되, `rm -rf`·`sudo`·`git push` 등 위험한 명령 패턴은
자동 모드에서만 별도로 차단된다(대화형에서는 경고만 표시하고 계속 진행).
`-p` 단발 실행은 `--auto` 없이는 확인을 띄우지 않고 그냥 거부한다(비대화형이므로).

대화 내용은 실행마다 `.loco/sessions/*.jsonl`에 한 줄씩 기록된다(최선 노력 —
기록 실패가 에이전트를 멈추지 않는다). 히스토리가 컨텍스트 예산을 넘으면
자동으로 절삭한다(오래된 툴 결과 생략 → 오래된 질문/답 쌍 제거 순서) — 더는
`/clear`로 직접 비울 필요가 없다.

- `/chat <메시지>` — 에이전트 없이 모델과 바로 스트리밍 대화 (빠른 질문용)
- `/clear` — 히스토리 초기화(수동으로 비우고 싶을 때만; 컨텍스트 관리 목적으로는
  불필요 — 위 자동 절삭 참고)
- `/config`, `/help`, `/quit`

`-p` 모드 종료 코드: `0` 정상(finish), `1` 에러(연결 실패·파싱 실패),
`2` 최대 턴 도달·같은 툴 호출 반복으로 조기 종료, 또는 Ctrl+C로 중단(실행 중이던
명령의 자식 프로세스까지 정리한 뒤 종료). 진행 표시는 stderr로 가므로 stdout만
파이프하면 답변만 남는다.

## 평가 하네스 (eval)

과제 세트를 돌려 모델의 코딩 성공률을 측정한다. 세트는 둘이다 — `tasks/`(소형
크레이트 12과제)와 `tasks-large/`(M8 신설, 5크레이트 워크스페이스 3과제 — 함정
대장·드리프트 절차는 `tasks-large/README.md`):

```
cargo run -- eval tasks/                          # 과제당 1회
cargo run -- eval tasks/ --repeats 3 --seed 0      # 과제당 3회 반복 (시드 0/1/2)
cargo run -- eval tasks/ --timeout-scale 2.0       # 느린 머신 — 모든 타임아웃 ×2
cargo run -- eval tasks/ --verify                  # 판정기 게이트 (LLM 없이) — 아래 참고
cargo run -- eval tasks-large/ --repeats 3         # 대형 저장소 트랙도 동일 사용법
```

`--verify`는 모델 없이 판정기 자체를 검증하는 메타테스트다(M6): 과제마다 원본 픽스처에서
`check`가 **실패**(변별성)하고 `solution/`을 덮은 뒤 **통과**(해결가능성)하는지 확인해, 둘 다
만족할 때만 종료 코드 0. `tasks/`를 손댈 때마다 돌린다. `--repeats`/`--seed`와는 배타.

과제마다: 픽스처를 임시 샌드박스에 복사 → `--auto`와 같은 권한으로 에이전트 실행
→ `protected`로 지정한 경로를 픽스처 원본으로 되돌리고(에이전트가 건드렸다면
그 흔적까지 지움 — 보상 해킹 방지) → `check` 명령의 종료 코드로 통과/실패를
가른다. `check`는 에이전트 실행이 중간에 실패했어도 항상 돈다.

결과는 통과율 표로 stdout에 출력되고, `./.loco/eval/<타임스탬프>/report.json`
(전체 리포트)과 과제×반복별 실행 기록 `run-<과제>-<반복>.jsonl`로도 저장된다.

종료 코드: `0` 정상 완료(통과율과 무관), `1` 하네스 에러(서버 다운, 잘못된 과제
정의) 또는 Ctrl+C로 중단되어 부분 리포트만 남긴 경우.

## 빌드 노트

TLS는 rustls+ring 고정 — OpenSSL도 aws-lc-sys(cmake/NASM)도 그래프에 없어
Windows 폐쇄망에서 `cargo vendor` 후 Rust 툴체인만으로 빌드된다.

## 설정 (선택)

`./.loco/config.toml` (프로젝트) 또는 전역 설정 파일. 전역 경로는 OS마다 다르며
(macOS는 `~/Library/Application Support/dev.loco.loco/config.toml`, Linux는
`~/.config/loco/config.toml`) REPL의 `/config` 명령으로 확인할 수 있다:

```toml
base_url = "http://localhost:1234/v1"
model = ""            # 비우면 서버의 첫 모델 자동 선택
temperature = 0.1
context_tokens = 8192
max_output_tokens = 2048
max_turns = 25
command_timeout_secs = 60
```

## 현재 상태

- [x] M1: 채팅 REPL (스트리밍)
- [x] M2: 읽기 도구 에이전트
- [x] M3: 가이드형 코딩 에이전트 (쓰기 툴 + 확인 게이트 + 세션 기록)
- [x] M4: 평가 하네스 (`loco eval` — 과제 세트 통과율 측정)
- [x] M5: 스캐폴딩 개선 (salvage 파싱, 반복 감지, edit_file 3단 매칭, 검증 넛지 등) —
      `eval tasks --repeats 3`(seed 0, ctx 8192) 기준 gemma-4-e4b 11.1%→66.7%,
      qwen3-vl-4b 33.3%→50.0%. 배치별 경과와 최종 분석은 `docs/baselines.md` 참고
- [x] M6: 판정·평가 신뢰성 개편 — 메타테스트 `eval --verify`(변별성+해결가능성 게이트,
      과제별 `solution/` 오버레이), answer 판정 정규화 사다리(협소 판정기 해소), 이중
      리포트(통과/엄격/거짓 성공 finish — 관대 채점·미계측 문제 해소), 에이전트 코드 동결
      하 v2 기준선 재측정. v1과는 판정기가 달라 직접 비교 불가 — 위 "프로젝트 상태"와
      `docs/baselines.md` 참고
- [x] M7: 모델 세트 재편·속도 지표·판정 무결성 보강 — qwen3-vl-4b 은퇴, Ornith 9B 기준선
      승격(재측정 없음), report 최상위 평균 s/런, cargo config 스냅샷 감지
      (`eval/integrity.rs`), 테스트·문서 부채 정리. 판정기·에이전트 코드 불변
- [x] M8: 대형 저장소 트랙 — `tasks-large/`(~11.6K LOC 워크스페이스 픽스처 + 과제 3,
      함정 11종, `--verify` 3/3) 신설, 8K 베이스라인·32K 민감도·ornith 실측 사양표 측정,
      레퍼런스 노트 3건(aider repo-map·codex-rs·grok-build), 27런 실패 분류로 M9
      우선순위 확정. 하네스 코드 변경 0 — 위 "프로젝트 상태"와 `docs/baselines.md` 참고
- [x] M9: 실행·종료 스캐폴딩 — edit_file S/R 처방 오류문 + 전용 2연속 교정(SR_CORRECTION),
      finish 인자누락 2연속 교정(FINISH_ARGS_CORRECTION) + 검증완료 후 반복 재확인 감지
      상태기계(`agent/finish_nudge.rs`), 2단 측정(리워드 재베이스라인 → 스캐폴딩 후 4배치)
      과 행동 지표 판정 — 위 "프로젝트 상태"와 `docs/baselines.md` M9 절 참고
