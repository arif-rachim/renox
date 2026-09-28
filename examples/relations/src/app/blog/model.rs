use renox::db::relations::Pivot;
use renox::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "categories")]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "posts")]
pub struct Post {
    pub id: i64,
    /// The foreign key is a plain field: `belongs to` a category.
    pub category_id: Option<i64>,
    pub title: String,
    pub body: String,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "comments")]
pub struct Comment {
    pub id: i64,
    pub post_id: i64,
    pub author: String,
    pub body: String,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "tags")]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

/// Posts ⇄ tags. `POST_TAGS.inverse()` goes from a tag to its posts.
pub const POST_TAGS: Pivot = Pivot::new("post_tags", "post_id", "tag_id");

/// For one post, a method per relation: one query each, visible where it's
/// called. For a page of posts, use the loaders (see `blog::index`).
impl Post {
    pub async fn category(&self, db: &Db) -> Result<Option<Category>> {
        match self.category_id {
            Some(id) => Category::find(db, id).await,
            None => Ok(None),
        }
    }

    pub async fn comments(&self, db: &Db) -> Result<Vec<Comment>> {
        Comment::where_eq("post_id", self.id)
            .order_by("id")
            .get(db)
            .await
    }

    pub async fn tags(&self, db: &Db) -> Result<Vec<Tag>> {
        let ids = POST_TAGS.ids(db, self.id).await?;
        Tag::find_many(db, ids).await
    }
}

impl Category {
    /// "Has many" is a query on the other side.
    pub async fn posts(&self, db: &Db) -> Result<Vec<Post>> {
        Post::where_eq("category_id", self.id)
            .latest()
            .get(db)
            .await
    }
}
