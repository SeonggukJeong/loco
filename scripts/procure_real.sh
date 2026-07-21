#!/bin/sh
# tasks-real 픽스처 조달 (M15 H4·H8·§3-5).
#
# 하네스 밖의 **명시 단계**다 — run_eval/run_verify 안에 넣으면 게이트가 네트워크
# 의존이 되고 기존 두 트리에도 걸린다(H4). eval/--verify는 픽스처가 없으면
# task.rs:61-63의 bail!로 명확히 실패한다.
#
# .git을 샌드박스에 넣지 않는 것이 핵심이다: eval은 AutoApprover로 돌고
# auto_deny_patterns 기본 11종 중 git 계열은 push뿐이라 `git show <fix-sha>`로
# 정답 열람이 가능하다. 그래서 `git archive`로 트리만 뽑는다.
#
# usage:
#   LOCO_REAL_REPOS=~/loco-real-repos \
#   LOCO_TASKS_REAL_CACHE=~/loco-tasks-real-cache \
#     scripts/procure_real.sh tasks-real/<task-dir> [...]
#   scripts/procure_real.sh --all tasks-real
#
# ⚠ 이 머신에 timeout/gtimeout이 없다 — 쓰면 rc=127로 조용히 무동작한다(§10-7).
set -eu

: "${LOCO_REAL_REPOS:?pristine bare 클론들이 있는 디렉터리를 지정하세요}"
: "${LOCO_TASKS_REAL_CACHE:?캐시 디렉터리를 지정하세요 (레포 밖)}"

SHA_CMD=""
if command -v shasum >/dev/null 2>&1; then SHA_CMD="shasum -a 256"
elif command -v sha256sum >/dev/null 2>&1; then SHA_CMD="sha256sum"
else echo "sha256 도구가 없습니다 (shasum/sha256sum)" >&2; exit 1
fi

# 트리의 일반 파일 매니페스트(경로\t크기\tSHA-256)를 stdout으로.
# ⚠ 파일당 포크를 쓰지 않는다 — 개정 4는 `wc`/`shasum`/`cut`/`tr`로 **파일당 4포크**를
# 돌아 N=2000에서 19.8초였다(`xargs shasum` 0.07초, 280배 — 4R 실측).
# 4레포 추적 파일은 총 672개(zoxide 72·fd 59·ripgrep 223·just 318)라 절대 시간은
# 작지만, 캐시 히트마다 도는 경로이므로 포크를 없앤다
manifest_of() {
  # ⚠ `xargs -a`는 GNU/util-linux 확장이라 BSD xargs(macOS)에 없다(rc=1,
  # "invalid option -- a") — 실측으로 드러난 버그. 두 구현 모두 지원하는
  # **표준입력 리다이렉트**로 파일 목록을 넘긴다
  ( cd "$1" && find . -type f | sed 's|^\./||' | LC_ALL=C sort > /tmp/m15-files.$$
    # 크기: find -printf가 BSD에 없으므로 stat. 한 번의 호출로 전부
    xargs stat -f '%z' 2>/dev/null < /tmp/m15-files.$$ > /tmp/m15-sizes.$$ \
      || xargs stat -c '%s' < /tmp/m15-files.$$ > /tmp/m15-sizes.$$
    xargs $SHA_CMD < /tmp/m15-files.$$ | awk '{print $1}' > /tmp/m15-hashes.$$
    paste /tmp/m15-files.$$ /tmp/m15-sizes.$$ /tmp/m15-hashes.$$
    rm -f /tmp/m15-files.$$ /tmp/m15-sizes.$$ /tmp/m15-hashes.$$ )
}

