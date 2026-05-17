use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum RepositoryError {
    #[error("database connection error: {0}")]
    Connection(String),

    #[error("query timeout after {0}ms")]
    Timeout(u64),

    #[error("database unavailable: {0}")]
    Unavailable(String),

    #[error("unique constraint violation on {constraint}: {detail}")]
    UniqueViolation { constraint: String, detail: String },

    #[error("foreign key violation: {detail}")]
    ForeignKeyViolation { detail: String },

    #[error("{entity} not found with id {id}")]
    NotFound { entity: String, id: String },

    #[error("row extraction failed for column '{column}': {reason}")]
    Extraction { column: String, reason: String },

    #[error("deserialization error: {0}")]
    Deserialization(String),

    #[error("optimistic lock failure: {entity} was modified by another transaction")]
    OptimisticLock { entity: String },

    #[error("database error: {0}")]
    Database(String),

    #[error("transaction error: {0}")]
    Transaction(String),
}

impl From<sqlx::Error> for RepositoryError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            // fetch_optional returns Ok(None) for missing rows — RowNotFound
            // only appears when using fetch_one on a query that returns zero rows,
            // which we avoid. Map it defensively rather than panicking.
            sqlx::Error::RowNotFound => RepositoryError::NotFound {
                entity: "unknown".into(),
                id: "unknown".into(),
            },
            sqlx::Error::Database(ref db_err) => {
                let code = db_err.code().map(|c| c.to_string());
                let msg = db_err.message().to_lowercase();

                // Unique constraint — checked by numeric code where available,
                // then by message text for SQLite (which has no numeric codes).
                if matches!(
                    code.as_deref(),
                    Some("23505") | Some("1062") | Some("1555") | Some("2067")
                ) || msg.contains("unique constraint")
                    || msg.contains("duplicate entry")
                    || msg.contains("unique_violation")
                {
                    return RepositoryError::UniqueViolation {
                        constraint: db_err.constraint().unwrap_or("unknown").into(),
                        detail: db_err.message().into(),
                    };
                }

                // Foreign key constraint
                if matches!(code.as_deref(), Some("23503") | Some("1452") | Some("787"))
                    || msg.contains("foreign key constraint")
                    || msg.contains("foreign key violation")
                {
                    return RepositoryError::ForeignKeyViolation {
                        detail: db_err.message().into(),
                    };
                }

                // Query timeout (Postgres)
                if code.as_deref() == Some("57014") {
                    return RepositoryError::Timeout(0);
                }

                RepositoryError::Database(db_err.message().into())
            }
            sqlx::Error::PoolTimedOut => RepositoryError::Connection("pool timed out".into()),
            sqlx::Error::PoolClosed => RepositoryError::Unavailable("pool closed".into()),
            other => RepositoryError::Database(other.to_string()),
        }
    }
}

pub type DbError = RepositoryError;
