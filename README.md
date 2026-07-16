# loco — 폐쇄망 소형모델 코딩 CLI

로컬에서 서빙되는 소형 LLM(OpenAI 호환 API)으로 코딩을 지원하는 CLI.
설계 문서: `docs/superpowers/specs/2026-07-02-loco-design.md`

## 프로젝트 상태: M7 완료 · 모델 세트 재편 (2026-07-16)

M7은 측정 체계 마무리다 — 모델 세트 재편(qwen3-vl-4b 은퇴, Ornith 9B를 Qwen3 계열
대표 기준선으로 승격), 평균 s/런의 리포트 1급 지표화, 판정 무결성 보강(cargo config
스냅샷 감지), 테스트·문서 부채 정리. 판정기·에이전트 코드는 불변이라 v2 수치는 그대로
비교 가능하다. 상세는 `docs/baselines.md` "모델 세트 재편 (M7)" 절.

### v2 기준선 요지 (상세 `docs/baselines.md`)

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

과제 세트를 돌려 모델의 코딩 성공률을 측정한다:

```
cargo run -- eval tasks/                          # 과제당 1회
cargo run -- eval tasks/ --repeats 3 --seed 0      # 과제당 3회 반복 (시드 0/1/2)
cargo run -- eval tasks/ --timeout-scale 2.0       # 느린 머신 — 모든 타임아웃 ×2
cargo run -- eval tasks/ --verify                  # 판정기 게이트 (LLM 없이) — 아래 참고
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
