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
curl localhost:3000/api/products -H 'authorization: Bearer <token>'
```

## What's where

| Feature | Where |
|---|---|
| Wiring: `Auth` without registration pages (users and tokens only), the seeder | [src/lib.rs](src/lib.rs) |
| Routes, token issue and revoke, list/show/create products, validation, CORS and throttling | [src/app/products/mod.rs](src/app/products/mod.rs) |
| The table | [migrations](migrations) |

Routes: `POST /api/tokens`, `GET /api/products`, `POST /api/products`,
`GET /api/products/{id}`, `DELETE /api/tokens/current` (revokes the token the request used: "log out" on one device), `DELETE /api/tokens` (revokes all of the user's tokens).

## Things worth copying

- **Only the login route skips CSRF.** `POST /api/tokens` uses `.without_csrf()`; no cookie is
  involved. The other routes use `.require_auth()`, which answers 401 to a missing or wrong token.
- **JSON bodies are validated like forms.** `Valid<T>` works on JSON too; bad input gets
  `422 {"message", "errors"}`. The product name is checked with `.unique("products", "name")`.
- **Pagination as JSON.** `Json<Paginated<Product>>` returns the page with its `total`.
- **CORS and a rate limit on the whole group.** `.cors(&["https://app.example.com"])` and
  `.throttle(60, Duration::from_secs(60))` (60 requests a minute).

## Tests

```bash
cargo test -p api
```
