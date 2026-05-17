use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use std::collections::HashSet;
use uuid::Uuid;

use crate::error::RepositoryError;

pub trait RowExtractor: Send + Sync + Default + 'static {
    type Row: Send + Sync;

    fn get_str(&self, row: &Self::Row, col: &str) -> Result<String, RepositoryError>;
    fn get_opt_str(&self, row: &Self::Row, col: &str) -> Result<Option<String>, RepositoryError>;
    fn get_uuid(&self, row: &Self::Row, col: &str) -> Result<Uuid, RepositoryError>;
    fn get_opt_uuid(&self, row: &Self::Row, col: &str) -> Result<Option<Uuid>, RepositoryError>;
    fn get_datetime(&self, row: &Self::Row, col: &str) -> Result<DateTime<Utc>, RepositoryError>;
    fn get_opt_datetime(
        &self,
        row: &Self::Row,
        col: &str,
    ) -> Result<Option<DateTime<Utc>>, RepositoryError>;
    fn get_bool(&self, row: &Self::Row, col: &str) -> Result<bool, RepositoryError>;
    fn get_i32(&self, row: &Self::Row, col: &str) -> Result<i32, RepositoryError>;
    fn get_i64(&self, row: &Self::Row, col: &str) -> Result<i64, RepositoryError>;
    fn get_opt_i64(&self, row: &Self::Row, col: &str) -> Result<Option<i64>, RepositoryError>;
    fn get_f64(&self, row: &Self::Row, col: &str) -> Result<f64, RepositoryError>;
    fn get_opt_f64(&self, row: &Self::Row, col: &str) -> Result<Option<f64>, RepositoryError>;
    fn get_json_value(
        &self,
        row: &Self::Row,
        col: &str,
    ) -> Result<serde_json::Value, RepositoryError>;
    fn get_opt_json_value(
        &self,
        row: &Self::Row,
        col: &str,
    ) -> Result<Option<serde_json::Value>, RepositoryError>;
    fn get_string_set(
        &self,
        row: &Self::Row,
        col: &str,
    ) -> Result<HashSet<String>, RepositoryError>;
    fn get_opt_string_set(
        &self,
        row: &Self::Row,
        col: &str,
    ) -> Result<Option<HashSet<String>>, RepositoryError>;
    fn get_string_vec(&self, row: &Self::Row, col: &str) -> Result<Vec<String>, RepositoryError>;
    fn get_opt_string_vec(
        &self,
        row: &Self::Row,
        col: &str,
    ) -> Result<Option<Vec<String>>, RepositoryError>;
    fn get_bytes(&self, row: &Self::Row, col: &str) -> Result<Vec<u8>, RepositoryError>;
    fn get_opt_bytes(&self, row: &Self::Row, col: &str)
        -> Result<Option<Vec<u8>>, RepositoryError>;

    fn get_json<T: DeserializeOwned>(
        &self,
        row: &Self::Row,
        col: &str,
    ) -> Result<T, RepositoryError> {
        let v = self.get_json_value(row, col)?;
        serde_json::from_value(v).map_err(|e| RepositoryError::Deserialization(e.to_string()))
    }

    fn get_opt_json<T: DeserializeOwned>(
        &self,
        row: &Self::Row,
        col: &str,
    ) -> Result<Option<T>, RepositoryError> {
        match self.get_opt_json_value(row, col)? {
            None => Ok(None),
            Some(v) => serde_json::from_value(v)
                .map(Some)
                .map_err(|e| RepositoryError::Deserialization(e.to_string())),
        }
    }

    fn get_json_vec<T: DeserializeOwned>(
        &self,
        row: &Self::Row,
        col: &str,
    ) -> Result<Vec<T>, RepositoryError> {
        let v = self.get_json_value(row, col)?;
        serde_json::from_value(v).map_err(|e| RepositoryError::Deserialization(e.to_string()))
    }
}

/// Re-exported from `entity` module for backward compatibility.
/// The canonical location is `rustdata::entity::EntityDescriptor`.
pub use crate::entity::EntityDescriptor;
