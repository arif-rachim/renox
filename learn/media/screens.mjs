// Takes the chapters' screenshots from the course app. Run it by hand with
// Node 24 (no npm packages), through the memory wrapper when you use one:
//
//   node learn/media/screens.mjs        every stage
//   node learn/media/screens.mjs 3 5    only stages 3 and 5
//
// The ```learn:screenshot blocks of learn/content/stages/NN-*.md are the one
// list of what to capture. Every stage is the package `coffee` and they share
// one target directory, so each stage is built on its own (`cargo clean -p
// coffee` first, as learn/course/run.sh does) and its binary is copied aside
// before the app is started. The images are 1x, viewport-sized webp files in
// learn/public/img/sNN/; they are committed, and CI never regenerates them.

import { spawnSync } from 'node:child_process';
import {
  copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Browser } from '../../tests/browser/lib/cdp.mjs';
import { ROOT, start } from '../../tests/browser/lib/app.mjs';

const LEARN = join(ROOT, 'learn');
const COURSE = join(LEARN, 'course');
const CHAPTERS = join(LEARN, 'content/stages');
const IMAGES = join(LEARN, 'public/img');
const BUDGET = 300 * 1024;
const SIZES = { desktop: [1280, 800], phone: [390, 844] };
const FREEZE = '*,*::before,*::after{animation:none!important;transition:none!important;caret-color:transparent!important}';
// The seeded users' password (learn/course/stage-NN/src/app/seed.rs).
const PASSWORD = 'password123';

/** The screenshot blocks of a chapter: `key: value` bodies. */
function directives(markdown) {
  const found = [];
  for (const match of markdown.matchAll(/^```learn:screenshot[ \t]*\n([\s\S]*?)^```/gm)) {
    const d = {};
    for (const line of match[1].split('\n')) {
      const text = line.trim();
      if (!text || text.startsWith('//')) continue;
      const at = text.indexOf(':');
      d[text.slice(0, at).trim()] = text.slice(at + 1).trim().replace(/^"(.*)"$/, '$1');
    }
    found.push(d);
  }
  return found;
}

/** Builds stage `n` alone and returns a target directory holding its binary. */
function build(n, dir) {
  const stage = join(COURSE, `stage-${n}`);
  const target = process.env.CARGO_TARGET_DIR || join(COURSE, 'target');
  const env = { ...process.env, CARGO_TARGET_DIR: target };
  for (const args of [['clean', '-q', '-p', 'coffee'], ['build', '-q', '--locked']]) {
    const run = spawnSync('cargo', args, { cwd: stage, env, stdio: 'inherit' });
    if (run.status !== 0) throw new Error(`cargo ${args.join(' ')} failed in stage-${n}`);
  }
  const name = readFileSync(join(stage, 'Cargo.toml'), 'utf8').match(/^name = "(.+)"$/m)[1];
  mkdirSync(join(dir, 'debug'), { recursive: true });
  copyFileSync(join(target, 'debug', name), join(dir, 'debug', name));
  return name;
}

async function capture(browser, base, d, out) {
  const [width, height] = SIZES[d.viewport || 'desktop'];
  await browser.with(
    async (page) => {
      await page.send('Network.clearBrowserCookies');
      if (d.login) {
        await page.goto(`${base}/login`);
        await page.type('input[name=email]', d.login);
        await page.type('input[name=password]', PASSWORD);
        await page.click('form button[type=submit]');
        await page.waitFor(() => !location.pathname.startsWith('/login'), { message: 'the login to finish' });
        await page.settle();
      }
      await page.goto(base + d.page);
      const wait = d.wait || 'main';
      await page.waitFor((s) => !!document.querySelector(s), { message: wait }, wait);
      if (d.click) {
        await page.click(d.click);
        await page.settle();
        await page.waitFor((s) => !!document.querySelector(s), { message: wait }, wait);
      }
      await page.eval(`(() => { const s = document.createElement('style'); s.textContent = ${JSON.stringify(FREEZE)}; document.head.append(s); })()`);
      await page.eval(() => document.activeElement && document.activeElement.blur && document.activeElement.blur());
      await new Promise((r) => setTimeout(r, 300));
      const { data } = await page.send('Page.captureScreenshot', { format: 'webp', quality: 80 });
      writeFileSync(out, Buffer.from(data, 'base64'));
      // A rejected form comes back as a 4xx the browser logs; nothing else is allowed.
      page.assertClean({ allow: d.click ? [/Failed to load resource/] : [] });
    },
    { width, height },
  );
}

const wanted = process.argv.slice(2).map((a) => String(Number(a)).padStart(2, '0'));
const chapters = readdirSync(CHAPTERS)
  .filter((f) => /^\d\d-.*\.md$/.test(f))
  .map((f) => ({ n: f.slice(0, 2), shots: directives(readFileSync(join(CHAPTERS, f), 'utf8')) }))
  .filter((c) => !wanted.length || wanted.includes(c.n));

const browser = await Browser.launch();
let failed = false;
try {
  for (const { n, shots } of chapters) {
    const dir = join(IMAGES, `s${n}`);
    const keep = new Set(shots.map((d) => d.file));
    if (existsSync(dir)) {
      for (const f of readdirSync(dir)) if (f.endsWith('.webp') && !keep.has(f)) rmSync(join(dir, f));
    }
    if (!shots.length) continue;
    mkdirSync(dir, { recursive: true });
    const aside = mkdtempSync(join(tmpdir(), `learn-shots-${n}-`));
    let app;
    try {
      console.log(`== stage ${n}`);
      const name = build(n, aside);
      app = await start(name, `learn/course/stage-${n}`, { seed: true, target: aside });
      for (const d of shots) {
        const out = join(dir, d.file);
        await capture(browser, app.url, d, out);
        const size = statSync(out).size;
        console.log(`${size > BUDGET ? 'TOO BIG ' : ''}${out.slice(ROOT.length + 1)}  ${(size / 1024).toFixed(1)} KB`);
        if (size > BUDGET) failed = true;
      }
    } finally {
      if (app) await app.stop();
      rmSync(aside, { recursive: true, force: true });
    }
  }
} finally {
  await browser.close();
}
if (failed) process.exit(1);
