use std::marker::PhantomData;

use crate::{
    backend::{Backend, DbBound, DbOf, AdOf, ExOf, RowOf},
    bind::BindAdapter,
    column::SqlTypeId,
    descriptor::RowExtractor,
    dialect::SqlDialect,
    entity::EntityDescriptor,
    error::RepositoryError,
    lifecycle::LifecycleHooks,
    pagination::{Page, Pageable, Filter},
    specification::{Predicate, SqlValue, Specification},
    query_methods::QueryMethodParser,
};

#[derive(Debug)]
pub struct CrudRepository<BA: Backend, D: EntityDescriptor> {
    pub pool: sqlx::Pool<DbOf<BA>>,
    _ba: PhantomData<BA>,
    _d: PhantomData<D>,
}

impl<BA: Backend, D: EntityDescriptor> CrudRepository<BA, D> {
    pub fn new(pool: sqlx::Pool<DbOf<BA>>) -> Self {
        Self { pool, _ba: PhantomData, _d: PhantomData }
    }

    pub fn dialect(&self) -> SqlDialect {
        BA::dialect()
    }
}

impl<BA, D> CrudRepository<BA, D>
where
    BA: DbBound,
    for<'q> <DbOf<BA> as sqlx::Database>::Arguments<'q>: sqlx::IntoArguments<'q, DbOf<BA>>,
    for<'c> &'c mut <DbOf<BA> as sqlx::Database>::Connection: sqlx::Executor<'c, Database = DbOf<BA>>,
    ExOf<BA>: RowExtractor<Row = <DbOf<BA> as sqlx::Database>::Row>,
    D: EntityDescriptor + LifecycleHooks<D::Entity>,
    D::Id: Clone,
{
    // ── SQL generation ───────────────────────────────────────────────────────

    fn insert_sql() -> String {
        let d = BA::dialect();
        let cols = D::insert_cols();
        let phs = d.ph_list(D::insert_param_count());
        format!("INSERT INTO {} ({}) VALUES ({})", D::TABLE, cols, phs)
    }

    fn find_by_id_sql() -> String {
        let d = BA::dialect();
        format!(
            "SELECT {} FROM {} WHERE {} = {}",
            D::select_cols(), D::TABLE, D::id_column().name, d.ph(1)
        )
    }

    fn update_sql() -> String {
        let d = BA::dialect();
        let set = D::update_set(d);
        let id_ph = d.ph(D::update_param_count() + 1);
        format!("UPDATE {} SET {} WHERE {} = {}", D::TABLE, set, D::id_column().name, id_ph)
    }

    fn delete_sql() -> String {
        let d = BA::dialect();
        format!("DELETE FROM {} WHERE {} = {}", D::TABLE, D::id_column().name, d.ph(1))
    }

    fn soft_delete_sql() -> String {
        let d = BA::dialect();
        let col = D::SOFT_DELETE_COL.expect("soft_delete_sql called but SOFT_DELETE_COL is None");
        format!(
            "UPDATE {} SET {} = {} WHERE {} = {} AND {} IS NULL",
            D::TABLE, col, d.current_timestamp(), D::id_column().name, d.ph(1), col
        )
    }

    fn build_where_clause(filters: &[Filter], dialect: SqlDialect) -> (String, Vec<SqlValue>) {
        let mut where_parts = Vec::new();
        let mut all_params = Vec::new();
        let mut offset = 1;
        for filter in filters {
            let (sql, params, new_offset) = filter.to_sql(dialect, offset);
            if !sql.is_empty() {
                where_parts.push(sql);
                all_params.extend(params);
            }
            offset = new_offset;
        }
        (where_parts.join(" AND "), all_params)
    }

    fn build_sort_clause(sort: &crate::pagination::Sort, default_order_by: &str) -> String {
        let sort_sql = sort.to_sql();
        if sort_sql.is_empty() {
            format!("ORDER BY {}", default_order_by)
        } else {
            format!("ORDER BY {}", sort_sql)
        }
    }

    fn count_sql(where_clause: &str) -> String {
        let base = format!("SELECT COUNT(*) as count FROM {}", D::TABLE);
        let soft = D::SOFT_DELETE_COL.map(|c| format!("{} IS NULL", c));
        if where_clause.is_empty() {
            soft.map(|s| format!("{} WHERE {}", base, s)).unwrap_or(base)
        } else if let Some(s) = soft {
            format!("{} WHERE {} AND ({})", base, s, where_clause)
        } else {
            format!("{} WHERE {}", base, where_clause)
        }
    }

    fn select_sql(where_clause: &str, sort_clause: &str) -> String {
        let base = format!("SELECT {} FROM {}", D::select_cols(), D::TABLE);
        let soft = D::SOFT_DELETE_COL.map(|c| format!("{} IS NULL", c));
        let from_where = if where_clause.is_empty() {
            soft.map(|s| format!("{} WHERE {}", base, s)).unwrap_or(base)
        } else if let Some(s) = soft {
            format!("{} WHERE {} AND ({})", base, s, where_clause)
        } else {
            format!("{} WHERE {}", base, where_clause)
        };
        format!("{} {}", from_where, sort_clause)
    }

    fn hydrate(row: &RowOf<BA>) -> Result<D::Entity, RepositoryError> {
        let ext = <ExOf<BA> as Default>::default();
        D::from_row::<ExOf<BA>>(row, &ext)
    }

    // ── Standard CRUD ────────────────────────────────────────────────────────

    pub async fn find_by_id(&self, id: D::Id) -> Result<Option<D::Entity>, RepositoryError> {
        let sql = Self::find_by_id_sql();
        let row = D::bind_id::<DbOf<BA>, AdOf<BA>>(sqlx::query(&sql), &id)
            .fetch_optional(&self.pool)
            .await
            .map_err(RepositoryError::from)?;
        match row {
            Some(r) => Ok(Some(Self::hydrate(&r)?)),
            None => Ok(None),
        }
    }

    /// Dynamic find_by — useful for runtime method name dispatch.
    /// Prefer the typed `find_by_<field>` methods generated by `#[derive(QueryMethods)]`.
    pub async fn find_by(
        &self,
        method_name: &str,
        values: Vec<SqlValue>,
    ) -> Result<Vec<D::Entity>, RepositoryError> {
        let parsed = QueryMethodParser::parse(method_name)?;
        let predicate = QueryMethodParser::build_predicate(parsed, values)?;
        self.find_all_pred(&predicate).await
    }

    pub async fn find_one_by(
        &self,
        method_name: &str,
        values: Vec<SqlValue>,
    ) -> Result<Option<D::Entity>, RepositoryError> {
        let parsed = QueryMethodParser::parse(method_name)?;
        let predicate = QueryMethodParser::build_predicate(parsed, values)?;
        self.find_one_pred(&predicate).await
    }

    pub async fn insert(&self, mut entity: D::Entity) -> Result<D::Entity, RepositoryError> {
        D::before_save(&mut entity)?;
        let sql = Self::insert_sql();
        D::bind_insert::<DbOf<BA>, AdOf<BA>>(sqlx::query(&sql), &entity)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::from)?;
        D::after_save(&entity)?;
        Ok(entity)
    }

    pub async fn update(&self, mut entity: D::Entity) -> Result<D::Entity, RepositoryError> {
        D::before_save(&mut entity)?;
        let sql = Self::update_sql();
        D::bind_update::<DbOf<BA>, AdOf<BA>>(sqlx::query(&sql), &entity)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::from)?;
        D::after_save(&entity)?;
        Ok(entity)
    }

    pub async fn delete(&self, id: &D::Id) -> Result<bool, RepositoryError> {
        let sql = if D::SOFT_DELETE_COL.is_some() {
            Self::soft_delete_sql()
        } else {
            Self::delete_sql()
        };
        let result = D::bind_id::<DbOf<BA>, AdOf<BA>>(sqlx::query(&sql), id)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::from)?;
        Ok(<AdOf<BA> as BindAdapter<DbOf<BA>>>::rows_affected(&result) > 0)
    }

    /// Hard-delete even when soft-delete is configured.
    pub async fn hard_delete(&self, id: &D::Id) -> Result<bool, RepositoryError> {
        let sql = Self::delete_sql();
        let result = D::bind_id::<DbOf<BA>, AdOf<BA>>(sqlx::query(&sql), id)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::from)?;
        Ok(<AdOf<BA> as BindAdapter<DbOf<BA>>>::rows_affected(&result) > 0)
    }

    pub async fn exists_by_id(&self, id: &D::Id) -> Result<bool, RepositoryError> {
        Ok(self.find_by_id(id.clone()).await?.is_some())
    }

    pub async fn count(&self) -> Result<u64, RepositoryError> {
        let ext = <ExOf<BA> as Default>::default();
        let row = sqlx::query(&Self::count_sql(""))
            .fetch_one(&self.pool)
            .await
            .map_err(RepositoryError::from)?;
        ext.get_i64(&row, "count").map(|n| n as u64)
    }

    pub async fn count_with_filters(&self, filters: &[Filter]) -> Result<u64, RepositoryError> {
        let (where_clause, params) = Self::build_where_clause(filters, self.dialect());
        let sql = Self::count_sql(&where_clause);
        let query = Self::bind_sql_values(sqlx::query(&sql), &params);
        let row = query.fetch_one(&self.pool).await.map_err(RepositoryError::from)?;
        let ext = <ExOf<BA> as Default>::default();
        ext.get_i64(&row, "count").map(|n| n as u64)
    }

    pub async fn list(&self, pageable: &Pageable) -> Result<Page<D::Entity>, RepositoryError> {
        let (where_clause, params) = Self::build_where_clause(&pageable.filters, self.dialect());
        let sort_clause = Self::build_sort_clause(&pageable.sort, D::ORDER_BY);
        let select_sql = Self::select_sql(&where_clause, &sort_clause);
        let paginated_sql = self.dialect().render_pagination(
            &select_sql,
            "",
            pageable.offset() as i64,
            pageable.size as i64,
        );
        let query = Self::bind_sql_values(sqlx::query(&paginated_sql), &params);
        let rows = query.fetch_all(&self.pool).await.map_err(RepositoryError::from)?;
        let content: Result<Vec<_>, _> = rows.iter().map(|r| Self::hydrate(r)).collect();
        let total = self.count_with_filters(&pageable.filters).await?;
        Ok(Page::new(content?, total, pageable))
    }

    /// Return all rows — soft-delete guard is applied when configured.
    ///
    /// Internally routes through `find_all_pred(&Predicate::None)` so that
    /// `SOFT_DELETE_COL IS NULL` is always appended, matching the behaviour
    /// of `list`, `find_all_spec`, etc.
    pub async fn find_all(&self) -> Result<Vec<D::Entity>, RepositoryError> {
        self.find_all_pred(&Predicate::None).await
    }

    pub async fn find_all_sorted(
        &self,
        sort: &crate::pagination::Sort,
    ) -> Result<Vec<D::Entity>, RepositoryError> {
        let sort_clause = Self::build_sort_clause(sort, D::ORDER_BY);
        let sql = Self::select_sql("", &sort_clause);
        let rows = sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(RepositoryError::from)?;
        rows.iter().map(|r| Self::hydrate(r)).collect()
    }

    pub async fn find_all_pred(
        &self,
        predicate: &Predicate,
    ) -> Result<Vec<D::Entity>, RepositoryError> {
        let (where_clause, params, _) = predicate.to_sql(self.dialect(), 1);
        let sort_clause = format!("ORDER BY {}", D::ORDER_BY);
        let sql = Self::select_sql(&where_clause, &sort_clause);
        let query = Self::bind_sql_values(sqlx::query(&sql), &params);
        let rows = query.fetch_all(&self.pool).await.map_err(RepositoryError::from)?;
        rows.iter().map(|r| Self::hydrate(r)).collect()
    }

    /// Like `find_all_pred`, but returns a paginated `Page<T>`.
    ///
    /// Prefer this over `find_all_pred` when the result set could be large.
    pub async fn find_all_pred_paged(
        &self,
        predicate: &Predicate,
        pageable: &Pageable,
    ) -> Result<Page<D::Entity>, RepositoryError> {
        let (where_clause, params, _) = predicate.to_sql(self.dialect(), 1);

        let count_sql = Self::count_sql(&where_clause);
        let count_query = Self::bind_sql_values(sqlx::query(&count_sql), &params);
        let count_row = count_query.fetch_one(&self.pool).await.map_err(RepositoryError::from)?;
        let ext = <ExOf<BA> as Default>::default();
        let total = ext.get_i64(&count_row, "count")? as u64;

        let sort_clause = Self::build_sort_clause(&pageable.sort, D::ORDER_BY);
        let select_sql = Self::select_sql(&where_clause, &sort_clause);
        let paginated_sql = self.dialect().render_pagination(
            &select_sql,
            "",
            pageable.offset() as i64,
            pageable.size as i64,
        );
        let query = Self::bind_sql_values(sqlx::query(&paginated_sql), &params);
        let rows = query.fetch_all(&self.pool).await.map_err(RepositoryError::from)?;
        let content: Result<Vec<_>, _> = rows.iter().map(|r| Self::hydrate(r)).collect();
        Ok(Page::new(content?, total, pageable))
    }

    pub async fn find_one_pred(
        &self,
        predicate: &Predicate,
    ) -> Result<Option<D::Entity>, RepositoryError> {
        let (where_clause, params, _) = predicate.to_sql(self.dialect(), 1);
        let sort_clause = format!("ORDER BY {}", D::ORDER_BY);
        let sql = Self::select_sql(&where_clause, &sort_clause);
        let query = Self::bind_sql_values(sqlx::query(&sql), &params);
        let row = query.fetch_optional(&self.pool).await.map_err(RepositoryError::from)?;
        match row {
            Some(r) => Ok(Some(Self::hydrate(&r)?)),
            None => Ok(None),
        }
    }

    pub async fn count_pred(
        &self,
        predicate: &Predicate,
    ) -> Result<u64, RepositoryError> {
        let (where_clause, params, _) = predicate.to_sql(self.dialect(), 1);
        let sql = Self::count_sql(&where_clause);
        let query = Self::bind_sql_values(sqlx::query(&sql), &params);
        let row = query.fetch_one(&self.pool).await.map_err(RepositoryError::from)?;
        let ext = <ExOf<BA> as Default>::default();
        ext.get_i64(&row, "count").map(|n| n as u64)
    }

    /// Delete all rows matching a predicate. Respects soft-delete when configured.
    pub async fn delete_pred(
        &self,
        predicate: &Predicate,
    ) -> Result<u64, RepositoryError> {
        let (where_clause, params, _) = predicate.to_sql(self.dialect(), 1);
        let sql = if let Some(col) = D::SOFT_DELETE_COL {
            let soft_where = if where_clause.is_empty() {
                format!("{} IS NULL", col)
            } else {
                format!("{} IS NULL AND ({})", col, where_clause)
            };
            format!(
                "UPDATE {} SET {} = {} WHERE {}",
                D::TABLE,
                col,
                BA::dialect().current_timestamp(),
                soft_where
            )
        } else if where_clause.is_empty() {
            format!("DELETE FROM {}", D::TABLE)
        } else {
            format!("DELETE FROM {} WHERE {}", D::TABLE, where_clause)
        };
        let result = Self::bind_sql_values(sqlx::query(&sql), &params)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::from)?;
        Ok(<AdOf<BA> as BindAdapter<DbOf<BA>>>::rows_affected(&result))
    }

    pub async fn find_one_spec(
        &self,
        spec: &dyn Specification<D::Entity>,
    ) -> Result<Option<D::Entity>, RepositoryError> {
        self.find_one_pred(&spec.predicate()).await
    }

    pub async fn find_all_spec(
        &self,
        spec: &dyn Specification<D::Entity>,
        pageable: &Pageable,
    ) -> Result<Page<D::Entity>, RepositoryError> {
        let predicate = spec.predicate();
        let (where_clause, params, _) = predicate.to_sql(self.dialect(), 1);

        let count_sql = Self::count_sql(&where_clause);
        let count_query = Self::bind_sql_values(sqlx::query(&count_sql), &params);
        let count_row = count_query.fetch_one(&self.pool).await.map_err(RepositoryError::from)?;
        let ext = <ExOf<BA> as Default>::default();
        let total = ext.get_i64(&count_row, "count")? as u64;

        let sort_clause = Self::build_sort_clause(&pageable.sort, D::ORDER_BY);
        let select_sql = Self::select_sql(&where_clause, &sort_clause);
        let paginated_sql = self.dialect().render_pagination(
            &select_sql,
            "",
            pageable.offset() as i64,
            pageable.size as i64,
        );
        let query = Self::bind_sql_values(sqlx::query(&paginated_sql), &params);
        let rows = query.fetch_all(&self.pool).await.map_err(RepositoryError::from)?;
        let content: Result<Vec<_>, _> = rows.iter().map(|r| Self::hydrate(r)).collect();
        Ok(Page::new(content?, total, pageable))
    }

    pub async fn count_spec(
        &self,
        spec: &dyn Specification<D::Entity>,
    ) -> Result<u64, RepositoryError> {
        let predicate = spec.predicate();
        let (where_clause, params, _) = predicate.to_sql(self.dialect(), 1);
        let sql = Self::count_sql(&where_clause);
        let query = Self::bind_sql_values(sqlx::query(&sql), &params);
        let row = query.fetch_one(&self.pool).await.map_err(RepositoryError::from)?;
        let ext = <ExOf<BA> as Default>::default();
        ext.get_i64(&row, "count").map(|n| n as u64)
    }

    pub async fn exists_spec(
        &self,
        spec: &dyn Specification<D::Entity>,
    ) -> Result<bool, RepositoryError> {
        self.count_spec(spec).await.map(|c| c > 0)
    }

    pub async fn insert_batch(
        &self,
        entities: Vec<D::Entity>,
    ) -> Result<Vec<D::Entity>, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(RepositoryError::from)?;
        let mut results = Vec::with_capacity(entities.len());
        for mut entity in entities {
            D::before_save(&mut entity)?;
            let sql = Self::insert_sql();
            D::bind_insert::<DbOf<BA>, AdOf<BA>>(sqlx::query(&sql), &entity)
                .execute(&mut *tx)
                .await
                .map_err(RepositoryError::from)?;
            D::after_save(&entity)?;
            results.push(entity);
        }
        tx.commit().await.map_err(RepositoryError::from)?;
        Ok(results)
    }

    pub async fn clear(&self) -> Result<(), RepositoryError> {
        let sql = match self.dialect() {
            SqlDialect::Postgres => format!("TRUNCATE TABLE {} CASCADE", D::TABLE),
            _ => format!("DELETE FROM {}", D::TABLE),
        };
        sqlx::query(&sql).execute(&self.pool).await.map_err(RepositoryError::from)?;
        Ok(())
    }

    // ── Custom SQL ───────────────────────────────────────────────────────────

    pub async fn find_one_by_sql(
        &self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Option<D::Entity>, RepositoryError> {
        let rendered = self.dialect().render(sql);
        let query = Self::bind_sql_values(sqlx::query(&rendered), params);
        let row = query.fetch_optional(&self.pool).await.map_err(RepositoryError::from)?;
        match row {
            Some(r) => Ok(Some(Self::hydrate(&r)?)),
            None => Ok(None),
        }
    }

    pub async fn find_many_by_sql(
        &self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<D::Entity>, RepositoryError> {
        let rendered = self.dialect().render(sql);
        let query = Self::bind_sql_values(sqlx::query(&rendered), params);
        let rows = query.fetch_all(&self.pool).await.map_err(RepositoryError::from)?;
        rows.iter().map(|r| Self::hydrate(r)).collect()
    }

    pub async fn execute_sql(
        &self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<u64, RepositoryError> {
        let rendered = self.dialect().render(sql);
        let result = Self::bind_sql_values(sqlx::query(&rendered), params)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::from)?;
        Ok(<AdOf<BA> as BindAdapter<DbOf<BA>>>::rows_affected(&result))
    }

    // ── Internal helper ───────────────────────────────────────────────────────

    fn bind_sql_values<'q>(
        query: crate::bind::QueryBuilder<'q, DbOf<BA>>,
        params: &'q [SqlValue],
    ) -> crate::bind::QueryBuilder<'q, DbOf<BA>> {
        params.iter().fold(query, |q, v| match v {
            SqlValue::Uuid(u) => <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_uuid(q, *u),
            SqlValue::Str(s) => <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_str(q, s),
            SqlValue::I64(i) => <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_int(q, *i),
            SqlValue::I32(i) => <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_int(q, *i as i64),
            SqlValue::F32(f) => <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_float(q, *f as f64),
            SqlValue::F64(f) => <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_float(q, *f),
            SqlValue::Bool(b) => <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_bool(q, *b),
            SqlValue::DateTime(d) => <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_datetime(q, *d),
            SqlValue::Json(v) => <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_json_value(q, v.clone()),
            SqlValue::Bytes(b) => <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_bytes(q, b.as_slice()),
            SqlValue::OptStr(s) => <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_opt_str(
                q,
                s.as_ref().map(|s| s.as_str()),
            ),
            SqlValue::Null(tid) => match tid {
                SqlTypeId::Uuid => <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_opt_uuid(q, None),
                SqlTypeId::TimestampTz => {
                    <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_opt_datetime(q, None)
                }
                SqlTypeId::Int | SqlTypeId::BigInt => {
                    <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_opt_int(q, None)
                }
                SqlTypeId::Boolean => <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_opt_bool(q, None),
                SqlTypeId::Float => <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_opt_float(q, None),
                SqlTypeId::Bytes => <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_opt_bytes(q, None),
                SqlTypeId::Json | SqlTypeId::Jsonb => {
                    <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_opt_json::<serde_json::Value>(q, None)
                }
                _ => <AdOf<BA> as BindAdapter<DbOf<BA>>>::bind_opt_str(q, None),
            },
        })
    }
}
