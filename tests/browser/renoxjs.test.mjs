// #264: renox.js in a real browser: the CSRF header on htmx requests, a
// 422 placed next to its fields with focus on the first in page order and
// the typed input kept, plain forms refilled after a redirect.

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

test('a 422 puts each error next to its field and focuses the first one in page order', () => browser.with(async (page) => {
  await page.goto(`${app.url}/form`);
  // `email` is wrong and `name` too long; alphabetical order would focus
  // email first, page order focuses name.
  await page.type('#rx-name', 'A name far too long');
  await page.type('#rx-email', 'not-an-email');
  await page.type('input[name=tags]', 'toolong');
  await page.click('#signup button[type=submit]');
  await page.waitFor(() => document.querySelectorAll('[aria-invalid="true"]').length >= 2, {
    message: 'fields marked invalid',
  });
  await page.settle();
  const state = await page.eval(() => ({
    nameInvalid: document.querySelector('#rx-name').getAttribute('aria-invalid'),
    emailInvalid: document.querySelector('#rx-email').getAttribute('aria-invalid'),
    nameError: document.querySelector('#rx-name-error')?.textContent.trim(),
    tagSlot: document.querySelector('#tag-slot').textContent.trim(),
    name: document.querySelector('#rx-name').value,
    email: document.querySelector('#rx-email').value,
    focused: document.activeElement?.id,
  }));
  assert.equal(state.nameInvalid, 'true');
  assert.equal(state.emailInvalid, 'true');
  assert.match(state.nameError, /may not be greater than 10|must not be greater than 10|10 characters/);
  // A `data-error-for` slot gets the error of a list item (`tags.0`).
  assert.match(state.tagSlot, /tags|5/i);
  // Nothing typed is lost: the form isn't reset or swapped.
  assert.equal(state.name, 'A name far too long');
  assert.equal(state.email, 'not-an-email');
  assert.equal(state.focused, 'rx-name');
  page.assertClean({ allow: [/422/] });

  // Fixed and sent again: the errors go and the toast says so.
  await page.type('#rx-name', 'Ana', { clear: true });
  await page.type('#rx-email', 'ana@example.com', { clear: true });
  await page.type('input[name=tags]', 'new', { clear: true });
  await page.click('#signup button[type=submit]');
  await page.waitFor(() => document.querySelector('.rx-toast')?.textContent.includes('Welcome, Ana.'), {
    message: 'the success toast',
  });
  const cleared = await page.eval(() => document.querySelectorAll('[aria-invalid="true"]').length);
  assert.equal(cleared, 0, 'aria-invalid is cleared after a successful submit');
}));

test('htmx requests carry the CSRF token (a POST without it would be refused)', () => browser.with(async (page) => {
  await page.goto(`${app.url}/form`);
  const sent = page.once('Network.requestWillBeSent', (p) => p.request.method === 'POST');
  await page.type('#rx-name', 'Bo');
  await page.type('#rx-email', 'bo@example.com');
  await page.click('#signup button[type=submit]');
  const { request } = await sent;
  const headers = Object.fromEntries(Object.entries(request.headers).map(([k, v]) => [k.toLowerCase(), v]));
  assert.ok(headers['x-csrf-token'], `the CSRF header: ${JSON.stringify(headers)}`);
  await page.waitFor(() => document.querySelector('.rx-toast')?.textContent.includes('Welcome, Bo.'));
  page.assertClean();
}));

test('a plain form comes back with its errors and old input, never the password', () => browser.with(async (page) => {
  await page.goto(`${app.url}/plain`);
  await page.type('#rx-name', 'Ana');
  await page.type('#rx-email', 'nope');
  await page.type('#rx-password', 'hunter2');
  const loaded = page.once('Page.loadEventFired');
  await page.click('#plain button[type=submit]');
  await loaded;
  await page.settle();
  const state = await page.eval(() => ({
    name: document.querySelector('#rx-name').value,
    email: document.querySelector('#rx-email').value,
    password: document.querySelector('#rx-password').value,
    summary: document.querySelector('[data-rx-error-summary]')?.textContent || '',
    invalid: document.querySelector('#rx-email').getAttribute('aria-invalid'),
  }));
  assert.equal(state.name, 'Ana');
  assert.equal(state.email, 'nope');
  assert.equal(state.password, '', 'passwords are never refilled');
  assert.match(state.summary, /email/i);
  assert.equal(state.invalid, 'true');
  page.assertClean();
}));
