//! Runs only with `--features postgres` and `RENOX_TEST_POSTGRES_URL` set,
//! e.g. `postgres://postgres:postgres@localhost:5432/renox_test`.

use renox::db::{Db, Dialect, sql};

async fn db() -> Option<Db> {
    let url = std::env::var("RENOX_TEST_POSTGRES_URL").ok()?;
    let pool = renox::sqlx::PgPool::connect(&url)
        .await
        .expect("RENOX_TEST_POSTGRES_URL is reachable");
    Some(Db::from(pool))
}

#[tokio::test]
async fn raw_sql_runs_on_postgres() {
    let Some(db) = db().await else {
        eprintln!("skipped: RENOX_TEST_POSTGRES_URL is not set");
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
