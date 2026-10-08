// The bike shop's blocks (examples/bikeshop/resources/views/blocks/,
// public/blocks/blocks.js|css) on /about/blocks, in a browser: every block at
// 1280 and 390 px wide, light and dark, under the default CSP and CSP=strict,
// with a clean console; then each block's keyboard and pointer paths: the
// gallery, the range slider sent in a GET form, the stepper, the keypad, the
// date and time range, the closed days, the variant chips (htmx), the whole
// form sent through Valid<T>, the kanban (keyboard, drag, a refused move),
// the calendar's months (htmx), a free slot, and prefers-reduced-motion.
// Screenshots go to BIKESHOP_SCREENS when it's set.

import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { Browser, sleep } from './lib/cdp.mjs';
import { start } from './lib/app.mjs';

const PHONE = { width: 390, height: 844 };
const PAGE = '/about/blocks';
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

const fitsWidth = (page) => page.eval(() => document.documentElement.scrollWidth <= window.innerWidth);

/** The browser's local date, as the page's scripts see it, `days` from today. */
const isoIn = (page, days) =>
  page.eval((n) => {
    const d = new Date();
    d.setDate(d.getDate() + n);
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
  }, days);

/** Collects the board's `bs:kanban-moved` events in `window.__moves`. */
const watchMoves = (page) =>
  page.eval(() => {
    window.__moves = [];
    document.addEventListener('bs:kanban-moved', (e) => window.__moves.push(e.detail));
  });

/** The column a card is in. */
const columnOf = (page, id) =>
  page.eval((card) => document.querySelector(`[data-bs-card="${card}"]`)?.closest('[data-bs-column]')?.dataset.bsColumn, id);

/** Drags `from` (a selector) to the point (x, y) with the mouse, in steps. */
async function drag(page, from, to) {
  const a = await page.point(from);
  await page.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: a.x, y: a.y });
  await page.send('Input.dispatchMouseEvent', { type: 'mousePressed', x: a.x, y: a.y, button: 'left', clickCount: 1 });
  const steps = 12;
  for (let i = 1; i <= steps; i++) {
    const x = a.x + ((to.x - a.x) * i) / steps;
    const y = a.y + ((to.y - a.y) * i) / steps;
    await page.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y, button: 'left', buttons: 1 });
    await sleep(16);
  }
  await page.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: to.x, y: to.y, button: 'left', clickCount: 1 });
  await sleep(50);
}

/** Animations on `selector` that take time (the kit's reduced-motion rule
 *  leaves 0.01 ms transitions on everything, which are instant). */
const moving = (page, selector) =>
  page.eval((s) => document.querySelector(s).getAnimations().filter((a) => a.effect.getTiming().duration > 1).length, selector);

const BLOCKS = [
  '[data-bs-gallery][data-bs-ready]',
  '[data-bs-range][data-bs-ready]',
  '[data-bs-quantity]',
  '[data-bs-keypad][data-bs-ready]',
  '[data-bs-kanban][data-bs-ready]',
  '.bs-month',
  '.bs-availability',
  '[data-bs-datetime][data-bs-ready]',
  '.bs-swatches--size',
  '.bs-swatches--colour',
  '.bs-history',
  '.bs-plans',
];

