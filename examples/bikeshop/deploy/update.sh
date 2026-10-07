#!/usr/bin/env bash
# Install or update Bikeshop on a Linux server from the builds GitHub makes
# (.github/workflows/release-bikeshop.yml). Run it as root:
#
#   curl -fsSL https://raw.githubusercontent.com/arif-rachim/renox/main/examples/bikeshop/deploy/update.sh \
#     | sudo bash -s -- --url https://shop.example.com --seed demo
#
# The first run installs everything: a `bikeshop` system user, /opt/bikeshop
# with its .env (production settings and a new APP_KEY), its storage, and the
# systemd units (the service, its socket for restarts without refused
# connections, and an hourly update timer). Later runs (by hand or the timer)
# download the newest build, check its SHA-256, run its migrations, switch to
# it, restart, and check /health; if the new build doesn't answer, they switch
# back to the previous one.
#
# Layout:
#   /opt/bikeshop/.env                  settings (kept across updates)
#   /opt/bikeshop/storage/              database (SQLite), uploads, logs
#   /opt/bikeshop/releases/<commit>/    each build (the last 3 are kept)
#   /opt/bikeshop/current -> releases/<commit>
#
# Options:
#   --version TAG   bikeshop-latest (default, the newest build of main) or a
#                   pinned release such as bikeshop-v1.0.0
#   --url URL       APP_URL on the first install (default http://<hostname>)
#   --port N        the local port the app listens on (default 3000)
#   --seed MODE     on the first install only: none (default), demo (small)
#                   or large (Pagila's volume, 18 months of history)
#   --dir DIR       install directory (default /opt/bikeshop)
#   --repo O/R      GitHub repository (default arif-rachim/renox)
#   --no-timer      don't install the hourly update timer
set -euo pipefail

VERSION=bikeshop-latest
APP_URL=""
PORT=""
SEED=none
DIR=/opt/bikeshop
REPO=arif-rachim/renox
TIMER=1
KEEP=3

while [ $# -gt 0 ]; do
  case "$1" in
    --version) VERSION=$2; shift 2 ;;
    --url) APP_URL=$2; shift 2 ;;
    --port) PORT=$2; shift 2 ;;
    --seed) SEED=$2; shift 2 ;;
    --dir) DIR=$2; shift 2 ;;
    --repo) REPO=$2; shift 2 ;;
    --no-timer) TIMER=0; shift ;;
    -h|--help) echo "see the comments at the top of examples/bikeshop/deploy/update.sh"; exit 0 ;;
    *) echo "update.sh: unknown option $1 (see --help)" >&2; exit 2 ;;
  esac
done

say() { echo "bikeshop: $*"; }
die() { echo "bikeshop: $*" >&2; exit 1; }

[ "$(id -u)" = 0 ] || die "run me as root (sudo)"
command -v systemctl >/dev/null || die "systemd is needed"
command -v curl >/dev/null || die "curl is needed"
command -v sha256sum >/dev/null || die "sha256sum is needed"
case "$SEED" in none|demo|large) ;; *) die "--seed is none, demo or large" ;; esac
# The port: --port, else the installed .env's APP_PORT, else 3000.
if [ -z "$PORT" ] && [ -f "$DIR/.env" ]; then
  PORT=$(sed -n 's/^APP_PORT=\([0-9]*\).*/\1/p' "$DIR/.env" | tail -1)
fi
PORT=${PORT:-3000}

case "$(uname -m)" in
  x86_64|amd64) ARCH=x86_64 ;;
  aarch64|arm64) ARCH=aarch64 ;;
  *) die "no build for $(uname -m) (x86_64 and aarch64 only)" ;;
esac
NAME="bikeshop-$ARCH-linux"
# BIKESHOP_RELEASE_BASE overrides where builds come from (a mirror, or tests).
BASE="${BIKESHOP_RELEASE_BASE:-https://github.com/$REPO/releases/download/$VERSION}"

# Only one run at a time (the timer and a run by hand). The lock's file
# descriptor is closed for everything this script starts (`9>&-`), so the
# app never holds it.
exec 9>/run/bikeshop-update.lock
flock -n 9 || die "another update is running"
svc() { systemctl "$@" 9>&-; }

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

say "downloading $VERSION for $ARCH"
curl -fsSL --retry 3 -o "$TMP/$NAME.tar.gz" "$BASE/$NAME.tar.gz" || die "download failed: $BASE/$NAME.tar.gz"
curl -fsSL --retry 3 -o "$TMP/SHA256SUMS" "$BASE/SHA256SUMS" || die "download failed: $BASE/SHA256SUMS"
(cd "$TMP" && grep " $NAME.tar.gz\$" SHA256SUMS | sha256sum -c --quiet -) || die "the checksum doesn't match: not installing"
tar -C "$TMP" -xzf "$TMP/$NAME.tar.gz"
NEW_REV=$(cat "$TMP/$NAME/REVISION")
[ -n "$NEW_REV" ] || die "the build has no REVISION"

