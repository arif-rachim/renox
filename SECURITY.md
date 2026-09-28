# Security policy

## Reporting a vulnerability

Please report security problems privately, not in a public issue:

- use GitHub's **Report a vulnerability** button on the
  [Security tab](https://github.com/arif-rachim/renox/security/advisories/new) of this
  repository.

Include what you found, how to reproduce it (a small app or a failing test is ideal), and
what an attacker could do with it. You'll get an answer within 7 days. Once there's a fix,
it's released with an advisory that credits you, unless you'd rather not be named.

Please give us a reasonable time to ship a fix before you publish details.

## Supported versions

Renox is before 1.0: only the latest commit on `main` (and, once published, the latest
release on crates.io) gets security fixes. From 1.0, the latest minor release of the
current major version does.

Apps made by `rnx new` are pinned to a Renox commit. To get a fix, move that pin (or the
version) forward and run `cargo update -p renox`.

## What's in scope

Anything in this repository: the `renox`, `renox-core`, `renox-macros` and `renox-cli`
crates, the templates and assets they ship, the files `rnx new` and `rnx make:*` write
(Dockerfile, systemd unit, …), and the examples when they show a pattern apps would copy.

Problems in a dependency (axum, sqlx, rustls, …) should go to that project. If Renox uses
it in an unsafe way, or its defaults expose apps, report it here too.

## What Renox already defends against

So you can tell a new problem from a known limit, `docs/audit/2026-09-pre-1.0.md` lists
what was audited before 1.0 and how each finding was fixed. In short:

- CSRF tokens on every unsafe request, sessions in an encrypted and signed cookie,
  `SameSite=Lax`, `Secure` behind `https://`;
- a Content-Security-Policy with nonces, security headers and HSTS;
- Argon2id passwords, a login lock per email and per IP, constant-time comparisons for
  tokens, signatures and webhook MACs;
- parameterized SQL everywhere, with column names checked against the model;
- uploads sniffed by content and served from a sandbox with `nosniff`;
- signed URLs with expiry, and redirects that never leave the site;
- `TRUSTED_PROXIES` so `X-Forwarded-For` is believed only from your proxy.

`docs/operations.md` covers running an app safely in production.
