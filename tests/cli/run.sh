#!/usr/bin/env bash
# Creates an app with `rnx new`, runs every generator in it, then builds and
# tests the result, so generated code that doesn't compile fails CI.
#
#   tests/cli/run.sh            # sqlite
#   tests/cli/run.sh postgres   # `rnx new --database postgres` (build only)
#   KEEP=1 tests/cli/run.sh     # keep the app and print where it is
#
#   FROM_GIT=1 DOCKER=1 tests/cli/run.sh
#       The app depends on Renox from GitHub, pinned to this checkout's commit
#       (as for users; the commit must be pushed), and the Dockerfile from
#       `make:deploy` is built and started, and must answer /health.
#
# Run from the repository root. CI runs it (.github/workflows/ci.yml).
set -euo pipefail

DATABASE=${1:-sqlite}
REPO=$(pwd)
WORK=$(mktemp -d)
# Reuse the workspace's build of Renox's dependencies.
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$REPO/target/cli-e2e}
cleanup() {
    if [ -n "${KEEP:-}" ]; then echo "app kept in $WORK/shop"; else rm -rf "$WORK"; fi
}
trap cleanup EXIT

step() { printf '\n== %s\n' "$*"; }

cargo build -q -p renox-cli
RNX="$CARGO_TARGET_DIR/debug/rnx"

step "rnx new shop --database $DATABASE"
cd "$WORK"
if [ -n "${FROM_GIT:-}" ]; then
    "$RNX" new shop --database "$DATABASE"
else
    "$RNX" new shop --renox-path "$REPO" --database "$DATABASE"
fi
cd shop
grep '^renox' Cargo.toml

step "every generator"
"$RNX" make:module catalog
"$RNX" make:model Product --module catalog --migration
"$RNX" make:module stock_movement
"$RNX" make:model StockMovement -m
"$RNX" make:job SendReceipt --module catalog
"$RNX" make:command catalog:import --module catalog
"$RNX" make:policy Product --module catalog
"$RNX" make:mail order_shipped
"$RNX" make:migration add_sku_to_products
"$RNX" make:deploy

step "cargo build and test"
cargo build --all-targets
if [ "$DATABASE" = sqlite ]; then
    cargo test
    step "the app's own commands"
    cargo run -q -- migrate
    cargo run -q -- migrate:status
    cargo run -q -- catalog:import
    cargo run -q -- route:list
fi

if [ -n "${DOCKER:-}" ]; then
    step "docker build and run"
    IMAGE=renox-cli-e2e
    NAME=renox-cli-e2e
    PORT=${E2E_PORT:-3088}
    docker build -t "$IMAGE" .
    docker rm -f "$NAME" >/dev/null 2>&1 || true
    docker run -d --name "$NAME" -p "$PORT:3000" \
        -e APP_KEY="$(grep '^APP_KEY=' .env | cut -d= -f2-)" -e APP_URL="http://127.0.0.1:$PORT" \
        "$IMAGE" >/dev/null
    trap 'docker rm -f "$NAME" >/dev/null 2>&1 || true; cleanup' EXIT
    status=000
    for _ in $(seq 60); do
        status=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$PORT/health" || true)
        [ "$status" = 200 ] && break
        sleep 1
    done
    if [ "$status" != 200 ]; then
        docker logs "$NAME" | tail -40
        echo "FAIL: /health answered $status"
        exit 1
    fi
    curl -s "http://127.0.0.1:$PORT/health"
    echo
    curl -sf -o /dev/null "http://127.0.0.1:$PORT/catalog" && echo "ok   /catalog"
    docker image ls "$IMAGE" --format 'image size: {{.Size}}'
fi

echo
if [ "$DATABASE" = sqlite ]; then
    echo "cli e2e: the generated app builds, passes its tests and runs its commands"
else
    echo "cli e2e ($DATABASE): the generated app builds"
fi
