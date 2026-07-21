# M15 실레포 표본 동결 (T21, 재실사 개정)

동결 시각: 2026-07-21. 브랜치 `m15/real-repo-track`.

## 요약

- **N = 17**
- delta: **1** (5.9%)
- fd: **6** (35.3%)
- ripgrep: **10** (58.8%)
- 편중 상한(T1 동결 60%): 최대 `ripgrep` 58.8% — **충족**
- 실격 대역: `0.98·√N = 4.0406` → **전승 ≥ 13 또는 전패 ≥ 13 이면 실격**
- `--verify`: **17/17 통과**

## 재실사 경과 (선택 1: 레포 추가)

T21 초판은 fd 6 + ripgrep 10 = N16에서 ripgrep 62.5%로 편중 상한을 넘겼다.
처분 1(레포 추가 후 재실사)에 따라 5개 CLI 레포를 bare 클론·공급 실사했다:

| 레포 | 닫힌 이슈 | 이슈연결 | 테스트동반 | proxy 후보 | 비고 |
|---|---|---|---|---|---|
| bat | 1298 | 194 | 23 | 22 | 조달: 서브모듈 gitlink + 비ASCII 경로로 가드 충돌 |
| hyperfine | 227 | 56 | 4 | 2 | #684 테스트 수정만(새 fn 없음) |
| **delta** | 622 | 175 | 23 | 3 | **1과제 채택** (`delta-1089`) |
| sd | 134 | 17 | 4 | 1 | Cargo.toml/lock 전용 |
| dust | 238 | 1 | 0 | 0 | 공급 0 |

조달 스크립트 수정: `scripts/procure_real.sh` export-ignore 가드에서
**gitlink(mode 160000) 제외** — `git archive`가 서브모듈을 안 넣는데 서브모듈 경로를
export-ignore로 오인하던 거짓 bail(bat 실측). 기존 fd/ripgrep 캐시 히트 회귀 확인.

## 제외 기록

- `rg-99-empty-lines`: convention4_compile_fail — rustc-serialize E0310/E0642 on current rustc; ancient ripgrep parent unbuildable
- `fd-1727-ignore-contain`: issue_commit_misalign — issue asked --prune-parent for repo-finding; commit implements unrelated --ignore-contain (CACHEDIR.TAG). Issue author later said it does not solve original request.
- `rg-2664-sortr-path`: convention6_named — panic at crates/core/flags/hiargs.rs:todo! — oracle path in failure report
- `rg-693-contextless-sep`: discriminability_fail — check passed on parent+overlay without solution (rc=0)
- `fd-250-invalid-utf8`: solvability_platform — macOS rejects invalid UTF-8 path creation (errno 92)
- `fd-89-excludes`: solvability_toolchain — old ignore crate panics on modern rustc (uninit Message)
- `rg-483-quiet-files`: solvability_toolchain — old ignore crate panics on modern rustc (uninit Message)

## 과제 표

