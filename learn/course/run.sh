#!/usr/bin/env bash
# Checks the course app: every stage (or the ones named) is formatted, lint-free,
# passes its tests and `view:check` / `db:check`, and has no schema change left
# for `db:diff`; every Cargo.lock equals stage-00's; stage-00 is still what
# `rnx new coffee` makes. Run it from anywhere:
#
#   learn/course/run.sh                         every stage
#   learn/course/run.sh learn/course/stage-01   one stage
#
# All stages are the package `coffee` and share one target directory, so the
# dependencies compile once. Cargo tells two of them apart by their files'
# times, so `cargo clean -p coffee` runs before and after each stage.
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
cd "$REPO"

# A caller that sets CARGO_TARGET_DIR (a CI cache, a memory-capped slot) keeps it
# for everything; otherwise rnx builds into the repo's target and the stages into
# learn/course/target (ignored by git).
if [ -n "${CARGO_TARGET_DIR:-}" ]; then
  RNX_TARGET=$CARGO_TARGET_DIR
else
  RNX_TARGET=$REPO/target
  export CARGO_TARGET_DIR=$HERE/target
fi

if [ "$#" -gt 0 ]; then
  stages=()
  for s in "$@"; do stages+=("$(cd "$s" && pwd)"); done
else
  stages=("$HERE"/stage-*)
fi

echo "== building rnx"
CARGO_TARGET_DIR=$RNX_TARGET cargo build -q -p renox-cli
RNX=$RNX_TARGET/debug/rnx

echo "== stage-00 is what rnx new coffee makes"
"$HERE/check-stage-00.sh" "$RNX"

for dir in "${stages[@]}"; do
  name=$(basename "$dir")
  echo "== $name"
  cmp "$HERE/stage-00/Cargo.lock" "$dir/Cargo.lock" \
    || { echo "$name/Cargo.lock differs from stage-00's" >&2; exit 1; }
  cd "$dir"
  cargo clean -q -p coffee
  cargo fmt --check
  cargo clippy -q --locked --all-targets -- -D warnings
  cargo test -q --locked
  cargo run -q --locked -- view:check
  cargo run -q --locked -- db:check
  if [ "$name" != "stage-00" ]; then
    tmp=$(mktemp -d)
    out=$(cargo run -q --locked -- db:diff --yes --path "$tmp")
    rm -rf "$tmp"
    echo "$out"
    grep -q "Nothing to change." <<< "$out" \
      || { echo "$name: the models and the migrations disagree (db:diff)" >&2; exit 1; }
  fi
  cargo clean -q -p coffee
  cd "$REPO"
done
echo "== the course app is fine"
