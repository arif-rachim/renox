#!/usr/bin/env bash
# Builds learn.renox.rs's CodeMirror 6 bundle (#528) in a pinned Node
# container and copies it, with the packages' licences, to public/vendor/.
# Run from anywhere; the result must be committed, and CI checks that a
# fresh run changes nothing (job learn-editor).
#
#   learn/editor/bundle.sh
#   CONTAINER=podman learn/editor/bundle.sh
#   CONTAINER_ARGS="--network host" learn/editor/bundle.sh   # extra run flags (a proxy, a mirror)
#
# Bumping a version: README.md next to this script.
set -euo pipefail
cd "$(dirname "$0")/../.."

IMAGE=node:24-bookworm-slim@sha256:d6aa754f16b3197301076f047b5def2f02ea1dbbc2ca920407d46d7ec7f87b20
CONTAINER=${CONTAINER:-docker}
vendor=learn/public/vendor

# shellcheck disable=SC2086 # CONTAINER_ARGS is a list of flags
"$CONTAINER" run --rm ${CONTAINER_ARGS:-} -u "$(id -u):$(id -g)" -e HOME=/tmp \
  -v "$PWD/learn/editor:/work" -w /work "$IMAGE" sh -euc '
    npm ci --ignore-scripts --no-audit --no-fund >&2
    npx esbuild entry.mjs --bundle --format=iife --global-name=CM --minify --target=es2022 --charset=utf8 --legal-comments=none \
      --banner:js="/* CodeMirror 6 for learn.renox.rs. MIT licence; see vendor/NOTICE. */" --outfile=out/codemirror.js >&2
    node -p "JSON.parse(require(\"fs\").readFileSync(\"node_modules/@codemirror/view/package.json\")).version" > out/version
  '

version=$(cat learn/editor/out/version)
mkdir -p "$vendor/licenses"
rm -f "$vendor"/codemirror-*.js "$vendor"/licenses/*
cp learn/editor/out/codemirror.js "$vendor/codemirror-$version.js"

# Every package the bundle is made of (not the dev dependencies), with its
# licence: what NOTICE lists (a test checks the two agree).
node -e '
  const lock = require("./learn/editor/package-lock.json");
  for (const [path, entry] of Object.entries(lock.packages)) {
    if (path && !entry.dev) console.log(path.replace(/^node_modules\//, ""));
  }
' | while read -r package; do
  licence=""
  for name in LICENSE LICENSE.md LICENSE.txt license; do
    [ -f "learn/editor/node_modules/$package/$name" ] && { licence="learn/editor/node_modules/$package/$name"; break; }
  done
  [ -n "$licence" ] || { echo "no licence file in $package" >&2; exit 1; }
  name=${package#@}
  cp "$licence" "$vendor/licenses/${name//\//-}-LICENSE"
done
echo "$vendor/codemirror-$version.js: $(wc -c < "$vendor/codemirror-$version.js") bytes"