| task | repo | issue | fix | parent | check | protected | oracle | nav | 지목 |
|---|---|---|---|---|---|---|---|---|---|
| delta-1089-whole-file-commit | delta | [1089](https://github.com/dandavison/delta/issues/1089) | `bd54a51205be` | `e28e97de7aa0` | `cargo test test_file_removal_in_log_output` | src/tests/test_example_diffs.rs | src/handlers/commit_meta.rs | 단축 안 됨 | 지목되지 않음 |
| fd-1873-path-sep | fd | [1873](https://github.com/sharkdp/fd/issues/1873) | `ed4766419152` | `90e73d72df25` | `cargo test --test tests test_pattern_with_forward_slash_is_rejected` | tests/tests.rs | src/main.rs | 단축 안 됨 | 지목되지 않음 |
| fd-404-min-exact-depth | fd | [404](https://github.com/sharkdp/fd/issues/404) | `d63c63be8cf8` | `47974b647959` | `cargo test --test tests test_min_depth` | tests/tests.rs | src/app.rs, src/main.rs, src/options.rs, src/walk.rs | 단축 안 됨 | 지목되지 않음 |
| fd-535-prune | fd | [535](https://github.com/sharkdp/fd/issues/535) | `ec4cc981fcf4` | `06eb231fbd64` | `cargo test --test tests test_prune` | tests/tests.rs | src/app.rs, src/main.rs, src/options.rs, src/walk.rs | 단축 안 됨 | 지목되지 않음 |
| fd-615-hidden-dot-pattern | fd | [615](https://github.com/sharkdp/fd/issues/615) | `cadaef3f076f` | `17bd256ae6e4` | `cargo test --test tests test_error_if_hidden_not_set_and_pattern_starts_with_dot` | tests/tests.rs | src/main.rs, src/regex_helper.rs | 단축 안 됨 | 지목되지 않음 |
| fd-675-number-parse-error | fd | [675](https://github.com/sharkdp/fd/issues/675) | `e0adb45d082d` | `ec4cc981fcf4` | `cargo test --test tests test_number_parsing_errors` | tests/testenv/mod.rs, tests/tests.rs | src/main.rs | 단축 안 됨 | 지목되지 않음 |
| fd-898-strip-cwd-exec | fd | [898](https://github.com/sharkdp/fd/issues/898) | `4ffc34956f9a` | `5039d2db9914` | `cargo test --test tests test_exec` | tests/tests.rs | src/app.rs, src/dir_entry.rs, src/exec/job.rs, src/main.rs, src/output.rs, src/walk.rs | 단축 안 됨 | 지목되지 않음 |
| rg-1138-no-ignore-dot | ripgrep | [1138](https://github.com/BurntSushi/ripgrep/issues/1138) | `12a6ca45f9da` | `9d703110cfe0` | `cargo test --test integration f1138_no_ignore_dot` | tests/feature.rs | src/app.rs, src/args.rs | 단축 안 됨 | 지목되지 않음 |
| rg-1159-exit-status | ripgrep | [1159](https://github.com/BurntSushi/ripgrep/issues/1159) | `f3164f2615ce` | `31d3e241306f` | `cargo test --test integration r1159` | tests/regression.rs | src/args.rs, src/main.rs, src/messages.rs, src/subject.rs | 단축 안 됨 | 지목되지 않음 |
| rg-1176-fixed-strings-file | ripgrep | [1176](https://github.com/BurntSushi/ripgrep/issues/1176) | `0df71240ff19` | `f3164f2615ce` | `cargo test --test integration r1176` | tests/regression.rs | src/args.rs | 단축됨 | 지목되지 않음 |
| rg-1293-glob-case-insensitive | ripgrep | [1293](https://github.com/BurntSushi/ripgrep/issues/1293) | `c2cb0a4de459` | `adb9332f52b8` | `cargo test --test integration glob_always_case_insensitive` | tests/misc.rs | src/app.rs, src/args.rs | 단축 안 됨 | 지목되지 않음 |
| rg-1390-no-context-sep | ripgrep | [1390](https://github.com/BurntSushi/ripgrep/issues/1390) | `e71eedf0eb80` | `88f46d12f1f3` | `cargo test --test integration no_context_sep` | tests/feature.rs | src/app.rs, src/args.rs | 단축 안 됨 | 지목되지 않음 |
| rg-1420-no-ignore-exclude | ripgrep | [1420](https://github.com/BurntSushi/ripgrep/issues/1420) | `297b428c8c92` | `804b43ecd8bd` | `cargo test --test integration f1420` | tests/feature.rs | src/app.rs, src/args.rs | 단축됨 | 지목되지 않음 |
| rg-1466-no-ignore-files | ripgrep | [1466](https://github.com/BurntSushi/ripgrep/issues/1466) | `c4c43c733ee9` | `447506ebe02f` | `cargo test --test integration f1466_no_ignore_files` | tests/feature.rs | crates/core/app.rs, crates/core/args.rs | 단축 안 됨 | 지목되지 않음 |
| rg-1868-passthru-context | ripgrep | [1868](https://github.com/BurntSushi/ripgrep/issues/1868) | `a77b914e7ac9` | `2e2af50a4df0` | `cargo test --test integration r1868_context_passthru_override` | tests/regression.rs | crates/core/app.rs | 단축 안 됨 | 지목되지 않음 |
| rg-568-leading-hyphen | ripgrep | [568](https://github.com/BurntSushi/ripgrep/issues/568) | `6dce04963d4e` | `d4b790fd8d97` | `cargo test --test integration regression_568_leading_hyphen_option_arguments` | tests/tests.rs | src/app.rs | 단축됨 | 지목되지 않음 |
| rg-740-passthru | ripgrep | [740](https://github.com/BurntSushi/ripgrep/issues/740) | `58bdc366ec29` | `34c0b1bc709f` | `cargo test --test integration feature_740_passthru` | tests/tests.rs | src/app.rs, src/args.rs | 단축 안 됨 | 지목되지 않음 |

## 감사 원 출력

`docs/experiments/2026-07-20-m15-real-repo-baseline/audit/<task>-check.txt`

## 컨트롤러 편향 고지 (§10-4)

diff 가르기·이슈↔커밋 정합·대체/추가 과제 선택은 컨트롤러 판단. 자동화 게이트가 3항 감사를 대체하지 않는다.
