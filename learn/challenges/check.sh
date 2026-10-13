#!/usr/bin/env bash
# Proves every challenge: its start FAILS its test, its solution PASSES it and
# passes clippy. For each challenge folder (or the ones named) the stage app is
# copied to a temporary directory (without target/), the start files and the
# test are copied over it, and `cargo test --test <test>` must fail; then the
# solution is copied over it, and the test and `cargo clippy -D warnings` must
# pass. The two outputs are compared with output-failing.txt and
# output-passing.txt after normalising (normalize.py); with RECORD=1 they are
# written instead (tests run one at a time, so the output is the same every run). The temporary directory is deleted after each challenge.
#
#   learn/challenges/check.sh                  every challenge
#   learn/challenges/check.sh s02/cheap-items  one
#   RECORD=1 learn/challenges/check.sh         write the recorded outputs
#   FULL=1 learn/challenges/check.sh           also run the whole test suite of each solution
#
# All challenges build the package `coffee` against one target directory
# (CARGO_TARGET_DIR, else learn/course/target), so the dependencies compile once.
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
COURSE=$HERE/../course
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$COURSE/target}
export CARGO_TERM_COLOR=never
export CARGO_INCREMENTAL=0
export RUST_BACKTRACE=0

if [ "$#" -gt 0 ]; then
  dirs=()
  for c in "$@"; do dirs+=("$HERE/${c#learn/challenges/}"); done
else
  dirs=("$HERE"/s[0-9][0-9]/*/)
fi

fail() { echo "FAIL $name: $*" >&2; exit 1; }

compare() { # file, text: record it or compare it with the recorded one
  local file=$1 text=$2
  if [ -n "${RECORD:-}" ]; then
    printf '%s\n' "$text" > "$file"
  elif [ ! -f "$file" ]; then
    fail "$(basename "$file") is not recorded (run with RECORD=1)"
  elif ! diff <(printf '%s\n' "$text") "$file" > /dev/null; then
    diff <(printf '%s\n' "$text") "$file" >&2 || true
    fail "$(basename "$file") is out of date (run with RECORD=1)"
  fi
}

for dir in "${dirs[@]}"; do
  dir=${dir%/}
  id=$(basename "$dir")
  stage=$(basename "$(dirname "$dir")")
  name=$stage/$id
  [ -f "$dir/challenge.md" ] || fail "no challenge.md"
  test=$(sed -n 's/^test:[[:space:]]*//p' "$dir/challenge.md" | head -1)
  [ -n "$test" ] || fail "no test: in challenge.md"
  echo "== $name"

  work=$(mktemp -d)
  cleanup() { (cd "$work" && cargo clean -q -p coffee 2>/dev/null || true); rm -rf "$work"; }
  trap cleanup EXIT

  tar -C "$COURSE/stage-${stage#s}" --exclude=./target --exclude=./storage -cf - . | tar -C "$work" -xf -
  # The stage apps reach the framework by a path relative to learn/course/stage-NN.
  sed -i "s#\"../../../crates/renox\"#\"$REPO/crates/renox\"#" "$work/Cargo.toml"
  cp -r "$dir/start/." "$work/"
  cp "$dir"/tests/*.rs "$work/tests/"
  cd "$work"

  set +e
  failing=$(cargo test --locked --test "$test" -- --test-threads=1 2>&1)
  status=$?
  set -e
  [ "$status" -ne 0 ] || fail "the start passes its test"
  grep -q "^error: could not compile\|test result: FAILED" <<< "$failing" \
    || fail "the start failed for another reason: $failing"
  compare "$dir/output-failing.txt" "$(python3 -I "$HERE/normalize.py" "$REPO" "$work" <<< "$failing")"

  cp -r "$dir/solution/." "$work/"
  set +e
  passing=$(cargo test --locked --test "$test" -- --test-threads=1 2>&1)
  status=$?
  set -e
  [ "$status" -eq 0 ] || fail "the solution fails its test: $passing"
  grep -q "^test result: ok. [1-9]" <<< "$passing" || fail "the solution ran no test"
  compare "$dir/output-passing.txt" "$(python3 -I "$HERE/normalize.py" "$REPO" "$work" <<< "$passing")"
  cargo clippy --locked --all-targets -q -- -D warnings || fail "the solution has clippy warnings"
  # FULL=1: the stage's own tests still pass with the solution in place.
  if [ -n "${FULL:-}" ]; then
    cargo test --locked -q > /dev/null 2>&1 || fail "the solution breaks the stage's own tests"
  fi

  cd "$REPO"
  cleanup
  trap - EXIT
  echo "ok   $name"
done
echo "== every challenge: the start fails, the solution passes"
