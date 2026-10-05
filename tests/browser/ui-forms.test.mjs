// #265: the form half of renox-ui.js in a real browser: busy buttons,
// revealable passwords, show_when, tags, the searchable select, the date
// picker, the repeater and the wizard.

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

/** Runs `fn` on a fresh /widgets page, closed afterwards whatever happens. */
const onWidgets = (fn) =>
  browser.with(async (page) => {
    await page.goto(`${app.url}/widgets`);
    await fn(page);
    page.assertClean({ allow: [/422/] });
  });

test('a regular form is sent once however often it is clicked; an htmx one shows it is busy', () =>
  onWidgets(async (page) => {
    const sent = [];
    page.on('Network.requestWillBeSent', (p) => p.request.method === 'POST' && sent.push(p.request.url));
    // htmx: the button is marked busy while the request runs.
    await page.click('#busy');
    await page.waitFor(() => document.querySelector('#busy').getAttribute('aria-busy') === 'true', {
      message: 'the htmx button marked busy',
    });
    await page.waitFor(() => !document.querySelector('#busy').hasAttribute('aria-busy'), { message: 'busy cleared' });
    // A regular form: a second click while it's sending is ignored (both
    // clicks in one go, before the page can change).
    const loaded = page.once('Page.loadEventFired');
    const busy = await page.eval(() => {
      const button = document.querySelector('#plain-busy-button');
      button.click();
      const marked = button.getAttribute('aria-busy');
      button.click();
      return marked;
    });
    assert.equal(busy, 'true', 'the regular button is marked busy');
    await loaded;
    assert.equal(sent.filter((u) => u.endsWith('/slow-plain')).length, 1, JSON.stringify(sent));
  }));

test('a password can be shown and hidden again', () => onWidgets(async (page) => {
  await page.click('[data-rx-reveal="rx-secret"]');
  assert.deepEqual(
    await page.eval(() => [document.querySelector('#rx-secret').type, document.querySelector('[data-rx-reveal]').getAttribute('aria-pressed')]),
    ['text', 'true'],
  );
  await page.click('[data-rx-reveal="rx-secret"]');
  assert.deepEqual(
    await page.eval(() => [document.querySelector('#rx-secret').type, document.querySelector('[data-rx-reveal]').getAttribute('aria-pressed')]),
    ['password', 'false'],
  );
}));

test('show_when shows a group and disables it while hidden', () => onWidgets(async (page) => {
  const state = () =>
    page.eval(() => {
      const group = document.querySelector('[data-rx-show-when="method"]');
      return { hidden: group.hidden || getComputedStyle(group).display === 'none', disabled: group.disabled };
    });
  assert.deepEqual(await state(), { hidden: true, disabled: true });
  await page.click('label[for="rx-method-2"]');
  await page.waitFor(() => !document.querySelector('[data-rx-show-when="method"]').disabled);
  assert.deepEqual(await state(), { hidden: false, disabled: false });
  // A disabled group's fields aren't sent with the form.
  await page.click('label[for="rx-method"]');
  const sent = await page.eval(() => [...new FormData(document.querySelector('#fields')).keys()]);
  assert.ok(!sent.includes('address'), JSON.stringify(sent));
}));

test('tags: Enter and comma add, duplicates are refused, Backspace removes', () => onWidgets(async (page) => {
  const tags = () => page.eval(() => [...document.querySelectorAll('[data-rx-tags="labels"] .rx-tag input[type=hidden]')].map((i) => i.value));
  assert.deepEqual(await tags(), ['new']);
  await page.type('#rx-labels', 'sale');
  await page.press('Enter');
  await page.type('#rx-labels', 'gift');
  await page.press(',');
  await page.type('#rx-labels', 'NEW');
  await page.press('Enter');
  assert.deepEqual(await tags(), ['new', 'sale', 'gift']);
  await page.press('Backspace');
  assert.deepEqual(await tags(), ['new', 'sale']);
}));

