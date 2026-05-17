# Architecture

This document describes the internal design of rustdata: how the crates relate, how data flows through the system at compile time and runtime, and why each component is built the way it is.

## Crate structure

```
rustdata/
├── crates/
│   ├── rustdata-core/        # Main library crate
│   ├── rustdata-macros/      # Proc-macro crate (separate by Rust requirement)
│   └── rustdata-migrations/  # Migration transpiler + runner
```

`rustdata-macros` is a separate crate because Rust requires proc-macro crates to compile for the build host, not the target. It is re-exported wholesale by `rustdata-core`, so consumers never add it directly.

## Dependency graph

```
migration-test (application)
    ├── rustdata-core          (Entity/QueryMethods derives, CrudRepository, backends)
    │       └── rustdata-macros   (proc-macros: Entity, QueryMethods, Projection, SqlType)
    └── rustdata-migrations    (migrate! macro, Transpiler, Runner)
            └── rustdata-macros   (include_migrations! proc-macro)
```

## Compile-time pipeline

The full data layer is generated before a single line of application code compiles. The sequence is:

```
1. Cargo compiles rustdata-macros (build host)
2. Cargo compiles rustdata-core  → re-exports macros
3. Application crate compiles:
   a. #[derive(Entity)]       → EntityDescriptor impl + UserRepo type alias
   b. #[derive(QueryMethods)] → UserCrudQueryMethods trait + impl
   c. migrate!(...)           → include_migrations! embeds SQL files as &'static str
4. Application binary links rustdata-migrations runner
```

Nothing in steps 3a–3c touches the filesystem at runtime.

## `rustdata-core`

### Backend abstraction

The `Backend` trait bundles three associated types that together describe a complete database driver:

```
Backend
  ├── Database    — the sqlx::Database marker type (Sqlite, Postgres, MySql)
  ├── Adapter     — BindAdapter<Database>: knows how to bind Rust values to query params
  └── Extractor   — RowExtractor: knows how to read typed values from a result row
```

Three concrete structs implement `Backend`:

| Struct | Database | Notes |
|---|---|---|
| `backends::Sqlite` | `sqlx::Sqlite` | Feature `sqlite` |
| `backends::Postgres` | `sqlx::PgDatabase` | Feature `postgres` |
| `backends::MySql` | `sqlx::MySql` | Feature `mysql` |

`DefaultBackend` is a type alias in `rustdata-core` that resolves to the active backend based on the crate's own feature flags. The `#[derive(Entity)]` macro references `::rustdata_core::DefaultBackend` so the generated `UserRepo` alias is always concrete — no generic parameter leaks to the call site.

This is important: `rustdata-macros` has no backend features of its own (proc-macro crates compile for the host, not target). Putting the `cfg(feature)` resolution in `rustdata-core` ensures it fires against the correct feature set.

### `EntityDescriptor` trait

The central trait that `#[derive(Entity)]` implements:

```rust
trait EntityDescriptor {
    type Entity;          // the struct itself (e.g. User)
    type Id;              // the primary key type (e.g. Uuid)

    const TABLE: &'static str;
    const ORDER_BY: &'static str;
    const SOFT_DELETE_COL: Option<&'static str>;

    fn columns() -> &'static [ColumnDef];   // column metadata, built at startup
    fn bind_insert(query, entity) -> query; // bind all INSERT parameters
    fn bind_update(query, entity) -> query; // bind all UPDATE parameters
    fn bind_id(query, id)         -> query; // bind the WHERE id = ? parameter
    fn from_row(row, extractor)   -> Entity;// extract a full entity from a row
}
```

`CrudRepository<BA, D>` is generic over any `BA: Backend` and `D: EntityDescriptor`. All SQL is generated at first call using `D`'s constants and `BA`'s dialect, then cached with `std::sync::OnceLock`.

### `CrudRepository` — method surface

```
Core CRUD
  insert(entity)              → Entity
  update(entity)              → Entity
  find_by_id(id)              → Option<Entity>
  delete(id)                  → bool
  hard_delete(id)             → bool    (bypasses soft-delete)
  exists_by_id(id)            → bool

Bulk / list
  find_all()                  → Vec<Entity>
  find_all_sorted(order)      → Vec<Entity>
  list(pageable)              → Page<Entity>
  insert_batch(entities)      → ()
  clear()                     → ()
  count()                     → u64

Predicate queries
  find_all_pred(pred)         → Vec<Entity>
  find_all_pred_paged(pred, pageable) → Page<Entity>
  find_one_pred(pred)         → Option<Entity>
  count_pred(pred)            → u64
  delete_pred(pred)           → u64
  count_with_filters(filters) → u64

Specification pattern
  find_one_spec(spec)         → Option<Entity>
  find_all_spec(spec)         → Vec<Entity>
  count_spec(spec)            → u64
  exists_spec(spec)           → bool

Ad-hoc SQL
  find_one_by_sql(sql, values)  → Option<Entity>
  find_many_by_sql(sql, values) → Vec<Entity>
  execute_sql(sql, values)      → ()
```

