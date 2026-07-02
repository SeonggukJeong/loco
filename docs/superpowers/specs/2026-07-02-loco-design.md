# loco — 폐쇄망 소형모델 코딩 CLI 설계

- 날짜: 2026-07-02
- 상태: 승인됨 (브레인스토밍 결과 + 아키텍처 리뷰 1차 반영)
- 이름 참고: `loco`(LOcal COder)는 가칭. Rust 웹 프레임워크 loco.rs와 이름이 겹치지만
  crates.io에 publish하지 않는 로컬 바이너리이므로 문제없음. 언제든 개명 가능.

## 1. 목표

폐쇄망(air-gapped) 환경에서 Tool Use 가능한 소형 LLM(~4B급)으로 코딩 작업을
지원하는 크로스플랫폼(Windows/Linux/macOS) CLI 에이전트.

- **v1 목표(가이드형)**: 파일 읽기/수정/검색/명령 실행을 툴로 수행하되, 변경이
  일어나는 단계마다 사용자 확인을 거치는 에이전트.
- **장기 목표(자율형)**: 평가 하네스로 측정하면서 스캐폴딩을 개선해, 4B급 모델의
  한계 내에서 자율 멀티스텝 작업 성공률을 끌어올린다. 플래너-이그제큐터 구조 등
  실험은 v1 완성 이후 평가 점수 기반으로 진행.

### 성공 기준

1. Windows/Linux/macOS 각각 단일 바이너리로 동작 (런타임 설치 불필요)
2. LM Studio(OpenAI 호환 API)에 연결해 가이드형 코딩 세션이 실제로 돌아감
3. `loco eval`이 과제 세트의 통과율 리포트를 산출함
4. 이후 모든 스캐폴딩 개선은 평가 통과율 변화로 검증 (반복 실행 평균 기준, §8)

### 비목표 (v1)

- 추론 엔진 내장 (llama.cpp/mistral.rs 임베딩) — OpenAI 호환 API로 충분, 후속 옵션
- 풀스크린 TUI (ratatui) — v1은 라인 기반 REPL
- 네이티브 function calling 지원 — 구조화 출력 방식으로 대체 (아래 §4)
- tree-sitter 기반 repo map — v1은 디렉터리 트리만
- 멀티 에이전트, 서브에이전트, MCP 연동
- 완전한 명령 실행 샌드박스 — v1은 cwd 고정 + 차단 패턴 수준 (§5), 컨테이너/저권한
  계정 사용은 운영 가이드로 안내

## 2. 확정된 기술 결정

| 항목 | 결정 | 이유 |
|---|---|---|
| 언어 | Rust (edition 2024) | 단일 정적 바이너리, serde 생태계, 사용자 학습 목표 포함 |
| 모델 연동 | OpenAI 호환 HTTP API만 | LM Studio/Ollama/vLLM/llama.cpp server 전부 호환 |
| TLS | rustls | 폐쇄망 Windows에서 OpenSSL 의존성 제거 |
| UI | rustyline 기반 REPL | v1 범위 최소화 |
| 오프라인 빌드 | `cargo vendor` 지원 | 폐쇄망 내부 빌드 대비 |
| 타깃 모델 | Gemma 4B급 (주), Qwen 4B급 (평가 비교군) | 사용자 환경 제약 |

주요 크레이트: `tokio`, `reqwest`(rustls), `serde`/`serde_json`, `clap`,
`rustyline`, `anyhow`/`thiserror`, `regex`, `ignore`, `directories`, `toml`,
`similar`(diff 미리보기), `encoding_rs`(명령 출력 디코딩), `wiremock`(dev-dep).

## 3. 아키텍처

단일 바이너리 크레이트(lib + thin bin), 모듈 경계는 다음과 같다.

