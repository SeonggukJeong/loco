# 레퍼런스 노트 — aider의 repo-map (M8 Task8)

M8 대형 저장소 트랙에서 `tasks-large/README.md`가 실측한 문제는 이렇다: loco의 시스템
프롬프트 트리(`project_tree()`, `src/agent/prompt.rs`)는 depth=3 재귀 상한 때문에
`inv-core/src/rules/pricing.rs`·`inv-core/src/rules/mod.rs` 같은 depth-4 정답 파일을
**알파벳 순서·100개 상한과 무관하게 무조건** 못 본다. 이건 "트리를 더 깊게 보여주면
되지 않나"로 끝날 문제가 아니다 — 깊게 보여주면 8K 컨텍스트에서 이번엔 개수 상한에
먼저 걸린다. aider의 repo-map은 정확히 이 트레이드오프(저장소는 크고 컨텍스트는 작다)를
정면으로 다루는 기존 구현이라 M9 설계의 참고선으로 조사했다.

이 노트는 aider 공식 문서 2건과 `aider/repomap.py` 소스(2026-07-17 시점 GitHub
`Aider-AI/aider` main, 867줄)를 직접 가져와 확인한 내용만 적는다. 소스에서 확인한
내용과 문서에서만 확인한 내용을 구분해 표기했다.

## 1. tree-sitter 태그 추출 방식

**소스 확인** (`get_tags_raw`, `repomap.py` 233-363행).

- 파일마다 `grep_ast.filename_to_lang(fname)`로 언어를 판별하고, 언어가 없으면
  스킵한다. `grep_ast.tsl.get_language`/`get_parser`로 tree-sitter 파서를 얻는다.
- 언어별 쿼리 파일은 `{lang}-tags.scm`이며, 패키지 내 `aider/queries/` 아래
  `tree-sitter-language-pack/`(신규, `USING_TSL_PACK=True`일 때) 또는
  `tree-sitter-languages/`(레거시 fallback) 서브디렉터리에서 읽는다
  (`get_scm_fname`, 805-829행). 공식 블로그(아래 출처)는 이 `.scm` 쿼리들이
  C/C#/C++/Elisp/Elixir/Elm/Go/Java/JavaScript/OCaml/PHP/Python/QL/R/Ruby/Rust/
  TypeScript 등 각 언어의 기존 tree-sitter 문법 저장소에서 가져와 수정한 것이라고
  밝힌다. `get_supported_languages_md()`는 `grep_ast.parsers.PARSERS`에 등록된
  전체 언어를 순회하며 `.scm` 파일 존재 여부로 "repo-map 지원" 표를 자동 생성한다 —
  즉 지원 언어 수는 하드코딩 목록이 아니라 쿼리 파일 존재 여부로 결정된다.
- 소스를 파싱해 AST를 얻은 뒤 `Query(language, query_scm)`을 루트 노드에 실행해
  캡처를 얻는다(`_run_captures`—tree-sitter 파이썬 바인딩 0.23→0.24 API 변경을
  두 갈래로 흡수). 캡처 이름이 `name.definition.`로 시작하면 `kind="def"`,
  `name.reference.`로 시작하면 `kind="ref"`, 그 외는 버린다. 각 태그는
  `Tag(rel_fname, fname, name=<노드 텍스트>, kind, line=<0-based 시작 줄>)`.
- **정의만 있고 참조가 하나도 없는 언어 보완**: 일부 `.scm`은 정의만 잡고 참조는
  안 잡는다(주석에 C++ 예시). 이 경우 pygments의 `guess_lexer_for_filename`으로
  전체 소스를 토크나이즈해 `Token.Name` 토큰 전부를 `kind="ref", line=-1`로
  대체 생성한다. 반대로 `ref`가 하나라도 잡혔으면 이 폴백은 아예 건너뛴다.

**캐싱** (소스 확인, 35-43행·217-264행).

