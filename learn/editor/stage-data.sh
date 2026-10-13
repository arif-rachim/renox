#!/usr/bin/env bash
# Writes each course stage's tags (#529): `view:data` (VS Code's
# html.customData: the kit's rx-* tags and the stage's <app-*> components)
# into learn/editor/data/tags/stage-NN.json. The output is committed; the
# learn-course CI job runs this and fails on a diff. Then
# `RENOX_UPDATE_SNAPSHOT=1 cargo test -p renox-learn --test editor_data`
# turns them into the editor's data/stage-NN.json.
#
#   learn/editor/stage-data.sh                 # every stage
#   learn/editor/stage-data.sh learn/course/stage-02
set -euo pipefail
cd "$(dirname "$0")/../.."

stages=("$@")
[ "${#stages[@]}" -gt 0 ] || stages=(learn/course/stage-*/)
for stage in "${stages[@]}"; do
  stage=${stage%/}
  n=${stage##*stage-}
  echo "== $stage"
  # Every stage is the package `coffee` in one target directory: cargo tells
  # them apart by file times only (learn/course/run.sh).
  (cd "$stage" && cargo clean -q -p coffee \
    && DATABASE_URL=sqlite::memory: cargo run -q --locked -- view:data --out "../../editor/data/tags/stage-$n.json" \
    && cargo clean -q -p coffee)
done
