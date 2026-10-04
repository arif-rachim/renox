# renox-2fa

Two-factor authentication for [Renox](https://github.com/arif-rachim/renox) apps: after the
password, a six-digit code from an authenticator app (TOTP, RFC 6238: Google Authenticator,
Authy, 1Password, …), and one-time recovery codes for a lost phone.

```rust
use renox::prelude::*;
use renox_2fa::TwoFactor;

App::new()
    .module(Auth::new().account()) // the account page, where users turn it on
    .module(TwoFactor::new())
```

It is built on two of Renox's extension points: the second login step
(`Registry::second_factor`) and sections on the account page (`Registry::account_section`).

**Status:** in progress (issue #146). This version has the `two_factor` table (the secret sealed
with `APP_KEY`), TOTP and base32 tested against the RFCs' test vectors, and the QR code as SVG.
Turning it on from the account page, the login challenge and recovery codes come next.

Versioned with `renox`: use the same version for both.
