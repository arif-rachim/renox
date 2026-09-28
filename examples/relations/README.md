# examples/relations

Relations the Renox way, on a small blog: a post belongs to a category, has many comments, and
has many tags through a pivot table. There are no lazy, hidden queries: one row gets a method per
relation, a page of rows gets one query per relation, and reports are SQL. Read it with
[docs/relations.md](../../docs/relations.md).

```bash
cd examples/relations
cargo run -- migrate
cargo run -- db:seed             # 3 categories, 5 tags, 24 posts with comments
cargo run                        # http://127.0.0.1:3000
```

## What's where

| Relation | How | Where |
|---|---|---|
| Schema: foreign keys, `ON DELETE CASCADE` / `SET NULL`, a pivot table with a unique index | SQL | [migrations/](migrations) |
| One post's category, comments and tags | methods on `Post` (`post.comments(&db)`), one query each | [model.rs](src/app/blog/model.rs) |
| A page of posts with their categories, comments and tags | `belongs_to`, `has_many`, `POST_TAGS.load_for`: one query per relation, whatever the page size | `index` in [mod.rs](src/app/blog/mod.rs) |
| Changing a post's tags from checkboxes | `POST_TAGS.sync` (in a transaction); `v.each(…).exists(…)` checks every id | `retag` |
| A tag's posts (many to many, the other way) | `POST_TAGS.inverse().load` | `tag` |
| A category's posts (has many, from the parent) | `Post::where_eq("category_id", …)` in `category.posts(&db)` | `category` |
| Report: posts and comments per category, posts per tag | SQL joins read with `fetch_as` into a `#[derive(FromRow)]` struct and a tuple | `report` |
| Posts that have comments | `where_in_query("id", Comment::query(), "post_id")`, a sub-query without SQL | `report` |

## Things worth copying

- **Foreign keys are plain fields.** `category_id: Option<i64>` is what the table has; a method
  or a loader turns it into a `Category` when a page needs it.
- **Loaders return maps.** `belongs_to` gives `HashMap<id, Category>`, `has_many` and
  `load_for` give `HashMap<id, Vec<…>>`; build what the template needs from them (see `Card`).
- **The database keeps relations tidy.** Deleting a post deletes its comments and tag links
  (`ON DELETE CASCADE`); deleting a category keeps its posts, without a category (`SET NULL`).
- **Sums and counts are SQL.** A join with `GROUP BY`, read into a struct, is shorter and faster
  than loading rows to count them.

## Tests

```bash
cargo test -p relations
```

[tests/blog.rs](tests/blog.rs) checks each card's relations, comments, tag syncing (including an
unknown tag being refused), both directions of many to many, the report's numbers and what
deletes take with them.
