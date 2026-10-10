// #422: dialogs announce what happened as `rx:<component>:<event>`
// (opened, closed, saved, failed, confirmed, cancelled), bubbling to document.

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

const NAMES = ['opened', 'closed', 'saved', 'failed'].map((n) => `rx:action-sheet:${n}`)
  .concat(['opened', 'closed', 'confirmed', 'cancelled'].map((n) => `rx:confirm:${n}`));

const listen = (page) =>
  page.eval((names) => {
    window.__ev = [];
    for (const n of names) document.addEventListener(n, (e) => window.__ev.push([n, e.target.id, e.detail]));
  }, NAMES);

const events = (page) => page.eval(() => window.__ev);

test('an action sheet says opened, failed (422, stays open), saved then closed', () =>
  browser.with(async (page) => {
    await page.goto(`${app.url}/events`);
    await listen(page);
    await page.click('[data-rx-open="ev-sheet"]');
    await page.waitFor(() => document.querySelector('#ev-sheet').open);
    assert.deepEqual((await events(page))[0], ['rx:action-sheet:opened', 'ev-sheet', { id: 'ev-sheet' }]);

    await page.click('#ev-sheet button[type=submit]');
    await page.waitFor(() => window.__ev.some((e) => e[0] === 'rx:action-sheet:failed'));
    const failed = (await events(page)).find((e) => e[0] === 'rx:action-sheet:failed');
    assert.deepEqual(failed[2], { id: 'ev-sheet', status: 422 });
    assert.ok(await page.eval(() => document.querySelector('#ev-sheet').open), 'stays open');

    await page.type('#rx-from', 'Bandung');
    await page.type('#rx-to', 'Jakarta');
    await page.click('#ev-sheet button[type=submit]');
    await page.waitFor(() => window.__ev.some((e) => e[0] === 'rx:action-sheet:closed'));
    const names = (await events(page)).map((e) => e[0]);
    const tail = names.slice(names.indexOf('rx:action-sheet:saved'));
    assert.deepEqual(tail, ['rx:action-sheet:saved', 'rx:action-sheet:closed']);
    page.assertClean({ allow: [/422/] });
  }));

test('a confirm says cancelled then closed on Cancel', () =>
  browser.with(async (page) => {
    await page.goto(`${app.url}/events`);
    await listen(page);
    await page.click('[data-rx-open="ev-confirm"]');
    await page.waitFor(() => document.querySelector('#ev-confirm').open);
    await page.click('#ev-confirm [data-rx-close]');
    await page.waitFor(() => window.__ev.some((e) => e[0] === 'rx:confirm:closed'));
    assert.deepEqual((await events(page)).map((e) => e[0]), ['rx:confirm:opened', 'rx:confirm:cancelled', 'rx:confirm:closed']);
    page.assertClean({ allow: [/422/] });
  }));

test('a confirm says confirmed when Delete is pressed', () =>
  browser.with(async (page) => {
    await page.goto(`${app.url}/events`);
    await page.eval(() => {
      sessionStorage.removeItem('confirmed');
      document.addEventListener('rx:confirm:confirmed', (e) =>
        sessionStorage.setItem('confirmed', JSON.stringify([e.target.id, e.detail])));
    });
    await page.click('[data-rx-open="ev-confirm"]');
    await page.waitFor(() => document.querySelector('#ev-confirm').open);
    await page.click('#ev-confirm button[type=submit]');
    await page.waitFor(() => location.pathname === '/confirm-done');
    assert.deepEqual(JSON.parse(await page.eval(() => sessionStorage.getItem('confirmed'))), ['ev-confirm', { id: 'ev-confirm' }]);
    page.assertClean({ allow: [/422/] });
  }));

// #423: tabs and wizards say `changed`, only when the selection really changes.
const listenChanged = (page) =>
  page.eval(() => {
    window.__ch = [];
    for (const n of ['rx:tabs:changed', 'rx:wizard:changed'])
      document.addEventListener(n, (e) => window.__ch.push([n, e.detail]));
  });
const changes = (page) => page.eval(() => window.__ch);

test('tabs say changed on a click and on ArrowRight, never for the selected tab', () =>
  browser.with(async (page) => {
    await page.goto(`${app.url}/events`);
    await listenChanged(page);
    await page.click('#evt-tab-a');
    assert.deepEqual(await changes(page), [], 'already selected');
    await page.click('#evt-tab-b');
    assert.deepEqual(await changes(page), [['rx:tabs:changed', { name: 'b' }]]);
    await page.eval(() => document.querySelector('#evt-tab-b').focus());
    await page.press('ArrowLeft');
    assert.deepEqual((await changes(page)).length, 2);
    await page.press('ArrowRight');
    assert.deepEqual((await changes(page)).length, 3);
    page.assertClean({ allow: [/422/] });
  }));

test('a wizard says changed on Next, not when its sheet opens', () =>
  browser.with(async (page) => {
    await page.goto(`${app.url}/events`);
    await listenChanged(page);
    await page.click('[data-rx-open="wizard-sheet"]');
    await page.waitFor(() => document.querySelector('#wizard-sheet').open);
    await page.waitFor(() => document.querySelector('[data-rx-wizard][data-rx-ready]'));
    assert.deepEqual(await changes(page), []);
    await page.type('#ws-from', 'Bandung');
    await page.click('#wizard-sheet [data-rx-wizard-next]');
    await page.waitFor(() => window.__ch.length > 0);
    assert.deepEqual(await changes(page), [['rx:wizard:changed', { name: 'end', index: 1 }]]);
    page.assertClean({ allow: [/422/] });
  }));
