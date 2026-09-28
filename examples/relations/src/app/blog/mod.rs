//! A blog showing every kind of relation without N+1 queries:
//! a post belongs to a category, has many comments, and has many tags
//! through a pivot table. Pages load the related rows of a whole page in one
//! query per relation; reports use SQL joins read into structs.

pub mod model;

use std::collections::HashMap;

use renox::db::relations::{belongs_to, has_many};
use renox::prelude::*;
use serde::{Deserialize, Serialize};

use model::{Category, Comment, POST_TAGS, Post, Tag};

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
    comments: usize,
    latest_comment: Option<Comment>,
}

/// 10 posts with their categories, comments and tags in 6 queries in all
/// (count and page, categories, comments, tag links and tags), whatever the
/// page size.
async fn index(State(db): State<Db>, Page(page): Page) -> Result<View> {
    let posts = Post::query().latest().paginate(&db, page, 10).await?;
    let categories = belongs_to::<Category, _, _>(&db, &posts.items, |p| p.category_id).await?;
    let mut comments = has_many(
        &db,
        &posts.items,
        Comment::query().order_by_desc("id"),
        "post_id",
        |c| c.post_id,
    )
    .await?;
    let mut tags = POST_TAGS.load_for::<Tag, _>(&db, &posts.items).await?;
    let posts = posts.map(|post| {
        let comments = comments.remove(&post.id).unwrap_or_default();
        Card {
            category: post.category_id.and_then(|id| categories.get(&id).cloned()),
            tags: tags.remove(&post.id).unwrap_or_default(),
            comments: comments.len(),
            latest_comment: comments.into_iter().next(),
            post,
        }
    });
    Ok(view("blog/index.html", context! { posts }))
}

/// One post: the relation methods on `Post`, one query each.
async fn show(State(db): State<Db>, Path(id): Path<i64>) -> Result<View> {
    let post = Post::find_or_404(&db, id).await?;
    let category = post.category(&db).await?;
    let comments = post.comments(&db).await?;
    let tags = post.tags(&db).await?;
    let tag_ids: Vec<i64> = tags.iter().map(|t| t.id).collect();
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

/// "Has many" from the other side: the category's posts.
async fn category(State(db): State<Db>, Path(id): Path<i64>) -> Result<View> {
    let category = Category::find_or_404(&db, id).await?;
    let posts = category.posts(&db).await?;
    Ok(view(
        "blog/list.html",
        context! { heading => category.name, posts },
    ))
}

/// Many to many, the other way round: `inverse()` goes from tags to posts.
async fn tag(State(db): State<Db>, Path(id): Path<i64>) -> Result<View> {
    let tag = Tag::find_or_404(&db, id).await?;
    let mut by_tag: HashMap<i64, Vec<Post>> = POST_TAGS.inverse().load(&db, [tag.id]).await?;
    let posts = by_tag.remove(&tag.id).unwrap_or_default();
    Ok(view(
        "blog/list.html",
        context! { heading => format!("#{}", tag.name), posts },
    ))
}

/// A row of the report: columns by name.
#[derive(FromRow, Serialize)]
struct CategoryStats {
    category: String,
    posts: i64,
    comments: i64,
}

/// Joins and aggregates are clearest as SQL, read into structs or tuples.
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
    // A sub-query without writing SQL: posts that have at least one comment.
    let discussed = Post::query()
        .where_in_query("id", Comment::query(), "post_id")
        .count(&db)
        .await?;
    Ok(view(
        "blog/report.html",
        context! { categories, tags, discussed },
    ))
}
