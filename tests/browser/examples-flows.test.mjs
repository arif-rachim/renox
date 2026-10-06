// #269, the rest: each example's main flow in a browser. htmx-recipes'
// inline cancel, a retargeted answer and toasts after a redirect or a
// refresh; crud's edit, delete with its confirmation, trash and restore,
// and its scoped page links; the guestbook refusing a text file named
// .png and switching language; the shop from browsing to an order and its
// bell, the admin's category combobox and the dashboard; teams switching,
// the new-team wizard with its repeater (nested names), two-factor login and a team's own
// host; an invoice issued in the back office; a product edited in the
// admin panel; a free trial opening the subscribers' pages.

import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { createHmac } from 'node:crypto';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Browser } from './lib/cdp.mjs';
import { start } from './lib/app.mjs';

let browser;

before(async () => {
  browser = await Browser.launch();
});

after(async () => {
  await browser?.close();
});

/** Submits the form `button` is in and waits for the next page. */
async function submitAndLoad(page, button) {
  const loaded = page.once('Page.loadEventFired');
  await page.click(button);
  await loaded;
  await page.settle();
}

async function logIn(page, url, email, password = 'password123') {
  // Whoever this browser was logged in as before goes first.
  await page.goto(`${url}/login`);
  if (!(await page.eval(() => !!document.querySelector('#rx-email')))) {
    await logOut(page);
    await page.goto(`${url}/login`);
  }
  await page.type('#rx-email', email);
  await page.type('#rx-password', password);
  await submitAndLoad(page, 'form button[type=submit]');
}

async function logOut(page) {
  await page.eval(() =>
    fetch('/logout', {
      method: 'POST',
      headers: { 'X-CSRF-Token': document.querySelector('meta[name="csrf-token"]').content },
    }),
  );
}

async function register(page, url, email) {
  await page.goto(`${url}/register`);
  await page.type('#rx-name', 'Browser Buyer');
  await page.type('#rx-email', email);
  await page.type('#rx-password', 'password123');
  await page.type('#rx-password_confirmation', 'password123');
  await submitAndLoad(page, 'form button[type=submit]');
}

const body = (page) => page.eval(() => document.body.textContent);

describe('htmx-recipes', () => {
  let app;
  before(async () => {
    app = await start('htmx-recipes', 'examples/htmx-recipes', { seed: true });
  });
  after(() => app?.stop());

  test('an inline edit cancelled with Escape gets the row back as it was', () =>
    browser.with(async (page) => {
      await page.goto(`${app.url}/`);
      const patches = [];
      page.on('Network.requestWillBeSent', (p) => p.request.method === 'PATCH' && patches.push(p.request.url));
      const id = await page.eval(() => document.querySelector('#tasks li[id^="task-"]').id);
      const title = await page.text(`#${id} .rx-list__main`);
      await page.eval((i) => document.querySelector(`#${i} .rx-list__main`).dispatchEvent(new MouseEvent('dblclick', { bubbles: true })), id);
      await page.waitFor((i) => !!document.querySelector(`#${i} input[name=title]`), {}, id);
      await page.type(`#${id} input[name=title]`, 'Never saved', { clear: true });
      await page.press('Escape');
      await page.waitFor((i) => !!document.querySelector(`#${i} .rx-list__main`), { message: 'the row back' }, id);
      assert.equal(await page.text(`#${id} .rx-list__main`), title);
      assert.deepEqual(patches, []);
      page.assertClean();
    }));

  test('a task already on the list: the answer replaces its row (HX-Retarget, HX-Reswap) with a toast', () =>
    browser.with(async (page) => {
      await page.goto(`${app.url}/`);
      const first = await page.eval(() => {
        const li = document.querySelector('#tasks li[id^="task-"]');
        return { id: li.id, title: li.querySelector('.rx-list__main').textContent.trim() };
      });
      const rows = () => page.eval(() => document.querySelectorAll('#tasks li[id^="task-"]').length);
      const before = await rows();
      await page.eval(() => document.activeElement?.blur());
      await page.press('n');
      await page.waitFor(() => document.querySelector('#new-task').open);
      await page.type('#new-title', first.title);
      await page.eval((i) => (document.getElementById(i).dataset.probe = 'old'), first.id);
      await page.click('#new-task button[type=submit]');
      await page.waitFor(() => document.querySelector('.rx-toast')?.textContent.includes('already on the list'));
      await page.waitFor((i) => document.getElementById(i) && !document.getElementById(i).dataset.probe, { message: 'the row swapped in place' }, first.id);
      assert.equal(await rows(), before, 'no second row');
      page.assertClean();
    }));

  test('toasts come with a refreshed page (HX-Refresh) and a redirect (HX-Redirect)', () =>
    browser.with(async (page) => {
      await page.goto(`${app.url}/`);
      // One task done, so both buttons have something to do.
      const id = await page.eval(() => document.querySelector('#tasks li[id^="task-"] input[type=checkbox]:not(:checked)').closest('li').id);
      await page.click(`#${id} input[type=checkbox]`);
      await page.settle();
      let loaded = page.once('Page.loadEventFired');
      await page.eval(() => document.querySelector('form[hx-post$="/tasks/clear-done"] button').click());
      await loaded;
      await page.settle();
      await page.waitFor(() => /done tasks? cleared/.test(document.querySelector('.rx-toast')?.textContent || ''), { message: 'the toast after HX-Refresh' });
      // Archive: off to the summary page, with its toast.
      const another = await page.eval(() => document.querySelector('#tasks li[id^="task-"] input[type=checkbox]:not(:checked)').closest('li').id);
      await page.click(`#${another} input[type=checkbox]`);
      await page.settle();
      loaded = page.once('Page.loadEventFired');
      await page.click('form[hx-post$="/tasks/archive"] button');
      await loaded;
      await page.settle();
      assert.equal(await page.eval(() => location.pathname), '/summary');
      await page.waitFor(() => /archived/.test(document.querySelector('.rx-toast')?.textContent || ''), { message: 'the toast after HX-Redirect' });
      page.assertClean();
    }));
});

