use chrono::{DateTime, Utc};
use std::collections::HashSet;
use uuid::Uuid;

use crate::{
    bind::{BindAdapter, QueryBuilder},
    descriptor::RowExtractor,
    error::RepositoryError,
};

pub trait SqlBind: Sized {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        value: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>;
}

pub trait SqlExtract: Sized {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError>;
}

impl SqlBind for String {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        B::bind_str(q, v.as_str())
    }
}

impl SqlExtract for String {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_str(row, col)
    }
}

impl SqlBind for Option<String> {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        B::bind_opt_str(q, v.as_deref())
    }
}

impl SqlExtract for Option<String> {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_opt_str(row, col)
    }
}

impl SqlBind for Uuid {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        B::bind_uuid(q, *v)
    }
}

impl SqlExtract for Uuid {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_uuid(row, col)
    }
}

impl SqlBind for Option<Uuid> {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        B::bind_opt_uuid(q, *v)
    }
}

impl SqlExtract for Option<Uuid> {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_opt_uuid(row, col)
    }
}

impl SqlBind for DateTime<Utc> {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        B::bind_datetime(q, *v)
    }
}

impl SqlExtract for DateTime<Utc> {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_datetime(row, col)
    }
}

impl SqlBind for Option<DateTime<Utc>> {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        B::bind_opt_datetime(q, *v)
    }
}

impl SqlExtract for Option<DateTime<Utc>> {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_opt_datetime(row, col)
    }
}

impl SqlBind for bool {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        B::bind_bool(q, *v)
    }
}

impl SqlExtract for bool {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_bool(row, col)
    }
}

impl SqlBind for i64 {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        B::bind_int(q, *v)
    }
}

impl SqlExtract for i64 {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_i64(row, col)
    }
}

impl SqlBind for Option<i64> {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        B::bind_opt_int(q, *v)
    }
}

impl SqlExtract for Option<i64> {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_opt_i64(row, col)
    }
}

impl SqlBind for i32 {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        B::bind_int(q, *v as i64)
    }
}

impl SqlExtract for i32 {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_i32(row, col)
    }
}

impl SqlBind for u32 {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        B::bind_int(q, *v as i64)
    }
}

impl SqlExtract for u32 {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_i32(row, col).map(|n| n as u32)
    }
}

impl SqlBind for HashSet<String> {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        let json = serde_json::to_value(v).unwrap_or_default();
        B::bind_json_value(q, json)
    }
}

impl SqlExtract for HashSet<String> {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_string_set(row, col)
    }
}

impl SqlBind for Vec<String> {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        let json = serde_json::to_value(v).unwrap_or_default();
        B::bind_json_value(q, json)
    }
}

impl SqlExtract for Vec<String> {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_string_vec(row, col)
    }
}

impl SqlBind for f32 {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        B::bind_float(q, *v as f64)
    }
}

impl SqlExtract for f32 {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_f64(row, col).map(|n| n as f32)
    }
}

impl SqlBind for Option<f32> {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        B::bind_opt_float(q, v.map(|n| n as f64))
    }
}

impl SqlExtract for Option<f32> {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_opt_f64(row, col).map(|opt| opt.map(|n| n as f32))
    }
}

impl SqlBind for f64 {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        B::bind_float(q, *v)
    }
}

impl SqlExtract for f64 {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_f64(row, col)
    }
}

impl SqlBind for Option<f64> {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        B::bind_opt_float(q, *v)
    }
}

impl SqlExtract for Option<f64> {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_opt_f64(row, col)
    }
}

impl SqlBind for serde_json::Value {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        B::bind_json_value(q, v.clone())
    }
}

impl SqlExtract for serde_json::Value {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_json_value(row, col)
    }
}

impl SqlBind for Option<serde_json::Value> {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        match v {
            Some(val) => B::bind_json_value(q, val.clone()),
            None => B::bind_json_value(q, serde_json::Value::Null),
        }
    }
}

impl SqlExtract for Option<serde_json::Value> {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_opt_json_value(row, col)
    }
}

impl SqlBind for Option<HashSet<String>> {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        match v {
            Some(set) => {
                let json = serde_json::to_value(set).unwrap_or_default();
                B::bind_json_value(q, json)
            }
            None => B::bind_json_value(q, serde_json::Value::Null),
        }
    }
}

impl SqlExtract for Option<HashSet<String>> {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_opt_string_set(row, col)
    }
}

impl SqlBind for Vec<u8> {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        B::bind_bytes(q, v.as_slice())
    }
}

impl SqlExtract for Vec<u8> {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_bytes(row, col)
    }
}

impl SqlBind for Option<Vec<u8>> {
    fn sql_bind<'q, DB, B>(
        q: QueryBuilder<'q, DB>,
        v: &'q Self,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>,
    {
        B::bind_opt_bytes(q, v.as_deref())
    }
}

impl SqlExtract for Option<Vec<u8>> {
    fn sql_extract<E: RowExtractor>(
        ext: &E,
        row: &E::Row,
        col: &str,
    ) -> Result<Self, RepositoryError> {
        ext.get_opt_bytes(row, col)
    }
}
