//! No N+1: the likes of a page are loaded in a fixed number of queries,
//! however many posts, comments and likes there are. Its own test binary,
//! because it counts every statement of the process (sqlx reports each one
//! as a `tracing` event with the target `sqlx::query`).

use std::sync::atomic::{AtomicUsize, Ordering};

use relations::app::blog::model::{self, Comment, LIKEABLE, Like, Post};
use renox::prelude::*;
use renox::testing::TestApp;
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

static QUERIES: AtomicUsize = AtomicUsize::new(0);

/// Counts sqlx's statement events and ignores everything else.
struct CountQueries;

impl Subscriber for CountQueries {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.target() == "sqlx::query"
    }
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, _: &Event<'_>) {
        QUERIES.fetch_add(1, Ordering::SeqCst);
    }
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
}

/// The statements `f` runs.
async fn queries<T>(f: impl Future<Output = T>) -> (T, usize) {
    let before = QUERIES.load(Ordering::SeqCst);
    let out = f.await;
    (out, QUERIES.load(Ordering::SeqCst) - before)
}

#[renox::test]
async fn likes_of_a_page_load_in_one_query_per_type() {
    tracing::subscriber::set_global_default(CountQueries).unwrap();
    let app = TestApp::new(relations::app()).await;
    let db = app.db();
    let mut posts = Vec::new();
    let mut comments = Vec::new();
    for i in 0..5 {
        let post = Post {
            title: format!("Post {i}"),
            body: "Body.".into(),
            ..Default::default()
        };
        let post = Post::create(db, post).await.unwrap();
        for _ in 0..i {
            Like::create(db, Like::on(&post)).await.unwrap();
        }
        let comment = Comment {
            post_id: post.id,
            author: format!("Author {i}"),
            body: "Nice.".into(),
            ..Default::default()
        };
        let comment = Comment::create(db, comment).await.unwrap();
        Like::create(db, Like::on(&comment)).await.unwrap();
        posts.push(post);
        comments.push(comment);
    }
    // The counter works: one statement is one event.
    let (_, n) = queries(Post::query().count(db)).await;
    assert_eq!(n, 1);

    // Like counts of a page of posts: one query (a comment's likes aren't
    // counted for the post with the same id).
    let (counts, n) = queries(model::like_counts(db, &posts)).await;
    assert_eq!(n, 1);
    let counts = counts.unwrap();
    assert_eq!(
        posts
            .iter()
            .map(|p| counts.get(&p.id).copied().unwrap_or(0))
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4]
    );
    let (counts, n) = queries(model::like_counts(db, &comments)).await;
    assert_eq!((n, counts.unwrap().values().sum::<i64>()), (1, 5));

    // The likes themselves, grouped by post: one query too.
    let (loaded, n) =
        queries(LIKEABLE.load_many(db, &posts, Like::query(), |l| l.likeable_id)).await;
    assert_eq!(n, 1);
    assert_eq!(loaded.unwrap()[&posts[4].id].len(), 4);

    // `morphTo`: 15 likes on posts and comments, then their parents: one
    // query for the likes and one per parent type.
    let (latest, n) = queries(model::latest_likes(db, 50)).await;
    assert_eq!(n, 3);
    let latest = latest.unwrap();
    assert_eq!(latest.len(), 15);
    assert_eq!(latest.iter().filter(|l| l.post.is_some()).count(), 10);
    assert_eq!(latest.iter().filter(|l| l.comment.is_some()).count(), 5);
}