describe('crud', () => {
  let app;
  before(async () => {
    app = await start('crud', 'examples/crud', { seed: true });
  });
  after(() => app?.stop());

  test('a product edited, moved to the trash after a confirmation, and restored', () =>
    browser.with(async (page) => {
      await logIn(page, app.url, 'demo@example.com');
      await page.goto(`${app.url}/products`);
      const name = await page.text('#products tbody tr:first-of-type strong');
      // Edit: a full page, saved, back to the list.
      await submitAndLoad(page, '#products tbody tr:first-of-type a[href$="/edit"]');
      await page.type('[name=name]', `${name} (edited)`, { clear: true });
      await submitAndLoad(page, 'form[data-live-validate] button[type=submit]');
      assert.match(await body(page), /\(edited\)/);
      // Delete: the confirmation first; Cancel keeps it.
      await page.goto(`${app.url}/products`);
      const row = await page.eval((n) => [...document.querySelectorAll('#products tbody tr')].findIndex((tr) => tr.textContent.includes(`${n} (edited)`)), name);
      assert.ok(row >= 0);
      const dialog = await page.eval((r) => document.querySelectorAll('#products tbody tr')[r].querySelector('dialog').id, row);
      await page.eval((d) => document.querySelector(`[aria-controls="${d}"], [data-rx-open="${d}"]`).click(), dialog);
      await page.waitFor((d) => document.getElementById(d).open, {}, dialog);
      await page.click(`#${dialog} [data-rx-close]`);
      await page.waitFor((d) => !document.getElementById(d).open, {}, dialog);
      await page.eval((d) => document.querySelector(`[aria-controls="${d}"], [data-rx-open="${d}"]`).click(), dialog);
      await page.waitFor((d) => document.getElementById(d).open, {}, dialog);
      await submitAndLoad(page, `#${dialog} button[type=submit]`);
      assert.doesNotMatch(await page.eval(() => document.querySelector('#products').textContent), /\(edited\)/);
      // The trash has it; restoring brings it back.
      await page.goto(`${app.url}/products/trash`);
      assert.match(await body(page), /\(edited\)/);
      await submitAndLoad(page, 'form[action$="/restore"] button');
      await page.goto(`${app.url}/products`);
      assert.match(await body(page), /\(edited\)/);
      page.assertClean({ allow: [/422/] });
    }));

  test('page links swap only the list, and edit links stay full pages', () =>
    browser.with(async (page) => {
      await page.goto(`${app.url}/products`);
      await page.eval(() => (document.querySelector('h1').dataset.probe = 'kept'));
      await page.click('#products a[href*="page=2"]');
      await page.waitFor(() => new URLSearchParams(location.search).get('page') === '2', { message: 'page 2 in the address' });
      await page.settle();
      const state = await page.eval(() => ({
        heading: document.querySelector('h1').dataset.probe,
        lists: document.querySelectorAll('#products').length,
        headings: document.querySelectorAll('h1').length,
      }));
      assert.deepEqual(state, { heading: 'kept', lists: 1, headings: 1 });
      // An edit link on the swapped page is a page of its own (a whole load).
      await logIn(page, app.url, 'demo@example.com');
      await page.goto(`${app.url}/products?page=2`);
      await submitAndLoad(page, '#products a[href$="/edit"]');
      assert.match(await page.eval(() => location.pathname), /^\/products\/\d+\/edit$/);
      page.assertClean();
    }));
});

