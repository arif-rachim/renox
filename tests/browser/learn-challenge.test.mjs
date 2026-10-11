// #531: a challenge on a stage page (/stages/models, cheap-items), under
// CSP=strict. The "I passed it" button is hidden without JavaScript and shown
// with it; it toggles aria-pressed and dispatches learn:challenge-done with
// the challenge and the state (progress.js stores it, #535). The Copy buttons
// copy the code. No overflow at 1280 and 390 px, light and dark, with the
// solution closed and open. Screenshots go to LEARN_SCREENS when it's set.

import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { Browser } from './lib/cdp.mjs';
import { start } from './lib/app.mjs';

let browser;
let app;

before(async () => {
  browser = await Browser.launch();
  app = await start('renox-learn', 'learn', { env: { CSP: 'strict', QUEUE_WORKERS: '0', SCHEDULER: 'false' } });
});

after(async () => {
  await app?.stop();
  await browser?.close();
});

const PAGE = '/stages/models';
const SECTION = `document.querySelector('section.learn-challenge[data-challenge="s02/cheap-items"]')`;
const BUTTON = `${SECTION}.querySelector('[data-challenge-done]')`;

test('the page has the challenge and it is complete', () =>
  browser.with(async (page) => {
    await page.goto(`${app.url}${PAGE}`);
    assert.equal(await page.eval(`document.querySelectorAll('.learn-problem').length`), 0);
    assert.equal(await page.eval(`${SECTION}.id`), 'challenge-cheap-items');
    assert.match(await page.eval(`${SECTION}.querySelector('h2').textContent`), /Only the cheap items/);
    assert.equal(await page.eval(`${SECTION}.querySelector('.eyebrow b').textContent`), '02');
    const steps = await page.eval(`[...${SECTION}.querySelectorAll('h3')].map((h) => h.textContent)`);
    assert.equal(steps.length, 4, steps.join('|'));
    assert.match(steps[0], /^1\. Start from your stage 2 app/);
    assert.equal(await page.eval(`${SECTION}.querySelectorAll('details.learn-challenge__output').length`), 2);
    assert.equal(await page.eval(`${SECTION}.querySelectorAll('details.learn-challenge__solution').length`), 1);
    assert.equal(await page.eval(`${SECTION}.querySelector('details.learn-challenge__solution').open`), false);
    assert.match(await page.eval(`${SECTION}.textContent`), /cargo test --test challenge_cheap_items/);
    page.assertClean();
  }));

test('the button shows with JavaScript and toggles with an event', () =>
  browser.with(async (page) => {
    await page.goto(`${app.url}${PAGE}`);
    await page.eval(() => localStorage.clear());
    await page.goto(`${app.url}${PAGE}`);
    try {
      // The page is long: a smooth scroll to the button would still be going when the click lands.
      await page.eval(() => { document.documentElement.style.scrollBehavior = 'auto'; });
      assert.equal(await page.eval(`${BUTTON}.hidden`), false, 'challenge.js shows the button');
      assert.equal(await page.eval(`${BUTTON}.getAttribute('aria-pressed')`), 'false');
      await page.eval(() => {
        window.__events = [];
        document.addEventListener('learn:challenge-done', (e) => window.__events.push(e.detail));
      });
      await page.click('section[data-challenge="s02/cheap-items"] [data-challenge-done]');
      assert.equal(await page.eval(`${BUTTON}.getAttribute('aria-pressed')`), 'true');
      assert.equal(await page.eval(`${SECTION}.classList.contains('is-passed')`), true);
      await page.click('section[data-challenge="s02/cheap-items"] [data-challenge-done]');
      assert.equal(await page.eval(`${BUTTON}.getAttribute('aria-pressed')`), 'false');
      assert.equal(await page.eval(`${SECTION}.classList.contains('is-passed')`), false);
      assert.deepEqual(await page.eval(() => window.__events), [
        { challenge: 's02/cheap-items', done: true },
        { challenge: 's02/cheap-items', done: false },
      ]);
      page.assertClean();
    } finally {
      await page.eval(() => localStorage.clear());
    }
  }));

