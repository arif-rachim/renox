#!/usr/bin/env bash
# Uploads the encoded videos to the server that serves them, then checks each
# one over HTTP against content/media.json (status 200, Content-Length = bytes).
#
#   LEARN_MEDIA_DEST=user@host:/path/to/storage/app/public/media/ learn/media/upload.sh
#
# or LEARN_MEDIA_HOST, LEARN_MEDIA_USER and LEARN_MEDIA_PATH. The destination comes from the environment only: it is never written to a
# file, and no host, user, key or path belongs in this repository.
# LEARN_URL (default https://learn.renox.rs) is where the check looks.
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
cd "$HERE/../.."

# LEARN_MEDIA_DEST, or LEARN_MEDIA_HOST + LEARN_MEDIA_USER + LEARN_MEDIA_PATH.
if [ -z "${LEARN_MEDIA_DEST:-}" ]; then
  missing=()
  for name in LEARN_MEDIA_HOST LEARN_MEDIA_USER LEARN_MEDIA_PATH; do
    [ -n "${!name:-}" ] || missing+=("$name")
  done
  if [ "${#missing[@]}" -gt 0 ]; then
    echo "upload.sh: set LEARN_MEDIA_DEST (user@host:/path/), or all of LEARN_MEDIA_HOST, LEARN_MEDIA_USER and LEARN_MEDIA_PATH (missing: ${missing[*]})" >&2
    exit 2
  fi
  LEARN_MEDIA_DEST="$LEARN_MEDIA_USER@$LEARN_MEDIA_HOST:${LEARN_MEDIA_PATH%/}/"
fi
URL=${LEARN_URL:-https://learn.renox.rs}
OUT=learn/media/out
shopt -s nullglob
files=("$OUT"/*.webm "$OUT"/*.mp4 "$OUT"/*.webp)
if [ "${#files[@]}" -eq 0 ]; then
  echo "upload.sh: nothing in $OUT: record and encode first (learn/README.md, \"Videos\")" >&2
  exit 1
fi

rsync -av --checksum --chmod=F644 "${files[@]}" "$LEARN_MEDIA_DEST"

# Every file of the manifest must answer 200 with the committed size.
status=0
while IFS=$'\t' read -r name bytes; do
  head=$(curl -sI "$URL/storage/media/$name" || true)
  code=$(printf '%s' "$head" | head -1 | cut -d' ' -f2)
  length=$(printf '%s' "$head" | tr -d '\r' | sed -n 's/^[Cc]ontent-[Ll]ength: *//p' | head -1)
  if [ "$code" = 200 ] && [ "$length" = "$bytes" ]; then
    echo "ok   $name"
  else
    echo "FAIL $name (status ${code:-none}, length ${length:-none}, want $bytes)" >&2
    status=1
  fi
done < <(node -e '
  const m = JSON.parse(require("fs").readFileSync("learn/content/media.json", "utf8"));
  for (const v of Object.values(m.videos))
    for (const f of [v.poster, ...v.sources]) console.log(f.file + "\t" + f.bytes);
')
exit $status