describe('the guestbook', () => {
  let app;
  let dir;
  before(async () => {
    app = await start('hello', 'examples/hello');
    dir = mkdtempSync(join(tmpdir(), 'renox-upload-'));
  });
  after(async () => {
    await app?.stop();
    rmSync(dir, { recursive: true, force: true });
  });

  test('a text file named .png is refused as a photo, and nothing is posted', () =>
    browser.with(async (page) => {
      await page.goto(`${app.url}/`);
      const fake = join(dir, 'holiday.png');
      writeFileSync(fake, 'not a picture at all');
      const { root } = await page.send('DOM.getDocument', { depth: 0 });
      const { nodeId } = await page.send('DOM.querySelector', { nodeId: root.nodeId, selector: 'input[type=file][name=photo]' });
      await page.send('DOM.setFileInputFiles', { nodeId, files: [fake] });
      await page.type('#rx-name', 'Ana');
      await page.type('#rx-message', 'With a picture, supposedly');
      const entries = await page.eval(() => document.querySelector('#entries').textContent);
      await page.click('form[hx-post] button[type=submit]');
      await page.waitFor(() => document.querySelector('input[type=file][name=photo]').getAttribute('aria-invalid') === 'true', { message: 'the photo refused' });
      assert.match(await page.eval(() => document.querySelector('[data-error-for="photo"], #rx-photo-error, .error')?.textContent || ''), /image/i);
      assert.equal(await page.eval(() => document.querySelector('#entries').textContent), entries, 'nothing posted');
      page.assertClean({ allow: [/422/] });
    }));

  test('the language menu switches the page to Spanish and back', () =>
    browser.with(async (page) => {
      await page.goto(`${app.url}/`);
      const english = await page.eval(() => document.querySelector('h1').textContent);
      await page.click('[aria-controls="language-menu"], #language-menu-button, [popovertarget="language-menu"]');
      await submitAndLoad(page, 'a[href$="/language/es"]');
      assert.equal(await page.eval(() => document.documentElement.lang), 'es');
      assert.notEqual(await page.eval(() => document.querySelector('h1').textContent), english);
      await page.goto(`${app.url}/language/en`);
      assert.equal(await page.eval(() => document.documentElement.lang), 'en');
      page.assertClean();
    }));
});