- 캐시 디렉터리 이름에 버전을 박아 넣는다: `CACHE_VERSION = 3`, 단
  `USING_TSL_PACK`이면 4 → `.aider.tags.cache.v{3|4}` (프로젝트 루트에 생성).
  쿼리 팩이 바뀌면 버전 상수를 올려 구 캐시를 자동 무효화하는 방식.
- `diskcache.Cache`(SQLite 기반)를 쓰고, SQLite 오류가 나면 `tags_cache_error`가
  캐시 디렉터리를 지우고 재생성을 시도하다 그래도 안 되면 평범한 `dict`로
  강등한다(오프라인/권한 문제 환경에서도 죽지 않게).
- 캐시 키는 **절대 경로**, 값은 `{"mtime": <파일 mtime>, "data": [Tag,...]}`.
  `get_tags()`는 현재 mtime과 캐시된 mtime이 같으면 그대로 반환하고, 다르면(또는
  캐시가 없으면) `get_tags_raw`를 다시 돌려 갱신한다 — 즉 내용 해시가 아니라
  **mtime 비교**로 무효화한다.

## 2. 심볼 랭킹(그래프 기반) 알고리즘

**소스 확인** (`get_ranked_tags`, 365-574행). `networkx`의 `MultiDiGraph`를 쓴다.

- **노드는 파일**이다(심볼이 아니라 `rel_fname`). `defines: ident -> {rel_fname 집합}`,
  `references: ident -> [rel_fname 리스트]`(중복 허용 — 참조 횟수 집계용),
  `definitions: (rel_fname, ident) -> {Tag 집합}`을 전체 파일에 대해 먼저 채운다.
  전체 저장소에 참조가 단 하나도 없으면(`.scm`이 극단적으로 def-only인 경우)
  `references`를 `defines`로 통째로 대체하는 폴백이 있다.
- **엣지**: `idents = defines.keys() ∩ references.keys()`(정의도 되고 참조도 되는
  식별자만) 각각에 대해, 참조 파일→정의 파일 방향 엣지를 추가한다. 참조가 전혀
  없는 정의 식별자는 정의 파일에 대한 self-edge(weight=0.1)를 하나씩 추가한다
  — 주석에 따르면 tree-sitter 0.23.2의 Ruby 문법이 `def greet(name)`을 def이자
  ref로 동시에 잡지 못하는 버전 차이를 흡수하기 위한 보정.
- **엣지 가중치 배율(mul)**은 식별자별로 계산한다(487-499행):
  - `mentioned_idents`(대화 중 언급된 식별자)에 포함되면 ×10.
  - snake_case/kebab-case/camelCase처럼 "제대로 된 이름" 형태고 길이 ≥ 8이면
    ×10(짧고 흔한 이름보다 의미 있는 긴 식별자를 우대).
  - `_`로 시작하면 ×0.1(관례상 private).
  - 5개 넘는 파일이 같은 이름을 정의하면(`new`, `run` 같은 흔한 이름) ×0.1.
  - 참조 파일이 **현재 채팅에 추가된 파일**(`chat_rel_fnames`)이면 추가로 ×50 —
    "지금 편집 중인 파일이 실제로 호출/참조하는 대상"에 압도적 가중치를 준다.
  - 참조 횟수는 `sqrt(num_refs)`로 눌러서, 한 파일이 같은 심볼을 100번 참조해도
    선형으로 폭주하지 않게 한다.
- **개인화(personalization) 벡터** — PageRank의 시작 확률 분포를 균등이 아니라
  치우치게 만드는 장치(422-445행). `personalize = 100 / len(전체 파일 수)`을
  단위로, 파일별로 다음을 합산한다: 채팅에 추가된 파일이면 +1단위; 대화에서
  파일명이 언급됐으면(`mentioned_fnames`) 최댓값으로 1단위(중복 가산 방지);
  경로의 각 구성요소(디렉터리명·확장자 포함/제외 파일명)가 `mentioned_idents`와
  겹치면 +1단위(한 번만). 이 값이 0보다 큰 파일만 `personalization` dict에
  들어가고, 없는 파일은 networkx 기본값(`1/노드수`)을 쓴다.