# 캐시 채우기 — <cache>/<repo>/<sha>/ 는 **읽기 전용 원본**이다.
# 멱등: .complete 마커 기반. 실패 시 부분 디렉터리를 지운다
fill_cache() {
  repo="$1"; sha="$2"
  git_dir="$LOCO_REAL_REPOS/$repo.git"
  dest="$LOCO_TASKS_REAL_CACHE/$repo/$sha"
  # ⚠ **히트 시 매니페스트를 대조한다** — 스펙 §3-5 조달 계약이
  # *"매니페스트를 남기고 **히트 시 대조**"*를 명시하는데, 개정 3까지는 `.complete`만
  # 보고 즉시 반환해 **대조 경로가 아예 없었다**(3R 실현 I3). §9-A2는 **캐시를 비운**
  # 재조달의 매니페스트끼리만 비교하므로 이 구멍을 안 덮는다 — 레포 밖에 오래 사는
  # 캐시가 배치 사이에 손상·변조돼도 아무도 못 본다.
  # ⚠ 이것은 **자기정합 검사이지 업스트림 검증이 아니다**(§10-5)
  if [ -f "$dest/meta/.complete" ]; then
    now=$(manifest_of "$dest/tree")
    if [ "$now" != "$(cat "$dest/meta/manifest.tsv")" ]; then
      echo "캐시 매니페스트 불일치 — 손상 또는 변조: $dest" >&2
      echo "  (의도적 재조달이면 그 디렉터리를 지우고 다시 실행하세요)" >&2
      exit 1
    fi
    # ⚠⚠ **심링크와 경로 집합도 함께 본다**(4R 실현 축1 C1). 매니페스트는
    # **일반 파일만** 담으므로 그것만 대조하면 심링크 재지정·삭제·추가가 전부
    # 통과한다 — 실측: `ln -s /etc/passwd tree/pwn.txt`가 "손상 또는 변조" 검사를
    # 지나 `<task_dir>/fixture`로 tar됐다. 일반 파일 변조는 6종 변형 전부 잡히는데
    # 심링크만 전부 샜다.
    # ⚠ **이것이 "검사를 추가하면서 그 검사가 못 보는 것을 안 센" 형태다** —
    # 3R에서 같은 지적을 받고 개정 4에서 재발했다. 그래서 개정 5부터는
    # **새 검사마다 "이것이 못 보는 것"을 주석으로 명시한다**(아래 참조).
    if [ "$(cd "$dest/tree" && find . -type l | sed 's|^\./||' | LC_ALL=C sort)" \
         != "$(cat "$dest/meta/symlinks.txt")" ] \
    || [ "$(cd "$dest/tree" && find . \( -type f -o -type l \) | sed 's|^\./||' | LC_ALL=C sort)" \
         != "$(cat "$dest/meta/extracted")" ]; then
      echo "캐시 심링크/경로 집합 불일치 — 손상 또는 변조: $dest" >&2
      exit 1
    fi
    # ⚠ **이 대조가 못 보는 것**: 디렉터리 퍼미션·mtime·빈 디렉터리 추가.
    #    셋 다 판정에 도달하지 않는다(check는 파일 내용만 읽고, copy_tree가
    #    mtime을 새로 쓴다). 심링크 **대상 문자열**은 symlinks.txt가 경로만
    #    담으므로 못 본다 — 재지정 자체는 위 extracted 대조가 잡지 못하나,
    #    H5가 심링크를 샌드박스 진입 전에 스킵하므로 판정 영향이 없다
    echo "  캐시 히트(매니페스트 일치): $repo/$sha"
    return 0
  fi

  [ -d "$git_dir" ] || { echo "pristine 클론이 없습니다: $git_dir" >&2; exit 1; }

  # shallow 경계 검증 — 4레포 전부 shallow라 부모 트리가 없을 수 있다.
  # unshallow는 공짜다(4레포 합 33MB·6.2초)이므로 여기서 즉시 고친다
  if [ -f "$git_dir/shallow" ]; then
    echo "  unshallow: $repo"
    git -C "$git_dir" fetch --unshallow origin || git -C "$git_dir" fetch --depth=2147483647 origin
  fi
  git -C "$git_dir" rev-parse --verify "$sha^{commit}" >/dev/null \
    || { echo "SHA가 업스트림에 없습니다: $repo $sha" >&2; exit 1; }

  # ⚠⚠ **메타 파일은 추출 트리 안에 두지 않는다**(2R Critical 2·3, Important 7).
  # 개정 2는 `<sha>/` 하나에 추출물과 메타(manifest.tsv·symlinks.txt·.complete…)를
  # 섞고 `not_meta()` 이름 필터로 갈랐는데, 그 설계가 결함 셋을 한꺼번에 낳았다:
  #   ① 필터에 `.files`를 안 넣어 매니페스트가 **자기 자신을 셌다**(3파일/2파일).
  #      게다가 결정적이라 §9-A2의 재조달 매니페스트 대조가 **통과하면서 틀린다**
  #   ② 픽스처 실체화의 `tar --exclude`가 libarchive에서 **basename 매칭**이라,
  #      레포 어디에 있든 `manifest.tsv`·`symlinks.txt` 같은 이름의 **진짜 파일이
  #      조용히 사라졌다**(exit 0, 매니페스트에는 남아 아무도 못 본다).
  #      컨트롤러 실측: `--exclude=manifest.tsv`가 모든 깊이의 동명 파일을 제거,
  #      앵커 형태(`--exclude=./x`·`--exclude=/x`)도 동일
  #   ③ 루트에 `manifest.tsv`를 가진 정상 레포가 export-ignore 오탐으로 조달 불가
  # **트리와 메타를 형제 디렉터리로 분리하면 셋이 동시에 사라진다** — 이름 필터도,
  # tar 제외도 필요 없어진다
  tree="$dest/tree"; meta="$dest/meta"
  rm -rf "$dest"; mkdir -p "$tree" "$meta"
  trap 'rm -rf "$dest"' EXIT INT TERM
  # ⚠ 파이프 실패 검출: #!/bin/sh라 pipefail이 없다(dash). git archive가 죽어도
  # tar가 0으로 끝나면 조용히 통과하고, 그것이 아래 export-ignore 가드에서
  # "의심"으로 잘못 표면화된다(1R 실현 I7). 아카이브를 **파일로 먼저 받아** 종료
  # 코드를 직접 본다 — 4레포 최대 트리가 수 MB라 임시 파일 비용은 무시할 만하다
  if ! git -C "$git_dir" archive "$sha" > "$meta/archive.tar"; then
    echo "git archive 실패: $repo $sha" >&2; exit 1
  fi
  tar -xf "$meta/archive.tar" -C "$tree"
  rm -f "$meta/archive.tar"

  # 심링크 목록 — H5의 스킵 대상이고, 아래 가드가 이 목록을 **포함해** 비교한다
  ( cd "$tree" && find . -type l | sed 's|^\./||' | LC_ALL=C sort ) > "$meta/symlinks.txt"

  # export-ignore/export-subst 가드 (§3-5). **경로 집합의 차집합**으로 본다 —
  # 파일 *수* 대조는 gitlink에서 거짓 bail이 나고 export-subst를 못 잡는다.
  # ⚠ export-subst는 파일 수도 경로도 안 바꾸므로 이 자동 가드로는 안 잡힌다.
  #    그 몫은 §3-4-2의 사람 감사다
  #
  # ⚠⚠ **심링크를 포함해 비교하는 것이 계약이다**(1R 실현 C4). `git ls-tree -r`는
  # 심링크를 blob으로 실어 주는데 `find -type f`는 심링크를 빼므로, 포함하지 않고
  # 비교하면 **export-ignore가 0건이어도 모든 심링크가 "의심"으로 잡혀 exit 1**이
  # 된다. 대상 4레포 중 ripgrep(HomebrewFormula)·just(www/man/{en,zh})가 심링크를
  # 가지므로 **조달 자체가 불가능해진다** — 그리고 그것은 T3의 "심링크는 스킵한다"
  # 정책과 이 스크립트 자신의 symlinks.txt 주석과도 정면으로 모순이다
  # 비교용 ls-tree 경로 집합. ⚠ **gitlink(mode 160000, 서브모듈)는 제외한다** —
  # `git archive`는 서브모듈 내용을 펼치지 않고 gitlink 항목도 아카이브에 안 넣는다.
  # 그래서 bat 같은 서브모듈 다수 레포에서 export-ignore 0건인데도 전 서브모듈 경로가
  # "의심"으로 잡혀 조달이 죽었다(M15 T21 재실사 실측). gitlink ≠ export-ignore.
  # 심링크(120000)는 아카이브에 남으므로 그대로 포함(아래 extracted와 대칭).
  git -C "$git_dir" ls-tree -r "$sha" \
    | grep -v '^160000 ' \
    | cut -f2 \
    | LC_ALL=C sort > "$meta/tree-paths"
  # 비교용: 일반 파일 **+ 심링크**. 트리에는 메타가 없으므로 이름 필터가 불필요하다
  ( cd "$tree" && find . \( -type f -o -type l \) | sed 's|^\./||' | LC_ALL=C sort ) \
      > "$meta/extracted"
  diff_out=$(LC_ALL=C comm -23 "$meta/tree-paths" "$meta/extracted") || true
  if [ -n "$diff_out" ]; then
    echo "export-ignore 의심 — ls-tree에 있고 아카이브에 없는 경로:" >&2
    echo "$diff_out" >&2
    exit 1
  fi

  # 매니페스트 = **산출물 자체**의 파일 목록 + 크기 + SHA-256.
  # **일반 파일만** — 심링크를 넣으면 dangling(just www/man/{en,zh})에서 wc/sha256이
  # 죽는다. 그래서 비교 목록(extracted)과 매니페스트 목록의 정의가 다르다.
  # ⚠ git 트리 해시는 쓸 수 없다(캐시는 .git 없는 추출 트리다).
  # ⚠ 이것은 **자기정합 검사이지 업스트림 검증이 아니다**(§10-5)
  # ⚠ **심링크 집합은 매니페스트가 안 본다** — §9-A2의 재조달 대조는
  #    `manifest.tsv`와 `symlinks.txt`를 **둘 다** 비교해야 한다(2R 측정 m4)
  manifest_of "$tree" > "$meta/manifest.tsv"

  trap - EXIT INT TERM
  : > "$meta/.complete"
  echo "  조달 완료: $repo/$sha ($(wc -l < "$meta/manifest.tsv" | tr -d ' ')파일, 심링크 $(wc -l < "$meta/symlinks.txt" | tr -d ' ')개)"
}