describe('fields', () => {
  let app;
  before(async () => {
    app = await start('fields', 'examples/fields', { seed: true });
  });
  after(() => app?.stop());

  test('every kind of field comes back as it was sent', () =>
    browser.with(async (page) => {
      await page.goto(`${app.url}/products/new`);
      await page.waitFor(() => !!document.querySelector('trix-editor')?.editor && !!document.querySelector('[data-rx-code-editor] [contenteditable]'));
      await page.type('#rx-name', 'Round trip');
      await page.type('#rx-description', 'Smooth *and* sweet');
      await page.type('#rx-stock', '12', { clear: true });
      await page.type('#rx-weight_kg', '0.75', { clear: true });
      await page.type('#rx-price', '64000', { clear: true });
      await page.click('label[for="rx-available"], #rx-available');
      await page.click('label[for="rx-size-3"]');
      await page.click('label[for="rx-colors-2"]');
      await page.type('#rx-tags', 'organic');
      await page.press('Enter');
      await page.type('#rx-tags', 'decaf');
      await page.press('Enter');
      await page.click('.rx-kv [data-rx-row-add]');
      await page.waitFor(() => !!document.querySelector('input[name="specs[0][key]"]'));
      await page.type('input[name="specs[0][key]"]', 'Origin');
      await page.type('input[name="specs[0][value]"]', 'Java');
      await page.click('trix-editor');
      await page.type('trix-editor', 'Roasted weekly');
      await page.click('[data-rx-code-editor] [contenteditable]');
      await page.type('[data-rx-code-editor] [contenteditable]', '{"grind": "fine"}');
      await page.eval(() => {
        const set = (id, value) => {
          const el = document.getElementById(id);
          el.value = value;
          el.dispatchEvent(new Event('input', { bubbles: true }));
          el.dispatchEvent(new Event('change', { bubbles: true }));
        };
        set('rx-opens_at', '08:30');
        set('rx-launch_at', '2026-11-01T09:30');
        // The date picker's own field (its calendar is tested in ui-forms).
        set('rx-released_on', '2026-12-24');
      });
      const sent = await page.eval(() => {
        const form = document.querySelector('form[novalidate]');
        return [...new FormData(form)].filter(([k]) => k !== '_token');
      });
      await submitAndLoad(page, 'form[novalidate] .rx-card__footer button[type=submit]');
      const errors = await page.eval(() => [...document.querySelectorAll('[aria-invalid="true"]')].map((el) => `${el.name}: ${document.getElementById(el.getAttribute('aria-describedby')?.split(' ').find((d) => d.endsWith('-error')) || '')?.textContent || ''}`));
      assert.match(await page.eval(() => location.pathname), /\/edit$/, `saved, on its edit page (errors: ${errors})`);
      const back = await page.eval(() => {
        const form = document.querySelector('form[novalidate]');
        return [...new FormData(form)].filter(([k]) => !['_token', '_method', 'key'].includes(k));
      });
      const pick = (pairs, name) => pairs.filter(([k]) => k === name).map(([, v]) => v);
      for (const name of ['name', 'description', 'stock', 'price', 'available', 'size', 'colors', 'tags', 'specs[0][key]', 'specs[0][value]', 'settings', 'opens_at', 'released_on']) {
        assert.deepEqual(pick(back, name).map((v) => v.replace(/:00$/, '')), pick(sent, name).map((v) => v.replace(/:00$/, '')), name);
      }
      assert.equal(Number(pick(back, 'weight_kg')[0]), 0.75);
      assert.match(pick(back, 'launch_at')[0], /^2026-11-01T09:30/);
      assert.match(pick(back, 'details')[0], /Roasted weekly/);
      page.assertClean();
    }));
});

describe('the shop', () => {
  let app;
  before(async () => {
    app = await start('shop', 'examples/shop', { seed: true });
  });
  after(() => app?.stop());

  test('a guest browses and is asked to log in; a new customer orders and the bell rings', () =>
    browser.with(async (page) => {
      await page.goto(`${app.url}/products`);
      const product = await page.eval(() => document.querySelector('a[href^="/products/"]').getAttribute('href'));
      await page.goto(`${app.url}${product}`);
      assert.ok(await page.eval(() => !!document.querySelector('a[href$="/login"].rx-button, .rx-button[href$="/login"]')), 'log in to buy');
      // A new customer.
      await register(page, app.url, `buyer${Date.now()}@example.com`);
      await page.goto(`${app.url}${product}`);
      await page.type('[name=quantity]', '2', { clear: true });
      await submitAndLoad(page, 'form[action$="/cart"] button[type=submit]');
      await page.goto(`${app.url}/cart`);
      assert.match(await body(page), /2/);
      await submitAndLoad(page, 'a[href$="/checkout"]');
      // Picked up: the address goes away (and isn't asked for).
      await page.click('label:has(input[name=delivery][value=pickup])');
      await page.waitFor(() => !document.querySelector('[name=address]') || document.querySelector('[name=address]').closest('fieldset')?.disabled, { message: 'no address for a pickup' });
      await submitAndLoad(page, 'form[action$="/checkout"] button[type=submit]');
      assert.match(await page.eval(() => location.pathname), /^\/orders\/\d+$/);
      // The confirmation lands in the bell (queued, then sent over SSE).
      await page.waitFor(() => {
        const count = document.querySelector('[data-rx-bell-count]');
        return count && !count.hidden && Number(count.textContent) >= 1;
      }, { timeout: 15_000, message: 'the bell counts the order' });
      page.assertClean();
    }));

  test("the admin's category field asks the server and adds a category; the dashboard has tabs and charts", () =>
    browser.with(async (page) => {
      await logIn(page, app.url, 'admin@example.com');
      await page.goto(`${app.url}/admin/products/new`);
      await page.type('#rx-category_id-search', 'Browser Gadgets');
      await page.waitFor(() => document.querySelector('.rx-combobox__option--create'), { message: 'the add option' });
      await page.eval(() => document.querySelector('.rx-combobox__option--create').click());
      await page.waitFor(() => {
        const select = document.querySelector('#rx-category_id');
        return select.options[select.selectedIndex]?.textContent === 'Browser Gadgets';
      }, { message: 'added and chosen' });
      const listed = await page.eval(async () => (await (await fetch('/admin/categories/options?q=gadg')).json()).map((o) => o.label));
      assert.ok(listed.includes('Browser Gadgets'), JSON.stringify(listed));
      await logOut(page);
      await logIn(page, app.url, 'admin@example.com');
      await page.goto(`${app.url}/admin`);
      assert.ok((await page.eval(() => document.querySelectorAll('figure.rx-chart').length)) >= 2, 'charts');
      const tabs = await page.eval(() => [...document.querySelectorAll('[role=tab]')].map((t) => t.id));
      assert.ok(tabs.length >= 2, JSON.stringify(tabs));
      await page.click(`#${tabs[1]}`);
      await page.waitFor((t) => document.getElementById(t).getAttribute('aria-selected') === 'true', {}, tabs[1]);
      const panel = await page.eval((t) => document.getElementById(document.getElementById(t).getAttribute('aria-controls')), tabs[1]);
      assert.ok(panel !== undefined);
      page.assertClean();
    }));
});

