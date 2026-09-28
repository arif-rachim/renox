use relations::app::blog::model::{Category, Comment, POST_TAGS, Post, Tag};
use renox::prelude::*;
use renox::testing::TestApp;

struct Blog {
    app: TestApp,
    coffee: Category,
    rust: Post,
    beans: Post,
    loose: Post,
    tags: Vec<Tag>,
}

/// Two categories, three posts (one without a category), three tags and
/// some comments.
async fn blog() -> Blog {
    let app = TestApp::new(relations::app()).await;
    let db = app.db();
    let make = |name: &str| Category {
        name: name.into(),
        ..Default::default()
    };
    let coffee = Category::create(db, make("Coffee")).await.unwrap();
    let code = Category::create(db, make("Code")).await.unwrap();
    let post = |title: &str, category: Option<i64>| Post {
        title: title.into(),
        body: format!("All about {title}."),
        category_id: category,
        ..Default::default()
    };
    let beans = Post::create(db, post("Beans", Some(coffee.id)))
        .await
        .unwrap();
    let rust = Post::create(db, post("Rust", Some(code.id))).await.unwrap();
    let loose = Post::create(db, post("Loose", None)).await.unwrap();
    let mut tags = Vec::new();
    for name in ["howto", "news", "review"] {
        let tag = Tag {
            name: name.into(),
            ..Default::default()
        };
        tags.push(Tag::create(db, tag).await.unwrap());
    }
    POST_TAGS
        .attach(db, beans.id, [tags[0].id, tags[2].id])
        .await
        .unwrap();
    POST_TAGS.attach(db, rust.id, [tags[0].id]).await.unwrap();
    for (post_id, author) in [(beans.id, "Ani"), (beans.id, "Budi"), (rust.id, "Citra")] {
        let comment = Comment {
            post_id,
            author: author.into(),
            body: "Nice.".into(),
            ..Default::default()
        };
        Comment::create(db, comment).await.unwrap();
    }
    Blog {
        app,
        coffee,
        rust,
        beans,
        loose,
        tags,
    }
}

#[renox::test]
async fn the_list_shows_each_posts_relations() {
    let b = blog().await;
    let page = b.app.get("/").await;
    page.assert_ok()
        .assert_see("#howto")
        .assert_see("#review")
        .assert_see("2 comments; latest by Budi")
        .assert_see("1 comment; latest by Citra")
        .assert_see("0 comments")
        .assert_see("Uncategorized");
    // Each card has its own category.
    let text = page.text();
    let card = |title: &str| {
        let start = text.find(&format!(">{title}<")).unwrap();
        let end = text[start..].find("</article>").unwrap() + start;
        text[start..end].to_owned()
    };
    assert!(card("Beans").contains("Coffee") && card("Beans").contains("#review"));
    assert!(card("Rust").contains("Code") && !card("Rust").contains("#review"));
    assert!(card("Loose").contains("Uncategorized"));
}

#[renox::test]
async fn a_post_with_its_comments_and_new_ones() {
    let b = blog().await;
    let url = format!("/posts/{}", b.beans.id);
    b.app
        .get(&url)
        .await
        .assert_see("Comments (2)")
        .assert_see("<strong>Ani</strong>")
        .assert_see(">Coffee</a>");
    b.app
        .post(
            &format!("/posts/{}/comments", b.beans.id),
            &[("author", "Dewi"), ("body", "Where to buy?")],
        )
        .await
        .assert_redirect(&format!("{url}#comments"));
    b.app
        .get(&url)
        .await
        .assert_see("Comments (3)")
        .assert_see("Where to buy?");
    b.app
        .htmx()
        .post(
            &format!("/posts/{}/comments", b.beans.id),
            &[("author", ""), ("body", "")],
        )
        .await
        .assert_invalid("author")
        .assert_invalid("body");
    b.app
        .post("/posts/999/comments", &[("author", "X"), ("body", "Y")])
        .await
        .assert_not_found();
}

#[renox::test]
async fn tags_are_synced_from_checkboxes() {
    let b = blog().await;
    let url = format!("/posts/{}/tags", b.beans.id);
    let news = b.tags[1].id.to_string();
    let howto = b.tags[0].id.to_string();
    // Exactly the checked ones: howto stays, review goes, news comes.
    b.app
        .put(&url, &[("tags", &howto), ("tags", &news)])
        .await
        .assert_redirect(&format!("/posts/{}", b.beans.id));
    let mut ids = POST_TAGS.ids(b.app.db(), b.beans.id).await.unwrap();
    ids.sort();
    assert_eq!(ids, vec![b.tags[0].id, b.tags[1].id]);
    // Nothing checked: no tags.
    b.app.put(&url, &[]).await;
    assert!(
        POST_TAGS
            .ids(b.app.db(), b.beans.id)
            .await
            .unwrap()
            .is_empty()
    );
    // A tag that doesn't exist is refused, and nothing changes.
    b.app
        .htmx()
        .put(&url, &[("tags", &howto), ("tags", "999")])
        .await
        .assert_invalid("tags.1");
    assert!(
        POST_TAGS
            .ids(b.app.db(), b.beans.id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[renox::test]
async fn categories_and_tags_list_their_posts() {
    let b = blog().await;
    b.app
        .get(&format!("/categories/{}", b.coffee.id))
        .await
        .assert_see("<h1>Coffee</h1>")
        .assert_see(">Beans<")
        .assert_dont_see(">Rust<");
    b.app
        .get(&format!("/tags/{}", b.tags[0].id))
        .await
        .assert_see("<h1>#howto</h1>")
        .assert_see(">Beans<")
        .assert_see(">Rust<")
        .assert_dont_see(">Loose<");
    b.app
        .get(&format!("/tags/{}", b.tags[1].id))
        .await
        .assert_see("No posts.");
    b.app.get("/tags/999").await.assert_not_found();
}

#[renox::test]
async fn the_report_joins_and_counts() {
    let b = blog().await;
    b.app
        .get("/report")
        .await
        .assert_see("2 posts with comments.")
        .assert_see("<td>Coffee</td><td>1</td><td>2</td>")
        .assert_see("<td>Code</td><td>1</td><td>1</td>")
        .assert_see("<td>Uncategorized</td><td>1</td><td>0</td>")
        .assert_see("<td>#howto</td><td>2</td>")
        .assert_see("<td>#news</td><td>0</td>");
}

#[renox::test]
async fn deleting_a_post_takes_its_comments_and_links() {
    let b = blog().await;
    let mut beans = b.beans.clone();
    beans.delete(b.app.db()).await.unwrap();
    b.app.assert_database_count("comments", 1).await;
    b.app
        .assert_database_missing("post_tags", &[("post_id", &beans.id)])
        .await;
    // Deleting a category keeps its posts, without a category.
    let code = b.rust.category(b.app.db()).await.unwrap().unwrap();
    let mut code = code;
    code.delete(b.app.db()).await.unwrap();
    let rust = Post::find_or_404(b.app.db(), b.rust.id).await.unwrap();
    assert_eq!(rust.category_id, None);
    assert!(Post::find(b.app.db(), b.loose.id).await.unwrap().is_some());
}
