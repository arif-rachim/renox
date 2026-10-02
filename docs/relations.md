# Relations and queries beyond one table

Renox has no Eloquent-style lazy relations: `product.category` doesn't quietly run a query. Rust
has no reflection to build them from, and hidden queries are where N+1 problems come from.
Instead, Renox gives you:
- a foreign key column in the struct;
- a method when you need one related row;
- loaders that fetch the related rows of a whole page in one query;
- plain SQL, read into structs, when a join is the clearest way.

The examples below use this schema:

```sql
CREATE TABLE categories (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL);
CREATE TABLE products (id INTEGER PRIMARY KEY AUTOINCREMENT, category_id INTEGER REFERENCES categories (id),
                       name TEXT NOT NULL, price INTEGER NOT NULL);
CREATE TABLE reviews (id INTEGER PRIMARY KEY AUTOINCREMENT, product_id INTEGER NOT NULL REFERENCES products (id),
                      stars INTEGER NOT NULL);
CREATE TABLE tags (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL);
CREATE TABLE product_tags (product_id INTEGER NOT NULL, tag_id INTEGER NOT NULL, UNIQUE (product_id, tag_id));
```

## One related row, or the rows of one model

Write a method. It is one line, it is typed, and the query it runs is visible where it's called:

```rust
use renox::prelude::*;

#[derive(Model, serde::Serialize, Default, Clone)]
#[model(table = "categories")]
pub struct Category { pub id: i64, pub name: String }

#[derive(Model, serde::Serialize, Default, Clone)]
#[model(table = "products")]
pub struct Product { pub id: i64, pub category_id: Option<i64>, pub name: String, pub price: i64 }

#[derive(Model, serde::Serialize, Default, Clone)]
#[model(table = "reviews")]
pub struct Review { pub id: i64, pub product_id: i64, pub stars: i64 }

impl Product {
    /// belongs to
    pub async fn category(&self, db: &Db) -> Result<Option<Category>> {
        match self.category_id {
            Some(id) => Category::find(db, id).await,
            None => Ok(None),
        }
    }

    /// has many
    pub async fn reviews(&self, db: &Db) -> Result<Vec<Review>> {
        Review::where_eq("product_id", self.id).order_by_desc("stars").get(db).await
    }
}
```

## A page of rows with their relations (no N+1)

Calling `product.category(&db)` in a loop over 20 products runs 21 queries. The loaders in
`renox::db::relations` fetch the related rows of the whole page in one query each, then you
look them up by id:

```rust
use renox::prelude::*;
use renox::db::relations::{Pivot, belongs_to, has_many};
# #[derive(Model, serde::Serialize, Default, Clone)] #[model(table = "categories")] pub struct Category { pub id: i64, pub name: String }
# #[derive(Model, serde::Serialize, Default, Clone)] #[model(table = "products")] pub struct Product { pub id: i64, pub category_id: Option<i64>, pub name: String, pub price: i64 }
# #[derive(Model, serde::Serialize, Default, Clone)] #[model(table = "reviews")] pub struct Review { pub id: i64, pub product_id: i64, pub stars: i64 }
# #[derive(Model, serde::Serialize, Default, Clone)] #[model(table = "tags")] pub struct Tag { pub id: i64, pub name: String }

/// many to many, through a pivot table
pub const PRODUCT_TAGS: Pivot = Pivot::new("product_tags", "product_id", "tag_id");

/// What the template gets for each product.
#[derive(serde::Serialize)]
struct ProductCard {
    #[serde(flatten)]
    product: Product,
    category: Option<Category>,
    reviews: Vec<Review>,
    tags: Vec<Tag>,
}

async fn index(State(db): State<Db>, Page(page): Page) -> Result<View> {
    let products = Product::query().order_by("name").paginate(&db, page, 20).await?; // 2 queries
    let mut categories = belongs_to::<Category, _, _>(&db, &products.items, |p| p.category_id).await?; // 1
    let mut reviews = has_many(&db, &products.items, Review::query().order_by_desc("stars"),
                               "product_id", |r| r.product_id).await?; // 1
    let mut tags = PRODUCT_TAGS.load_for::<Tag, _>(&db, &products.items).await?; // 2
    let cards = products.map(|product| ProductCard {
        category: product.category_id.and_then(|id| categories.remove(&id)),
        reviews: reviews.remove(&product.id).unwrap_or_default(),
        tags: tags.remove(&product.id).unwrap_or_default(),
        product,
    });
    Ok(view("products/index.html", context! { products => cards }))
}
```

