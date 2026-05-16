use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::dialect::SqlDialect;

pub type QueryBuilder<'q, DB> =
    sqlx::query::Query<'q, DB, <DB as sqlx::Database>::Arguments<'q>>;

pub trait BindAdapter<DB: sqlx::Database>: Sized + Send + Sync + 'static {
    fn dialect() -> SqlDialect;

    fn bind_uuid<'q>(q: QueryBuilder<'q, DB>, v: Uuid) -> QueryBuilder<'q, DB>;
    fn bind_opt_uuid<'q>(q: QueryBuilder<'q, DB>, v: Option<Uuid>) -> QueryBuilder<'q, DB>;
    fn bind_str<'q>(q: QueryBuilder<'q, DB>, v: &'q str) -> QueryBuilder<'q, DB>;
    fn bind_opt_str<'q>(q: QueryBuilder<'q, DB>, v: Option<&'q str>) -> QueryBuilder<'q, DB>;
    fn bind_int<'q>(q: QueryBuilder<'q, DB>, v: i64) -> QueryBuilder<'q, DB>;
    fn bind_opt_int<'q>(q: QueryBuilder<'q, DB>, v: Option<i64>) -> QueryBuilder<'q, DB>;
    fn bind_bool<'q>(q: QueryBuilder<'q, DB>, v: bool) -> QueryBuilder<'q, DB>;
    fn bind_opt_bool<'q>(q: QueryBuilder<'q, DB>, v: Option<bool>) -> QueryBuilder<'q, DB>;
    fn bind_datetime<'q>(q: QueryBuilder<'q, DB>, v: DateTime<Utc>) -> QueryBuilder<'q, DB>;
    fn bind_opt_datetime<'q>(
        q: QueryBuilder<'q, DB>,
        v: Option<DateTime<Utc>>,
    ) -> QueryBuilder<'q, DB>;
    fn bind_json<'q, T: serde::Serialize>(
        q: QueryBuilder<'q, DB>,
        v: &'q T,
    ) -> QueryBuilder<'q, DB>;
    fn bind_opt_json<'q, T: serde::Serialize>(
        q: QueryBuilder<'q, DB>,
        v: Option<&'q T>,
    ) -> QueryBuilder<'q, DB>;
    fn bind_json_value<'q>(
        q: QueryBuilder<'q, DB>,
        v: serde_json::Value,
    ) -> QueryBuilder<'q, DB>;
    fn bind_float<'q>(q: QueryBuilder<'q, DB>, v: f64) -> QueryBuilder<'q, DB>;
    fn bind_opt_float<'q>(q: QueryBuilder<'q, DB>, v: Option<f64>) -> QueryBuilder<'q, DB>;
    fn bind_bytes<'q>(q: QueryBuilder<'q, DB>, v: &'q [u8]) -> QueryBuilder<'q, DB>;
    fn bind_opt_bytes<'q>(q: QueryBuilder<'q, DB>, v: Option<&'q [u8]>) -> QueryBuilder<'q, DB>;
    fn rows_affected(result: &DB::QueryResult) -> u64;
}