- `nx.pagerank(G, weight="weight", personalization=..., dangling=...)`으로
  **파일 단위** PageRank를 계산한다(ZeroDivisionError 시 개인화 없이 재시도,
  그래도 실패하면 빈 리스트 반환).
- **파일 랭크 → (파일,식별자) 랭크로 분배**: 각 파일 노드의 PageRank 점수를
  그 파일이 뻗은 out-edge 가중치 비율대로 쪼개, 도착 파일·식별자 쌍
  `(dst, ident)`에 누적한다(533-545행). 즉 최종적으로 랭킹되는 단위는
  "이 파일의 이 심볼"이지, 파일 전체가 아니다.
- 랭킹된 `(fname, ident)`를 점수 내림차순으로 정렬해 `ranked_tags` 리스트를
  만들되, **채팅에 이미 추가된 파일은 제외**한다(모델이 이미 전체 내용을 보고
  있으므로 맵에 중복 노출하지 않음). 태그가 하나도 안 걸린 나머지 파일들은
  파일 단위 PageRank 순서로, 그마저 없는 파일은 원래 순서로 뒤에 덧붙인다.

## 3. 토큰 예산 맞춤 로직

**소스 확인** (`get_repo_map`/`get_ranked_tags_map`/`get_ranked_tags_map_uncached`,
103-167행·576-706행).

- 기본 예산은 `map_tokens=1024`(CLI `--map-tokens`, 문서에서 "0이면 비활성화"로
  확인). **채팅에 파일이 하나도 없으면** 예산이 커진다: `map_mul_no_files` 배수와
  `max_context_window - 4096(padding)` 중 작은 쪽을 목표로 잡아 예산을 임시
  확대한다(122-132행) — "아직 뭘 볼지 안 정했을 때는 저장소 개관을 더 넓게
  보여준다"는 의도. **주의**: 소스의 생성자 기본값은 `map_mul_no_files=8`인데,
  공식 CLI 옵션 문서(`--map-multiplier-no-files`)가 명시하는 기본값은 **2**다 —
  둘 다 직접 확인했고 불일치를 그대로 남긴다(CLI 계층에서 재정의하거나 문서가
  구버전일 가능성; 이 노트에서 원인까지 확정하진 않았다).
- **토큰 카운트 추정**(`token_count`, 89-101행): 텍스트가 200자 미만이면 실제
  모델 토크나이저를 바로 호출한다. 그 이상이면 전체를 토크나이즈하지 않고
  `num_lines // 100`줄 간격으로 샘플링한 뒤(대략 100줄 샘플) 그 샘플만
  토크나이즈해 `sample_tokens / len(sample_text) * len(전체 텍스트)`로
  선형 외삽한다 — 이진 탐색 루프 안에서 반복 호출되므로 정확도보다 속도를
  택한 근사.
- **태그 개수에 대한 이진 탐색**(`get_ranked_tags_map_uncached`, 666-706행):
  랭킹된 태그 리스트(길이 `num_tags`)에서 상위 몇 개(`middle`)를 잘라 트리로
  렌더링하고 토큰 수를 재는 과정을 반복한다. 초기 `middle`은
  `max_map_tokens // 25`(태그당 평균 25토큰이라는 경험적 추정)와 `num_tags`
  중 작은 값. 매 반복에서 `pct_err = |실측 - 목표| / 목표`를 계산해, 예산 이하이면서
  지금까지 중 가장 큰 트리이거나 **오차 15% 이내**(`ok_err=0.15`)면
  `best_tree`로 채택하고, 15% 이내면 그 자리에서 즉시 종료한다(정확한 최적점을
  끝까지 찾지 않고 "충분히 가까우면 멈춘다"). 아니면 `lower_bound`/
  `upper_bound`를 갱신하며 표준 이진 탐색을 이어간다.
