use std::collections::HashMap;

use renox::db::relations::{Morph, Pivot};
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
/// `with_timestamps()`: attaching (`attach`, `attach_with`, `sync`) fills the
/// pivot's `created_at`/`updated_at`, and `update_pivot` its `updated_at`.
pub const POST_TAGS: Pivot = Pivot::new("post_tags", "post_id", "tag_id").with_timestamps();

/// A like on a post or a comment: a polymorphic "belongs to". The type
/// column holds the parent's table (`Post::TABLE`, `Comment::TABLE`).
#[derive(Model, Serialize, Deserialize, Default, Debug, Clone)]
#[model(table = "likes")]
pub struct Like {
    pub id: i64,
    pub likeable_type: String,
    pub likeable_id: i64,
    pub created_at: Option<DateTime>,
    pub updated_at: Option<DateTime>,
}

/// Posts and comments ⇄ likes (`morphMany` / `morphTo`).
pub const LIKEABLE: Morph = Morph::new("likeable_type", "likeable_id");

impl Like {
    /// A new like on `parent`, a post or a comment.
    pub fn on<P: Model>(parent: &P) -> Self {
        Like {
            likeable_type: P::TABLE.into(),
            likeable_id: parent.id(),
            ..Default::default()
        }
    }
}

/// The number of likes of each post (or each comment) of a page, in one
/// `GROUP BY` query; 0 for those without likes. `LIKEABLE.count_many`
/// counts rows without loading them; `LIKEABLE.load_many` is the loader
/// for when a page needs the likes themselves.
pub fn like_counts<'a, P: Model>(
    db: &'a Db,
    parents: &[P],
) -> impl Future<Output = Result<HashMap<i64, i64>>> + Send + 'a {
    LIKEABLE.count_many(db, parents, Like::query())
}

/// A like with what was liked, for the "latest likes" list.
#[derive(Serialize, Debug)]
pub struct LikeOn {
    #[serde(flatten)]
    pub like: Like,
    pub post: Option<Post>,
    pub comment: Option<Comment>,
}

/// The latest likes and their parents (`morphTo`): one query for the likes,
/// then one per parent type (`LIKEABLE.parents`), however many likes.
pub async fn latest_likes(db: &Db, limit: u64) -> Result<Vec<LikeOn>> {
    let likes = Like::query()
        .order_by_desc("id")
        .limit(limit)
        .get(db)
        .await?;
    let parent = |like: &Like| (like.likeable_type.clone(), like.likeable_id);
    let posts = LIKEABLE.parents::<Post, _>(db, &likes, parent).await?;
    let comments = LIKEABLE.parents::<Comment, _>(db, &likes, parent).await?;
    Ok(likes
        .into_iter()
        .map(|like| {
            let (post, comment) = match like.likeable_type.as_str() {
                Post::TABLE => (posts.get(&like.likeable_id).cloned(), None),
                _ => (None, comments.get(&like.likeable_id).cloned()),
            };
            LikeOn {
                like,
                post,
                comment,
            }
        })
        .collect())
}

/// The pivot's own columns, read next to each tag (or post) by
/// `Pivot::load_with_pivot`.
#[derive(FromRow, Serialize, Debug, Clone)]
pub struct Tagging {
    /// Pinned posts come first on the tag's page.
    pub pinned: bool,
    /// When the post was tagged.
    pub created_at: Option<DateTime>,
}

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

    /// The tags with the pivot's columns (pinned, tagged when), by name.
    pub async fn taggings(&self, db: &Db) -> Result<Vec<(Tag, Tagging)>> {
        let mut by_post = POST_TAGS
            .load_with_pivot::<Tag, Tagging>(db, [self.id])
            .await?;
        let mut taggings = by_post.remove(&self.id).unwrap_or_default();
        taggings.sort_by(|(a, _), (b, _)| a.name.cmp(&b.name));
        Ok(taggings)
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
