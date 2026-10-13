// #545: docs.renox.rs in renox.rs's design system (the shared tokens.css,
// like learn.renox.rs). A guide and the home page at 1280 and 390 px, light
// and dark: no horizontal overflow (the sidebar included), the texts listed
// at WCAG AA (callout titles and text, links, body), a focus ring at 3:1, the
// dark top bar and the orange logo tile, and no console errors or CSP
// violations (CSP=strict). SITE_SCREENS=<dir> saves screenshots.

import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { Browser } from './lib/cdp.mjs';
import { start } from './lib/app.mjs';

let browser;
let app;

before(async () => {
  browser = await Browser.launch();
  app = await start('renox-site', 'site', { env: { CSP: 'strict', QUEUE_WORKERS: '0', SCHEDULER: 'false' } });
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

const PAGES = {
  guide: {
    path: '/docs/authorization',
    texts: [
      '.site-head__section', '.site-head__title h1', '.site-head__blurb', '.site-head__meta', '.site-prose p', '.site-prose a',
      '.site-prose :not(pre) > code', '.site-prose th', '.site-toc a', '.site-pager__label', '.site-pager__name',
      ...['note', 'tip', 'important', 'warning'].flatMap((k) => [`.site-callout--${k} .site-callout__title`, `.site-callout--${k} p:not(.site-callout__title)`]),
      '.site-code__lang', '.hl-com', '.hl-kw', '.hl-str',
      '.rx-sidebar__brand', '.site-nav-section', '.site-nav-link', '.site-nav-link[aria-current]', '.site-top-link',
    ],
    focus: '.site-prose a',
  },
  home: {
    path: '/',
    texts: [
      '.site-hero__eyebrow', '.site-hero__title', '.site-hero__title em', '.site-hero__lead', '.site-home__heading',
      '.site-home__note', '.site-home__note a', '.site-card strong', '.site-card__blurb',
      '.rx-button--primary', '.rx-button--secondary', '.rx-button--plain', '.site-top-link',
    ],
    focus: '.site-card',
  },
};

describe('docs.renox.rs design system (CSP=strict)', () => {
  for (const [name, spec] of Object.entries(PAGES)) {
    for (const width of [1280, 390]) {
      for (const mode of ['light', 'dark']) {
        test(`${name} at ${width} px, ${mode}`, () =>
          browser.with(
            async (page) => {
              await scheme(page, mode);
              await page.goto(`${app.url}${spec.path}`);
              await page.eval(() => document.fonts.ready.then(() => true));

              assert.equal(await page.eval(() => document.documentElement.scrollWidth - document.documentElement.clientWidth), 0, 'page overflow');
              const sidebar = await page.eval(() => {
                const s = document.querySelector('.rx-sidebar');
                return s.scrollWidth - s.clientWidth;
              });
              assert.ok(sidebar <= 1, `the sidebar scrolls sideways by ${sidebar}px`);
              assert.ok(await page.eval(() => document.fonts.check('16px Inter')), 'Inter');
              assert.ok(await page.eval(() => document.fonts.check('16px "JetBrains Mono"')), 'JetBrains Mono');

              const bg = (selector) => page.eval((s) => getComputedStyle(document.querySelector(s)).backgroundColor, selector);
              assert.equal(await bg('body'), mode === 'light' ? 'rgb(244, 243, 239)' : 'rgb(7, 9, 10)');
              assert.equal(await bg('.site-top .rx-navbar'), 'rgb(7, 9, 10)', 'the dark top bar');
              assert.equal(await bg('.rx-brand-mark'), 'rgb(255, 106, 43)', 'the orange logo tile');

              const results = await contrasts(page, spec.texts);
              console.log(`${name} ${width} ${mode}:`, results.map(([s, r]) => `${s} ${r}`).join(' | '));
              assert.deepEqual(results.filter(([, r]) => r === null).map(([s]) => s), [], 'selectors with no element');
              assert.deepEqual(results.filter(([, r]) => r < 4.5), [], 'text at WCAG AA');

              // The focus ring, from the keyboard.
              await page.press('Tab');
              await page.eval((s) => {
                const el = document.querySelector(s);
                el.scrollIntoView({ block: 'center' });
                el.focus();
              }, spec.focus);
              assert.equal(await page.eval((s) => document.activeElement === document.querySelector(s), spec.focus), true);
              const [[, ring]] = await contrasts(page, [[spec.focus, 'outlineColor']]);
              assert.ok(ring >= 3, `focus ring contrast ${ring}`);

              if (process.env.SITE_SCREENS) {
                await page.eval(() => window.scrollTo(0, 0));
                await page.screenshot(join(process.env.SITE_SCREENS, `docs-${name}-${width}-${mode}.png`));
              }
              page.assertClean();
            },
            { width, height: 900 },
          ));
      }
    }
  }
});
