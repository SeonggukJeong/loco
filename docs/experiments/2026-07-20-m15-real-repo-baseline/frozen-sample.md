# M15 실레포 표본 동결 (T21)

동결 시각: 2026-07-21. 브랜치 `m15/real-repo-track`.

## 요약

- **N = 16**
- 레포 분포: fd **6** (37.5%) · ripgrep **10** (62.5%)
- 편중 상한(T1 동결 60%): ripgrep 62.5% — **초과 (A1b 위반 후보)**
- 실격 대역 절대값: `0.98·√N = 3.9200` → **전승 ≥ 13 또는 전패 ≥ 13 이면 실격**
- `--verify`: **16/16 통과** (재조달 후 재확인)
- 캐시 비운 재조달 매니페스트·심링크: **전 항목 일치**
- 스테일 뮤테이션 수동: audit/stale-mutation.json (fd·rg 각 1과제 after_age=pass)

## 편중·최소표본 메모 (사전등록 T23 입력)

선정 표본 ripgrep 비중이 60% 상한을 넘긴다(62.5%). fd 쪽 추가 과제가
플랫폼(무효 UTF-8 경로)·툴체인(구 ignore 패닉)·이슈↔커밋 불일치·help-text 전용 등으로
소진돼, 상한을 지키면서 N≥16을 동시에 만족하는 조합이 없다.
**T23 사전등록 전 사용자 처분 필요**(T1 허용 3항: 레포 추가 재실사 / M15 연기 / 확보 수로 진행하되 A1b 실패 보고).

## 제외 기록

- `rg-99-empty-lines`: convention4_compile_fail — rustc-serialize E0310/E0642 on current rustc; ancient ripgrep parent unbuildable
- `fd-1727-ignore-contain`: issue_commit_misalign — issue asked --prune-parent for repo-finding; commit implements unrelated --ignore-contain (CACHEDIR.TAG). Issue author later said it does not solve original request.
- `rg-2664-sortr-path`: convention6_named — panic at crates/core/flags/hiargs.rs:todo! — oracle path in failure report
- `rg-693-contextless-sep`: discriminability_fail — check passed on parent+overlay without solution (rc=0)
- `fd-250-invalid-utf8`: solvability_platform — macOS rejects invalid UTF-8 path creation (errno 92)
- `fd-89-excludes`: solvability_toolchain — old ignore crate panics on modern rustc (uninit Message)
- `rg-483-quiet-files`: solvability_toolchain — old ignore crate panics on modern rustc (uninit Message)

## 과제 표