procure_task() {
  task_dir="$1"
  toml="$task_dir/procure.toml"
  [ -f "$toml" ] || { echo "procure.toml이 없습니다: $task_dir" >&2; exit 1; }
  # ⚠ 줄끝 앵커(`$`)와 CR 제거가 계약이다(1R 실현 I6). 앵커가 없으면 인라인 주석
  # (`repo = "demo"  # 메모`)이 값에 새어 들고, CRLF 줄끝이면 `\r`이 값에 남는다 —
  # `\r`은 터미널에서 커서를 되돌려 **눈에 안 보이는데** 바이트에는 남아
  # `demo\r.git` 같은 오도적 에러를 낸다. 이 프로젝트는 이미 CRLF 픽스처를 다룬다
  val() { tr -d '\r' < "$toml" | sed -n "s/^$1 *= *\"\\([^\"]*\\)\" *$/\\1/p" | head -1; }
  repo=$(val repo); parent=$(val parent_sha); fix=$(val fix_sha)
  [ -n "$repo" ] && [ -n "$parent" ] && [ -n "$fix" ] \
    || { echo "procure.toml에 repo/parent_sha/fix_sha가 필요합니다: $toml" >&2; exit 1; }

  echo "[$task_dir] $repo $parent (fix $fix)"
  fill_cache "$repo" "$parent"

  # 픽스처 실체화 — H8. <task_dir>/fixture는 git-ignore다
  # ⚠ **`--exclude`를 쓰지 않는다**(2R Critical 3). libarchive의 `--exclude`는
  # **basename 매칭**이라 트리 어디에 있든 그 이름의 진짜 파일을 지운다 —
  # 컨트롤러 실측으로 `src/docs/manifest.tsv`까지 조용히 사라졌고 앵커 형태도
  # 동일했다. 캐시가 `tree/`와 `meta/`를 분리하므로 **제외할 것이 아예 없다**
  src="$LOCO_TASKS_REAL_CACHE/$repo/$parent/tree"
  dst="$task_dir/fixture"
  rm -rf "$dst"; mkdir -p "$dst"
  ( cd "$src" && tar -cf - . ) | ( cd "$dst" && tar -xf - )

  # fixture-overlay/ — 백포트 테스트 등 사람이 얹는 것 (§3-3)
  if [ -d "$task_dir/fixture-overlay" ]; then
    ( cd "$task_dir/fixture-overlay" && tar -cf - . ) | ( cd "$dst" && tar -xf - )
    echo "  오버레이 적용: $task_dir/fixture-overlay"
  fi

  # target/ 가드 (§3-5·§9-A3) — 캐시도 <task_dir>/fixture도 **빌드 디렉터리로
  # 겸하지 않는다**. copy_tree는 .gitignore를 안 보므로(ignore 크레이트 미사용)
  # 픽스처에 target/이 있으면 60런 × 최대 1GB 복사가 된다.
  # 실측: zoxide 371M / fd 255M / ripgrep 459M / just 998M
  for guard in "$src" "$dst"; do
    if [ -e "$guard/target" ]; then
      echo "target/이 있습니다 (빌드 디렉터리 겸용 금지, §3-5): $guard/target" >&2
      exit 1
    fi
  done

  # .gitignore의 /target 요구 확인 — 없으면 모델의 bare list_files가
  # target/ 경로 ~14KB를 최종 메시지로 뱉어 컨텍스트 초과 400을 만든다
  grep -qE '^/?target/?$' "$dst/.gitignore" 2>/dev/null \
    || echo "  경고: .gitignore에 /target 규칙이 없습니다 — 배치 전 확인할 것" >&2
}

if [ "${1:-}" = "--all" ]; then
  root="${2:?tasks-real 루트를 지정하세요}"
  for d in "$root"/*/; do [ -f "$d/procure.toml" ] && procure_task "${d%/}"; done
else
  [ $# -ge 1 ] || { echo "usage: $0 <task-dir> [...] | --all <tasks-real>" >&2; exit 1; }
  for d in "$@"; do procure_task "$d"; done
fi
echo "조달 완료."
