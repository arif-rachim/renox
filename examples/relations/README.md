# examples/relations

Relations the Renox way, on a small blog: a post belongs to a category, has many comments, and
has many tags through a pivot table with columns of its own (pinned, timestamps), and posts
and comments both have likes (a polymorphic relation). There are no
lazy, hidden queries: one row gets a method per relation, a page of rows gets one query per
relation (counts included), and reports use the query builder or SQL joins. Read it with
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
| Schema: foreign keys, `ON DELETE CASCADE` / `SET NULL`, a pivot table with a unique index; pivot columns added later | SQL | [migrations/](migrations) |
| One post's category, comments and tags | methods on `Post` (`post.comments(&db)`, `post.taggings(&db)` with the pivot's columns) | [model.rs](src/app/blog/model.rs) |
| A page of posts with their categories, comment counts, latest comments and tags | `belongs_to`, `count_many` (`withCount`), `has_many` filtered to one row per post, `POST_TAGS.load_for`: one query per relation, whatever the page size | `index` in [mod.rs](src/app/blog/mod.rs) |
| Changing a post's tags from checkboxes | `POST_TAGS.sync` (in a transaction); `v.each(…).exists(…)` checks every id | `retag` |
| Pivot columns: pin a post on a tag's page, when it was tagged | `Pivot::with_timestamps()`, `attach_with` (seeder), `update_pivot`, `load_with_pivot::<Tag, Tagging>` | `pin`, `show`, [lib.rs](src/lib.rs) |
| A tag's posts (many to many, the other way), pinned first | `POST_TAGS.inverse().load_with_pivot` | `tag` |
| A category's posts (has many, from the parent) | `Post::where_eq("category_id", …)` in `category.posts(&db)` | `category` |
| Report: posts and comments per category, posts per tag | SQL joins read with `fetch_as` into a `#[derive(FromRow)]` struct and a tuple | `report` |
| Report: most active commenters | `Comment::query().group_by("author").select_as(…)` into a `#[derive(FromRow)]` struct | `report` |
| Likes on a post or a comment (polymorphic): one `likes` table with `likeable_type` / `likeable_id`, a `Like` button on the post and on each comment | `Like`, `LIKEABLE: Morph`, `Like::on` in [model.rs](src/app/blog/model.rs); `like_post`, `like_comment` |
| A post's like count; its comments' like counts; each card's like count | `LIKEABLE.of(&post, Like::query()).count(..)`; `like_counts` (`Morph::count_many`: `LIKEABLE.count_many(db, parents, Like::query())`, one query per page) | `show`, `index` |
| The latest likes with what was liked (`morphTo`) | `LIKEABLE.parents::<Post, _>` and `::<Comment, _>`: one query for the likes, one per parent type | `latest_likes`, `report` |
| Posts that have comments | `where_has(Comment::query(), "post_id")` (`whereHas`, an `EXISTS` without SQL) | `report` |

## Things worth copying

- **Foreign keys are plain fields.** `category_id: Option<i64>` is what the table has; a method
  or a loader turns it into a `Category` when a page needs it.
- **Loaders return maps.** `belongs_to` gives `HashMap<id, Category>`, `has_many` and
  `load_for` give `HashMap<id, Vec<…>>`; build what the template needs from them (see `Card`).
- **The database keeps relations tidy.** Deleting a post deletes its comments and tag links
  (`ON DELETE CASCADE`); deleting a category keeps its posts, without a category (`SET NULL`).
- **Count, don't load.** `count_many` (and `sum_many` for totals) gives each post its number in
  one `GROUP BY` query, with 0 for posts without comments; loading every comment to call `len()`
  gets slower as comments grow. For the latest comment only, `index` filters `has_many` so it
  loads one row per post.
- **Pivot tables can carry data.** `attach_with` / `update_pivot` write the extra columns,
  `load_with_pivot` reads them next to each model into a `#[derive(FromRow)]` struct
  (`Tagging`), and `with_timestamps()` keeps `created_at`/`updated_at` on every link.
- **Polymorphic relations: the type column holds the parent's table.** `Like::on(&post)` fills
  `likeable_type` with `Post::TABLE`; `Morph::of` narrows a query to one parent,
  `Morph::load_many` loads the children of a page (grouped by parent id), `Morph::parents` goes
  the other way, once per parent type. To count, `Morph::count_many` counts the children of many parents
  in one query rather than loading every like.
- **No foreign key, so triggers tidy up.** A foreign key can't point at two tables; the likes
  migration adds `AFTER DELETE` triggers on `posts` and `comments`, which also fire for comments
  removed by `ON DELETE CASCADE` (a model hook never sees those).
- **Reports: the builder for one table, SQL for joins.** `group_by` + `select_as` reads an
  aggregate of one table into a struct; a join with `GROUP BY` is clearest as SQL with
  `fetch_as`.

## Tests

```bash
cargo test -p relations
```

[tests/blog.rs](tests/blog.rs) checks each card's relations, comments, tag syncing (including an
unknown tag being refused), both directions of many to many, pivot columns (pinning, timestamps,
a tag the post doesn't have), the report's numbers, likes on posts and comments, and what
deletes take with them. [tests/queries.rs](tests/queries.rs) counts the statements with
`renox::db::capture_queries` to prove the likes of a page load in one query per type, however
many there are. While developing, `/_renox/debug` shows each request's SQL and flags a statement
run three or more times (a likely N+1).
