// #528: learn.renox.rs's code editor. On /editor (the playground) both
// CodeMirror editors mount from their textareas, Rust is highlighted with the
// docs' colours, the HTML mode marks Renox's `{{ … }}` and `rx-*` tags, typing
// keeps the hidden textarea in step, Esc then Tab leaves an editor, and nothing
// is logged or blocked, also under CSP=strict. #529: the suggestions (Renox's
// API, the stage's models and columns, the chain rule, tags and attributes).

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

    test('suggestions come from Renox\'s API and the stage', () =>
      browser.with(async (page) => {
        await page.goto(`${app.url}/editor`);
        await page.waitFor(() => document.querySelectorAll('.cm-editor').length === 2, { message: 'two editors' });
        for (const path of ['src/app/menu/model.rs', 'resources/views/menu/index.html']) {
          assert.equal(await page.eval((p) => window.LearnEditor.editors.get(p).ready.then(() => true), path), true);
        }
        // Types `text` into the editor at `path`, emptied first, and waits for `labels` among the suggestions.
        const suggest = async (path, start, text, labels) => {
          await page.eval(
            (p, doc) => {
              const editor = window.LearnEditor.editors.get(p);
              editor.setValue(doc);
              editor.view.dispatch({ selection: { anchor: doc.length } });
              editor.focus();
            },
            path,
            start,
          );
          for (const ch of text) await page.send('Input.insertText', { text: ch });
          const shown = () => [...document.querySelectorAll('.cm-tooltip-autocomplete .cm-completionLabel')].map((e) => e.textContent);
          await page.waitFor(
            (want) => {
              const have = [...document.querySelectorAll('.cm-tooltip-autocomplete .cm-completionLabel')].map((e) => e.textContent);
              return want.every((w) => have.includes(w));
            },
            { message: `${labels} after ${JSON.stringify(text)} (shown: ${await page.eval(shown)})` },
            labels,
          );
          await page.press('Escape');
        };
        const rust = 'src/app/menu/model.rs';
        await suggest(rust, 'fn cheap(db: &Db) {\n    ', 'MenuItem::', ['find_or_404', 'PRICE', 'query']);
        await suggest(rust, 'fn cheap(db: &Db) {\n    ', 'MenuItem::query().', ['latest', 'where_', 'order_by']);
        await suggest(rust, 'fn cheap(db: &Db) {\n    ', 'MenuItem::query().where_(MenuItem::PRICE.', ['lt', 'eq']);
        await suggest(rust, '', 'renox::db::', ['Model', 'Query']);
        await suggest(rust, '', '#[derive(Mo', ['Model']);
        const html = 'resources/views/menu/index.html';
        await suggest(html, '', '<rx-', ['rx-table']);
        await suggest(html, '', '<app-', ['app-price-tag']);
        await suggest(html, '', '<rx-table ', [':rows', 'rx-if']);
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
