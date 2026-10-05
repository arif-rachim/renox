// #269: example apps end to end in a browser: htmx-recipes' interactions,
// crud's forms and trash, and the guestbook, the last two under CSP=strict.

import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { Browser, sleep } from './lib/cdp.mjs';
import { start } from './lib/app.mjs';

let browser;

before(async () => {
  browser = await Browser.launch();
});

after(async () => {
  await browser?.close();
});

describe('htmx-recipes', () => {
  let app;
  before(async () => {
    app = await start('htmx-recipes', 'examples/htmx-recipes', { seed: true });
  });
  after(() => app?.stop());

  const onTasks = (fn) =>
    browser.with(async (page) => {
      await page.goto(`${app.url}/`);
      await fn(page);
      page.assertClean({ allow: [/422/] });
    });

  const openCount = (page) => page.eval(() => parseInt(document.querySelector('#open-count').textContent, 10));

  test('the modal form adds a row at the top and updates the count out of band', () =>
    onTasks(async (page) => {
      const before = await openCount(page);
      // Its keyboard shortcut opens it.
      await page.eval(() => document.activeElement?.blur());
      await page.press('n');
      await page.waitFor(() => document.querySelector('#new-task').open);
      await page.click('#new-task button[type=submit]');
      await page.waitFor(() => document.querySelector('#new-title').getAttribute('aria-invalid') === 'true');
      assert.ok(await page.eval(() => document.querySelector('#new-task').open), 'the 422 stays in the sheet');
      await page.type('#new-title', 'Water the plants');
      await page.click('#new-task button[type=submit]');
      await page.waitFor(() => !document.querySelector('#new-task').open);
      await page.waitFor(() => document.querySelector('#tasks li')?.textContent.includes('Water the plants'));
      assert.equal(await openCount(page), before + 1);
    }));

  test('a task is ticked in place, edited inline and deleted from its menu', () =>
    onTasks(async (page) => {
      const before = await openCount(page);
      const id = await page.eval(() => document.querySelector('#tasks li[id^="task-"]').id);
      await page.click(`#${id} input[type=checkbox]`);
      await page.waitFor((b) => parseInt(document.querySelector('#open-count').textContent, 10) === b - 1, {}, before);
      await page.settle();

      // Double-click the title: a form; Enter saves.
      await page.eval((i) => {
        const title = document.querySelector(`#${i} .rx-list__main`);
        title.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
      }, id);
      await page.waitFor((i) => !!document.querySelector(`#${i} input[name=title]`), {}, id);
      await page.type(`#${id} input[name=title]`, 'Renamed task', { clear: true });
      await page.press('Enter');
      await page.waitFor((i) => document.querySelector(`#${i} .rx-list__main`)?.textContent.includes('Renamed task'), {}, id);

      // Delete from the menu (hx-confirm's dialog is accepted).
      await page.click(`#${id} [aria-haspopup="menu"]`);
      await page.click(`#${id} .rx-menu__item--danger`);
      await page.waitFor((i) => !document.querySelector(`#${i}`), { message: 'the row removed' }, id);
      assert.deepEqual(page.dialogs.map((d) => d.message), ['Delete this task?']);
      await page.waitFor(() => document.querySelector('.rx-toast')?.textContent.includes('deleted'));
    }));

  test('scrolling to the end loads the next rows once each', () =>
    onTasks(async (page) => {
      const ids = () => page.eval(() => [...document.querySelectorAll('#tasks li[id^="task-"]')].map((li) => li.id));
      const first = await ids();
      await page.eval(() => document.querySelector('[hx-trigger="revealed"]').scrollIntoView());
      await page.waitFor((n) => document.querySelectorAll('#tasks li[id^="task-"]').length > n, {}, first.length);
      await page.settle();
      const all = await ids();
      assert.equal(new Set(all).size, all.length, 'no row twice');
    }));

  test('Alpine tabs filter the rows without a request', () =>
    onTasks(async (page) => {
      const requests = [];
      page.on('Network.requestWillBeSent', (p) => requests.push(p.request.url));
      const visible = () =>
        page.eval(() => [...document.querySelectorAll('#tasks li[id^="task-"]')].filter((li) => li.offsetParent).length);
      const all = await visible();
      await page.click('[role=tab]:nth-of-type(3)'); // Done
      await sleep(100);
      assert.ok((await visible()) < all);
      assert.deepEqual(requests, []);
    }));
});

describe('crud under CSP=strict', () => {
  let app;
  before(async () => {
    app = await start('crud', 'examples/crud', { seed: true, env: { CSP: 'strict' } });
  });
  after(() => app?.stop());

  test('logging in, a live-validated form and a product made', () =>
    browser.with(async (page) => {
      await page.goto(`${app.url}/login`);
      await page.type('#rx-email', 'demo@example.com');
      await page.type('#rx-password', 'password123');
      let loaded = page.once('Page.loadEventFired');
      await page.click('form button[type=submit]');
      await loaded;
      await page.goto(`${app.url}/products/new`);
      // Live validation: a price below zero complains on leaving the field,
      // and the error goes once it's fixed (while typing).
      await page.type('[name=price]', '-5', { clear: true });
      await page.press('Tab');
      await page.waitFor(() => document.querySelector('[name=price]').getAttribute('aria-invalid') === 'true', {
        message: 'live validation',
      });
      await page.type('[name=price]', '25000', { clear: true });
      await page.waitFor(() => !document.querySelector('[name=price]').hasAttribute('aria-invalid'), {
        message: 'the error gone',
      });
      await page.type('[name=name]', 'Browser blend');
      loaded = page.once('Page.loadEventFired');
      await page.click('form[data-live-validate] button[type=submit]');
      await loaded;
      await page.settle();
      assert.match(await page.eval(() => document.body.textContent), /Browser blend/);
      assert.match(await page.eval(() => document.querySelector('.rx-toast')?.textContent || ''), /created/i);
      page.assertClean({ allow: [/422/] });
    }));
});

describe('the guestbook under CSP=strict', () => {
  let app;
  before(async () => {
    app = await start('hello', 'examples/hello', { env: { CSP: 'strict' } });
  });
  after(() => app?.stop());

  test('a 422 keeps the input; a post clears the form and lists the entry', () =>
    browser.with(async (page) => {
      await page.goto(`${app.url}/`);
      await page.type('#rx-name', 'Ana');
      await page.type('#rx-message', 'Hi'); // too short (3 to 280)
      await page.click('form[hx-post] button[type=submit]');
      await page.waitFor(() => document.querySelector('#rx-message').getAttribute('aria-invalid') === 'true');
      assert.equal(await page.eval(() => document.querySelector('#rx-name').value), 'Ana', 'the input is kept');
      await page.type('#rx-message', 'Hello from a browser', { clear: true });
      await page.click('form[hx-post] button[type=submit]');
      await page.waitFor(() => document.querySelector('#entries').textContent.includes('Hello from a browser'));
      await page.waitFor(() => document.querySelector('#rx-message').value === '', { message: 'the form cleared' });
      page.assertClean({ allow: [/422/] });
    }));
});
