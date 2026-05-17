# RustData

Spring Data-style generic CRUD and query repositories for [sqlx](https://github.com/launchbadge/sqlx), with a multi-dialect SQL transpilation engine.

Write your entity struct and your migration SQL. rustdata generates the repository, typed finder methods, parameter binding, and row extraction — across PostgreSQL, SQLite, and MySQL — from those two inputs alone.

## Crates

| Crate | Description |
|---|---|
| [`rustdata-core`](crates/rustdata-core) | Generic CRUD repository, derive macros, dialect engine, soft-delete, pagination, specification pattern, and `QueryRepository`. |
| [`rustdata-macros`](crates/rustdata-macros) | Proc-macro crate powering `#[derive(Entity)]`, `#[derive(QueryMethods)]`, `#[derive(Projection)]`, and `#[derive(SqlType)]`. Re-exported by `rustdata-core` — never add it directly. |
| [`rustdata-migrations`](crates/rustdata-migrations) | Compile-time SQL transpiler and `migrate!` macro: embeds migration files into the binary, detects the dialect from the pool type, and applies pending migrations at startup. |

## Quick start

### 1. Add dependencies

```toml
[dependencies]
rustdata-core       = { version = "0.1", features = ["sqlite"] }  # or "postgres" / "mysql"
rustdata-migrations = { version = "0.1", features = ["sqlite"] }
sqlx  = { version = "0.8", features = ["runtime-tokio", "sqlite", "uuid", "chrono"] }
tokio = { version = "1",   features = ["full"] }
uuid  = { version = "1",   features = ["v4"] }
```

Activate **exactly one** backend feature (`sqlite`, `postgres`, or `mysql`). The same feature flag must be set on both `rustdata-core` and `rustdata-migrations`.

### 2. Write your migration SQL

Place canonical SQL files under `migrations/` using a numeric version prefix:

```
migrations/
  v1__create_users.sql
  v2__add_user_fields.sql
```

Write standard Postgres-style DDL — the transpiler converts types and syntax for the target dialect automatically:

```sql
-- migrations/v1__create_users.sql
CREATE TABLE users (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    username   VARCHAR     NOT NULL,
    email      VARCHAR     NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    active     BOOLEAN     NOT NULL DEFAULT TRUE
);
```

### 3. Define your entity

```rust
use rustdata_core::prelude::*;

#[derive(Debug, Clone, Entity, QueryMethods)]
#[entity(table = "users", order_by = "created_at DESC")]
pub struct User {
    #[entity(id)]
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub active: bool,
    pub bio: Option<String>,

    #[entity(auto_generated)]   // excluded from INSERT/UPDATE — DB manages it
    pub updated_at: DateTime<Utc>,
}
```

`#[derive(Entity)]` generates:
- An `EntityDescriptor` impl with column metadata, bind/extract logic, and SQL fragments.
- A `UserRepo` type alias pinned to the active backend (no generics at the call site).
- A `RowExtractable` impl for use with `QueryRepository`.

`#[derive(QueryMethods)]` generates typed `find_by_*`, `exists_by_*`, `count_by_*`, and `delete_by_*` methods for every field.

### 4. Apply migrations and use the repository

```rust
use rustdata_core::prelude::*;
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await?;

    // Embed migrations at compile time, detect dialect from pool type,
    // apply any pending versions. One line.
    rustdata_migrations::migrate!(&pool).await?;

    let repo = UserRepo::new(pool);

    // ── CRUD ─────────────────────────────────────────────────────────────────
    let user = repo.insert(user).await?;
    let found = repo.find_by_id(user.id).await?;          // Option<User>
    let updated = repo.update(user).await?;
    repo.delete(&user.id).await?;

    // ── Generated typed finders ───────────────────────────────────────────────
    let actives   = repo.find_by_active(true).await?;
    let free_tier = repo.find_by_is_premium(false).await?;
    let page      = repo.find_by_active_paged(true, &Pageable::new(0, 20)).await?;
    let exists    = repo.exists_by_email("alice@example.com").await?;
    let n         = repo.count_by_active(true).await?;
    repo.delete_by_active(false).await?;

    // ── Compound finders (both field orderings generated automatically) ───────
    let result = repo.find_by_active_and_is_premium(true, false).await?;

    // ── Pagination ────────────────────────────────────────────────────────────
    let page: Page<User> = repo.list(&Pageable::new(0, 10)).await?;
    println!("{}/{} pages", page.page + 1, page.total_pages);

    Ok(())
}
```

## Repository API

### `CrudRepository` — full CRUD

| Method | Description |
|---|---|
| `insert(entity)` | Insert a new row; returns the inserted entity. |
| `update(entity)` | Update all non-id, non-insert-only columns; returns the updated entity. |
| `find_by_id(id)` | Fetch by primary key; returns `Option<Entity>`. |
| `find_all()` | Fetch all rows ordered by `ORDER_BY`. |
| `find_all_sorted(order)` | Fetch all rows with a custom sort. |
| `list(pageable)` | Paginated fetch; returns `Page<Entity>`. |
| `exists_by_id(id)` | Boolean existence check. |
| `count()` | Total row count. |
| `delete(id)` | Delete by primary key; returns `true` if a row was removed. |
| `hard_delete(id)` | Bypass soft-delete and physically remove the row. |
| `clear()` | Delete all rows. |
| `insert_batch(entities)` | Bulk insert. |
| `find_all_pred(pred)` | Fetch rows matching a `Predicate`. |
| `find_all_pred_paged(pred, pageable)` | Paginated predicate query. |
| `find_one_pred(pred)` | Fetch the first matching row. |
| `count_pred(pred)` | Count matching rows. |
| `delete_pred(pred)` | Delete matching rows. |
| `find_one_spec(spec)` | Fetch via a `Specification`. |
| `find_all_spec(spec)` | Fetch all via a `Specification`. |
| `find_one_by_sql(sql, values)` | Ad-hoc SQL, single result. |
| `find_many_by_sql(sql, values)` | Ad-hoc SQL, multiple results. |
| `execute_sql(sql, values)` | Ad-hoc SQL, no result (DDL / mutations). |

### `QueryRepository` — read-only / ad-hoc SQL

```rust
use rustdata_core::{QueryRepository, backends::Sqlite, SqlValue};

let qrepo = QueryRepository::<Sqlite, User>::new(pool);

let adults: Vec<User> = qrepo
    .find_all_by_sql("SELECT * FROM users WHERE age > ?", &[SqlValue::I32(21)])
    .await?;
```

## Generated finder methods

`#[derive(QueryMethods)]` inspects every non-id, non-auto-generated field and emits finders for it. The supported operator suffixes are:

| Suffix | SQL | Example |
|---|---|---|
| *(none)* / `_eq` | `= ?` | `find_by_active(true)` |
| `_ne` | `<> ?` | `find_by_status_ne("inactive")` |
| `_lt` / `_lte` | `< ?` / `<= ?` | `find_by_age_lt(18)` |
| `_gt` / `_gte` | `> ?` / `>= ?` | `find_by_age_gt(21)` |
| `_like` | `LIKE ?` | `find_by_email_like("%@corp.com")` |
| `_in` | `IN (…)` | `find_by_status_in(vec!["a","b"])` |
| `_is_null` | `IS NULL` | `find_by_bio_is_null()` |
| `_is_not_null` | `IS NOT NULL` | `find_by_bio_is_not_null()` |

Each field gets six method families: `find_by_*`, `find_by_*_paged`, `find_one_by_*`, `exists_by_*`, `count_by_*`, `delete_by_*`. Compound `_and_` / `_or_` finders are also generated for every pair of fields, in both field orderings.

## Field attributes

```rust
#[derive(Entity)]
#[entity(table = "posts", order_by = "published_at DESC")]
pub struct Post {
    #[entity(id)]
    pub id: Uuid,

    #[entity(column = "body_text")]   // map to a different column name
    pub body: String,

    #[entity(insert_only)]            // written on INSERT, never on UPDATE
    pub author_id: Uuid,

    #[entity(auto_generated)]         // excluded from INSERT and UPDATE
    pub created_at: DateTime<Utc>,

    #[entity(json)]                   // serialized to/from JSON in the DB
    pub metadata: serde_json::Value,

    #[entity(skip)]                   // not mapped to any column
    pub computed_field: String,
}
```

`#[entity(hooks = "MyHooks")]` lets you supply a custom `LifecycleHooks` impl for `before_save` / `after_save` callbacks.

## Soft delete

```rust
#[derive(Entity)]
#[entity(table = "users", soft_delete = "deleted_at")]
pub struct User { … }
```

With `soft_delete` set, `repo.delete(&id)` sets `deleted_at = now()` instead of removing the row. Use `repo.hard_delete(&id)` to physically remove it.

## Specification pattern

```rust
use rustdata_core::specification::{Predicate, SqlValue};

let spec = Predicate::And(vec![
    Predicate::Eq { column: "active".into(), value: SqlValue::Bool(true) },
    Predicate::Gt { column: "age".into(),    value: SqlValue::I32(18)    },
]);

let users = repo.find_all_pred(spec).await?;
```

## Migrations

### Naming convention

Files must start with a numeric version prefix. All of these are valid:

```
v1__create_users.sql
V2__add_fields.sql
001_init.sql
20240101_schema.sql
```

Files are embedded at compile time via `include_str!`, sorted by their numeric prefix, and applied in ascending order. Applied versions are tracked in a `schema_migrations` table.

### Dialect-specific blocks

Wrap dialect-specific DDL in annotation comments when one statement can't be transpiled automatically:

```sql
-- @dialect sqlite_only
CREATE TABLE users (id TEXT PRIMARY KEY);
-- @end_dialect

-- @dialect postgres_only
CREATE TABLE users (id UUID PRIMARY KEY DEFAULT gen_random_uuid());
-- @end_dialect
```

### Custom migration path

```rust
rustdata_migrations::migrate!(&pool, "db/migrations").await?;
```

### Type mappings

The transpiler handles the most common cross-dialect differences automatically:

| Canonical (Postgres) | SQLite | MySQL |
|---|---|---|
| `UUID` | `TEXT` | `CHAR(36)` |
| `TIMESTAMPTZ` | `TEXT` | `DATETIME` |
| `BOOLEAN` | `INTEGER` | `TINYINT(1)` |
| `VARCHAR` | `TEXT` | `VARCHAR` |
| `BIGSERIAL` | `INTEGER AUTOINCREMENT` | `BIGINT AUTO_INCREMENT` |
| `DEFAULT gen_random_uuid()` | `DEFAULT (lower(hex(randomblob(16))))` | *(removed)* |
| `DEFAULT now()` | `DEFAULT (datetime('now'))` | `DEFAULT CURRENT_TIMESTAMP` |

## Error handling

All repository methods return `Result<_, RepositoryError>`. Errors are fully typed and match across all three dialects:

```rust
match repo.insert(user).await {
    Ok(u)  => println!("inserted {}", u.id),
    Err(RepositoryError::UniqueViolation { constraint, .. }) =>
        println!("duplicate on {constraint}"),
    Err(RepositoryError::ForeignKeyViolation { detail }) =>
        println!("FK failed: {detail}"),
    Err(e) => return Err(e.into()),
}
```

## Features

| Feature | Crate | Enables |
|---|---|---|
| `sqlite` | `rustdata-core`, `rustdata-migrations` | SQLite backend via `sqlx/sqlite` |
| `postgres` | `rustdata-core`, `rustdata-migrations` | PostgreSQL backend via `sqlx/postgres` |
| `mysql` | `rustdata-core`, `rustdata-migrations` | MySQL backend via `sqlx/mysql` |

Activate exactly one backend feature. Activating multiple is supported for library crates that need to remain backend-agnostic, but application crates should pin to one.

## Requirements

- Rust 2021 edition — MSRV **1.75**
- Async-only (tokio runtime)
- MIT licensed