| task | repo | issue | fix_sha | parent_sha | check | protected | oracle | nav | 지목 |
|---|---|---|---|---|---|---|---|---|---|
| fd-1873-path-sep | fd | [1873](https://github.com/sharkdp/fd/issues/1873) | `ed4766419152` | `90e73d72df25` | `cargo test --test tests test_pattern_with_forward_slash_is_rejected` | tests/tests.rs | src/main.rs | 단축 안 됨 | 지목되지 않음 |
| fd-404-min-exact-depth | fd | [404](https://github.com/sharkdp/fd/issues/404) | `d63c63be8cf8` | `47974b647959` | `cargo test --test tests test_min_depth` | tests/tests.rs | src/app.rs, src/main.rs, src/options.rs, src/walk.rs | 단축 안 됨 | 지목되지 않음 |
| fd-535-prune | fd | [535](https://github.com/sharkdp/fd/issues/535) | `ec4cc981fcf4` | `06eb231fbd64` | `cargo test --test tests test_prune` | tests/tests.rs | src/app.rs, src/main.rs, src/options.rs, src/walk.rs | 단축 안 됨 | 지목되지 않음 |
| fd-615-hidden-dot-pattern | fd | [615](https://github.com/sharkdp/fd/issues/615) | `cadaef3f076f` | `17bd256ae6e4` | `cargo test --test tests test_error_if_hidden_not_set_and_pattern_starts_with_dot` | tests/tests.rs | src/main.rs, src/regex_helper.rs | 단축 안 됨 | 지목되지 않음 |
| fd-675-number-parse-error | fd | [675](https://github.com/sharkdp/fd/issues/675) | `e0adb45d082d` | `ec4cc981fcf4` | `cargo test --test tests test_number_parsing_errors` | tests/testenv/mod.rs, tests/tests.rs | src/main.rs | 단축 안 됨 | 지목되지 않음 |
| fd-898-strip-cwd-exec | fd | [898](https://github.com/sharkdp/fd/issues/898) | `4ffc34956f9a` | `5039d2db9914` | `cargo test --test tests test_exec` | tests/tests.rs | src/app.rs, src/dir_entry.rs, src/exec/job.rs, src/main.rs, src/output.rs, src/walk.rs | 단축 안 됨 | 지목되지 않음 |
| rg-1138-no-ignore-dot | ripgrep | [1138](https://github.com/BurntSushi/ripgrep/issues/1138) | `12a6ca45f9da` | `9d703110cfe0` | `cargo test --test integration f1138_no_ignore_dot` | tests/feature.rs | src/app.rs, src/args.rs | 단축 안 됨 | 지목되지 않음 (verify 통과 후 재조달; 원 출력 audit/<task>-check.txt 참조) |
| rg-1159-exit-status | ripgrep | [1159](https://github.com/BurntSushi/ripgrep/issues/1159) | `f3164f2615ce` | `31d3e241306f` | `cargo test --test integration r1159` | tests/regression.rs | src/args.rs, src/main.rs, src/messages.rs, src/subject.rs | 단축 안 됨 | 지목되지 않음 |
| rg-1176-fixed-strings-file | ripgrep | [1176](https://github.com/BurntSushi/ripgrep/issues/1176) | `0df71240ff19` | `f3164f2615ce` | `cargo test --test integration r1176` | tests/regression.rs | src/args.rs | 단축됨 | 지목되지 않음 |
| rg-1293-glob-case-insensitive | ripgrep | [1293](https://github.com/BurntSushi/ripgrep/issues/1293) | `c2cb0a4de459` | `adb9332f52b8` | `cargo test --test integration glob_always_case_insensitive` | tests/misc.rs | src/app.rs, src/args.rs | 단축 안 됨 | 지목되지 않음 (verify 통과 후 재조달; 원 출력 audit/<task>-check.txt 참조) |
| rg-1390-no-context-sep | ripgrep | [1390](https://github.com/BurntSushi/ripgrep/issues/1390) | `e71eedf0eb80` | `88f46d12f1f3` | `cargo test --test integration no_context_sep` | tests/feature.rs | src/app.rs, src/args.rs | 단축 안 됨 | 지목되지 않음 (verify 통과 후 재조달; 원 출력 audit/<task>-check.txt 참조) |
| rg-1420-no-ignore-exclude | ripgrep | [1420](https://github.com/BurntSushi/ripgrep/issues/1420) | `297b428c8c92` | `804b43ecd8bd` | `cargo test --test integration f1420` | tests/feature.rs | src/app.rs, src/args.rs | 단축됨 | 지목되지 않음 (verify 통과 후 재조달; 원 출력 audit/<task>-check.txt 참조) |
| rg-1466-no-ignore-files | ripgrep | [1466](https://github.com/BurntSushi/ripgrep/issues/1466) | `c4c43c733ee9` | `447506ebe02f` | `cargo test --test integration f1466_no_ignore_files` | tests/feature.rs | crates/core/app.rs, crates/core/args.rs | 단축 안 됨 | 지목되지 않음 (verify 통과 후 재조달; 원 출력 audit/<task>-check.txt 참조) |
| rg-1868-passthru-context | ripgrep | [1868](https://github.com/BurntSushi/ripgrep/issues/1868) | `a77b914e7ac9` | `2e2af50a4df0` | `cargo test --test integration r1868_context_passthru_override` | tests/regression.rs | crates/core/app.rs | 단축 안 됨 | 지목되지 않음 |
| rg-568-leading-hyphen | ripgrep | [568](https://github.com/BurntSushi/ripgrep/issues/568) | `6dce04963d4e` | `d4b790fd8d97` | `cargo test --test integration regression_568_leading_hyphen_option_arguments` | tests/tests.rs | src/app.rs | 단축됨 | 지목되지 않음 |
| rg-740-passthru | ripgrep | [740](https://github.com/BurntSushi/ripgrep/issues/740) | `58bdc366ec29` | `34c0b1bc709f` | `cargo test --test integration feature_740_passthru` | tests/tests.rs | src/app.rs, src/args.rs | 단축 안 됨 | 지목되지 않음 |

## 감사 원 출력

과제별 `check` 캡처: `docs/experiments/2026-07-20-m15-real-repo-baseline/audit/<task>-check.txt`
추출기: `scripts/leak_audit.py`. gitattributes: 전 과제 parent_sha에서 export-ignore/subst **0건**.

## 컨트롤러 편향 고지 (§10-4)

diff를 테스트/비테스트로 가르는 판단·이슈↔커밋 정합·대체 과제 선택은 컨트롤러가 수행했다.
자동화 게이트가 이 3항을 대체하지 않는다. 이 편향은 리포트에 남긴다.
