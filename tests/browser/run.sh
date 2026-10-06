#!/usr/bin/env bash
# Browser tests (#262): builds the apps they drive, one at a time, then runs
# every tests/browser/*.test.mjs with Node's own test runner, one file at a
# time, against headless Chrome. No npm packages.
#
#   tests/browser/run.sh                 # everything
#   tests/browser/run.sh renoxjs grid    # only these files
#
# Needs Node 24+ and Chrome (CHROME_BIN to use another binary). Screenshots of
# failures go to target/browser-screens.
set -euo pipefail
cd "$(dirname "$0")/../.."

jobs="${CARGO_BUILD_JOBS:-2}"
for package in browser-fixture grid fields htmx-recipes hello crud shop teams backoffice admin billing; do
  echo "building $package"
  cargo build --quiet -j "$jobs" -p "$package"
done

files=()
if [ "$#" -gt 0 ]; then
  for name in "$@"; do files+=("tests/browser/$name.test.mjs"); done
else
  files=(tests/browser/*.test.mjs)
fi
# A test that hangs fails after two minutes, with its name, instead of
# holding the whole run (a CI job once hung for hours with no sign which).
node --test --test-concurrency=1 --test-timeout=120000 "${files[@]}"
