use sqlx::sqlite::SqlitePool;
use rustdata_core::{QueryRepository, backends::Sqlite};
use rustdata_core::Entity;
use rustdata_core::SqlValue;
use serde::{Deserialize, Serialize};

// ─── Entity ───────────────────────────────────────────────
// The #[derive(Entity)] macro auto-generates:
//   • EntityDescriptor (columns, bind_insert/update/id, from_row)
//   • RowExtractable  (extract_row → from_row)  ← enables QueryRepository
//
// Because the macro also generates RowExtractable for the struct itself,
// we can pass User directly to QueryRepository<B, User>.

#[derive(Debug, Clone, Serialize, Deserialize, Entity)]
#[entity(table = "users", order_by = "created_at DESC")]
struct User {
    #[entity(id)]
    id: uuid::Uuid,

    #[entity(column = "username")]
    username: String,

    #[entity(column = "email")]
    email: String,

    #[entity(column = "age")]
    age: i32,

    #[entity(column = "status")]
    status: String,

    #[entity(auto_generated)]
    created_at: chrono::DateTime<chrono::Utc>,
}

// Schema setup
async fn setup_schema(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
            username TEXT NOT NULL UNIQUE,
            email TEXT NOT NULL UNIQUE,
            age INTEGER NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
    "#).execute(pool).await?;
    Ok(())
}

// Insert test data
async fn insert_test_data(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Utc::now();

    sqlx::query("INSERT INTO users (id, username, email, age, status, created_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(uuid::Uuid::new_v4().to_string())
        .bind("john_admin")
        .bind("john.admin@example.com")
        .bind(35i32)
        .bind("active")
        .bind((now - chrono::Duration::days(60)).to_rfc3339())
        .execute(pool).await?;

    sqlx::query("INSERT INTO users (id, username, email, age, status, created_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(uuid::Uuid::new_v4().to_string())
        .bind("jane_user")
        .bind("jane@example.com")
        .bind(28i32)
        .bind("active")
        .bind((now - chrono::Duration::days(30)).to_rfc3339())
        .execute(pool).await?;

    sqlx::query("INSERT INTO users (id, username, email, age, status, created_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(uuid::Uuid::new_v4().to_string())
        .bind("bob_ghost")
        .bind("bob@old.com")
        .bind(22i32)
        .bind("inactive")
        .bind((now - chrono::Duration::days(90)).to_rfc3339())
        .execute(pool).await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- Database setup ---
    let pool = SqlitePool::connect("sqlite::memory:").await?;
    setup_schema(&pool).await?;
    insert_test_data(&pool).await?;
    // ─── QueryRepository: read-only, no insert/update/delete ────────
    // Because #[derive(Entity)] also generates RowExtractable for User,
    // we can create a QueryRepository directly from the same pool:
    let query_repo = QueryRepository::<Sqlite, User>::new(pool.clone());

    // find_all returns every row in the table (no WHERE, no pagination)
    let all: Vec<User> = query_repo.find_all("users").await?;
    println!("[QueryRepository] all rows: {}", all.len());

    // find_by_id reads one row by primary key (UUID)
    let first = &all[0];
    let fetched = query_repo.find_by_id("users", SqlValue::Str(first.id.to_string())).await?;
    println!("[QueryRepository] first user: {:?}", fetched.as_ref().map(|u| &u.username));

    // find_all_by_sql executes arbitrary SELECTs and maps rows → User via RowExtractable
    let adults: Vec<User> = query_repo.find_all_by_sql(
        "SELECT * FROM users WHERE age > ?1",
        &[SqlValue::I32(25)],
    ).await?;
    println!("[QueryRepository] adults: {}", adults.len());

    // execute_sql handles non-SELECT statements; returns rows-affected
    let rows_affected: u64 = query_repo.execute_sql(
        "UPDATE users SET status = ?1 WHERE status = ?2",
        &[SqlValue::Str("archived".into()), SqlValue::Str("inactive".into())],
    ).await?;
    println!("[QueryRepository] rows updated: {}", rows_affected);

    let repo = QueryRepository::<Sqlite, User>::new(pool);

    // ============================================================
    // NON-CRUD QUERY PATTERNS (QueryRepository is read-only)
    // ============================================================

    // find_one_by_sql / find_all_by_sql  — raw SQL → RowExtractable mapping
    let adults: Vec<User> = repo.find_all_by_sql(
        "SELECT * FROM users WHERE age > ?1",
        &[SqlValue::I32(21)],
    ).await?;
    println!("Adults (age > 21): {}", adults.len());

    // execute_sql  — non-SELECT statements (returns rows-affected)
    let rows_affected: u64 = repo.execute_sql(
        "UPDATE users SET status = ?1 WHERE age < ?2",
        &[SqlValue::Str("senior".into()), SqlValue::I32(30)],
    ).await?;
    println!("Rows updated: {}", rows_affected);

    // verify
    let all_after_update: Vec<User> = repo.find_all("users").await?;
    for u in all_after_update {
        println!("  {} age={} status={}", u.username, u.age, u.status);
    }

    Ok(())
}
