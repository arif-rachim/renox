# Releasing Renox

How a release goes to crates.io. Only a maintainer with publish rights on the seven crates
(`renox`, `renox-core`, `renox-macros`, `renox-cli` and the plugins `renox-2fa`,
`renox-editors` and `renox-oauth`) can do it. All seven share the workspace's version and are
released together.

## The machine that publishes

- A crates.io account with a **verified email address** (crates.io refuses to publish without
  one), and publish rights on the seven names. The release candidates (`1.0.0-rc.1` to `rc.4`)
  created them; a new crate (a new plugin) gets its name at its first `cargo publish`, which
  gives the publisher those rights.
- `cargo login` on the machine that publishes. The token stays in `~/.cargo/credentials.toml`:
  never put it in the repository, an issue, a chat or an environment variable that a script
  prints.
- A release candidate is worth it before a big release: publish `x.y.0-rc.1`, check that
  docs.rs builds the API reference and that `cargo install renox-cli --version x.y.0-rc.1 &&
  rnx new demo` works from crates.io, then publish `x.y.0`. Cargo never picks a pre-release
  unless asked (`renox = "1.0.0-rc.1"`).

## Every release

1. **Main is green.** The last CI run on `main` passed, including PostgreSQL, chaos, the CLI
   jobs and the semver checks.
2. **Choose the version** (see [docs/stability.md](docs/stability.md)): a fix is a patch,
   anything new a minor, a breaking change a major. Set it in the workspace `Cargo.toml` in
   seven places that must agree: `[workspace.package] version` and the `version` of `renox`,
   `renox-core`, `renox-macros`, `renox-2fa`, `renox-editors` and `renox-oauth` under `[workspace.dependencies]` (written
   `=1.2.0`: the crates are released in lockstep and pin each other exactly, since the macros
   write code against renox-core's items of the same release).
   While 1.0 is a release candidate, other places name the version too; change all of them:
   - the install lines in `README.md` (quick start) and `docs/tutorial.md`
     (`cargo install renox-cli --version …`);
   - the `git clone --branch v…` line in `README.md` ("Use Renox with Claude Code");
   - the version in `README.md`'s "Status" section;
   - the example dependency line in `docs/stability.md` (`renox = "…"`).
   `grep -rn "1.0.0-rc" README.md docs` finds them.
3. **The changelog.** In `CHANGELOG.md`, rename "Unreleased" to `## 1.2.0 · 2026-11-01` (the
   version and the date) and start a new empty "Unreleased" above it.
4. **Check everything** (as in [CONTRIBUTING.md](CONTRIBUTING.md)), then a dry run of the
   crates that don't need another Renox crate on crates.io first:
   ```bash
   cargo publish --dry-run -p renox-macros
   cargo publish --dry-run -p renox-core
   cargo publish --dry-run -p renox-cli
   ```
   (`renox-cli` doesn't depend on the other Renox crates.) `renox` can't be dry-run before
   this version of `renox-core` and `renox-macros` is on crates.io, and `renox-2fa`,
   `renox-editors` and `renox-oauth` not before `renox` is: they depend on them.
5. **Commit and merge** the version and changelog as a pull request, as for any change.
6. **Publish in this order** from an up-to-date `main` (each waits until the previous one is in
   the index):
   ```bash
   cargo publish -p renox-macros
   cargo publish -p renox-core
   cargo publish -p renox
   cargo publish -p renox-cli
   cargo publish -p renox-2fa    # plugins depend on renox, so they go after it
   cargo publish -p renox-editors
   cargo publish -p renox-oauth
   ```
   `renox-core` and `renox-macros` use `renox` only as a path dev-dependency, which
   `cargo publish` leaves out, so the order has no cycle.
7. **Tag the commit:** `git tag v1.2.0 && git push origin v1.2.0`. Apps made by
   `rnx new` from crates.io link their `AGENTS.md` to the docs at that tag.
8. **Check the release:**
   - docs.rs shows `renox` and `renox-core` (built with `postgres`, `uuid` and `xlsx`), and
     `renox-2fa`, `renox-editors` and `renox-oauth`;
   - `cargo install renox-cli` then `rnx new demo`: `demo/Cargo.toml` has
     `renox = { version = "1.2" }` and `cargo test` passes in it.
9. **A GitHub release** for the tag, with the version's changelog section as its notes.

## After the first release

- The README gets the crates.io and docs.rs badges, and the quick start becomes
  `cargo install renox-cli` (keep the `--git` line for the latest `main`).
- The `semver` CI job compares with the release on crates.io instead of the base branch, and
  stops being informational: drop `--baseline-rev` and `continue-on-error` in
  `.github/workflows/ci.yml`.
- `rnx new` keeps pinning git commits when `rnx` itself is installed from git, so the
  development flow doesn't change.

## A broken release

Yank it (`cargo yank --version 1.2.0 renox-core`, and the other four of that version), fix
it, and publish the next patch version. A yanked version stays for apps that already lock it,
but new apps don't get it. Never reuse a version number.
