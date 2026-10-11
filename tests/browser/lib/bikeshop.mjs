// Shared by the bikeshop tests that log in with the form's own fields.

/**
 * Logs in as a seeded demo user (password `password`): the login form's
 * own fields, its CSRF token too, sent from the page with fetch (a submit
 * clicked in CI sometimes never left the page, #342).
 *
 * #359: now and then the answer was 419. The token belongs to the session
 * cookie of the page that was loaded, and a cookie cleared or set by a
 * request still in flight from the page before can make them disagree. So a
 * 419 loads the form again and sends it once more (up to three times).
 *
 * #354: a tab that submitted the login form by a click can get no key presses
 * afterwards (in CI), so a test that presses keys after logging in uses this
 * too. `keepCookies` keeps the session the page already has (a language the
 * visitor chose before logging in, say) instead of starting a fresh one.
 *
 * @param {import('./cdp.mjs').Page} page
 * @param {string} url the app's address
 * @param {string} email
 * @param {{ keepCookies?: boolean }} [options]
 * @returns {Promise<string>} the last answer, `"<status> <path>"`
 */
export async function logInWithFetch(page, url, email, { keepCookies = false } = {}) {
  let answer = '';
  for (let attempt = 1; attempt <= 3; attempt++) {
    if (!keepCookies) await page.send('Network.clearBrowserCookies');
    await page.goto(`${url}/login`);
    await page.waitFor(() => location.pathname === '/login' && !!document.querySelector('form input[name=email]'), { message: 'the login form' });
    answer = await page.eval(async (e) => {
      const form = document.querySelector('form input[name=email]').form;
      form.elements.email.value = e;
      form.elements.password.value = 'password';
      const res = await fetch(form.action, { method: 'POST', body: new URLSearchParams(new FormData(form)) });
      return `${res.status} ${new URL(res.url).pathname}`;
    }, email);
    if (!answer.startsWith('419')) break;
  }
  return answer;
}
