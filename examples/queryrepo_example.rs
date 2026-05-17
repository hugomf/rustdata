//! # QueryRepository example
//!
//! Demonstrates read-only queries against an existing schema.
//! `QueryRepository` has no insert/update/delete — it is ideal for views,
//! aggregations, projections, and read-only service layers.
//!
//! ## What you write as a developer
//!
//!  1. SQL files in `migrations/`               — canonical SQL
//!  2. `#[derive(Entity)]` on your read struct  — generates `UserRepo` (CRUD)
//!                                                 and `RowExtractable` (queries)
//!  3. `migrate!(&pool)`                         — one call
//!  4. `QueryRepository::new(pool)` for ad-hoc SQL queries

use rustdata_core::prelude::*;
use sqlx::sqlite::SqlitePool;

// ── Entity definition ─────────────────────────────────────────────────────────
//
// `#[derive(Entity)]` also generates `RowExtractable` for the struct, which is
// the only requirement for using it with `QueryRepository`.

#[derive(Debug, Clone, Entity)]
#[entity(table = "users", order_by = "created_at DESC")]
struct User {
    #[entity(id)]
    id: uuid::Uuid,

    username: String,

    email: Option<String>,

    age: i32,

    status: String,

    #[entity(auto_generated)]
    created_at: chrono::DateTime<chrono::Utc>,
}

// ── Helper: seed test data using the CRUD repo ────────────────────────────────

async fn seed(repo: &UserRepo) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Utc::now();
    let rows: &[(&str, Option<&str>, i32, &str)] = &[
        ("john_admin", Some("john@example.com"), 35, "active"),
        ("jane_user",  Some("jane@example.com"), 28, "active"),
        ("bob_ghost",  None,                      22, "inactive"),
        ("alice_dev",  Some("alice@dev.com"),     30, "active"),
        ("eve_reader", Some("eve@books.com"),     17, "inactive"),
    ];
    for (username, email, age, status) in rows {
        repo.save(&User {
            id:         uuid::Uuid::new_v4(),
            username:   username.to_string(),
            email:      email.map(str::to_string),
            age:        *age,
            status:     status.to_string(),
            created_at: now,
        }).await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Connect + migrate ─────────────────────────────────────────────────────
    let pool = SqlitePool::connect("sqlite::memory:").await?;
    rustdata_migrations::migrate!(&pool, "examples/migrations").await?;
    println!("✓ Migrations applied");

    // ── Seed via the generated CRUD repo ──────────────────────────────────────
    let crud_repo = UserRepo::new(pool.clone());
    seed(&crud_repo).await?;

    // ── QueryRepository for read-only / ad-hoc queries ────────────────────────
    //
    // `QueryRepository` uses the same `User` struct (which implements
    // `RowExtractable` via `#[derive(Entity)]`) but exposes only query methods.
    let repo = QueryRepository::<rustdata_core::backends::Sqlite, User>::new(pool);

    // find_all_by_sql — execute any SELECT and map rows to User
    let adults: Vec<User> = repo
        .find_all_by_sql("SELECT * FROM users WHERE age > ?", &[SqlValue::I64(21)])
        .await?;
    println!("✓ Adults (age > 21): {}", adults.len());

    // find_one_by_sql — returns Option<User>
    let first = repo
        .find_one_by_sql(
            "SELECT * FROM users WHERE status = ? ORDER BY created_at DESC LIMIT 1",
            &[SqlValue::Str("active".into())],
        )
        .await?;
    println!("✓ Latest active user: {:?}", first.as_ref().map(|u| &u.username));

    // execute_sql — non-SELECT (UPDATE/DELETE), returns rows_affected
    let archived = repo
        .execute_sql(
            "UPDATE users SET status = ? WHERE status = ?",
            &[SqlValue::Str("archived".into()), SqlValue::Str("inactive".into())],
        )
        .await?;
    println!("✓ Archived {} inactive user(s)", archived);

    // Verify the update
    let inactive_after: Vec<User> = repo
        .find_all_by_sql("SELECT * FROM users WHERE status = ?", &[SqlValue::Str("inactive".into())])
        .await?;
    assert!(inactive_after.is_empty(), "all inactive should now be archived");
    println!("✓ No inactive users remain");

    println!("\n✅  Done.");
    Ok(())
}