test('the searchable select filters, picks with the keyboard and keeps required', () => onWidgets(async (page) => {
  await page.waitFor(() => document.querySelector('#rx-size-search'));
  // Required still holds: the native select is only hidden.
  assert.equal(await page.eval(() => document.querySelector('#rx-size').checkValidity()), false);
  await page.type('#rx-size-search', 'lar');
  const shown = await page.waitFor(() => {
    const options = [...document.querySelectorAll('.rx-combobox__option')].filter((o) => !o.hidden && o.offsetParent);
    return options.length ? options.map((o) => o.textContent.trim()) : null;
  });
  assert.deepEqual(shown, ['Large', 'Extra large']);
  // The first match is highlighted: Enter picks it; ArrowDown moves on.
  await page.press('ArrowDown');
  await page.press('ArrowUp');
  await page.press('Enter');
  await page.waitFor(() => document.querySelector('#rx-size').value !== '');
  assert.equal(await page.eval(() => document.querySelector('#rx-size').value), 'l');
  assert.equal(await page.eval(() => document.querySelector('#rx-size').checkValidity()), true);
}));

test('the date picker opens beside its field and picks a day with the keyboard', () => onWidgets(async (page) => {
  await page.click('[popovertarget="rx-on-calendar"]');
  await page.waitFor(() => document.querySelector('#rx-on-calendar').matches(':popover-open'));
  const inView = await page.eval(() => {
    const r = document.querySelector('#rx-on-calendar').getBoundingClientRect();
    return r.top >= 0 && r.left >= 0 && r.bottom <= innerHeight + 1 && r.right <= innerWidth + 1;
  });
  assert.ok(inView, 'the calendar stays inside the window');
  // The calendar is focused on the field's day; one day on, then pick it.
  await page.waitFor(() => !!document.querySelector('calendar-date'));
  await sleep(200);
  await page.eval(() => {
    const cal = document.querySelector('calendar-date');
    cal.focus();
  });
  await page.press('ArrowRight');
  await page.press('Enter');
  await page.waitFor(() => document.querySelector('#rx-on').value !== '2026-10-02', { timeout: 5000 });
  assert.equal(await page.eval(() => document.querySelector('#rx-on').value), '2026-10-03');
}));

test('repeater rows are added, renumbered after a removal, within their limits', () => onWidgets(async (page) => {
  const names = () =>
    page.eval(() => [...document.querySelectorAll('#rx-lines [data-rx-row] input')].map((i) => i.name));
  await page.click('#rx-lines [data-rx-row-add]');
  await page.click('#rx-lines [data-rx-row-add]');
  assert.equal((await names()).length, 6, 'three rows of two fields');
  // At the maximum (3), adding is off.
  assert.ok(await page.eval(() => document.querySelector('#rx-lines [data-rx-row-add]').disabled));
  // Remove the first: the others are renumbered from 0.
  await page.click('#rx-lines [data-rx-row] [data-rx-row-remove]');
  assert.deepEqual(await names(), ['lines[0][name]', 'lines[0][qty]', 'lines[1][name]', 'lines[1][qty]']);
  // A 422 on the second row lands on that row's field.
  await page.type('[name="lines[1][name]"]', 'Tea');
  await page.click('#order button[type=submit]');
  await page.waitFor(() => document.querySelector('[name="lines[1][qty]"]').getAttribute('aria-invalid') === 'true', {
    message: 'the second row marked invalid',
  });
}));

test('the wizard checks a step before the next and lands on the step with an error', () => onWidgets(async (page) => {
  const step = () => page.eval(() => document.querySelector('#w').getAttribute('data-rx-step-index'));
  assert.equal(await step(), '0');
  // Next with the step's required field empty stays on the step.
  await page.click('#w [data-rx-wizard-next]');
  await sleep(200);
  assert.equal(await step(), '0');
  await page.type('#rx-from', 'Bandung');
  // Enter moves to the next step instead of sending the form.
  await page.press('Enter');
  await page.waitFor(() => document.querySelector('#w').getAttribute('data-rx-step-index') === '1');
  assert.equal(await page.text('#trip-result'), '');
  await page.type('#rx-to', 'Jakarta');
  await page.click('#w [data-rx-wizard-submit]');
  await page.waitFor(() => document.querySelector('#trip-result').textContent.includes('Bandung to Jakarta'));
}));
