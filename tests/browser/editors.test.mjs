// #268: editors.js (renox-editors) on examples/fields' product form: Trix
// keeps its hidden input in step, the Markdown toolbar and preview, CodeJar
// keeps its textarea in step, and each library loads only when needed.

import { after, before, test } from 'node:test';
import assert from 'node:assert/strict';
import { Browser, sleep } from './lib/cdp.mjs';
import { start } from './lib/app.mjs';

let browser;
let app;

before(async () => {
  app = await start('fields', 'examples/fields', { seed: true });
  browser = await Browser.launch();
});

after(async () => {
  await browser?.close();
  await app?.stop();
});

const onForm = (fn) =>
  browser.with(async (page) => {
    const loaded = [];
    page.on('Network.requestWillBeSent', (p) => loaded.push(new URL(p.request.url).pathname));
    await page.goto(`${app.url}/products/new`);
    await fn(page, loaded);
    page.assertClean();
  });

test('the rich text editor writes into its hidden field and takes no files', () =>
  onForm(async (page, loaded) => {
    await page.waitFor(() => !!document.querySelector('trix-editor')?.editor, { message: 'Trix ready' });
    assert.ok(loaded.some((p) => p.includes('/_renox/editors/trix-')), 'Trix was loaded for the page');
    await page.click('trix-editor');
    await page.type('trix-editor', 'Fresh beans');
    await page.waitFor(() => document.querySelector('#rx-details').value.includes('Fresh beans'));
    // Bold from the toolbar, with the keyboard.
    await page.eval(() => document.querySelector('trix-editor').editor.setSelectedRange([0, 5]));
    await page.focus('#rx-details-toolbar [data-trix-attribute="bold"]');
    await page.press('Enter');
    await page.waitFor(() => document.querySelector('#rx-details').value.includes('<strong>Fresh</strong>'));
    assert.equal(
      await page.eval(() => document.querySelector('#rx-details-toolbar [data-trix-attribute="bold"]').getAttribute('aria-pressed')),
      'true',
    );
    // Files are refused: nothing to attach to.
    const accepted = await page.eval(() => {
      const event = new Event('trix-file-accept', { cancelable: true, bubbles: true });
      document.querySelector('trix-editor').dispatchEvent(event);
      return !event.defaultPrevented;
    });
    assert.equal(accepted, false);
  }));

test('the Markdown toolbar wraps the selection and the preview is drawn by the server', () =>
  onForm(async (page) => {
    await page.type('#rx-description', 'strong coffee');
    await page.eval(() => {
      const area = document.querySelector('#rx-description');
      area.setSelectionRange(0, 6);
    });
    await page.click('[data-md="bold"]');
    assert.equal(await page.eval(() => document.querySelector('#rx-description').value), '**strong** coffee');
    // Undo keeps working (execCommand), so Ctrl+Z gives the old text back.
    await page.click('[data-md-preview]');
    await page.waitFor(() => document.querySelector('#rx-description-preview').innerHTML.includes('<strong>strong</strong>'), {
      message: 'the preview',
    });
    assert.equal(await page.eval(() => document.querySelector('[data-md-preview]').getAttribute('aria-pressed')), 'true');
    await page.click('[data-md-preview]');
    assert.ok(await page.eval(() => document.querySelector('#rx-description-preview').hidden));
  }));

test('the code editor keeps its textarea in step and lets Escape leave', () =>
  onForm(async (page, loaded) => {
    await page.waitFor(() => !!document.querySelector('[data-rx-code-editor] [contenteditable]'), { message: 'CodeJar ready' });
    assert.ok(loaded.some((p) => p.includes('/_renox/editors/codejar-')), 'CodeJar was loaded');
    await page.click('[data-rx-code-editor] [contenteditable]');
    await page.type('[data-rx-code-editor] [contenteditable]', '{"grind": "fine"}');
    await page.waitFor(() => document.querySelector('#rx-settings').value.includes('"grind"'));
    // Highlighted by Prism.
    assert.ok(await page.eval(() => !!document.querySelector('[data-rx-code-editor] .token')));
    // Escape, then Tab, leaves the editor (keyboard users aren't trapped).
    await page.press('Escape');
    await page.press('Tab');
    const inEditor = await page.eval(() => !!document.activeElement.closest('[data-rx-code-editor]'));
    assert.equal(inEditor, false);
    await sleep(50);
  }));
