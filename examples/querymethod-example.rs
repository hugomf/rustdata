//! # QueryMethods derive — full showcase
//!
//! `#[derive(QueryMethods)]` generates typed async finder methods on the
//! generated `UserRepo` type alias. No raw SQL, no manual query building.
//!
//! Generated methods include:
//!   find_by_status(value)
//!   find_by_age_gt(value)
//!   find_by_email_is_null()
//!   find_by_status_in(vec![...])
//!   find_by_status_and_age_gt(s, n)
//!   find_by_age_gt_paged(n, &pageable)
//!   count_by_status(value)
//!   exists_by_age_lte(value)
//!   delete_by_status(value)

use rustdata_core::prelude::*;
use sqlx::sqlite::SqlitePool;

// ── Entity ────────────────────────────────────────────────────────────────────
//
// Both `Entity` and `QueryMethods` are needed:
//   Entity       → EntityDescriptor, UserRepo type alias, RowExtractable
//   QueryMethods → all the find_by_* / count_by_* / exists_by_* / delete_by_* methods

#[derive(Debug, Clone, Entity, QueryMethods)]
#[entity(table = "users", order_by = "created_at DESC")]
struct User {
    #[entity(id)]
    id: uuid::Uuid,

    username: String,

    // Option<String> → _is_null / _is_not_null finders are generated
    email: Option<String>,

    age: i32,

    status: String,

    // auto_generated → excluded from QueryMethods (no find_by_created_at)
    #[entity(auto_generated)]
    created_at: chrono::DateTime<chrono::Utc>,
}

// Bring generated query-method traits into scope.
// These trait names are generated as `{Struct}CrudQueryMethods` and
// `{Struct}QueryQueryMethods` — the wildcard import is the easiest approach.
use UserCrudQueryMethods as _;
use UserQueryQueryMethods as _;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn names(users: &[User]) -> Vec<&str> {
    users.iter().map(|u| u.username.as_str()).collect()
}

async fn seed(repo: &UserRepo) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Utc::now();
    let rows: &[(&str, Option<&str>, i32, &str)] = &[
        ("john_admin", Some("john@example.com"), 35, "active"),
        ("jane_user",  Some("jane@example.com"), 28, "active"),
        ("bob_ghost",  None,                      22, "inactive"),
        ("alice_dev",  Some("alice@dev.com"),     30, "active"),
        ("eve_reader", Some("eve@books.com"),     17, "inactive"),
        ("zara_pend",  Some("zara@new.com"),      25, "pending"),
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

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect + migrate — one call each
    let pool = SqlitePool::connect("sqlite::memory:").await?;
    rustdata_migrations::migrate!(&pool, "examples/migrations").await?;

    // The generated UserRepo alias — no `CrudRepository<Sqlite, User>` needed
    let repo = UserRepo::new(pool.clone());
    seed(&repo).await?;

    println!("══════════════════════════════════════════");
    println!(" QueryMethods — typed finder methods demo");
    println!("══════════════════════════════════════════\n");

    // ── Basic comparisons ─────────────────────────────────────────────────────

    let adults = repo.find_by_age_gt(21).await?;
    println!("age > 21          → {:?}", names(&adults));

    let not_active = repo.find_by_status_ne("active").await?;
    println!("status != active  → {:?}", names(&not_active));

    let j_users = repo.find_by_username_like("j%").await?;
    println!("username LIKE j%  → {:?}", names(&j_users));

    // ── IN clause ─────────────────────────────────────────────────────────────

    let multi_status = repo.find_by_status_in(vec!["active", "pending"]).await?;
    println!("\nstatus IN (active, pending) → {:?}", names(&multi_status));

    let specific_ages = repo.find_by_age_in(vec![17i32, 28, 35]).await?;
    println!("age IN (17, 28, 35)         → {:?}", names(&specific_ages));

    // ── IS NULL / IS NOT NULL ─────────────────────────────────────────────────

    let no_email = repo.find_by_email_is_null().await?;
    println!("\nemail IS NULL     → {:?}", names(&no_email));

    let has_email = repo.find_by_email_is_not_null().await?;
    println!("email IS NOT NULL → {:?}", names(&has_email));

    // ── count_by / exists_by ──────────────────────────────────────────────────

    let active_count = repo.count_by_status("active").await?;
    println!("\ncount active      → {active_count}");

    let any_minors = repo.exists_by_age_lte(18).await?;
    println!("exists age <= 18  → {any_minors}");

    // ── delete_by ─────────────────────────────────────────────────────────────

    let deleted = repo.delete_by_status("inactive").await?;
    println!("\ndeleted inactive  → {deleted} row(s)");

    let remaining = repo.find_by_age_gt(0).await?;
    println!("remaining users   → {:?}", names(&remaining));

    // ── Compound conditions ───────────────────────────────────────────────────

    let active_adults = repo.find_by_status_and_age_gt("active", 21).await?;
    println!("\nstatus=active AND age > 21 → {:?}", names(&active_adults));

    let active_under_30 = repo.find_by_status_and_age_lt("active", 30).await?;
    println!("status=active AND age < 30 → {:?}", names(&active_under_30));

    // ── Paginated ─────────────────────────────────────────────────────────────

    let page = repo.find_by_age_gt_paged(21, &Pageable::new(0, 2)).await?;
    println!(
        "\nage > 21 (page 1/{}, size 2) → {:?}",
        page.total_pages,
        names(&page.content)
    );

    let page2 = repo.find_by_status_paged("active", &Pageable::new(1, 2)).await?;
    println!(
        "status=active (page 2/{}, size 2) → {:?}",
        page2.total_pages,
        names(&page2.content)
    );

    // ── QueryRepository (same generated methods, read-only) ───────────────────

    println!("\n══════════════════════════════════════════");
    println!(" QueryRepository — same query methods");
    println!("══════════════════════════════════════════\n");

    let qrepo = QueryRepository::<rustdata_core::backends::Sqlite, User>::new(pool);

    let multi_q = qrepo.find_by_status_in("users", vec!["active", "pending"]).await?;
    println!("status IN (active, pending) → {:?}", names(&multi_q));

    let no_email_q = qrepo.find_by_email_is_null("users").await?;
    println!("email IS NULL               → {:?}", names(&no_email_q));

    let compound_q = qrepo.find_by_status_and_age_gte("users", "active", 28).await?;
    println!("status=active AND age >= 28 → {:?}", names(&compound_q));

    println!("\n✅  Done.");
    Ok(())
}
