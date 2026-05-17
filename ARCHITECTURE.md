# Architecture

This document describes the internal design of rustdata: how the crates relate, how data flows through the system at compile time and at runtime, and why each component is built the way it is.

---

## Table of contents

1. [Crate structure](#1-crate-structure)
2. [Dependency graph](#2-dependency-graph)
3. [Compile-time pipeline](#3-compile-time-pipeline)
4. [`rustdata-core`](#4-rustdata-core)
   - [Backend abstraction](#41-backend-abstraction)
   - [`EntityDescriptor` trait](#42-entitydescriptor-trait)
   - [`CrudRepository` method surface](#43-crudrepository-method-surface)
   - [SQL generation and caching](#44-sql-generation-and-caching)
   - [`#[derive(Entity)]`](#45-deriveentity)
   - [`#[derive(QueryMethods)]`](#46-derivequerymethods)
   - [Soft delete](#47-soft-delete)
   - [Lifecycle hooks](#48-lifecycle-hooks)
   - [Pagination](#49-pagination)
   - [Specification pattern](#410-specification-pattern)
   - [Error handling](#411-error-handling)
   - [Transactions](#412-transactions)
5. [`rustdata-migrations`](#5-rustdata-migrations)
   - [Compile-time embedding](#51-compile-time-embedding)
   - [`migrate!` macro](#52-migrate-macro)
   - [Runner](#53-runner)
   - [Transpiler](#54-transpiler)
6. [Data flow: a single `insert`](#6-data-flow-a-single-insert)
7. [Data flow: migration startup](#7-data-flow-migration-startup)
8. [Design decisions](#8-design-decisions)

---

## 1. Crate structure

```
rustdata/
├── crates/
│   ├── rustdata-core/        # Main library — repository, derives, backends, dialect
│   ├── rustdata-macros/      # Proc-macro crate (separate Rust requirement)
│   └── rustdata-migrations/  # SQL transpiler + compile-time migration runner
```

`rustdata-macros` must be a separate crate because Rust requires proc-macro crates to compile for the **build host**, not the target. It is re-exported wholesale by `rustdata-core`, so consumers never add it directly to their `Cargo.toml`.

---

## 2. Dependency graph

```mermaid
graph TD
    App["Application crate\n(e.g. migration-test)"]

    Core["rustdata-core\n#[derive(Entity / QueryMethods)]\nCrudRepository · backends · dialect"]
    Mig["rustdata-migrations\nmigrate! · Transpiler · Runner"]
    Mac["rustdata-macros\nproc-macros: Entity · QueryMethods\nProjection · SqlType · include_migrations!"]
    Sqlx["sqlx\n(sqlite / postgres / mysql)"]

    App -->|"features: sqlite"| Core
    App -->|"features: sqlite"| Mig
    Core --> Mac
    Core --> Sqlx
    Mig  --> Mac
    Mig  --> Sqlx

    style Mac fill:#f5f0e8,stroke:#b8a080
    style Core fill:#e8f0f5,stroke:#6090b0
    style Mig  fill:#e8f5e8,stroke:#60a060
```

> `rustdata-macros` is a build-host dependency. The arrows into it are compile-time only; it is not linked into the final binary.
>
> **Macro reference:** `Entity` and `QueryMethods` generate the repository traits and implementations. `Projection` generates partial structs for selecting a subset of columns. `SqlType` maps custom Rust types to their SQL representations. `include_migrations!` embeds migration files at compile time.

---

## 3. Compile-time pipeline

The entire data layer — repository types, typed finders, bound SQL fragments, and embedded migration SQL — is generated before a single line of application code is executed.

```mermaid
sequenceDiagram
    participant Cargo
    participant Mac  as "rustdata-macros(build host)"
    participant Core as rustdata-core
    participant MigC as rustdata-migrations
    participant App  as "application crate"

    Cargo->>Mac:  compile (proc-macro crate, host target)
    Cargo->>Core: compile to re-exports macros
    Cargo->>MigC: compile to Transpiler, Runner, migrate! macro

    Note over App: Application crate compiles
    App->>Mac:  #[derive(Entity)] to EntityDescriptor impl to UserRepo type alias
    App->>Mac:  #[derive(QueryMethods)] to UserCrudQueryMethods trait + impl
    App->>Mac:  migrate!(&pool) to include_migrations!("migrations") to embeds *.sql as &'static str

    Note over App: Binary links rustdata-migrations runner
    Note over App: Zero filesystem access at runtime
```

---

## 4. `rustdata-core`

### 4.1 Backend abstraction

The `Backend` trait bundles three associated types that together describe a complete database driver. Every generic bound on `CrudRepository` is expressed in terms of this trait, keeping the repository itself dialect-agnostic.

```mermaid
classDiagram
    class Backend {
        <<trait>>
        +type Database : sqlx::Database
        +type Adapter  : BindAdapter
        +type Extractor: RowExtractor
        +dialect() SqlDialect
    }

    class BindAdapter {
        <<trait>>
        Converts Rust values to DB wire params
    }

    class RowExtractor {
        <<trait>>
        Reads typed values from a result row
    }

    class Sqlite { }
    class Postgres { }
    class MySql { }

    Backend <|.. Sqlite
    Backend <|.. Postgres
    Backend <|.. MySql
    Backend --> BindAdapter : Adapter
    Backend --> RowExtractor : Extractor
```

Three concrete backend structs are provided, each gated behind its Cargo feature:

| Struct | `sqlx::Database` | Feature |
|---|---|---|
| `backends::Sqlite` | `sqlx::Sqlite` | `sqlite` |
| `backends::Postgres` | `sqlx::Postgres` | `postgres` |
| `backends::MySql` | `sqlx::MySql` | `mysql` |

**`DefaultBackend`** is a type alias in `rustdata-core` that resolves to the active backend via `cfg(feature)`. The `#[derive(Entity)]` macro references `::rustdata_core::DefaultBackend`, so the generated `UserRepo` alias is always concrete — no generic parameter leaks to the call site.

> **Why not resolve the feature in the macro?** Proc-macro crates compile for the build host and do not inherit the consumer crate's Cargo features. A `cfg(feature = "sqlite")` guard inside `rustdata-macros` is always false. Placing the feature-conditioned alias in `rustdata-core` — where the features are actually declared — ensures correct resolution. See [§8](#8-design-decisions) for the full rationale.

---

### 4.2 `EntityDescriptor` trait

The central trait implemented by `#[derive(Entity)]`. Every repository method is written purely against this trait; no concrete struct types appear in `rustdata-core`.

```mermaid
classDiagram
    class EntityDescriptor {
        <<trait>>
        +type Entity
        +type Id
        +TABLE : &'static str
        +ORDER_BY : &'static str
        +SOFT_DELETE_COL : Option~&str~
        +columns() &[ColumnDef]
        +bind_insert(query, entity) query
        +bind_update(query, entity) query
        +bind_id(query, id) query
        +from_row(row, extractor) Entity
    }

    class CrudRepository {
        +BA : Backend
        +D  : EntityDescriptor
        +pool : Pool
        +insert(entity) Entity
        +update(entity) Entity
        +find_by_id(id) Option~Entity~
        +delete(id) bool
        +list(pageable) Page~Entity~
    }

    class LifecycleHooks {
        <<trait>>
        +before_save(entity)
        +after_save(entity)
    }

    EntityDescriptor --> CrudRepository : D
    LifecycleHooks   --> CrudRepository : D must impl
```

All SQL strings are generated on first call from `D`'s constants and `BA`'s dialect, then cached in a `std::sync::OnceLock<String>`.

---

### 4.3 `CrudRepository` method surface

```
┌─────────────────────────────────────────────────────────────────┐
│ CrudRepository<BA: Backend, D: EntityDescriptor>                │
├──────────────────────┬──────────────────────────────────────────┤
│ Core CRUD            │ insert · update · find_by_id             │
│                      │ delete · hard_delete · exists_by_id      │
├──────────────────────┼──────────────────────────────────────────┤
│ Bulk / list          │ find_all · find_all_sorted · list        │
│                      │ insert_batch · clear · count             │
├──────────────────────┼──────────────────────────────────────────┤
│ Predicate queries    │ find_all_pred · find_all_pred_paged      │
│                      │ find_one_pred · count_pred · delete_pred │
│                      │ count_with_filters                       │
├──────────────────────┼──────────────────────────────────────────┤
│ Specification        │ find_one_spec · find_all_spec            │
│                      │ count_spec · exists_spec                 │
├──────────────────────┼──────────────────────────────────────────┤
│ Ad-hoc SQL           │ find_one_by_sql · find_many_by_sql       │
│                      │ execute_sql                              │
└──────────────────────┴──────────────────────────────────────────┘
```

> **`count_with_filters`** accepts raw SQL `WHERE` fragments and bound parameters, bridging the gap between the strongly-typed Specification pattern and completely ad-hoc SQL.
>
> **`QueryRepository<BA, D>`** is a read-only subset of `CrudRepository` that only exposes `find_*`, `count_*`, and `exists_*` methods. It is useful for injecting into services that should not mutate state.

---

### 4.4 SQL generation and caching

Static SQL strings are assembled once per (`Backend`, `EntityDescriptor`) pair and stored in `OnceLock<String>` statics. Predicate queries build SQL dynamically at call time with a fresh placeholder counter.

```mermaid
flowchart LR
    invoke["repo.find_by_id(id)"]
    lock{{"OnceLock\nhit?"}}
    gen["Assemble SQL from\nD::TABLE, D::id_column,\nBA::dialect().ph(1)"]
    cached["Cached SQL string"]
    bind["D::bind_id(query, id)\nvia BindAdapter"]
    exec["sqlx executes\nagainst pool"]
    extract["D::from_row via\nRowExtractor"]
    result["Option<Entity>"]

    invoke --> lock
    lock -- "miss (first call)" --> gen --> cached
    lock -- hit --> cached
    cached --> bind --> exec --> extract --> result
```

---

### 4.5 `#[derive(Entity)]`

For a struct `User`, the macro emits four items into the user's crate:

```mermaid
flowchart TD
    derive["#[derive(Entity)]\npub struct User { … }"]

    derive --> lh["impl LifecycleHooks<User> for User {}\n(no-op default, or delegates to #[entity(hooks = …)])"]
    derive --> ed["impl EntityDescriptor for User\n- TABLE, ORDER_BY, SOFT_DELETE_COL\n- columns()\n- bind_insert / bind_update / bind_id\n- from_row"]
    derive --> re["impl RowExtractable for User\n(delegates to EntityDescriptor::from_row)"]
    derive --> ta["pub type UserRepo =\n  CrudRepository<DefaultBackend, User>"]
```

---

### 4.6 `#[derive(QueryMethods)]`

Rust's coherence rules forbid adding methods to a foreign type (`CrudRepository` lives in `rustdata-core`) from an outside crate. The macro works around this by generating **local traits** — traits defined in the user's crate after expansion, which may then be implemented for any foreign type.

```mermaid
flowchart TD
    derive["#[derive(QueryMethods)]\npub struct User { active: bool, email: String, … }"]

    derive --> ct["trait UserCrudQueryMethods<BA>\n(local to user's crate)"]
    derive --> qt["trait UserQueryQueryMethods<BA>\n(local to user's crate)"]

    ct --> ci["impl UserCrudQueryMethods<BA>\n  for CrudRepository<BA, User>\n\nfind_by_active(val)\nfind_by_active_paged(val, pageable)\nfind_one_by_active(val)\nexists_by_active(val)\ncount_by_active(val)\ndelete_by_active(val)\n… (x every non-id field)\n… (+ compound _and_ / _or_ pairs,\n     both field orderings)"]

    qt --> qi["impl UserQueryQueryMethods<BA>\n  for QueryRepository<BA, User>\n(read-only variants)"]
```

All generated method bodies delegate to inherent methods on `CrudRepository` (`find_all_pred`, `find_one_pred`, etc.), passing a `Predicate` built from the typed argument.

---

### 4.7 Soft delete

```mermaid
flowchart LR
    del["repo.delete(&id)"]
    check{"SOFT_DELETE_COL\nset?"}
    soft["UPDATE ... SET deleted_at = now()\nWHERE id = ?"]
    phys["DELETE FROM ...\nWHERE id = ?"]
    hard["repo.hard_delete(&id)\nalways physical DELETE"]

    del --> check
    check -- yes --> soft
    check -- no  --> phys
    hard --> phys
```

When `#[entity(soft_delete = "deleted_at")]` is present, `EntityDescriptor::SOFT_DELETE_COL` is `Some("deleted_at")` and `CrudRepository::delete` emits an `UPDATE` instead of a `DELETE`. `hard_delete` always emits the physical `DELETE` regardless.

> **`clear()`** always performs a physical `DELETE FROM` (or `TRUNCATE`), bypassing soft-delete logic entirely. It is intended for test teardown or complete table wipes.

---

### 4.8 Lifecycle hooks

`LifecycleHooks<Entity>` provides two hooks called by every `insert` and `update`. `#[derive(Entity)]` emits a blank no-op impl by default. To customise, set `#[entity(hooks = "MyHooks")]` — the macro then delegates to `MyHooks` instead.

```rust
impl LifecycleHooks<User> for MyHooks {
    fn before_save(entity: &mut User) -> Result<(), RepositoryError> {
        entity.updated_at = Utc::now();
        Ok(())
    }

    fn after_save(entity: &User) -> Result<(), RepositoryError> {
        tracing::info!(id = %entity.id, "entity saved");
        Ok(())
    }
}
```

---

### 4.9 Pagination

`Pageable` carries a zero-based `page` index and a `size`. `Page<E>` wraps the result set with metadata:

```
Page<E> {
    content        : Vec<E>   — the rows for this page
    total_elements : u64      — COUNT(*) of the full result set
    total_pages    : u64      — ceil(total_elements / size)
    page           : u64      — current page index (0-based)
    size           : u64      — requested page size
}
```

Pagination always issues two queries: a `SELECT COUNT(*)` for the total, then `SELECT … LIMIT ? OFFSET ?` for the page content.

---

### 4.10 Specification pattern

`Predicate` is a recursive enum. Combine variants to build arbitrarily complex query conditions without writing SQL.

```mermaid
classDiagram
    class Predicate {
        <<enum>>
        Eq(column, value)
        Ne(column, value)
        Lt / Lte / Gt / Gte
        Like(column, pattern)
        In(column, values)
        IsNull(column)
        IsNotNull(column)
        And(Vec~Predicate~)
        Or(Vec~Predicate~)
        Not(Box~Predicate~)
    }

    class Specification {
        <<trait>>
        +predicate() Predicate
    }

    Specification --> Predicate : produces
    Predicate --> CrudRepository : consumed by find_all_pred\nfind_one_pred · count_pred\ndelete_pred
```

Implement `Specification<User>` on a domain type to encapsulate a business rule as a reusable, composable object.

---

### 4.11 Error handling

`RepositoryError` normalises all database-specific error codes into typed variants so application code is dialect-independent.

```mermaid
flowchart LR
    sqlx["sqlx::Error"]
    conv["From<sqlx::Error>\n\nPG 23505 · MySQL 1062 · SQLite text\n→ UniqueViolation\n\nPG 23503 · MySQL 1452 · SQLite 787 extended\n→ ForeignKeyViolation\n\nPG 57014 → Timeout\nPoolTimedOut → Connection\nPoolClosed → Unavailable\n_ → Database"]
    re["RepositoryError\n\nConnection · Unavailable · Timeout\nUniqueViolation { constraint, detail }\nForeignKeyViolation { detail }\nNotFound { entity, id }\nExtraction { column, reason }\nDeserialization\nOptimisticLock { entity }\nDatabase · Transaction"]

    sqlx --> conv --> re
```

---

### 4.12 Transactions

`CrudRepository` can be constructed from either a `sqlx::Pool` for auto-commit operations, or a `sqlx::Transaction` to participate in an explicit transaction. When using a transaction, all operations (`insert`, `update`, `delete`, etc.) execute within that transaction scope. The transaction commits or rolls back based on the caller's logic, keeping repository methods unaware of transaction boundaries.

```rust
let mut tx = pool.begin().await?;
let repo = UserRepo::new(&mut tx);
repo.insert(user.clone()).await?;
repo.delete(&user.id).await?;
tx.commit().await?;
```

---

## 5. `rustdata-migrations`

### 5.1 Compile-time embedding

`include_migrations!(path)` is a proc-macro that runs entirely at compile time:

```mermaid
flowchart LR
    fs["migrations/*.sql\non developer's filesystem"]
    mac["include_migrations!(path)\n[proc-macro, build host]"]
    sort["Sort files by numeric\nversion prefix\nv1→1  V2→2  001→1  20240101→20240101"]
    emit["Emit &'static [(&'static str, &'static str)]\n= [(stem, include_str!(abs_path)), …]"]
    bin["Baked into binary\n— no runtime filesystem access"]

    fs --> mac --> sort --> emit --> bin
```

### 5.2 `migrate!` macro

```rust
rustdata_migrations::migrate!(&pool).await?;

// expands to:
const MIGRATIONS: &[(&str, &str)] =
    rustdata_macros::include_migrations!("migrations");
rustdata_migrations::__run_migrations(&pool, MIGRATIONS).await?;
```

`__run_migrations` dispatches via the sealed `__PoolDispatch` trait:

```mermaid
flowchart TD
    invoke["__run_migrations(&amp;pool, MIGRATIONS)"]
    disp{"pool type\n(sealed __PoolDispatch)"}
    sq["run_sqlite"]
    pg["run_postgres"]
    my["run_mysql"]

    invoke --> disp
    disp -- SqlitePool --> sq
    disp -- PgPool     --> pg
    disp -- MySqlPool  --> my
```

### 5.3 Runner

Each dialect runner applies the embedded migrations in ascending version order:

```mermaid
sequenceDiagram
    participant App
    participant Runner
    participant DB

    Runner->>DB: CREATE TABLE IF NOT EXISTS schema_migrations

    Runner->>DB: SELECT version, checksum FROM schema_migrations
    DB-->>Runner: [applied versions and checksums]

    loop For each embedded migration (sorted by version)
        alt version already applied AND checksum matches
            Runner-->>Runner: skip
        else version already applied BUT checksum mismatch
            Runner-->>App: Err(ChecksumMismatch { version, expected, actual })
        else pending (not yet applied)
            Runner->>Runner: Transpiler.transpile(sql, dialect)
            Runner->>DB: BEGIN TRANSACTION
            Runner->>DB: Execute transpiled DDL
            Runner->>DB: INSERT INTO schema_migrations (version, checksum, applied_at)
            Runner->>DB: COMMIT
        end
    end

    Runner-->>App: Ok(())
```

The checksum (SHA-256 of the original canonical SQL) detects accidental edits to already-applied migrations and fails fast rather than silently applying a corrupted history.

### 5.4 Transpiler

The transpiler is a line-oriented, annotation-aware two-pass engine.

```mermaid
flowchart TD
    src["Canonical SQL\n(Postgres-style DDL)"]

    p1["Pass 1 - Dialect block filtering\n\n-- @dialect sqlite_only\n  ... SQLite-only lines ...\n-- @end_dialect\n\nKeep block only if dialect matches.\nLines outside any block pass through."]

    p2["Pass 2 - Type substitution\nToken-level lookup table:\n(source_token, target_dialect) to replacement\nMulti-word tokens matched before single-word\nto prevent partial substitution."]

    out["Target dialect SQL"]

    src --> p1 --> p2 --> out
```

Type mapping reference:

| Canonical (Postgres) | SQLite | MySQL |
|---|---|---|
| `UUID` | `TEXT` | `CHAR(36)` |
| `TIMESTAMPTZ` | `TEXT` | `DATETIME` |
| `BOOLEAN` | `INTEGER` | `TINYINT(1)` |
| `BIGSERIAL` | `INTEGER AUTOINCREMENT` | `BIGINT AUTO_INCREMENT` |
| `DEFAULT gen_random_uuid()` | `DEFAULT (lower(hex(randomblob(16))))` | *(removed)* |
| `DEFAULT now()` | `DEFAULT (datetime('now'))` | `DEFAULT CURRENT_TIMESTAMP` |
| `DOUBLE PRECISION` | `REAL` | `DOUBLE` |

`SqlDialect::ph(n)` selects the placeholder style: `$n` (Postgres), `?` (SQLite/MySQL), `@Pn` (MSSQL).

> **MSSQL support:** The `@Pn` placeholder format is reserved for forward compatibility. Full MSSQL backend support is planned but not yet implemented.

---

## 6. Data flow: a single `insert`

```mermaid
sequenceDiagram
    participant App
    participant Repo    as CrudRepository
    participant Hooks   as LifecycleHooks
    participant Cache   as OnceLock SQL cache
    participant Desc    as EntityDescriptor (generated)
    participant Bind    as BindAdapter
    participant Pool    as sqlx::Pool

    App->>Repo: insert(user)

    Repo->>Hooks: before_save(&mut user)
    Hooks-->>Repo: Ok(())

    Repo->>Cache: get insert_sql
    alt first call (cache miss)
        Cache->>Cache: assemble "INSERT INTO users (id, ...) VALUES (?, ...)"
    end
    Cache-->>Repo: &str

    Repo->>Desc: bind_insert(query, &user)
    Desc->>Bind: SqlBind::sql_bind per field (Uuid, String, DateTime, bool ...)
    Bind-->>Repo: bound query

    Repo->>Pool: execute(query)
    Pool-->>Repo: Ok(query_result)

    Note over Repo: For auto-generated columns (e.g., IDs, DEFAULTs),\nre-query the row or use RETURNING clause\n(strategy depends on the Backend dialect).

    Repo->>Hooks: after_save(&user)
    Hooks-->>Repo: Ok(())

    Repo-->>App: Ok(user)
```

---

## 7. Data flow: migration startup

```mermaid
sequenceDiagram
    participant Mac as "include_migrations! (compile time)"
    participant Bin as "Binary (&'static [])"
    participant App
    participant Run as "Runner"
    participant Tran as "Transpiler"
    participant DB

    Note over Mac,Bin: At compile time
    Mac->>Bin: embed [(stem, sql_text), …] as &'static str

    Note over App,DB: At runtime — migrate!(&pool).await?
    App->>Run: __run_migrations(&pool, MIGRATIONS)
    Run->>DB: CREATE TABLE IF NOT EXISTS schema_migrations
    Run->>DB: SELECT applied versions
    DB-->>Run: [v1, v2]

    loop for each pending migration
        Run->>Tran: transpile(canonical_sql, dialect)
        Tran-->>Run: dialect_sql
        Run->>DB: BEGIN
        Run->>DB: execute dialect_sql
        Run->>DB: INSERT INTO schema_migrations (version, checksum)
        Run->>DB: COMMIT
    end

    Run-->>App: Ok(())
```

---

## 8. Design decisions

### `EntityDescriptor` instead of direct sqlx traits

sqlx's `FromRow`, `Encode`, and `Decode` traits carry a concrete database type as a type parameter, which leaks the dialect through every generic bound in the call tree. `EntityDescriptor` encapsulates bind and extract logic behind a trait with generic `DB` and `B: BindAdapter<DB>` parameters. `CrudRepository` then stays fully dialect-generic — switching from SQLite to Postgres requires only a feature flag change, with no application code changes.

### Local traits for `QueryMethods`

Rust's orphan rule forbids `impl<BA> CrudRepository<BA, User> { … }` from a crate that doesn't own `CrudRepository`. The macro generates a *local* trait (`UserCrudQueryMethods`) in the expansion crate (the user's crate), then implements it for the foreign type. `impl LocalTrait for ForeignType` is always permitted. Method bodies live in the `impl` block rather than as trait defaults so that `self.find_all_pred(…)` resolves correctly against `CrudRepository`'s concrete inherent methods without additional bounds on the trait definition.

### `DefaultBackend` in `rustdata-core`, not in the macro

Proc-macro crates compile for the build host and do not inherit the consumer crate's Cargo features. Every `cfg(feature = "sqlite")` guard inside `rustdata-macros` evaluates against the macro crate's own feature set — which declares none — so the guard is always false. Placing the feature-conditioned `DefaultBackend` alias in `rustdata-core` (where `sqlite`, `postgres`, and `mysql` are actually declared) ensures the correct backend is selected and the generated `UserRepo` alias is always a concrete, fully-resolved type.

### Compile-time migration embedding

`include_str!` bakes each SQL file into the binary at compile time. This means:

- **Self-contained binary** — no migration directory needed at the deployment target.
- **Compiler catches missing files** — a deleted or renamed SQL file is a compile error, not a silent runtime failure.
- **No runtime I/O** — startup is faster; the binary works in read-only environments (containers, serverless).

The trade-off is that adding a migration requires a recompile. For a database-backed application this is not a meaningful constraint: schema changes always require a deployment anyway.