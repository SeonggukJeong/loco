#!/usr/bin/env python3
"""§3-4-3 픽스처 누설 감사 — `check` 출력에서 "지목된" 소스 경로를 뽑는다 (M15).

초판의 오류는 종류가 **"프롬프트만 보고 픽스처를 안 봤다"**였다. 결론만 고치고
절차를 안 고치면 다음 배치도 누설을 감사하지 않은 채 착수한다 — 그래서 이
추출을 스크립트로 못박는다. **사람 판독 금지**(감사자 간 일치를 구조적으로 보장).

usage:
  python3 scripts/leak_audit.py <check-output.txt> [--sandbox <abs-prefix>]
                                [--oracle a.rs --oracle b.rs]
  python3 scripts/leak_audit.py --selftest

판정: 추출 집합 ∩ 오라클 집합 ≠ ∅ 이면 **지목됨 → 과제 제외**(§3-2 규약 6).
처분은 하나로 고정돼 있다 — "제외하거나 라벨한다"는 표본 수와 쿼터를 동시에
조작할 자유도가 된다.

표준 라이브러리만 사용(폐쇄망 개발 도구).
"""
import re
import sys

# 소스 경로 후보. `:행:열` 접미는 뒤에서 떼므로 여기서는 .rs까지만 문다
# ⚠ 한 줄이 아주 길면 백트래킹으로 느려진다(1R 실현 m4 실측: 10만 자 한 줄에 7.7초).
# 지수 폭발은 아니지만 큰 단언 덤프가 한 줄로 잡히면 감사가 길어지므로, 줄 길이를
# 잘라서 먼다 — 누설 판정에 필요한 경로는 줄 앞부분에 있다
MAX_LINE = 4000
PATH_RE = re.compile(r"[A-Za-z0-9_./-]+\.rs")

# **제외** 대상 줄 — 판정 범위 한정(4R 측정 I7).
# 컴파일러 진단은 미완성 기능 주변에 남기 쉬워 누설과 무관하게 과제를 떨어뜨린다.
# Running/Compiling은 테스트 **바이너리·크레이트** 이름이지 원인 지점이 아니다.
#
# ⚠⚠ **이 패턴은 `failures:` 구간 *밖*에서만 쓴다**(플랜 1R Critical 4). 구간 안쪽에
# 적용하면 `note:`/`help:`/`warning:` 로 시작하는 **테스트 자신의 단언 메시지**를
# 컴파일러 진단으로 오인해 삼킨다 — 실측 재현: 패닉 본문의
# `note: see src/secret.rs for the actual computation` 이 통째로 버려져
# 오라클 `src/secret.rs`가 "지목되지 않음(rc=0)"으로 통과했다. §3-2 규약 6이 이
# 스크립트를 누설 차단 게이트로 쓰므로 안전장치 자체의 결함이었다.
SKIP_RE = re.compile(r"^\s*(-->|Running|Compiling|Finished|warning:|error(\[|:)|note:|help:)")

# `failures:` 구간 **안**에서만 쓰는 좁은 제외 — 컴파일러 진단의 위치 화살표만 뺀다.
# 진단 본문(`warning:` 등)은 구간 안에 나타나면 그것은 libtest가 캡처한 **테스트
# 출력**이지 컴파일러 출력이 아니다(컴파일은 `failures:` 이전에 끝난다)
SKIP_IN_FAILURES_RE = re.compile(r"^\s*-->")


def failure_region(text):
    """실패 보고 구간만 돌려준다.

    두 구간을 센다:
      ① `failures:` 절 이후 (libtest가 실패 테스트 이름과 상세를 모아 찍는 곳)
      ② 각 실패 테스트의 패닉 메시지 (`thread '…' panicked at <path>:<line>`)

    ⚠ 그 밖은 전부 버린다 — 특히 컴파일러 진단. 항해 단축은 *실패 보고*가
    원인 지점을 가리킬 때만 일어난다(§3-4-3 판정 범위 한정).
    """
    lines = text.splitlines()
    out, in_failures = [], False
    for line in lines:
        if re.match(r"^failures:\s*$", line):
            in_failures = True
            continue
        if re.match(r"^test result:", line):
            in_failures = False
            continue
        if in_failures:
            # 구간 안: 좁은 제외만. 넓은 SKIP_RE를 쓰면 테스트 자신의 note:/help:
            # 단언 메시지가 삼켜져 진짜 누설을 놓친다 (1R Critical 4)
            if not SKIP_IN_FAILURES_RE.match(line):
                out.append(line)
            continue
        # 구간 밖: 컴파일러 진단·Running/Compiling을 전부 뺀 뒤, 패닉 줄만 줍는다
        if SKIP_RE.match(line):
            continue
        if "panicked at" in line:
            out.append(line)
    return out


