# The course app: coffee

The app that learn.renox.rs builds, stage by stage: **coffee**, a coffee shop's menu. Each
`stage-NN/` is a complete Renox app, the previous stage plus that stage's changes.

| Stage | What it adds |
|---|---|
| `stage-00` | exactly what `rnx new coffee` writes |
| `stage-01` | the `menu` module; the `categories` table by hand and `menu_items` by `make:migration --auto`; the `Category` and `MenuItem` models |
| `stage-02` | the demo menu (`src/app/seed.rs`) and tests that query it through the models |
| `stage-03` | the pages `menu.index` and `menu.show`, with typed contexts (`ViewData`) |
| `stage-04` | the `<app-price-tag>` and `<app-stock-badge>` components |
| `stage-05` | the form that adds an item (`#[model(form)]`, `menu.create`, `menu.store`) |
| `stage-06` | the `DrinkBuilder` live component on the home page: state, actions, `rx-click`, `rx-model` |
| `stage-07` | the `StockCounter` live component on the item page, the `MenuItem` policy, authorization in every action |
| `stage-08` | `Factory` for `Category` and `MenuItem`, the tests of the counter and a whole journey |

## Layout

- Every stage is the package `coffee`, so a chapter can say `coffee::app()` and stage 0 equals
  `rnx new coffee`. That rules out one cargo workspace (it refuses two packages with one name):
  each stage is standalone, and the root `Cargo.toml` excludes `learn/course`.
- All stages share one `Cargo.lock` (no stage adds a dependency) and one target directory, set in
  [`.cargo/config.toml`](.cargo/config.toml), so Renox and its dependencies compile once.
  Never create a `target` directory inside a stage.
- The `renox` dependency is `{ path = "../../../crates/renox" }` in every stage.
- No stage commits `AGENTS.md`, `CLAUDE.md` (coding agents working in this repo would load them)
  or `.env`; `.env.example` is committed.
- Migrations have fixed versions (`20261012090000_create_categories_table`,
  `20261012090100_create_menu_items_table`), so chapters can name them.

## Regions

The site shows code from the stages. A *region* is marked by two comment lines in a stage's file,
`[learn:NAME]` and `[/learn:NAME]`, in the file's comment syntax (`// ` in Rust and JavaScript,
`{# … #}` in templates, `-- ` in SQL, `/* … */` in CSS, `# ` in TOML and `.env.example`). Names
are `[a-z0-9._-]+` and belong to one stage: the same name may exist in every stage. `learn/build.rs`
reads them (rules in `learn/build_course.rs`) and a chapter includes one with a `learn:include`
block. Stage 0 has no markers, nor do component files or anything before a page's root `<rx-page>`:
a chapter includes those whole with `file:`.

## Checks

```bash
learn/course/run.sh                         # every stage
learn/course/run.sh learn/course/stage-01   # one stage
```

`check-stage-00.sh RNX` fails when stage 0 differs from `rnx new coffee`. `run.sh` also compares
every `Cargo.lock` with stage 0's and, per stage, runs `cargo fmt --check`, clippy, the tests,
`view:check`, `db:check` and (from stage 1) `db:diff`, which must find nothing to change. Cargo
tells the stages apart by file times, so `run.sh` runs `cargo clean -p coffee` before and after
each one. Set `CARGO_TARGET_DIR` to keep the build output somewhere else.

## Making a stage

Copy the previous stage (without `target/`, `storage/`), make the changes with `rnx make:*`
where a generator does the job (the chapter shows the command), add the markers a chapter
includes, update the tests, then run `run.sh` on it.
