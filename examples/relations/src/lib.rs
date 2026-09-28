//! Example: relations the Renox way, on a small blog. A post belongs to a
//! category, has many comments, and has many tags through a pivot table.
//!
//! - One row: a method per relation (`post.comments(&db)`), in
//!   [`app::blog::model`].
//! - A page of rows: `belongs_to`, `has_many` and `Pivot::load_for` fetch
//!   each relation of the whole page in one query (no N+1).
//! - Changing a many-to-many: `Pivot::sync` from a form's checkboxes.
//! - Reports: SQL joins read into `#[derive(FromRow)]` structs and tuples.
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

use app::blog::model::{Category, Comment, POST_TAGS, Post, Tag};

pub fn app() -> App {
    App::new()
        .embed(renox::embedded!())
        .migrations(renox::migrations!())
        .module(app::blog::Blog)
        .seeder(seed)
}

async fn seed(db: Db) -> Result {
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
            Comment::create(&db, comment).await?;
        }
        let picked: Vec<i64> = tags
            .iter()
            .filter(|_| (0..3).fake::<u8>() == 0)
            .map(|t| t.id)
            .collect();
        POST_TAGS.attach(&db, post.id, picked).await?;
    }
    Ok(())
}
