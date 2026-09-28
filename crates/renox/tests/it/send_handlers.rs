//! Every data API with a closure, a generic executor or a borrowed slice,
//! called from a handler that is really routed. axum needs a handler's
//! future to be `Send`; an API whose future holds a closure over `&T` across
//! an `.await` fails that check (rustc issue #100013) only where it's
//! routed, so doctests and plain `async fn` tests don't catch it. The
//! relation loaders did, until examples/shop routed them.

use std::time::Duration;

use renox::db::relations::{Pivot, belongs_to, has_many};
use renox::prelude::*;
use renox::testing::TestApp;

#[derive(Model, serde::Serialize, Default, Clone)]
#[model(table = "notes")]
struct Note {
    id: i64,
    body: String,
    stars: i64,
}

#[derive(Model, serde::Serialize, Default, Clone)]
#[model(table = "tags")]
struct Tag {
    id: i64,
    note_id: Option<i64>,
    name: String,
}

const NOTE_TAGS: Pivot = Pivot::new("note_tags", "note_id", "tag_id");

async fn relations(State(db): State<Db>) -> Result<String> {
    let notes = Note::query().order_by("id").get(&db).await?;
    let tags = Tag::query().get(&db).await?;
    let parents = belongs_to::<Note, _, _>(&db, &tags, |t| t.note_id).await?;
    let children = has_many(&db, &notes, Tag::query(), "note_id", |t| t.note_id).await?;
    let linked = NOTE_TAGS.load_for::<Tag, _>(&db, &notes).await?;
    let loaded = NOTE_TAGS
        .load::<Tag>(&db, notes.iter().map(|n| n.id))
        .await?;
    NOTE_TAGS.attach(&db, 1, [1]).await?;
    NOTE_TAGS.detach(&db, 1, vec![2]).await?;
    NOTE_TAGS.sync(&db, 1, [1]).await?;
    let ids = NOTE_TAGS.ids(&db, 1).await?;
    Ok(format!(
        "{} {} {} {} {:?}",
        parents.len(),
        children.len(),
        linked.len(),
        loaded.len(),
        ids
    ))
}

/// A helper taking `&Db`, the way apps split handlers up.
async fn tagged(db: &Db, notes: &[Note]) -> Result<usize> {
    Ok(
        belongs_to::<Note, _, _>(db, &Tag::query().get(db).await?, |t| t.note_id)
            .await?
            .len()
            + notes.len(),
    )
}

async fn queries(State(state): State<AppState>) -> Result<String> {
    let db = &state.db;
    let first = Note::where_eq("body", "a")
        .first_or_create(db, || Note {
            body: "a".into(),
            ..Default::default()
        })
        .await?;
    let mut seen = 0;
    Note::query()
        .chunk(db, 10, |rows| {
            seen += rows.len();
            async { Ok(()) }
        })
        .await?;
    let cached: i64 = state
        .cache
        .remember("notes", Duration::from_secs(5), || async {
            Note::query().count(db).await.map(|n| n as i64)
        })
        .await?;
    let notes = Note::find_many(db, vec![first.id]).await?;
    let helped = tagged(db, &notes).await?;
    let names: Vec<String> = Note::query().pluck(db, "body").await?;
    let mut tx = db.begin().await?;
    Note::create(
        &mut tx,
        Note {
            body: "b".into(),
            ..Default::default()
        },
    )
    .await?;
    Note::where_eq("body", "b")
        .update(&mut tx, &[("body", &"c")])
        .await?;
    tx.commit().await?;
    Ok(format!("{seen} {cached} {helped} {}", names.len()))
}

