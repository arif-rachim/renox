// The bike shop's customer accounts (#238) in a browser: signing up, "My
// account" with every area's section on its CSS grid (desktop and phone),
// the notification bell, the preferences' segmented controls, the language
// switch remembered on the account, "About this page", and deleting the
// account from its sheet. Screenshots go to BIKESHOP_SCREENS when it's set.

import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { Browser } from './lib/cdp.mjs';
import { start } from './lib/app.mjs';

// Demo staff log in without setting up two-factor login (#239 makes it
// required; tests/browser/bikeshop-staff.test.mjs checks that it is).
const STAFF_2FA_OPTIONAL = { BIKESHOP_STAFF_2FA: 'optional' };

const PHONE = { width: 390, height: 844 };
let browser;
let app;

before(async () => {
  browser = await Browser.launch();
  // Seeded: countries for the address, the shop's stores.
  app = await start('bikeshop', 'examples/bikeshop', { seed: true, env: STAFF_2FA_OPTIONAL });
});

after(async () => {
  await app?.stop();
  await browser?.close();
});

async function shot(page, name) {
  if (process.env.BIKESHOP_SCREENS) await page.screenshot(join(process.env.BIKESHOP_SCREENS, `${name}.png`));
}

const fitsWidth = (page) => page.eval(() => document.documentElement.scrollWidth <= window.innerWidth);

/** Signs up through Renox's register page and lands logged in. */
async function signUp(page, email) {
  // Every test starts as a guest (the tabs share the browser's cookies).
  await page.send('Network.clearBrowserCookies');
  await page.goto(`${app.url}/register`);
  await page.type('#rx-name', 'Nia Lopez');
  await page.type('#rx-email', email);
  await page.type('#rx-password', 'password123');
  await page.type('#rx-password_confirmation', 'password123');
  await page.click('form button[type=submit]');
  await page.waitFor(() => location.pathname !== '/register', { message: 'registered' });
}

describe('bikeshop customer accounts', () => {
  test('sign up, then my account on desktop and phone', () =>
    browser.with(async (page) => {
      await signUp(page, 'nia.desktop@example.com');
      await page.goto(`${app.url}/account`);
      const text = await page.text('main');
      for (const part of ['Contact details', 'ID check', 'Notifications', 'Language', 'Your data', 'Two-factor authentication']) {
        assert.ok(text.includes(part), `the page has "${part}"`);
      }
      // The sections sit on a two-column grid on a wide screen…
      const columns = await page.eval(
        () => getComputedStyle(document.querySelector('.bs-account')).gridTemplateColumns.split(' ').length,
      );
      assert.equal(columns, 2, 'two columns on a desktop');
      // …and the bell is in the bar.
      assert.ok(await page.eval(() => !!document.querySelector('[data-rx-bell]')), 'the bell');
      assert.ok(await fitsWidth(page));
      await shot(page, 'account-desktop');

      // The panel explains the page.
      await page.click('[data-rx-open="about-page"]');
      await page.waitFor(() => document.querySelector('#about-page')?.open);
      assert.ok((await page.text('#about-page')).includes('Registry::account_section'));
      await page.press('Escape');

      await page.send('Emulation.setDeviceMetricsOverride', { ...PHONE, deviceScaleFactor: 1, mobile: true });
      await page.goto(`${app.url}/account`);
      const phone = await page.eval(
        () => getComputedStyle(document.querySelector('.bs-account')).gridTemplateColumns.split(' ').length,
      );
      assert.equal(phone, 1, 'one column on a phone');
      assert.ok(await fitsWidth(page), 'no sideways scrolling on a phone');
      await shot(page, 'account-phone');
      page.assertClean();
    }));

  test('notification choices and the language are saved', () =>
    browser.with(async (page) => {
      await signUp(page, 'nia.prefs@example.com');
      await page.goto(`${app.url}/account`);
      // Marketing is off until chosen; pick "Mail" with the keyboard-usable toggles.
      assert.ok(await page.eval(() => document.querySelector('[data-bs-pref="marketing"] input[value="none"]').checked));
      await page.click('[data-bs-pref="marketing"] input[value="mail"] + .rx-toggle__label');
      await page.click('#account-notifications button[type=submit]');
      await page.waitFor(() => document.querySelector('.rx-toast'), { message: 'a toast' });
      await page.waitFor(() => document.querySelector('[data-bs-pref="marketing"] input[value="mail"]')?.checked, {
        message: 'the choice is kept',
      });

      // Spanish, then the page speaks it.
      await page.click('#account-language input[value="es"] + .rx-toggle__label');
      await page.click('#account-language button[type=submit]');
      await page.waitFor(() => document.documentElement?.lang === 'es', { message: 'the page is in Spanish' });
      assert.ok((await page.text('main')).includes('Datos de contacto'));
      await shot(page, 'account-spanish');
      page.assertClean();
    }));

  test('deleting the account asks for the password in a sheet', () =>
    browser.with(async (page) => {
      await signUp(page, 'nia.leaving@example.com');
      await page.goto(`${app.url}/account`);
      await page.click('[data-rx-open="delete-account"]');
      await page.waitFor(() => document.querySelector('#delete-account')?.open, { message: 'the sheet' });
      assert.ok((await page.text('#delete-account')).includes("can't be undone"));
      await shot(page, 'account-delete');
      await page.type('#rx-password-delete', 'password123');
      await page.click('#delete-account button.rx-button--danger');
      await page.waitFor(() => location.pathname === '/', { message: 'back home' });
      await page.goto(`${app.url}/account`);
      await page.waitFor(() => location.pathname === '/login', { message: 'logged out' });
      page.assertClean();
    }));
});
