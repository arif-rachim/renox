// The bike shop's skeleton (#232, part 1) in a browser: the public and staff
// layouts at desktop and phone width, the "About this page" panel, the index
// of every page, Motion (vendored) under the default CSP and CSP=strict, and
// prefers-reduced-motion. Screenshots go to BIKESHOP_SCREENS when it's set.

import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { Browser } from './lib/cdp.mjs';
import { start } from './lib/app.mjs';

const PHONE = { width: 390, height: 844 };
let browser;

before(async () => {
  browser = await Browser.launch();
});

after(async () => {
  await browser?.close();
});

async function shot(page, name) {
  if (process.env.BIKESHOP_SCREENS) await page.screenshot(join(process.env.BIKESHOP_SCREENS, `${name}.png`));
}

/** No sideways scrolling: nothing is wider than the viewport. */
const fitsWidth = (page) => page.eval(() => document.documentElement.scrollWidth <= window.innerWidth);

/** Waits until the content Motion slides in is in place. */
const revealed = (page) =>
  page.waitFor(() =>
    [...document.querySelectorAll('[data-bs-reveal] > *')].every(
      (el) =>
        getComputedStyle(el).opacity === '1' &&
        ['none', 'matrix(1, 0, 0, 1, 0, 0)'].includes(getComputedStyle(el).transform),
    ),
  );

/** Opens the panel with its button and checks what it shows. */
async function openPanel(page, title) {
  await page.click('[data-rx-open="about-page"]');
  await page.waitFor(() => document.querySelector('#about-page')?.open, { message: 'the panel open' });
  const text = await page.text('#about-page');
  assert.ok(text.includes(title), `the panel is about "${title}"`);
  for (const part of ['Renox features used, and why', 'Under the hood', 'In the guide', 'Source files']) {
    assert.ok(text.includes(part), `the panel shows "${part}"`);
  }
  // Its entries slide in with Motion and end fully shown.
  await page.waitFor(() =>
    [...document.querySelectorAll('#about-page .rx-infolist > .rx-entry')].every(
      (el) => getComputedStyle(el).opacity === '1',
    ),
  );
}

for (const csp of ['relaxed', 'strict']) {
  describe(`bikeshop under CSP=${csp}`, () => {
    let app;
    before(async () => {
      app = await start('bikeshop', 'examples/bikeshop', { env: { CSP: csp } });
    });
    after(() => app?.stop());

    for (const [size, options] of [
      ['desktop', undefined],
      ['phone', PHONE],
    ]) {
      test(`the home page and its panel (${size})`, () =>
        browser.with(async (page) => {
          await page.goto(`${app.url}/`);
          assert.equal(await page.eval(() => typeof window.Motion?.animate), 'function', 'Motion is loaded');
          // The content slid in and ended where it belongs.
          await revealed(page);
          assert.ok(await fitsWidth(page), 'no sideways scrolling');
          await shot(page, `home-${size}-${csp}`);
          await openPanel(page, 'Home');
          await shot(page, `home-panel-${size}-${csp}`);
          await page.press('Escape');
          await page.waitFor(() => !document.querySelector('#about-page').open, { message: 'Esc closes it' });
          page.assertClean();
        }, options));
    }

    test('the index of every page filters by feature', () =>
      browser.with(async (page) => {
        await page.goto(`${app.url}/about/pages`);
        const all = await page.eval(() => document.querySelectorAll('#about-pages > li').length);
        assert.ok(all >= 10, `${all} pages listed`);
        await revealed(page);
        await shot(page, `about-pages-${csp}`);
        // A feature's badge is a link to the pages that use it.
        await page.click('#about-pages a[href*="feature=App%3A%3Adetect_locale"]');
        await page.waitFor(() => location.search.includes('feature='));
        const shown = await page.eval(() => document.querySelectorAll('#about-pages > li').length);
        assert.ok(shown >= 1 && shown < all, `${shown} of ${all} pages use it`);
        assert.ok(await fitsWidth(page));
        page.assertClean();
      }));

    test('the staff layout, after registering (desktop and phone)', () =>
      browser.with(async (page) => {
        await page.goto(`${app.url}/register`);
        await page.type('#rx-name', 'Sam Staff');
        await page.type('#rx-email', `sam-${csp}@example.com`);
        await page.type('#rx-password', 'secret123');
        await page.type('#rx-password_confirmation', 'secret123');
        await page.click('form button[type=submit]');
        await page.waitFor(() => location.pathname === '/');
        await page.goto(`${app.url}/staff`);
        assert.ok(await page.eval(() => document.body.classList.contains('rx-shell')));
        assert.ok(await fitsWidth(page));
        await revealed(page);
        await shot(page, `staff-desktop-${csp}`);
        await openPanel(page, 'Staff dashboard');
        await page.press('Escape');
        await page.send('Emulation.setDeviceMetricsOverride', { ...PHONE, deviceScaleFactor: 1, mobile: true });
        await page.goto(`${app.url}/staff`);
        assert.ok(await fitsWidth(page), 'no sideways scrolling on a phone');
        await revealed(page);
        await shot(page, `staff-phone-${csp}`);
        page.assertClean();
      }));
  });
}

describe('bikeshop with prefers-reduced-motion', () => {
  let app;
  before(async () => {
    app = await start('bikeshop', 'examples/bikeshop');
  });
  after(() => app?.stop());

  test('nothing moves, everything shows', () =>
    browser.with(async (page) => {
      await page.send('Emulation.setEmulatedMedia', {
        features: [{ name: 'prefers-reduced-motion', value: 'reduce' }],
      });
      await page.goto(`${app.url}/`);
      // Motion never touched the content: no inline styles left behind.
      assert.equal(
        await page.eval(() => [...document.querySelectorAll('[data-bs-reveal] > *')].filter((el) => el.style.opacity || el.style.transform).length),
        0,
      );
      page.assertClean();
    }));

  test('BIKESHOP_EXPLAIN=false hides the panel', async () => {
    const quiet = await start('bikeshop', 'examples/bikeshop', { env: { BIKESHOP_EXPLAIN: 'false' } });
    try {
      await browser.with(async (page) => {
        await page.goto(`${quiet.url}/`);
        assert.equal(await page.eval(() => document.querySelector('[data-rx-open="about-page"]')), null);
        page.assertClean();
      });
    } finally {
      await quiet.stop();
    }
  });
});
