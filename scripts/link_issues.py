#!/usr/bin/env python3
"""git log 레코드(NUL 구분)에서 이슈↔커밋 연결을 뽑는다 (M15 T20 Step 3a).

usage: link_issues.py <records-file> <out.tsv>

⚠ **NUL 구분이 계약이다.** `%b`는 다중행이라 줄 단위로 읽으면 `Fixes #N`이 든 본문
줄에 SHA가 없고, `cut -f1`이 그 줄을 통째로 SHA로 넘긴다 — 5R 실측으로 ripgrep
269행 중 212행이 쓰레기였고 후보 수가 37→11로, 편중이 57%→91%로 뒤집혔다.
⚠ **stdin을 쓰지 않는다.** heredoc과 파이프를 함께 쓰면 파이프가 이겨서 python이
git 출력을 프로그램으로 읽는다(컨트롤러 실측: `SyntaxError: source code cannot
contain null bytes`). 레코드는 반드시 **파일로** 넘긴다.
"""
import re, sys

pat = re.compile(r"\b(clos(?:e|es|ed)|fix(?:|es|ed)|resolv(?:e|es|ed)) +#(\d+)", re.I)
recs = open(sys.argv[1], "rb").read().split(b"\x00")
n = 0
with open(sys.argv[2], "w") as out:
    for rec in recs:
        if not rec.strip():
            continue
        parts = rec.decode("utf-8", "replace").lstrip("\n").split("\x1f")
        if len(parts) < 3:
            continue
        sha, subj, body = parts[0].strip(), parts[1], parts[2]
        if len(sha) != 40 or not re.fullmatch(r"[0-9a-f]{40}", sha):
            continue            # SHA가 아닌 레코드는 버린다 (조용한 오염 차단)
        issues = sorted({m.group(2) for m in pat.finditer(subj + "\n" + body)}, key=int)
        if issues:
            out.write(f"{sha}\t{','.join(issues)}\t{subj}\n")
            n += 1
print(n)
