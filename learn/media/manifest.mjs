// Writes learn/content/media.json from what encode.sh left in learn/media/out.
//
//   node learn/media/manifest.mjs [id...]    the ids named, else every encoded one
//
// Entries of other ids are kept as they are; the file is sorted by id. Run it
// after encode.sh and commit the result (the videos themselves are uploaded,
// never committed).

import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkScene, sha256 } from './scene.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const OUT = join(HERE, 'out');
export const MANIFEST = resolve(HERE, '..', 'content', 'media.json');

const file = (name) => {
  const bytes = readFileSync(join(OUT, name));
  return { file: name, bytes: bytes.length, sha256: sha256(bytes) };
};

/** The manifest entry of an encoded scene. */
export function entry(id, today = new Date().toISOString().slice(0, 10)) {
  const scenePath = join(HERE, 'scenes', `${id}.json`);
  const sceneBytes = readFileSync(scenePath);
  const scene = checkScene(JSON.parse(sceneBytes.toString('utf8')), `${id}.json`);
  for (const ext of ['webm', 'mp4', 'webp', 'duration']) {
    if (!existsSync(join(OUT, `${id}.${ext}`))) throw new Error(`out/${id}.${ext} is missing: run encode.sh ${id}`);
  }
  const duration = Number.parseFloat(readFileSync(join(OUT, `${id}.duration`), 'utf8'));
  if (!(duration > 0)) throw new Error(`out/${id}.duration is not a number`);
  return {
    stage: scene.stage,
    title: scene.title,
    width: scene.viewport.width,
    height: scene.viewport.height,
    duration: Math.round(duration * 10) / 10,
    poster: file(`${id}.webp`),
    sources: [
      { ...file(`${id}.webm`), type: 'video/webm' },
      { ...file(`${id}.mp4`), type: 'video/mp4' },
    ],
    scene_sha256: sha256(sceneBytes),
    recorded_at: today,
  };
}

/** `current` with the entries of `ids` replaced, sorted by id. */
export function merge(current, ids, make = entry) {
  const videos = { ...current.videos };
  for (const id of ids) videos[id] = make(id);
  const sorted = Object.fromEntries(Object.keys(videos).sort().map((k) => [k, videos[k]]));
  return { version: 1, videos: sorted };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  let ids = process.argv.slice(2);
  if (ids.length === 0) {
    ids = readdirSync(OUT).filter((f) => f.endsWith('.mp4') && statSync(join(OUT, f)).isFile()).map((f) => f.slice(0, -4));
  }
  const current = existsSync(MANIFEST) ? JSON.parse(readFileSync(MANIFEST, 'utf8')) : { version: 1, videos: {} };
  const next = merge(current, ids);
  writeFileSync(MANIFEST, `${JSON.stringify(next, null, 2)}\n`);
  console.log(`${MANIFEST}: ${Object.keys(next.videos).length} videos (${ids.join(', ') || 'none re-encoded'})`);
}
