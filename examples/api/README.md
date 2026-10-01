# examples/api

A JSON API for a mobile app or a separate front end. Callers trade an email and password for an
API token, then send it as `Authorization: Bearer <token>`. Read it when your app needs an API
next to (or instead of) HTML pages.

```bash
cd examples/api
cargo run -- migrate
cargo run -- db:seed             # demo@example.com / password123, two products
cargo run                        # http://127.0.0.1:3000
```

Get a token, then call the API with it:

```bash
curl -X POST localhost:3000/api/tokens -H 'content-type: application/json' \
  -d '{"email":"demo@example.com","password":"password123","device":"curl"}'
# → {"token":"1|…","abilities":["products:read","products:write"],"expires_at":"…","user":{…}}
curl localhost:3000/api/products -H 'authorization: Bearer <token>'
# → {"items":[…],"per_page":20,"next_cursor":null}   (next page: ?cursor=<next_cursor>)
curl -X POST localhost:3000/api/products -H 'authorization: Bearer <token>' \
  -H 'content-type: application/json' -d '{"name":"Susu","price":12000}'
```

Add `"read_only":true` to the login body for a token that can only read: writing with it gets
`403`.

## What's where

| Feature | Where |
|---|---|
| Wiring: `Auth` without registration pages (users and tokens only), the nightly token cleanup, the seeder | [src/lib.rs](src/lib.rs) |
| Routes and their abilities, token issue and revoke, list/show/create/delete products, validation, CORS and throttling | [src/app/products/mod.rs](src/app/products/mod.rs) |
| The `api` rate limiter: per user, or per IP for guests | [src/lib.rs](src/lib.rs) |
| The table | [migrations](migrations) |

Routes: `POST /api/tokens`; with `products:read`: `GET /api/products`, `GET /api/products/{id}`;
with `products:write`: `POST /api/products`, `DELETE /api/products/{id}`; any valid token:
`DELETE /api/tokens/current` (revokes the token the request used: "log out" on one device),
`DELETE /api/tokens` (revokes all of the user's tokens).

## Things worth copying

- **Only the login route skips CSRF.** `POST /api/tokens` uses `.without_csrf()`; no cookie is
  involved. The other routes use `.require_auth()`, which answers 401 to a missing, wrong or
  expired token.
- **Tokens get abilities and an expiry.** `user.create_token_with(&db, device, &["products:read",
  "products:write"], Some(expires_at))` issues a token that works for 30 days; a read-only login
  gets only `products:read`. Routes ask for an ability with `.require_ability("products:write")`
  (403 without it; logged-in browser sessions always pass). Like every route layer it guards only
  the routes added before it, so reads and writes are separate groups, merged and then wrapped
  in `.require_auth()` (added last, so it answers 401 before any ability is checked).
- **Expired tokens are cleaned up.** The `Auth` module adds the `tokens:prune` command (`cargo run
  -- tokens:prune` deletes tokens that expired over a day ago); the app also runs the same cleanup
  every night with `.schedule(|s| s.daily_at("03:00", …))` calling
  `renox::auth::prune_expired_tokens`.
- **JSON bodies are validated like forms.** `Valid<T>` works on JSON too; bad input gets
  `422 {"message", "errors"}`. The product name is checked with `.unique("products", "name")`.
- **ULIDs as public ids.** `Product::id` is a `Ulid` (`01J9Z3…`, a `TEXT PRIMARY KEY`), made
  when the product is saved. Clients see ids that sort by creation time but don't reveal how
  many products there are, and `Path<Ulid>` answers 404 for a malformed one.
- **Cursor pagination for the list.** `Product::query().cursor_paginate(&db, cursor, 20)` returns
  `Json<CursorPage<Product>>`: `{"items", "per_page", "next_cursor"}`, newest first. The client
  sends `next_cursor` back as `?cursor=…` until it is `null`. Unlike page numbers, rows added in
  between don't shift the pages, and there is no `COUNT(*)`.
- **CORS and a rate limit on the whole group.** `.cors(&["https://app.example.com"])` and
  `.throttle_by("api")`. The `api` limiter (`App::rate_limiter` in lib.rs) picks the limit per
  request: 120 a minute per user, 10 a minute per IP for guests, who can only log in. Over the
  limit: `429` with `Retry-After`. Counting per user means several apps behind one office IP
  don't share a limit.

## Tests

```bash
cargo test -p api
```

[tests/api.rs](tests/api.rs) logs in through the API and checks the JSON with
`assert_json_path("items", json!([]))`, `json_path("errors.name.0")` and `assert_json`
(the listed keys only), including that ids are 26-character ULIDs and a malformed one is a
404. Time is moved, not waited for: `app.travel(29 * DAY)` keeps a token
working and two more days expire it, and `app.travel(61 s)` lets a rate-limited guest try again.
