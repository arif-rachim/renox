#!/usr/bin/env bash
# Encodes recorded scenes: learn/media/out/frames/<id>/frames.txt (made by
# record.mjs) -> out/<id>.webm, <id>.mp4, <id>.webp and <id>.duration.
# ffmpeg runs in the pinned image (Containerfile.ffmpeg), without a network.
#
#   learn/media/encode.sh s05-add-item [id...]      run from anywhere
#
# `docker` or `podman` is used (set ENGINE to choose). Scenes' poster_at comes
# from learn/media/scenes/<id>.json.
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
cd "$HERE/../.."
IMAGE=renox-learn-ffmpeg:1
ENGINE=${ENGINE:-$(command -v docker || command -v podman || true)}
[ -n "$ENGINE" ] || { echo "encode.sh: neither docker nor podman is installed" >&2; exit 1; }
[ "$#" -gt 0 ] || { echo "usage: learn/media/encode.sh <id>..." >&2; exit 2; }

if ! "$ENGINE" image inspect "$IMAGE" >/dev/null 2>&1; then
  "$ENGINE" build -t "$IMAGE" -f learn/media/Containerfile.ffmpeg learn/media
fi

ff() {
  "$ENGINE" run --rm --network none --user "$(id -u):$(id -g)" \
    -v "$PWD/learn/media/out:/work" -w /work "$IMAGE" "$@"
}

if [ -z "${SKIP_ENCODER_CHECK:-}" ]; then
  ff ffmpeg -hide_banner -encoders | grep -E 'libvpx-vp9|libx264|libwebp' >/dev/null \
    || { echo "encode.sh: the image lacks libvpx-vp9, libx264 or libwebp" >&2; exit 1; }
fi

VF="fps=30,scale=1280:720:flags=lanczos,format=yuv420p"
for id in "$@"; do
  [[ "$id" =~ ^s[0-9]{2}-[a-z0-9-]{1,40}$ ]] || { echo "encode.sh: bad scene id $id" >&2; exit 2; }
  frames="learn/media/out/frames/$id/frames.txt"
  [ -f "$frames" ] || { echo "encode.sh: $frames is missing: run record.mjs $id first" >&2; exit 1; }
  poster_at=$(sed -n 's/.*"poster_at"[[:space:]]*:[[:space:]]*\([0-9.]*\).*/\1/p' "learn/media/scenes/$id.json" | head -1)
  poster_at=${poster_at:-1}
  echo "== $id"
  ff ffmpeg -y -loglevel error -f concat -safe 0 -i "frames/$id/frames.txt" -vf "$VF" \
    -c:v libvpx-vp9 -b:v 0 -crf 36 -row-mt 1 -deadline good -cpu-used 2 -an "$id.webm"
  ff ffmpeg -y -loglevel error -f concat -safe 0 -i "frames/$id/frames.txt" -vf "$VF" \
    -c:v libx264 -preset slow -crf 24 -profile:v high -movflags +faststart -an "$id.mp4"
  ff ffmpeg -y -loglevel error -ss "$poster_at" -i "$id.mp4" -frames:v 1 -c:v libwebp -quality 80 "$id.webp"
  ff ffprobe -v error -show_entries format=duration -of csv=p=0 "$id.mp4" > "learn/media/out/$id.duration"
  size=$(stat -c %s "learn/media/out/$id.mp4")
  if [ "$size" -gt 4194304 ]; then
    echo "note: $id.mp4 is $size bytes (over 4 MB)"
  fi
done