describe('teams', () => {
  let app;
  before(async () => {
    app = await start('teams', 'examples/teams', { seed: true });
  });
  after(() => app?.stop());

  test('switching team, and a new team made with the wizard and its repeater', () =>
    browser.with(async (page) => {
      await logIn(page, app.url, 'alice@example.com');
      await page.goto(`${app.url}/teams`);
      const switchTo = await page.eval(() => document.querySelector('form[action*="/switch"] button')?.textContent.trim());
      assert.ok(switchTo, 'a team to switch to');
      await submitAndLoad(page, 'form[action*="/switch"] button');
      await page.goto(`${app.url}/teams`);
      assert.match(await body(page), /Current team/);
      // The wizard: a name, then two members (a repeater row added).
      await page.type('#rx-name', 'Browser Team');
      await page.click('[data-rx-wizard-next]');
      await page.waitFor(() => !!document.querySelector('[data-rx-row-add]')?.offsetParent, { message: 'the members step' });
      // The repeater starts empty: a row per member.
      await page.click('[data-rx-row-add]');
      await page.waitFor(() => !!document.querySelector('input[name="invites[0][email]"]'));
      await page.type('input[name="invites[0][email]"]', 'bob@example.com');
      await page.click('[data-rx-row-add]');
      await page.waitFor(() => !!document.querySelector('input[name="invites[1][email]"]'));
      await page.type('input[name="invites[1][email]"]', 'carol@example.com');
      await submitAndLoad(page, '[data-rx-wizard-submit]');
      assert.match(await body(page), /Browser Team/);
      await page.waitFor(() => /Team Browser Team created, with 2 members/.test(document.querySelector('.rx-toast')?.textContent || ''), { message: 'the toast counts Bob and Carol' });
      page.assertClean({ allow: [/422/] });
    }));

  test("a team's page answers on its own host", () =>
    browser.with(async (page) => {
      const port = new URL(app.url).port;
      await page.goto(`http://acme.localhost:${port}/`);
      assert.match(await page.eval(() => document.querySelector('h1').textContent), /Acme/);
      await page.goto(`http://acme.localhost:${port}/anything`);
      assert.equal(await page.eval(() => location.pathname), '/');
      page.assertClean();
    }));

  test('two-factor login: turned on with a code, then asked for after the password', () =>
    browser.with(async (page) => {
      const totp = (secret, offset = 0) => {
        const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';
        let bits = '';
        for (const ch of secret.replace(/[\s=]/g, '').toUpperCase()) bits += alphabet.indexOf(ch).toString(2).padStart(5, '0');
        const key = Buffer.from(bits.match(/.{8}/g).map((b) => parseInt(b, 2)));
        const step = Math.floor(Date.now() / 1000 / 30) + offset;
        const counter = Buffer.alloc(8);
        counter.writeBigUInt64BE(BigInt(step));
        const mac = createHmac('sha1', key).update(counter).digest();
        const at = mac[mac.length - 1] & 0xf;
        const n = (mac.readUInt32BE(at) & 0x7fffffff) % 1_000_000;
        return String(n).padStart(6, '0');
      };
      await logIn(page, app.url, 'carol@example.com');
      await page.goto(`${app.url}/account`);
      await submitAndLoad(page, 'form[action$="/two-factor/enable"] button, form[action*="two-factor"] button');
      const secret = await page.text('.rx-2fa__key');
      assert.match(secret, /^[A-Z2-7 ]+$/);
      await page.type('#rx-code', totp(secret));
      await page.click('form[action*="two-factor/confirm"] button[type=submit]');
      await page.waitFor(() => !!document.querySelector('.rx-2fa__codes'), { message: 'the recovery codes' });
      await logOut(page);
      // The password, then the code (the next one: a code works once).
      await logIn(page, app.url, 'carol@example.com');
      assert.match(await page.eval(() => location.pathname), /two-factor/);
      await page.type('#rx-code', totp(secret, 1));
      const loaded = page.once('Page.loadEventFired');
      await page.click('form[action*="two-factor"] button[type=submit]');
      await loaded;
      await page.settle();
      assert.doesNotMatch(await page.eval(() => location.pathname), /two-factor|login/);
      page.assertClean();
    }));
});

