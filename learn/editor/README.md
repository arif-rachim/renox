# learn.renox.rs's code editor

[CodeMirror 6](https://codemirror.net/), vendored as **one pinned, prebuilt bundle**
([#528](https://github.com/arif-rachim/renox/issues/528)). Nothing here runs on the server or
in the browser from npm: `bundle.sh` builds `../public/vendor/codemirror-<V>.js` once, and the
result is committed and compiled into `renox-learn` with the other public files.

| File | What |
|---|---|
| `package.json`, `package-lock.json` | the packages, at exact versions (no `^` or `~`) |
| `entry.mjs` | what the bundle exports: only CodeMirror's pieces, as `window.CM` |
| `bundle.sh` | `npm ci` and esbuild in `node:24-bookworm-slim` pinned by digest; copies the bundle and the licences to `public/vendor/` |
| `samples/` | the `/editor` playground's Rust and HTML text |
| `../public/editor.js` | learn's own glue (plain ES2022, not bundled): `window.LearnEditor` |
| `../public/vendor/NOTICE` | every package in the bundle, its version, licence and source |

## The editor in a page

A `<textarea data-learn-editor="rust|html" data-path="src/…">` becomes an editor when
`vendor/codemirror-<V>.js` and `editor.js` are loaded (both `<script src defer>`, so they work
under `CSP=strict`); the textarea is hidden and kept in step, so a form still posts it. The
editor is dark in both colour modes, like the code panels, and tokens get the docs'
highlighter classes (`hl-kw`, `hl-str`…), coloured by `learn.css`. In HTML mode `{{ … }}`,
`{# … #}` and `{% … %}` are marked `cm-rx-expr`, and `rx-*`, `app-*` and `live-*` tag names
`cm-rx-tag`. Tab indents; Esc, then Tab, leaves the editor (no keyboard trap).

```js
const editor = LearnEditor.mount(el, { language: "rust", doc: "fn main() {}", path: "src/main.rs" });
editor.getValue(); editor.setValue("…"); editor.reveal(3, 5); editor.destroy();
LearnEditor.editors.get("src/main.rs");   // every mounted editor, by path
```

`/editor` (noindex, in no sitemap) shows one editor of each kind; `/design` has a read-only
one under `editor`.

## Rebuilding, or bumping a version

Needs Docker or Podman (`CONTAINER=podman`). Behind a proxy, pass extra run flags in
`CONTAINER_ARGS` (for example `--network host -e HTTPS_PROXY`).

1. Change the versions: in the same container,
   `docker run --rm -u "$(id -u):$(id -g)" -e HOME=/tmp -v "$PWD/learn/editor:/work" -w /work <IMAGE from bundle.sh> npm install --save-exact --ignore-scripts @codemirror/view@<new> …`
   (`npm view <package> version` gives the latest). Keep every version exact.
2. Run `learn/editor/bundle.sh`. It deletes the old `codemirror-*.js` and licences and writes
   the new ones; the file is named after `@codemirror/view`'s version.
3. Update `CODEMIRROR` in `learn/src/editor.rs` to the new file name, and `../public/vendor/NOTICE`
   (every package and version in the lock, and the size the script printed).
4. `cargo test -p renox-learn --test editor` checks the name, the 900 000-byte budget and the
   NOTICE; `tests/browser/run.sh learn-editor` checks the editor in Chrome.

CI (job `learn-editor`) runs `bundle.sh` on a clean checkout and fails if `public/vendor/`
changes: the committed bundle must be exactly what the lock makes. Every package must be
MIT-licensed; if a new one isn't, ask the owner first.
