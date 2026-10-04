# renox-2fa

Two-factor authentication for [Renox](https://github.com/arif-rachim/renox) apps, when it is
finished: after the password, a six-digit code from an authenticator app (TOTP, RFC 6238:
Google Authenticator, Authy, 1Password, …), and one-time recovery codes for a lost phone.

> [!WARNING]
> **Not usable yet.** This crate is being built in steps (issues #170 to #173). Adding
> `TwoFactor::new()` to an app today creates the `two_factor` table and nothing else: nobody is
> asked for a code at login, and the account page shows nothing new.

```rust
use renox::prelude::*;
use renox_2fa::TwoFactor;

App::new()
    .module(Auth::new().account()) // the account page, where users will turn it on
    .module(TwoFactor::new())
```

What works today:

- the `two_factor` table, one row per user, with the secret sealed with `APP_KEY`
  (`TwoFactorCredential`, `TwoFactorCredential::enabled`);
- TOTP (`totp`): new secrets, codes, checking a code (each code works once), the
  `otpauth://` link for authenticator apps, and base32, tested against the RFCs' test vectors;
- the QR code of that link as an SVG (`qr::svg`).

Not done yet:

- turning it on and off from the account page (it will add a card there with
  `Registry::account_section`);
- the code challenge after the password (it will use `Registry::second_factor`);
- recovery codes (the table has a column for them, but nothing makes or checks them yet).

Until then, an app that needs a second login step today can write its own with
`Registry::second_factor`, as the "A second login step" part of docs/authorization.md shows.

Versioned with `renox`: use the same version for both.
