//! PostgreSQL-only checks. They run with `--features postgres` and
//! `TEST_DATABASE_URL` set, e.g. `postgres://postgres:postgres@localhost:5432/renox_test`
//! (which also runs every other test against PostgreSQL).

use renox::chrono;
use renox::db::{Db, Dialect, sql};

async fn db() -> Option<Db> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    let pool = renox::db::sqlx::PgPool::connect(&url)
        .await
        .expect("TEST_DATABASE_URL is reachable");
    Some(Db::from(pool))
}

#[tokio::test]
async fn raw_sql_runs_on_postgres() {
    let Some(db) = db().await else {
        eprintln!("skipped: TEST_DATABASE_URL is not set");
        return;
    };
    assert_eq!(db.dialect(), Dialect::Postgres);
    let mut tx = db.begin().await.unwrap();
    sql("CREATE TEMPORARY TABLE products (id BIGSERIAL PRIMARY KEY, name TEXT NOT NULL, price BIGINT NOT NULL)")
        .execute(&mut tx)
        .await
        .unwrap();
    let inserted = sql("INSERT INTO products (name, price) VALUES (?, ?), (?, ?)")
        .bind("Coffee")
        .bind(18_000_i64)
        .bind("Tea '?'")
        .bind(9_000_i64)
        .execute(&mut tx)
        .await
        .unwrap();
    assert_eq!(inserted, 2);
    let rows =
        sql("SELECT name, price FROM products WHERE price < ? AND name <> '?' ORDER BY name")
            .bind(20_000_i64)
            .fetch_all(&mut tx)
            .await
            .unwrap();
    assert_eq!(rows[0].columns(), ["name", "price"]);
    assert_eq!(rows[1].try_get::<String>("name").unwrap(), "Tea '?'");
    let total: i64 = sql("SELECT SUM(price)::BIGINT FROM products")
        .scalar(&mut tx)
        .await
        .unwrap();
    assert_eq!(total, 27_000);
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn typed_values_bind_on_postgres() {
    let Some(db) = db().await else {
        eprintln!("skipped: TEST_DATABASE_URL is not set");
        return;
    };
    let mut tx = db.begin().await.unwrap();
    sql("CREATE TEMPORARY TABLE typed_values (active BOOLEAN, happened_at TIMESTAMPTZ, day DATE, body TEXT)")
        .execute(&mut tx)
        .await
        .unwrap();
    let at = renox::db::now();
    let day = at.date_naive();
    // NULL goes in untyped, so it fits a TIMESTAMPTZ as well as a TEXT column.
    sql("INSERT INTO typed_values (active, happened_at, day, body) VALUES (?, ?, ?, ?), (?, ?, ?, ?)")
        .bind(true)
        .bind(at)
        .bind(day)
        .bind("x")
        .bind(false)
        .bind(None::<renox::db::DateTime>)
        .bind(None::<chrono::NaiveDate>)
        .bind(None::<String>)
        .execute(&mut tx)
        .await
        .unwrap();
    let row = sql("SELECT active, happened_at, day FROM typed_values WHERE active = ?")
        .bind(true)
        .fetch_one(&mut tx)
        .await
        .unwrap();
    assert!(row.try_get::<bool>("active").unwrap());
    assert_eq!(
        row.try_get::<renox::db::DateTime>("happened_at").unwrap(),
        at
    );
    assert_eq!(row.try_get::<chrono::NaiveDate>("day").unwrap(), day);
    let empty: Option<renox::db::DateTime> =
        sql("SELECT happened_at FROM typed_values WHERE NOT active")
            .scalar(&mut tx)
            .await
            .unwrap();
    assert_eq!(empty, None);
}
