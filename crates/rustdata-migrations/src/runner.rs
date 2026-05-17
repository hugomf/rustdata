//! High-level migration runner — drives transpile + apply in one call.
//!
//! This module is the implementation behind the `migrate!` macro.
//! Developers never need to interact with it directly.

use crate::{Dialect, TranspileError, Transpiler};

/// Error type for the migration runner.
#[derive(Debug)]
pub enum RunnerError {
    Transpile(TranspileError),
    Database(String),
    InvalidVersion(String),
}

impl std::fmt::Display for RunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transpile(e) => write!(f, "transpile error: {}", e),
            Self::Database(e) => write!(f, "database error: {}", e),
            Self::InvalidVersion(s) => write!(f, "invalid migration filename: {}", s),
        }
    }
}

impl std::error::Error for RunnerError {}

impl From<TranspileError> for RunnerError {
    fn from(e: TranspileError) -> Self {
        Self::Transpile(e)
    }
}

/// A single applied migration record tracked in `schema_migrations`.
#[derive(Debug)]
pub struct AppliedMigration {
    pub version: u64,
    pub checksum: String,
}

/// Parse the numeric version from a migration file stem.
///
/// Accepts these naming conventions:
/// - `v1__create_users`   → 1
/// - `V2__add_fields`     → 2
/// - `001_create_users`   → 1
/// - `20240101_init`      → 20240101
pub fn parse_version(stem: &str) -> Result<u64, RunnerError> {
    let s = stem
        .strip_prefix('v')
        .or_else(|| stem.strip_prefix('V'))
        .unwrap_or(stem);
    let s = s
        .strip_prefix('m')
        .or_else(|| s.strip_prefix('M'))
        .unwrap_or(s);
    let prefix = s
        .split(|c: char| ['_', '-'].contains(&c))
        .next()
        .unwrap_or(s);
    prefix
        .parse::<u64>()
        .map_err(|_| RunnerError::InvalidVersion(stem.to_string()))
}

fn compute_checksum(sql: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(sql.as_bytes());
    hex::encode(h.finalize())
}

// ── SQLite runner ────────────────────────────────────────────────────────────

#[cfg(feature = "sqlite")]
pub async fn run_sqlite(
    pool: &sqlx::SqlitePool,
    entries: &'static [(&'static str, &'static str)],
) -> Result<(), RunnerError> {
    run_for_pool(pool, entries, Dialect::Sqlite, SqliteRunner).await
}

// ── Postgres runner ──────────────────────────────────────────────────────────

#[cfg(feature = "postgres")]
pub async fn run_postgres(
    pool: &sqlx::PgPool,
    entries: &'static [(&'static str, &'static str)],
) -> Result<(), RunnerError> {
    run_for_pool(pool, entries, Dialect::Postgres, PostgresRunner).await
}

// ── MySQL runner ─────────────────────────────────────────────────────────────

#[cfg(feature = "mysql")]
pub async fn run_mysql(
    pool: &sqlx::MySqlPool,
    entries: &'static [(&'static str, &'static str)],
) -> Result<(), RunnerError> {
    run_for_pool(pool, entries, Dialect::MySql, MySqlRunner).await
}

// ── Internal dialect-dispatched runner ───────────────────────────────────────

/// Trait implemented once per database so the generic algorithm can call
/// dialect-specific SQL without knowing the concrete pool type.
#[async_trait::async_trait]
trait DialectRunner {
    type Pool;

    async fn ensure_table(&self, pool: &Self::Pool) -> Result<(), RunnerError>;
    async fn load_applied(&self, pool: &Self::Pool) -> Result<Vec<AppliedMigration>, RunnerError>;
    async fn execute_sql(&self, pool: &Self::Pool, sql: &str) -> Result<(), RunnerError>;
    async fn record_applied(
        &self,
        pool: &Self::Pool,
        version: u64,
        description: &str,
        checksum: &str,
    ) -> Result<(), RunnerError>;
}

async fn run_for_pool<P, R>(
    pool: &P,
    entries: &'static [(&'static str, &'static str)],
    dialect: Dialect,
    runner: R,
) -> Result<(), RunnerError>
where
    R: DialectRunner<Pool = P>,
{
    let transpiler = Transpiler::new(dialect);

    runner.ensure_table(pool).await?;
    let applied = runner.load_applied(pool).await?;
    let applied_versions: std::collections::HashSet<u64> =
        applied.iter().map(|m| m.version).collect();

    for (stem, canonical_sql) in entries {
        let version = parse_version(stem)?;
        if applied_versions.contains(&version) {
            continue;
        }

        let out = transpiler.transpile(canonical_sql)?;
        runner.execute_sql(pool, &out.sql).await?;

        let description = stem.split_once("__").map(|x| x.1).unwrap_or(stem);
        let checksum = compute_checksum(canonical_sql);
        runner
            .record_applied(pool, version, description, &checksum)
            .await?;
    }
    Ok(())
}

// ── SQLite implementation ────────────────────────────────────────────────────

#[cfg(feature = "sqlite")]
struct SqliteRunner;

#[cfg(feature = "sqlite")]
#[async_trait::async_trait]
impl DialectRunner for SqliteRunner {
    type Pool = sqlx::SqlitePool;

