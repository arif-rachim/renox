// #543: learn.renox.rs's design system. /design shows every component in
// paper and dark mode at 1280 and 390 px with no overflow, the listed texts
// at WCAG AA, a visible focus ring, the right fonts, and no CSP violation
// (CSP=strict). The toggle overrides the system and fails safe when storage
// is blocked. Screenshots go to LEARN_SCREENS when it's set.

import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { Browser } from './lib/cdp.mjs';
import { start } from './lib/app.mjs';

// learn/src/design.rs: COMPONENTS.len(). Keep both in step.
const COMPONENTS = 31;

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

const scheme = (page, value) =>
  page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value }] });

/**
 * WCAG contrast of each selector's first match: its text colour (or the
 * property after the selector, `['.x', 'outlineColor']`) over the colours
 * painted behind it. null when nothing matches.
 */
const contrasts = (page, selectors) =>
  page.eval((list) => {
    const ctx = document.createElement('canvas').getContext('2d', { willReadFrequently: true });
    const rgba = (css) => {
      ctx.clearRect(0, 0, 1, 1);
      ctx.fillStyle = '#000';
      ctx.fillStyle = css;
      ctx.fillRect(0, 0, 1, 1);
      const [r, g, b, a] = ctx.getImageData(0, 0, 1, 1).data;
      return [r, g, b, a / 255];
    };
    const over = (top, bottom) => top.slice(0, 3).map((c, i) => c * top[3] + bottom[i] * (1 - top[3])).concat(1);
    const backdrop = (el) => {
      const layers = [];
      for (let node = el; node; node = node.parentElement) {
        const style = getComputedStyle(node);
        const bg = rgba(style.backgroundColor);
        if (style.backgroundImage !== 'none' && node !== document.body) {
          const first = style.backgroundImage.match(/(rgb|color|oklch|oklab)\([^)]*\)/);
          if (first) layers.push(rgba(first[0]));
        }
        if (bg[3] > 0) layers.push(bg);
        if (bg[3] === 1) break;
      }
      return layers.reverse().reduce((acc, layer) => over(layer, acc), [255, 255, 255, 1]);
    };
    const lum = ([r, g, b]) =>
      [r, g, b]
        .map((v) => v / 255)
        .map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4))
        .reduce((sum, v, i) => sum + v * [0.2126, 0.7152, 0.0722][i], 0);
    return list.map((item) => {
      const [selector, prop] = Array.isArray(item) ? item : [item, 'color'];
      const el = document.querySelector(selector);
      if (!el) return [selector, null];
      const bg = backdrop(el);
      const fg = over(rgba(getComputedStyle(el)[prop]), bg);
      const [a, b] = [lum(fg), lum(bg)].sort((x, y) => y - x);
      return [selector, Math.round(((a + 0.05) / (b + 0.05)) * 100) / 100];
    });
  }, selectors);

const callouts = ['note', 'tip', 'important', 'warning', 'caution'].map((k) => `.site-callout--${k} .site-callout__title`);
const TEXTS = [
  '.nav__links a', '.nav__theme', '.foot p', '.foot nav a', '.hero__lead', '.eyebrow', '.eyebrow b',
  '.stage__title a', '.stage__summary', '.stage__more', '.stage__done', '.topics code',
  '.chapter__summary', '.crumbs a', '.toc a', '.prose p', '.prose a', '.prose :not(pre) > code', 'kbd', '.prose th', 'figcaption',
  '.pager__label', '.pager__name', '.rx-button--primary', '.rx-button--secondary',
  '.site-code__lang', '.hl-com', '.hl-kw', '.learn-diff .ctx', '.learn-diff .add', '.learn-diff .del',
  ...callouts,
  '.is-correct .learn-quiz__result', '.is-wrong .learn-quiz__result', '.learn-problem',
];

const PHONE = { width: 390, height: 844 };
const DESKTOP = { width: 1280, height: 900 };

