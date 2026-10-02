//! Example: relations the Renox way, on a small blog. A post belongs to a
//! category, has many comments, and has many tags through a pivot table.
//!
//! - One row: a method per relation (`post.comments(&db)`), in
//!   [`app::blog::model`].
//! - A page of rows: `belongs_to`, `has_many`, `count_many` and
//!   `Pivot::load_for` fetch each relation of the whole page in one query
//!   (no N+1).
//! - Changing a many-to-many: `Pivot::sync` from a form's checkboxes.
//! - Pivot columns: `Pivot::with_timestamps`, `attach_with`, `update_pivot`
//!   and `load_with_pivot` (a post pinned on a tag's page).
//! - Polymorphic: likes on posts or comments (`Morph`: `of`, `parents`,
//!   and `Morph::count_many` for many parents in one query).
//! - Reports: SQL joins read into `#[derive(FromRow)]` structs and tuples,
//!   `group_by` + `select_as` for one table, `where_has` for "has any".
//!
//! See docs/relations.md for the reference.
//!
//! ```text
//! cargo run -- migrate
//! cargo run -- db:seed
//! cargo run
//! ```

pub mod app;

use renox::fake::Fake;
use renox::fake::faker::lorem::en::{Paragraph, Sentence};
use renox::fake::faker::name::en::FirstName;
use renox::prelude::*;

use app::blog::model::{Category, Comment, Like, POST_TAGS, Post, Tag};

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        .module(app::blog::Blog)
        .seeder(seed)
}

async fn seed(db: Db) -> Result {
    // Seeding twice is harmless: a seeded database stays as it is.
    if Category::query().exists(&db).await? {
        return Ok(());
    }
    let mut categories = Vec::new();
    for name in ["Coffee", "Travel", "Rust"] {
        let category = Category {
            name: name.into(),
            ..Default::default()
        };
        categories.push(Category::create(&db, category).await?);
    }
    let mut tags = Vec::new();
    for name in ["beginner", "howto", "news", "opinion", "review"] {
        let tag = Tag {
            name: name.into(),
            ..Default::default()
        };
        tags.push(Tag::create(&db, tag).await?);
    }
    for i in 0..24 {
        let post = Post::create(
            &db,
            Post {
                category_id: (i % 4 != 3).then(|| categories[i % 3].id),
                title: Sentence(3..7).fake(),
                body: Paragraph(2..4).fake(),
                ..Default::default()
            },
        )
        .await?;
        for _ in 0..(0..4).fake::<usize>() {
            let comment = Comment {
                post_id: post.id,
                author: FirstName().fake(),
                body: Sentence(4..12).fake(),
                ..Default::default()
            };
            let comment = Comment::create(&db, comment).await?;
            for _ in 0..(0..3).fake::<usize>() {
                Like::create(&db, Like::on(&comment)).await?;
            }
        }
        for _ in 0..(0..6).fake::<usize>() {
            Like::create(&db, Like::on(&post)).await?;
        }
        let picked: Vec<i64> = tags
            .iter()
            .filter(|_| (0..3).fake::<u8>() == 0)
            .map(|t| t.id)
            .collect();
        // Every fifth post is pinned on its first tag's page: a pivot column.
        for (n, tag_id) in picked.into_iter().enumerate() {
            let pinned = n == 0 && i % 5 == 0;
            POST_TAGS
                .attach_with(&db, post.id, tag_id, &[("pinned", &pinned)])
                .await?;
        }
    }
    Ok(())
}
