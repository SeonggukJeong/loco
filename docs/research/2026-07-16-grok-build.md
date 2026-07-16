# 레퍼런스 노트 — xai-org/grok-build (M8 Task9)

M8 스펙 §7(확장 조항)의 1차 승격 대상. 2026-07-16 사용자 지적("약한 모델 보정
공학 가설")을 검증하기 위해 조사했다 — grok-build가 loco의 `src/agent/
protocol.rs` 살베지 파싱(모델 출력의 형식 일탈을 관용적으로 흡수하는 다단계
파서)과 유사한 "견고성 공학"을 갖고 있는지가 핵심 질문이다. 스펙 §1에서
grok-build는 "풀스크린 TUI가 정체성"이라 후순위였으나, 이번 조사는 **TUI/
pager 크레이트를 명시적으로 제외**하고 에이전트 메커니즘(`crates/codegen/
xai-grok-agent`, `xai-grok-tools`, `xai-grok-shell`, `xai-grok-subagent-
resolution`, `crates/common/xai-tool-runtime`)에 한정했다. `gh api
repos/xai-org/grok-build/contents/...`로 2026-07-17 `main`에서 직접 가져온
소스만 근거로 쓴다.

## 1. 에이전트 루프의 응답 파싱·도구 디스패치 — 가설 검증

**아키텍처**: grok-build가 지원하는 3개 API 백엔드(`ApiBackend` enum,
`xai-grok-sampling-types/src/types.rs` 1013-1021행) — `ChatCompletions`
(`/v1/chat/completions`), `Responses`(`/v1/responses`), `Messages`
(Anthropic `/v1/messages`) — 는 **전부 네이티브 구조화 툴 콜링**을 쓴다.
즉 모델의 자유 텍스트에서 JSON을 긁어내는 경로 자체가 없다 — API가 이미
파싱된 `tool_calls[].function.{name, arguments}`를 돌려준다.

**툴 인자 디코딩**(`crates/common/xai-tool-runtime/src/tool.rs` 359-362행,
`ToolDyn::execute` 블랭킷 구현):
```rust
let typed_args: T::Args = match serde_json::from_value(args) {
    Ok(v) => v,
    Err(e) => return terminal_only(Err(ToolError::invalid_arguments(e.to_string()))),
};
```
1회성 `serde_json::from_value` — 스키마 불일치는 **즉시 하드 에러**로
끝난다. 살베지(펜스 제거·필드 승격·재시도 사다리) 없음. "재시도"는 결국
그 에러가 툴 결과로 모델에게 돌아가 모델이 다음 턴에 스스로 고쳐 부르는
자연스러운 대화 루프일 뿐, 별도의 파싱-복구 코드 경로는 없다.

**존재하는 재시도 인프라**(`xai-grok-tools::retry`,
`xai-grok-sampler::retry`)는 지수 백오프 기반 **일반 네트워크/레이트리밋
재시도**다(`BackoffConfig`, `max_retries`/`base_delay_ms`) — malformed
응답 복구와는 무관한, 전송 계층 문제 대응이다.

**가설과 다른 방향에서 발견한 견고성 공학 2건** (loco의 살베지와 메커니즘은
다르지만 "약한/변칙 입력에 대한 관용" 철학은 공유):

- **설정 파일 관용 파싱**(`xai-grok-shell/src/agent/
  config_model_override_parse.rs` 1-13행 모듈 주석): *"A model entry must
  survive a bad field: warn and skip the field, never drop the model."*
  `[model.<id>]` TOML 오버라이드를 통째로 파싱 실패시키는 대신 실패한
  필드만 제거하며 재시도하고(`prune_invalid_fields`), 그래도 안 되면 빈
  오버라이드로 모델 항목만은 살려둔다. 대상은 **모델 출력이 아니라
  사용자가 손으로 쓴 config**이지만, "부분 실패를 전체 실패로 전파시키지
  않는다"는 철학은 loco의 살베지 파서와 동형이다.
- **서버 신호 "doom loop" 감지**(`xai-grok-sampler/src/doom_loop.rs`):
  xAI 백엔드 전용 SSE 확장 이벤트(`response.doom_loop_check`)로 모델의
  토큰 반복 루프(`tail_repetition:N@channel`)를 **서버가** 감지해 보고하고,
  클라이언트는 이 신호의 페이로드가 깨져 있어도(비-JSON, 태그 없음 등)
  스트림 전체를 실패시키지 않고 조용히 삼킨다(테스트
  `named_event_with_garbage_payload_still_swallowed`가 이를 고정). "확신
  트리거"가 쌓이면 스트림을 중단·재시도하고 재시도 예산을 다 쓰면 해제해
  마지막 시도는 그대로 받아들인다. 이는 **퇴화 반복 출력**에 대한 견고성
  공학이지 **형식이 어긋난 툴 호출 JSON**에 대한 것이 아니며, 탐지 자체가
  클라이언트가 아니라 xAI 서버 쪽에서 일어난다.

**판정**: 가설(loco류 JSON 살베지 파싱)은 **핵심 메커니즘 차원에서는
불확인(negative)** — 구조화 API 툴콜링을 전제하는 한 그런 파서가 필요
없기 때문이다. 다만 "약한/변칙 입력에 대한 관용"이라는 **더 넓은 주제는
확인**됐고, grok-build는 그 관용을 다른 층위(사용자 config 파싱, 서버발
퇴화-신호 처리)에 흩어 구현하고 있다. loco가 겪는 문제(로컬 소형 모델이
JSON 봉투 형태를 자주 어긴다)에 대한 직접적 선례는 아니다 — grok-build가
지원하는 3개 백엔드 모두 진짜 함수 호출 API를 전제하기 때문에, 해당 API를
안정적으로 구현하지 않는 로컬 서버를 만나면 grok-build도 loco와 같은
문제를 겪을 것으로 추정되나 이는 소스에서 확인할 수 없었다(로컬 백엔드가
tool_calls 필드를 어떻게 채우는지는 백엔드 구현 몫).

## 2. 도구 계층의 편집·검색 설계

**search_replace**(주 편집 툴, `xai-grok-tools/src/implementations/
grok_build/search_replace/mod.rs`, ~2500행) 매칭 순서: ① CRLF→LF 정규화
후 리터럴 완전일치(`match_text.match_indices`) ② 0개 매칭이면, 설정
`unicode_normalized_fallback`이 켜진 경우에 한해 `find_normalized_match_
positions`(helpers.rs 177-238행)로 **유니코드 컨퓨저블 정규화** 폴백 1
단계(스마트 따옴표·em-dash·nbsp·ellipsis 등을 ASCII로 접어 비교, 라운드트립
검증으로 부분확장 오탐 차단, 겹치는 매치는 fail-closed로 Ambiguous 처리).
**공백/들여쓰기 관용 단계는 없다** — codex-rs의 rstrip/trim 단계나 loco의
들여쓰기 시프트 단계에 해당하는 것이 없고, 퍼지 폴백은 유니코드
컨퓨저블 1종뿐이다.

**실패 시 진단 힌트** 2종(both `is_legacy` 플래그로 구버전 문구 호환 시
생략): `build_nearest_match_hint`(old_string 첫 줄에서 가장 긴 단어를
뽑아 그 단어가 포함된 파일 내 첫 줄을 "Nearest match: line N: ..."로
보고, 200자 절단)와 `build_confusable_hint`(파일에 유니코드 컨퓨저블이
있고 정규화 매치가 존재하면 정확한 영향 줄 번호를 나열하며 ASCII 앵커로
다시 시도하라고 안내) — **loco `edit_file`의 0-매칭 힌트(첫 줄 substring
→ char-bigram 폴백, CLAUDE.md)와 목적이 같다**. 다만 grok-build는 단일
전략(최장 단어 substring)이고 loco는 2단계 폴백이라는 점이 다르다.
매칭이 2개 이상이고 `replace_all`이 없으면 `MultipleMatchesFound` 에러 —
loco의 ≥2-매칭 에러와 동형.

특히 인상적인 발견: 툴 설명 템플릿(mod.rs 근처 62행 주석)이 *"`${{
tools.by_kind.read }}`가 각 줄 앞에 'LINE_NUMBER→'를 붙인다. 이 접두는
파일의 일부가 아니다: → 뒤에 오는 것만, 정확한 들여쓰기로 매치하라"*고
명시한다 — **loco가 CLAUDE.md에 적어둔 정확히 같은 실패 모드**("line
numbers in the header only — models copy body prefixes into their next
search")를 grok-build도 별도로 문서화해 대응하고 있다.

**검색 툴**(`grok_build/grep/{mod.rs,ripgrep.rs}`): 실제 `rg` 바이너리를
셸아웃한다 — 릴리스 빌드는 번들된 rg를 `~/.grok/vendor/`에 추출해
실행권한을 부여하고, 아니면 PATH/`RG_BIN_PATH`/hermetic 테스트 러너의
`RUNFILES_DIR`에서 찾는다. 출력은 head-limit·플랫폼별 타임아웃(WSL은
별도 값)·3가지 출력 모드(content/files_with_matches/count)로 후처리된다.
loco의 자체 구현 regex 기반 grep(정규식 무효 시 리터럴 검색 폴백,
CLAUDE.md)과 달리 grok-build는 진짜 ripgrep이라 정규식/글롭 표현력은
높지만 바이너리 번들이라는 배포 비용을 진다.

**범위 밖 참고**: 같은 트리에 `implementations/{codex/apply_patch,opencode/
edit,grok_build_hashline}`(앵커/해시 기반의 별도 편집 스킴)이 공존한다 —
에이전트 페르소나/임포트 호환을 위한 다중 방언 지원으로 보이나, 이번
노트는 주 편집 경로(search_replace)에 집중하고 존재만 기록한다.

## 3. 커스텀/로컬 모델 지원 폭

**설정 파일 위치 확인**: `xai-grok-config/src/loader.rs::load_from_disk()`
→ `load_user_config_layer(user_grok_home(), "config.toml")`, 그리고
`grok_home()`(`xai-grok-config/src/paths.rs`)은 `$GROK_HOME` 환경변수 또는
`~/.grok`(dunce로 정규화) — **`~/.grok/config.toml` 존재를 소스로 확인**
(플랜의 가설 확인).

**`[model.<id>]` 오버라이드**(`ConfigModelOverride` 구조체,
`xai-grok-shell/src/agent/config.rs` 3570-3610행): 모델별로 `base_url`,
`api_base_url`(별도 필드로 존재 — 둘의 관계는 소스만으로 확정 못함),
`api_key`, `env_key`(문자열 또는 배열), `api_backend`(ChatCompletions/
Responses/Messages), `context_window`, `max_completion_tokens`,
`temperature`, `top_p`, `extra_headers`, `auto_compact_threshold_percent`,
`stream_tool_calls` 등을 지정할 수 있다 — **loco의 단일 전역 `base_url`과
달리, 임의 개수의 커스텀/로컬 엔드포인트를 각자 다른 모델 키 아래
동시에 등록**할 수 있는 폭이다. 기본 백엔드 `ChatCompletions`
(`/v1/chat/completions`)는 Ollama·LM Studio·vLLM 등 OpenAI-호환 로컬
서버가 그대로 타깃할 수 있는 형태 — loco 자신이 호출하는 엔드포인트
형태와 동일하다.

**관용적 config 파싱**(§1 참고)이 이 표면에도 적용된다 — 손으로 쓴
`[model.custom-llm]` 항목의 필드 하나가 깨져도 그 필드만 건너뛰고
경고(`grok inspect`로 조회 가능)하지, loco의 `deny_unknown_fields`
(CLAUDE.md, 오타를 즉시 거부)처럼 전체를 거부하지 않는다 — 방향이
반대인 설계 선택이다.

**managed config 레이어**: 시스템/사용자 스코프의 `managed_config.toml`,
macOS의 `ClaudeCode/managed-settings.json` 호환 경로까지 언급되는
조직-정책 레이어가 있다 — loco에는 대응 개념이 없다(조사만 하고 이식
판단은 보류).

## 4. 서브에이전트 구조 + loco에의 시사점

**성숙한 서브에이전트 시스템**이 실재한다 — `xai-grok-subagent-resolution`
(순수 설정-해석 크레이트: 명시적 오버라이드 > role > persona > parent
상속 순의 우선순위, persona 지침 로딩, resume 신원 검증)과 `task` 툴
(`xai-grok-tools/src/implementations/grok_build/task/`) + 셸 쪽 코디네이터로
나뉜다.

- **깊이 제한**: `MAX_SUBAGENT_DEPTH = 1` — 서브에이전트는 재귀적으로 또
  서브에이전트를 띄울 수 없다(하드 캡).
- **격리**: `SubagentIsolationMode`에 워크트리 격리가 포함된다
  (`SubagentResult.worktree_path`) — 대화 수준이 아니라 **파일시스템
  수준**의 컨텍스트 격리.
- **결과 형태**(`SubagentResult`, `task/types.rs` 306-335행): `output:
  Arc<str>`을 "서브에이전트의 최종 출력 텍스트... 서브에이전트 출력은
  임의로 클 수 있다(전체 트랜스크립트)"라고 문서화하고 있다 — 즉
  부모에게 돌아가는 계약은 **자식의 최종 텍스트 응답**이며, 자식이
  실제로 수행한 툴 호출·중간 탐색 이력은 자식 세션에 남고 부모 컨텍스트로
  재생되지 않는다. `tool_calls`/`turns`/`duration_ms`/`tokens_used` 등은
  메타데이터로만 곁들여진다. **이것이 m9-candidates의 "컨텍스트
  임대"(서브에이전트에 위임하고 증류된 결과만 회수)를 정확히 구현한
  실제 사례다.**
- **resume**(2차 패턴): `resume_from`으로 **새 서브에이전트가 이미 완료된
  다른 서브에이전트의 원시 트랜스크립트·툴 상태·모델을 통째로 이어받을
  수 있다** — 단순 부모→자식 위임을 넘어, 서브에이전트 간 순차적 컨텍스트
  인계까지 지원한다.
- **백그라운드 실행**: `run_in_background`로 분리 실행, 결과는 나중에
  `get_task_output`으로 회수, 부모 턴 취소는 `parent_prompt_id` 스코핑으로
  그 턴이 띄운 서브에이전트만 취소하고 이전 턴의 백그라운드 서브에이전트는
  건드리지 않는다. `task` 툴 자체가 `get_task_output`/`kill_task` 툴의
  동시 존재를 전제 조건으로 건다(`requires_expr`) — 부모가 자신이 띄운
  것을 관리할 수단 없이는 위임 자체를 못 하게 막는 설계.

**loco에의 시사점**: grok-build는 m9-candidates "컨텍스트 임대" 아이디어가
성숙한 규모에서 실제로 동작함을 보여주는 참조 구현이다 — 아이디어 자체의
타당성 검증으로는 유효하다. 다만 규모 격차가 크다: role/persona/워크트리
격리/resume/백그라운드 실행을 갖춘 이 기계장치는 "서브에이전트에 과제를
정식화해 위임하고, 돌아온 요약을 통합할 수 있는" 유능한 오케스트레이터
모델을 전제한다 — m9-candidates가 이미 적어둔 리스크(*"소형 모델은 위임
능력이 약해 오케스트레이터 역할 자체가 병목일 수 있음"*)와 정확히
부딪힌다. 또한 grok-build의 백그라운드/병렬 서브에이전트 실행은 xAI
백엔드의 서버측 동시 처리를 전제하는데, loco의 로컬 GPU 1장 환경은
서브런이 병렬이 아니라 **직렬 + 회당 프리필 추가 비용**이라는
m9-candidates 리스크 ②를 그대로 진다 — grok-build는 이 축을 해결할
필요가 없다. 가장 작고 이식성 높은 조각만 추리면: ① `MAX_SUBAGENT_DEPTH`
류 하드 깊이 캡(재귀 위임 폭주 방지, 구현 비용 거의 0) ② 워크트리
격리는 loco eval의 fixture→temp-sandbox 복사 메커니즘과 결이 비슷해
재사용 여지가 있다 — 전체 role/persona/resume 체계보다 훨씬 작은
첫걸음이다.

## 출처

전부 `gh api repos/xai-org/grok-build/contents/<path>`로 2026-07-17 `main`에서
직접 취득(`-H "Accept: application/vnd.github.raw"`로 원문, 대형 파일은 grep으로
대상 구간만 확인).

- §1: `crates/common/xai-tool-runtime/src/{tool,dispatch,error}.rs`(툴
  트레이트·인자 디코딩, 무살베지 확인) · `xai-grok-tools/src/retry.rs` +
  `xai-grok-sampler/src/{retry,doom_loop}.rs`(229행, 서버발 퇴화-반복 신호
  관용 파싱) · `xai-grok-shell/src/agent/config_model_override_parse.rs`
  (24213바이트, config 관용 파싱 철학) · `xai-grok-sampling-types/src/
  types.rs` 1013-1030행(`ApiBackend` 3종 전부 구조화 API 확인)
- §2: `xai-grok-tools/src/implementations/grok_build/search_replace/
  {mod,helpers}.rs`(2482+532행, 매칭 사다리·진단 힌트) ·
  `.../grok_build/grep/{mod,ripgrep}.rs`(ripgrep 셸아웃)
- §3: `xai-grok-config/src/{paths,loader}.rs`(`~/.grok/config.toml` 경로) ·
  `xai-grok-shell/src/agent/config.rs` 3570-3610행(`ConfigModelOverride`,
  파일 전체 462KB 중 grep으로 대상 구간만 취득 — 전체 미열람) ·
  `xai-grok-sampler/src/config.rs`(`SamplerConfig`)
- §4: `xai-grok-subagent-resolution/src/{lib,types}.rs` ·
  `xai-grok-tools/src/implementations/grok_build/task/{mod,types,backend}.rs`
  (TaskTool·`SubagentResult`·깊이 제한) · `xai-grok-agent/src/agent.rs`
  (296행, `should_auto_compact` 참고용 — 필수 절 범위 밖)
- 로컬: `CLAUDE.md`, `src/agent/protocol.rs`, `docs/m9-candidates.md`,
  `docs/superpowers/specs/2026-07-16-m8-large-repo-track-design.md` §7 —
  loco 비교 기준 및 승격 배경 (§1, §4)

**미확인/조사 범위 밖**: `api_base_url`과 `base_url` 필드의 정확한 역할
분담(양쪽 다 `ConfigModelOverride`에 존재, `apply()` 메서드 일부만 확인),
managed_config.toml의 조직-정책 병합 순서 상세, `grok_build_hashline`/
`opencode`/`codex` 방언 툴들의 내부 매칭 로직(스코프 가드로 제외), 로컬
OpenAI-호환 서버가 `tool_calls` 필드를 불완전하게 채울 때 grok-build가
실제로 어떻게 반응하는지(백엔드 구현에 좌우되며 클라이언트 소스만으로는
확인 불가).