```
src/
├── main.rs      — clap 파싱, 서브커맨드 디스패치 (배선만)
├── lib.rs       — 모듈 선언
├── config.rs    — 설정 로드/병합
├── llm/         — OpenAI 호환 클라이언트
│   ├── client.rs    — HTTP, 스트리밍(SSE), 재시도
│   └── types.rs     — 요청/응답 타입 (serde)
├── tools/       — 툴 정의 + 실행
│   ├── mod.rs       — Tool 트레이트, 레지스트리, 디스패치, 경로 확인
│   └── (툴별 파일)
├── agent/       — ReAct 루프, 컨텍스트 예산 관리, 반복 감지
├── session.rs   — 대화 상태, 히스토리 절삭, 트랜스크립트 저장
├── ui/          — REPL, 스트리밍 출력, 확인 프롬프트
└── eval/        — 평가 하네스
```

### 핵심 트레이트 경계

- `LlmClient` 트레이트: `chat(request) -> response` + 스트리밍 변형.
  agent 루프는 이 트레이트에만 의존 → 테스트에서 스크립트된 가짜 LLM 주입 가능.
- `Tool` 트레이트: 이름, JSON 인자 스키마, `run(args, ctx) -> ToolResult`,
  `is_mutating() -> bool` (확인 게이트 대상 여부).

### 데이터 흐름 (에이전트 한 사이클)

1. `session`이 시스템 프롬프트 + 히스토리 + 사용자 요청으로 메시지 조립
2. `agent`가 `LlmClient::chat` 호출 (JSON Schema 강제, 온도 0.1, **비스트리밍** —
   구조화 출력과 스트리밍 표시는 양립하지 않으므로 에이전트 턴은 스피너 표시.
   스트리밍은 일반 채팅(`/chat`, §7) 전용)
3. 응답 JSON 파싱 → `{thought, action}` — 실패 시 에러를 모델에 되먹여 **총 3회
   시도**(초기 1회 + 재시도 2회)
4. `thought`를 사용자에게 표시
5. `action.tool`이 mutating이면 미리보기(diff/명령어) 후 y/n 확인
6. 툴 실행 → 결과(성공/에러 모두)를 **`role: "user"` 메시지**로 히스토리에 추가.
   `<tool_result name="read_file">...</tool_result>` 형태의 구분자로 감싼다.
   (`role: "tool"`은 네이티브 함수호출의 `tool_call_id`를 요구하고 Gemma 챗템플릿에
   tool role이 없어 깨지므로 사용 금지. system/user/assistant만 쓰는 이 방식이
   모든 모델 템플릿에서 동작하는 공통분모다)
   툴 결과·교정 메시지 등 한 턴에 모델에게 줄 피드백이 여럿이면 **하나의 user
   메시지로 합친다** — 연속된 동일 role 메시지를 거부하는 템플릿이 있음.
   또한 순정 Gemma 템플릿은 system role 자체가 없으므로(백엔드 shim에 의존),
   서버가 system role을 거부하면 시스템 프롬프트를 첫 user 메시지 앞에 붙이는
   폴백을 둔다
7. `finish` 툴이 호출되거나 최대 턴 수(기본 25) 도달 시 사용자에게 제어 반환

### 반복(루프) 감지

4B 모델은 특히 에러 후에 동일한 툴 호출을 반복하는 경향이 있다.

