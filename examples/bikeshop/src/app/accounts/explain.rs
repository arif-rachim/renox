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

/// The social login buttons under the sign-in forms.
const OAUTH_BUTTONS: Feature = Feature {
    api: "renox-oauth",
    why: "\"Continue with Google\" and \"Continue with GitHub\" appear under the form \
          when their keys are set (`GOOGLE_CLIENT_ID`…), and not at all otherwise, so \
          the same code runs on a laptop with no keys. The plugin does the OAuth \
          round trip with PKCE and a single-use `state`, and links only by an address \
          the provider verified.",
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
                OAUTH_BUTTONS,
                Feature {
                    api: "renox-2fa (Registry::second_factor)",
                    why: "Someone with two-factor login on (every member of staff, any \
                          customer who chose it) isn't logged in after the password: \
                          the login waits at `/two-factor/challenge` for the code from \
                          their phone.",
                },
                CSRF,
                AUTH_LAYOUT,
            ],
            under_hood: "On submit: the form is validated, the throttle counts the \
                         attempt, the user is looked up by the normalized email and the \
                         password checked with Argon2 (on a blocking thread, with a dummy \
                         hash for unknown emails so timing reveals nothing). On success \
                         the session id rotates, a `LoggedIn` event is emitted and the \
                         visitor goes back to the page they wanted (`Redirect::intended`). \
                         A member of staff without two-factor login is then sent to set it \
                         up before the staff side opens (`src/app/staff/two_factor.rs`).",
            docs: &[
                "docs/authorization.md#the-auth-modules-routes",
                "docs/routing.md#sessions",
                "docs/routing.md#rate-limits",
                "docs/validation.md#when-validation-fails",
                "docs/oauth.md#what-your-users-see",
                "docs/two-factor.md#what-your-users-see",
            ],
            sources: &[
                "crates/renox-core/src/auth/module.rs",
                "crates/renox-core/src/auth/throttle.rs",
                "crates/renox-oauth/views/login_options.html",
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
                Feature {
                    api: "Auth::on_registered",
                    why: "The hook runs after the user is saved and before they're logged \
                          in, for `/register` and for a first social login alike: it adds \
                          the `customers` row that orders, rentals and bikes point at, and \
                          saves the language they signed up in. If it fails, Renox removes \
                          the user again, so there is never a login without a customer.",
                },
                Feature {
                    api: "Auth::verify_email",
                    why: "A mail with a signed link asks the new customer to confirm the \
                          address, so mails about orders and rentals reach the right person.",
                },
                OAUTH_BUTTONS,
                CSRF,
                AUTH_LAYOUT,
            ],
            under_hood: "On submit: validation (with the email lowercased before the \
                         unique check), the password hashed with Argon2, the user \
                         inserted and read back, then `accounts::registration::on_registered` \
                         inserts the `customers` row and sets `users.locale`; a `Registered` \
                         event is emitted, the verification mail sent, and the new customer \
                         logged in with a fresh session id. A walk-in who already has a \
                         record at the counter isn't linked by email here (the address \
                         isn't verified yet): they claim it from an invitation.",
            docs: &[
                "docs/authorization.md#the-auth-modules-routes",
                "docs/validation.md#passwords",
            ],
            sources: &[
                "crates/renox-core/src/auth/module.rs",
                "crates/renox-core/src/auth/user.rs",
                "crates/renox-core/views/auth/register.html",
                "examples/bikeshop/src/app/accounts/registration.rs",
                "examples/bikeshop/tests/accounts.rs",
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
            title: "My account",
            purpose: "Everything a customer has with the shop, in one place: their \
                      name and login, contact details and address, whether their ID was \
                      checked (needed to rent), their bikes, orders, rentals, service \
                      visits, plan and payments, how they want to be told about each, \
                      their language, and their data (download it, or delete the \
                      account). Staff see the parts that concern a login: password, \
                      devices, two-factor authentication.",
            who: "Every logged-in customer, and staff for their own login.",
            audience: &[Audience::Customer, Audience::Staff],
            flow: Flow::Account,
            features: &[
                AUTH_MODULE,
                Feature {
                    api: "Registry::account_section",
                    why: "Each area adds its own card to Renox's account page from its \
                          module's `register`: a template and a closure that loads what \
                          it shows for the logged-in user. The accounts area adds contact, \
                          ID check, notifications, language and privacy; sales, rentals, \
                          the workshop and plans add their lists; `renox-2fa` and \
                          `renox-oauth` add theirs. No area edits another's code, and \
                          `accounts::section_order` keeps the page in a sensible order.",
                },
                Feature {
                    api: "View overrides (renox/auth/account.html)",
                    why: "The page is Renox's handler with the shop's template: a file of \
                          the same name in `resources/views` replaces the built-in one, so \
                          the page sits in the shop's layout, with its cards on a CSS grid \
                          (two columns on a wide screen, one on a phone), while the \
                          password, devices and deletion forms still post to Renox's routes.",
                },
                Feature {
                    api: "Valid<T> + #[derive(Validate)]",
                    why: "The contact, notification and language forms are typed structs \
                          with their rules on the fields (`required`, `max`, `one_of`, \
                          `exists(\"countries\", \"id\")`); a mistake comes back next to \
                          the field with what was typed kept.",
                },
                Feature {
                    api: "UI kit: toggle_buttons",
                    why: "The notification preferences are one segmented control per kind \
                          (both, mail, in the app, none) and the language another: the \
                          kit's radio-based `toggle_buttons`, keyboard-usable, so no custom \
                          \"matrix of toggles\" was needed.",
                },
                Feature {
                    api: "Method spoofing",
                    why: "The forms send `PUT` and `DELETE` through a hidden `_method` \
                          field, so each action is its own route with its own name.",
                },
                Feature {
                    api: "AccountDeleted event",
                    why: "Deleting the account is Renox's route (it asks for the password \
                          first). The shop listens to the `AccountDeleted` event it \
                          announces and makes the customer anonymous: personal data goes, \
                          orders and payments stay for the books, plans are cancelled, the \
                          ID document's files are deleted.",
                },
                Feature {
                    api: "Job (queue)",
                    why: "\"Download my data\" only queues `ExportMyData`: a worker gathers \
                          everything into a JSON file on the storage disk and mails a link \
                          that works for seven days (`Storage::temporary_url`), so the page \
                          answers at once however much history there is.",
                },
            ],
            under_hood: "Loading runs each section's closure in order (a query or two each: \
                         the customer, their address with city and country, the countries \
                         for the select). Contact: the city is found by name in the country \
                         or added, the address saved, the customer updated. Notifications: \
                         the choices are stored as JSON in `users.notification_preferences`, \
                         which every notification reads (`accounts::channels_for`). \
                         Language: saved in `users.locale` and in the session; Renox's \
                         notifications write mails in `users.locale` by themselves. Changing \
                         the password ends the other sessions; \"log out other devices\" bumps \
                         `users.sessions_revoked_at`. Deleting: Renox deletes the login, then \
                         `accounts::privacy::on_account_deleted` anonymises the customer in a \
                         transaction and deletes `customers/{id}/` from the disk.",
            docs: &[
                "docs/authorization.md#a-section-on-the-account-page",
                "docs/authorization.md#the-auth-modules-routes",
                "docs/ui.md#renoxs-own-pages",
                "docs/validation.md#derivevalidate",
                "docs/queue.md#a-job",
                "docs/mail.md#localized-notifications",
                "docs/routing.md#method-spoofing",
            ],
            sources: &[
                "examples/bikeshop/src/app/accounts/mod.rs",
                "examples/bikeshop/src/app/accounts/preferences.rs",
                "examples/bikeshop/src/app/accounts/privacy.rs",
                "examples/bikeshop/resources/views/renox/auth/account.html",
                "examples/bikeshop/resources/views/accounts/sections/contact.html",
                "examples/bikeshop/resources/views/accounts/sections/notifications.html",
                "examples/bikeshop/tests/accounts.rs",
                "crates/renox-core/src/auth/account.rs",
            ],
        },
        Explanation {
            route: "notifications.index",
            path: "/notifications",
            title: "Notifications",
            purpose: "The customer's in-app notifications: an order ready to collect, a \
                      rental due back, a bike ready at the workshop, the next plan visit. \
                      The same list opens as a panel from the bell in the top bar, where \
                      new ones arrive while the page is open.",
            who: "Logged-in customers, and staff for what concerns them.",
            audience: &[Audience::Customer, Audience::Staff],
            flow: Flow::Account,
            features: &[
                Feature {
                    api: "Auth::notifications",
                    why: "One builder call adds the `notifications.*` routes (this page, \
                          mark read or unread, delete, clear), `unread_notifications` in \
                          every view, and the live stream; the shop writes none of it.",
                },
                Feature {
                    api: "UI kit: notification_bell",
                    why: "The bell in both layouts shows the unread count; a click opens \
                          the latest in a panel, and new ones arrive as a toast and in the \
                          badge. Without JavaScript it is a link to this page.",
                },
                Feature {
                    api: "DatabaseMessage",
                    why: "A notification sent on `Channel::Database` stores a \
                          `DatabaseMessage` (a status, a title, a line, a link), the shape \
                          the bell and this page show.",
                },
                Feature {
                    api: "Server-Sent Events (Hub)",
                    why: "`/notifications/stream` is a Server-Sent Events stream: Renox's \
                          in-process `Hub` wakes it when a notification is stored, and it \
                          polls the table every 15 seconds for ones stored by another \
                          process (a queue worker on its own).",
                },
                Feature {
                    api: "Notification::channels",
                    why: "Whether a notification lands here at all is the customer's \
                          choice on their account page: every notification in the app \
                          asks `accounts::channels_for(to, Kind)`, which turns \"mail\", \
                          \"in the app\", \"both\" or \"none\" into Renox's channels.",
                },
            ],
            under_hood: "The page reads the user's rows of `notifications`, newest first. \
                         Opening one marks it read and follows its link; the bell's stream \
                         ends after five minutes and the browser opens a new one, so a \
                         logged-out session doesn't keep one open.",
            docs: &[
                "docs/mail.md#database-notifications",
                "docs/mail.md#the-bell",
                "docs/mail.md#how-new-ones-arrive",
            ],
            sources: &[
                "crates/renox-core/src/auth/inbox.rs",
                "crates/renox-core/views/notifications.html",
                "examples/bikeshop/src/app/accounts/preferences.rs",
                "examples/bikeshop/resources/views/layouts/app.html",
            ],
        },
        Explanation {
            route: "accounts.claim",
            path: "/claim/{customer}/{email}",
            title: "Claim your record",
            purpose: "A walk-in customer the cashier already knows opens the link from \
                      their invitation and links their past purchases, rentals and bikes \
                      to the account they just made (or already had).",
            who: "A customer who was invited by staff, logged in with the invited address.",
            audience: &[Audience::Customer],
            flow: Flow::Account,
            features: &[
                Feature {
                    api: "Signed URLs (ValidSignature)",
                    why: "The link is `state.signed_url(\"accounts.claim\", …)`: the \
                          customer's id, the address it went to and an expiry (seven days) \
                          signed with `APP_KEY`. Changing any of them, or opening it late, \
                          answers 403, so nobody can claim another record by editing the \
                          address bar.",
                },
                Feature {
                    api: "Routes::require_auth",
                    why: "The link needs a login: a guest goes to the login page (with a \
                          link to register) and Renox brings them back here afterwards \
                          (`Redirect::intended`).",
                },
                Feature {
                    api: "Transactions",
                    why: "Claiming moves anything the new account already had onto the \
                          walk-in record and links it, in one transaction; the update only \
                          touches a record with no login yet, so it works once.",
                },
                Feature {
                    api: "audit::record",
                    why: "The claim is written to the audit log (`customer.claimed`), \
                          since it gives a login access to someone's history.",
                },
            ],
            under_hood: "Loading checks the signature, then whether the record is still \
                         unclaimed and the logged-in address is the invited one, and counts \
                         the record's orders, rentals and bikes (three `COUNT` queries). The \
                         button posts to the same signed address: in a transaction the \
                         walk-in row gets the user's id and address, the rows of the account's \
                         own customer record (`orders`, `payments`, `rentals`, \
                         `customer_bikes`) move to it, and that empty record is deleted.",
            docs: &[
                "docs/routing.md#signed-urls",
                "docs/routing.md#guards",
                "docs/authorization.md#sensitive-actions-and-the-audit-trail",
            ],
            sources: &[
                "examples/bikeshop/src/app/accounts/claim.rs",
                "examples/bikeshop/resources/views/accounts/claim.html",
                "examples/bikeshop/resources/views/mail/accounts/claim_invitation.html",
                "examples/bikeshop/tests/accounts.rs",
            ],
        },
        Explanation {
            route: "accounts.invite",
            path: "/staff/customers/{customer}/invite",
            title: "Invite a customer to their account",
            purpose: "At the counter: send a walk-in customer (someone with a record but \
                      no login) a mail with a link to claim their record online, so their \
                      past purchases and bikes are there when they sign up.",
            who: "Cashiers and managers (`customers.manage` in the store they work in).",
            audience: &[Audience::Cashier, Audience::Manager, Audience::Owner],
            flow: Flow::Account,
            features: &[
                Feature {
                    api: "Routes::require_permission",
                    why: "`customers.manage`, checked in the active store \
                          (`access::staff_routes`): a mechanic gets a 403, a guest the login \
                          page. Customers belong to the company, so any store may invite.",
                },
                Feature {
                    api: "Valid<T> + #[derive(Validate)]",
                    why: "The address is `required` and an `email`; a mistake comes back \
                          next to the field.",
                },
                Feature {
                    api: "Signed URLs",
                    why: "The mail's link is signed and expires in seven days \
                          (`state.signed_url`), so it needs no table of invitations.",
                },
                Feature {
                    api: "queue_mail",
                    why: "The mail is queued (`state.queue_mail`), so the counter doesn't \
                          wait for the mail server; a worker sends it with retries.",
                },
            ],
            under_hood: "The record must exist, have no login and not be deleted (else a \
                         404). Sending saves the address on the record, builds the signed \
                         link, renders `mail/accounts/claim_invitation.html` in the staff \
                         member's language and queues it, then comes back with a toast.",
            docs: &[
                "docs/routing.md#signed-urls",
                "docs/mail.md#sending-a-mail",
                "docs/authorization.md#roles-per-branch-a-role-in-one-store-for-a-while",
            ],
            sources: &[
                "examples/bikeshop/src/app/accounts/claim.rs",
                "examples/bikeshop/resources/views/accounts/invite.html",
                "examples/bikeshop/tests/accounts.rs",
            ],
        },
    ]
}

/// GET routes of this area that aren't pages (JSON, files, streams).
pub fn not_pages() -> Vec<NotAPage> {
    vec![
        NotAPage {
            route: "verification.verify",
            reason: "the signed link from the verification mail: it marks the email \
                     verified and redirects, showing no page of its own",
        },
        NotAPage {
            route: "notifications.stream",
            reason: "the bell's Server-Sent Events stream, not a page",
        },
        NotAPage {
            route: "oauth.redirect",
            reason: "renox-oauth: sends the browser to Google or GitHub (a redirect)",
        },
        NotAPage {
            route: "oauth.callback",
            reason: "renox-oauth: the provider sends the browser back here; it logs in or \
                     links, then redirects",
        },
    ]
}
