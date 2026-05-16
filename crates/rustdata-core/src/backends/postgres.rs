//! PostgreSQL backend.
//!
//! Uses native Postgres types: UUID, TIMESTAMPTZ, JSONB, BYTEA, etc.

use chrono::{DateTime, Utc};
use sqlx::types::Json;
use sqlx::Row;
use std::collections::HashSet;
use uuid::Uuid;

use crate::bind::QueryBuilder;
use crate::{Backend, BindAdapter, DbBound, RepositoryError, RowExtractor, SqlDialect};

pub struct PgBindAdapter;

impl BindAdapter<sqlx::Postgres> for PgBindAdapter {
    fn dialect() -> SqlDialect {
        SqlDialect::Postgres
    }

    fn bind_uuid<'q>(
        q: QueryBuilder<'q, sqlx::Postgres>,
        v: Uuid,
    ) -> QueryBuilder<'q, sqlx::Postgres> {
        q.bind(v)
    }
    fn bind_opt_uuid<'q>(
        q: QueryBuilder<'q, sqlx::Postgres>,
        v: Option<Uuid>,
    ) -> QueryBuilder<'q, sqlx::Postgres> {
        q.bind(v)
    }
    fn bind_str<'q>(
        q: QueryBuilder<'q, sqlx::Postgres>,
        v: &'q str,
    ) -> QueryBuilder<'q, sqlx::Postgres> {
        q.bind(v)
    }
    fn bind_opt_str<'q>(
        q: QueryBuilder<'q, sqlx::Postgres>,
        v: Option<&'q str>,
    ) -> QueryBuilder<'q, sqlx::Postgres> {
        q.bind(v)
    }
    fn bind_int<'q>(
        q: QueryBuilder<'q, sqlx::Postgres>,
        v: i64,
    ) -> QueryBuilder<'q, sqlx::Postgres> {
        q.bind(v)
    }
    fn bind_opt_int<'q>(
        q: QueryBuilder<'q, sqlx::Postgres>,
        v: Option<i64>,
    ) -> QueryBuilder<'q, sqlx::Postgres> {
        q.bind(v)
    }
    fn bind_bool<'q>(
        q: QueryBuilder<'q, sqlx::Postgres>,
        v: bool,
    ) -> QueryBuilder<'q, sqlx::Postgres> {
        q.bind(v)
    }
    fn bind_opt_bool<'q>(
        q: QueryBuilder<'q, sqlx::Postgres>,
        v: Option<bool>,
    ) -> QueryBuilder<'q, sqlx::Postgres> {
        q.bind(v)
    }
    fn bind_datetime<'q>(
        q: QueryBuilder<'q, sqlx::Postgres>,
        v: DateTime<Utc>,
    ) -> QueryBuilder<'q, sqlx::Postgres> {
        q.bind(v)
    }
    fn bind_opt_datetime<'q>(
        q: QueryBuilder<'q, sqlx::Postgres>,
        v: Option<DateTime<Utc>>,
    ) -> QueryBuilder<'q, sqlx::Postgres> {
        q.bind(v)
    }
    fn bind_json<'q, T: serde::Serialize>(
        q: QueryBuilder<'q, sqlx::Postgres>,
        v: &'q T,
    ) -> QueryBuilder<'q, sqlx::Postgres> {
        q.bind(Json(v))
    }
    fn bind_opt_json<'q, T: serde::Serialize>(
        q: QueryBuilder<'q, sqlx::Postgres>,
        v: Option<&'q T>,
    ) -> QueryBuilder<'q, sqlx::Postgres> {
        q.bind(v.map(Json))
    }
    fn bind_json_value<'q>(
        q: QueryBuilder<'q, sqlx::Postgres>,
        v: serde_json::Value,
    ) -> QueryBuilder<'q, sqlx::Postgres> {
        q.bind(Json(v))
    }
    fn bind_float<'q>(
        q: QueryBuilder<'q, sqlx::Postgres>,
        v: f64,
    ) -> QueryBuilder<'q, sqlx::Postgres> {
        q.bind(v)
    }
    fn bind_opt_float<'q>(
        q: QueryBuilder<'q, sqlx::Postgres>,
        v: Option<f64>,
    ) -> QueryBuilder<'q, sqlx::Postgres> {
        q.bind(v)
    }
    fn bind_bytes<'q>(
        q: QueryBuilder<'q, sqlx::Postgres>,
        v: &'q [u8],
    ) -> QueryBuilder<'q, sqlx::Postgres> {
        q.bind(v)
    }
    fn bind_opt_bytes<'q>(
        q: QueryBuilder<'q, sqlx::Postgres>,
        v: Option<&'q [u8]>,
    ) -> QueryBuilder<'q, sqlx::Postgres> {
        q.bind(v)
    }
    fn rows_affected(result: &<sqlx::Postgres as sqlx::Database>::QueryResult) -> u64 {
        result.rows_affected()
    }
}

#[derive(Default)]
pub struct PgExtractor;

