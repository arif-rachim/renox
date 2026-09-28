//! A blog showing every kind of relation without N+1 queries:
//! a post belongs to a category, has many comments, and has many tags
//! through a pivot table that has columns of its own (pinned, timestamps).
//! Pages load the related rows of a whole page in one query per relation
//! (counts included); reports use the query builder's `group_by` and SQL
//! joins read into structs.

pub mod model;

use std::collections::HashMap;

use renox::db::relations::{belongs_to, count_many, has_many};
use renox::prelude::*;
use serde::{Deserialize, Serialize};

use model::{Category, Comment, POST_TAGS, Post, Tag, Tagging};

pub struct Blog;

impl Module for Blog {
    fn name(&self) -> &'static str {
        "blog"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/", index)
            .name("posts.index")
            .get("/posts/{id}", show)
            .name("posts.show")
            .post("/posts/{id}/comments", comment)
            .name("posts.comment")
            .put("/posts/{id}/tags", retag)
            .name("posts.tags")
            .put("/posts/{id}/tags/{tag}/pin", pin)
            .name("posts.pin")
            .get("/categories/{id}", category)
            .name("categories.show")
            .get("/tags/{id}", tag)
            .name("tags.show")
            .get("/report", report)
            .name("report")
    }
}

/// A post with everything its card shows.
#[derive(Serialize)]
struct Card {
    #[serde(flatten)]
    post: Post,
    category: Option<Category>,
    tags: Vec<Tag>,
    comments: i64,
    latest_comment: Option<Comment>,
}

/// 10 posts with their categories, comment counts, latest comments and tags
/// in 7 queries in all (count and page, categories, comment counts, latest
/// comments, tag links and tags), whatever the page size.
async fn index(State(db): State<Db>, Page(page): Page) -> Result<View> {
    let posts = Post::query().latest().paginate(&db, page, 10).await?;
    let categories = belongs_to::<Category, _, _>(&db, &posts.items, |p| p.category_id).await?;
    // `withCount`: one GROUP BY query; posts without comments get 0.
    let counts = count_many(&db, &posts.items, Comment::query(), "post_id").await?;
    // Only the latest comment of each post: `has_many` with a filter that
    // keeps one row per post, so a post with 500 comments loads one.
    let mut latest = has_many(
        &db,
        &posts.items,
        Comment::query().where_raw(
            "id = (SELECT MAX(latest.id) FROM comments latest \
             WHERE latest.post_id = comments.post_id)",
            std::iter::empty::<i64>(),
        ),
        "post_id",
        |c| c.post_id,
    )
    .await?;
    let mut tags = POST_TAGS.load_for::<Tag, _>(&db, &posts.items).await?;
    let posts = posts.map(|post| Card {
        category: post.category_id.and_then(|id| categories.get(&id).cloned()),
        tags: tags.remove(&post.id).unwrap_or_default(),
        comments: counts.get(&post.id).copied().unwrap_or(0),
        latest_comment: latest.remove(&post.id).and_then(|mut c| c.pop()),
        post,
    });
    Ok(view("blog/index.html", context! { posts }))
}

/// A tag of the post with the pivot's columns, for the template.
#[derive(Serialize)]
struct TagLink {
    #[serde(flatten)]
    tag: Tag,
    pivot: Tagging,
}

/// One post: the relation methods on `Post`, one query each (the tags with
/// their pivot columns: one for the links, one for the tags).
async fn show(State(db): State<Db>, Path(id): Path<i64>) -> Result<View> {
    let post = Post::find_or_404(&db, id).await?;
    let category = post.category(&db).await?;
    let comments = post.comments(&db).await?;
    let tags: Vec<TagLink> = post
        .taggings(&db)
        .await?
        .into_iter()
        .map(|(tag, pivot)| TagLink { tag, pivot })
        .collect();
    let tag_ids: Vec<i64> = tags.iter().map(|t| t.tag.id).collect();
    let all_tags = Tag::query().order_by("name").get(&db).await?;
    Ok(view(
        "blog/show.html",
        context! { post, category, comments, tags, tag_ids, all_tags },
    ))
}

#[derive(Deserialize, Serialize)]
struct CommentForm {
    author: String,
    body: String,
}

impl Validate for CommentForm {
    fn rules(&self, v: &mut Validator) {
        v.field("author", &self.author).required().max(50);
        v.field("body", &self.body).required().max(1000);
    }
}

async fn comment(
    State(db): State<Db>,
    Path(id): Path<i64>,
    Valid(form): Valid<CommentForm>,
) -> Result<Redirect> {
    let post = Post::find_or_404(&db, id).await?;
    Comment::create(
        &db,
        Comment {
            post_id: post.id,
            author: form.author,
            body: form.body,
            ..Default::default()
        },
    )
    .await?;
    Ok(Redirect::to(&format!("/posts/{}#comments", post.id)))
}

/// The checked boxes of the tag form (none checked sends nothing).
#[derive(Deserialize)]
struct TagsForm {
    #[serde(default)]
    tags: Vec<i64>,
}

