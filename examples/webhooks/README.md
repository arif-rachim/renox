# examples/webhooks

Payment gateway webhooks from Midtrans, Xendit and Stripe. When a provider calls back, the
order is marked paid. Each call is verified with that provider's signature, stored once per
event, and processed by a queue worker. Read it before taking card or e-wallet payments.

The secrets go in `.env`:

```bash
MIDTRANS_SERVER_KEY=SB-Mid-server-...    # Midtrans server key
XENDIT_CALLBACK_TOKEN=...                # Xendit callback verification token
STRIPE_WEBHOOK_SECRET=whsec_...          # Stripe endpoint signing secret
```

Only the providers you use need theirs; a call to a provider whose secret is missing fails.

```bash
cd examples/webhooks
cargo run -- migrate
cargo run -- db:seed             # pending orders INV-1, INV-2, INV-3
cargo run                        # http://127.0.0.1:3000 lists the orders
```

Point the provider's dashboard at `https://<your host>/webhooks/<provider>`
(`midtrans`, `xendit` or `stripe`). While developing, expose your machine with a tunnel such
as `cloudflared tunnel --url http://localhost:3000`. Xendit's check is the simplest to try by
hand:

```bash
curl -X POST localhost:3000/webhooks/xendit -H 'content-type: application/json' \
  -H 'x-callback-token: <XENDIT_CALLBACK_TOKEN>' \
  -d '{"id":"inv_1","external_id":"INV-1","status":"PAID"}'
```

Once the queue worker (part of `cargo run`) has handled the call, `/` shows INV-1 paid via
xendit.

## What's where

| Feature | Where |
|---|---|
| Wiring, the seeder | [src/lib.rs](src/lib.rs) |
| The order model, the idempotent `mark_paid`, the webhook routes and their registration | [src/app/payments/mod.rs](src/app/payments/mod.rs) |
| Midtrans: SHA-512 `signature_key` in the JSON body | [src/app/payments/midtrans.rs](src/app/payments/midtrans.rs) |
| Xendit: `x-callback-token` header | [src/app/payments/xendit.rs](src/app/payments/xendit.rs) |
| Stripe: timestamped HMAC-SHA256 in `Stripe-Signature`, older than five minutes refused | [src/app/payments/stripe.rs](src/app/payments/stripe.rs) |

## Things worth copying

- **One `impl Webhook` per provider.** `verify` checks the signature (a forged call gets 401),
  `event_id` names the event, and `handle` runs later in the queue.
- **Duplicates are dropped by event id.** A repeated call with the same `event_id` is stored
  once. Midtrans and Xendit use transaction id plus status, so each status change still counts.
- **Handlers are idempotent anyway.** `mark_paid` does nothing if the order is already paid,
  and only logs a warning for an unknown order.
- **Secrets are compared in constant time** with `webhook::same`, and read with
  `webhook::secret`.
- **Failed calls can be retried.** `cargo run -- webhook:failed` lists them;
  `cargo run -- webhook:retry <id>` queues one again.

## Tests

```bash
cargo test -p webhooks
```

[tests/payments.rs](tests/payments.rs) sends correctly signed, forged and repeated calls for
each provider.
