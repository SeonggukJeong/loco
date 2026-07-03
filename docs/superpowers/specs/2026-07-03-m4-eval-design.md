# M4 — 평가 하네스(`loco eval`) 설계

마스터 스펙(`2026-07-02-loco-design.md`) §8을 구현 수준으로 구체화한 문서.
§8과 충돌하는 내용은 없으며, §8이 정하지 않은 결정만 추가한다.

## 범위

- **선행 수정 ①**: `-p` 모드 취소 배선 — `run_oneshot`에 Ctrl+C 처리가 없어
  실행 중 `run_command` 자식 프로세스가 고아가 되는 문제. eval의 과제 타임아웃
  킬이 정확히 같은 메커니즘을 요구하므로 M4 첫 항목으로 수정
- **eval 본체**: `loco eval` 서브커맨드 + 과제 세트(12개 내외) + 하네스 자체 테스트
- **기준선 측정**: LM Studio에서 Gemma/Qwen 4B급으로 `--repeats 3` 실행 (사용자 협조)
- 백로그 ②(chat_packed 통합)·③(length 루프 완화)·minor 정리는 **M5로 이연** —
  측정 수단 없이 고치면 개선 여부를 알 수 없으므로, M4 결과를 보고 우선순위 결정

## 결정 사항 (2026-07-03 브레인스토밍)

| 결정 | 선택 | 근거 |
|---|---|---|
| 실행 모델 | **인프로세스** — eval이 `Agent::run`을 라이브러리로 직접 호출 | 턴 수·outcome 직접 수집, 과제마다 `/models` 불필요, 하네스 자체 테스트를 스크립트된 가짜 `LlmClient`로 서버 없이 가능(§11) |
| 과제 픽스처 | **무의존 cargo 크레이트 중심**, check = `cargo test` | 폐쇄망(오프라인 check)·크로스플랫폼·protected로 테스트 보호. 코드찾기 과제는 "답을 answer.txt에 써라"로 변형해 판정 경로 통일 |
| 느린 머신 대응 | **`--timeout-scale <배수>`**(기본 1.0) — 모든 타임아웃에 곱함 | 머신 속도는 과제 속성이 아니라 실행 환경 속성. 배수는 과제 간 상대 난이도를 보존하고 report.json에 기록돼 측정 조건이 남는다 |
| 턴 수 집계 | `on_event`의 `Thought` 이벤트 카운트 | `session.messages()` 카운트는 패킹 절삭 시 과소집계 |
| check 실행 조건 | **outcome과 무관하게 항상 실행** | MaxTurns로 끝났어도 작업이 됐으면 통과가 공정. outcome 종류는 리포트에 별도 기록 |
| LLM 에러 | 재시도 소진 시 **하네스 전체 중단, exit 1** | 서버가 죽었는데 남은 과제를 도는 건 무의미 (§8 하네스 에러) |
| 새 설정 키 | 없음 — CLI 플래그만 | config 스키마 안정 유지 (`deny_unknown_fields`) |
| 임시 디렉터리 | `std::env::temp_dir()` + 고유 이름 수제 생성 | `tempfile`은 dev-dependency — 의존성 목록은 스펙이 고정하므로 본체로 승격하지 않는다 |

## 1. 선행 수정 ① — `-p` 취소 배선

- `run_oneshot`: `tokio::select!`로 `signal::ctrl_c()`와 `agent.run()` 경쟁
- Ctrl+C 수신 시: `ToolCtx.cancel` 플래그 세트 → **run 퓨처를 계속 await**
  (즉시 프로세스 종료하면 process group의 자식이 고아가 됨 — 이 버그가 ①).
  `run_command`가 cancel을 감지해 프로세스 그룹을 죽이면 run이 곧 반환된다
- LLM 호출 중이면 cancel 플래그로는 안 깨어나므로 **유예 5초** 후 강제 반환
  (run 퓨처 드롭 — reqwest 요청은 드롭 시 취소됨)
- 종료: stderr에 "(중단됨)" 메시지, 종료 코드 2 (조기 종료 계열, §7)
- eval의 과제 타임아웃도 같은 메커니즘 재사용: 타임아웃 발화 → cancel 세트 →
  유예 await → 실행을 `timeout`으로 기록