describe('learn.renox.rs design system (CSP=strict)', () => {
  for (const [size, metrics] of [['1280', DESKTOP], ['390', PHONE]]) {
    for (const mode of ['light', 'dark']) {
      test(`/design at ${size} px, ${mode}`, () =>
        browser.with(async (page) => {
          await scheme(page, mode);
          await page.goto(`${app.url}/design`);
          await page.eval(() => document.fonts.ready.then(() => true));

          // No overflow; panels, tables and diffs scroll inside themselves.
          assert.equal(await page.eval(() => document.documentElement.scrollWidth - document.documentElement.clientWidth), 0, 'page overflow');
          const wide = await page.eval(() =>
            [...document.querySelectorAll('.design__item')].filter((el) => el.scrollWidth > el.clientWidth + 1).map((el) => el.id),
          );
          assert.deepEqual(wide, [], 'sections wider than the page');

          assert.equal(await page.eval(() => document.querySelectorAll('[data-component]').length), COMPONENTS);
          assert.ok(await page.eval(() => document.fonts.check('16px Inter')), 'Inter');
          assert.ok(await page.eval(() => document.fonts.check('16px "JetBrains Mono"')), 'JetBrains Mono');

          const page_bg = await page.eval(() => getComputedStyle(document.body).backgroundColor);
          assert.equal(page_bg, mode === 'light' ? 'rgb(244, 243, 239)' : 'rgb(7, 9, 10)');
          assert.equal(await page.eval(() => getComputedStyle(document.querySelector('.nav')).backgroundColor), 'rgb(7, 9, 10)');

          const results = await contrasts(page, TEXTS);
          console.log(`${size} ${mode}:`, results.map(([s, r]) => `${s} ${r}`).join(' | '));
          const missing = results.filter(([, ratio]) => ratio === null).map(([s]) => s);
          assert.deepEqual(missing, [], 'selectors with no element');
          const low = results.filter(([, ratio]) => ratio < 4.5);
          assert.deepEqual(low, [], 'text at WCAG AA');

          // Focus: tab to the first link of the prose sample.
          await page.eval(() => document.querySelector('.prose a').scrollIntoView({ block: 'center' }));
          let focused = false;
          for (let i = 0; i < 300 && !focused; i++) {
            await page.press('Tab');
            focused = await page.eval(() => document.activeElement === document.querySelector('.prose a'));
          }
          assert.ok(focused, 'tabbed to the prose link');
          const [[, ring]] = await contrasts(page, [['.prose a', 'outlineColor']]);
          assert.ok(ring >= 3, `focus ring contrast ${ring}`);

          if (process.env.LEARN_SCREENS) {
            await page.eval(() => window.scrollTo(0, 0));
            const height = await page.eval(() => document.documentElement.scrollHeight);
            await page.send('Emulation.setDeviceMetricsOverride', { width: metrics.width, height, deviceScaleFactor: 1, mobile: metrics.width < 600 });
            await page.screenshot(join(process.env.LEARN_SCREENS, `design-${size}-${mode}.png`));
          }
          page.assertClean();
        }, metrics));
    }
  }

  test('the toggle overrides the system and survives a reload', () =>
    browser.with(async (page) => {
      await scheme(page, 'light');
      await page.goto(`${app.url}/design`);
      try {
        await page.click('[data-theme-toggle]');
        assert.equal(await page.eval(() => document.documentElement.dataset.theme), 'dark');
        assert.equal(await page.eval(() => getComputedStyle(document.body).backgroundColor), 'rgb(7, 9, 10)');
        assert.equal(await page.eval(() => document.querySelector('[data-theme-toggle]').getAttribute('aria-pressed')), 'true');
        await page.goto(`${app.url}/design`);
        assert.equal(await page.eval(() => document.documentElement.dataset.theme), 'dark');
        assert.equal(await page.eval(() => getComputedStyle(document.body).backgroundColor), 'rgb(7, 9, 10)');
        page.assertClean();
      } finally {
        await page.eval(() => localStorage.clear());
      }
    }));

  test('the toggle still switches the page when storage is blocked', () =>
    browser.with(async (page) => {
      await scheme(page, 'light');
      await page.send('Page.addScriptToEvaluateOnNewDocument', {
        source: `Storage.prototype.setItem = function () { throw new Error('blocked'); };`,
      });
      await page.goto(`${app.url}/design`);
      await page.click('[data-theme-toggle]');
      assert.equal(await page.eval(() => document.documentElement.dataset.theme), 'dark');
      assert.equal(await page.eval(() => getComputedStyle(document.body).backgroundColor), 'rgb(7, 9, 10)');
      page.assertClean();
    }));

  test('the stage numbers are circles and a done stage is filled', () => {
    return Promise.all(
      ['light', 'dark'].map((mode) =>
        browser.with(async (page) => {
          await scheme(page, mode);
          await page.goto(`${app.url}/design`);
          assert.equal(await page.eval(() => getComputedStyle(document.querySelector('.stage__number')).borderRadius), '50%');
          const done = await page.eval(() => getComputedStyle(document.querySelector('.stage.is-done .stage__number')).backgroundColor);
          assert.equal(done, mode === 'light' ? 'rgb(10, 107, 68)' : 'rgb(61, 220, 151)');
          page.assertClean();
        }),
      ),
    );
  });

  test('print hides the frame', () =>
    browser.with(async (page) => {
      await page.goto(`${app.url}/design`);
      await page.send('Emulation.setEmulatedMedia', { media: 'print' });
      for (const selector of ['.nav', '.foot', '.toc', '.pager']) {
        assert.equal(await page.eval((s) => getComputedStyle(document.querySelector(s)).display, selector), 'none', selector);
      }
    }));

  test('reduced motion keeps the page still', () =>
    browser.with(async (page) => {
      await page.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] });
      await page.goto(`${app.url}/design`);
      assert.equal(await page.eval(() => getComputedStyle(document.documentElement).scrollBehavior), 'auto');
      await page.hover('.stage:has(.stage__title a)');
      await new Promise((r) => setTimeout(r, 300));
      assert.equal(await page.eval(() => getComputedStyle(document.querySelector('.stage:has(.stage__title a)')).transform), 'none');
    }));
});
