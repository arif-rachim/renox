// Records scenes of the course app as screencast frames.
//
//   node learn/media/record.mjs [id...]     every scene when no id is given
//
// For each scene: builds its stage of the course app, starts it on a free port
// with a fresh seeded database, opens headless Chrome, and writes every
// `Page.screencastFrame` to learn/media/out/frames/<id>/f-NNNNN.jpg together
// with frames.txt for ffmpeg's concat demuxer. encode.sh turns those into
// webm, mp4 and a poster; manifest.mjs writes content/media.json.
//
// Node 24, no npm packages. Run it through the memory wrapper when you have
// one (it starts cargo and Chrome). Frames are large: encode, then delete
// learn/media/out/frames.

import { spawnSync } from 'node:child_process';
import { mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Browser, sleep } from '../../tests/browser/lib/cdp.mjs';
import { start } from '../../tests/browser/lib/app.mjs';
import { MAX_SECONDS, TYPE_DELAY_MS, checkScene, framesTxt } from './scene.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const LEARN = resolve(HERE, '..');
const REPO = resolve(LEARN, '..');
const OUT = join(HERE, 'out');
/** Every stage is the package `coffee`, built into one shared target. */
const BINARY = 'coffee';
const TARGET = process.env.CARGO_TARGET_DIR || join(LEARN, 'course', 'target');

export function loadScenes(ids) {
  const dir = join(HERE, 'scenes');
  const all = readdirSync(dir)
    .filter((f) => f.endsWith('.json'))
    .sort()
    .map((f) => {
      const scene = checkScene(JSON.parse(readFileSync(join(dir, f), 'utf8')), f);
      if (`${scene.id}.json` !== f) throw new Error(`${f}: the file must be named after its id (${scene.id}.json)`);
      return scene;
    });
  if (ids.length === 0) return all;
  return ids.map((id) => {
    const scene = all.find((s) => s.id === id);
    if (!scene) throw new Error(`no scene ${id} (have: ${all.map((s) => s.id).join(', ')})`);
    return scene;
  });
}

/** Builds a stage of the course app. Cargo tells stages apart by file times, hence the clean. */
function build(stage) {
  const dir = join(LEARN, 'course', `stage-${String(stage).padStart(2, '0')}`);
  const env = { ...process.env, CARGO_TARGET_DIR: TARGET };
  for (const args of [['clean', '-q', '-p', BINARY], ['build', '-q', '--locked']]) {
    const run = spawnSync('cargo', args, { cwd: dir, env, stdio: 'inherit' });
    if (run.status !== 0) throw new Error(`cargo ${args[0]} failed in ${dir}`);
  }
}

async function typeSlowly(page, selector, text) {
  await page.focus(selector);
  for (const ch of text) {
    await page.send('Input.insertText', { text: ch });
    await sleep(TYPE_DELAY_MS);
  }
}

async function runStep(page, step) {
  const [key] = Object.keys(step);
  const value = step[key];
  switch (key) {
    case 'goto':
      return page.goto(`${page.base}${value}`);
    case 'pause':
      return sleep(value);
    case 'type':
      return typeSlowly(page, value[0], value[1]);
    case 'click':
      await page.click(value);
      return page.settle();
    case 'press':
      return page.press(value);
    case 'wait':
      return page.waitFor((s) => !!document.querySelector(s), { message: value }, value);
    case 'scroll':
      return page.eval((s) => document.querySelector(s).scrollIntoView({ block: 'center' }), value);
  }
}

async function record(scene) {
  const frameDir = join(OUT, 'frames', scene.id);
  rmSync(frameDir, { recursive: true, force: true });
  mkdirSync(frameDir, { recursive: true });
  build(scene.stage);
  const stageDir = `learn/course/stage-${String(scene.stage).padStart(2, '0')}`;
  const app = await start(BINARY, stageDir, { seed: true, target: TARGET, env: { QUEUE_WORKERS: '0', SCHEDULER: 'false' } });
  const browser = await Browser.launch();
  try {
    await browser.with(
      async (page) => {
        page.base = app.url;
        if (scene.login) {
          // Signing in is not part of the video.
          await page.goto(`${app.url}/login`);
          await typeSlowly(page, 'input[name=email]', scene.login.email);
          await typeSlowly(page, 'input[name=password]', scene.login.password);
          await page.press('Enter');
          await page.waitFor(() => !location.pathname.startsWith('/login'), { message: 'to leave /login' });
          await page.settle();
        }
        await runStep(page, scene.steps[0]);
        const frames = [];
        let last = null;
        let writing = Promise.resolve();
        page.on('Page.screencastFrame', ({ data, metadata, sessionId }) => {
          const file = `f-${String(frames.length + 1).padStart(5, '0')}.jpg`;
          frames.push({ file, at: metadata.timestamp });
          last = { at: metadata.timestamp, wall: Date.now() };
          writing = writing.then(() => writeFileSync(join(frameDir, file), Buffer.from(data, 'base64')));
          // Without the ack Chrome stops sending frames.
          page.send('Page.screencastFrameAck', { sessionId }).catch(() => {});
        });
        const { width, height } = scene.viewport;
        const began = Date.now();
        await page.send('Page.startScreencast', { format: 'jpeg', quality: 90, maxWidth: width, maxHeight: height, everyNthFrame: 1 });
        for (const step of scene.steps.slice(1)) {
          await runStep(page, step);
          if (Date.now() - began > MAX_SECONDS * 1000) throw new Error(`${scene.id} ran over ${MAX_SECONDS} s`);
        }
        await page.send('Page.stopScreencast');
        await writing;
        page.assertClean();
        // A still page sends no frames: the last one lasts until the steps ended.
        const endAt = last ? last.at + (Date.now() - last.wall) / 1000 : 0;
        writeFileSync(join(frameDir, 'frames.txt'), framesTxt(frames, endAt));
        console.log(`${scene.id}: ${frames.length} frames in ${((Date.now() - began) / 1000).toFixed(1)} s`);
      },
      scene.viewport,
    );
  } finally {
    await browser.close();
    await app.stop();
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const scenes = loadScenes(process.argv.slice(2));
  for (const scene of scenes) await record(scene);
}
