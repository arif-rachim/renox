#!/usr/bin/env python3
"""Trims `cargo test` output to what a reader needs: the test lines, the
failures block, compile errors and `test result:`. Paths are made relative and
timings and thread numbers replaced, so a recorded output stays the same from
one run to the next. Reads stdin, writes stdout."""
import re
import sys

text = sys.stdin.read()
text = re.sub(r"\x1b\[[0-9;]*m", "", text)
repo = sys.argv[1] if len(sys.argv) > 1 else ""
work = sys.argv[2] if len(sys.argv) > 2 else ""
for prefix in (work, repo):
    if prefix:
        text = text.replace(prefix.rstrip("/") + "/", "")

noise = re.compile(
    r"^\s*(Compiling|Finished|Running|Blocking|Downloading|Downloaded|Updating|Locking|Fresh|Checking|Doc-tests)\b"
)
out = []
skipping_warning = False
skipping_page = False
for line in text.splitlines():
    # A response's page is cut after its first line: it holds a random token.
    if skipping_page:
        if line.startswith(("note:", "failures:", "----")) or line.strip() == "":
            skipping_page = False
        else:
            if line.endswith("…"):
                skipping_page = False
            continue
    if noise.match(line):
        continue
    if line.startswith("warning"):
        skipping_warning = True
        continue
    if skipping_warning:
        if line.strip() == "":
            skipping_warning = False
        continue
    if line.startswith("For more information about"):
        continue
    line = re.sub(r"finished in [0-9.]+s", "finished in …", line)
    line = re.sub(r"(thread '[^']*') \(\d+\)", r"\1", line)
    out.append(line.rstrip())
    if re.match(r"^expected .*:$", line):
        out.append("…")
        skipping_page = True

# Tests finish in any order: the `test … ok` lines are sorted.
sorted_out = []
run = []
for line in out + [None]:
    if line is not None and line.startswith("test ") and not line.startswith("test result"):
        run.append(line)
        continue
    sorted_out.extend(sorted(run))
    run = []
    if line is not None:
        sorted_out.append(line)
out = sorted_out

# One blank line at most in a row, none at the ends.
result = []
for line in out:
    if line == "" and (not result or result[-1] == ""):
        continue
    result.append(line)
while result and result[-1] == "":
    result.pop()
print("\n".join(result))