describe('the back office, the admin panel and billing', () => {
  test('an invoice made with its lines and issued', async () => {
    const app = await start('backoffice', 'examples/backoffice', { seed: true });
    try {
      await browser.with(async (page) => {
        await logIn(page, app.url, 'admin@example.com');
        await page.goto(`${app.url}/invoices/new`);
        // The first option that fits (products say how many are in stock).
        const pick = (id, fits) =>
          page.eval(
            (i, f) => {
              const select = document.getElementById(i);
              const at = [...select.options].findIndex((o) => o.value && new RegExp(f).test(o.textContent));
              select.selectedIndex = at;
              select.dispatchEvent(new Event('change', { bubbles: true }));
              return select.options[at].textContent;
            },
            id,
            fits,
          );
        await pick('rx-customer_id', '.');
        await pick('rx-lines-0-product_id', '\\(([2-9]|\\d\\d+) in stock\\)');
        await page.type('#rx-lines-0-quantity', '2', { clear: true });
        await submitAndLoad(page, 'form[action$="/invoices"] button[type=submit]');
        assert.match(await page.eval(() => location.pathname), /^\/invoices\/\d+$/);
        assert.match(await body(page), /Draft/);
        await submitAndLoad(page, 'form[action$="/issue"] button');
        assert.match(await body(page), /Issued/);
        page.assertClean();
      });
    } finally {
      await app.stop();
    }
  });

  test('a product edited in the admin panel', async () => {
    const app = await start('admin', 'examples/admin', { seed: true });
    try {
      await browser.with(async (page) => {
        await logIn(page, app.url, 'admin@example.com');
        await page.goto(`${app.url}/admin/products`);
        const edit = await page.eval(() => document.querySelector('a[href$="/edit"]').getAttribute('href'));
        await page.goto(`${app.url}${edit}`);
        await page.type('[name=name]', 'Edited in a browser', { clear: true });
        await submitAndLoad(page, 'form[data-live-validate] button[type=submit]');
        await page.goto(`${app.url}/admin/products?search=browser`);
        assert.match(await body(page), /Edited in a browser/);
        page.assertClean();
      });
    } finally {
      await app.stop();
    }
  });

  test('a free trial opens the pages for subscribers', async () => {
    const app = await start('billing', 'examples/billing', { seed: true });
    try {
      await browser.with(async (page) => {
        await register(page, app.url, `trial${Date.now()}@example.com`);
        // Without a subscription the reports aren't open.
        await page.goto(`${app.url}/reports`);
        assert.notEqual(await page.eval(() => location.pathname), '/reports');
        await page.goto(`${app.url}/billing`);
        await submitAndLoad(page, 'form[action$="/billing/trial/pro"] button');
        await page.goto(`${app.url}/reports`);
        assert.equal(await page.eval(() => location.pathname), '/reports');
        await page.goto(`${app.url}/exports`);
        assert.equal(await page.eval(() => location.pathname), '/exports');
        page.assertClean();
      });
    } finally {
      await app.stop();
    }
  });
});

