//! MySQL backend.
//!
//! UUIDs are stored as text (MySQL has no native UUID type).
//! JSON is stored as TEXT. Booleans are native MySQL BOOL.

use chrono::{DateTime, Utc};
use sqlx::Row;
use std::collections::HashSet;
use uuid::Uuid;

use crate::bind::QueryBuilder;
use crate::{Backend, BindAdapter, DbBound, RepositoryError, RowExtractor, SqlDialect};

pub struct MysqlBindAdapter;

impl BindAdapter<sqlx::MySql> for MysqlBindAdapter {
    fn dialect() -> SqlDialect { SqlDialect::MySql }

    fn bind_uuid<'q>(q: QueryBuilder<'q, sqlx::MySql>, v: Uuid) -> QueryBuilder<'q, sqlx::MySql> { q.bind(v.to_string()) }
    fn bind_opt_uuid<'q>(q: QueryBuilder<'q, sqlx::MySql>, v: Option<Uuid>) -> QueryBuilder<'q, sqlx::MySql> { q.bind(v.map(|v| v.to_string())) }
    fn bind_str<'q>(q: QueryBuilder<'q, sqlx::MySql>, v: &'q str) -> QueryBuilder<'q, sqlx::MySql> { q.bind(v) }
    fn bind_opt_str<'q>(q: QueryBuilder<'q, sqlx::MySql>, v: Option<&'q str>) -> QueryBuilder<'q, sqlx::MySql> { q.bind(v) }
    fn bind_int<'q>(q: QueryBuilder<'q, sqlx::MySql>, v: i64) -> QueryBuilder<'q, sqlx::MySql> { q.bind(v) }
    fn bind_opt_int<'q>(q: QueryBuilder<'q, sqlx::MySql>, v: Option<i64>) -> QueryBuilder<'q, sqlx::MySql> { q.bind(v) }
    fn bind_bool<'q>(q: QueryBuilder<'q, sqlx::MySql>, v: bool) -> QueryBuilder<'q, sqlx::MySql> { q.bind(v) }
    fn bind_opt_bool<'q>(q: QueryBuilder<'q, sqlx::MySql>, v: Option<bool>) -> QueryBuilder<'q, sqlx::MySql> { q.bind(v) }
    fn bind_datetime<'q>(q: QueryBuilder<'q, sqlx::MySql>, v: DateTime<Utc>) -> QueryBuilder<'q, sqlx::MySql> { q.bind(v) }
    fn bind_opt_datetime<'q>(q: QueryBuilder<'q, sqlx::MySql>, v: Option<DateTime<Utc>>) -> QueryBuilder<'q, sqlx::MySql> { q.bind(v) }
    fn bind_json<'q, T: serde::Serialize>(q: QueryBuilder<'q, sqlx::MySql>, v: &'q T) -> QueryBuilder<'q, sqlx::MySql> {
        q.bind(serde_json::to_string(v).unwrap_or_else(|_| "null".into()))
    }
    fn bind_opt_json<'q, T: serde::Serialize>(q: QueryBuilder<'q, sqlx::MySql>, v: Option<&'q T>) -> QueryBuilder<'q, sqlx::MySql> {
        q.bind(v.map(|v| serde_json::to_string(v).unwrap_or_else(|_| "null".into())))
    }
    fn bind_json_value<'q>(q: QueryBuilder<'q, sqlx::MySql>, v: serde_json::Value) -> QueryBuilder<'q, sqlx::MySql> {
        q.bind(serde_json::to_string(&v).unwrap_or_else(|_| "null".into()))
    }
    fn bind_float<'q>(q: QueryBuilder<'q, sqlx::MySql>, v: f64) -> QueryBuilder<'q, sqlx::MySql> { q.bind(v) }
    fn bind_opt_float<'q>(q: QueryBuilder<'q, sqlx::MySql>, v: Option<f64>) -> QueryBuilder<'q, sqlx::MySql> { q.bind(v) }
    fn bind_bytes<'q>(q: QueryBuilder<'q, sqlx::MySql>, v: &'q [u8]) -> QueryBuilder<'q, sqlx::MySql> { q.bind(v) }
    fn bind_opt_bytes<'q>(q: QueryBuilder<'q, sqlx::MySql>, v: Option<&'q [u8]>) -> QueryBuilder<'q, sqlx::MySql> { q.bind(v) }
    fn rows_affected(result: &<sqlx::MySql as sqlx::Database>::QueryResult) -> u64 { result.rows_affected() }
}

#[derive(Default)]
pub struct MysqlExtractor;

