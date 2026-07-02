# loco — 폐쇄망 소형모델 코딩 CLI 설계

- 날짜: 2026-07-02
- 상태: 승인됨 (브레인스토밍 세션 결과)
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
4. 이후 모든 스캐폴딩 개선은 평가 통과율 변화로 검증

### 비목표 (v1)

- 추론 엔진 내장 (llama.cpp/mistral.rs 임베딩) — OpenAI 호환 API로 충분, 후속 옵션
- 풀스크린 TUI (ratatui) — v1은 라인 기반 REPL
- 네이티브 function calling 지원 — 구조화 출력 방식으로 대체 (아래 §4)
- tree-sitter 기반 repo map — v1은 디렉터리 트리만
- 멀티 에이전트, 서브에이전트, MCP 연동

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
`similar`(diff 미리보기), `wiremock`(dev-dep).

## 3. 아키텍처

단일 바이너리 크레이트, 모듈 경계는 다음과 같다.

```
src/
├── main.rs      — clap 파싱, 서브커맨드 디스패치
├── config.rs    — 설정 로드/병합
├── llm/         — OpenAI 호환 클라이언트
│   ├── client.rs    — HTTP, 스트리밍(SSE), 재시도
│   └── types.rs     — 요청/응답 타입 (serde)
├── tools/       — 툴 정의 + 실행
│   ├── mod.rs       — Tool 트레이트, 레지스트리, 디스패치
│   └── (툴별 파일)
├── agent/       — ReAct 루프, 컨텍스트 예산 관리
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
2. `agent`가 `LlmClient::chat` 호출 (JSON Schema 강제, 온도 0.1)
3. 응답 JSON 파싱 → `{thought, action}` — 실패 시 에러를 모델에 되먹여 재시도(최대 3회)
4. `thought`를 사용자에게 표시
5. `action.tool`이 mutating이면 미리보기(diff/명령어) 후 y/n 확인
6. 툴 실행 → 결과(성공/에러 모두)를 tool 메시지로 히스토리에 추가
7. `finish` 툴이 호출되거나 최대 턴 수(기본 25) 도달 시 사용자에게 제어 반환

## 4. 툴 프로토콜 — 4B 대응의 핵심 설계

**네이티브 function calling을 쓰지 않는다.** 소형 모델(특히 Gemma 계열)은 함수
호출 포맷 훈련이 약하다. 대신 매 턴 아래 형태의 JSON 하나를 출력하도록
`response_format: json_schema`(grammar-constrained decoding)로 강제한다.

```json
{"thought": "...", "action": {"tool": "read_file", "args": {"path": "src/main.rs"}}}
```

- 서버가 json_schema를 지원하지 않으면 텍스트에서 JSON 추출 + 재시도 폴백
- 한 턴에 툴 호출 하나만 (병렬 호출 없음)
- 시스템 프롬프트는 영어(소형 모델의 지시 이행률이 영어에서 더 안정적),
  few-shot 예시 1~2개 포함. 사용자 대화 언어는 자유.

### 툴 목록 (7개 고정)

| 툴 | 인자 | 동작/제한 |
|---|---|---|
| `read_file` | path, offset?, limit? | 기본 최대 200줄, 초과 시 페이지 안내 |
| `write_file` | path, content | 새 파일 생성 또는 전체 덮어쓰기. mutating |
| `edit_file` | path, search, replace | search는 정확히 1회 매칭 필수. 실패 시 근접 부분을 에러로 반환. mutating |
| `list_files` | path?, depth? | ignore 크레이트(gitignore 존중), 항목 수 상한 |
| `grep` | pattern, path? | regex, 최대 50매치, 매치당 전후 2줄 |
| `run_command` | command | 타임아웃 60초(설정 가능), stdout+stderr 합산 후 절삭. mutating |
| `finish` | summary | 작업 종료 선언, 요약을 사용자에게 표시 |

### 파일 수정 형식

unified diff는 금지 (소형 모델이 라인 번호/컨텍스트를 못 맞춤). `edit_file`의
검색/치환 블록과 `write_file` 전체 재작성만 허용. edit 적용 전 `similar`로 diff를
렌더링해 확인 게이트에서 보여준다. CRLF 파일은 라인엔딩을 보존한다.

## 5. 확인 게이트 (가이드형 모드)

- 읽기 툴: 자동 실행, `→ read_file src/main.rs` 한 줄 알림
- mutating 툴: 미리보기 표시 후 `[y]es / [n]o` 프롬프트.
  거부 시 거부 사실이 tool 결과로 모델에 전달됨 (모델이 다른 접근 시도 가능)
- `--auto` 플래그: 전부 자동 승인. 평가 하네스가 사용하며 향후 자율 모드의 기반

## 6. 컨텍스트 관리

- 토큰 추정: `chars / 4` 휴리스틱 (모델별 토크나이저 정합성보다 단순함 우선)
- 예산: 설정 `context_tokens` 기본 8192, 출력용 1024 예약
- 시스템 프롬프트에 프로젝트 디렉터리 트리 주입 (상한 있음)
- 예산 초과 시 오래된 턴의 툴 결과부터 `[결과 생략]`으로 치환, 그래도 초과면
  가장 오래된 사용자/어시스턴트 턴 제거 (시스템 프롬프트와 현재 요청은 보존)

## 7. CLI 인터페이스

```
loco                     # 대화형 REPL (기본)
loco -p "..."            # 단발 실행 (비대화형, CI/스크립트용)
loco --auto              # 확인 게이트 전부 자동 승인
loco eval <tasks-dir>    # 평가 하네스 실행
```

REPL 슬래시 커맨드: `/help`, `/clear`(히스토리 초기화), `/config`(현재 설정 표시),
`/quit`.

### 설정

- 전역: `directories` 크레이트의 config_dir (`~/.config/loco/config.toml` 등)
- 프로젝트: `./.loco/config.toml` (전역을 덮어씀), 세션 기록: `./.loco/sessions/*.jsonl`
- 항목: `base_url`(기본 `http://localhost:1234/v1` — LM Studio 기본 포트),
  `api_key`(선택), `model`, `temperature`(기본 0.1), `context_tokens`(기본 8192),
  `max_turns`(기본 25), `command_timeout_secs`(기본 60)

## 8. 평가 하네스 (`loco eval`)

과제 하나 = 디렉터리 하나:

```
tasks/add-function/
├── task.toml       # prompt, check 명령, timeout, max_turns
└── fixture/        # 과제 시작 시점의 작업 디렉터리 내용
```

실행 흐름: fixture를 임시 샌드박스에 복사 → `--auto`로 에이전트 실행 →
`check` 명령(예: `cargo test`, `python -m pytest`) 종료코드로 판정 →
과제별 통과/실패 + 턴 수 + 소요시간 리포트 (표 + JSON 저장).

초기 과제 세트 10~20개: 함수 추가, 명백한 버그 수정, 실패 테스트 통과시키기,
grep 기반 코드 찾기 등 난이도 사다리 구성. 과제 세트 자체도 리포지토리에 포함.

## 9. 에러 처리

- 연결 실패/5xx: 지수 백오프 재시도 3회, 최종 실패 시 "LM Studio 서버가 떠있는지
  확인하세요 (base_url: ...)" 수준의 실행 가능한 메시지
- 모델 출력 파싱 실패: 에러를 모델에 되먹여 재시도 3회 → 그래도 실패면 사용자에게
  원문 표시 후 제어 반환
- 툴 실행 에러(파일 없음, 매칭 실패, 명령 실패): 크래시가 아니라 **모델에게
  반환되는 데이터**. 에이전트 루프는 계속된다
- 라이브러리성 모듈(llm, tools)은 `thiserror`, 앱 레벨은 `anyhow`

## 10. 크로스플랫폼 세부

- `run_command`: Windows `cmd /C`, 그 외 `sh -c`. 프로세스 킬로 타임아웃 강제
- 경로: 내부는 `std::path`, 모델에게 보여줄 때는 `/` 정규화
- 배포: OS별 크로스컴파일(또는 OS별 빌드), 바이너리 + 설정 예시 파일만 반입하면 끝

## 11. 테스트 전략

- 구현은 TDD로 진행 (superpowers:test-driven-development)
- `tools`: 임시 디렉터리 기반 단위 테스트 (경로 탈출, CRLF, 매칭 실패 케이스 포함)
- `llm`: wiremock 목 서버로 요청 형식/재시도/스트리밍 파싱 검증
- `agent`: 스크립트된 가짜 `LlmClient`로 루프 시나리오 테스트
  (정상 완료, 파싱 실패 재시도, 확인 거부, 최대 턴 도달)
- `eval` 하네스 자체도 픽스처로 테스트
- 실모델 통합 검증은 M4의 평가 하네스가 겸함

## 12. 마일스톤

1. **M1 — 채팅 REPL**: 설정 로드, OpenAI 호환 클라이언트(스트리밍), REPL.
   툴 없음. LM Studio와 대화가 됨
2. **M2 — 읽기 에이전트**: 구조화 출력 루프 + read_file/list_files/grep + finish.
   코드베이스 질문에 답할 수 있음
3. **M3 — 가이드형 완성**: write_file/edit_file/run_command + 확인 게이트 +
   diff 미리보기 + 히스토리 절삭. **v1 목표 달성 지점**
4. **M4 — 평가**: eval 서브커맨드 + 과제 세트 + Gemma/Qwen 4B급 기준선 측정
5. **M5+ — 자율성 실험**: 플래너-이그제큐터, 자기검증 루프, tree-sitter repo map,
   컨텍스트 압축 등. 항상 M4 평가로 개선 여부 판정
