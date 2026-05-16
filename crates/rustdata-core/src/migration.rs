use chrono::{DateTime, Utc};
use std::marker::PhantomData;
use std::path::PathBuf;

use crate::{
    backend::{AdOf, Backend, DbOf, ExOf},
    bind::BindAdapter,
    descriptor::RowExtractor,
    dialect::SqlDialect,
};

#[derive(Debug, Clone)]
pub struct Migration {
    pub version: u64,
    pub description: String,
    pub checksum: String,
    pub sql: String,
    pub applied_at: Option<DateTime<Utc>>,
}

#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("invalid migration file: {0}")]
    InvalidFile(String),
    #[error("migration {version} checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch {
        version: u64,
        expected: String,
        actual: String,
    },
    #[error("missing migration file for version {0}")]
    MissingMigrationFile(u64),
    #[error("database error: {0}")]
    Database(String),
}

pub struct MigrationManager<'a, B: Backend> {
    pool: &'a sqlx::Pool<DbOf<B>>,
    migration_path: PathBuf,
    table_name: String,
    dialect: SqlDialect,
    validate_checksums: bool,
    _b: PhantomData<B>,
}

impl<'a, B: Backend> MigrationManager<'a, B>
where
    for<'q> <DbOf<B> as sqlx::Database>::Arguments<'q>: sqlx::IntoArguments<'q, DbOf<B>>,
    for<'c> &'c mut <DbOf<B> as sqlx::Database>::Connection: sqlx::Executor<'c, Database = DbOf<B>>,
    ExOf<B>: RowExtractor<Row = <DbOf<B> as sqlx::Database>::Row>,
{
    pub fn new(pool: &'a sqlx::Pool<DbOf<B>>) -> Self {
        Self {
            pool,
            migration_path: PathBuf::from("./migrations"),
            table_name: "schema_migrations".to_string(),
            dialect: SqlDialect::Postgres,
            validate_checksums: true,
            _b: PhantomData,
        }
    }

    pub fn with_migration_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.migration_path = path.into();
        self
    }

    pub fn with_table(mut self, table: &str) -> Self {
        self.table_name = table.to_string();
        self
    }

    pub fn with_dialect(mut self, dialect: SqlDialect) -> Self {
        self.dialect = dialect;
        self
    }

    pub fn with_validate_checksums(mut self, validate: bool) -> Self {
        self.validate_checksums = validate;
        self
    }

    pub async fn migrate(&self) -> Result<Vec<Migration>, MigrationError> {
        self.create_migration_table().await?;
        let files = self.read_migration_files()?;
        let applied = self.get_applied_migrations().await?;
        let applied_versions: std::collections::HashSet<u64> =
            applied.iter().map(|m| m.version).collect();

        if self.validate_checksums {
            self.validate(&files, &applied)?;
        }

        let mut results = Vec::new();
        for migration in files {
            if !applied_versions.contains(&migration.version) {
                self.run_migration(&migration).await?;
                results.push(migration);
            }
        }
        Ok(results)
    }

    async fn create_migration_table(&self) -> Result<(), MigrationError> {
        let sql = match self.dialect {
            SqlDialect::Postgres => format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    version BIGINT PRIMARY KEY,
                    description VARCHAR(255) NOT NULL,
                    checksum VARCHAR(64) NOT NULL,
                    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    success BOOLEAN NOT NULL DEFAULT TRUE
                )",
                self.table_name
            ),
            SqlDialect::Sqlite => format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    version INTEGER PRIMARY KEY,
                    description TEXT NOT NULL,
                    checksum TEXT NOT NULL,
                    applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now')),
                    success INTEGER NOT NULL DEFAULT 1
                )",
                self.table_name
            ),
            _ => format!(
                "CREATE TABLE IF NOT EXISTS {} (
                    version BIGINT PRIMARY KEY,
                    description VARCHAR(255) NOT NULL,
                    checksum VARCHAR(64) NOT NULL,
                    applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    success BOOLEAN NOT NULL DEFAULT TRUE
                )",
                self.table_name
            ),
        };
        sqlx::query(&sql)
            .execute(self.pool)
            .await
            .map_err(|e| MigrationError::Database(e.to_string()))?;
        Ok(())
    }

    fn parse_sqlx_filename(stem: &str) -> Option<(u64, String)> {
        // sqlx format: {version}_{description}.sql
        // Standard: "20240426001_create_users" → version=20240426001, desc="create_users"
        // UserBrew format: "m001_create_users" → version=001, desc="create_users"
        // Strip leading 'm' or 'M' if present
        let stem = stem.strip_prefix('m').unwrap_or(stem);
        let stem = stem.strip_prefix('M').unwrap_or(stem);

        if let Some(idx) = stem.find('_') {
            let (version_str, description) = stem.split_at(idx);
            let description = description.strip_prefix('_').unwrap_or("").to_string();
            let version = version_str.parse::<u64>().ok()?;
            Some((version, description))
        } else {
            let version = stem.parse::<u64>().ok()?;
            Some((version, String::new()))
        }
    }

    fn read_migration_files(&self) -> Result<Vec<Migration>, MigrationError> {
        use hex;
        use sha2::{Digest, Sha256};
        use std::fs;

        let mut migrations = Vec::new();
        let dir = fs::read_dir(&self.migration_path)
            .map_err(|e| MigrationError::InvalidFile(e.to_string()))?;

        for entry in dir {
            let entry = entry.map_err(|e| MigrationError::InvalidFile(e.to_string()))?;
            let path = entry.path();
            if path.extension().map(|ext| ext == "sql").unwrap_or(false) {
                let filename = path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
                    MigrationError::InvalidFile(format!("Invalid filename: {:?}", path))
                })?;

                if let Some((version, description)) = Self::parse_sqlx_filename(filename) {
                    let sql = fs::read_to_string(&path)
                        .map_err(|e| MigrationError::InvalidFile(e.to_string()))?;
                    let mut hasher = Sha256::new();
                    hasher.update(sql.as_bytes());
                    let checksum = hex::encode(hasher.finalize().as_slice());

                    migrations.push(Migration {
                        version,
                        description,
                        checksum,
                        sql,
                        applied_at: None,
                    });
                }
            }
        }
        migrations.sort_by_key(|m| m.version);
        Ok(migrations)
    }

    async fn get_applied_migrations(&self) -> Result<Vec<Migration>, MigrationError> {
        let ext = <ExOf<B> as Default>::default();
        let sql = format!(
            "SELECT version, description, checksum, applied_at FROM {} ORDER BY version",
            self.table_name
        );
        let rows = sqlx::query(&sql)
            .fetch_all(self.pool)
            .await
            .map_err(|e| MigrationError::Database(e.to_string()))?;

        let mut migrations = Vec::new();
        for row in &rows {
            let version = ext
                .get_i64(row, "version")
                .map_err(|e| MigrationError::Database(e.to_string()))?;
            let description = ext
                .get_str(row, "description")
                .map_err(|e| MigrationError::Database(e.to_string()))?;
            let checksum = ext
                .get_str(row, "checksum")
                .map_err(|e| MigrationError::Database(e.to_string()))?;
            let applied_at = ext
                .get_datetime(row, "applied_at")
                .map_err(|e| MigrationError::Database(e.to_string()))?;

            migrations.push(Migration {
                version: version as u64,
                description,
                checksum,
                sql: String::new(),
                applied_at: Some(applied_at),
            });
        }
        Ok(migrations)
    }

    fn validate(&self, files: &[Migration], applied: &[Migration]) -> Result<(), MigrationError> {
        for applied_migration in applied {
            let file = files
                .iter()
                .find(|f| f.version == applied_migration.version)
                .ok_or(MigrationError::MissingMigrationFile(
                    applied_migration.version,
                ))?;
            if file.checksum != applied_migration.checksum {
                return Err(MigrationError::ChecksumMismatch {
                    version: applied_migration.version,
                    expected: applied_migration.checksum.clone(),
                    actual: file.checksum.clone(),
                });
            }
        }
        Ok(())
    }

    async fn run_migration(&self, migration: &Migration) -> Result<(), MigrationError> {
        sqlx::query(&migration.sql)
            .execute(self.pool)
            .await
            .map_err(|e| MigrationError::Database(e.to_string()))?;

        let ph1 = self.dialect.ph(1);
        let ph2 = self.dialect.ph(2);
        let ph3 = self.dialect.ph(3);
        let ts = self.dialect.current_timestamp();
        let insert_sql = format!(
            "INSERT INTO {} (version, description, checksum, applied_at) VALUES ({ph1}, {ph2}, {ph3}, {ts})",
            self.table_name,
        );
        let mut query = sqlx::query(&insert_sql);
        query = <AdOf<B> as BindAdapter<DbOf<B>>>::bind_int(query, migration.version as i64);
        query = <AdOf<B> as BindAdapter<DbOf<B>>>::bind_str(query, &migration.description);
        query = <AdOf<B> as BindAdapter<DbOf<B>>>::bind_str(query, &migration.checksum);
        query
            .execute(self.pool)
            .await
            .map_err(|e| MigrationError::Database(e.to_string()))?;
        Ok(())
    }
}
