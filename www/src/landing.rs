//! What the landing page says, as data the template renders on the server:
//! every word, number and code sample is in the HTML a crawler gets, and
//! public/www.js only animates it.

use serde::Serialize;

/// The docs site.
pub const DOCS: &str = "https://docs.renox.rs";
/// The repository.
pub const REPOSITORY: &str = "https://github.com/arif-rachim/renox";
/// The live demo.
pub const DEMO: &str = "https://bikeshop.renox.rs";
/// renox.rs's Google Analytics 4 property. `GA4_MEASUREMENT_ID` overrides it;
/// the tag is only sent in production (see the layout's `ga4`).
pub const GA4: &str = "G-48GJK3ZM4J";

/// A figure in the strip under the hero (counted from the repository).
#[derive(Serialize)]
pub struct Metric {
    pub value: u32,
    pub suffix: &'static str,
    pub label: &'static str,
}

pub fn metrics() -> Vec<Metric> {
    vec![
        Metric {
            value: 1750,
            suffix: "",
            label: "tests",
        },
        Metric {
            value: 90,
            suffix: "%",
            label: "line coverage, enforced",
        },
        Metric {
            value: 99,
            suffix: "",
            label: "rx-* tags for pages",
        },
        Metric {
            value: 130,
            suffix: "",
            label: "demo pages explained beside their code",
        },
        Metric {
            value: 6,
            suffix: "",
            label: "official plugins",
        },
        Metric {
            value: 112,
            suffix: "",
            label: "icons in the UI kit",
        },
    ]
}

/// The crates a Rust web app usually wires by hand ("twenty crates").
pub const CRATES: &[&str] = &[
    "axum",
    "tower",
    "sqlx",
    "tokio",
    "minijinja",
    "serde",
    "validator",
    "argon2",
    "lettre",
    "cookie",
    "tower-sessions",
    "apalis",
    "cron",
    "tracing",
    "reqwest",
    "object_store",
    "chrono",
    "uuid",
    "csrf",
    "governor",
    "rust-embed",
    "clap",
];

/// One of the eight feature cards.
#[derive(Serialize)]
pub struct Feature {
    pub icon: &'static str,
    pub title: &'static str,
    pub text: &'static str,
    pub code: &'static str,
    /// Its guide on docs.renox.rs.
    pub doc: &'static str,
}

pub fn features() -> Vec<Feature> {
    vec![
        Feature {
            icon: "⌁",
            title: "Routing & requests",
            text: "Named routes, groups, domains, guards, rate limits, signed URLs, route model binding.",
            code: "Routes::new().get(\"/\", home)",
            doc: "routing",
        },
        Feature {
            icon: "▤",
            title: "Models & migrations",
            text: "Typed columns, relations without N+1, migrations generated from your models, and db:check to catch drift. SQLite or PostgreSQL.",
            code: "Product::PRICE.lt(20_000)",
            doc: "relations",
        },
        Feature {
            icon: "✓",
            title: "Validation",
            text: "Form requests as structs with Laravel's rules, hooks, live validation, errors placed next to the field.",
            code: "Valid<NewOrder>",
            doc: "validation",
        },
        Feature {
            icon: "⚿",
            title: "Auth & permissions",
            text: "Login, registration, reset, verification, API tokens, roles per tenant, policies, two-factor login, OAuth.",
            code: ".require_permission(\"orders.refund\")",
            doc: "authorization",
        },
        Feature {
            icon: "⟳",
            title: "Queues & scheduler",
            text: "Jobs with retries, chains and batches; a cron scheduler that is safe across servers. No Redis needed.",
            code: "state.queue.dispatch(SendInvoice { … })",
            doc: "queue",
        },
        Feature {
            icon: "✉",
            title: "Mail & notifications",
            text: "SMTP with failover, mail views, notifications by mail, in the database and as a live stream.",
            code: "state.notify(user, &OrderShipped { … })",
            doc: "mail",
        },
        Feature {
            icon: "▦",
            title: "Pages as tags",
            text: "Write pages in HTML tags: 99 accessible rx-* tags, a data grid with exports, live components, htmx and Alpine.js built in.",
            code: "<rx-table :rows=\"orders\">",
            doc: "ui",
        },
        Feature {
            icon: "⬢",
            title: "One-binary deploys",
            text: "Views, translations, migrations and assets compiled in; systemd socket activation, so restarts refuse no connection.",
            code: "rnx build",
            doc: "operations",
        },
    ]
}