- eval의 Ctrl+C는 **장수 SIGINT 리스너 + 공유 플래그**로 감시한다 — tokio의
  `ctrl_c()`는 첫 폴링에서 프로세스 기본 SIGINT 동작을 영구 대체하고 등록 이후의
  신호만 보므로, select! 창 밖 구간(샌드박스 복사·protected 동기화·check 실행)의
  Ctrl+C가 유실되지 않으려면 리스너가 계속 살아 있어야 한다. 공유 플래그는 check
  실행의 cancel로도 전달돼 check 중 Ctrl+C가 프로세스 그룹을 죽인다; check가
  중단으로 잘리면 그 실행은 기록하지 않는다(잘린 판정은 측정 오염)

## 2. CLI

```
loco eval <tasks-dir> [--repeats N] [--seed N] [--timeout-scale F]
```

- clap 서브커맨드 — 기존 `loco [-p] [--auto]`와 공존(선택적 서브커맨드).
  `eval`은 `--auto` 의미를 내포하므로 별도 플래그 불필요
- `--repeats N`: 기본 1, 권장 3~5 (§8)
- `--seed N`: base seed, 기본 0. 실행 시드 = `base_seed + repeat_index` (§8 —
  반복마다 다른 시드)
- `--timeout-scale F`: 기본 1.0. 과제 `timeout_secs`·check 타임아웃에 곱함
- 종료 코드: 하네스 정상 완료 0(통과율 무관), 하네스 에러(과제 정의 오류, 서버
  접속 불가, fixture 복사 실패 등) 1

## 3. 모듈 구조 (`src/eval/`)

- `task.rs` — `task.toml` 로드·검증. 스키마(`deny_unknown_fields`):
  - `prompt: String` (필수) — 에이전트에게 줄 요청
  - `check: String` (필수) — 샌드박스 루트에서 실행할 판정 명령 (예: `cargo test`)
  - `timeout_secs: u64` (기본 300) — 에이전트 실행 전체(LLM 호출 포함) 상한.
    §8 경고: 파싱 재시도 때문에 최악 LLM 호출 = max_turns×3회 — 넉넉하게
  - `check_timeout_secs: u64` (기본 120) — check 명령 상한 (콜드 빌드 감안)
  - `max_turns: usize` (선택) — 설정보다 우선하는 과제별 상한
  - `protected: Vec<String>` (필수) — 판정 자산 경로(파일/디렉터리). fixture와
    정확히 일치하도록 동기화되는 대상. 과제 작성 가이드: `tests/`와 `Cargo.toml`
    반드시 포함 (§8 보상 해킹 차단)
- `sandbox.rs` — fixture → 임시 샌드박스 재귀 복사(심링크 없음 가정, 발견 시
  하네스 에러), 실행 후 **protected 동기화**: fixture 쪽 원본으로 덮어쓰기 +
  샌드박스 쪽에만 있는 protected 하위 파일 삭제. 종료 시 샌드박스 제거
- `report.rs` — 집계·출력. 과제별 통과율/평균 턴/평균 소요시간을 stdout에
  한국어 표로, 전체 데이터를 JSON으로 저장
- `mod.rs` — 오케스트레이터: 과제 로드(과제 정의 오류는 실행 시작 전 일괄 검증)
  → 과제×repeats 순차 루프 → 수집·리포트

## 4. 실행 흐름 (과제 1회분)

1. fixture를 임시 샌드박스로 복사
2. `ToolCtx::new(샌드박스 루트)` + `Registry::guided()` + `AutoApprover`
   (config의 `auto_deny_patterns` 적용) + 과제별 `max_turns` 오버라이드 +
   시드 주입으로 `Agent::run`
   - `ChatRequest`에 `seed: Option<u64>` 추가 (None이면 직렬화 생략 —
     기존 경로 무영향). Agent에 시드 세터 추가
   - 트랜스크립트는 리포트 디렉터리에 `run-<과제>-<반복>.jsonl`로 기록
     (실패 디버깅·재현용)
3. 타임아웃(`timeout_secs × scale`) 감시 — 발화 시 §1 메커니즘으로 킬,
   outcome을 `timeout`으로 기록
4. protected 동기화 (§8 — check보다 먼저)
5. `check` 실행: `run_command`의 실행 기반(sh -c/cmd /C, 프로세스 그룹 킬,
   CP949 폴백, 출력 절삭)을 공용 헬퍼로 추출해 재사용. 종료 코드 0 = 통과
6. 기록: 통과 여부, outcome 종류(`finished|max_turns|repetition_stop|`
   `parse_failed|timeout`), 턴 수, 소요시간, 시드

