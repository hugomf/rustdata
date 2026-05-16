use crate::{
    backend::{Backend, DbBound, DbOf, AdOf, ExOf},
    bind::BindAdapter,
    column::SqlTypeId,
    descriptor::RowExtractor,
    dialect::SqlDialect,
    error::RepositoryError,
    specification::SqlValue,
};
use std::marker::PhantomData;

/// A trait for structs that can be extracted from a database row.
///
/// Implement via `#[derive(Entity)]` — the macro generates this automatically
/// by delegating to `EntityDescriptor::from_row`.  For custom projections or
/// view types, implement it manually.
pub trait RowExtractable: Sized {
    fn extract_row<E: RowExtractor>(row: &E::Row, extractor: &E) -> Result<Self, RepositoryError>;
}

/// A lightweight, query-only repository backed by a SQLx pool.
///
/// Uses `RowExtractable` instead of `EntityDescriptor` — no insert/update/delete.
/// Ideal for read-only lookups, views, aggregations, and projections.
#[derive(Debug)]
pub struct QueryRepository<BA: Backend, R: RowExtractable> {
    pub pool: sqlx::Pool<DbOf<BA>>,
    _ba: PhantomData<BA>,
    _r: PhantomData<R>,
}

impl<BA: Backend, R: RowExtractable> QueryRepository<BA, R> {
    pub fn new(pool: sqlx::Pool<DbOf<BA>>) -> Self {
        Self { pool, _ba: PhantomData, _r: PhantomData }
    }

    pub fn dialect(&self) -> SqlDialect {
        BA::dialect()
    }

    pub fn pool(&self) -> &sqlx::Pool<DbOf<BA>> {
        &self.pool
    }
}