impl Validate for TagsForm {
    fn rules(&self, v: &mut Validator) {
        v.each("tags", &self.tags, |tag| tag.exists("tags", "id"));
    }
}

/// Makes the post's tags exactly the checked ones, in one transaction.
async fn retag(
    State(db): State<Db>,
    session: Session,
    Path(id): Path<i64>,
    Valid(form): Valid<TagsForm>,
) -> Result<Redirect> {
    let post = Post::find_or_404(&db, id).await?;
    POST_TAGS.sync(&db, post.id, form.tags).await?;
    session.flash("status", "Tags saved.")?;
    Ok(Redirect::to(&format!("/posts/{}", post.id)))
}

#[derive(Deserialize)]
struct PinForm {
    pinned: bool,
}

impl Validate for PinForm {
    fn rules(&self, _v: &mut Validator) {}
}

/// Changes a column of the pivot row (and its `updated_at`):
/// `update_pivot` returns false when the post doesn't have the tag.
async fn pin(
    State(db): State<Db>,
    session: Session,
    Path((id, tag)): Path<(i64, i64)>,
    Valid(form): Valid<PinForm>,
) -> Result<Redirect> {
    let post = Post::find_or_404(&db, id).await?;
    if !POST_TAGS
        .update_pivot(&db, post.id, tag, &[("pinned", &form.pinned)])
        .await?
    {
        return Err(Error::NotFound);
    }
    let status = if form.pinned { "Pinned." } else { "Unpinned." };
    session.flash("status", status)?;
    Ok(Redirect::to(&format!("/posts/{}", post.id)))
}

/// "Has many" from the other side: the category's posts.
async fn category(State(db): State<Db>, Path(id): Path<i64>) -> Result<View> {
    let category = Category::find_or_404(&db, id).await?;
    let posts = category.posts(&db).await?;
    let pinned: Vec<i64> = Vec::new();
    Ok(view(
        "blog/list.html",
        context! { heading => category.name, posts, pinned },
    ))
}

/// Many to many, the other way round: `inverse()` goes from tags to posts.
/// `load_with_pivot` reads the pivot's columns too: pinned posts first,
/// then the most recently tagged.
async fn tag(State(db): State<Db>, Path(id): Path<i64>) -> Result<View> {
    let tag = Tag::find_or_404(&db, id).await?;
    let mut by_tag: HashMap<i64, Vec<(Post, Tagging)>> =
        POST_TAGS.inverse().load_with_pivot(&db, [tag.id]).await?;
    let mut links = by_tag.remove(&tag.id).unwrap_or_default();
    links.sort_by(|(a, x), (b, y)| {
        (y.pinned, y.created_at, b.id).cmp(&(x.pinned, x.created_at, a.id))
    });
    let pinned: Vec<i64> = links
        .iter()
        .filter(|(_, t)| t.pinned)
        .map(|(p, _)| p.id)
        .collect();
    let posts: Vec<Post> = links.into_iter().map(|(post, _)| post).collect();
    Ok(view(
        "blog/list.html",
        context! { heading => format!("#{}", tag.name), posts, pinned },
    ))
}

/// A row of the report: columns by name.
#[derive(FromRow, Serialize)]
struct CategoryStats {
    category: String,
    posts: i64,
    comments: i64,
}

/// A row of the "most active commenters" table.
#[derive(FromRow, Serialize)]
struct Commenter {
    author: String,
    comments: i64,
}

/// Joins are clearest as SQL, read into structs or tuples; an aggregate over
/// one table reads well from the query builder (`group_by` + `select_as`).
async fn report(State(db): State<Db>) -> Result<View> {
    let categories: Vec<CategoryStats> = renox::db::sql(
        "SELECT COALESCE(c.name, 'Uncategorized') AS category, \
                COUNT(DISTINCT p.id) AS posts, COUNT(m.id) AS comments \
         FROM posts p \
         LEFT JOIN categories c ON c.id = p.category_id \
         LEFT JOIN comments m ON m.post_id = p.id \
         GROUP BY c.name ORDER BY posts DESC, category",
    )
    .fetch_as(&db)
    .await?;
    let tags: Vec<(String, i64)> = renox::db::sql(
        "SELECT t.name, COUNT(pt.post_id) FROM tags t \
         LEFT JOIN post_tags pt ON pt.tag_id = t.id \
         GROUP BY t.id, t.name ORDER BY 2 DESC, t.name",
    )
    .fetch_as(&db)
    .await?;
    // One table, grouped: `group_by` checks the column name, `select_as`
    // reads the aggregate into a `#[derive(FromRow)]` struct.
    let commenters: Vec<Commenter> = Comment::query()
        .group_by("author")
        .order_by_raw("comments DESC, author")
        .limit(5)
        .select_as(&db, "author, COUNT(*) AS comments")
        .await?;
    // `whereHas` without writing SQL: posts with at least one comment
    // (EXISTS); `where_doesnt_have` is the opposite.
    let discussed = Post::query()
        .where_has(Comment::query(), "post_id")
        .count(&db)
        .await?;
    Ok(view(
        "blog/report.html",
        context! { categories, tags, commenters, discussed },
    ))
}
