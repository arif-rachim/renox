//! A blog showing every kind of relation without N+1 queries:
//! a post belongs to a category, has many comments, and has many tags
//! through a pivot table that has columns of its own (pinned, timestamps).
//! Likes belong to a post or a comment (a polymorphic relation, `Morph`).
//! Pages load the related rows of a whole page in one query per relation
//! (counts included); reports use the query builder's `group_by` and SQL
//! joins read into structs.
//!
//! It is also a public blog: bodies are Markdown (the `markdown` filter),
//! each post has its own title and description for search engines
//! (`seo()`), the posts are searchable (`?q=`, full-text: `Post::search`),
//! and there is an RSS feed
//! (`/feed.xml`) and a sitemap (`/sitemap.xml`, `renox::seo::Sitemap`). The
//! pages are styled with Tailwind (resources/css/app.css, built by
//! `rnx tailwind` into public/css/app.css).
//!
//! Made with `rnx make:module blog`, `rnx make:model Post --module blog` (and
//! `Category`, `Comment`, `Tag`, `Like`) and `rnx make:migration` for each
//! migration (the blog tables, the pivot columns, the likes table), then
//! filled in.

pub mod model;

use std::collections::HashMap;

use renox::db::relations::{belongs_to, count_many, has_many, has_many_through};
use renox::prelude::*;
use serde::{Deserialize, Serialize};

use model::{Category, Comment, LIKEABLE, Like, POST_TAGS, Post, Tag, Tagging};

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
            .post("/posts/{id}/like", like_post)
            .name("posts.like")
            .post("/comments/{id}/like", like_comment)
            .name("comments.like")
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
            .merge(
                Routes::new()
                    .get("/feed.xml", feed)
                    .name("feed")
                    // Named `sitemap`: robots.txt points search engines at it.
                    .get("/sitemap.xml", sitemap)
                    .name("sitemap")
                    // Feed readers and crawlers ask again and again: a 304
                    // without the body when nothing changed since their last
                    // visit. (A layer covers the routes added before it, so
                    // these two have a group of their own.)
                    .etag(),
            )
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
    likes: i64,
    /// The first paragraph as plain text, under the title.
    summary: String,
}

/// `?q=`: words to look for in titles and bodies.
#[derive(Deserialize, Default)]
struct Search {
    q: Option<String>,
}

/// 10 posts with their categories, comment counts, latest comments, tags
/// and like counts in 8 queries in all (count and page, categories, comment
/// counts, latest comments, tag links and tags, like counts), whatever the
/// page size. `?q=` keeps the posts whose title or body has every word
/// (or a longer form of it: "roast" finds "roasting"), best match first.
async fn index(
    State(db): State<Db>,
    Page(page): Page,
    Query(search): Query<Search>,
) -> Result<View> {
    let q = search.q.unwrap_or_default().trim().to_owned();
    // Full-text: best matches first, then the newest. Without words (no
    // `q`), every post, newest first.
    let posts = Post::search(&q).latest().paginate(&db, page, 10).await?;
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
    // Polymorphic: only likes whose `likeable_type` is "posts".
    let likes = model::like_counts(&db, &posts.items).await?;
    let posts = posts.map(|post| Card {
        category: post.category_id.and_then(|id| categories.get(&id).cloned()),
        tags: tags.remove(&post.id).unwrap_or_default(),
        comments: counts.get(&post.id).copied().unwrap_or(0),
        latest_comment: latest.remove(&post.id).and_then(|mut c| c.pop()),
        likes: likes.get(&post.id).copied().unwrap_or(0),
        summary: post.summary(220),
        post,
    });
    Ok(view("blog/index.html", context! { posts, q }))
}

/// A tag of the post with the pivot's columns, for the template.
#[derive(Serialize)]
struct TagLink {
    #[serde(flatten)]
    tag: Tag,
    pivot: Tagging,
}

/// A comment with its number of likes, for the template.
#[derive(Serialize)]
struct CommentRow {
    #[serde(flatten)]
    comment: Comment,
    likes: i64,
}

/// One post: the relation methods on `Post`, one query each (the tags with
/// their pivot columns: one for the links, one for the tags). Likes: one
/// query for the post's (`LIKEABLE.of`), one for all its comments'.
/// `Found` loads the post `{id}` names, or answers 404.
async fn show(State(db): State<Db>, Found(post): Found<Post>) -> Result<View> {
    let category = post.category(&db).await?;
    let comments = post.comments(&db).await?;
    let likes = LIKEABLE.of(&post, Like::query()).count(&db).await?;
    let comment_likes = model::like_counts(&db, &comments).await?;
    let comments: Vec<CommentRow> = comments
        .into_iter()
        .map(|comment| CommentRow {
            likes: comment_likes.get(&comment.id).copied().unwrap_or(0),
            comment,
        })
        .collect();
    let tags: Vec<TagLink> = post
        .taggings(&db)
        .await?
        .into_iter()
        .map(|(tag, pivot)| TagLink { tag, pivot })
        .collect();
    let tag_ids: Vec<i64> = tags.iter().map(|t| t.tag.id).collect();
    // Every tag as [id, name], the kit's checkbox_list options.
    let tag_options: Vec<(i64, String)> = Tag::query()
        .order_by("name")
        .get(&db)
        .await?
        .into_iter()
        .map(|tag| (tag.id, tag.name))
        .collect();
    let description = post.summary(160);
    Ok(view(
        "blog/show.html",
        context! { post, description, category, comments, tags, tag_ids, tag_options, likes },
    ))
}

