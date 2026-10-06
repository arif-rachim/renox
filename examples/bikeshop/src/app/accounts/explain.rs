//! "About this page" entries for the accounts area (see `crate::explain`).
//!
//! The sign-in pages and the account page come from Renox's `Auth` module
//! (`Auth::new().account()` in `src/lib.rs`); the bike shop only gives them
//! its look (`resources/views/renox/auth/layout.html`). Their entries live
//! here, with the customer account pages #238 adds.

use crate::explain::{Audience, Explanation, Feature, Flow, NotAPage};

/// Every sign-in page is drawn in the shop's version of Renox's auth layout.
const AUTH_MODULE: Feature = Feature {
    api: "Auth module",
    why: "Login, registration, password reset, email verification and the account \
          page are Renox's, added by one line (`.module(Auth::new().account())`), \
          so the shop writes none of that security-sensitive code itself.",
};

/// The shop's own look for Renox's pages.
const AUTH_LAYOUT: Feature = Feature {
    api: "View overrides (renox/auth/layout.html)",
    why: "A file named like a built-in view replaces it: the shop's \
          `renox/auth/layout.html` gives every sign-in page the brand, the motion \
          and this panel, without copying the pages themselves.",
};

const CSRF: Feature = Feature {
    api: "CSRF protection",
    why: "The form carries the session's token (`csrf_field()`), checked by the CSRF \
          middleware before the handler runs, so another site can't post it for you.",
};

const AUTH_DOCS: &[&str] = &[
    "docs/authorization.md#the-auth-modules-routes",
    "docs/ui.md#renoxs-own-pages",
];