In the template, `{{ card.category.name }}` and `{% for tag in card.tags %}` read plain data.
Each loader takes the page's rows, so it works the same for `get()`, `paginate()` and `chunk()`.

- `belongs_to::<Parent, _, _>(db, &children, |child| child.parent_id)` returns a
  `HashMap<parent id, Parent>`. The closure returns the parent's key or an `Option` of it:
  `i64`, or a `Ulid`, `Uuid` or `String` for parents keyed that way (`.clone()` those).
- `has_many(db, &parents, Child::query()…, "parent_id", |child| child.parent_id)` returns a
  `HashMap<parent id, Vec<Child>>`. The query sets the children's order and filters.
- `Pivot::load_for::<Target, _>(db, &parents)` / `load(db, ids)` returns a
  `HashMap<parent id, Vec<Target>>`. `Pivot::inverse()` gives the other direction.
- `Model::find_many(db, ids)` returns the rows with these ids.

The maps are keyed by the parent's key type (`Model::Key`). A pivot between models with other
keys names them, left then right: `Pivot<Ulid, i64>`; `Pivot` alone is `Pivot<i64, i64>`.
A polymorphic relation's id column holds the parents' key, so every parent type of one `Morph`
needs the same key type.

## Changing a many-to-many

```rust
# use renox::prelude::*;
# use renox::db::relations::Pivot;
# const PRODUCT_TAGS: Pivot = Pivot::new("product_tags", "product_id", "tag_id");
# async fn demo(db: Db, product_id: i64, checked: Vec<i64>) -> Result {
PRODUCT_TAGS.attach(&db, product_id, [1, 2]).await?; // adds links that aren't there yet; returns how many
PRODUCT_TAGS.detach(&db, product_id, [2]).await?; // returns how many links were removed
PRODUCT_TAGS.sync(&db, product_id, checked).await?;  // exactly these (a form's checkboxes), in a transaction
let tag_ids = PRODUCT_TAGS.ids(&db, product_id).await?;
# let _ = tag_ids; Ok(()) }
```

## Pivot columns

A pivot table may carry more than the two ids: a member's role, a quantity, when it was added.

```rust
# use renox::prelude::*;
use renox::db::relations::Pivot;

// team_user (team_id, user_id, role TEXT NOT NULL, created_at, updated_at)
const MEMBERS: Pivot = Pivot::new("team_user", "team_id", "user_id").with_timestamps();

#[derive(FromRow)]
struct Membership { role: String, created_at: Option<DateTime> }

# async fn demo(db: Db, team_id: i64) -> Result {
MEMBERS.attach_with(&db, team_id, 7, &[("role", &"admin")]).await?; // false if already linked
MEMBERS.update_pivot(&db, team_id, 7, &[("role", &"member")]).await?; // false if not linked
let (added, removed) = MEMBERS.toggle(&db, team_id, [7, 8]).await?; // flips each link
let members = MEMBERS.load_with_pivot::<User, Membership>(&db, [team_id]).await?;
for (user, membership) in members.get(&team_id).map_or(&[][..], Vec::as_slice) {
    println!("{}: {}", user.name, membership.role);
}
# let _ = (added, removed); Ok(()) }
```

`with_timestamps()` fills `created_at`/`updated_at` on `attach`, `attach_with`, `sync` and
`toggle`, and `updated_at` on `update_pivot`.

## Polymorphic relations

A child that belongs to one of several tables (comments on posts and on videos) keeps the
parent's table name and id in two columns. Renox writes `P::TABLE` (`"posts"`) as the type.