impl RowExtractor for PgExtractor {
    type Row = sqlx::postgres::PgRow;

    fn get_str(&self, row: &Self::Row, col: &str) -> Result<String, RepositoryError> {
        row.try_get::<String, _>(col)
            .map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })
    }
    fn get_opt_str(&self, row: &Self::Row, col: &str) -> Result<Option<String>, RepositoryError> {
        row.try_get::<Option<String>, _>(col)
            .map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })
    }
    fn get_uuid(&self, row: &Self::Row, col: &str) -> Result<Uuid, RepositoryError> {
        row.try_get::<Uuid, _>(col)
            .map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })
    }
    fn get_opt_uuid(&self, row: &Self::Row, col: &str) -> Result<Option<Uuid>, RepositoryError> {
        row.try_get::<Option<Uuid>, _>(col)
            .map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })
    }
    fn get_datetime(&self, row: &Self::Row, col: &str) -> Result<DateTime<Utc>, RepositoryError> {
        row.try_get::<DateTime<Utc>, _>(col)
            .map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })
    }
    fn get_opt_datetime(
        &self,
        row: &Self::Row,
        col: &str,
    ) -> Result<Option<DateTime<Utc>>, RepositoryError> {
        row.try_get::<Option<DateTime<Utc>>, _>(col)
            .map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })
    }
    fn get_bool(&self, row: &Self::Row, col: &str) -> Result<bool, RepositoryError> {
        row.try_get::<bool, _>(col)
            .map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })
    }
    fn get_i32(&self, row: &Self::Row, col: &str) -> Result<i32, RepositoryError> {
        row.try_get::<i32, _>(col)
            .map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })
    }
    fn get_i64(&self, row: &Self::Row, col: &str) -> Result<i64, RepositoryError> {
        row.try_get::<i64, _>(col)
            .map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })
    }
    fn get_opt_i64(&self, row: &Self::Row, col: &str) -> Result<Option<i64>, RepositoryError> {
        row.try_get::<Option<i64>, _>(col)
            .map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })
    }
    fn get_f64(&self, row: &Self::Row, col: &str) -> Result<f64, RepositoryError> {
        row.try_get::<f64, _>(col)
            .map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })
    }
    fn get_opt_f64(&self, row: &Self::Row, col: &str) -> Result<Option<f64>, RepositoryError> {
        row.try_get::<Option<f64>, _>(col)
            .map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })
    }
    fn get_json_value(
        &self,
        row: &Self::Row,
        col: &str,
    ) -> Result<serde_json::Value, RepositoryError> {
        let json: Json<serde_json::Value> =
            row.try_get(col).map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })?;
        Ok(json.0)
    }
    fn get_opt_json_value(
        &self,
        row: &Self::Row,
        col: &str,
    ) -> Result<Option<serde_json::Value>, RepositoryError> {
        let json: Option<Json<serde_json::Value>> =
            row.try_get(col).map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })?;
        Ok(json.map(|j| j.0))
    }
    fn get_string_set(
        &self,
        row: &Self::Row,
        col: &str,
    ) -> Result<HashSet<String>, RepositoryError> {
        let json: Json<HashSet<String>> =
            row.try_get(col).map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })?;
        Ok(json.0)
    }
    fn get_opt_string_set(
        &self,
        row: &Self::Row,
        col: &str,
    ) -> Result<Option<HashSet<String>>, RepositoryError> {
        let json: Option<Json<HashSet<String>>> =
            row.try_get(col).map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })?;
        Ok(json.map(|j| j.0))
    }
    fn get_string_vec(&self, row: &Self::Row, col: &str) -> Result<Vec<String>, RepositoryError> {
        let json: Json<Vec<String>> =
            row.try_get(col).map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })?;
        Ok(json.0)
    }
    fn get_opt_string_vec(
        &self,
        row: &Self::Row,
        col: &str,
    ) -> Result<Option<Vec<String>>, RepositoryError> {
        let json: Option<Json<Vec<String>>> =
            row.try_get(col).map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })?;
        Ok(json.map(|j| j.0))
    }
    fn get_bytes(&self, row: &Self::Row, col: &str) -> Result<Vec<u8>, RepositoryError> {
        row.try_get::<Vec<u8>, _>(col)
            .map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })
    }
    fn get_opt_bytes(
        &self,
        row: &Self::Row,
        col: &str,
    ) -> Result<Option<Vec<u8>>, RepositoryError> {
        row.try_get::<Option<Vec<u8>>, _>(col)
            .map_err(|e| RepositoryError::Extraction {
                column: col.into(),
                reason: e.to_string(),
            })
    }
}

/// Type alias — use `Postgres` as the backend type parameter.
pub type Postgres = PgBackend;

#[derive(Debug)]
pub struct PgBackend;

impl Backend for PgBackend {
    type Database = sqlx::Postgres;
    type Adapter = PgBindAdapter;
    type Extractor = PgExtractor;
}

impl DbBound for PgBackend {}