/// The explanation of every page in this area.
pub fn entries() -> Vec<Explanation> {
    vec![
        Explanation {
            route: "login",
            path: "/login",
            title: "Log in",
            purpose: "Where customers and staff sign in. Customers go on to their \
                      orders, rentals and bikes; staff to the back office of the stores \
                      they work in.",
            who: "Everyone with an account: customers, cashiers, mechanics, store \
                  managers and the owner.",
            audience: &[
                Audience::Visitor,
                Audience::Customer,
                Audience::Staff,
                Audience::Owner,
            ],
            flow: Flow::Account,
            features: &[
                AUTH_MODULE,
                Feature {
                    api: "Valid<T>",
                    why: "The email and password are checked before anything else; a \
                          failed attempt comes back with the errors next to the fields \
                          (a 422 for htmx, a redirect with old input otherwise).",
                },
                Feature {
                    api: "Login throttle",
                    why: "Too many failed attempts for one email, one address or the \
                          pair lock that login for a while, so passwords can't be \
                          guessed by brute force.",
                },
                Feature {
                    api: "Sessions",
                    why: "Logging in gives the session a new id and stores the user's id \
                          with a fingerprint of the password hash: changing the password \
                          ends the other sessions.",
                },
                CSRF,
                AUTH_LAYOUT,
            ],
            under_hood: "On submit: the form is validated, the throttle counts the \
                         attempt, the user is looked up by the normalized email and the \
                         password checked with Argon2 (on a blocking thread, with a dummy \
                         hash for unknown emails so timing reveals nothing). On success \
                         the session id rotates, a `LoggedIn` event is emitted and the \
                         visitor goes back to the page they wanted (`Redirect::intended`).",
            docs: &[
                "docs/authorization.md#the-auth-modules-routes",
                "docs/routing.md#sessions",
                "docs/routing.md#rate-limits",
                "docs/validation.md#when-validation-fails",
            ],
            sources: &[
                "crates/renox-core/src/auth/module.rs",
                "crates/renox-core/src/auth/throttle.rs",
                "crates/renox-core/views/auth/login.html",
                "examples/bikeshop/resources/views/renox/auth/layout.html",
            ],
        },
        Explanation {
            route: "register",
            path: "/register",
            title: "Register",
            purpose: "A customer creates an account to buy online, book rentals and \
                      services, and see their bikes. Staff accounts are made by the \
                      store managers, not here.",
            who: "Visitors who want to become customers.",
            audience: &[Audience::Visitor],
            flow: Flow::Account,
            features: &[
                AUTH_MODULE,
                Feature {
                    api: "Password rules",
                    why: "The password must be long enough and confirmed; the rules are \
                          Renox's `Password` policy, shared with the reset and account \
                          pages.",
                },
                Feature {
                    api: "Valid<T>",
                    why: "Name, email and password are checked, and the email must be \
                          unique (a `unique` rule that asks the database).",
                },
                CSRF,
                AUTH_LAYOUT,
            ],
            under_hood: "On submit: validation (with the email lowercased before the \
                         unique check), the password hashed with Argon2, the user \
                         inserted and read back, a `Registered` event emitted, and the \
                         new customer logged in with a fresh session id.",
            docs: &[
                "docs/authorization.md#the-auth-modules-routes",
                "docs/validation.md#passwords",
            ],
            sources: &[
                "crates/renox-core/src/auth/module.rs",
                "crates/renox-core/src/auth/user.rs",
                "crates/renox-core/views/auth/register.html",
            ],
        },
        Explanation {
            route: "password.request",
            path: "/forgot-password",
            title: "Forgot your password?",
            purpose: "Asks for an email address and mails a link to choose a new \
                      password.",
            who: "Customers and staff who can't log in.",
            audience: &[Audience::Visitor, Audience::Customer, Audience::Staff],
            flow: Flow::Account,
            features: &[
                AUTH_MODULE,
                Feature {
                    api: "Mail",
                    why: "The reset link goes out as a mail rendered from Renox's \
                          template, in the visitor's language; with `MAIL_MAILER=log` \
                          it shows in `/_renox/mail` while developing.",
                },
                AUTH_LAYOUT,
            ],
            under_hood: "On submit: a random token is stored hashed in \
                         `password_reset_tokens`, and the mail with the link is sent. \
                         The answer is the same whether the email exists or not, so the \
                         page can't be used to find out who has an account.",
            docs: &[
                "docs/authorization.md#the-auth-modules-routes",
                "docs/mail.md#sending-a-mail",
            ],
            sources: &[
                "crates/renox-core/src/auth/passwords.rs",
                "crates/renox-core/views/auth/forgot-password.html",
            ],
        },
        Explanation {
            route: "password.reset",
            path: "/reset-password/{token}",
            title: "Choose a new password",
            purpose: "The page the reset mail's link opens: a new password, typed twice.",
            who: "Whoever received the reset mail.",
            audience: &[Audience::Visitor, Audience::Customer, Audience::Staff],
            flow: Flow::Account,
            features: &[
                AUTH_MODULE,
                Feature {
                    api: "Password rules",
                    why: "The same `Password` policy as registration, so a reset can't \
                          set a weaker password.",
                },
                AUTH_LAYOUT,
            ],
            under_hood: "The token in the address is checked against its hash and its \
                         age when the form is sent; the new password is hashed and \
                         saved, the token deleted, and every session of that user ends \
                         (their password fingerprint changed).",
            docs: AUTH_DOCS,
            sources: &[
                "crates/renox-core/src/auth/passwords.rs",
                "crates/renox-core/views/auth/reset-password.html",
            ],
        },
        Explanation {
            route: "password.confirm",
            path: "/confirm-password",
            title: "Confirm your password",
            purpose: "Asks for the password again before a sensitive action, such as \
                      deleting the account.",
            who: "Logged-in customers and staff.",
            audience: &[Audience::Customer, Audience::Staff],
            flow: Flow::Account,
            features: &[
                AUTH_MODULE,
                Feature {
                    api: "require_password_confirmed",
                    why: "Routes behind it send the user here first unless they confirmed \
                          recently, then back to where they were going.",
                },
                AUTH_LAYOUT,
            ],
            under_hood: "On submit the password is checked with Argon2 and the time of \
                         the confirmation is stored in the session.",
            docs: AUTH_DOCS,
            sources: &[
                "crates/renox-core/src/auth/account.rs",
                "crates/renox-core/views/auth/confirm-password.html",
            ],
        },
        Explanation {
            route: "verification.notice",
            path: "/verify-email",
            title: "Check your email",
            purpose: "Tells a new user to open the verification link mailed to them, \
                      with a button to send it again.",
            who: "Logged-in users whose email isn't verified yet.",
            audience: &[Audience::Customer],
            flow: Flow::Account,
            features: &[
                AUTH_MODULE,
                Feature {
                    api: "Signed URLs",
                    why: "The link in the mail is signed with the app's key, so it can't \
                          be forged or changed to verify someone else.",
                },
                AUTH_LAYOUT,
            ],
            under_hood: "Sending again posts to `verification.send`, which mails a new \
                         signed link to `verification.verify`.",
            docs: &[
                "docs/authorization.md#the-auth-modules-routes",
                "docs/routing.md#signed-urls",
            ],
            sources: &[
                "crates/renox-core/src/auth/verification.rs",
                "crates/renox-core/views/auth/verify-email.html",
            ],
        },
        Explanation {
            route: "account.show",
            path: "/account",
            title: "Your account",
            purpose: "Change your name, email and password, log out your other devices, \
                      or delete the account. Modules add their own sections (two-factor \
                      authentication, for example).",
            who: "Every logged-in customer and member of staff.",
            audience: &[Audience::Customer, Audience::Staff],
            flow: Flow::Account,
            features: &[
                AUTH_MODULE,
                Feature {
                    api: "Method spoofing",
                    why: "The forms send `PUT` and `DELETE` through a hidden `_method` \
                          field, so each action is its own route with its own name.",
                },
                Feature {
                    api: "UI kit: sheet",
                    why: "Deleting the account asks first, in the kit's sheet.",
                },
                AUTH_LAYOUT,
            ],
            under_hood: "Changing the password re-hashes it and ends the other sessions; \
                         \"log out other devices\" bumps `users.sessions_revoked_at`, \
                         checked on every request; deleting the account removes the user \
                         and their rows in Renox's tables, then logs out.",
            docs: &[
                "docs/authorization.md#the-auth-modules-routes",
                "docs/authorization.md#a-section-on-the-account-page",
                "docs/routing.md#method-spoofing",
            ],
            sources: &[
                "crates/renox-core/src/auth/account.rs",
                "crates/renox-core/views/auth/account.html",
            ],
        },
    ]
}

/// GET routes of this area that aren't pages (JSON, files, streams).
pub fn not_pages() -> Vec<NotAPage> {
    vec![NotAPage {
        route: "verification.verify",
        reason: "the signed link from the verification mail: it marks the email \
                 verified and redirects, showing no page of its own",
    }]
}