- **렌더링**(`to_tree`/`render_tree`, 708-784행): 태그를 파일별로 묶어 "관심
  줄(lines of interest)" 집합을 만들고, `grep_ast`의 `TreeContext`로 그 줄
  주변만 남기고 나머지는 생략(`...`) 부호로 축약한 소스 스니펫을 만든다 —
  단순 "심볼명 한 줄 나열"이 아니라 실제 시그니처+주변 문맥이 보이는 형태다.
  `render_tree` 결과는 `(rel_fname, 정렬된 lois, mtime)` 키로 `tree_cache`에
  캐싱되고(이진 탐색 반복 중 같은 파일이 여러 `middle` 값에 걸쳐 재사용됨),
  `TreeContext` 객체 자체도 파일별로 mtime 기준 캐싱된다. 한 줄은 100자에서
  잘라 미니파이된/생성된 코드가 맵을 망치지 않게 막는다. 채팅에 이미 추가된
  파일은 여기서도 건너뛴다.
- **갱신(refresh) 전략**(`get_ranked_tags_map`, 592-627행, CLI
  `--map-refresh`, 기본 `auto`): 캐시 키는 정렬된 `chat_fnames`/`other_fnames`/
  `max_map_tokens`이고, `auto` 모드에서만 `mentioned_fnames`/`mentioned_idents`도
  키에 포함된다. `manual`은 `force_refresh`가 없는 한 무조건 `last_map`을
  재사용(사용자가 명시적으로 갱신을 요청할 때만 재계산). `always`는 매번
  재계산. `files`는 파일 목록·토큰 예산이 같으면 언급된 식별자가 바뀌어도
  캐시를 그대로 씀(재랭킹 안 함, 더 저렴). `auto`는 직전 계산이 1초 넘게
  걸렸을 때만(`map_processing_time > 1.0`) 캐시를 쓴다 — 저장소가 작아
  계산이 빠르면 매번 새로 계산하고, 크고 느린 저장소에서만 캐시 재사용으로
  전환한다.

## 4. loco에의 시사점

**제약 조건**: loco는 스펙(`docs/superpowers/specs/2026-07-02-loco-design.md`)이
의존성 목록을 고정한다 — `CLAUDE.md` "Hard constraints": *"Dependency list is
fixed by the spec — ask the user before adding any crate."* 현재 `Cargo.toml`에는
`tree-sitter`도 `syn`도 없다(`regex 1.12.4`만 파싱류로 존재). aider의 접근을
그대로 이식하려면 `tree-sitter` 코어 크레이트 + 언어별 문법 크레이트(최소
`tree-sitter-rust`) + `.scm` 쿼리 파일 번들이 필요한데, 이건 전부 **사용자 승인
없이 추가할 수 없다**. 게다가 loco는 오프라인/폐쇄망 배포가 북극성이므로,
tree-sitter를 쓰더라도 파이썬 생태계처럼 PyPI에서 프리컴파일 언어 팩을 받는 게
아니라 크레이트를 정적으로 링크해야 하고, Rust 외 언어(JS/Python/Go 등)까지
지원하려면 문법 크레이트가 언어 수만큼 늘어난다 — M8 픽스처(`tasks-large/`)가
전부 Rust 워크스페이스인 점을 감안하면 1차 범위는 Rust 문법 하나로 좁힐 수 있다.

**tree-sitter 없이 흉내 내는 축소판(degraded 버전)**: `syn`(Rust용 파서 크레이트,
이것도 스펙에 없어 승인 필요)이나, 크레이트 추가 없이 기존 `regex` 의존성만으로
`fn `/`struct `/`enum `/`pub use ` 같은 패턴을 줄 단위로 긁는 방식이 있다. 후자는
tree-sitter의 AST 기반 def/ref 구분과 달리 오탐(문자열·주석 안의 `fn ` 등)에
취약하지만, 이미 `edit_file`의 검색 사다리나 `grep`처럼 loco 도구층이 "완벽하진
않지만 실용적인 정규식 기반 근사"를 택해온 선례와 결이 맞는다. 최소 버전은
def만 뽑고(`pub fn`, `pub struct` 등 최상위 시그니처), ref 카운트는 아예 생략한
채 "정의된 심볼 목록 + 파일당 개수"만 보여주는 것도 고려할 만하다 — aider도
def만 있고 ref가 없는 언어(C++ 등)를 pygments 토큰 폴백으로 처리하는데,
loco라면 그 폴백 자리에 `grep`으로 식별자 이름을 저장소 전체에서 세는 것으로
대체할 수 있다(이미 `Registry`에 있는 도구 재사용, 신규 의존성 0).

