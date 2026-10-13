# learn.renox.rs

The Renox course: one app (a coffee shop's menu) built stage by stage, from `rnx new`
to a live component with tests. Each stage will have its chapter, screenshots, a short video
and a challenge solved in an editor that knows Renox and checked by compiling and testing the
student's code. The plan is the epic
[#538](https://github.com/arif-rachim/renox/issues/538); this directory starts with its
skeleton ([#523](https://github.com/arif-rachim/renox/issues/523)): the layout, the course's
outline on the home page, the sitemap, `robots.txt` and `llms.txt`.

It is a Renox app compiled into one binary (views and public files embedded), like
[`site/`](../site) (docs.renox.rs) and [`www/`](../www) (renox.rs).

| Where | What |
|---|---|
| [`src/lib.rs`](src/lib.rs) | the app: routes, JSON-LD (`Course`; per stage `LearningResource` + `BreadcrumbList`), sitemap, `llms.txt`, `/stages/{slug}.md` |
| [`src/course.rs`](src/course.rs) | the stages: number, title, summary, topics, the guide that goes deeper |
| [`resources/views/`](resources/views) | the layout, the home page, the 404 page |
| [`public/learn.css`](public/learn.css) | the design system: tokens first, then the components (paper and dark) |
| [`public/theme.js`](public/theme.js) | the light/dark toggle |
| [`src/design.rs`](src/design.rs), [`resources/views/design.html`](resources/views/design.html) | `/design`, the style guide |
| [`content/stages/`](content/stages) | one Markdown file per stage, `NN-slug.md`, compiled in by [`build.rs`](build.rs) |
| [`src/content.rs`](src/content.rs), [`src/directive.rs`](src/directive.rs), [`src/render.rs`](src/render.rs), [`src/code.rs`](src/code.rs), [`build_course.rs`](build_course.rs) | front matter and `Problem`s; the directives and their types; Markdown to HTML; the course app's regions, files and diffs |
| [`resources/views/stage.html`](resources/views/stage.html), [`public/learn.js`](public/learn.js) | a chapter's page; the code panels' copy buttons |
| [`tests/content.rs`](tests/content.rs) | the chapters have no problems, render in the layout, link to each other |
| [`tests/learn.rs`](tests/learn.rs) | every route, the tags, the stages' order, links into the docs |
| [`editor/`](editor), [`public/editor.js`](public/editor.js), [`src/editor.rs`](src/editor.rs) | the code editor: CodeMirror 6 vendored as one pinned bundle in `public/vendor/`, `window.LearnEditor`, the `/editor` playground ([editor/README.md](editor/README.md)) |
| [`tests/editor.rs`](tests/editor.rs) | the playground, the bundle's budget and its NOTICE |
| [`tests/editor_data.rs`](tests/editor_data.rs), [`editor/data/`](editor/data) | the autocomplete's data: Renox's API read from source with `syn`, each stage's models and tags (snapshots; `RENOX_UPDATE_SNAPSHOT=1`) |
| [`deploy/`](deploy) | the systemd socket and service |

## Design system

[#543](https://github.com/arif-rachim/renox/issues/543) fixed it before any chapter UI. Every
later issue uses these tokens and classes and adds no colour, size or font of its own.

- **It is renox.rs's** (`www/public/www.css`): Inter and JetBrains Mono, the orange accent, a
  dark header, footer and hero, mono uppercase eyebrows with a number (`001 The stages`),
  thin-bordered cards, grid-paper bands, and code panels like the landing page's terminal.
  Chapters sit on www's light paper; dark mode is www's dark palette (system preference, plus
  a toggle remembered in `localStorage` under `renox-learn:theme`, applied as `data-theme` on
  `<html>` by [`theme.js`](public/theme.js)).
- **Tokens** are at the top of [`learn.css`](public/learn.css): www's `:root` block, copied
  verbatim and checked by `the_www_tokens_match_renox_rs` (a change in www.css fails the test
  until learn copies it), then learn's own `--learn-*` constants and **roles** (page, surface,
  text, link, ok, warn, bad…) set on `body.learn` for paper and dark. The kit's `rx-button`,
  `rx-badge` and `<rx-icon>` are re-coloured through a short list of `--rx-*` overrides there.
  Use the roles (`var(--learn-text)`), not the raw colours, so a component follows the mode.
- **Below the line `/* == end of tokens == */` there is no colour literal and no
  `font-family`** (`the_stylesheet_keeps_colours_and_fonts_in_its_tokens`). A new colour is a
  new token above the line, with its contrast checked against the table in #543.
- **Classes:** the page frame uses unprefixed BEM (`nav`, `hero`, `eyebrow`, `stage`, `toc`,
  `pager`…); what the renderer puts inside `.prose` uses `learn-` (`learn-quiz`,
  `learn-challenge`, `learn-shot`, `learn-video`, `learn-code`, `learn-change`) plus the docs
  renderer's `site-code`, `site-callout` and `hl-*`. States are `is-*` or ARIA.
- **`/design`** (noindex, in no sitemap) renders every component with sample content.
  `design::COMPONENTS` lists them; an issue that adds a component adds its name there and a
  `data-component` section to `design.html`. Components that later issues fill (stage cards,
  challenge, quiz, progress, figures) are already there with their final markup: reuse the
  classes.
- **Check it:** `tests/browser/run.sh learn-design` opens `/design` at 1280 and 390 px in light
  and dark under `CSP=strict`, checks overflow, fonts, text contrast (WCAG AA) and the focus
  ring. `LEARN_SCREENS=/some/dir` also saves the eight full-page screenshots.

## Writing a chapter

A chapter is `content/stages/NN-slug.md`: `NN` is the stage's number (two digits) and `slug` the
stage's slug in [`src/course.rs`](src/course.rs). It starts with four front matter lines, all
required; `title` and `summary` must equal the outline's (a test fails otherwise):

```text
---
title: Set up and run an app
stage: 0
summary: Install rnx, create the coffee app and see it answer in the browser.
minutes: 10
---
```

The title is the page's `<h1>`, so the chapter's own headings start at `##` (`##` and `###` go in
the table of contents). Code blocks get the docs' code panel, `> [!TIP]` (also NOTE, IMPORTANT,
WARNING, CAUTION) makes a callout, and raw HTML is shown as text, except `<kbd>`.

Six **directives** pull in more. A directive is a fenced block whose info string is
`learn:<name>`, with one `key: value` per line (empty lines and lines starting with `//` are
ignored; a value may be quoted to keep spaces or a `:`):

````markdown
```learn:include
region: menu.item-model
```
````

| Directive | Keys (required in **bold**) | Rules |
|---|---|---|
| `learn:include` | exactly one of **region**, **file**; stage | `region` is `[a-z0-9._-]+`, marked `[learn:name]` in the stage's files (below); `file` is relative to the stage's folder (`src/lib.rs`), with no `..` and no leading `/`; `stage` is 0-99, the chapter's own by default. Shows the code under its file name. |
| `learn:changes` | none | What the chapter's stage changed against the stage before: a fold-out diff per added, changed or removed file. Not in stage 0. |
| `learn:screenshot` | **file**, **page**, **alt**, caption, viewport, wait, click, login | `file` is `[a-z0-9-]+.webp`, the image `public/img/sNN/<file>`; `page` is a path on the stage's app; `viewport` is `desktop` (1280 x 800, the default) or `phone` (390 x 844); `wait` is a selector awaited before the capture (`main`); `click` one selector clicked first; `login` a seeded user's email (the password is `password123`). See "Screenshots" below. |
| `learn:video` | **id**, **title**, caption | `id` is `[a-z0-9-]+`, a key of `content/media.json` |
| `learn:challenge` | **id** | `id` is `[a-z0-9-]+`, the directory `challenges/sNN/<id>/` |
| `learn:quiz` | **id**, **question**, `choice`…, `correct`, explain | `choice` and `correct` repeat, in order; exactly one `correct`, at least 2 options; `id` is `[a-z0-9-]+` and unique across the course; question, options and explain are inline Markdown |

Each key may appear once (except `choice`). Video draws the player from
`content/media.json` (below) and the challenge a stub until #531 fills it (one `*_html` function each in
`render.rs`); include and changes read the course app (below) and the quiz is complete (its
script is #535). An `include` of a region or file the stage doesn't have is a `Problem` too. A mistake is a `Problem` (`file:line:
message`, lines counted in the whole file): `rnx serve` shows it as a red box where it is, and
`the_content_has_no_problems` fails. A new key goes in the directive's list in `directive.rs`.

## Screenshots

The images in `public/img/sNN/` are taken from the course app by `media/screens.mjs`, which
reads the `learn:screenshot` blocks of the chapters (the one list of what to capture). Run it
by hand with Node 24 (no npm packages; Chrome as for `tests/browser`) and commit the result:

```bash
node learn/media/screens.mjs        # every stage
node learn/media/screens.mjs 3 5    # only stages 3 and 5
```

For each stage with screenshots it builds that stage's app (`cargo clean -p coffee`, then
`cargo build`, because every stage is the package `coffee`), serves it seeded with
`tests/browser/lib/app.mjs` (`start(..., { target })`), captures each page with
`Page.captureScreenshot` as webp (quality 80, animations off) and deletes the stage's
images that no block names. A console error fails the run. CI never regenerates them (fonts
and Chrome differ between machines); `tests/content.rs` checks that every directive's image
exists, is a webp of at most 300 KB, and that every image is used.

## Videos

The short videos in the chapters are recorded by script from the course app, so they never drift
from the code. The videos stay out of git: they go to the server's disk and are served from
`LEARN_MEDIA_URL` (default `/storage/media`: Renox's public disk, `STORAGE_PATH/app/public/media/`,
which Caddy can serve straight from disk; it answers `Range` with `206`, and `tests/media.rs`
pins that). What is committed is `content/media.json`, the manifest the `learn:video` directive
and `tests/content.rs` read. All steps are by hand, never in CI.

1. **Scene.** `media/scenes/<id>.json` (`s05-add-item`: a stage number first). It names a
   `viewport`, `poster_at` (seconds into the video), an optional `login` (signed in before the
   recording starts, so it isn't in the video) and `steps`, each with exactly one key: `goto` (a
   path), `pause` (ms), `type` (`[selector, text]`, one character every 70 ms), `click`
   (selector, then waits for htmx), `press` (a key), `wait` (a selector) or `scroll` (a
   selector). A scene is at most 60 s. A chapter uses it with a block (the title is the player's
   label, `id` a key of the manifest):

   ````
   ```learn:video
   id: s05-add-item
   title: Adding a menu item: errors appear next to the fields
   caption: One line under the video.
   ```
   ````
2. **Record.** `node learn/media/record.mjs [id…]` (every scene without ids; Node 24 and Chrome as
   for `tests/browser`) builds the stage's app, serves it seeded, and writes Chrome's screencast
   frames to `media/out/frames/<id>/` (`f-NNNNN.jpg` and `frames.txt` for ffmpeg). A console
   error fails the run. Frames are big: a 10 s scene is a few MB, but delete `media/out/frames`
   after encoding.
3. **Encode.** `learn/media/encode.sh <id>…` runs ffmpeg in the image `renox-learn-ffmpeg:1`
   (built from `media/Containerfile.ffmpeg`: Debian bookworm, pinned by digest, ffmpeg 5.1; with
   `docker` or `podman`, no network at run time) and writes `out/<id>.webm` (VP9), `.mp4` (H.264,
   `+faststart`), `.webp` (the poster) and `.duration`: 1280x720, 30 fps, silent. An mp4 over
   4 MB is reported, not refused.
4. **Manifest.** `node learn/media/manifest.mjs [id…]` writes `content/media.json` (sizes and
   SHA-256 of each file, the scene's hash, the date), keeping the entries it doesn't touch.
   Commit it. The `?v=` on each URL is the start of the file's hash, so a re-encoded video isn't
   served from a cache.
5. **Upload.** The owner runs `LEARN_MEDIA_DEST=user@host:/path/to/storage/app/public/media/
   learn/media/upload.sh` (or `LEARN_MEDIA_HOST`, `LEARN_MEDIA_USER` and `LEARN_MEDIA_PATH`).
   The destination is read from the environment only: no host, user, key or path is in the
   repository. It rsyncs `out/*.webm|mp4|webp`, then checks each manifest file over HTTP
   (`LEARN_URL`, default `https://learn.renox.rs`): `200` and the manifest's `Content-Length`.

`node --test learn/media/scene.test.mjs` checks the scene format, `frames.txt` and the manifest
merge without a browser. `LEARN_MEDIA_URL` must start with `/` (the same origin, which the default CSP
allows for `<video>`); another value is logged and the default used. A media host later needs
`media-src` in the CSP.

## Progress

`public/progress.js` keeps the student's progress in the browser only, in
`localStorage["renox-learn:v1"]`; nothing is sent to the server, and the site reads the same
without the script or with storage blocked. The colour theme is stored apart
(`renox-learn:theme`, `theme.js`), so a reset leaves it alone.

```json
{
  "v": 1,
  "last": { "stage": "migrations", "at": "2026-10-11T09:30:00.000Z" },
  "stages": { "setup": { "visited": "2026-10-11T09:00:00.000Z", "done": "2026-10-11T09:12:00.000Z" } },
  "quizzes": { "where-it-runs": { "choice": 1, "correct": true, "at": "2026-10-11T09:05:00.000Z" } },
  "challenges": { "s02/cheap-items": { "done": "2026-10-11T09:41:00.000Z" } }
}
```

- `stages` is keyed by stage slug, `quizzes` by quiz id, `challenges` by the section's
  `data-challenge`. A value that is missing, not JSON, not an object or not `v: 1` means "start
  empty"; unknown fields are kept when saving.
- A stage is *visited* when its page loads and *done* when the student presses the button at the
  end. A quiz is checked in the browser on Check. A challenge is stored when the
  `learn:challenge-done` event arrives (`detail: { challenge, done }`), and its section is marked
  `is-passed` on the next load.
- The home page shows the Continue link, the done marks and a two-click reset.

## The course app

`learn/course/stage-00` ... `stage-08` are the app the chapters build: **`coffee`**, a coffee
shop's menu (stages 0-2 so far: [#525](https://github.com/arif-rachim/renox/issues/525)). Each
stage is a complete app, the previous stage plus that stage's changes, and every one is the
package `coffee`, so stage 0 is exactly what `rnx new coffee` writes and a chapter can say
`coffee::app()`. They are not a cargo workspace (it refuses two packages of one name): each is
standalone, all use one `Cargo.lock` and build into one target directory
(`learn/course/.cargo/config.toml`), so the dependencies compile once. Stages hold no
`AGENTS.md`/`CLAUDE.md` and no `.env`.

Chapters show this code through regions: two comment lines in a stage's file,
`// [learn:menu.item-model]` and `// [/learn:menu.item-model]` (`{# … #}` in templates, `-- `
in SQL, `/* … */` in CSS, `# ` in TOML), in every stage that has them: the key is (stage, name).
`build.rs` reads them (`build_course.rs`, rules in its header) and writes the regions, every
file and the diff against the previous stage into `src/code.rs`. Marker lines never show in
includes or diffs. Stage 0 has none (it must equal `rnx new`), and neither do component files or
anything before a page's root `<rx-page>`: include those whole with `file:`.

```bash
learn/course/run.sh                         # every stage
learn/course/run.sh learn/course/stage-01   # one stage
```

`run.sh` checks that stage 0 is still what `rnx new coffee` makes (`check-stage-00.sh`), that
every `Cargo.lock` is stage 0's, and runs per stage `cargo fmt --check`, clippy, the tests,
`view:check`, `db:check` and (from stage 1) `db:diff`, which must find nothing to change. Cargo
tells the stages apart by file times, so it runs `cargo clean -p coffee` between them. Never
create a `target` directory inside a stage.

## Run it

```bash
cd learn
cp .env.example .env
cargo run            # or: rnx serve
```

Then open <http://127.0.0.1:3000>.

[tests/learn.rs](tests/learn.rs) checks the tags and the structured data, that the stages are
numbered in order, and that every link to `docs.renox.rs/docs/…` names a page the docs site has
(`renox_site::content::PAGES`).

## Where it runs

`learn.renox.rs` runs on the owner's server, like the docs and renox.rs; GitHub holds no key
to it.

- [`.github/workflows/release-learn.yml`](../.github/workflows/release-learn.yml) ("Release
  build (learn)") tests and builds `renox-learn` on Ubuntu 24.04 when `learn/`, the docs site's
  code or the crates change on `main`. It uploads the stripped binary as the artifact
  `renox-learn-<sha>` (with `REVISION` and a sha256), kept 30 days. It can also be run by hand
  from the Actions tab.
- The server pulls each new `renox-learn-<sha>` the way it pulls `renox-site-<sha>` and
  `renox-www-<sha>`: it checks the sha256, installs the binary, restarts the service (socket
  activation: visitors wait, nobody is refused) and checks `/health`.

### By hand

```bash
cargo build --release -p renox-learn        # from the repository root
sudo useradd --system --home /opt/renox-learn renox-learn
sudo mkdir -p /opt/renox-learn/storage
sudo cp target/release/renox-learn /opt/renox-learn/
sudo cp learn/.env.example /opt/renox-learn/.env   # then edit it, below
sudo chown -R renox-learn /opt/renox-learn
sudo cp learn/deploy/renox-learn.service learn/deploy/renox-learn.socket /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now renox-learn.socket
sudo systemctl start renox-learn
```

Set these in `/opt/renox-learn/.env`:
- `APP_ENV=production` and `APP_DEBUG=false`;
- `APP_KEY=`, from `rnx key:generate --show`;
- `APP_URL=https://learn.renox.rs` (canonical links, the sitemap and the JSON-LD use it);
- `APP_PORT=3100` (the socket's port);
- `TRUSTED_PROXIES=127.0.0.1` and `TRUSTED_HOSTS=learn.renox.rs`.

Then the proxy, for example Caddy:

```text
learn.renox.rs {
    reverse_proxy 127.0.0.1:3100
}
```
