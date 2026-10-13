// #528: learn.renox.rs's code editor. On /editor (the playground) both
// CodeMirror editors mount from their textareas, Rust is highlighted with the
// docs' colours, the HTML mode marks Renox's `{{ … }}` and `rx-*` tags, typing
// keeps the hidden textarea in step, Esc then Tab leaves an editor, and nothing is logged or blocked, also
// under CSP=strict.

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

for (const csp of ['relaxed', 'strict']) {
  describe(`learn.renox.rs's editor (CSP=${csp})`, () => {
    let app;

    before(async () => {
      app = await start('renox-learn', 'learn', { env: { CSP: csp, QUEUE_WORKERS: '0', SCHEDULER: 'false' } });
    });

    after(async () => {
      await app?.stop();
    });

    test('both editors mount, highlight and keep their textareas in step', () =>
      browser.with(async (page) => {
        await page.goto(`${app.url}/editor`);
        await page.waitFor(() => document.querySelectorAll('.cm-editor').length === 2, { message: 'two editors' });
        assert.equal(await page.eval(`window.LearnEditor.editors.size`), 2);
        assert.equal(await page.eval(`document.querySelectorAll('textarea[data-learn-editor][hidden]').length`), 2);

        // Rust: highlighted tokens in a colour other than the text's.
        const coloured = await page.eval(() => {
          const editor = document.querySelector('[data-learn-editor="rust"]').nextElementSibling;
          const text = getComputedStyle(editor.querySelector('.cm-content')).color;
          return [...editor.querySelectorAll('.cm-line span')].filter((s) => getComputedStyle(s).color !== text).length;
        });
        assert.ok(coloured >= 10, `${coloured} coloured tokens in the Rust editor`);

        // HTML: Renox's expressions and tags are marked.
        const marks = await page.eval(() => {
          const editor = document.querySelector('[data-learn-editor="html"]').nextElementSibling;
          return {
            expr: [...editor.querySelectorAll('.cm-rx-expr')].map((e) => e.textContent),
            tags: [...editor.querySelectorAll('.cm-rx-tag')].map((e) => e.textContent),
          };
        });
        assert.ok(marks.expr.includes('{{ item.name }}'), `expressions: ${marks.expr}`);
        assert.ok(marks.tags.includes('rx-table'), `tags: ${marks.tags}`);
        assert.ok(marks.tags.includes('app-price-tag'), `tags: ${marks.tags}`);

        // Typing goes into the hidden textarea too.
        await page.eval(() => window.LearnEditor.editors.get('src/app/menu/model.rs').reveal(1, 1));
        for (const ch of '// hi\n') await page.send('Input.insertText', { text: ch });
        const value = await page.eval(`document.querySelector('[data-learn-editor="rust"]').value`);
        assert.ok(value.startsWith('// hi\n'), `the textarea follows: ${JSON.stringify(value.slice(0, 20))}`);
        assert.equal(await page.eval(() => window.LearnEditor.editors.get('src/app/menu/model.rs').getValue().startsWith('// hi')), true);

        // Tab indents inside the editor; Esc then Tab leaves it (no keyboard trap).
        await page.press('Tab');
        assert.equal(await page.eval(() => document.activeElement.classList.contains('cm-content')), true, 'Tab stays in the editor');
        await page.press('Escape');
        await page.press('Tab');
        const left = await page.eval(() => !window.LearnEditor.editors.get('src/app/menu/model.rs').view.dom.contains(document.activeElement));
        assert.equal(left, true, 'Esc, Tab leaves the editor');

        // No horizontal scroll on the page (the editor scrolls inside).
        assert.equal(await page.eval(`document.documentElement.scrollWidth - document.documentElement.clientWidth`), 0);
        page.assertClean();
      }));

    test('the editor fits a phone', () =>
      browser.with(
        async (page) => {
          await page.goto(`${app.url}/editor`);
          await page.waitFor(() => document.querySelectorAll('.cm-editor').length === 2, { message: 'two editors' });
          assert.equal(await page.eval(`document.documentElement.scrollWidth - document.documentElement.clientWidth`), 0);
          page.assertClean();
        },
        { width: 390, height: 844 },
      ));
  });
}