/// A layer of the request pipeline.
#[derive(Serialize)]
pub struct Layer {
    pub short: &'static str,
    pub name: &'static str,
    pub title: &'static str,
    pub text: &'static str,
    pub code: String,
}

pub fn pipeline() -> Vec<Layer> {
    let layer = |short, name, title, text, lang: &str, code: &str| Layer {
        short,
        name,
        title,
        text,
        code: renox_site::highlight::highlight(lang, code),
    };
    vec![
        layer(
            "HTTP",
            "request",
            "The request arrives",
            "Axum and Tokio underneath: async, on every core, nothing to install next to the binary.",
            "",
            "GET /orders/42 HTTP/1.1\nHost: shop.example.com\nCookie: renox_session=…",
        ),
        layer(
            "SEC",
            "security",
            "Security headers & CSP",
            "nosniff, frame options, HSTS and a Content-Security-Policy with a nonce per request, on every response. Trusted hosts and proxies too.",
            "",
            "content-security-policy: default-src 'self';\n  script-src 'self' 'nonce-r4Nd0m'\nstrict-transport-security: max-age=31536000",
        ),
        layer(
            "SES",
            "session",
            "Sessions",
            "An encrypted, signed cookie by default; a database driver when you outgrow it. Flash data, remember-me, per-device logout.",
            "rust",
            "session.flash(\"status\", \"Saved\")?;\nsession.put(\"cart\", &cart)?;",
        ),
        layer(
            "AUTH",
            "auth",
            "Authentication",
            "The user is loaded once per request, from the session or a Bearer token with its abilities, with roles and permissions per tenant.",
            "rust",
            "Routes::new()\n    .get(\"/orders\", index)\n    .require_permission(\"orders.view\")",
        ),
        layer(
            "CSRF",
            "csrf",
            "CSRF",
            "Every unsafe method is checked: a hidden field for forms, the X-XSRF-TOKEN header for htmx and JSON.",
            "html",
            "<form method=\"post\">\n  {{ csrf_field() }}\n  …\n</form>",
        ),
        layer(
            "FN",
            "handler",
            "Your handler",
            "A plain async fn. Extractors validate the input before it runs; ? turns any error into the right response.",
            "rust",
            "async fn store(State(state): State<AppState>,\n               Valid(form): Valid<NewOrder>) -> Result<Redirect> {\n    let order = form.into_order().insert(&state.db).await?;\n    Redirect::route(\"orders.show\", &[&order.id])\n}",
        ),
        layer(
            "VIEW",
            "view",
            "Views",
            "Pages written as rx-* tags that compile to MiniJinja when they load, live components, htmx fragments and out-of-band swaps, error pages in your layout.",
            "html",
            "<rx-card :title=\"'Order #' ~ order.id\">\n  <rx-table :rows=\"order.lines\" as=\"line\">\n    <rx-column label=\"Item\">{{ line.name }}</rx-column>\n  </rx-table>\n</rx-card>",
        ),
        layer(
            "200",
            "response",
            "The response",
            "A full page, an htmx fragment, JSON, a download or a stream, with a request id, and an ETag when you ask for one.",
            "",
            "HTTP/1.1 200 OK\nx-request-id: W5-L3oxyBUY2tvYT\ncontent-type: text/html; charset=utf-8",
        ),
    ]
}

/// One Laravel ↔ Renox comparison.
#[derive(Serialize)]
pub struct Pair {
    pub name: &'static str,
    pub laravel_file: &'static str,
    pub laravel: String,
    pub renox_file: &'static str,
    pub renox: String,
}