    async fn ensure_table(&self, pool: &sqlx::SqlitePool) -> Result<(), RunnerError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version     INTEGER PRIMARY KEY,
                description TEXT    NOT NULL DEFAULT '',
                checksum    TEXT    NOT NULL DEFAULT '',
                applied_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S','now'))
            )",
        )
        .execute(pool)
        .await
        .map_err(|e| RunnerError::Database(e.to_string()))?;
        Ok(())
    }

    async fn load_applied(
        &self,
        pool: &sqlx::SqlitePool,
    ) -> Result<Vec<AppliedMigration>, RunnerError> {
        use sqlx::Row;
        let rows = sqlx::query("SELECT version, checksum FROM schema_migrations ORDER BY version")
            .fetch_all(pool)
            .await
            .map_err(|e| RunnerError::Database(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| AppliedMigration {
                version: r.try_get::<i64, _>(0).unwrap_or(0) as u64,
                checksum: r.try_get::<String, _>(1).unwrap_or_default(),
            })
            .collect())
    }

    async fn execute_sql(&self, pool: &sqlx::SqlitePool, sql: &str) -> Result<(), RunnerError> {
        // SQLite requires splitting on `;` for multi-statement batches
        for stmt in sql.split(';') {
            let stmt = stmt.trim();
            if stmt.is_empty() {
                continue;
            }
            sqlx::query(stmt)
                .execute(pool)
                .await
                .map_err(|e| RunnerError::Database(format!("{}: {}", e, stmt)))?;
        }
        Ok(())
    }

    async fn record_applied(
        &self,
        pool: &sqlx::SqlitePool,
        version: u64,
        description: &str,
        checksum: &str,
    ) -> Result<(), RunnerError> {
        sqlx::query(
            "INSERT INTO schema_migrations (version, description, checksum) VALUES (?1, ?2, ?3)",
        )
        .bind(version as i64)
        .bind(description)
        .bind(checksum)
        .execute(pool)
        .await
        .map_err(|e| RunnerError::Database(e.to_string()))?;
        Ok(())
    }
}

// ── Postgres implementation ──────────────────────────────────────────────────

#[cfg(feature = "postgres")]
struct PostgresRunner;

#[cfg(feature = "postgres")]
#[async_trait::async_trait]
impl DialectRunner for PostgresRunner {
    type Pool = sqlx::PgPool;

    async fn ensure_table(&self, pool: &sqlx::PgPool) -> Result<(), RunnerError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version     BIGINT      PRIMARY KEY,
                description TEXT        NOT NULL DEFAULT '',
                checksum    TEXT        NOT NULL DEFAULT '',
                applied_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(pool)
        .await
        .map_err(|e| RunnerError::Database(e.to_string()))?;
        Ok(())
    }

    async fn load_applied(
        &self,
        pool: &sqlx::PgPool,
    ) -> Result<Vec<AppliedMigration>, RunnerError> {
        use sqlx::Row;
        let rows = sqlx::query("SELECT version, checksum FROM schema_migrations ORDER BY version")
            .fetch_all(pool)
            .await
            .map_err(|e| RunnerError::Database(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| AppliedMigration {
                version: r.try_get::<i64, _>(0).unwrap_or(0) as u64,
                checksum: r.try_get::<String, _>(1).unwrap_or_default(),
            })
            .collect())
    }

    async fn execute_sql(&self, pool: &sqlx::PgPool, sql: &str) -> Result<(), RunnerError> {
        sqlx::query(sql)
            .execute(pool)
            .await
            .map_err(|e| RunnerError::Database(e.to_string()))?;
        Ok(())
    }

    async fn record_applied(
        &self,
        pool: &sqlx::PgPool,
        version: u64,
        description: &str,
        checksum: &str,
    ) -> Result<(), RunnerError> {
        sqlx::query(
            "INSERT INTO schema_migrations (version, description, checksum) VALUES ($1, $2, $3)",
        )
        .bind(version as i64)
        .bind(description)
        .bind(checksum)
        .execute(pool)
        .await
        .map_err(|e| RunnerError::Database(e.to_string()))?;
        Ok(())
    }
}

// ── MySQL implementation ─────────────────────────────────────────────────────

#[cfg(feature = "mysql")]
struct MySqlRunner;

#[cfg(feature = "mysql")]
#[async_trait::async_trait]
impl DialectRunner for MySqlRunner {
    type Pool = sqlx::MySqlPool;

    async fn ensure_table(&self, pool: &sqlx::MySqlPool) -> Result<(), RunnerError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version     BIGINT      PRIMARY KEY,
                description TEXT        NOT NULL,
                checksum    TEXT        NOT NULL,
                applied_at  TIMESTAMP   NOT NULL DEFAULT CURRENT_TIMESTAMP
            )",
        )
        .execute(pool)
        .await
        .map_err(|e| RunnerError::Database(e.to_string()))?;
        Ok(())
    }

    async fn load_applied(
        &self,
        pool: &sqlx::MySqlPool,
    ) -> Result<Vec<AppliedMigration>, RunnerError> {
        use sqlx::Row;
        let rows = sqlx::query("SELECT version, checksum FROM schema_migrations ORDER BY version")
            .fetch_all(pool)
            .await
            .map_err(|e| RunnerError::Database(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| AppliedMigration {
                version: r.try_get::<i64, _>(0).unwrap_or(0) as u64,
                checksum: r.try_get::<String, _>(1).unwrap_or_default(),
            })
            .collect())
    }

    async fn execute_sql(&self, pool: &sqlx::MySqlPool, sql: &str) -> Result<(), RunnerError> {
        sqlx::query(sql)
            .execute(pool)
            .await
            .map_err(|e| RunnerError::Database(e.to_string()))?;
        Ok(())
    }

    async fn record_applied(
        &self,
        pool: &sqlx::MySqlPool,
        version: u64,
        description: &str,
        checksum: &str,
    ) -> Result<(), RunnerError> {
        sqlx::query(
            "INSERT INTO schema_migrations (version, description, checksum) VALUES (?, ?, ?)",
        )
        .bind(version as i64)
        .bind(description)
        .bind(checksum)
        .execute(pool)
        .await
        .map_err(|e| RunnerError::Database(e.to_string()))?;
        Ok(())
    }
}
