//! # Typed query methods
//!
//! `#[derive(QueryMethods)]` generates trait-based `async fn` methods on
//! `CrudRepository<BA, E>` and `QueryRepository<BA, E>`, so you can call:
//!
//!   repo.find_by_age_gt(21)            instead of  repo.find_by("find_by_age_gt", vec![…])
//!   repo.find_by_status_ne("active")               (method name + arg types checked at compile time)
//!
//! The traits must be in scope — bring them in with `use` after the derive.

use sqlx::sqlite::SqlitePool;
use rustdata_core::{
    CrudRepository, QueryRepository, backends::Sqlite, Entity, QueryMethods,
};

// ─── Entity ────────────────────────────────────────────────
// `#[derive(Entity)]`       → EntityDescriptor + RowExtractable
// `#[derive(QueryMethods)]` → UserCrudQueryMethods + UserQueryQueryMethods traits

#[derive(Debug, Clone, Entity, QueryMethods)]
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

// ─── Schema ────────────────────────────────────────────────

async fn setup_schema(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
            username TEXT NOT NULL UNIQUE,
            email TEXT NOT NULL UNIQUE,
            age INTEGER NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_test_data(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Utc::now();

    for (username, email, age, status) in [
        ("john_admin",  "john@example.com",  35, "active"),
        ("jane_user",   "jane@example.com",  28, "active"),
        ("bob_ghost",   "bob@old.com",        22, "inactive"),
        ("alice_dev",   "alice@dev.com",      30, "active"),
        ("eve_reader",  "eve@books.com",      17, "inactive"),
    ] {
        sqlx::query(
            "INSERT INTO users (id, username, email, age, status, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(username)
        .bind(email)
        .bind(age)
        .bind(status)
        .bind(now.to_rfc3339())
        .execute(pool)
        .await?;
    }
    Ok(())
}

// ─── Main ──────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = SqlitePool::connect("sqlite::memory:").await?;
    setup_schema(&pool).await?;
    insert_test_data(&pool).await?;

    // ════════════════════════════════════════════════════════
    // CrudRepository — table is known at compile time via
    // EntityDescriptor::TABLE, so no table name is needed.
    // ════════════════════════════════════════════════════════
    let crud = CrudRepository::<Sqlite, User>::new(pool.clone());

    println!("=== CrudRepository ===");

    // find_by_*  — matches WHERE col OP ?
    let adults = crud.find_by_age_gt(21).await?;
    println!("age > 21 : {} users  {:?}",
             adults.len(),
             adults.iter().map(|u| &u.username).collect::<Vec<_>>());

    let thirty_plus = crud.find_by_age_gte(30).await?;
    println!("age >= 30: {} users  {:?}",
             thirty_plus.len(),
             thirty_plus.iter().map(|u| &u.username).collect::<Vec<_>>());

    let minors = crud.find_by_age_lt(18).await?;
    println!("age < 18 : {} users  {:?}",
             minors.len(),
             minors.iter().map(|u| &u.username).collect::<Vec<_>>());

    let not_active = crud.find_by_status_ne("active").await?;
    println!("status != 'active': {} users  {:?}",
             not_active.len(),
             not_active.iter().map(|u| &u.username).collect::<Vec<_>>());

    let j_users = crud.find_by_username_like("j%").await?;
    println!("username LIKE 'j%': {} users  {:?}",
             j_users.len(),
             j_users.iter().map(|u| &u.username).collect::<Vec<_>>());

    // find_one_by_*  — returns Option<User>
    let maybe = crud.find_one_by_email("jane@example.com").await?;
    println!("find_one_by_email: {:?}", maybe.as_ref().map(|u| &u.username));

    // _and_ / _or_ compound queries (equality on both sides)
    let active_at_30 = crud
        .find_by_status_and_age("active", 30)
        .await?;
    println!("status='active' AND age=30: {} users  {:?}",
             active_at_30.len(),
             active_at_30.iter().map(|u| &u.username).collect::<Vec<_>>());

    let named_or_young = crud
        .find_by_username_or_age("bob_ghost", 17)
        .await?;
    println!("username='bob_ghost' OR age=17: {} users  {:?}",
             named_or_young.len(),
             named_or_young.iter().map(|u| &u.username).collect::<Vec<_>>());

    // ════════════════════════════════════════════════════════
    // QueryRepository — read-only, table name is a runtime arg.
    // Useful when the same Row type is reused across multiple
    // views/tables, or when EntityDescriptor is not available.
    // ════════════════════════════════════════════════════════
    let query = QueryRepository::<Sqlite, User>::new(pool);

    println!("\n=== QueryRepository ===");

    let adults_q = query.find_by_age_gt("users", 21).await?;
    println!("age > 21 : {} users  {:?}",
             adults_q.len(),
             adults_q.iter().map(|u| &u.username).collect::<Vec<_>>());

    let maybe_q = query.find_one_by_email("users", "jane@example.com").await?;
    println!("find_one_by_email: {:?}", maybe_q.as_ref().map(|u| &u.username));

    let active_at_30_q = query
        .find_by_status_and_age("users", "active", 30)
        .await?;
    println!("status='active' AND age=30: {} users  {:?}",
             active_at_30_q.len(),
             active_at_30_q.iter().map(|u| &u.username).collect::<Vec<_>>());

    Ok(())
}
