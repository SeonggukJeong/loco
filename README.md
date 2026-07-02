# loco — 폐쇄망 소형모델 코딩 CLI

로컬에서 서빙되는 소형 LLM(OpenAI 호환 API)으로 코딩을 지원하는 CLI.
설계 문서: `docs/superpowers/specs/2026-07-02-loco-design.md`

## 시작하기

1. LM Studio(또는 Ollama, llama.cpp server 등)에서 모델을 로드하고 서버 시작
   - LM Studio 기본 주소 `http://localhost:1234/v1` 는 설정 없이 바로 동작
2. 실행:

   ```
   cargo run                 # 대화형 REPL
   cargo run -- -p "질문"    # 단발 실행
   ```

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
- [ ] M2: 읽기 도구 에이전트
- [ ] M3: 가이드형 코딩 에이전트
- [ ] M4: 평가 하네스
