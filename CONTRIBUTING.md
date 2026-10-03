# Contributing to Renox

Thanks for helping. Bug reports, docs fixes, examples and code are all welcome.

## Before you start

- **A bug:** open an issue with the smallest app or test that shows it, what you expected and
  what happened. For a security problem, see [SECURITY.md](SECURITY.md) instead.
- **A feature:** open an issue first. Renox keeps a small surface on purpose (see "Not
  planned" and "Decisions" in [ROADMAP.md](ROADMAP.md)), and it's better to agree on the API
  before you write it.
- **Docs and examples:** send a pull request directly.

## Set up

You need Rust 1.94 or later (the MSRV; `rust-version` in `Cargo.toml`). Docker is only needed
for the PostgreSQL, S3 and chaos tests.

```bash
git clone https://github.com/arif-rachim/renox && cd renox
cargo test --workspace
```

[CLAUDE.md](CLAUDE.md) describes the architecture, the conventions and the problems solved so
far. Read it before changing anything large; it's written for human contributors as much as
for coding agents.

## What every change needs

CI runs all of these; running them first saves a round trip:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test --workspace
```

- **Tests.** New behaviour gets a test, and a fix gets a test that failed before it.
  Integration tests live in `crates/renox/tests/it/` and use `renox::testing::TestApp`.
  Include the negative cases: bad input, a forged request, a failing dependency.
- **Both databases.** Anything that touches SQL must pass on PostgreSQL too:
  ```bash
  docker run -d --rm --name renox-pg --shm-size=512m -e POSTGRES_PASSWORD=postgres \
      -e POSTGRES_DB=renox_test -p 55432:5432 postgres:17-alpine
  TEST_DATABASE_URL=postgres://postgres:postgres@localhost:55432/renox_test \
      cargo test -p renox -p renox-core -p renox-cli -p postgres-app -p fields --features renox/postgres
  ```
- **Docs that compile.** Public items have doc comments (`#![warn(missing_docs)]` in `renox`,
  `renox-core` and `renox-macros`, so clippy's `-D warnings` refuses a public item without
  one), and their examples run as doctests.
  A new API also gets a line in [CHEATSHEET.md](CHEATSHEET.md) (compiled as a doctest too).
- **Generators.** A change to `rnx new` or `rnx make:*` must keep `tests/cli/run.sh` passing:
  it builds and tests an app made with every generator.
- **Docs and examples in step.** When a change adds or changes behaviour, update what
  describes it: [CHEATSHEET.md](CHEATSHEET.md), [README.md](README.md) (feature tour),
  [llms.txt](llms.txt), the guides in `docs/` (operations: new commands, tables that grow,
  config), the new-app agent guide (`crates/renox-cli/stubs/AGENTS.md.stub`), and the examples
  that show that area (use the new API where an example worked around its absence).
- **Stability.** Read [docs/stability.md](docs/stability.md) before changing a public type.
  Breaking changes go in [CHANGELOG.md](CHANGELOG.md) under "Breaking".

Other CI jobs you can run locally when your change touches their area:
`cargo clippy -p renox --no-default-features -- -D warnings`, the guard against C crypto in
default builds (`cargo tree -p hello -e normal -i aws-lc-rs` must print nothing),
`tests/chaos/run.sh sqlite|postgres`, `tests/cli/run.sh sqlite|postgres`, the S3 tests (see
the top of `crates/renox/tests/it/s3.rs`, and `cargo test -p uploads --features s3`),
`cargo hack check -p renox-core -p renox --each-feature --no-dev-deps` and `cargo deny check`.

- **UI changes.** Check pages in a real browser (desktop, a phone width, dark mode): several
  bugs only showed there (see CLAUDE.md §6.4).

## Style

- Code reads like the code around it: plain names, short functions, comments that say *why*.
- Everything in the repository is in English: code, comments, docs, example content, tests
  and commit messages. An example that needs a second language uses Spanish.
- Docs and messages are in plain English with short sentences. Error messages say what to do
  next (`there is no module `x`; create it with `rnx make:module x``).
- Commit messages explain what changed and why, with the details a reviewer needs.

## License

Renox is dual-licensed under MIT or Apache-2.0. Unless you say otherwise, any contribution
you submit is licensed the same way, without additional terms.
