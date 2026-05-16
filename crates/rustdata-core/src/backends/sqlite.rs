//! SQLite backend — in-memory or file-based.
//!
//! UUIDs are stored as text. Booleans are stored as INTEGER (0/1).
//! JSON is stored as TEXT. Datetimes are RFC3339 strings.

use chrono::{DateTime, Utc};
use sqlx::Row;
use std::collections::HashSet;
use uuid::Uuid;

use crate::bind::QueryBuilder;
use crate::{Backend, BindAdapter, DbBound, RepositoryError, RowExtractor, SqlDialect};

pub struct SqliteBindAdapter;

impl BindAdapter<sqlx::Sqlite> for SqliteBindAdapter {
    fn dialect() -> SqlDialect { SqlDialect::Sqlite }

    fn bind_uuid<'q>(q: QueryBuilder<'q, sqlx::Sqlite>, v: Uuid) -> QueryBuilder<'q, sqlx::Sqlite> {
        q.bind(v.to_string())
    }
    fn bind_opt_uuid<'q>(q: QueryBuilder<'q, sqlx::Sqlite>, v: Option<Uuid>) -> QueryBuilder<'q, sqlx::Sqlite> {
        q.bind(v.map(|v| v.to_string()))
    }
    fn bind_str<'q>(q: QueryBuilder<'q, sqlx::Sqlite>, v: &'q str) -> QueryBuilder<'q, sqlx::Sqlite> {
        q.bind(v)
    }
    fn bind_opt_str<'q>(q: QueryBuilder<'q, sqlx::Sqlite>, v: Option<&'q str>) -> QueryBuilder<'q, sqlx::Sqlite> {
        q.bind(v)
    }
    fn bind_int<'q>(q: QueryBuilder<'q, sqlx::Sqlite>, v: i64) -> QueryBuilder<'q, sqlx::Sqlite> {
        q.bind(v)
    }
    fn bind_opt_int<'q>(q: QueryBuilder<'q, sqlx::Sqlite>, v: Option<i64>) -> QueryBuilder<'q, sqlx::Sqlite> {
        q.bind(v)
    }
    fn bind_bool<'q>(q: QueryBuilder<'q, sqlx::Sqlite>, v: bool) -> QueryBuilder<'q, sqlx::Sqlite> {
        q.bind(v as i64)
    }
    fn bind_opt_bool<'q>(q: QueryBuilder<'q, sqlx::Sqlite>, v: Option<bool>) -> QueryBuilder<'q, sqlx::Sqlite> {
        q.bind(v.map(|b| b as i64))
    }
    fn bind_datetime<'q>(q: QueryBuilder<'q, sqlx::Sqlite>, v: DateTime<Utc>) -> QueryBuilder<'q, sqlx::Sqlite> {
        q.bind(v.to_rfc3339())
    }
    fn bind_opt_datetime<'q>(q: QueryBuilder<'q, sqlx::Sqlite>, v: Option<DateTime<Utc>>) -> QueryBuilder<'q, sqlx::Sqlite> {
        q.bind(v.map(|v| v.to_rfc3339()))
    }
    fn bind_json<'q, T: serde::Serialize>(q: QueryBuilder<'q, sqlx::Sqlite>, v: &'q T) -> QueryBuilder<'q, sqlx::Sqlite> {
        q.bind(serde_json::to_string(v).unwrap_or_else(|_| "null".into()))
    }
    fn bind_opt_json<'q, T: serde::Serialize>(q: QueryBuilder<'q, sqlx::Sqlite>, v: Option<&'q T>) -> QueryBuilder<'q, sqlx::Sqlite> {
        q.bind(v.map(|v| serde_json::to_string(v).unwrap_or_else(|_| "null".into())))
    }
    fn bind_json_value<'q>(q: QueryBuilder<'q, sqlx::Sqlite>, v: serde_json::Value) -> QueryBuilder<'q, sqlx::Sqlite> {
        q.bind(serde_json::to_string(&v).unwrap_or_else(|_| "null".into()))
    }
    fn bind_float<'q>(q: QueryBuilder<'q, sqlx::Sqlite>, v: f64) -> QueryBuilder<'q, sqlx::Sqlite> {
        q.bind(v)
    }
    fn bind_opt_float<'q>(q: QueryBuilder<'q, sqlx::Sqlite>, v: Option<f64>) -> QueryBuilder<'q, sqlx::Sqlite> {
        q.bind(v)
    }
    fn bind_bytes<'q>(q: QueryBuilder<'q, sqlx::Sqlite>, v: &'q [u8]) -> QueryBuilder<'q, sqlx::Sqlite> {
        q.bind(v)
    }
    fn bind_opt_bytes<'q>(q: QueryBuilder<'q, sqlx::Sqlite>, v: Option<&'q [u8]>) -> QueryBuilder<'q, sqlx::Sqlite> {
        q.bind(v)
    }
    fn rows_affected(result: &<sqlx::Sqlite as sqlx::Database>::QueryResult) -> u64 {
        result.rows_affected()
    }
}

#[derive(Default)]
pub struct SqliteExtractor;

impl RowExtractor for SqliteExtractor {
    type Row = sqlx::sqlite::SqliteRow;

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
        let s: String = row.try_get(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })?;
        // Try RFC3339 first, fall back to SQLite's bare datetime format
        DateTime::parse_from_rfc3339(&s)
            .map(|dt| dt.with_timezone(&Utc))
            .or_else(|_| {
                chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S")
                    .map(|ndt| ndt.and_utc())
                    .map_err(|_| std::fmt::Error)
            })
            .map_err(|_| RepositoryError::Extraction { column: col.into(), reason: format!("Failed to parse datetime: {}", s) })
    }
    fn get_opt_datetime(&self, row: &Self::Row, col: &str) -> Result<Option<DateTime<Utc>>, RepositoryError> {
        let s: Option<String> = row.try_get(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })?;
        s.map(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .or_else(|_| {
                    chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S")
                        .map(|ndt| ndt.and_utc())
                        .map_err(|_| std::fmt::Error)
                })
                .map_err(|_| RepositoryError::Extraction { column: col.into(), reason: format!("Failed to parse datetime: {}", s) })
        }).transpose()
    }
    fn get_bool(&self, row: &Self::Row, col: &str) -> Result<bool, RepositoryError> {
        let n: i64 = row.try_get(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })?;
        Ok(n != 0)
    }
    fn get_i32(&self, row: &Self::Row, col: &str) -> Result<i32, RepositoryError> {
        let n: i64 = row.try_get(col).map_err(|e| RepositoryError::Extraction { column: col.into(), reason: e.to_string() })?;
        Ok(n as i32)
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

/// Type alias — use `Sqlite` as the backend type parameter.
pub type Sqlite = SqliteBackend;

#[derive(Debug)]
pub struct SqliteBackend;

impl Backend for SqliteBackend {
    type Database = sqlx::Sqlite;
    type Adapter = SqliteBindAdapter;
    type Extractor = SqliteExtractor;
}

impl DbBound for SqliteBackend {}
