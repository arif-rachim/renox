// node --test learn/media/scene.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkScene, framesTxt, plannedSeconds, MAX_SECONDS } from './scene.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const good = () => ({
  id: 's05-add-item',
  stage: 5,
  title: 'T',
  viewport: { width: 1280, height: 720 },
  steps: [{ goto: '/menu/new' }, { type: ['#name', 'ab'] }, { pause: 100 }],
});

test('a good scene passes', () => {
  assert.equal(checkScene(good()).id, 's05-add-item');
  assert.equal(plannedSeconds(good()), 0.24);
});

test('bad scenes are refused with the file and the reason', () => {
  const bad = (change, message) => {
    const s = good();
    change(s);
    assert.throws(() => checkScene(s, 'x.json'), message);
  };
  bad((s) => (s.id = 'S5-x'), /x\.json: `id` must match/);
  bad((s) => (s.stage = 6), /starts with s05 but `stage` is 6/);
  bad((s) => (s.extra = 1), /unknown key `extra`/);
  bad((s) => s.steps.push({ hover: '#a' }), /unknown step `hover`/);
  bad((s) => s.steps.push({ pause: 1, click: '#a' }), /exactly one key/);
  bad((s) => s.steps.push({ type: ['#a'] }), /bad value for `type`/);
  bad((s) => s.steps.push({ pause: -1 }), /bad value for `pause`/);
  bad((s) => (s.steps = [{ pause: 10 }]), /first step must be a `goto`/);
  bad((s) => (s.steps = []), /non-empty/);
  bad((s) => (s.steps[0] = { goto: 'menu' }), /bad value for `goto`/);
});

test('frames.txt gives each frame the gap to the next, the last one second, and repeats the last file', () => {
  const text = framesTxt([
    { file: 'f-00001.jpg', at: 10.0 },
    { file: 'f-00002.jpg', at: 10.25 },
    { file: 'f-00003.jpg', at: 11.0 },
  ]);
  assert.equal(
    text,
    "file 'f-00001.jpg'\nduration 0.250\nfile 'f-00002.jpg'\nduration 0.750\nfile 'f-00003.jpg'\nduration 1.000\nfile 'f-00003.jpg'\n",
  );
  assert.throws(() => framesTxt([]), /no frames/);
  // A still page: the last frame lasts until the end.
  assert.match(framesTxt([{ file: 'a.jpg', at: 5 }], 8.5), /duration 3\.500/);
});

test('every committed scene is valid, named after its file and at most 60 s of planned time', () => {
  const dir = join(HERE, 'scenes');
  const files = readdirSync(dir).filter((f) => f.endsWith('.json'));
  assert.ok(files.length > 0);
  for (const f of files) {
    const scene = checkScene(JSON.parse(readFileSync(join(dir, f), 'utf8')), f);
    assert.equal(`${scene.id}.json`, f);
    assert.ok(plannedSeconds(scene) < MAX_SECONDS, `${f} plans too long`);
  }
});

test('the manifest merges new entries in and keeps the others, sorted by id', async () => {
  const { merge } = await import('./manifest.mjs');
  const current = { version: 1, videos: { 's06-b': { keep: true } } };
  const next = merge(current, ['s05-a', 's06-b'], (id) => ({ made: id }));
  assert.deepEqual(Object.keys(next.videos), ['s05-a', 's06-b']);
  assert.deepEqual(next.videos['s06-b'], { made: 's06-b' });
  assert.deepEqual(merge(current, [], () => assert.fail()).videos, current.videos);
});
