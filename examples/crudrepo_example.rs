//! # CrudRepository example
//!
//! Demonstrates the full lifecycle of a rustdata entity:
//! migrations → insert → find → update → delete.
//!
//! ## What you write as a developer
//!
//!  1. SQL files in `migrations/`          — canonical, dialect-agnostic
//!  2. `#[derive(Entity)]` on your struct  — generates the repo type alias
//!  3. `migrate!(&pool)`                   — one call, everything applied
//!  4. `UserRepo::new(pool)`               — no generics, no backend import

use rustdata_core::prelude::*;
use sqlx::sqlite::SqlitePoolOptions;

// ── 1. Entity definition ─────────────────────────────────────────────────────
//
// `#[derive(Entity)]` generates:
//   • EntityDescriptor  — column metadata + bind/extract logic
//   • UserRepo          — a concrete type alias pinned to the active backend
//                         (Sqlite here, because `features = ["sqlite"]`)
//
// No `CrudRepository<Sqlite, User>` needed anywhere.

#[derive(Debug, Clone, Entity)]
#[entity(table = "users", order_by = "created_at DESC")]
struct User {
    #[entity(id)]
    id: uuid::Uuid,

    username: String,

    email: String,

    #[entity(auto_generated)]   // excluded from INSERT / UPDATE — the DB sets it
    created_at: chrono::DateTime<chrono::Utc>,
}

// ── 2. Migrations live in `examples/migrations/` as plain SQL files ──────────
//
// examples/migrations/v1__create_users.sql:
//
//   CREATE TABLE users (
//       id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
//       username    VARCHAR(255) NOT NULL UNIQUE,
//       email       VARCHAR(255) NOT NULL UNIQUE,
//       created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW()
//   );
//
// The framework transpiles `UUID` → `TEXT`, `TIMESTAMPTZ` → `TEXT`,
// `gen_random_uuid()` → SQLite-compatible form, etc. automatically.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── 3. Connect ────────────────────────────────────────────────────────────
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite::memory:")
        .await?;

    // ── 4. Migrate — dialect inferred from pool type, no arguments needed ─────
    rustdata_migrations::migrate!(&pool, "examples/migrations").await?;
    println!("✓ Migrations applied");

    // ── 5. Create the repository — just the generated alias, no generics ──────
    let repo = UserRepo::new(pool);

    // ── Insert ────────────────────────────────────────────────────────────────
    let user = User {
        id:         uuid::Uuid::new_v4(),
        username:   "john_doe".into(),
        email:      "john@example.com".into(),
        created_at: chrono::Utc::now(),
    };
    repo.save(&user).await?;
    println!("✓ Inserted: {} <{}>", user.username, user.email);

    // ── Find by id ────────────────────────────────────────────────────────────
    if let Some(found) = repo.find_by_id(&user.id).await? {
        println!("✓ Found:    {} <{}>", found.username, found.email);
    }

    // ── Update ────────────────────────────────────────────────────────────────
    let mut updated = user.clone();
    updated.email = "john.doe@example.com".into();
    repo.save(&updated).await?;
    println!("✓ Updated email → {}", updated.email);

    // ── Find all ──────────────────────────────────────────────────────────────
    let all = repo.find_all(rustdata_core::pagination::Pageable::default()).await?;
    println!("✓ find_all: {} user(s)", all.content.len());

    // ── Delete ────────────────────────────────────────────────────────────────
    repo.delete_by_id(&user.id).await?;
    let gone = repo.find_by_id(&user.id).await?;
    assert!(gone.is_none(), "user should be deleted");
    println!("✓ Deleted — find_by_id returns None");

    println!("\n✅  Done.");
    Ok(())
}
