//! # rustdata-migrations
//!
//! SQL transpilation and zero-boilerplate migration runner for rustdata.
//!
//! ## For developers — the only thing you need
//!
//! ```ignore
//! // Apply all migrations in `migrations/` to your pool.
//! // Dialect is inferred automatically from the pool type.
//! rustdata_migrations::migrate!(&pool).await?;
//!
//! // Custom path:
//! rustdata_migrations::migrate!(&pool, "db/migrations").await?;
//! ```
//!
//! That's it. You never touch `Transpiler`, `Dialect`, or `RunnerError` directly.
//!
//! ## What happens under the hood
//!
//! 1. `migrate!` bakes all `*.sql` files from the given folder into the binary
//!    at compile time (no runtime filesystem reads).
//! 2. At runtime it detects the target SQL dialect from the pool type
//!    (e.g. `sqlx::SqlitePool` → SQLite).
//! 3. Each canonical migration is transpiled (type mapping, placeholder style)
//!    for the target dialect.
//! 4. A `schema_migrations` table is created if it doesn't exist.
//! 5. Only migrations not yet recorded in that table are executed.

pub mod dialects;
pub mod transpiler;
pub mod runner;

pub use dialects::Dialect;
pub use transpiler::{Transpiler, TranspileOutput, TranspileError};
pub use runner::RunnerError;

// Re-export dialect-specific runners so the macro can call them.
#[cfg(feature = "sqlite")]
pub use runner::run_sqlite;
#[cfg(feature = "postgres")]
pub use runner::run_postgres;
#[cfg(feature = "mysql")]
pub use runner::run_mysql;

/// Apply pending migrations from the given folder to the pool.
///
/// # Usage
///
/// ```ignore
/// // Default path: `migrations/` relative to your crate root
/// rustdata_migrations::migrate!(&pool).await?;
///
/// // Explicit path
/// rustdata_migrations::migrate!(&pool, "db/migrations").await?;
/// ```
///
/// The macro detects the pool type at compile time and calls the correct
/// dialect runner. The SQL files are embedded in the binary via `include_str!`
/// — there are no runtime filesystem reads.
///
/// ## Naming convention
///
/// Migration files must start with a numeric version prefix:
/// - `v1__create_users.sql`
/// - `v2__add_fields.sql`
/// - `001_init.sql`
///
/// Files are applied in ascending version order. Already-applied versions
/// (tracked in `schema_migrations`) are skipped.
#[macro_export]
macro_rules! migrate {
    // migrate!(&pool)  →  default path "migrations"
    ($pool:expr) => {
        $crate::migrate!($pool, "migrations")
    };

    // migrate!(&pool, "some/path")
    ($pool:expr, $path:literal) => {{
        // `include_migrations!` is a proc-macro that glob-expands at compile
        // time and embeds every *.sql file as a &'static str pair.
        // It sorts entries by their numeric version prefix automatically.
        const MIGRATIONS: &[(&str, &str)] =
            rustdata_macros::include_migrations!($path);

        $crate::__run_migrations($pool, MIGRATIONS)
    }};
}

/// Internal dispatcher — do not call directly. Use `migrate!` instead.
///
/// Routes to the correct backend runner based on which feature is enabled.
/// We expose this as a `pub` fn (not truly public API) so the macro can
/// call it from outside the crate.
#[doc(hidden)]
pub async fn __run_migrations<P: __PoolDispatch>(
    pool: &P,
    entries: &'static [(&'static str, &'static str)],
) -> Result<(), RunnerError> {
    pool.__dispatch(entries).await
}

/// Sealed trait that lets the macro pick the right runner for any pool type
/// without the developer knowing it exists.
#[doc(hidden)]
pub trait __PoolDispatch {
    fn __dispatch<'a>(
        &'a self,
        entries: &'static [(&'static str, &'static str)],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), RunnerError>> + Send + 'a>>;
}

#[cfg(feature = "sqlite")]
impl __PoolDispatch for sqlx::SqlitePool {
    fn __dispatch<'a>(
        &'a self,
        entries: &'static [(&'static str, &'static str)],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), RunnerError>> + Send + 'a>> {
        Box::pin(run_sqlite(self, entries))
    }
}

#[cfg(feature = "postgres")]
impl __PoolDispatch for sqlx::PgPool {
    fn __dispatch<'a>(
        &'a self,
        entries: &'static [(&'static str, &'static str)],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), RunnerError>> + Send + 'a>> {
        Box::pin(run_postgres(self, entries))
    }
}

#[cfg(feature = "mysql")]
impl __PoolDispatch for sqlx::MySqlPool {
    fn __dispatch<'a>(
        &'a self,
        entries: &'static [(&'static str, &'static str)],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), RunnerError>> + Send + 'a>> {
        Box::pin(run_mysql(self, entries))
    }
}