## 5. 리포트

- 위치: `./.loco/eval/<ISO8601 타임스탬프>/` (`.loco/.gitignore`로 커밋 제외)
  - `report.json` — 아래 스키마
  - `run-<과제>-<반복>.jsonl` — 실행별 세션 트랜스크립트
- `report.json` 스키마(개요):
  ```json
  {
    "model": "...", "base_seed": 0, "repeats": 3, "timeout_scale": 1.0,
    "started_at": "...", "duration_secs": 0,
    "tasks": [{
      "name": "add-function", "pass_rate": 0.67,
      "avg_turns": 5.3, "avg_duration_secs": 41.2,
      "runs": [{"repeat": 0, "seed": 0, "passed": true,
                "outcome": "finished", "turns": 5, "duration_secs": 38.5}]
    }],
    "total_pass_rate": 0.58
  }
  ```
- stdout 표(한국어): 과제명 | 통과율 | 평균 턴 | 평균 시간, 마지막 줄에 전체
  통과율과 report.json 경로

## 6. 과제 세트 (`tasks/`, 리포지토리 포함)

무의존 cargo 크레이트, check = `cargo test`(코드찾기류는 answer.txt를 읽어
검증하는 테스트 포함). 초안 12개 — 난이도 사다리, 플랜에서 확정:

1. `find-definition` — 함수가 정의된 파일 경로를 answer.txt에 (읽기·grep)
2. `count-usages` — 특정 식별자 사용 횟수를 answer.txt에 (grep)
3. `add-function` — 시그니처·테스트가 주어진 함수 구현 (write 초급)
4. `fix-off-by-one` — 명백한 버그 수정 (edit 초급)
5. `fix-failing-test` — `cargo test` 돌려 실패 원인 찾고 수정 (run_command+edit)
6. `multiline-string-edit` — 여러 줄·따옴표 많은 리터럴 편집 (§4 이스케이프
   리스크 측정 — 마스터 스펙이 의무화)
7. `edit-crlf-file` — CRLF 파일 편집 후 EOL 보존 검증
8. `create-module` — 새 파일 생성 + `mod` 등록 (write_file 복합)
9. `rename-function` — 정의·호출부 여러 곳 일괄 변경 (edit 다중)
10. `implement-from-doc` — doc 주석 명세만 보고 함수 구현 (중급)
11. `fix-compile-error` — 컴파일 에러 읽고 수정 (run_command 활용)
12. `chain-edits` — 한 파일에 연쇄 편집 3곳 (edit 지구력)

모든 과제: `protected = ["tests", "Cargo.toml"]` 이상. answer.txt류 과제의
기대값은 테스트 안에 있으므로 자동 보호된다

## 7. 테스트 (§11)

- `task.rs`: 스키마 파싱, 미지 키 거부, 필수 키 누락 에러
- `sandbox.rs`: 복사 충실성, protected 동기화 — 수정 복원·**추가 파일 삭제**·
  중첩 디렉터리, 심링크 발견 시 에러
- `report.rs`: 집계 산술(통과율·평균), JSON 직렬화 스키마
- 시드 파생: base_seed + repeat_index
- 통합: 스크립트된 가짜 `LlmClient`로 미니 fixture 과제의 eval 전체 흐름
  (통과·실패·타임아웃·protected 복원) — 실서버 불필요. check가 sh 명령이라
  기존 관례대로 `#[cfg(unix)]` 게이트 (윈도우 러너 검증은 라이브 스모크에 위임)
- 선행 수정 ①: run_oneshot 취소 경로는 기존 agent 테스트 패턴(스크립트된
  클라이언트)으로 커버 가능한 범위까지; 실 Ctrl+C는 라이브 스모크로
- 실모델 검증: 기준선 측정이 겸함 (§11)

## 8. 완료 기준

1. `cargo test` 전체 통과, `cargo clippy --all-targets -- -D warnings` 클린
2. 서버 없이 eval 자체 테스트 통과 (가짜 클라이언트)
3. 서버-다운 스모크: `loco eval`이 접속 불가 시 exit 1 + 실행 가능한 메시지
4. 라이브 기준선: gemma 4B급·qwen 4B급 각각 `loco eval tasks/ --repeats 3`
   완주, report.json 확보 — 결과는 문서/메모리에 기록 (M5 개선 판정의 기준점)
5. CLAUDE.md·사용법 문서 갱신
