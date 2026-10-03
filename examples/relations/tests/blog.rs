use relations::app::blog::model::{Category, Comment, Like, POST_TAGS, Post, Tag, Tagging};
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
    for (post_id, author) in [(beans.id, "Anna"), (beans.id, "Ben"), (rust.id, "Clara")] {
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
        .assert_see("2 comments; latest by Ben")
        .assert_see("1 comment; latest by Clara")
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
        .assert_see("<strong>Anna</strong>")
        .assert_see(">Coffee</a>");
    b.app
        .post(
            &format!("/posts/{}/comments", b.beans.id),
            &[("author", "Diana"), ("body", "Where to buy?")],
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
        .assert_see(r#"<h1 class="rx-title">Coffee</h1>"#)
        .assert_see(">Beans<")
        .assert_dont_see(">Rust<");
    b.app
        .get(&format!("/tags/{}", b.tags[0].id))
        .await
        .assert_see(r#"<h1 class="rx-title">#howto</h1>"#)
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
        .assert_see(r#"<td>Coffee</td><td class="rx-num">1</td><td class="rx-num">2</td>"#)
        .assert_see(r#"<td>Code</td><td class="rx-num">1</td><td class="rx-num">1</td>"#)
        .assert_see(r#"<td>Uncategorized</td><td class="rx-num">1</td><td class="rx-num">0</td>"#)
        .assert_see(r#"<td>#howto</td><td class="rx-num">2</td>"#)
        .assert_see(r#"<td>#news</td><td class="rx-num">0</td>"#)
        .assert_see(r#"<td>Anna</td><td class="rx-num">1</td>"#);
}

#[renox::test]
async fn pivot_columns_pin_a_post_on_a_tag_page() {
    let b = blog().await;
    let db = b.app.db();
    let (howto, review) = (b.tags[0].id, b.tags[2].id);
    // Links carry timestamps; `pinned` defaults to false.
    let beans = b.beans.taggings(db).await.unwrap();
    assert_eq!(beans.len(), 2);
    assert!(
        beans
            .iter()
            .all(|(_, t)| !t.pinned && t.created_at.is_some())
    );
    b.app
        .get(&format!("/posts/{}", b.beans.id))
        .await
        .assert_see("tagged ")
        .assert_see("Pin on #howto");
    // `attach_with` sets pivot columns; an existing link is left alone.
    let added = POST_TAGS
        .attach_with(db, b.rust.id, howto, &[("pinned", &true)])
        .await
        .unwrap();
    assert!(!added);
    let added = POST_TAGS
        .attach_with(db, b.rust.id, review, &[("pinned", &true)])
        .await
        .unwrap();
    assert!(added);
    // Rust is pinned on #review, so it comes before Beans.
    let text = b.app.get(&format!("/tags/{review}")).await.text();
    let (rust, beans) = (text.find(">Rust<").unwrap(), text.find(">Beans<").unwrap());
    assert!(rust < beans && text.contains("pinned"));
    // Pinning from the post page: `update_pivot`.
    let url = format!("/posts/{}/tags/{howto}/pin", b.beans.id);
    b.app
        .put(&url, &[("pinned", "true")])
        .await
        .assert_redirect(&format!("/posts/{}", b.beans.id));
    let links = POST_TAGS
        .load_with_pivot::<Tag, Tagging>(db, [b.beans.id])
        .await
        .unwrap();
    let pinned: Vec<&str> = links[&b.beans.id]
        .iter()
        .filter(|(_, t)| t.pinned)
        .map(|(tag, _)| tag.name.as_str())
        .collect();
    assert_eq!(pinned, ["howto"]);
    b.app
        .get(&format!("/posts/{}", b.beans.id))
        .await
        .assert_see("Pinned.")
        .assert_see("Unpin on #howto");
    // A tag the post doesn't have: nothing to pin.
    b.app
        .put(
            &format!("/posts/{}/tags/{howto}/pin", b.loose.id),
            &[("pinned", "true")],
        )
        .await
        .assert_not_found();
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

#[renox::test]
async fn posts_and_comments_are_liked() {
    let b = blog().await;
    let url = format!("/posts/{}", b.beans.id);
    b.app.get(&url).await.assert_see("0 likes");
    b.app
        .post(&format!("{url}/like"), &[])
        .await
        .assert_redirect(&url);
    b.app.post(&format!("{url}/like"), &[]).await;
    let anna = Comment::where_eq("author", "Anna")
        .first(b.app.db())
        .await
        .unwrap()
        .unwrap();
    b.app
        .post(&format!("/comments/{}/like", anna.id), &[])
        .await
        .assert_redirect(&format!("{url}#comment-{}", anna.id));

    // Two likes on the post, one on Anna's comment, none on Ben's.
    let page = b.app.get(&url).await;
    page.assert_ok();
    let text = page.text();
    let likes = |from: &str| {
        let start = text.find(from).unwrap();
        let at = text[start..]
            .find(r#"<span class="likes rx-subtitle">"#)
            .unwrap()
            + start;
        text[at..at + 48].to_owned()
    };
    assert!(likes("<h1").contains(">2 likes<"));
    assert!(likes("<strong>Anna</strong>").contains(">1 like<"));
    assert!(likes("<strong>Ben</strong>").contains(">0 likes<"));
    b.app
        .assert_database_has(
            "likes",
            &[("likeable_type", &"comments"), ("likeable_id", &anna.id)],
        )
        .await;

    // The list counts the post's likes only; the report says what was liked.
    let text = b.app.get("/").await.text();
    let start = text.find(">Beans<").unwrap();
    let end = text[start..].find("</article>").unwrap() + start;
    assert!(text[start..end].contains("2 likes"));
    b.app
        .get("/report")
        .await
        .assert_see(">Beans</a>")
        .assert_see("a comment by Anna");

    b.app.post("/posts/999/like", &[]).await.assert_not_found();
    b.app
        .post("/comments/999/like", &[])
        .await
        .assert_not_found();
}

#[renox::test]
async fn deleting_a_parent_deletes_its_likes() {
    let b = blog().await;
    let db = b.app.db();
    let comment = Comment::where_eq("post_id", b.beans.id)
        .first(db)
        .await
        .unwrap()
        .unwrap();
    Like::create(db, Like::on(&b.beans)).await.unwrap();
    Like::create(db, Like::on(&comment)).await.unwrap();
    Like::create(db, Like::on(&b.rust)).await.unwrap();
    // The comment goes by ON DELETE CASCADE; the triggers take both likes.
    let mut beans = b.beans.clone();
    beans.delete(db).await.unwrap();
    b.app.assert_database_count("likes", 1).await;
}

#[renox::test]
async fn the_seeder_fills_the_app_and_can_run_again() {
    let app = TestApp::new(relations::app()).await;
    app.kernel().seed().await.unwrap();
    let seeded = Post::query().count(app.db()).await.unwrap();
    assert!(seeded > 0);
    // A second `db:seed` leaves a seeded database as it is.
    app.kernel().seed().await.unwrap();
    assert_eq!(Post::query().count(app.db()).await.unwrap(), seeded);
}

#[renox::test]
async fn posts_are_markdown_with_their_own_description() {
    let b = blog().await;
    let mut post = b.beans.clone();
    post.body = "Beans from **Ethiopia**, roasted on Tuesdays.\n\n## Brewing\n\n- V60\n- Aeropress\n\n<script>alert(1)</script>".into();
    post.save(b.app.db()).await.unwrap();
    b.app
        .get(&format!("/posts/{}", post.id))
        .await
        .assert_ok()
        .assert_see("<strong>Ethiopia</strong>")
        .assert_see("<h2>Brewing</h2>")
        .assert_see("<li>V60</li>")
        // Raw HTML in a body is shown as text, never run.
        .assert_dont_see("<script>alert(1)</script>")
        // Search engines and link previews get the first paragraph.
        .assert_see(
            r#"<meta name="description" content="Beans from Ethiopia, roasted on Tuesdays.">"#,
        )
        .assert_see(r#"<meta property="og:type" content="article">"#)
        .assert_see("<title>Beans · ");
    // The list shows it as plain text.
    b.app
        .get("/")
        .await
        .assert_see("Beans from Ethiopia, roasted on Tuesdays.");
}

#[renox::test]
async fn search_finds_every_word_in_titles_and_bodies() {
    let b = blog().await;
    let found = b.app.get("/?q=all+rust").await;
    found
        .assert_ok()
        .assert_see(">Rust</a>")
        .assert_dont_see(">Beans</a>")
        .assert_see("Posts with “all rust”");
    b.app
        .get("/?q=nothing-like-this")
        .await
        .assert_see("No posts with “nothing-like-this”");
    // Odd input is only text to look for: no error.
    b.app.get("/?q=%25_%27").await.assert_ok();
}

#[renox::test]
async fn the_feed_and_the_sitemap_list_the_posts() {
    let b = blog().await;
    let feed = b.app.get("/feed.xml").await;
    feed.assert_ok()
        .assert_header("content-type", "application/rss+xml; charset=utf-8");
    let xml = feed.text();
    assert!(xml.starts_with("<?xml"), "{xml}");
    assert!(xml.contains("<title>Beans</title>"), "{xml}");
    assert!(
        xml.contains(&format!("http://127.0.0.1:3000/posts/{}</link>", b.rust.id)),
        "{xml}"
    );
    // A reader that has this version gets a 304 without the body.
    let tag = feed.header("etag").expect("an ETag").to_owned();
    b.app
        .request()
        .header("if-none-match", &tag)
        .get("/feed.xml")
        .await
        .assert_status(304);
    // The pages themselves don't get one (`.etag()` covers only the feeds).
    assert_eq!(b.app.get("/").await.header("etag"), None);
    let sitemap = b.app.get("/sitemap.xml").await;
    sitemap.assert_ok();
    let text = sitemap.text();
    assert!(
        text.contains(&format!("/posts/{}</loc>", b.loose.id)),
        "{text}"
    );
    assert!(
        text.contains(&format!("/categories/{}</loc>", b.coffee.id)),
        "{text}"
    );
    // Every page links the feed for readers that look for it.
    b.app
        .get("/")
        .await
        .assert_see(r#"<link rel="alternate" type="application/rss+xml""#);
}