def extract(text, sandbox_prefix=None):
    """실패 보고 구간의 소스 경로 집합. 샌드박스 절대 경로 접두를 제거해
    레포 상대 경로로 정규화하고 `:행:열` 접미를 뗀다."""
    found = set()
    for line in failure_region(text):
        for m in PATH_RE.findall(line[:MAX_LINE]):
            p = m
            if sandbox_prefix:
                pre = sandbox_prefix.rstrip("/") + "/"
                if p.startswith(pre):
                    p = p[len(pre):]
            p = p.lstrip("./")
            found.add(p)
    return found


def main(argv):
    if "--selftest" in argv:
        return selftest()
    if not argv:
        print(__doc__)
        return 2
    path, sandbox, oracle = argv[0], None, []
    i = 1
    while i < len(argv):
        if argv[i] == "--sandbox":
            sandbox = argv[i + 1]; i += 2
        elif argv[i] == "--oracle":
            oracle.append(argv[i + 1].lstrip("./")); i += 2
        else:
            i += 1
    text = open(path, encoding="utf-8", errors="replace").read()
    found = extract(text, sandbox)
    print("# 실패 보고 구간에서 추출된 소스 경로:")
    for p in sorted(found):
        print(f"  {p}")
    if not oracle:
        print("# (오라클 미지정 — 판정은 하지 않는다)")
        return 0
    hit = found & set(oracle)
    print(f"# 오라클: {sorted(oracle)}")
    if hit:
        print(f"지목됨 — 교집합 {sorted(hit)} ≠ ∅ → **과제 제외** (§3-2 규약 6)")
        return 1
    print("지목되지 않음 — 교집합 공집합 → 채택 가능")
    return 0


def selftest():
    # ① 패닉 메시지가 원인 소스를 지목하는 경우 → 지목됨
    leaky = """
   Compiling foo v0.1.0 (/tmp/loco-eval-1/foo)
warning: unused variable: `x`
  --> /tmp/loco-eval-1/src/walk.rs:12:9
    Finished test [unoptimized] target(s)
     Running tests/cli.rs (target/debug/deps/cli-abc)

running 1 test
test walks_hidden ... FAILED

failures:

---- walks_hidden stdout ----
thread 'walks_hidden' panicked at /tmp/loco-eval-1/src/walk.rs:88:5:
assertion failed

failures:
    walks_hidden

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 filtered out
"""
    got = extract(leaky, "/tmp/loco-eval-1")
    assert "src/walk.rs" in got, got
    # 컴파일러 진단(-->)만 있는 경로는 안 잡혀야 한다
    clean = """
warning: unused import
  --> /tmp/loco-eval-1/src/secret.rs:3:5
     Running tests/cli.rs (target/debug/deps/cli-abc)

failures:

---- t stdout ----
thread 't' panicked at tests/cli.rs:10:5:
assertion failed

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 filtered out
"""
    got2 = extract(clean, "/tmp/loco-eval-1")
    assert "src/secret.rs" not in got2, f"컴파일러 진단은 제외해야 한다: {got2}"
    assert "tests/cli.rs" in got2, got2
    # `test result:` 이후 잡음이 구간을 오염시키지 않는다
    assert not any(p.startswith("target/") for p in got2), got2

    # ③ **1R Critical 4 회귀 방지** — `failures:` 구간 **안**의 note:/help:/warning:은
    #    테스트 자신의 단언 메시지다. 컴파일러 진단으로 오인해 삼키면 진짜 누설을 놓친다.
    #    초판은 정확히 이 입력에서 "지목되지 않음(rc=0)"을 냈다
    leaky_note = """
failures:

---- t stdout ----
thread 't' panicked at tests/cli.rs:10:5:
assertion failed
note: see src/secret.rs for the actual computation
help: compare with src/helper.rs
warning: value drifted in src/drift.rs

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 filtered out
"""
    got3 = extract(leaky_note)
    for must in ("src/secret.rs", "src/helper.rs", "src/drift.rs"):
        assert must in got3, f"failures: 구간 안의 단언 메시지를 삼켰다 — {must} 누락: {got3}"

    # ④ 구간 안의 `-->`(컴파일러 위치 화살표 형태)는 여전히 뺀다
    assert "src/arrow.rs" not in extract(
        "failures:\n\n  --> src/arrow.rs:1:1\n\ntest result: FAILED. 0 passed; 1 failed;\n"
    ), "구간 안이라도 --> 는 제외"

    print("leak_audit selftest OK")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
