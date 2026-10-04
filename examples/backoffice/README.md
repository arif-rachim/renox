# Back office example

The back office of a small business: customers, products with a
stock ledger, invoices from draft to paid, online payments through Midtrans
or Xendit, staff with roles, the activity log and the company's settings.
It is what Filament's demo shows for Laravel, built from Renox's parts: data
grids, the UI kit's forms and sheets, `renox::chart`, the `Permissions` and
`Audit` modules, the queue, notifications and webhooks.

```text
cp .env.example .env    # optional: the settings this example reads
cargo run -- migrate
cargo run -- db:seed    # 150 invoices; admin@, cashier@ and warehouse@example.com / password123
cargo run               # http://127.0.0.1:3000
```

There is no sign-up page: staff are added by an admin. Log in as
`admin@example.com` to see everything, or as `cashier@example.com` or
`warehouse@example.com` to see what each role may do.

## What to try

- **Sign in.** The login page wears the company's name, address and
  colour from the settings (`resources/views/renox/auth/layout.html`
  replaces Renox's layout for every sign-in page).
- **The dashboard.** Billed and collected in the period chosen (7 days to
  this year) against the period before, what is unpaid, invoices past their
  due date and products below their minimum.
- **Write an invoice.** Invoices → New invoice: choose a customer, add lines
  (the kit's `repeater`; each line's errors show in the line), save a draft.
  Issue it: the stock leaves, one ledger row per line, in the same
  transaction. Not enough of something and nothing changes (409). Void it
  and the stock comes back. Print opens a page for paper.
- **Get paid.** "Paid in cash" marks it paid. With a gateway chosen in
  Settings and its key in `.env`, "Payment link" asks Midtrans or Xendit for
  a payment page; when the customer pays, the gateway's webhook marks the
  invoice paid and the cashiers' bell rings.
- **Stock.** A product's page shows its ledger. "Adjust stock" receives
  deliveries, writes off damage, or sets the count from the shelf; stock
  never goes below zero. Products → More → "Import from CSV…" reads a CSV
  file (`sku,name,price,stock`; "Download the CSV template" gives an empty
  one) with `renox::import`: each row is checked with `ProductRow`'s rules,
  like a form, and the good rows land in one transaction, a savepoint each.
  Refused rows stay listed in the sheet with their row numbers and why.
- **Actions with more to them.** "New product" is a two-step wizard in a
  sheet (`wizard_action`): Next checks the step with the server's rules, and
  an error found at the end opens the step it belongs to. A product's "⋯"
  menu (`action_group`) has "Duplicate", the new-product form filled from a
  copy (`Product::replicate`), and "Export ledger", the product's ledger as
  CSV (`Grid::export_as`). The product grid has "Duplicate" as a row action.
- **Export in the background.** On Invoices, filter (say, Status = Issued),
  tick "select all matching" and choose "Export in the background". A job
  makes the CSV with the grid's filters and stores it on a disk of its own
  (`App::disk("exports", …)` in [src/lib.rs](src/lib.rs): `storage/exports`,
  or a bucket with `EXPORTS_DISK=s3`); the bell holds the link (it works for
  a day).
- **Grids everywhere.** Customers are edited in place (cells send `PATCH`);
  invoices group by status, total in the footer and remember their filters;
  products activate and deactivate in bulk; the activity log pages and
  exports like the rest.
- **Roles.** Staff → Add someone (they get a mail to verify their address
  and see only the verification page until they do); Roles changes what
  they may do. An admin can't take away their own admin role. Log in as the
  cashier: no Staff, Activity or Settings, and no stock changes; as the
  warehouse: no invoices or customers to change.
- **Settings.** The company's name, address and colour, the invoice prefix,
  tax and days to pay, and the payment gateway. Changes go to the activity
  log with what they were before.

## Where things are

| Part | Files |
|---|---|
| Roles and permissions | `src/lib.rs` (`ROLES`), `src/app/mod.rs` (routes, guards) |
| Invoices | `src/app/invoices/mod.rs`, `model.rs`, views `invoices/*` |
| Payments | `src/app/invoices/payments.rs` (payment pages, webhooks, `mark_paid`) |
| Exports | `src/app/invoices/export.rs` (`ExportInvoices` job, `ExportReady`) |
| Stock ledger | `src/app/products/stock.rs` (`change`, `StockMovement`) |
| CSV import | `src/app/products/import.rs` (`renox::import`, `ProductRow`), the `import_action` in `resources/views/products/index.html` |
| Wizard, action group, duplicate | `resources/views/products/index.html` (`wizard_action`, `action_group`), `show.html`, `replicate.html`; `replicate` and `export_ledger` in `src/app/products/mod.rs` |
| Staff | `src/app/staff.rs` |
| Activity log | `src/app/activity.rs` (a model over `audit_logs`) |
| Settings | `src/app/settings.rs`, shared as `company` (`src/lib.rs`) |
| Dashboard | `src/app/dashboard.rs`, `resources/views/dashboard/show.html` |
| The frame (all from the kit) | `resources/views/layouts/app.html`: `rx-shell`, `sidebar`, `navbar`, `page_header`s in the pages |
| Branded sign-in | `resources/views/renox/auth/layout.html` (the built-in layout with the brand mark), `resources/views/layouts/_brand.html` (the settings' colour as the kit's `--rx-accent`) |
| Tests | `tests/backoffice.rs` |

## Patterns worth copying

- **Stock that can't go negative** without reading first: one
  `UPDATE … SET stock = stock + ? WHERE id = ? AND stock >= ?` through
  `Query::increment`, and the ledger row written in the same transaction.
  The `UPDATE` changing no row means "not enough".
- **A state change claimed once**: `UPDATE invoices SET status = 'issued'
  WHERE id = ? AND status = 'draft'`; a second click finds nothing to change
  and gets a 409.
- **Exports from a job with the grid's filters**: the bulk action carries
  the grid's query string, and the job builds a `GridRequest` from it.
- **A model over a framework table** (`Activity` over `audit_logs`) to show
  it in a grid.
- **Settings as typed rows**: one `settings` row per key, read into a
  `#[derive(Validate)]` struct with defaults, the same struct the form posts.