/// The rest of the query builder and model API, each awaited in a handler.
async fn more(State(state): State<AppState>, user: Option<AuthUser>) -> Result<String> {
    let db = &state.db;
    let page = Note::query().latest().paginate(db, 1, 10).await?;
    let total: i64 = Note::query().sum(db, "id").await?;
    Note::where_eq("id", 1).increment(db, "id", 0).await?;
    let exists = Note::where_eq("id", 1).exists(db).await?;
    let found = Note::find(db, 1).await?;
    let one = Note::find_or_404(db, 1).await?;
    Note::insert_many(
        db,
        vec![Note {
            body: "d".into(),
            ..Default::default()
        }],
    )
    .await?;
    Note::upsert(db, vec![one.clone()], &["body"], &["stars"]).await?;
    let gone = Note::where_eq("body", "d").delete(db).await?;
    let rows = renox::db::sql("SELECT id, body FROM notes WHERE id = ?")
        .bind(1)
        .fetch_optional(db)
        .await?;
    let pairs: Vec<(i64, String)> = renox::db::sql("SELECT id, body FROM notes")
        .fetch_as(db)
        .await?;
    if let Some(user) = user {
        user.notifications(db, 5).await?;
    }
    state.emit(Ping).await?;
    Ok(format!(
        "{} {total} {exists} {} {gone} {} {}",
        page.total,
        found.is_some(),
        rows.is_some(),
        pairs.len()
    ))
}

#[derive(Clone)]
struct Ping;

impl Event for Ping {}

/// Roles, permissions, token abilities and scoped rules (routed so the
/// futures are checked for `Send`; the tables aren't created here).
async fn access(State(db): State<Db>, user: AuthUser, session: Session) -> Result<String> {
    use renox::auth::permissions;
    permissions::define_role(&db, "editor", &["posts.publish"]).await?;
    permissions::grant(&db, "editor", &["posts.edit"]).await?;
    permissions::revoke(&db, "editor", &["posts.edit"]).await?;
    user.assign_role(&db, "editor").await?;
    user.sync_roles(&db, &["editor"]).await?;
    user.remove_role(&db, "editor").await?;
    let roles = user.roles(&db).await?;
    let all = permissions::roles(&db).await?;
    permissions::delete_role(&db, "editor").await?;
    user.create_token_with(&db, "t", &["a"], None).await?;
    renox::auth::prune_expired_tokens(&db, Duration::from_secs(60)).await?;
    let scoped = Note::unscoped().none().count(&db).await?;
    let mut me = user.user().clone();
    renox::auth::change_password(&db, &session, &mut me, "a new password").await?;
    renox::auth::logout_other_devices(&db, &session, &me).await?;
    renox::audit::record(&db, renox::audit::Entry::new("x").user(me.id)).await?;
    renox::audit::latest(&db, 5).await?;
    renox::audit::for_user(&db, me.id, 5).await?;
    renox::audit::prune(&db, Duration::from_secs(60)).await?;
    renox::auth::logout(&db, &session).await?;
    me.delete_account(&db).await?;
    Ok(format!("{} {} {scoped}", roles.len(), all.len()))
}

struct Handlers;

impl Module for Handlers {
    fn name(&self) -> &'static str {
        "handlers"
    }

    fn routes(&self) -> Routes {
        Routes::new()
            .get("/relations", relations)
            .get("/queries", queries)
            .get("/more", more)
            .get("/access", access)
    }
}

#[renox::test]
async fn data_apis_work_in_routed_handlers() {
    let app = TestApp::new(App::new().module(Handlers)).await;
    let id = match app.db().dialect() {
        renox::db::Dialect::Postgres => "BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY",
        _ => "INTEGER PRIMARY KEY",
    };
    for statement in [
        format!(
            "CREATE TABLE notes (id {id}, body TEXT NOT NULL UNIQUE, stars BIGINT NOT NULL DEFAULT 0)"
        ),
        format!("CREATE TABLE tags (id {id}, note_id BIGINT, name TEXT NOT NULL)"),
        "CREATE TABLE note_tags (note_id BIGINT NOT NULL, tag_id BIGINT NOT NULL)".into(),
        "INSERT INTO notes (body) VALUES ('x')".into(),
        "INSERT INTO tags (note_id, name) VALUES (1, 'kopi'), (NULL, 'teh')".into(),
    ] {
        renox::db::sql(statement).execute(app.db()).await.unwrap();
    }
    app.get("/relations")
        .await
        .assert_ok()
        .assert_see("1 1 0 0 [1]");
    app.get("/queries").await.assert_ok().assert_see("2 2 2 2");
    app.get("/more")
        .await
        .assert_ok()
        .assert_see("3 6 true true 1 true 3");
}