### SQL generation and caching

Each SQL string is computed once and stored in a `OnceLock<String>` static. The template is assembled from `EntityDescriptor` constants and `SqlDialect::ph()` / `ph_list()` for placeholder style (`$1` vs `?`).

```
insert_sql   →  INSERT INTO {table} ({cols}) VALUES ({phs})
update_sql   →  UPDATE {table} SET col=$1,… WHERE id=$N
find_by_id   →  SELECT {cols} FROM {table} WHERE {id_col} = $1 LIMIT 1
delete_sql   →  DELETE FROM {table} WHERE {id_col} = $1
```

Predicate queries build SQL dynamically from `Predicate` variants at call time, with a fresh placeholder counter per query.

### `#[derive(Entity)]` macro

Emits, for a struct `User`:

1. `impl LifecycleHooks<User> for User {}` — no-op default, or delegates to a custom hooks type if `#[entity(hooks = "…")]` is set.
2. `impl EntityDescriptor for User` — all constants, `columns()`, `bind_insert`, `bind_update`, `bind_id`, `from_row`.
3. `impl RowExtractable for User` — delegates to `EntityDescriptor::from_row` so `QueryRepository` can reuse the same extraction logic.
4. `pub type UserRepo = CrudRepository<DefaultBackend, User>` — the concrete type alias.

### `#[derive(QueryMethods)]` macro

Inspects every non-id, non-auto-generated, non-skip field and generates two local traits:

- `UserCrudQueryMethods<BA>` — implemented for `CrudRepository<BA, User>`
- `UserQueryQueryMethods<BA>` — implemented for `QueryRepository<BA, User>`

