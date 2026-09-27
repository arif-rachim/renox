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
    sql("CREATE TEMPORARY TABLE produk (id BIGSERIAL PRIMARY KEY, nama TEXT NOT NULL, harga BIGINT NOT NULL)")
        .execute(&mut tx)
        .await
        .unwrap();
    let inserted = sql("INSERT INTO produk (nama, harga) VALUES (?, ?), (?, ?)")
        .bind("Kopi")
        .bind(18_000_i64)
        .bind("Teh '?'")
        .bind(9_000_i64)
        .execute(&mut tx)
        .await
        .unwrap();
    assert_eq!(inserted, 2);
    let rows = sql("SELECT nama, harga FROM produk WHERE harga < ? AND nama <> '?' ORDER BY nama")
        .bind(20_000_i64)
        .fetch_all(&mut tx)
        .await
        .unwrap();
    assert_eq!(rows[0].columns(), ["nama", "harga"]);
    assert_eq!(rows[1].try_get::<String>("nama").unwrap(), "Teh '?'");
    let total: i64 = sql("SELECT SUM(harga)::BIGINT FROM produk")
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
    sql("CREATE TEMPORARY TABLE catatan (aktif BOOLEAN, pada TIMESTAMPTZ, tanggal DATE, isi TEXT)")
        .execute(&mut tx)
        .await
        .unwrap();
    let at = renox::db::now();
    let day = at.date_naive();
    // NULL goes in untyped, so it fits a TIMESTAMPTZ as well as a TEXT column.
    sql("INSERT INTO catatan (aktif, pada, tanggal, isi) VALUES (?, ?, ?, ?), (?, ?, ?, ?)")
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
    let row = sql("SELECT aktif, pada, tanggal FROM catatan WHERE aktif = ?")
        .bind(true)
        .fetch_one(&mut tx)
        .await
        .unwrap();
    assert!(row.try_get::<bool>("aktif").unwrap());
    assert_eq!(row.try_get::<renox::db::DateTime>("pada").unwrap(), at);
    assert_eq!(row.try_get::<chrono::NaiveDate>("tanggal").unwrap(), day);
    let empty: Option<renox::db::DateTime> = sql("SELECT pada FROM catatan WHERE NOT aktif")
        .scalar(&mut tx)
        .await
        .unwrap();
    assert_eq!(empty, None);
}