pub fn pairs() -> Vec<Pair> {
    let pair = |name, laravel_file, laravel: &str, renox_file, renox: &str| Pair {
        name,
        laravel_file,
        laravel: renox_site::highlight::highlight("php", laravel),
        renox_file,
        renox: renox_site::highlight::highlight("rust", renox),
    };
    vec![
        pair(
            "Routes",
            "routes/web.php",
            "Route::get('/orders', [OrderController::class, 'index'])\n    ->name('orders.index')\n    ->middleware('auth');\n\nRoute::post('/orders', [OrderController::class, 'store'])\n    ->name('orders.store')\n    ->middleware(['auth', 'throttle:10,1']);",
            "src/app/orders/mod.rs",
            "Routes::new()\n    .get(\"/orders\", index).name(\"orders.index\")\n    .post(\"/orders\", store).name(\"orders.store\")\n    .throttle(10, Duration::from_secs(60))\n    .require_auth()",
        ),
        pair(
            "Models",
            "app/Models/Order.php",
            "class Order extends Model\n{\n    use SoftDeletes;\n\n    protected $fillable = ['customer_id', 'total'];\n\n    public function customer()\n    {\n        return $this->belongsTo(Customer::class);\n    }\n}",
            "src/app/orders/model.rs",
            "#[derive(Model, Serialize, Default)]\n#[model(soft_deletes)]\npub struct Order {\n    pub id: i64,\n    pub customer_id: i64,\n    pub total: i64,\n    pub deleted_at: Option<DateTime>,\n}\n\n// The customers of a page of orders, in one query:\nlet customers = belongs_to::<Customer, _, _>(\n    &db, &orders, |o| o.customer_id,\n).await?;",
        ),
        pair(
            "Validation",
            "app/Http/Requests/StoreOrder.php",
            "class StoreOrder extends FormRequest\n{\n    public function rules(): array\n    {\n        return [\n            'email' => 'required|email',\n            'quantity' => 'required|integer|min:1',\n        ];\n    }\n}",
            "src/app/orders/form.rs",
            "#[derive(Deserialize, Validate)]\npub struct StoreOrder {\n    #[validate(required, email)]\n    pub email: String,\n    #[validate(required, min = 1)]\n    pub quantity: i64,\n}\n\nasync fn store(Valid(form): Valid<StoreOrder>) -> Result<Redirect> { … }",
        ),
        pair(
            "Jobs",
            "app/Jobs/SendInvoice.php",
            "class SendInvoice implements ShouldQueue\n{\n    public $tries = 5;\n\n    public function __construct(public int $orderId) {}\n\n    public function handle(): void\n    {\n        Mail::to($this->order->email)->send(new Invoice($this->order));\n    }\n}\n\nSendInvoice::dispatch($order->id);",
            "src/app/orders/jobs.rs",
            "#[derive(Serialize, Deserialize)]\npub struct SendInvoice { pub order_id: i64 }\n\nimpl Job for SendInvoice {\n    const NAME: &'static str = \"send-invoice\";\n    const MAX_ATTEMPTS: u32 = 5;\n\n    async fn handle(self, ctx: JobContext) -> Result {\n        let order = Order::find_or_404(&ctx.state.db, self.order_id).await?;\n        ctx.state.mailer.send(invoice(&order)).await\n    }\n}\n\nstate.queue.dispatch(SendInvoice { order_id: order.id }).await?;",
        ),
    ]
}

/// One panel of a code duo: a file name and its highlighted code.
#[derive(Serialize)]
pub struct Panel {
    pub file: &'static str,
    pub code: String,
}

/// Two panels side by side, with the sentence that ties them.
#[derive(Serialize)]
pub struct Duo {
    pub name: &'static str,
    pub text: &'static str,
    pub left: Panel,
    pub right: Panel,
}