CURRENT_REV=""
[ -L "$DIR/current" ] && CURRENT_REV=$(basename "$(readlink "$DIR/current")")
if [ "$NEW_REV" = "$CURRENT_REV" ]; then
  say "already on ${NEW_REV:0:12}; nothing to do"
  exit 0
fi

FIRST=0
if [ ! -f "$DIR/.env" ]; then
  FIRST=1
  say "first install in $DIR"
  id bikeshop >/dev/null 2>&1 || useradd --system --home "$DIR" --shell /usr/sbin/nologin bikeshop
  mkdir -p "$DIR/storage" "$DIR/releases"
  chown bikeshop:bikeshop "$DIR" "$DIR/storage" "$DIR/releases"
  [ -n "$APP_URL" ] || APP_URL="http://$(hostname -f 2>/dev/null || hostname)"
  KEY="base64:$(head -c 32 /dev/urandom | base64)"
  # Production settings on top of the example's comments and defaults.
  sed -e "s|^APP_ENV=.*|APP_ENV=production|" \
      -e "s|^APP_DEBUG=.*|APP_DEBUG=false|" \
      -e "s|^APP_KEY=.*|APP_KEY=$KEY|" \
      -e "s|^APP_URL=.*|APP_URL=$APP_URL|" \
      -e "s|^APP_HOST=.*|APP_HOST=127.0.0.1|" \
      -e "s|^APP_PORT=.*|APP_PORT=$PORT|" \
      "$TMP/$NAME/.env.example" > "$DIR/.env"
  grep -q '^TRUSTED_PROXIES=' "$DIR/.env" || echo "TRUSTED_PROXIES=127.0.0.1" >> "$DIR/.env"
  chown bikeshop:bikeshop "$DIR/.env"
  chmod 600 "$DIR/.env"
fi

REL="$DIR/releases/$NEW_REV"
rm -rf "$REL"
mkdir -p "$REL"
cp -r "$TMP/$NAME/." "$REL/"
chown -R bikeshop:bikeshop "$REL"

# The app reads .env and storage/ from its working directory.
run_as_app() { (cd "$DIR" && setpriv --reuid=bikeshop --regid=bikeshop --init-groups "$@" 9>&-); }

say "running migrations with ${NEW_REV:0:12}"
run_as_app "$REL/bikeshop" migrate || die "migrations failed; still on ${CURRENT_REV:0:12}"

if [ "$FIRST" = 1 ]; then
  case "$SEED" in
    demo) say "seeding the demo shop"; run_as_app "$REL/bikeshop" db:seed ;;
    large) say "seeding the large demo shop (Pagila's volume)"; run_as_app "$REL/bikeshop" demo:seed --size large ;;
  esac
  # The units, from the build, pointed at this directory and port.
  for unit in bikeshop.service bikeshop.socket bikeshop-update.service bikeshop-update.timer; do
    [ -f "$REL/deploy/$unit" ] || continue
    sed -e "s|/opt/bikeshop|$DIR|g" -e "s|127.0.0.1:3000|127.0.0.1:$PORT|" \
      "$REL/deploy/$unit" > "/etc/systemd/system/$unit"
  done
  cp "$REL/deploy/update.sh" /usr/local/sbin/bikeshop-update
  chmod 755 /usr/local/sbin/bikeshop-update
  svc daemon-reload
  svc enable bikeshop.socket bikeshop.service
  [ "$TIMER" = 1 ] && svc enable --now bikeshop-update.timer
fi

switch_to() {
  ln -sfn "releases/$1" "$DIR/.current.tmp"
  mv -T "$DIR/.current.tmp" "$DIR/current"
}

healthy() {
  for _ in $(seq 1 30); do
    curl -fs -o /dev/null "http://127.0.0.1:$PORT/health" && return 0
    sleep 1
  done
  return 1
}

switch_to "$NEW_REV"
svc start bikeshop.socket
svc restart bikeshop.service
if healthy; then
  say "running ${NEW_REV:0:12} on 127.0.0.1:$PORT ($VERSION)"
else
  if [ -n "$CURRENT_REV" ] && [ -d "$DIR/releases/$CURRENT_REV" ]; then
    say "${NEW_REV:0:12} doesn't answer /health: back to ${CURRENT_REV:0:12}"
    switch_to "$CURRENT_REV"
    svc restart bikeshop.service
    die "update failed (the migrations it ran stay; see journalctl -u bikeshop)"
  fi
  die "the app doesn't answer /health (journalctl -u bikeshop)"
fi

# Keep the newest $KEEP releases (and always the current one).
ls -1t "$DIR/releases" | tail -n +$((KEEP + 1)) | while read -r old; do
  [ "$old" = "$NEW_REV" ] || rm -rf "${DIR:?}/releases/$old"
done

# Keep the script itself up to date.
[ -f "$REL/deploy/update.sh" ] && install -m 755 "$REL/deploy/update.sh" /usr/local/sbin/bikeshop-update
exit 0
