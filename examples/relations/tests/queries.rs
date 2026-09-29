//! No N+1: the likes of a page are loaded in a fixed number of queries,
//! however many posts, comments and likes there are.
//! `renox::db::capture_queries` records the statements a future runs.

use relations::app::blog::model::{self, Comment, LIKEABLE, Like, Post};
use renox::prelude::*;
use renox::testing::TestApp;

/// The statements `f` runs.
async fn queries<T>(f: impl Future<Output = T>) -> (T, usize) {
    let (out, statements) = renox::db::capture_queries(f).await;
    (out, statements.len())
}

#[renox::test]
async fn likes_of_a_page_load_in_one_query_per_type() {
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
    // The counter works: one statement is one entry.
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