impl<BA, R> QueryRepository<BA, R>
where
    BA: DbBound,
    R: RowExtractable,
    for<'q> <DbOf<BA> as sqlx::Database>::Arguments<'q>: sqlx::IntoArguments<'q, DbOf<BA>>,
    for<'c> &'c mut <DbOf<BA> as sqlx::Database>::Connection:
        sqlx::Executor<'c, Database = DbOf<BA>>,
    ExOf<BA>: RowExtractor<Row = <DbOf<BA> as sqlx::Database>::Row>,
{
    // ── Core SQL execution ───────────────────────────────────────────────────

    /// Execute a SQL query with params and return a single result, or None.
    ///
    /// **Note:** This method does NOT automatically append `LIMIT 1` to the
    /// SQL — callers are expected to include it in their query (e.g. via
    /// [`find_one_pred`](Self::find_one_pred)).
    pub async fn find_one_by_sql(
        &self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Option<R>, RepositoryError> {
        let rendered = self.dialect().render(sql);
        let query = bind_values::<BA>(sqlx::query::<DbOf<BA>>(&rendered), params);
        let row = query.fetch_optional(&self.pool).await.map_err(RepositoryError::from)?;
        match row {
            Some(r) => {
                let ext = <ExOf<BA> as Default>::default();
                Ok(Some(R::extract_row(&r, &ext)?))
            }
            None => Ok(None),
        }
    }

    /// Execute a SQL query with params and return all results.
    pub async fn find_all_by_sql(
        &self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<R>, RepositoryError> {
        let rendered = self.dialect().render(sql);
        let query = bind_values::<BA>(sqlx::query::<DbOf<BA>>(&rendered), params);
        let rows = query.fetch_all(&self.pool).await.map_err(RepositoryError::from)?;
        let ext = <ExOf<BA> as Default>::default();
        rows.iter().map(|r| R::extract_row(r, &ext)).collect()
    }

    /// Execute a SQL write statement. Returns rows affected.
    pub async fn execute_sql(
        &self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<u64, RepositoryError> {
        let rendered = self.dialect().render(sql);
        let result = bind_values::<BA>(sqlx::query::<DbOf<BA>>(&rendered), params)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::from)?;
        Ok(<AdOf<BA> as BindAdapter<DbOf<BA>>>::rows_affected(&result))
    }

    // ── Convenience helpers ──────────────────────────────────────────────────

    /// `SELECT * FROM table` — returns all rows.
    pub async fn find_all(&self, table: &str) -> Result<Vec<R>, RepositoryError> {
        let sql = format!("SELECT * FROM {}", table);
        self.find_all_by_sql(&sql, &[]).await
    }

    /// `SELECT * FROM table WHERE id = ?` — single result by typed id value.
    ///
    /// Pass the id as the appropriate `SqlValue` variant so Postgres receives
    /// a native UUID rather than a text cast:
    /// ```ignore
    /// repo.find_by_id("users", SqlValue::Uuid(some_uuid)).await?
    /// repo.find_by_id("users", SqlValue::I64(42)).await?
    /// repo.find_by_id("users", SqlValue::Str("slug".into())).await?
    /// ```
    pub async fn find_by_id(
        &self,
        table: &str,
        id: SqlValue,
    ) -> Result<Option<R>, RepositoryError> {
        let sql = format!("SELECT * FROM {} WHERE id = {} LIMIT 1", table, self.dialect().ph(1));
        self.find_one_by_sql(&sql, &[id]).await
    }

    /// Dynamic find_by: `WHERE <col> <op> $1 … $N`.
    ///
    /// `method_name` follows the same naming convention as
    /// `CrudRepository::find_by` (e.g. `"find_by_age_gt"`,
    /// `"find_by_status_and_email"`).
    /// Prefer the typed methods generated by `#[derive(QueryMethods)]`.
    pub async fn find_by(
        &self,
        table: &str,
        method_name: &str,
        values: Vec<SqlValue>,
    ) -> Result<Vec<R>, RepositoryError> {
        let parsed = crate::query_methods::QueryMethodParser::parse(method_name)?;
        let predicate = crate::query_methods::QueryMethodParser::build_predicate(parsed, values)?;
        self.find_all_pred(table, &predicate).await
    }

    /// Dynamic find-one: `WHERE <col> <op> $1 … $N`.
    pub async fn find_one_by(
        &self,
        table: &str,
        method_name: &str,
        values: Vec<SqlValue>,
    ) -> Result<Option<R>, RepositoryError> {
        let parsed = crate::query_methods::QueryMethodParser::parse(method_name)?;
        let predicate = crate::query_methods::QueryMethodParser::build_predicate(parsed, values)?;
        self.find_one_pred(table, &predicate).await
    }

    /// `SELECT * FROM table WHERE <predicate>` → all rows.
    ///
    /// # Soft-delete notice
    ///
    /// `QueryRepository` is schema-agnostic — it has no knowledge of
    /// `SOFT_DELETE_COL` and will therefore return soft-deleted rows.
    /// If your table uses soft-delete, add an explicit `IsNull` predicate:
    ///
    /// ```ignore
    /// use rustdata_core::specification::Predicate;
    /// let active = Predicate::And(vec![
    ///     your_predicate,
    ///     Predicate::IsNull { column: "deleted_at".into() },
    /// ]);
    /// repo.find_all_pred("users", &active).await?
    /// ```
    pub async fn find_all_pred(
        &self,
        table: &str,
        predicate: &crate::specification::Predicate,
    ) -> Result<Vec<R>, RepositoryError> {
        let (where_clause, params, _) = predicate.to_sql(self.dialect(), 1);
        let sql = if where_clause.is_empty() {
            format!("SELECT * FROM {}", table)
        } else {
            format!("SELECT * FROM {} WHERE {}", table, where_clause)
        };
        self.find_all_by_sql(&sql, &params).await
    }

    /// `SELECT * FROM table WHERE <predicate>` → single row.
    ///
    /// # Soft-delete notice
    ///
    /// See [`find_all_pred`](Self::find_all_pred) for the soft-delete caveat.
    pub async fn find_one_pred(
        &self,
        table: &str,
        predicate: &crate::specification::Predicate,
    ) -> Result<Option<R>, RepositoryError> {
        let (where_clause, params, _) = predicate.to_sql(self.dialect(), 1);
        let sql = if where_clause.is_empty() {
            format!("SELECT * FROM {} LIMIT 1", table)
        } else {
            format!("SELECT * FROM {} WHERE {} LIMIT 1", table, where_clause)
        };
        self.find_one_by_sql(&sql, &params).await
    }
}

/// Bind a slice of `SqlValue` params onto a query builder.
pub fn bind_values<'a, BA: Backend>(
    mut query: crate::bind::QueryBuilder<'a, DbOf<BA>>,
    params: &'a [SqlValue],
) -> crate::bind::QueryBuilder<'a, DbOf<BA>> {
    for v in params {
        query = match v {
            SqlValue::Uuid(u) => AdOf::<BA>::bind_uuid(query, *u),
            SqlValue::Str(s) => AdOf::<BA>::bind_str(query, s),
            SqlValue::OptStr(s) => AdOf::<BA>::bind_opt_str(query, s.as_ref().map(|s| s.as_str())),
            SqlValue::I64(i) => AdOf::<BA>::bind_int(query, *i),
            SqlValue::I32(i) => AdOf::<BA>::bind_int(query, *i as i64),
            SqlValue::F32(f) => AdOf::<BA>::bind_float(query, *f as f64),
            SqlValue::F64(f) => AdOf::<BA>::bind_float(query, *f),
            SqlValue::Bool(b) => AdOf::<BA>::bind_bool(query, *b),
            SqlValue::DateTime(d) => AdOf::<BA>::bind_datetime(query, *d),
            SqlValue::Json(v) => AdOf::<BA>::bind_json_value(query, v.clone()),
            SqlValue::Bytes(b) => AdOf::<BA>::bind_bytes(query, b.as_slice()),
            SqlValue::Null(tid) => match tid {
                SqlTypeId::Uuid => AdOf::<BA>::bind_opt_uuid(query, None),
                SqlTypeId::TimestampTz => AdOf::<BA>::bind_opt_datetime(query, None),
                SqlTypeId::Int | SqlTypeId::BigInt => AdOf::<BA>::bind_opt_int(query, None),
                SqlTypeId::Boolean => AdOf::<BA>::bind_opt_bool(query, None),
                SqlTypeId::Float => AdOf::<BA>::bind_opt_float(query, None),
                SqlTypeId::Bytes => AdOf::<BA>::bind_opt_bytes(query, None),
                SqlTypeId::Json | SqlTypeId::Jsonb => {
                    AdOf::<BA>::bind_opt_json::<serde_json::Value>(query, None)
                }
                _ => AdOf::<BA>::bind_opt_str(query, None),
            },
        };
    }
    query
}