for (const csp of ['relaxed', 'strict']) {
  describe(`bikeshop blocks under CSP=${csp}`, () => {
    let app;
    before(async () => {
      app = await start('bikeshop', 'examples/bikeshop', { env: { CSP: csp } });
    });
    after(() => app?.stop());

    for (const [size, options] of [
      ['desktop', undefined],
      ['phone', PHONE],
    ]) {
      for (const scheme of ['light', 'dark']) {
        test(`every block is on the page (${size}, ${scheme})`, () =>
          browser.with(async (page) => {
            await page.send('Emulation.setEmulatedMedia', {
              features: [{ name: 'prefers-color-scheme', value: scheme }],
            });
            await page.goto(`${app.url}${PAGE}`);
            for (const selector of BLOCKS) {
              assert.ok(await page.eval((s) => !!document.querySelector(s), selector), `${selector} is set up`);
            }
            // The scheme reached the kit's tokens (and so the blocks).
            const surface = await page.eval(() => getComputedStyle(document.querySelector('.bs-plan')).backgroundColor);
            // The shop's surface colour (public/theme.css: #fffdf9, dark #1d1b18).
            assert.equal(surface, scheme === 'dark' ? 'rgb(29, 27, 24)' : 'rgb(255, 253, 249)', 'cards follow the scheme');
            assert.ok(await fitsWidth(page), 'no sideways scrolling');
            if (size === 'phone') {
              // The month becomes a list of the days with something on.
              const shown = await page.eval(() =>
                [...document.querySelectorAll('.bs-month__day')].filter((d) => getComputedStyle(d).display !== 'none').length,
              );
              assert.ok(shown >= 5 && shown <= 7, `${shown} days listed on a phone`);
              assert.equal(await page.eval(() => getComputedStyle(document.querySelector('.bs-month__weekdays')).display), 'none');
            } else {
              assert.equal(
                await page.eval(() => getComputedStyle(document.querySelector('.bs-month__days')).gridTemplateColumns.split(' ').length),
                7,
                'seven columns on a desktop',
              );
            }
            await shot(page, `blocks-${size}-${scheme}-${csp}`);
            page.assertClean();
          }, options));
      }
    }

    test('the gallery: arrows, thumbnails, keys, a drag and the enlarged photo', () =>
      browser.with(async (page) => {
        await page.goto(`${app.url}${PAGE}`);
        const index = () => page.eval(() => document.querySelector('#demo-gallery').dataset.bsIndex);
        assert.equal(await index(), '0');
        assert.equal(await page.eval(() => document.querySelector('[data-bs-gallery-prev]').disabled), true);
        await page.click('#demo-gallery [data-bs-gallery-next]');
        assert.equal(await index(), '1');
        assert.equal(await page.eval(() => document.querySelector('[data-bs-gallery-thumb="1"]').getAttribute('aria-current')), 'true');
        // Only the current photo is reachable by screen readers.
        assert.equal(await page.eval(() => document.querySelectorAll('#demo-gallery .bs-gallery__slide[inert]').length), 3);
        await page.click('[data-bs-gallery-thumb="3"]');
        assert.equal(await index(), '3');
        assert.equal(await page.eval(() => location.hash), '', 'a thumbnail does not jump the page');
        // The keyboard, from the photo itself.
        await page.focus('#demo-gallery [data-bs-gallery-viewport]');
        await page.press('Home');
        assert.equal(await index(), '0');
        await page.press('ArrowRight');
        await page.press('ArrowRight');
        assert.equal(await index(), '2');
        await page.press('End');
        assert.equal(await index(), '3');
        await page.press('ArrowLeft');
        assert.equal(await index(), '2');
        assert.match(await page.text('[data-bs-gallery-status]'), /Photo 3 of 4/);
        // A drag to the left shows the next photo, and it ends in place.
        const box = await page.point('#demo-gallery [data-bs-gallery-viewport]');
        await drag(page, '#demo-gallery [data-bs-gallery-viewport]', { x: box.x - box.w / 2, y: box.y });
        await page.waitFor(() => document.querySelector('#demo-gallery').dataset.bsIndex === '3');
        await page.waitFor(() => {
          const width = document.querySelector('[data-bs-gallery-viewport]').clientWidth;
          return document.querySelector('[data-bs-gallery-track]').style.transform === `translateX(${-3 * width}px)`;
        });
        // Enlarge: the kit's sheet, with the photo shown.
        await page.click('#demo-gallery [data-bs-gallery-enlarge]');
        await page.waitFor(() => document.querySelector('#demo-gallery-zoom').open);
        assert.match(await page.eval(() => document.querySelector('[data-bs-gallery-zoom]').src), /city-ride\.svg$/);
        await page.press('Escape');
        await page.waitFor(() => !document.querySelector('#demo-gallery-zoom').open);
        assert.equal(await page.eval(() => document.activeElement.hasAttribute('data-bs-gallery-enlarge')), true, 'focus back on the button');
        page.assertClean();
      }));

    test('the range slider: keys move each handle, they never cross, a GET form sends both', () =>
      browser.with(async (page) => {
        await page.goto(`${app.url}${PAGE}`);
        const low = '#bs-price_min-range-min';
        await page.focus(low);
        await page.press('ArrowRight');
        assert.equal(await page.eval((s) => document.querySelector(s).value, low), '125000');
        assert.match(await page.eval((s) => document.querySelector(s).getAttribute('aria-valuetext'), low), /\$1[.,]250[.,]00\b/);
        // End would pass the other handle: it stops there.
        await page.press('End');
        assert.equal(await page.eval((s) => document.querySelector(s).value, low), '500000');
        await page.press('PageDown');
        await page.press('Home');
        assert.equal(await page.eval((s) => document.querySelector(s).value, low), '0');
        await page.focus('#bs-price_min-range-max');
        await page.press('ArrowLeft');
        assert.equal(await page.eval(() => document.querySelector('#bs-price_min-range-max').value), '475000');
        assert.match(await page.text('[data-bs-range-shown="max"]'), /\$4[.,]750[.,]00\b/);
        await page.click('#filter button[type=submit]');
        await page.waitFor(() => location.search.includes('price_max='));
        const query = await page.eval(() => location.search);
        assert.match(query, /price_min=0/);
        assert.match(query, /price_max=475000/);
        assert.ok(await page.text('[data-bs-filtering]'));
        page.assertClean();
      }));

    test('the stepper and the keypad', () =>
      browser.with(async (page) => {
        await page.goto(`${app.url}${PAGE}`);
        const qty = () => page.eval(() => document.querySelector('#bs-quantity').value);
        const plus = '[data-bs-quantity] [data-bs-step="1"]';
        const minus = '[data-bs-quantity] [data-bs-step="-1"]';
        assert.equal(await page.eval((s) => document.querySelector(s).disabled, minus), true, 'at the minimum');
        for (let i = 0; i < 5; i++) await page.click(plus);
        assert.equal(await qty(), '5');
        assert.equal(await page.eval((s) => document.querySelector(s).disabled, plus), true, 'at the maximum');
        // The field's own arrow keys.
        await page.focus('#bs-quantity');
        await page.press('ArrowDown');
        assert.equal(await qty(), '4');
        assert.equal(await page.eval((s) => document.querySelector(s).disabled, plus), false);

        // The keypad: one Tab stop, arrows between keys, digits typed on it.
        const paid = () => page.eval(() => document.querySelector('#rx-paid').value);
        await page.focus('[data-bs-keypad] [data-bs-key="7"]');
        await page.press('3');
        await page.press('5');
        assert.equal(await paid(), '35');
        await page.press('ArrowRight');
        assert.equal(await page.eval(() => document.activeElement.dataset.bsKey), '8');
        await page.press('ArrowDown');
        assert.equal(await page.eval(() => document.activeElement.dataset.bsKey), '5');
        await page.press('Enter');
        assert.equal(await paid(), '355');
        await page.press('Backspace');
        assert.equal(await paid(), '35');
        await page.click('[data-bs-keypad] [data-bs-key="00"]');
        assert.equal(await paid(), '3500');
        assert.equal(
          await page.eval(() => [...document.querySelectorAll('[data-bs-key]')].filter((k) => k.tabIndex === 0).length),
          1,
          'one Tab stop',
        );
        // Delete clears (lib/cdp.mjs's press() knows no Delete key).
        for (const type of ['rawKeyDown', 'keyUp']) {
          await page.send('Input.dispatchKeyEvent', { type, key: 'Delete', code: 'Delete', windowsVirtualKeyCode: 46 });
        }
        assert.equal(await paid(), '');
        page.assertClean();
      }));

    test('the date and time range fills its two fields and refuses an end before its start', () =>
      browser.with(async (page) => {
        await page.goto(`${app.url}${PAGE}`);
        const day = await isoIn(page, 7);
        const next = await isoIn(page, 8);
        await page.type('#bs-starts_at-range-start-date', day);
        await page.eval(() => {
          const s = document.querySelector('#bs-starts_at-range-start-time');
          s.value = '10:00';
          s.dispatchEvent(new Event('change', { bubbles: true }));
        });
        await page.type('#bs-starts_at-range-end-date', next);
        await page.focus('#bs-starts_at-range-end-time');
        // A select answers to the keyboard: pick 12:00.
        await page.eval(() => {
          const s = document.querySelector('#bs-starts_at-range-end-time');
          s.value = '12:00';
          s.dispatchEvent(new Event('change', { bubbles: true }));
        });
        assert.equal(await page.eval(() => document.querySelector('[name=starts_at]').value), `${day}T10:00`);
        assert.equal(await page.eval(() => document.querySelector('[name=ends_at]').value), `${next}T12:00`);
        assert.equal(await page.text('[data-bs-dt-summary]'), '1 day 2 hours');
        // The end's calendar starts at the start's day.
        assert.equal(await page.eval(() => document.querySelector('#bs-starts_at-range-end-date-calendar calendar-date').getAttribute('min')), day);
        // An end before the start is pointed out at once.
        await page.type('#bs-starts_at-range-end-date', day, { clear: true });
        await page.eval(() => {
          const s = document.querySelector('#bs-starts_at-range-end-time');
          s.value = '09:00';
          s.dispatchEvent(new Event('change', { bubbles: true }));
        });
        assert.equal(await page.eval(() => document.querySelector('[data-bs-dt-summary]').hasAttribute('data-bs-invalid')), true);
        assert.equal(await page.eval(() => document.querySelector('#bs-starts_at-range-end-time').getAttribute('aria-invalid')), 'true');

        page.assertClean();
      }));

    test('the variant chips ask the server for the price and the stock', () =>
      browser.with(async (page) => {
        await page.goto(`${app.url}${PAGE}`);
        assert.match(await page.text('[data-bs-variant-stock]'), /6 in stock/);
        await page.click('.bs-swatches--size .bs-swatch:nth-child(3) .bs-swatch__chip');
        await page.waitFor(() => document.querySelector('[name=size]:checked').value === 'L');
        await page.waitFor(() => /3 in stock/.test(document.querySelector('[data-bs-variant-stock]').textContent));
        // With the keyboard: the arrows choose the next colour.
        await page.focus('.bs-swatches--colour [name=colour]:checked');
        await page.press('ArrowRight');
        await page.press('ArrowRight');
        assert.equal(await page.eval(() => document.querySelector('[name=colour]:checked').value), 'graphite');
        await page.waitFor(() => /1 in stock/.test(document.querySelector('[data-bs-variant-stock]').textContent));
        // The sold-out size can't be chosen.
        assert.equal(await page.eval(() => document.querySelector('[name=size][value=XL]').disabled), true);
        page.assertClean();
      }));

    test('the form blocks are sent and accepted by Valid<T>', () =>
      browser.with(async (page) => {
        await page.goto(`${app.url}${PAGE}`);
        const day = await isoIn(page, 1);
        await page.click('[data-bs-quantity] [data-bs-step="1"]');
        await page.focus('[data-bs-keypad] [data-bs-key="7"]');
        await page.press('4');
        await page.press('2');
        await page.type('#bs-starts_at-range-start-date', day);
        await page.type('#bs-starts_at-range-end-date', day);
        await page.eval(() => {
          for (const [id, v] of [['start', '10:00'], ['end', '15:30']]) {
            const s = document.querySelector(`#bs-starts_at-range-${id}-time`);
            s.value = v;
            s.dispatchEvent(new Event('change', { bubbles: true }));
          }
        });
        // The keypad's Enter key sends the form.
        await page.click('[data-bs-keypad] [data-bs-key="enter"]');
        await page.waitFor(() => location.hash === '#form' && document.querySelector('.rx-alert'));
        const status = await page.text('.rx-alert');
        assert.match(status, /The server accepted: 2 × M teal, paid 42/);
        assert.match(status, new RegExp(`${day} 10:00 → ${day} 15:30`));
        page.assertClean();
      }));

    test('the kanban: keyboard moves, a drag, a refused move put back', () =>
      browser.with(async (page) => {
        await page.goto(`${app.url}${PAGE}`);
        await watchMoves(page);
        const live = () => page.text('[data-bs-kanban-live]');
        // Pick up "Tune-up" (101), move it right and down, drop it.
        await page.focus('[data-bs-card="101"]');
        await page.press(' ');
        assert.match(await live(), /Picked up Tune-up, in Waiting, position 1 of 3/);
        await page.press('ArrowRight');
        assert.equal(await columnOf(page, 101), 'working');
        assert.match(await live(), /Tune-up: In the stand, position 1 of 3/);
        await page.press('ArrowDown');
        assert.match(await live(), /position 2 of 3/);
        assert.equal(await page.eval(() => document.activeElement.dataset.bsCard), '101', 'the card keeps the focus');
        await page.press(' ');
        assert.match(await live(), /Tune-up dropped in In the stand, position 2 of 3/);
        await page.waitFor(() => window.__moves.length === 1);
        assert.deepEqual(await page.eval(() => window.__moves[0]), { card: '101', column: 'working', position: 1, ok: true });
        assert.equal(await page.eval(() => document.querySelector('[data-bs-column="working"] [data-bs-column-count]').textContent), '3');

        // Escape puts a picked-up card back.
        await page.focus('[data-bs-card="103"]');
        await page.press('Enter');
        await page.press('ArrowRight');
        await page.press('ArrowRight');
        assert.equal(await columnOf(page, 103), 'ready');
        await page.press('Escape');
        assert.equal(await columnOf(page, 103), 'waiting');
        assert.match(await live(), /Move cancelled/);

        // Without holding a card, the arrows move the focus.
        await page.focus('[data-bs-card="102"]');
        await page.press('ArrowDown');
        assert.equal(await page.eval(() => document.activeElement.dataset.bsCard), '103');

        // A drag with the mouse into "Ready for pickup".
        const target = await page.point('[data-bs-column="ready"] [data-bs-kanban-list]');
        await drag(page, '[data-bs-card="102"] .bs-kanban__body', { x: target.x, y: target.y + target.h / 2 - 4 });
        await page.waitFor(() => window.__moves.length === 2);
        assert.equal(await columnOf(page, 102), 'ready');
        assert.equal(await page.eval(() => window.__moves[1].ok), true);
        assert.equal(await page.eval(() => document.querySelector('[data-bs-card="102"]').getAttribute('style') || ''), '');

        // The server refuses a move (a column it doesn't know): the card goes back.
        await page.eval(() => document.querySelector('[data-bs-column="ready"]').setAttribute('data-bs-column', 'nowhere'));
        await page.focus('[data-bs-card="104"]');
        await page.press(' ');
        await page.press('ArrowRight');
        await page.press(' ');
        await page.waitFor(() => window.__moves.length === 3);
        assert.equal(await page.eval(() => window.__moves[2].ok), false);
        assert.equal(await columnOf(page, 104), 'working');
        assert.match(await live(), /couldn't be saved/);
        // The refusal was the 422 renox.js expects; nothing else went wrong.
        page.assertClean({ allow: [/422/] });
      }));

    test('the calendar changes month in place; a free slot is a link', () =>
      browser.with(async (page) => {
        await page.goto(`${app.url}${PAGE}`);
        await page.eval(() => (window.__same = true));
        const title = await page.text('.bs-month__title');
        await page.click('[data-bs-month-next]');
        await page.waitFor((t) => document.querySelector('.bs-month__title').textContent.trim() !== t, {}, title);
        assert.equal(await page.eval(() => window.__same), true, 'no page load');
        await page.settle();
        assert.match(await page.eval(() => location.search), /month=\d{4}-\d{2}/);
        await page.click('[data-bs-month-prev]');
        await page.waitFor((t) => document.querySelector('.bs-month__title').textContent.trim() === t, {}, title);
        assert.ok(await page.eval(() => document.querySelector('.bs-month__day--today')), 'today is marked');

        // htmx scrolls a boosted swap into view: wait until the page is still.
        await page.settle();
        await sleep(400);
        await page.click('.bs-availability__book');
        await page.waitFor(() => location.search.includes('slot='));
        assert.match(await page.text('#availability .rx-alert'), /Booking City 3 11:00/);
        page.assertClean();
      }));
  });
}

describe('bikeshop blocks with prefers-reduced-motion', () => {
  let app;
  before(async () => {
    app = await start('bikeshop', 'examples/bikeshop');
  });
  after(() => app?.stop());

  for (const [size, options] of [
    ['desktop', undefined],
    ['phone', PHONE],
  ]) {
    test(`nothing moves, everything works (${size})`, () =>
      browser.with(async (page) => {
        await page.send('Emulation.setEmulatedMedia', {
          features: [{ name: 'prefers-reduced-motion', value: 'reduce' }],
        });
        await page.goto(`${app.url}${PAGE}`);
        await watchMoves(page);
        // The gallery jumps: no animation runs, the track is already in place.
        // (A DOM click: on a phone-sized page CDP's mouse lands off target
        // once the page has scrolled.)
        await page.eval(() => document.querySelector('#demo-gallery [data-bs-gallery-next]').click());
        assert.equal(await moving(page, '[data-bs-gallery-track]'), 0);
        assert.equal(
          await page.eval(() => document.querySelector('[data-bs-gallery-track]').style.transform),
          `translateX(-${await page.eval(() => document.querySelector('[data-bs-gallery-viewport]').clientWidth)}px)`,
        );
        // A kanban move with no glide.
        await page.focus('[data-bs-card="105"]');
        await page.press(' ');
        await page.press('ArrowRight');
        assert.equal(await moving(page, '[data-bs-card="105"]'), 0);
        await page.press(' ');
        await page.waitFor(() => window.__moves.length === 1);
        // The history's events were never faded in.
        assert.equal(
          await page.eval(() => [...document.querySelectorAll('.bs-history > li')].filter((el) => el.style.opacity || el.style.transform).length),
          0,
        );
        assert.ok(await fitsWidth(page));
        page.assertClean();
      }, options));
  }
});