test('the button is hidden when JavaScript is off', () =>
  browser.with(async (page) => {
    await page.send('Emulation.setScriptExecutionDisabled', { value: true });
    await page.goto(`${app.url}${PAGE}`);
    await page.send('Emulation.setScriptExecutionDisabled', { value: false });
    // The script never ran: the attribute in the HTML is still there.
    assert.equal(await page.eval(`${BUTTON}.hasAttribute('hidden')`), true);
    assert.equal(await page.eval(`getComputedStyle(${BUTTON}).display`), 'none');
  }));

test('the copy buttons copy the code of their panel', () =>
  browser.with(async (page) => {
    await page.goto(`${app.url}${PAGE}`);
    await page.eval(() => {
      window.__copied = [];
      navigator.clipboard.writeText = (text) => { window.__copied.push(text); return Promise.resolve(); };
    });
    const count = await page.eval(`${SECTION}.querySelectorAll('[data-copy]').length`);
    assert.ok(count >= 6, `${count} copy buttons`);
    // The command (step 4), then the test file (step 3).
    const labels = await page.eval(
      `[...${SECTION}.querySelectorAll('.site-code')].map((c) => c.querySelector('code').innerText.slice(0, 30))`,
    );
    const at = labels.findIndex((l) => l.startsWith('cargo test --test'));
    assert.ok(at >= 0, labels.join('|'));
    await page.eval(`${SECTION}.querySelectorAll('[data-copy]')[${at}].click(); true`);
    await page.waitFor(() => window.__copied.length === 1);
    assert.equal(await page.eval(() => window.__copied[0]), 'cargo test --test challenge_cheap_items');
    assert.equal(await page.eval(`${SECTION}.querySelectorAll('.site-code__copy-label')[${at}].textContent`), 'Copied');
    page.assertClean();
  }));

for (const [size, width] of [['1280', 1280], ['390', 390]]) {
  for (const mode of ['light', 'dark']) {
    test(`the challenge looks right (${size}, ${mode})`, () =>
      browser.with(
        async (page) => {
          await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: mode }] });
          await page.goto(`${app.url}${PAGE}`);
          await page.eval(() => document.fonts.ready.then(() => true));
          await page.eval(() => { document.documentElement.style.scrollBehavior = 'auto'; });
          const shoot = async (name) => {
            const overflow = await page.eval(`document.documentElement.scrollWidth - document.documentElement.clientWidth`);
            assert.equal(overflow, 0, `${name}: horizontal overflow`);
            const inside = await page.eval(
              `[...${SECTION}.querySelectorAll('*')].filter((e) => e.getBoundingClientRect().right > ${SECTION}.getBoundingClientRect().right + 1 && !e.closest('pre')).length`,
            );
            assert.equal(inside, 0, `${name}: something sticks out of the section`);
            if (process.env.LEARN_SCREENS) {
              await page.eval(`${SECTION}.scrollIntoView(); true`);
              const box = await page.eval(`(() => { const r = ${SECTION}.getBoundingClientRect(); return r.height; })()`);
              await page.send('Emulation.setDeviceMetricsOverride', {
                width,
                height: Math.min(Math.ceil(box) + 80, 8000),
                deviceScaleFactor: 1,
                mobile: width < 600,
              });
              await page.eval(`window.scrollTo(0, ${SECTION}.getBoundingClientRect().top + window.scrollY - 20); true`);
              await page.screenshot(join(process.env.LEARN_SCREENS, `challenge-${name}-${size}-${mode}.png`));
              await page.send('Emulation.clearDeviceMetricsOverride');
            }
          };
          await shoot('closed');
          await page.eval(`document.querySelectorAll('.learn-challenge details').forEach((d) => { d.open = true; }); true`);
          await shoot('open');
          page.assertClean();
        },
        { width, height: 900 },
      ));
  }
}