```rust
# use renox::prelude::*;
use renox::db::relations::Morph;

# #[derive(Model, serde::Serialize, Default, Clone)] #[model(table = "posts")] struct Post { id: i64, title: String }
# #[derive(Model, serde::Serialize, Default, Clone)] #[model(table = "videos")] struct Video { id: i64, title: String }
#[derive(Model, serde::Serialize, Default, Clone)]
#[model(table = "comments")]
struct Comment { id: i64, commentable_type: String, commentable_id: i64, body: String }

const COMMENTABLE: Morph = Morph::new("commentable_type", "commentable_id");

# async fn demo(db: Db, post: Post) -> Result {
let comment = Comment {
    commentable_type: Post::TABLE.into(),
    commentable_id: post.id,
    body: "Nice".into(),
    ..Default::default()
};
Comment::create(&db, comment).await?;

let count = COMMENTABLE.of(&post, Comment::query()).count(&db).await?; // this post's comments
let posts = Post::query().latest().limit(20).get(&db).await?;
let comments = COMMENTABLE.load_many(&db, &posts, Comment::query(), |c| c.commentable_id).await?;

// The other way (morphTo): one query per parent type.
let recent = Comment::query().latest().limit(50).get(&db).await?;
let parent = |c: &Comment| (c.commentable_type.clone(), c.commentable_id);
let on_posts = COMMENTABLE.parents::<Post, _>(&db, &recent, parent).await?;
let on_videos = COMMENTABLE.parents::<Video, _>(&db, &recent, parent).await?;
# let _ = (count, comments, on_posts, on_videos); Ok(()) }
```

The database can't enforce a foreign key here, so delete the children yourself, e.g. in the
parent's `deleting` hook (see `ModelHooks` in the cheatsheet). Index the two columns together.

## Joins and reports: SQL read into structs

A join or an aggregate is clearest as SQL. `fetch_as` reads the rows into:
- a `#[derive(FromRow)]` struct (columns by name; `#[row(rename = "…")]`, `#[row(skip)]`);
- a model;
- a tuple (columns by position).

```rust
use renox::prelude::*;

#[derive(FromRow, serde::Serialize)]
struct CategorySales {
    category: String,
    products: i64,
    revenue: i64,
}

# async fn demo(db: Db) -> Result {
let sales: Vec<CategorySales> = renox::db::sql(
    "SELECT c.name AS category, COUNT(p.id) AS products, CAST(SUM(p.price) AS BIGINT) AS revenue \
     FROM categories c JOIN products p ON p.category_id = c.id \
     GROUP BY c.name ORDER BY revenue DESC",
)
.fetch_as(&db)
.await?;
let names: Vec<(i64, String)> = renox::db::sql("SELECT id, name FROM products").fetch_as(&db).await?;
# let _ = (sales, names); Ok(()) }
```

On PostgreSQL, `SUM` and `COUNT` of `BIGINT` come back as `NUMERIC` and `BIGINT`. Cast sums with
`CAST(… AS BIGINT)`, as above, so the same struct reads on both databases.

## Filtering by a related table without a join

```rust
# use renox::prelude::*;
# #[derive(Model, serde::Serialize, Default)] #[model(table = "categories")] struct Category { id: i64, name: String, active: bool }
# #[derive(Model, serde::Serialize, Default)] #[model(table = "products")] struct Product { id: i64, category_id: Option<i64>, name: String, price: i64 }
# async fn demo(db: Db, q: String) -> Result {
let products = Product::query()
    .where_in_query("category_id", Category::where_eq("active", true), "id") // IN (SELECT id FROM categories …)
    .when(!q.is_empty(), |query| {
        query.where_any(|any| any.where_like("name", format!("%{q}%")).where_op("price", "<", 10_000))
    })
    .get(&db)
    .await?;
# let _ = products; Ok(()) }
```

## More of the query builder

