// Live components in a browser (#436): rx-click and rx-submit send the
// action, the answer is morphed in, and typed text and focus survive.

import { after, before, test } from 'node:test';
import assert from 'node:assert/strict';
import { Browser } from './lib/cdp.mjs';
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

const onLive = (fn) =>
  browser.with(async (page) => {
    await page.goto(`${app.url}/live`);
    await fn(page);
    page.assertClean();
  });

const count = (page) => page.eval(() => document.querySelector('#count').textContent);

test('clicking rx-click twice counts to 2', () =>
  onLive(async (page) => {
    await page.click('#inc');
    await page.waitFor(() => document.querySelector('#count').textContent === '1');
    await page.click('#inc');
    await page.waitFor(() => document.querySelector('#count').textContent === '2');
    assert.equal(await count(page), '2');
    assert.equal(await page.eval(() => document.querySelector('[data-rx-live]').hasAttribute('aria-busy')), false);
  }));

test('rx-click with an argument adds it', () =>
  onLive(async (page) => {
    await page.click('#add5');
    await page.waitFor(() => document.querySelector('#count').textContent === '5');
  }));

test('typed text and focus survive a morph, and the snapshot changes', () =>
  onLive(async (page) => {
    const before = await page.eval(() => document.querySelector('[data-rx-live]').getAttribute('data-rx-snapshot'));
    await page.type('#note', 'keep me');
    // .click() keeps the focus in the input, as a timer or a key would.
    await page.eval(() => document.querySelector('#inc').click());
    await page.waitFor(() => document.querySelector('#count').textContent === '1');
    const after = await page.eval(() => ({
      value: document.querySelector('#note').value,
      focused: document.activeElement && document.activeElement.id,
      snapshot: document.querySelector('[data-rx-live]').getAttribute('data-rx-snapshot'),
    }));
    assert.equal(after.value, 'keep me');
    assert.equal(after.focused, 'note');
    assert.notEqual(after.snapshot, before);
  }));

test('an rx-submit form sends its fields', () =>
  onLive(async (page) => {
    await page.eval(() => {
      document.querySelector('#name').value = 'Ada';
    });
    await page.click('#rename-go');
    await page.waitFor(() => document.querySelector('#shown-name').textContent === 'Ada');
  }));

const countRequests = (page) =>
  page.eval(() => {
    window.__reqs = 0;
    document.addEventListener('htmx:beforeRequest', () => window.__reqs++);
  });

test('rx-model.live debounces typing into one request and keeps focus and caret', () =>
  onLive(async (page) => {
    await countRequests(page);
    await page.type('#search', 'tea');
    await page.waitFor(() => document.querySelector('#echo').textContent === 'tea');
    const after = await page.eval(() => ({
      reqs: window.__reqs,
      focused: document.activeElement && document.activeElement.id,
      caret: document.querySelector('#search').selectionStart,
      value: document.querySelector('#search').value,
    }));
    assert.equal(after.reqs, 1);
    assert.equal(after.focused, 'search');
    assert.equal(after.value, 'tea');
    assert.equal(after.caret, 3);
  }));

test('rx-model.blur sends only when the field is left', () =>
  onLive(async (page) => {
    await countRequests(page);
    await page.type('#lazy', 'later');
    await new Promise((r) => setTimeout(r, 600));
    assert.equal(await page.eval(() => window.__reqs), 0);
    await page.eval(() => document.querySelector('#lazy').blur());
    await page.waitFor(() => document.querySelector('#lazy-echo').textContent === 'later');
    assert.equal(await page.eval(() => window.__reqs), 1);
  }));

test('a plain rx-model sends nothing but its value reaches the next rx-click', () =>
  onLive(async (page) => {
    await countRequests(page);
    await page.type('#nick', 'ada');
    await new Promise((r) => setTimeout(r, 600));
    assert.equal(await page.eval(() => window.__reqs), 0);
    await page.click('#inc');
    await page.waitFor(() => document.querySelector('#nick-echo').textContent === 'ada');
  }));