/// The "Pages" section: pages as tags, and a live component.
pub fn duos() -> Vec<Duo> {
    let panel = |file, lang: &str, code: &str| Panel {
        file,
        code: renox_site::highlight::highlight(lang, code),
    };
    vec![
        Duo {
            name: "Pages are HTML tags",
            text: "A page is a template of rx-* tags: the UI kit, forms, tables, navigation and layouts. They are compiled to MiniJinja when the template loads, so there is no build step and live reload keeps working. A wrong tag or attribute fails with the file, the line and a suggestion, and rnx view:check finds them all before a page is opened.",
            left: panel(
                "resources/views/orders/index.html",
                "html",
                "<rx-page layout=\"layouts/app.html\" title=\"Orders\">\n  <rx-page-header title=\"Orders\">\n    <rx-link-button route=\"orders.create\" label=\"New order\" variant=\"primary\"/>\n  </rx-page-header>\n  <rx-table :rows=\"orders\" caption=\"Orders\">\n    <rx-column label=\"Number\">{{ row.number }}</rx-column>\n    <rx-column label=\"Total\" align=\"num\">{{ row.total | money }}</rx-column>\n    <rx-slot name=\"empty\"><rx-empty title=\"No orders yet\"/></rx-slot>\n  </rx-table>\n</rx-page>",
            ),
            right: panel(
                "rnx view:check",
                "",
                "orders/index.html:3: <rx-link-button> has no attribute\n  \"varient\"; did you mean \"variant\"? It takes: href, label,\n  variant, size, icon, badge, key, new-tab\nChecked 41 templates: 1 problems.",
            ),
        },
        Duo {
            name: "Live components",
            text: "A part of a page whose state lives in a Rust struct, Renox's answer to Livewire. A button calls a method, the server changes the state and renders the component again, and the browser morphs the page into it. One struct and one template: no route, no fragment, no JavaScript. The state travels signed.",
            left: panel(
                "src/app/counter.rs",
                "rust",
                "#[derive(Serialize, Deserialize, Default)]\npub struct Counter {\n    count: i64,\n}\n\n#[renox::live_component(view = \"live/counter.html\")]\nimpl Counter {\n    #[live(action)]\n    async fn add(&mut self, _ctx: &mut LiveContext, n: i64) -> Result {\n        self.count += n;\n        Ok(())\n    }\n}",
            ),
            right: panel(
                "resources/views/live/counter.html",
                "html",
                "<p>{{ state.count }}</p>\n<button type=\"button\" rx-click=\"add(1)\">+1</button>\n<button type=\"button\" rx-click=\"add(10)\">+10</button>\n\n{# on the page: <live-counter :component=\"counter\"/> #}",
            ),
        },
    ]
}

/// A row of the comparison table: Renox, Laravel, Loco, Axum + crates.
#[derive(Serialize)]
pub struct Compare {
    pub what: &'static str,
    pub cells: [&'static str; 4],
}

pub fn comparison() -> Vec<Compare> {
    vec![
        Compare {
            what: "Language",
            cells: ["Rust", "PHP", "Rust", "Rust"],
        },
        Compare {
            what: "Checked at compile time",
            cells: ["yes", "no", "yes", "yes"],
        },
        Compare {
            what: "Auth, roles, two-factor, OAuth",
            cells: ["yes", "with packages", "auth only", "no"],
        },
        Compare {
            what: "Queue and scheduler without Redis",
            cells: ["yes", "yes", "yes", "no"],
        },
        Compare {
            what: "UI kit and data grid",
            cells: ["yes", "with Filament", "no", "no"],
        },
        Compare {
            what: "Admin panel, billing, OAuth plugins",
            cells: ["yes", "with packages", "no", "no"],
        },
        Compare {
            what: "Live components, no JavaScript to write",
            cells: ["yes", "with Livewire", "no", "no"],
        },
        Compare {
            what: "Deploys as one file",
            cells: ["yes", "no", "yes", "yes"],
        },
        Compare {
            what: "Wiring you write yourself",
            cells: ["none", "none", "some", "all of it"],
        },
    ]
}

/// An example app in the showcase.
#[derive(Serialize)]
pub struct Example {
    pub name: &'static str,
    pub text: &'static str,
}

pub fn examples() -> Vec<Example> {
    vec![
        Example {
            name: "bikeshop",
            text: "Three stores that sell, rent and service bikes",
        },
        Example {
            name: "hello",
            text: "The smallest app: a guestbook",
        },
    ]
}

/// The benchmark results shown on the page: `content/benchmarks.json`,
/// written from a run of `benchmarks/run.sh` (`null` until there is one).
pub fn benchmarks() -> Option<serde_json::Value> {
    let value: serde_json::Value =
        serde_json::from_str(include_str!("../content/benchmarks.json")).ok()?;
    (!value.is_null()).then_some(value)
}