| Laravel | Renox |
|---|---|
| `where(fn …)` / `orWhere` | `.where_any(\|q\| …)` (OR), `.where_all(\|q\| …)` (AND), nestable |
| `whereBetween`, `whereNotIn` | `.where_between(col, low, high)`, `.where_not_in(col, list)` |
| `when($cond, fn …)` | `.when(cond, \|q\| …)` |
| `sum`, `avg`, `min`, `max` | `.sum::<i64, _>(&db, col)`, `.avg(&db, col)`, `.min::<T, _>(…)`, `.max::<T, _>(…)` |
| `pluck` | `.pluck::<T, _>(&db, col)` |
| `update([...])` | `.update(&db, &[("status", &"paid")])`, which sets `updated_at` too |
| `increment` / `decrement` | `.increment(&db, "stock", -1)` |
| `firstOrFail`, `firstOrCreate` | `.first_or_404(&db)`, `.first_or_create(&db, \|\| new)` |
| `chunk` | `.chunk(&db, 1000, \|rows\| async { … })` (in id order; the query's own order and limit are ignored) |
| `insert([...])`, `upsert` | `Model::insert_many(&db, rows)`, `Model::upsert(&db, rows, &["sku"], &["qty"])` |
| `with('category')` | `relations::belongs_to` / `has_many` / `Pivot::load_for` (above) |
| pivot `withPivot`, `withTimestamps`, `toggle`, `updateExistingPivot` | `Pivot::with_timestamps()`, `attach_with`, `load_with_pivot::<T, Row>`, `toggle`, `update_pivot` |
| `morphMany` (eager, for a page of parents) | `Morph::load_many` |
| `$post->comments()` (one parent's children, as a query) | `COMMENTABLE.of(&post, Comment::query())` (`Morph::of`) |
| `morphTo` | `Morph::parents::<P, _>` (one query per parent type) |
| `withCount` on a morph relation | `Morph::count_many` |
| model events / observers | `#[model(hooks)]` + `impl ModelHooks` (`saving`, `saved`, `deleting`, `deleted`) |
| `save()` of dirty columns, `update([...])` on a model | `model.save_changes(&db, &original)`, `model.save_only(&db, &["price"])` |
| `withCount`, `withSum` | `relations::count_many(&db, &posts, Comment::query(), "post_id")`, `sum_many::<i64, _, _>(…, "total")` (0 for rows without children) |
| `whereHas`, `whereDoesntHave` | `.where_has(Comment::where_eq("approved", true), "post_id")`, `.where_doesnt_have(…)` (EXISTS) |
| `whereNotIn(fn …)` (sub-query) | `.where_not_in_query(col, Other::query(), "col")` |
| `whereRaw`, `orderByRaw` | `.where_raw("DATE(created_at) = DATE(?)", [value])`, `.order_by_raw("total DESC")` |
| `groupBy`, `having`, `selectRaw` | `.group_by(col).having_raw("COUNT(*) > ?", [2]).select_as::<(i64, i64), _>(&db, "col, COUNT(*)")` |
| `lockForUpdate`, `sharedLock` | `.lock_for_update()` / `.shared_lock()` on `&mut tx` (PostgreSQL; SQLite: `db.begin_immediate()`) |
| `firstOrNew`, `updateOrCreate`, `refresh` | `.first_or_new(&db, \|\| new)`, `.update_or_create(&db, \|\| new, \|m\| …)`, `model.refresh(&db)` |
| `HasUlids`, `HasUuids`, string keys | `id: Ulid` / `id: Uuid` / `id: String` (made on insert, or set by you); `Pivot<Ulid, i64>` |
| `insert()` of one model with its key | `model.insert(&db)` (always an INSERT; `Model::insert_many` for many rows) |
| `encrypted` cast | `Encrypted<T>` fields |
| `DB::transaction` inside a transaction | `tx.savepoint(\|tx\| Box::pin(async move { … }))` |
| `simplePaginate`, `cursorPaginate` | `.simple_paginate(&db, page, per)` (`simple_pagination` macro), `.cursor_paginate(&db, cursor, per)` (newest id first; the query's own order is ignored) |
| `DB::transaction(fn, 3)` | `db.retrying(3, \|\| async { let mut tx = db.begin().await?; … })` (borrows from the caller), `db.transaction_retrying(…)`, `db.transaction(…)` |
| `toSql` | `.to_sql(db.dialect())` |
