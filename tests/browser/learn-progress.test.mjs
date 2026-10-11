// #535: learn.renox.rs keeps progress, quiz answers and challenge marks in the
// browser (localStorage["renox-learn:v1"]) and sends nothing. Both CSP modes.
// LEARN_SCREENS=<dir> saves screenshots.

import { after, before, describe, test } from 'node:test';
import { join } from 'node:path';
import assert from 'node:assert/strict';
import { Browser } from './lib/cdp.mjs';
import { start } from './lib/app.mjs';

let browser;

before(async () => {
  browser = await Browser.launch();
});

after(async () => {
  await browser?.close();
});

const KEY = 'renox-learn:v1';

// Clears the storage of the origin, then loads `path`.
async function fresh(page, app, path) {
  await page.goto(`${app.url}/`);
  await page.eval(`localStorage.clear(); true`);
  await page.goto(`${app.url}${path}`);
}

const stored = (page) => page.eval(`JSON.parse(localStorage.getItem('${KEY}'))`);

async function choose(page, value) {
  await page.eval(`document.querySelector('.learn-quiz input[value="${value}"]').click(); true`);
  await page.eval(`document.querySelector('.learn-quiz__check').click(); true`);
}

const result = (page) => page.eval(`document.querySelector('.learn-quiz__result').textContent`);

// Stage 1 (migrations) ends with a real challenge section (#531).
const CHALLENGE_PAGE = '/stages/migrations';