- 동일한 `(tool, args)` 액션이 **3회 연속**되면 교정 메시지("같은 호출을 반복하고
  있다. 다른 접근을 시도하라")를 주입
- 교정 후에도 **2회 더 연속** 반복되면 조기 종료하고 사용자에게 제어 반환.
  다른 액션이 나오면 카운터는 리셋된다
- 알려진 사각지대(v1 수용): A/B 교대 반복, `finish_reason: length` 반복은 감지하지
  못한다 — `max_turns`가 상한 역할

## 4. 툴 프로토콜 — 4B 대응의 핵심 설계

**네이티브 function calling을 쓰지 않는다.** 소형 모델(특히 Gemma 계열)은 함수
호출 포맷 훈련이 약하다. 대신 매 턴 아래 형태의 JSON 하나를 출력하도록
`response_format: json_schema`(grammar-constrained decoding)로 강제한다.

```json
{"thought": "...", "action": {"tool": "read_file", "args": {"path": "src/main.rs"}}}
```

- **스키마는 의도적으로 얕게 유지한다**: `tool`은 문자열 enum, `args`는 자유
  오브젝트. 툴별 인자 검증은 앱 쪽에서 수행하고 위반 시 §9 재시도 경로로 되먹인다.
  (툴 7개짜리 깊은 `oneOf` 유니온은 백엔드에 따라 grammar 컴파일이 느리거나
  미지원일 수 있음)
- 서버가 json_schema를 지원하지 않으면 텍스트에서 JSON 추출 + 재시도 폴백
- 한 턴에 툴 호출 하나만 (병렬 호출 없음)
- 시스템 프롬프트는 영어(소형 모델의 지시 이행률이 영어에서 더 안정적),
  few-shot 예시 1~2개 포함. 사용자 대화 언어는 자유.
- **답변 채널은 `finish.summary`다.** 코드베이스 질문 응답, 사용자에게 전할 설명은
  전부 `finish`의 summary로 전달하며 시스템 프롬프트에 이를 명시한다.
  (모든 턴이 툴 호출로 강제되므로 별도의 자유 산문 채널이 없음)

### 알려진 리스크: JSON 문자열 안 코드 이스케이프

grammar 강제는 *유효한* JSON을 보장할 뿐, 소형 모델이 여러 줄 코드를 `content`/
`search`/`replace`에 넣을 때 `\n`, `\"`, 백슬래시를 정확히 이스케이프한다는 보장이
없다. 대응: (a) M4 평가 과제에 여러 줄·따옴표 많은 편집 과제를 반드시 포함,
(b) 실패율이 높으면 파일 내용만 펜스드 플레인 텍스트 블록으로 받는 대체 프로토콜로
전환한다 (사전 등록된 폴백 설계).

### 툴 목록 (7개 고정)

| 툴 | 인자 | 동작/제한 |
|---|---|---|
| `read_file` | path, offset?, limit? | 기본 최대 200줄, 초과 시 페이지 안내. **라인 번호를 붙이지 않는다** (모델이 search 블록에 복사하는 오염 방지). UTF-8이 아니거나 바이너리면 명확한 에러 반환 |
| `write_file` | path, content | 새 파일 생성 또는 전체 덮어쓰기. 기존 파일 덮어쓰기 시 기존 지배적 라인엔딩에 맞춰 변환, 새 파일은 `\n`. mutating |
| `edit_file` | path, search, replace | 매칭 사다리: 정확 일치 → 후행 공백 무시 → 균일 들여쓰기 시프트. 적용된 모드를 결과에 보고. 어느 단계든 매칭은 정확히 1회여야 하며, 실패 시 근접 부분을 에러로 반환. CRLF 보존. mutating |
| `list_files` | path?, depth? | ignore 크레이트(gitignore 존중), 항목 수 상한 |
| `grep` | pattern, path? | regex, 최대 50매치, 매치당 전후 2줄 |
| `run_command` | command | cwd는 프로젝트 루트 고정. 타임아웃 60초(설정 가능), stdout+stderr 합산 후 절삭. 출력은 UTF-8 우선, 실패 시 `encoding_rs` 손실 디코딩(한국어 Windows의 CP949 대응). mutating |
| `finish` | summary | 작업 종료 선언. summary가 사용자에게 전달되는 답변 채널 |

### 경로 확인 (path confinement)

모든 파일 툴(read/write/edit/list/grep)은 다음 규칙을 따른다:

- 인자 경로는 프로젝트 루트 기준 상대 경로로 해석: `root.join(path)` 후 정규화
- 정규화 결과가 루트를 벗어나면 거부 (`..` 탈출, 절대 경로, Windows 드라이브 문자,
  UNC `\\server\share` 모두 거부 대상)
- 루트 밖을 가리키는 심볼릭 링크는 따라가지 않고 에러 반환
- 모델이 Windows식 `\` 구분자를 내도 수용한다 (양방향 정규화: 표시할 때는 `/`,
  받을 때는 둘 다 허용)
- `run_command`의 cwd도 프로젝트 루트로 고정

### 파일 수정 형식

unified diff는 금지 (소형 모델이 라인 번호/컨텍스트를 못 맞춤). `edit_file`의
검색/치환 블록과 `write_file` 전체 재작성만 허용. 시스템 프롬프트에서 기존 파일은
`edit_file`을 우선하도록 지시한다 (전체 재작성은 출력 토큰 예산을 초과하기 쉬움).
edit 적용 전 `similar`로 diff를 렌더링해 확인 게이트에서 보여준다.

매칭 사다리의 정확한 규칙:

- **매칭 전 파일 내용과 search 블록 모두 라인엔딩을 `\n`으로 정규화**해 비교한다.
  쓰기 시에만 원본 라인엔딩을 복원 (CRLF 파일 + 모델의 `\n` search 블록이 영원히
  실패하는 문제 방지)
- 각 단계에서 **0회 매칭 → 다음 단계로 진행**, **2회 이상 매칭 → 즉시 모호성 에러
  반환** (더 느슨한 단계는 매칭이 늘어날 뿐이므로 fallthrough 하지 않는다)

## 5. 확인 게이트 (가이드형 모드)

- 읽기 툴: 자동 실행, `→ read_file src/main.rs` 한 줄 알림
- mutating 툴: 미리보기 표시 후 `[y]es / [n]o` 프롬프트.
  거부 시 거부 사실이 tool 결과로 모델에 전달됨 (모델이 다른 접근 시도 가능)
- `--auto` 플래그: 전부 자동 승인. 평가 하네스가 사용하며 향후 자율 모드의 기반

### `--auto` 모드 가드레일

`--auto`에서 `run_command`는 사용자 권한으로 임의 명령을 실행한다. 폐쇄망은 유출
위험을 줄일 뿐 파괴 위험을 줄이지 않는다. v1 대응:

- 설정 `auto_deny_patterns`(정규식 목록): 매치되는 명령은 실행 거부하고 거부 사유를
  모델에 반환. 기본 목록은 **크로스플랫폼**으로 구성 —
  Unix: `sudo`, `rm\s+-\w*[rf]`(플래그 순서/결합 변형 포괄), `mkfs`, `dd\s+if=`, `shutdown`;
  Windows: `rd\s+/s`, `del\s+/[fsq]`, `format\s`, `Remove-Item\s+.*-Recurse`, `reg\s+delete`;
  공통: `git\s+push`. 이 목록은 완전한 방어가 아닌 최선 노력(defense-in-depth)임
- cwd 고정(§4)으로 상대 경로 파괴 범위를 프로젝트로 제한
- 잔여 위험은 문서화: eval/자율 모드는 가능하면 저권한 계정·컨테이너·VM에서 실행
  권장. 완전한 샌드박스는 비목표(§1)

## 6. 컨텍스트 관리

- 토큰 추정: **`utf8_bytes / 4`** 휴리스틱. (`chars / 4`는 한국어에서 4~8배
  과소추정 — 한글 1자 = 3바이트 = 추정 0.75토큰으로 보수적으로 근사)
- 예산 공식: **`input_budget = (context_tokens − max_output_tokens) × 0.9`** —
  기본값 기준 (8192−2048)×0.9 ≈ 5529토큰까지만 입력을 패킹한다 (0.9는 추정 오차
  안전 마진). `context_tokens` 기본 8192, `max_output_tokens` 기본 2048
- **주의**: `context_tokens`는 서버에 실제 로드된 컨텍스트 길이와 일치해야 한다
  (LM Studio는 클라이언트 요청과 무관하게 로드 시 설정된 컨텍스트로 동작).
  서버의 컨텍스트 초과 에러를 감지하면 설정 안내를 포함한 메시지를 표시한다
- 시스템 프롬프트에 프로젝트 디렉터리 트리 주입 (상한 있음)
- 툴 결과는 툴별 상한으로 절삭 (예: read_file 최대 200줄 + 페이지 인자)
- 예산 초과 시 오래된 턴의 툴 결과부터 `[결과 생략]`으로 치환, 그래도 초과면
  가장 오래된 **user+assistant 턴 쌍을 원자적으로 제거** (시스템 프롬프트와 현재
  요청은 보존). 낱개 제거는 §3의 role 교대 규칙을 깨뜨린다 — 제거 후 교대가
  유지되는지 재검증하고 필요하면 인접 동일 role 메시지를 병합한다

## 7. CLI 인터페이스

```
loco                     # 대화형 REPL (기본)
loco -p "..."            # 단발 실행 (비대화형, CI/스크립트용)
loco --auto              # 확인 게이트 전부 자동 승인
loco eval <tasks-dir>    # 평가 하네스 실행
```

- `-p` 비대화형 모드에서 mutating 툴이 호출되면: `--auto`가 없으면 툴이 "비대화형
  모드에서는 --auto 없이 변경/실행 불가" 에러를 모델에 반환한다 (TTY 프롬프트를
  띄우지 않음)
- 종료 코드: `0` = finish 정상 종료, `1` = 에러(연결 실패, 파싱 3회 실패 등),
  `2` = max_turns/반복 감지로 조기 종료
- `-p` 출력 계약(스크립트 사용 전제): 최종 답변(`finish.summary` 또는 채팅 응답)만
  **stdout**으로, thought/툴 알림/진행 표시는 **stderr**로. stdout이 TTY가 아니면
  스피너를 끈다

REPL 슬래시 커맨드: `/help`, `/clear`(히스토리 초기화), `/config`(현재 설정 표시),
`/quit`. M2부터 REPL의 기본 입력은 에이전트 루프로 가며, M1의 스트리밍 일반 채팅
경로는 `/chat <메시지>` 커맨드로 유지한다 (기존 코드 재사용, 빠른 질문용).

### 설정

- 전역: `directories` 크레이트의 config_dir — OS마다 다름 (Linux
  `~/.config/loco/config.toml`, macOS `~/Library/Application Support/dev.loco.loco/config.toml`).
  `/config` 명령이 실제 경로를 표시한다
- 프로젝트: `./.loco/config.toml` (전역을 덮어씀)
- 항목: `base_url`(기본 `http://localhost:1234/v1` — LM Studio 기본 포트),
  `api_key`(선택), `model`, `temperature`(기본 0.1), `context_tokens`(기본 8192),
  `max_output_tokens`(기본 2048), `max_turns`(기본 25),
  `command_timeout_secs`(기본 60), `auto_deny_patterns`(기본 목록 내장 — 목록은
  M3에서 run_command와 함께 도입, 그 전까지는 빈 값)

### 세션 기록

- 위치: `./.loco/sessions/<ISO8601 타임스탬프>.jsonl`, `-p` 실행도 기록
- 레코드: 한 줄에 하나, `{ts, kind, content}` — kind는
  `user | assistant | tool_result | system`, tool_result에는 `tool`, `args` 필드 추가
- v1은 기록 전용(재개 기능 없음). `/clear`는 새 세션 파일을 연다
- loco가 `./.loco/.gitignore`(내용 `*`)를 자동 생성해 커밋 오염 방지

## 8. 평가 하네스 (`loco eval`)

과제 하나 = 디렉터리 하나:

```
tasks/add-function/
├── task.toml       # prompt, check 명령, timeout, max_turns, protected 경로 목록
└── fixture/        # 과제 시작 시점의 작업 디렉터리 내용
```

실행 흐름: fixture를 임시 샌드박스에 복사 → `--auto`로 에이전트 실행 →
**`protected`로 선언된 경로(테스트 파일 등 판정 자산)를 원본 fixture와 정확히
일치하도록 동기화** — 에이전트가 protected 디렉터리 아래 추가한 파일은 삭제한다
(단순 복사는 추가된 파일을 못 잡음) → `check` 명령(예: `cargo test`) 종료코드로
판정. (복원 단계가 없으면 모델이 테스트를 고쳐서 "통과"하는 보상 해킹을 막을 수
없음.) 과제 작성 가이드: check가 의존하는 매니페스트/빌드 스크립트(예: `Cargo.toml`)도
protected에 포함할 것

- **`--repeats N`(기본 1, 권장 3~5)**: 과제당 N회 반복 실행, 과제별 평균 통과율
  리포트. 온도 0.1이어도 로컬 서버는 비결정적이므로 1회 실행 비교는 노이즈다.
  서버가 지원하면 `seed`를 전달하되 **반복마다 다른 시드**(`base_seed + repeat_index`, base_seed는 `loco eval --seed N`으로 지정하며 기본 0)를
  쓴다 — 고정 시드는 N회 반복을 1표본 측정으로 만들어 통계를 조용히 무효화한다.
  사용된 시드는 JSON 리포트에 기록해 개별 실행을 재현 가능하게 한다
- 리포트: 과제별 통과율 + 턴 수 + 소요시간 (표 + JSON 저장)
- 과제 timeout 산정 주의: 파싱 재시도(턴당 최대 3회)는 max_turns에 계상되지 않아
  최악의 경우 태스크당 LLM 호출이 max_turns×3회다
- `loco eval` 자체 종료 코드: 하네스가 정상 완료하면 0(과제 통과율과 무관),
  하네스 에러(과제 정의 오류, 서버 접속 불가 등)면 1
- 초기 과제 세트 10~20개: 함수 추가, 명백한 버그 수정, 실패 테스트 통과시키기,
  grep 기반 코드 찾기 등 난이도 사다리 구성. **여러 줄·따옴표 많은 편집 과제 포함**
  (§4 이스케이프 리스크 측정). 과제 세트 자체도 리포지토리에 포함

## 9. 에러 처리

- 연결 실패/5xx: 지수 백오프로 **총 3회 시도**(초기 1회 + 재시도 2회), 최종 실패 시
  "LM Studio 서버가 떠있는지 확인하세요 (base_url: ...)" 수준의 실행 가능한 메시지.
  예외: 시작 시 모델 목록 조회(`GET /models`)는 재시도하지 않는다 — 시작 실패는
  빨리 드러나는 편이 낫다
- 모델 출력 파싱 실패: 에러를 모델에 되먹여 총 3회 시도(초기 1회 + 재시도 2회) →
  그래도 실패면 사용자에게 원문 표시 후 제어 반환
- **`finish_reason: "length"`(출력 잘림)는 파싱 실패와 구분**해서 처리: 재시도가
  아니라 "edit_file을 쓰거나 더 작은 단위로 작업하라"는 교정 메시지를 되먹인다
  (같은 요청 재시도는 같은 지점에서 다시 잘림)
- 서버 컨텍스트 초과 에러: bytes/4 휴리스틱은 한국어 밀도가 높으면 과소추정할 수
  있으므로, 먼저 히스토리를 한 단계 더 공격적으로 절삭하고 1~2회 재시도한다.
  이미 최소 프롬프트인데도 초과하면 그때 `context_tokens` 설정과 서버 로드 설정을
  맞추라는 안내를 표시
- 툴 실행 에러(파일 없음, 매칭 실패, 명령 실패, 경로 탈출, 차단 패턴): 크래시가
  아니라 **모델에게 반환되는 데이터**. 에이전트 루프는 계속된다
- 라이브러리성 모듈(llm, tools)은 `thiserror`, 앱 레벨은 `anyhow`

## 10. 크로스플랫폼 세부

- `run_command`: Windows `cmd /C`, 그 외 `sh -c`
- **타임아웃 킬은 프로세스 트리 전체를 대상으로 한다**: 셸만 죽이면 손자 프로세스
  (예: `cargo test`가 띄운 테스트 바이너리)가 고아로 남는다. Unix는 프로세스 그룹
  (`setpgid` 후 `kill(-pgid)`), Windows는 Job Object(또는 `taskkill /T /F`) 사용
- 명령 출력 디코딩: UTF-8 우선, 실패 시 `encoding_rs` 손실 디코딩 (한국어 Windows
  콘솔 출력은 CP949)
- 경로: 내부는 `std::path`, 모델에게 보여줄 때는 `/` 정규화, 입력은 `/`와 `\` 모두
  수용 (§4 경로 확인 규칙 참조)
- 배포: OS별 크로스컴파일(또는 OS별 빌드), 바이너리 + 설정 예시 파일만 반입하면 끝

## 11. 테스트 전략

- 구현은 TDD로 진행 (superpowers:test-driven-development)
- `tools`: 임시 디렉터리 기반 단위 테스트 (경로 탈출, 심링크, CRLF, 비UTF-8 파일,
  매칭 사다리 각 단계, 차단 패턴 케이스 포함)
- `llm`: wiremock 목 서버로 요청 형식/재시도/스트리밍 파싱 검증
- `agent`: 스크립트된 가짜 `LlmClient`로 루프 시나리오 테스트
  (정상 완료, 파싱 실패 재시도, 확인 거부, 최대 턴 도달, 반복 감지, 출력 잘림)
- `eval` 하네스 자체도 픽스처로 테스트 (protected 복원 포함)
- 실모델 통합 검증은 M4의 평가 하네스가 겸함

## 12. 마일스톤

1. **M1 — 채팅 REPL**: 설정 로드, OpenAI 호환 클라이언트(스트리밍), REPL.
   툴 없음. LM Studio와 대화가 됨
2. **M2 — 읽기 에이전트**: 구조화 출력 루프 + read_file/list_files/grep + finish.
   코드베이스 질문에 답할 수 있음 (답변은 finish.summary로)
3. **M3 — 가이드형 완성**: write_file/edit_file/run_command + 확인 게이트 +
   diff 미리보기 + 히스토리 절삭 + 반복 감지. **v1 목표 달성 지점**
4. **M4 — 평가**: eval 서브커맨드 + 과제 세트 + Gemma/Qwen 4B급 기준선 측정
   (`--repeats`로 평균 통과율)
5. **M5+ — 자율성 실험**: 플래너-이그제큐터, 자기검증 루프, tree-sitter repo map,
   컨텍스트 압축 등. 항상 M4 평가로 개선 여부 판정

## 개정 이력

- 2026-07-02: 최초 승인본
- 2026-07-02: 아키텍처 리뷰 19건 반영 — tool role 금지(user 래핑), bytes/4 토큰
  추정, max_output_tokens 분리 + length 처리, --auto 차단 패턴, 프로세스 트리 킬,
  경로 확인 규칙, 매칭 사다리, 반복 감지, -p/종료코드 계약, eval repeats/protected,
  인코딩 정책, 세션 기록 스키마, 답변 채널 명시, 에이전트 턴 비스트리밍, 얕은 스키마
- 2026-07-02: 2차 리뷰 12건 반영 — 반복별 상이 시드, 입력 예산 공식 명시, 컨텍스트
  초과 시 절삭 재시도, 턴 쌍 단위 절삭(교대 보존), 크로스플랫폼 차단 패턴, 매칭
  사다리 정밀화(EOL 정규화·다중매칭 즉시 에러), -p stdout 계약, /chat 커맨드 결정,
  protected 동기화 의미론, Gemma system role 폴백, 반복 감지 사각지대 명시, eval
  종료코드/타임아웃 산정
