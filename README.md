# loco — 폐쇄망 소형모델 코딩 CLI

로컬에서 서빙되는 소형 LLM(OpenAI 호환 API)으로 코딩을 지원하는 CLI.
설계 문서: `docs/superpowers/specs/2026-07-02-loco-design.md`

## 시작하기

1. LM Studio(또는 Ollama, llama.cpp server 등)에서 모델을 로드하고 서버 시작
   - LM Studio 기본 주소 `http://localhost:1234/v1` 는 설정 없이 바로 동작
2. 실행:

   ```
   cargo run                 # 대화형 에이전트 REPL
   cargo run -- -p "질문"    # 단발 실행 (답변만 stdout, 종료코드 0/1/2)
   ```

## 사용법

REPL에 입력한 내용은 에이전트가 처리한다 — 모델이 read_file/list_files/grep
툴로 프로젝트를 조사하고, 답은 마지막에 한 번에 출력된다 (`finish`).
진행 중 Ctrl+C 로 취소할 수 있다.

- `/chat <메시지>` — 에이전트 없이 모델과 바로 스트리밍 대화 (빠른 질문용)
- `/clear` — 히스토리 초기화. M2는 히스토리 절삭이 없으므로 긴 세션에서
  컨텍스트가 넘치면 이 명령으로 비운다 (자동 절삭은 M3)
- `/config`, `/help`, `/quit`

`-p` 모드 종료 코드: `0` 정상(finish), `1` 에러(연결 실패·파싱 실패),
`2` 최대 턴 도달. 진행 표시는 stderr로 가므로 stdout만 파이프하면 답변만 남는다.

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
- [ ] M3: 가이드형 코딩 에이전트
- [ ] M4: 평가 하네스
