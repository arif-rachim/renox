// #266: the other half of renox-ui.js: toasts, action sheets, menus, tabs,
// tooltips and keyboard shortcuts, as a keyboard or mouse user meets them.

import { after, before, test } from 'node:test';
import assert from 'node:assert/strict';
import { Browser, sleep } from './lib/cdp.mjs';
import { fixture } from './lib/app.mjs';

let browser;
let app;

before(async () => {
  app = await fixture();
  browser = await Browser.launch();
});

after(async () => {
  await browser?.close();
  await app?.stop();
});

const onOverlays = (fn, allow = []) =>
  browser.with(async (page) => {
    await page.goto(`${app.url}/overlays`);
    await fn(page);
    page.assertClean({ allow });
  });

test('a toast from htmx shows, keeps only safe links, and goes after its time', () =>
  onOverlays(async (page) => {
    await page.click('#toast-me');
    await page.waitFor(() => document.querySelector('.rx-toast--warning'), { message: 'the toast' });
    const toast = await page.eval(() => {
      const el = document.querySelector('.rx-toast--warning');
      return {
        text: el.textContent,
        links: [...el.querySelectorAll('a')].map((a) => a.getAttribute('href')),
      };
    });
    assert.match(toast.text, /Stock is low\./);
    // renox-ui.js drops a javascript: link that rode the HX-Trigger as data.
    assert.deepEqual(toast.links, ['/stock']);
    // `seconds(1)`: gone soon after (not hovered).
    await page.waitFor(() => !document.querySelector('.rx-toast--warning'), {
      timeout: 6000,
      message: 'the toast to go',
    });
  }));

test('a toast stays while hovered and its close button dismisses it', () =>
  onOverlays(async (page) => {
    await page.click('#toast-me');
    await page.waitFor(() => document.querySelector('.rx-toast--warning'));
    await page.hover('.rx-toast--warning');
    await sleep(1600);
    assert.ok(await page.eval(() => !!document.querySelector('.rx-toast--warning')), 'kept while hovered');
    await page.click('.rx-toast--warning .rx-toast__close');
    await page.waitFor(() => !document.querySelector('.rx-toast--warning'), { message: 'dismissed' });
  }));

test('an action sheet takes focus, keeps a 422 inside, closes on success and on Escape', () =>
  onOverlays(
    async (page) => {
      await page.click('[data-rx-open="trip-sheet"]');
      await page.waitFor(() => document.querySelector('#trip-sheet').open);
      assert.equal(await page.focused(), 'rx-from', 'the first field is focused');
      // Escape closes it and focus goes back to the button that opened it.
      await page.press('Escape');
      await page.waitFor(() => !document.querySelector('#trip-sheet').open);
      assert.ok(await page.eval(() => document.activeElement.matches('[data-rx-open="trip-sheet"]')));

      await page.click('[data-rx-open="trip-sheet"]');
      await page.waitFor(() => document.querySelector('#trip-sheet').open);
      await page.click('#trip-sheet button[type=submit]');
      await page.waitFor(() => document.querySelector('#rx-from').getAttribute('aria-invalid') === 'true');
      assert.ok(await page.eval(() => document.querySelector('#trip-sheet').open), 'a 422 stays in the sheet');
      await page.type('#rx-from', 'Bandung');
      await page.type('#rx-to', 'Jakarta');
      await page.click('#trip-sheet button[type=submit]');
      await page.waitFor(() => !document.querySelector('#trip-sheet').open, { message: 'closed on success' });
      await page.waitFor(() => document.querySelector('.rx-toast')?.textContent.includes('Bandung saved.'));
      // The form is reset for next time (as the dialog's close event runs).
      await page.waitFor(() => document.querySelector('#rx-from').value === '', { message: 'the form reset' });
    },
    [/422/],
  ));

test('a menu opens, moves with the arrow keys and closes on Escape', () =>
  onOverlays(async (page) => {
    await page.click('[aria-controls="more-menu"]');
    await page.waitFor(() => !document.querySelector('#more-menu').hidden);
    assert.equal(await page.eval(() => document.querySelector('[aria-controls="more-menu"]').getAttribute('aria-expanded')), 'true');
    // Opening focuses the first item; the arrows move between them.
    assert.equal(await page.eval(() => document.activeElement.textContent.trim()), 'Stock');
    await page.press('ArrowDown');
    assert.equal(await page.eval(() => document.activeElement.textContent.trim()), 'Home');
    await page.press('ArrowUp');
    assert.equal(await page.eval(() => document.activeElement.textContent.trim()), 'Stock');
    await page.press('Escape');
    await page.waitFor(() => document.querySelector('#more-menu').hidden);
    assert.ok(await page.eval(() => document.activeElement.matches('[aria-controls="more-menu"]')));
    // A click elsewhere closes it too.
    await page.click('[aria-controls="more-menu"]');
    await page.waitFor(() => !document.querySelector('#more-menu').hidden);
    await page.clickAt(1260, 880);
    await page.waitFor(() => document.querySelector('#more-menu').hidden);
  }));

test('tabs follow the arrow keys, Home and End', () =>
  onOverlays(async (page) => {
    const selected = () =>
      page.eval(() => ({
        tab: document.querySelector('[role=tab][aria-selected="true"]').id,
        panel: [...document.querySelectorAll('[role=tabpanel]')].find((p) => !p.hidden)?.id,
      }));
    await page.focus('#t-tab-one');
    await page.press('ArrowRight');
    assert.deepEqual(await selected(), { tab: 't-tab-two', panel: 't-panel-two' });
    await page.press('End');
    assert.deepEqual(await selected(), { tab: 't-tab-three', panel: 't-panel-three' });
    await page.press('Home');
    assert.deepEqual(await selected(), { tab: 't-tab-one', panel: 't-panel-one' });
    await page.click('#t-tab-three');
    assert.deepEqual(await selected(), { tab: 't-tab-three', panel: 't-panel-three' });
  }));

test('a tooltip shows on hover and on focus, inside the window', () =>
  onOverlays(async (page) => {
    await page.hover('#tipped');
    const tip = await page.waitFor(() => {
      const el = document.querySelector('[role=tooltip]');
      if (!el || el.hidden || !el.textContent.includes('More about this')) return null;
      const r = el.getBoundingClientRect();
      return { left: r.left, right: r.right, top: r.top };
    });
    assert.ok(tip.left >= 0 && tip.right <= 1280 && tip.top >= 0, JSON.stringify(tip));
    // Keyboard focus (focus-visible) shows it at once.
    await page.hover('#typing');
    await page.waitFor(() => !document.querySelector('[role=tooltip]'), { message: 'hidden again' });
    // (the visible tab panel comes just before it)
    await page.focus('#t-panel-one');
    await page.press('Tab');
    assert.equal(await page.focused(), 'tipped');
    await page.waitFor(() => {
      const el = document.querySelector('[role=tooltip]');
      return el && !el.hidden && el.textContent.includes('More about this');
    });
  }));

test('a keyboard shortcut presses its button, but not while typing', () =>
  onOverlays(async (page) => {
    const pressed = () => page.eval(() => Number(document.querySelector('#shortcut').dataset.pressed || 0));
    await page.eval(() => document.activeElement?.blur());
    await page.press('k');
    assert.equal(await pressed(), 1);
    await page.type('#typing', 'k');
    assert.equal(await pressed(), 1, 'typing a k in a field');
    assert.equal(await page.eval(() => document.querySelector('#typing').value), 'k');
  }));