for (const csp of ['relaxed', 'strict']) {
  describe(`learn.renox.rs progress (CSP=${csp})`, () => {
    let app;

    before(async () => {
      app = await start('renox-learn', 'learn', { env: { CSP: csp, QUEUE_WORKERS: '0', SCHEDULER: 'false' } });
    });

    after(async () => {
      await app?.stop();
    });

    test('visiting a stage offers to continue it from the home page', () =>
      browser.with(async (page) => {
        await fresh(page, app, '/stages/setup');
        await page.goto(`${app.url}/`);
        assert.equal(await page.eval(`document.querySelector('[data-learn-continue] a').getAttribute('href')`), '/stages/setup');
        assert.equal(await page.eval(`document.querySelector('[data-learn-continue]').hidden`), false);
        assert.match(await page.eval(`document.querySelector('[data-learn-continue] span').textContent`), /^Stage 0: /);
        page.assertClean();
      }));

    test('marking a stage done survives a reload', () =>
      browser.with(async (page) => {
        await fresh(page, app, '/stages/setup');
        const button = `document.querySelector('[data-learn-done]')`;
        assert.equal(await page.eval(`${button}.hidden`), false);
        await page.eval(`${button}.click(); true`);
        assert.equal(await page.eval(`${button}.getAttribute('aria-pressed')`), 'true');
        await page.goto(`${app.url}/stages/setup`);
        assert.equal(await page.eval(`${button}.getAttribute('aria-pressed')`), 'true');
        await page.goto(`${app.url}/`);
        assert.equal(await page.eval(`document.querySelector('#setup').classList.contains('is-done')`), true);
        assert.equal(await page.eval(`document.querySelectorAll('#setup .stage__done').length`), 1);
        await page.goto(`${app.url}/stages/setup`);
        await page.eval(`${button}.click(); true`);
        assert.equal(await page.eval(`${button}.getAttribute('aria-pressed')`), 'false');
        await page.goto(`${app.url}/`);
        assert.equal(await page.eval(`document.querySelector('#setup').classList.contains('is-done')`), false);
        page.assertClean();
      }));

    test('a quiz is checked in the browser and its answer survives a reload', () =>
      browser.with(async (page) => {
        await fresh(page, app, '/stages/setup');
        await page.eval(`document.querySelector('.learn-quiz__check').click(); true`);
        assert.equal(await result(page), 'Choose an answer first.');
        await choose(page, 0);
        assert.equal(await result(page), 'Not quite. Try again.');
        assert.equal(await page.eval(`document.querySelector('.learn-quiz').classList.contains('is-wrong')`), true);
        assert.equal(await page.eval(`document.querySelector('.learn-quiz__explain').hidden`), true);
        await choose(page, 1);
        assert.equal(await result(page), 'Correct.');
        assert.equal(await page.eval(`document.querySelector('.learn-quiz__explain').hidden`), false);
        await page.goto(`${app.url}/stages/setup`);
        assert.equal(await page.eval(`document.querySelector('.learn-quiz input[value="1"]').checked`), true);
        assert.equal(await page.eval(`document.querySelector('.learn-quiz').classList.contains('is-correct')`), true);
        assert.equal(await result(page), 'Correct.');
        assert.equal(await page.eval(`document.querySelector('.learn-quiz__explain').hidden`), false);
        page.assertClean();
      }));

    test('reset forgets everything', () =>
      browser.with(async (page) => {
        await fresh(page, app, '/stages/setup');
        await page.eval(`document.querySelector('[data-learn-done]').click(); localStorage.setItem('renox-learn:theme', 'dark'); true`);
        await page.goto(`${app.url}/`);
        assert.equal(await page.eval(`document.querySelector('[data-learn-reset]').hidden`), false);
        await page.eval(`document.querySelector('[data-learn-reset] summary').click(); true`);
        await page.click('[data-learn-reset-confirm]');
        assert.equal(await page.eval(`localStorage.getItem('${KEY}')`), null);
        assert.equal(await page.eval(`localStorage.getItem('renox-learn:theme')`), 'dark');
        assert.equal(await page.eval(`document.querySelector('[data-learn-continue]').hidden`), true);
        assert.equal(await page.eval(`document.querySelectorAll('.stage.is-done').length`), 0);
        assert.equal(await page.eval(`document.querySelectorAll('.stage__done').length`), 0);
        assert.equal(await page.eval(`document.querySelector('[role=status].reset__status').textContent`), 'Your progress was cleared.');
        page.assertClean();
      }));

    test('a broken value in storage is ignored', () =>
      browser.with(async (page) => {
        await page.goto(`${app.url}/`);
        await page.eval(`localStorage.setItem('${KEY}', '{nope'); true`);
        await page.goto(`${app.url}/stages/setup`);
        assert.equal((await stored(page)).v, 1);
        await page.eval(`localStorage.setItem('${KEY}', '[1,2]'); true`);
        await page.goto(`${app.url}/stages/setup`);
        assert.equal((await stored(page)).v, 1);
        page.assertClean();
      }));

    test('unknown fields are kept when saving', () =>
      browser.with(async (page) => {
        await page.goto(`${app.url}/`);
        await page.eval(`localStorage.setItem('${KEY}', JSON.stringify({ v: 1, future: { a: 1 } })); true`);
        await page.goto(`${app.url}/stages/setup`);
        assert.deepEqual((await stored(page)).future, { a: 1 });
        page.assertClean();
      }));

    test('blocked storage does not break the page', () =>
      browser.with(async (page) => {
        await page.send('Page.addScriptToEvaluateOnNewDocument', {
          source: "Storage.prototype.getItem = Storage.prototype.setItem = Storage.prototype.removeItem = () => { throw new DOMException('blocked', 'SecurityError') }",
        });
        await page.goto(`${app.url}/stages/setup`);
        await choose(page, 1);
        assert.equal(await result(page), 'Correct.');
        const button = `document.querySelector('[data-learn-done]')`;
        await page.eval(`${button}.click(); true`);
        assert.equal(await page.eval(`${button}.getAttribute('aria-pressed')`), 'true');
        await page.eval(`${button}.click(); true`);
        assert.equal(await page.eval(`${button}.getAttribute('aria-pressed')`), 'false');
        await page.goto(`${app.url}/`);
        assert.equal(await page.eval(`document.querySelector('[data-learn-continue]').hidden`), true);
        page.assertClean();
      }));

    test('a passed challenge survives a reload', () =>
      browser.with(async (page) => {
        await fresh(page, app, CHALLENGE_PAGE);
        const button = `document.querySelector('section.learn-challenge [data-challenge-done]')`;
        const section = `document.querySelector('section.learn-challenge')`;
        assert.equal(await page.eval(`${button}.getAttribute('aria-pressed')`), 'false');
        await page.eval(`${button}.click(); true`);
        const id = await page.eval(`${section}.getAttribute('data-challenge')`);
        assert.ok((await stored(page)).challenges[id].done, 'stored with a time');
        await page.goto(`${app.url}${CHALLENGE_PAGE}`);
        assert.equal(await page.eval(`${button}.getAttribute('aria-pressed')`), 'true');
        assert.equal(await page.eval(`${section}.classList.contains('is-passed')`), true);
        await page.eval(`${button}.click(); true`);
        assert.equal((await stored(page)).challenges[id], undefined);
        await page.goto(`${app.url}${CHALLENGE_PAGE}`);
        assert.equal(await page.eval(`${button}.getAttribute('aria-pressed')`), 'false');
        assert.equal(await page.eval(`${section}.classList.contains('is-passed')`), false);
        page.assertClean();
      }));

    for (const [size, width] of [['1280', 1280], ['390', 390]]) {
      for (const mode of ['light', 'dark']) {
        test(`progress looks right (${size}, ${mode})`, () =>
          browser.with(
            async (page) => {
              await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: mode }] });
              const shoot = async (name) => {
                const overflow = await page.eval(`document.documentElement.scrollWidth - document.documentElement.clientWidth`);
                assert.equal(overflow, 0, `${name}: horizontal overflow`);
                if (process.env.LEARN_SCREENS && csp === 'strict') {
                  const height = await page.eval(`document.documentElement.scrollHeight`);
                  await page.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: width < 600 });
                  await page.screenshot(join(process.env.LEARN_SCREENS, `progress-${name}-${size}-${mode}.png`));
                }
              };
              await fresh(page, app, '/stages/setup');
              await page.eval(`document.querySelector('[data-learn-done]').click(); true`);
              await page.goto(`${app.url}/`);
              await page.eval(`document.fonts.ready.then(() => true)`);
              assert.equal(await page.eval(`document.querySelector('#setup').classList.contains('is-done')`), true);
              await shoot('home');
              await page.goto(`${app.url}/stages/setup`);
              await choose(page, 0);
              await shoot('quiz-wrong');
              await choose(page, 1);
              await shoot('quiz-correct');
              page.assertClean();
            },
            { width, height: 900 },
          ));
      }
    }
  });
}

test('progress.js sends nothing', async () => {
  const { readFile } = await import('node:fs/promises');
  const source = await readFile(new URL('../../learn/public/progress.js', import.meta.url), 'utf8');
  assert.doesNotMatch(source, /fetch|XMLHttpRequest|sendBeacon/);
});
