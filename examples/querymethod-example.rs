//! # Typed query methods — full feature showcase
//!
//! `#[derive(QueryMethods)]` generates trait-based async methods on
//! `CrudRepository<BA, E>` and `QueryRepository<BA, E>`.
//!
//! New in this version:
//!   find_by_status_in(vec!["active", "pending"])
//!   find_by_email_is_null() / find_by_email_is_not_null()
//!   count_by_status("active")
//!   exists_by_age_gt(18)
//!   delete_by_status("inactive")
//!   find_by_status_and_age_gt("active", 21)   ← compound with operator
//!   find_by_age_gt_paged(21, &pageable)        ← paginated

use rustdata_core::{
    backends::Sqlite,
    pagination::Pageable,
    CrudRepository, Entity, QueryMethods, QueryRepository,
};
use sqlx::sqlite::SqlitePool;

// ─── Entity ────────────────────────────────────────────────

#[derive(Debug, Clone, Entity, QueryMethods)]
#[entity(table = "users", order_by = "created_at DESC")]
struct User {
    #[entity(id)]
    id: uuid::Uuid,

    #[entity(column = "username")]
    username: String,

    // nullable — so _is_null / _is_not_null are useful here
    #[entity(column = "email")]
    email: Option<String>,

    #[entity(column = "age")]
    age: i32,

    #[entity(column = "status")]
    status: String,

    // auto_generated — excluded from QueryMethods so no find_by_created_at is generated
    #[entity(auto_generated)]
    created_at: chrono::DateTime<chrono::Utc>,
}

// Bring the generated traits into scope.
use UserCrudQueryMethods as _;
use UserQueryQueryMethods as _;

// ─── Schema ────────────────────────────────────────────────

async fn setup_schema(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id          TEXT PRIMARY KEY,
            username    TEXT NOT NULL UNIQUE,
            email       TEXT,
            age         INTEGER NOT NULL,
            status      TEXT NOT NULL,
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_test_data(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Utc::now().to_rfc3339();
    let rows: &[(&str, Option<&str>, i32, &str)] = &[
        ("john_admin",  Some("john@example.com"),  35, "active"),
        ("jane_user",   Some("jane@example.com"),  28, "active"),
        ("bob_ghost",   None,                       22, "inactive"),
        ("alice_dev",   Some("alice@dev.com"),      30, "active"),
        ("eve_reader",  Some("eve@books.com"),      17, "inactive"),
        ("zara_pend",   Some("zara@new.com"),       25, "pending"),
    ];
    for (username, email, age, status) in rows {
        sqlx::query(
            "INSERT INTO users (id, username, email, age, status, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(username)
        .bind(*email)
        .bind(age)
        .bind(status)
        .bind(&now)
        .execute(pool)
        .await?;
    }
    Ok(())
}

fn names(users: &[User]) -> Vec<&str> {
    users.iter().map(|u| u.username.as_str()).collect()
}

// ─── Main ──────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = SqlitePool::connect("sqlite::memory:").await?;
    setup_schema(&pool).await?;
    insert_test_data(&pool).await?;

    let crud = CrudRepository::<Sqlite, User>::new(pool.clone());

    println!("══════════════════════════════════════════");
    println!(" CrudRepository — typed QueryMethods demo");
    println!("══════════════════════════════════════════\n");

    // ── Basic comparisons ─────────────────────────────────────────────────────

    let adults = crud.find_by_age_gt(21).await?;
    println!("age > 21          → {:?}", names(&adults));

    let not_active = crud.find_by_status_ne("active").await?;
    println!("status != active  → {:?}", names(&not_active));

    let j_users = crud.find_by_username_like("j%").await?;
    println!("username LIKE j%  → {:?}", names(&j_users));

    // ── IN clause ─────────────────────────────────────────────────────────────

    let multi_status = crud
        .find_by_status_in(vec!["active", "pending"])
        .await?;
    println!("\nstatus IN (active, pending) → {:?}", names(&multi_status));

    let specific_ages = crud
        .find_by_age_in(vec![17i32, 28, 35])
        .await?;
    println!("age IN (17, 28, 35)         → {:?}", names(&specific_ages));

    // ── IS NULL / IS NOT NULL ─────────────────────────────────────────────────

    let no_email = crud.find_by_email_is_null().await?;
    println!("\nemail IS NULL     → {:?}", names(&no_email));

    let has_email = crud.find_by_email_is_not_null().await?;
    println!("email IS NOT NULL → {:?}", names(&has_email));

    let gap_exists = crud.exists_by_email_is_null().await?;
    println!("any email IS NULL → {gap_exists}");

    // ── count_by / exists_by ──────────────────────────────────────────────────

    let active_count = crud.count_by_status("active").await?;
    println!("\ncount active      → {active_count}");

    let any_minors = crud.exists_by_age_lte(18).await?;
    println!("exists age <= 18  → {any_minors}");

    let adults_count = crud.count_by_age_gt(21).await?;
    println!("count age > 21    → {adults_count}");

    // ── delete_by ─────────────────────────────────────────────────────────────

    let deleted = crud.delete_by_status("inactive").await?;
    println!("\ndeleted inactive  → {deleted} rows");

    let remaining = crud.find_by_age_gt(0).await?;
    println!("remaining users   → {:?}", names(&remaining));

    // ── Compound with operators ───────────────────────────────────────────────

    let active_adults = crud
        .find_by_status_and_age_gt("active", 21)
        .await?;
    println!("\nstatus=active AND age > 21 → {:?}", names(&active_adults));

    let active_under_30 = crud
        .find_by_status_and_age_lt("active", 30)
        .await?;
    println!("status=active AND age < 30 → {:?}", names(&active_under_30));

    let active_exact_30 = crud
        .find_by_status_and_age("active", 30)
        .await?;
    println!("status=active AND age = 30 → {:?}", names(&active_exact_30));

    // ── Paginated queries ─────────────────────────────────────────────────────

    let page = crud
        .find_by_age_gt_paged(21, &Pageable::new(0, 2))
        .await?;
    println!(
        "\nage > 21 (page 1 of {}, size 2) → {:?}",
        page.total_pages,
        names(&page.content)
    );

    let page2 = crud
        .find_by_status_paged("active", &Pageable::new(1, 2))
        .await?;
    println!(
        "status=active (page 2 of {}, size 2) → {:?}",
        page2.total_pages,
        names(&page2.content)
    );

    // ── QueryRepository (read-only, table name at runtime) ────────────────────

    println!("\n══════════════════════════════════════════");
    println!(" QueryRepository — same new methods");
    println!("══════════════════════════════════════════\n");

    let qrepo = QueryRepository::<Sqlite, User>::new(pool);

    let multi_q = qrepo
        .find_by_status_in("users", vec!["active", "pending"])
        .await?;
    println!("status IN (active, pending) → {:?}", names(&multi_q));

    let no_email_q = qrepo.find_by_email_is_null("users").await?;
    println!("email IS NULL               → {:?}", names(&no_email_q));

    let compound_q = qrepo
        .find_by_status_and_age_gte("users", "active", 28)
        .await?;
    println!("status=active AND age >= 28 → {:?}", names(&compound_q));

    Ok(())
}