**랭킹 아이디어를 지금의 depth-3/100개 트리에 적용한다면**: `project_tree()`
(`src/agent/prompt.rs` 6-7행, `TREE_DEPTH=3`/`TREE_MAX_ENTRIES=100`)는 지금
"알파벳 순 pre-order DFS로 자르기"만 한다 — 파일의 중요도를 전혀 보지 않는다.
이것이 바로 M8이 잡아낸 결함의 근본 원인이다: `inv-core/src/rules/pricing.rs`가
과제2의 정답 파일인데도 depth=4라는 이유만으로 트리에서 통째로 빠진다. 굳이
그래프 PageRank 전체를 이식하지 않아도, aider의 **개인화 벡터** 아이디어의
축소판은 크레이트 추가 없이도 적용 가능하다: 예컨대 최근 `grep`/`read_file`로
실제 열어본 파일이나 `mentioned_idents`에 해당하는(에이전트의 최근 턴에서 언급된
식별자와 경로가 겹치는) 파일을 depth 상한과 무관하게 트리에 강제 포함시키는
것 — 이는 그래프도 tree-sitter도 없이 "지금까지의 대화/도구 호출 이력"만으로
구현 가능한 개인화다. 더 완전한 형태(참조 그래프 기반 랭킹)는 tree-sitter나
`syn` 도입을 전제하므로 M9에서 실패 데이터가 우선순위를 정하기 전까지는
백로그로 남겨야 한다 — 이는 스펙 §8 백로그 항목 *"repo-map 도구·검색 강화·
컨텍스트 관리 개선 — M9, 실패 데이터가 정한다"*와 정확히 일치한다. 요약하면:
1단계(의존성 0, 즉시 가능)는 트리에 "최근 접근/언급 파일 강제 포함" 개인화만
넣는 것이고, 2단계(정규식 기반 def 추출, 의존성 0이지만 오탐 감수)는 depth 상한
자체를 없애고 대신 "정의 개수/최근성"으로 항목을 골라내는 것이며, 3단계
(tree-sitter/syn 도입, 사용자 승인 필요)가 aider와 동등한 def/ref 그래프 랭킹이다.

## 출처

- https://aider.chat/docs/repomap.html — 공식 문서, repo-map 개요·`--map-tokens`
  설명·그래프 랭킹 개요(레벨: 개괄적, tree-sitter 세부·캐싱·바이너리 서치는
  이 페이지에 없음을 직접 확인)
- https://aider.chat/2023/10/22/repomap.html — 공식 블로그, tree-sitter 채택
  배경과 `.scm` 쿼리 출처 크레딧(C/C#/C++/Elisp/Elixir/Elm/Go/Java/
  JavaScript/OCaml/PHP/Python/QL/R/Ruby/Rust/TypeScript), 지원 언어 언급
  (PageRank 수식·개인화·바이너리 서치 세부는 이 페이지에도 없음을 직접 확인 —
  이 부분은 전부 소스 코드에서만 확인했다)
- https://aider.chat/docs/config/options.html — 공식 CLI 옵션 문서,
  `--map-tokens`/`--map-refresh`/`--map-multiplier-no-files` 기본값·설명
  (`--map-multiplier-no-files` 기본값 2 — 소스 생성자 기본값 8과 불일치, 위에서
  명시)
- `aider/repomap.py`, GitHub `Aider-AI/aider`(구 `paul-gauthier/aider`) main
  브랜치, `gh api repos/Aider-AI/aider/contents/aider/repomap.py`로 2026-07-17
  직접 취득(867줄) — 이 노트의 §1~§3 세부 로직(캐시 버전·mtime 비교·
  MultiDiGraph·개인화 가중치·mul 배율·이진 탐색·TreeContext 캐싱·refresh 4모드)은
  전부 이 파일 실측