Local traits (defined in the user's crate after macro expansion) sidestep Rust's coherence rules: you cannot write an inherent `impl` on a foreign type, but you can `impl LocalTrait for ForeignType`.

For each field `active: bool` the macro emits six method families with operator suffix variants:

```
find_by_active(val)
find_by_active_paged(val, pageable)
find_one_by_active(val)
exists_by_active(val)
count_by_active(val)
delete_by_active(val)
```

For compound finders, every pair of fields is emitted in both orderings (`_and_` and `_or_` conjunctions), so the caller can use whichever reads naturally.

All generated methods delegate to `self.find_all_pred(…)` / `self.find_one_pred(…)` / etc., which are inherent methods on `CrudRepository`. The trait wraps them with typed, named parameters.

### Soft delete

When `#[entity(soft_delete = "deleted_at")]` is set, `EntityDescriptor::SOFT_DELETE_COL` is `Some("deleted_at")`. `CrudRepository::delete` detects this and emits `UPDATE … SET deleted_at = now()` instead of `DELETE`. `hard_delete` always emits the physical `DELETE`.

### Lifecycle hooks

`LifecycleHooks<Entity>` provides `before_save` and `after_save` callbacks. `#[derive(Entity)]` emits a blank impl by default. Supply `#[entity(hooks = "MyHooks")]` to delegate to a custom type that implements the trait.

### Pagination

`Pageable` carries `page` (zero-based index) and `size`. `Page<E>` wraps the result with `total_elements`, `total_pages`, and helpers like `is_first()` / `is_last()`. The repository counts with a `SELECT COUNT(*)` query and fetches the page with `LIMIT ? OFFSET ?`.

### Specification pattern

`Predicate` is a recursive enum:

```
Predicate::Eq / Ne / Lt / Lte / Gt / Gte / Like / In / IsNull / IsNotNull
Predicate::And(Vec<Predicate>)
Predicate::Or(Vec<Predicate>)
Predicate::Not(Box<Predicate>)
```

`Specification<E>` is a trait with `fn predicate(&self) -> Predicate`. Implement it on domain types to express business rules as composable, reusable predicates.

### Error handling

`RepositoryError` variants cover the common failure modes:

```
Connection / Unavailable / Timeout
UniqueViolation { constraint, detail }
ForeignKeyViolation { detail }
NotFound { entity, id }
Extraction { column, reason }
Deserialization
OptimisticLock { entity }
Database / Transaction
```

The `From<sqlx::Error>` impl normalises database-specific error codes (Postgres `23505`, MySQL `1062`, SQLite text matching) into typed variants so application code is dialect-independent.

## `rustdata-migrations`

### Compile-time embedding

`include_migrations!(path)` is a proc-macro that runs at compile time:

1. Reads `$CARGO_MANIFEST_DIR/<path>/*.sql`.
2. Sorts files by numeric version prefix (`v1`, `V2`, `001`, `20240101`, etc.).
3. Emits a `&'static [(&'static str, &'static str)]` literal — `(stem, sql_text)` pairs — using `include_str!` for each file.

The result is baked into the binary. No filesystem access at runtime.

### `migrate!` macro

```rust
rustdata_migrations::migrate!(&pool).await?;
// expands to:
const MIGRATIONS: &[(&str, &str)] = rustdata_macros::include_migrations!("migrations");
rustdata_migrations::__run_migrations(&pool, MIGRATIONS).await?;
```

`__run_migrations` dispatches via the sealed `__PoolDispatch` trait, which routes to `run_sqlite`, `run_postgres`, or `run_mysql` depending on the pool type.

### Runner

Each dialect runner:

1. Creates `schema_migrations (version INTEGER, checksum TEXT, applied_at TEXT)` if absent.
2. Reads applied versions from the table.
3. For each pending migration (in version order): transpiles the SQL to the target dialect, executes it in a transaction, records the version and checksum.

The checksum (SHA-256 of the original canonical SQL) detects accidental edits to already-applied migrations.

### Transpiler

The transpiler is a line-oriented, annotation-aware multi-pass engine:

**Pass 1 — dialect block filtering.** Lines between `-- @dialect X_only` and `-- @end_dialect` are included only if `X` matches the target dialect. Other dialect blocks are dropped entirely.

**Pass 2 — type substitution.** Token-level replacement using a lookup table keyed on `(source_token, target_dialect)`. Handles multi-word tokens (`DOUBLE PRECISION`, `DEFAULT now()`) before single-word ones to avoid partial matches.

The type map covers the most common DDL differences:

```
UUID             → TEXT (SQLite) / CHAR(36) (MySQL)
TIMESTAMPTZ      → TEXT (SQLite) / DATETIME (MySQL)
BOOLEAN          → INTEGER (SQLite) / TINYINT(1) (MySQL)
BIGSERIAL        → INTEGER AUTOINCREMENT (SQLite) / BIGINT AUTO_INCREMENT (MySQL)
gen_random_uuid()→ (lower(hex(randomblob(16)))) (SQLite) / removed (MySQL)
now()            → (datetime('now')) (SQLite) / CURRENT_TIMESTAMP (MySQL)
```

### `SqlDialect`

```rust
enum SqlDialect { Postgres, Sqlite, MySql, MsSql }
```

`ph(n)` returns the dialect's placeholder style (`$n` for Postgres, `?` for SQLite/MySQL, `@Pn` for MSSQL). `ph_list(count)` builds a comma-separated list. `render(template)` substitutes `{n}` placeholders in a SQL template string.

## Data flow: a single `insert`

```
Application code
    user_repo.insert(user)
        │
        ▼
CrudRepository::insert
    1. LifecycleHooks::before_save(&mut entity)
    2. Build SQL: INSERT INTO users (id, username, …) VALUES (?, ?, …)
       (cached in OnceLock after first call)
    3. D::bind_insert(query, &entity)
       → generated code calls SqlBind::sql_bind for each field
       → BindAdapter converts Uuid / DateTime / bool to the DB wire type
    4. sqlx executes the query against the pool
    5. LifecycleHooks::after_save(&entity)
    6. Return entity to caller
```

## Design decisions

**Why `EntityDescriptor` instead of direct sqlx traits?**
sqlx's `FromRow` and `Encode`/`Decode` traits have dialect-specific bounds that leak the concrete database type throughout the call tree. `EntityDescriptor` keeps the bind/extract logic behind a trait with generic `DB` and `B: BindAdapter<DB>` parameters, so the repository itself stays fully generic.

**Why local traits for `QueryMethods`?**
Rust's coherence rules forbid `impl<BA> CrudRepository<BA, User> { fn find_by_active… }` from a crate that doesn't define `CrudRepository`. Generating a local trait (`UserCrudQueryMethods`) and implementing it for the foreign type is the standard workaround, allowed because the trait is local to the crate after macro expansion.

**Why `DefaultBackend` in `rustdata-core` instead of `cfg(feature)` in the macro?**
Proc-macro crates compile for the build host and do not inherit the consumer crate's Cargo features. A `cfg(feature = "sqlite")` guard inside `rustdata-macros` is always false. Placing the feature-conditioned type alias in `rustdata-core` — where the `sqlite`/`postgres`/`mysql` features are actually declared — ensures it resolves correctly.

**Why embed migrations at compile time?**
Compile-time embedding (`include_str!`) means the binary is self-contained. No migration directory must be present at the deployment target, no runtime path resolution is needed, and the compiler catches missing files.