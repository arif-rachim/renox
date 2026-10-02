# Faster builds while developing

Rust compiles everything before the first page appears. After that, `rnx serve` rebuilds only
your crate on each change, and templates, translations and `public/` files reload without a
build. These settings make both steps quicker.

## What `rnx new` already does

- **Argon2 and BLAKE2 are optimised in dev builds** (`[profile.dev.package.argon2]` and
  `[profile.dev.package.blake2]`, both `opt-level = 3`; Argon2 hashes with BLAKE2). Password
  hashing is slow on purpose, and very slow unoptimised; every login test would pay for it.
- **Debug info is only line tables** (`[profile.dev] debug = "line-tables-only"`). Backtraces
  keep file and line numbers, while binaries are much smaller and linking, the part of each
  rebuild you wait for, is faster. Use `debug = true` when you need a debugger that shows
  variables.

## A faster linker

Linking is most of an incremental rebuild. With [mold](https://github.com/rui314/mold) (Linux) or
lld, add `.cargo/config.toml` to the app:

```toml
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
```

On macOS the default linker (ld-prime) is already fast. On Windows, use `rust-lld`:
`rustflags = ["-C", "link-arg=-fuse-ld=lld"]` for `x86_64-pc-windows-msvc`.

## Fewer dependencies

Renox's default features are `fake` (the `renox::fake` re-export used by factories), `http`
(real requests for `state.http`, which brings reqwest; the test fake works without it) and
`server-events` (`analytics::ServerEvent`, which needs `http`). An app that uses none of them
can drop them:

```toml
renox = { git = "…", rev = "…", default-features = false }
# or keep some: default-features = false, features = ["fake"]
```

`postgres`, `s3`, `uuid` and `xlsx` (Excel exports of data grids) are off unless you turn them
on. `s3` is the heaviest: it turns on object_store's `aws` feature, which brings reqwest and
aws-lc-rs (a C crypto library); it is not the AWS SDK.

TLS uses rustls with the `ring` provider, so no C crypto library (aws-lc) is compiled; only
SQLite's C source is.

## Sharing compiled dependencies

Several apps on one machine can share compiled dependencies with
[sccache](https://github.com/mozilla/sccache) (`RUSTC_WRAPPER=sccache`). The first build of each
app then reuses the others' work.

## Docker

The Dockerfile from `rnx make:deploy` builds dependencies in their own layer (cargo-chef). Docker
reuses that layer until `Cargo.toml` or `Cargo.lock` change, so after a code change only your
crate is compiled. Commit `Cargo.lock`.
