// #269: an accessibility smoke test. Every control on the main pages of a
// few examples has an accessible name (from Chrome's accessibility tree);
// crud's product form and the shop's checkout are completed with the
// keyboard alone.

import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { Browser } from './lib/cdp.mjs';
import { start } from './lib/app.mjs';

let browser;

before(async () => {
  browser = await Browser.launch();
});

after(async () => {
  await browser?.close();
});

const CONTROLS = new Set([
  'button', 'link', 'textbox', 'searchbox', 'checkbox', 'radio', 'combobox', 'listbox', 'menuitem',
  'menuitemcheckbox', 'menuitemradio', 'tab', 'spinbutton', 'switch', 'slider', 'option',
]);

/** The controls in the accessibility tree, and those of them with no name. */
async function unnamed(page) {
  const { nodes } = await page.send('Accessibility.getFullAXTree');
  const controls = nodes.filter((n) => !n.ignored && CONTROLS.has(n.role?.value));
  const missing = controls
    .filter((n) => !String(n.name?.value ?? '').trim())
    .map((n) => `${n.role.value} (backend node ${n.backendDOMNodeId})`);
  return { count: controls.length, missing };
}

async function describeNodes(page, list) {
  const out = [];
  for (const item of list) {
    const id = Number(item.match(/node (\d+)/)[1]);
    try {
      const { outerHTML } = await page.send('DOM.getOuterHTML', { backendNodeId: id });
      out.push(`${item}: ${outerHTML.slice(0, 160)}`);
    } catch {
      out.push(item);
    }
  }
  return out;
}

async function submitAndLoad(page, selector) {
  const loaded = page.once('Page.loadEventFired');
  if (selector) await page.click(selector);
  else await page.press('Enter');
  await loaded;
  await page.settle();
}

async function logIn(page, url, email, password = 'password123') {
  await page.goto(`${url}/login`);
  await page.type('#rx-email', email);
  await page.type('#rx-password', password);
  await submitAndLoad(page, 'form button[type=submit]');
}

/** Tabs forward until `selector` has the focus (at most `max` presses). */
async function tabTo(page, selector, max = 60) {
  for (let i = 0; i < max; i++) {
    if (await page.eval((s) => document.activeElement?.matches(s), selector)) return;
    await page.press('Tab');
  }
  throw new Error(`Tab never reached ${selector}`);
}

const pages = {
  crud: { dir: 'examples/crud', login: 'demo@example.com', paths: ['/', '/products', '/products/new', '/products/trash'] },
  shop: { dir: 'examples/shop', login: 'admin@example.com', paths: ['/', '/products', '/cart', '/admin', '/admin/products', '/admin/products/new', '/admin/orders'] },
  grid: { dir: 'examples/grid', login: 'demo@example.com', password: 'password', paths: ['/', '/regions', '/follow-up'] },
  fields: { dir: 'examples/fields', paths: ['/', '/products/new'] },
};

for (const [name, spec] of Object.entries(pages)) {
  describe(name, () => {
    let app;
    before(async () => {
      app = await start(name, spec.dir, { seed: true });
    });
    after(() => app?.stop());

    test('every control on its pages has a name', () =>
      browser.with(async (page) => {
        if (spec.login) await logIn(page, app.url, spec.login, spec.password);
        const problems = [];
        for (const path of spec.paths) {
          await page.goto(`${app.url}${path}`);
          const { count, missing } = await unnamed(page);
          assert.ok(count >= 3, `${path}: only ${count} controls in the tree`);
          if (missing.length) problems.push(`${path}:\n    ${(await describeNodes(page, missing)).join('\n    ')}`);
        }
        assert.deepEqual(problems, []);
      }));
  });
}

describe('with the keyboard alone', () => {
  test("crud's product form", async () => {
    const app = await start('crud', 'examples/crud', { seed: true });
    try {
      await browser.with(async (page) => {
        await logIn(page, app.url, 'demo@example.com');
        await page.goto(`${app.url}/products/new`);
        await page.eval(() => document.activeElement?.blur());
        await tabTo(page, '[name=name]');
        await page.type('[name=name]', 'Keyboard blend');
        await tabTo(page, '[name=price]');
        await page.type('[name=price]', '30000', { clear: true });
        await submitAndLoad(page); // Enter in a field sends the form
        assert.match(await page.eval(() => document.body.textContent), /Keyboard blend/);
        page.assertClean();
      });
    } finally {
      await app.stop();
    }
  });

  test("the shop's checkout", async () => {
    const app = await start('shop', 'examples/shop', { seed: true });
    try {
      await browser.with(async (page) => {
        await logIn(page, app.url, 'admin@example.com');
        await page.goto(`${app.url}/products`);
        const product = await page.eval(() => document.querySelector('a[href^="/products/"]').getAttribute('href'));
        await page.goto(`${app.url}${product}`);
        await tabTo(page, 'form[action$="/cart"] button[type=submit]');
        await submitAndLoad(page);
        await page.goto(`${app.url}/cart`);
        await tabTo(page, 'a[href$="/checkout"]');
        await submitAndLoad(page);
        // Delivery: the arrow keys move between the choices.
        await tabTo(page, 'input[name=delivery]');
        await page.press('ArrowRight');
        assert.equal(await page.eval(() => document.querySelector('input[name=delivery]:checked').value), 'pickup');
        await tabTo(page, 'form[action$="/checkout"] button[type=submit]');
        await submitAndLoad(page);
        assert.match(await page.eval(() => location.pathname), /^\/orders\/\d+$/);
        page.assertClean();
      });
    } finally {
      await app.stop();
    }
  });
});
