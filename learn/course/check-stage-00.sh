#!/usr/bin/env bash
# Stage 0 must be exactly what `rnx new coffee` writes (only the `renox =` line
# differs: it points into this checkout). Usage: check-stage-00.sh RNX
# Exits 1, with the diff, when they differ.
set -euo pipefail

RNX=$(realpath "${1:?usage: check-stage-00.sh RNX}")
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

(cd "$tmp" && "$RNX" new coffee --renox-path "$REPO" > /dev/null)

# The stub points at this checkout; the course points at it relatively.
line=$(grep '^renox = ' "$HERE/stage-00/Cargo.toml")
python3 - "$tmp/coffee/Cargo.toml" "$line" <<'PY'
import re, sys
path, line = sys.argv[1], sys.argv[2]
text = open(path).read()
text, n = re.subn(r'(?m)^renox = .*$', lambda _: line, text)
assert n == 1, "no `renox =` line in the new app's Cargo.toml"
open(path, "w").write(text)
PY

if ! diff -r -x target -x storage -x .env -x .cargo -x Cargo.lock -x AGENTS.md -x CLAUDE.md \
    -x renox-components.json "$tmp/coffee" "$HERE/stage-00"; then
  echo "stage-00 is not what \`rnx new coffee\` writes (see the diff above)." >&2
  exit 1
fi
echo "stage-00 matches rnx new coffee."
