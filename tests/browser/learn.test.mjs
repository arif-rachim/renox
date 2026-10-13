// #523: learn.renox.rs's skeleton. The home page lists every stage, loads
// with no console errors or CSP violations (also under CSP=strict), and
// has no horizontal scroll on a phone. #524: a stage page (/stages/setup) in
// both widths and both colour modes. LEARN_SCREENS=<dir> saves screenshots.

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

for (const csp of ['relaxed', 'strict']) {
  describe(`learn.renox.rs (CSP=${csp})`, () => {
    let app;

    before(async () => {
      app = await start('renox-learn', 'learn', { env: { CSP: csp, QUEUE_WORKERS: '0', SCHEDULER: 'false' } });
    });

    after(async () => {
      await app?.stop();
    });

    test('the home page lists the stages and loads cleanly', () =>
      browser.with(async (page) => {
        await page.goto(`${app.url}/`);
        const stages = await page.eval(`document.querySelectorAll('.stage').length`);
        assert.equal(stages, 9);
        const h1 = await page.eval(`document.querySelector('h1').textContent`);
        assert.match(h1, /one stage at a time/);
        // The stylesheet applied: the numbers are drawn as circles.
        const radius = await page.eval(`getComputedStyle(document.querySelector('.stage__number')).borderRadius`);
        assert.equal(radius, '50%');
        page.assertClean();
      }));

    for (const [size, width] of [['desktop', 1280], ['phone', 375], ['390', 390]]) {
      for (const mode of ['light', 'dark']) {
        test(`a stage page loads cleanly (${size}, ${mode})`, () =>
          browser.with(
            async (page) => {
              await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: mode }] });
              await page.goto(`${app.url}/stages/setup`);
              await page.eval(() => document.fonts.ready.then(() => true));
              assert.equal(await page.eval(`document.querySelectorAll('h1').length`), 1);
              assert.ok(await page.eval(`document.querySelectorAll('.prose .site-code').length >= 2`), 'code panels');
              assert.equal(await page.eval(`document.querySelectorAll('.prose form.learn-quiz[data-quiz]').length`), 1);
              assert.equal(await page.eval(`document.querySelector('.learn-quiz__check').disabled`), false, 'progress.js enables Check');
              assert.ok(await page.eval(`document.querySelectorAll('.toc a').length >= 2`), 'a table of contents');
              // The table of contents sits beside the text from 900 px, above it on a phone.
              const beside = await page.eval(
                `document.querySelector('.toc').getBoundingClientRect().top < document.querySelector('.prose').getBoundingClientRect().top + 5 && document.querySelector('.toc').getBoundingClientRect().right <= document.querySelector('.prose').getBoundingClientRect().left`,
              );
              assert.equal(beside, width >= 900);
              // The copy button is shown (learn.js ran).
              assert.equal(await page.eval(`document.querySelector('[data-copy]').hidden`), false);
              const overflow = await page.eval(`document.documentElement.scrollWidth - document.documentElement.clientWidth`);
              assert.equal(overflow, 0, 'horizontal overflow');
              if (process.env.LEARN_SCREENS && csp === 'strict') {
                const height = await page.eval(`document.documentElement.scrollHeight`);
                await page.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: width < 600 });
                await page.screenshot(join(process.env.LEARN_SCREENS, `stage-${size}-${mode}.png`));
              }
              page.assertClean();
            },
            { width, height: 900 },
          ));
      }
    }

    // #525: code from the course app and the "What changed" list, opened.
    for (const [size, width] of [['1280', 1280], ['390', 390]]) {
      for (const mode of ['light', 'dark']) {
        test(`the migrations chapter shows code and changes (${size}, ${mode})`, () =>
          browser.with(
            async (page) => {
              await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: mode }] });
              await page.goto(`${app.url}/stages/migrations`);
              await page.eval(() => document.fonts.ready.then(() => true));
              assert.equal(await page.eval(`document.querySelectorAll('.learn-problem').length`), 0);
              // Included files are named above their code.
              assert.ok(await page.eval(`document.querySelectorAll('figure.learn-code .site-code').length >= 4`), 'included code');
              assert.match(
                await page.eval(`document.querySelector('figure.learn-code figcaption code').textContent`),
                /^[a-z_/.0-9]+$/,
              );
              // Open every change.
              const changes = await page.eval(`document.querySelectorAll('details.learn-change').length`);
              assert.ok(changes >= 6, `${changes} changed files`);
              await page.eval(`document.querySelectorAll('details.learn-change').forEach((d) => { d.open = true; }); true`);
              const kinds = await page.eval(
                `[...document.querySelectorAll('.learn-change__kind')].map((e) => e.textContent).filter((v, i, all) => all.indexOf(v) === i).sort().join()`,
              );
              assert.equal(kinds, 'added,changed');
              assert.ok(await page.eval(`document.querySelectorAll('.learn-diff .add').length > 20`), 'added lines');
              // A diff scrolls inside itself; the page does not.
              const overflow = await page.eval(`document.documentElement.scrollWidth - document.documentElement.clientWidth`);
              assert.equal(overflow, 0, 'horizontal overflow');
              if (process.env.LEARN_SCREENS && csp === 'strict') {
                await page.eval(`document.querySelector('.learn-changes').scrollIntoView(); true`);
                const height = await page.eval(`document.documentElement.scrollHeight`);
                await page.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: width < 600 });
                await page.screenshot(join(process.env.LEARN_SCREENS, `migrations-${size}-${mode}.png`));
              }
              page.assertClean();
            },
            { width, height: 900 },
          ));
      }
    }

    // #533: a screenshot never makes the page wider than the phone.
    test('a screenshot fits a phone', () =>
      browser.with(
        async (page) => {
          await page.goto(`${app.url}/stages/pages`);
          const figures = await page.eval(
            `[...document.querySelectorAll('figure.learn-shot')].map((f) => Math.round(f.getBoundingClientRect().right))`,
          );
          assert.ok(figures.length >= 2, 'the pages chapter has screenshots');
          assert.ok(figures.every((right) => right <= 390), `a figure is wider than the viewport: ${figures}`);
          page.assertClean();
        },
        { width: 390, height: 800 },
      ));

    // #534: a video fits a phone and keeps its 16:9 shape. Its files aren't on
    // this test's disk, so the poster's 404 is the only problem allowed.
    test('a video fits a phone and keeps 16:9', () =>
      browser.with(
        async (page) => {
          await page.goto(`${app.url}/stages/forms`);
          const boxes = await page.eval(
            `[...document.querySelectorAll('figure.learn-video video')].map((v) => { const r = v.getBoundingClientRect(); return [Math.round(r.right), r.width / r.height]; })`,
          );
          assert.ok(boxes.length >= 1, 'the forms chapter has a video');
          for (const [right, ratio] of boxes) {
            assert.ok(right <= 390, `a video is wider than the viewport: ${right}`);
            assert.ok(Math.abs(ratio - 16 / 9) < 0.02, `a video is not 16:9: ${ratio}`);
          }
          const overflow = await page.eval(`document.documentElement.scrollWidth - document.documentElement.clientWidth`);
          assert.equal(overflow, 0, 'horizontal overflow');
          page.assertClean({ allow: [/status of 404/] });
        },
        { width: 390, height: 800 },
      ));

    test('a phone gets no horizontal scroll', () =>
      browser.with(
        async (page) => {
          await page.goto(`${app.url}/`);
          const overflow = await page.eval(`document.documentElement.scrollWidth - document.documentElement.clientWidth`);
          assert.equal(overflow, 0);
          page.assertClean();
        },
        { width: 375, height: 800 },
      ));
  });
}