#[derive(Deserialize, Serialize, Validate)]
struct CommentForm {
    #[validate(required, max = 50)]
    author: String,
    #[validate(required, max = 1000)]
    body: String,
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

/// Likes the post: a `Like` whose type is "posts".
async fn like_post(State(db): State<Db>, Path(id): Path<i64>) -> Result<Redirect> {
    let post = Post::find_or_404(&db, id).await?;
    Like::create(&db, Like::on(&post)).await?;
    Redirect::route("posts.show", &[&post.id])
}

/// Likes a comment: the same table, type "comments".
async fn like_comment(State(db): State<Db>, Path(id): Path<i64>) -> Result<Redirect> {
    let comment = Comment::find_or_404(&db, id).await?;
    Like::create(&db, Like::on(&comment)).await?;
    Ok(Redirect::to(&format!(
        "/posts/{}#comment-{}",
        comment.post_id, comment.id
    )))
}

/// The checked boxes of the tag form (none checked sends nothing).
#[derive(Deserialize, Validate)]
struct TagsForm {
    #[serde(default)]
    #[validate(each(exists("tags", "id")))]
    tags: Vec<i64>,
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
    Redirect::route("posts.show", &[&post.id])
}

/// A checkbox: nothing to check, but `Valid` still turns "on"/missing
/// into a bool.
#[derive(Deserialize, Validate)]
struct PinForm {
    pinned: bool,
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
    Redirect::route("posts.show", &[&post.id])
}

/// "Has many" from the other side: the category's posts. And "has many
/// through": the latest comments on any of them, the category reaching its
/// comments through its posts (two queries, `has_many_through`).
async fn category(State(db): State<Db>, Found(category): Found<Category>) -> Result<View> {
    let posts = category.posts(&db).await?;
    let mut through = has_many_through(
        &db,
        std::slice::from_ref(&category),
        Post::query(),
        "category_id",
        |p: &Post| p.category_id,
        Comment::query().latest().limit(5),
        "post_id",
        |c: &Comment| c.post_id,
    )
    .await?;
    let comments = through.remove(&category.id).unwrap_or_default();
    let pinned: Vec<i64> = Vec::new();
    Ok(view(
        "blog/list.html",
        context! { heading => category.name, posts, pinned, comments },
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
    // `morphTo`: the latest likes with the post or comment each is on.
    let likes = model::latest_likes(&db, 10).await?;
    Ok(view(
        "blog/report.html",
        context! { categories, tags, commenters, discussed, likes },
    ))
}

/// The 20 latest posts as RSS 2.0, for feed readers.
async fn feed(State(state): State<AppState>) -> Result<Response> {
    let posts = Post::query().latest().limit(20).get(&state.db).await?;
    let site = state.config.url.trim_end_matches('/');
    let mut items = String::new();
    for post in &posts {
        let link = format!("{site}{}", state.url("posts.show", &[&post.id])?);
        let date = post
            .created_at
            .map(|at| at.to_rfc2822())
            .unwrap_or_default();
        items.push_str(&format!(
            "<item><title>{}</title><link>{link}</link><guid>{link}</guid>\
             <pubDate>{date}</pubDate><description>{}</description></item>",
            xml(&post.title),
            xml(&post.summary(300)),
        ));
    }
    let body = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <rss version=\"2.0\"><channel><title>{}</title><link>{site}/</link>\
         <description>The latest posts</description>{items}</channel></rss>",
        xml(&state.config.name)
    );
    Ok((
        [(
            renox::axum::http::header::CONTENT_TYPE,
            "application/rss+xml; charset=utf-8",
        )],
        body,
    )
        .into_response())
}

/// Text inside an XML element.
fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Every page worth finding: the home page, the posts, the categories.
async fn sitemap(State(state): State<AppState>) -> Result<renox::seo::Sitemap> {
    let mut map = renox::seo::Sitemap::new(&state).route("posts.index", &[], None)?;
    for post in Post::query().latest().get(&state.db).await? {
        map = map.route("posts.show", &[&post.id], post.updated_at)?;
    }
    for category in Category::all(&state.db).await? {
        map = map.route("categories.show", &[&category.id], None)?;
    }
    Ok(map)
}