impl RowExtractor for MysqlExtractor {
    type Row = sqlx::mysql::MySqlRow;

    fn get_str(&self, row: &Self::Row, col: &str) -> Result<String, RepositoryError> {
        row.try_get::<String, _>(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })
    }
    fn get_opt_str(&self, row: &Self::Row, col: &str) -> Result<Option<String>, RepositoryError> {
        row.try_get::<Option<String>, _>(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })
    }
    fn get_uuid(&self, row: &Self::Row, col: &str) -> Result<Uuid, RepositoryError> {
        let s: String = row.try_get(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })?;
        Uuid::parse_str(&s).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })
    }
    fn get_opt_uuid(&self, row: &Self::Row, col: &str) -> Result<Option<Uuid>, RepositoryError> {
        let s: Option<String> = row.try_get(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })?;
        s.map(|s| Uuid::parse_str(&s).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })).transpose()
    }
    fn get_datetime(&self, row: &Self::Row, col: &str) -> Result<DateTime<Utc>, RepositoryError> {
        row.try_get::<DateTime<Utc>, _>(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })
    }
    fn get_opt_datetime(&self, row: &Self::Row, col: &str) -> Result<Option<DateTime<Utc>>, RepositoryError> {
        row.try_get::<Option<DateTime<Utc>>, _>(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })
    }
    fn get_bool(&self, row: &Self::Row, col: &str) -> Result<bool, RepositoryError> {
        let n: i64 = row.try_get(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })?;
        Ok(n != 0)
    }
    fn get_i32(&self, row: &Self::Row, col: &str) -> Result<i32, RepositoryError> {
        row.try_get::<i32, _>(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })
    }
    fn get_i64(&self, row: &Self::Row, col: &str) -> Result<i64, RepositoryError> {
        row.try_get::<i64, _>(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })
    }
    fn get_opt_i64(&self, row: &Self::Row, col: &str) -> Result<Option<i64>, RepositoryError> {
        row.try_get::<Option<i64>, _>(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })
    }
    fn get_f64(&self, row: &Self::Row, col: &str) -> Result<f64, RepositoryError> {
        row.try_get::<f64, _>(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })
    }
    fn get_opt_f64(&self, row: &Self::Row, col: &str) -> Result<Option<f64>, RepositoryError> {
        row.try_get::<Option<f64>, _>(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })
    }
    fn get_json_value(&self, row: &Self::Row, col: &str) -> Result<serde_json::Value, RepositoryError> {
        let s: String = row.try_get(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })?;
        serde_json::from_str(&s).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })
    }
    fn get_opt_json_value(&self, row: &Self::Row, col: &str) -> Result<Option<serde_json::Value>, RepositoryError> {
        let s: Option<String> = row.try_get(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })?;
        s.map(|s| serde_json::from_str(&s).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })).transpose()
    }
    fn get_string_set(&self, row: &Self::Row, col: &str) -> Result<HashSet<String>, RepositoryError> {
        let s: String = row.try_get(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })?;
        serde_json::from_str(&s).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })
    }
    fn get_opt_string_set(&self, row: &Self::Row, col: &str) -> Result<Option<HashSet<String>>, RepositoryError> {
        let s: Option<String> = row.try_get(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })?;
        s.map(|s| serde_json::from_str(&s).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })).transpose()
    }
    fn get_string_vec(&self, row: &Self::Row, col: &str) -> Result<Vec<String>, RepositoryError> {
        let s: String = row.try_get(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })?;
        serde_json::from_str(&s).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })
    }
    fn get_opt_string_vec(&self, row: &Self::Row, col: &str) -> Result<Option<Vec<String>>, RepositoryError> {
        let s: Option<String> = row.try_get(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })?;
        s.map(|s| serde_json::from_str(&s).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })).transpose()
    }
    fn get_bytes(&self, row: &Self::Row, col: &str) -> Result<Vec<u8>, RepositoryError> {
        row.try_get::<Vec<u8>, _>(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })
    }
    fn get_opt_bytes(&self, row: &Self::Row, col: &str) -> Result<Option<Vec<u8>>, RepositoryError> {
        row.try_get::<Option<Vec<u8>>, _>(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })
    }
}

/// Type alias — use `MySql` as the backend type parameter.
pub type MySql = MysqlBackend;

#[derive(Debug)]
pub struct MysqlBackend;

impl Backend for MysqlBackend {
    type Database = sqlx::MySql;
    type Adapter = MysqlBindAdapter;
    type Extractor = MysqlExtractor;
}

impl DbBound for MysqlBackend {}
