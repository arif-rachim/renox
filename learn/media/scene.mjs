// Scene files (learn/media/scenes/<id>.json): reading, checking, and the pure
// helpers record.mjs and manifest.mjs share. No browser in here, so
// scene.test.mjs checks it with `node --test`.

import { createHash } from 'node:crypto';

export const MAX_SECONDS = 60;
export const ID = /^s[0-9]{2}-[a-z0-9-]{1,40}$/;
const STEP_KEYS = ['goto', 'pause', 'type', 'click', 'press', 'wait', 'scroll'];
const SCENE_KEYS = ['id', 'stage', 'title', 'viewport', 'poster_at', 'login', 'steps'];

/** Typing speed: one character every 70 ms. */
export const TYPE_DELAY_MS = 70;

const isText = (v) => typeof v === 'string' && v.length > 0;

/**
 * Checks a parsed scene and returns it; throws an Error naming the file and
 * the problem. `file` is only used in messages.
 */
export function checkScene(scene, file = 'scene') {
  const fail = (why) => {
    throw new Error(`${file}: ${why}`);
  };
  if (scene === null || typeof scene !== 'object' || Array.isArray(scene)) fail('must be an object');
  for (const key of Object.keys(scene)) {
    if (!SCENE_KEYS.includes(key)) fail(`unknown key \`${key}\``);
  }
  if (typeof scene.id !== 'string' || !ID.test(scene.id)) fail('`id` must match ^s[0-9]{2}-[a-z0-9-]{1,40}$');
  if (!Number.isInteger(scene.stage) || scene.stage < 0 || scene.stage > 99) fail('`stage` must be a number from 0 to 99');
  if (scene.id.slice(1, 3) !== String(scene.stage).padStart(2, '0')) {
    fail(`\`id\` starts with s${scene.id.slice(1, 3)} but \`stage\` is ${scene.stage}`);
  }
  if (!isText(scene.title)) fail('`title` is required');
  const v = scene.viewport;
  if (!v || !Number.isInteger(v.width) || !Number.isInteger(v.height) || v.width < 320 || v.height < 240) {
    fail('`viewport` needs integer `width` and `height`');
  }
  if (scene.poster_at !== undefined && !(typeof scene.poster_at === 'number' && scene.poster_at >= 0)) {
    fail('`poster_at` must be a number of seconds');
  }
  if (scene.login !== undefined) {
    const l = scene.login;
    if (!l || !isText(l.email) || !isText(l.password)) fail('`login` needs `email` and `password`');
  }
  if (!Array.isArray(scene.steps) || scene.steps.length === 0) fail('`steps` must be a non-empty list');
  scene.steps.forEach((step, i) => {
    const at = `step ${i + 1}`;
    if (step === null || typeof step !== 'object' || Array.isArray(step)) fail(`${at} must be an object`);
    const keys = Object.keys(step);
    if (keys.length !== 1) fail(`${at} must have exactly one key (has ${keys.length})`);
    const [key] = keys;
    if (!STEP_KEYS.includes(key)) fail(`${at}: unknown step \`${key}\` (known: ${STEP_KEYS.join(', ')})`);
    const value = step[key];
    const ok =
      key === 'pause'
        ? Number.isInteger(value) && value >= 0 && value <= MAX_SECONDS * 1000
        : key === 'type'
          ? Array.isArray(value) && value.length === 2 && value.every((x) => typeof x === 'string') && isText(value[0])
          : key === 'goto'
            ? isText(value) && value.startsWith('/')
            : isText(value);
    if (!ok) fail(`${at}: bad value for \`${key}\``);
  });
  if (scene.steps[0].goto === undefined) fail('the first step must be a `goto`');
  return scene;
}

/** What the steps cost at least: pauses plus typing, in seconds. */
export function plannedSeconds(scene) {
  let ms = 0;
  for (const step of scene.steps) {
    if ('pause' in step) ms += step.pause;
    if ('type' in step) ms += step.type[1].length * TYPE_DELAY_MS;
  }
  return ms / 1000;
}

/**
 * The text of `frames.txt` for ffmpeg's concat demuxer. `frames` is a list of
 * `{ file, at }` (`at` in seconds). Each frame lasts until the next one's
 * `at`, the last lasts 1 s (or until `endAt`, when the page stood still
 * after its last frame and that is longer), and the last file is listed once more, as the
 * demuxer ignores the duration of the final entry.
 */
export function framesTxt(frames, endAt) {
  if (frames.length === 0) throw new Error('no frames were captured');
  let out = '';
  frames.forEach((frame, i) => {
    const next = frames[i + 1];
    const seconds = next ? Math.max(next.at - frame.at, 0.001) : Math.max(1, (endAt ?? 0) - frame.at);
    out += `file '${frame.file}'\nduration ${seconds.toFixed(3)}\n`;
  });
  out += `file '${frames[frames.length - 1].file}'\n`;
  return out;
}

export const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
