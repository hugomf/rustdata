use crate::{
    bind::{BindAdapter, QueryBuilder},
    column::ColumnDef,
    descriptor::RowExtractor,
    dialect::SqlDialect,
    error::RepositoryError,
};

/// Metadata trait for entities that don't need the full EntityDescriptor.
///
/// Provides basic table and column metadata without bind/extract methods.
/// Useful for projections, views, and read-only queries.
pub trait EntityMetadata: Send + Sync + 'static {
    type Entity: Clone + Send + Sync + 'static;

    const TABLE: &'static str;

    fn columns() -> &'static [ColumnDef];
}

/// Declarative metadata + bind/extract logic for a single entity.
///
/// Do NOT implement this manually. Use `#[derive(Entity)]`.
/// The macro generates all constants, bind methods, and from_row.
pub trait EntityDescriptor: Send + Sync + 'static {
    type Entity: Clone + Send + Sync + 'static;
    type Id: Clone + Send + Sync + 'static;

    const TABLE: &'static str;
    const ORDER_BY: &'static str;
    const SOFT_DELETE_COL: Option<&'static str> = None;

    fn columns() -> &'static [ColumnDef];

    fn id_column() -> &'static ColumnDef {
        Self::columns()
            .iter()
            .find(|c| c.is_id)
            .expect("Entity must have an id column")
    }

    fn select_cols() -> String {
        crate::column::sql_gen::select_cols(Self::columns())
    }

    fn insert_cols() -> String {
        crate::column::sql_gen::insert_cols(Self::columns())
    }

    fn insert_param_count() -> usize {
        crate::column::sql_gen::insert_param_count(Self::columns())
    }

    fn update_set(dialect: SqlDialect) -> String {
        crate::column::sql_gen::update_set(Self::columns(), dialect)
    }

    fn update_param_count() -> usize {
        crate::column::sql_gen::update_param_count(Self::columns())
    }

    fn bind_insert<'q, DB, B>(
        query: QueryBuilder<'q, DB>,
        entity: &'q Self::Entity,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>;

    fn bind_update<'q, DB, B>(
        query: QueryBuilder<'q, DB>,
        entity: &'q Self::Entity,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>;

    fn bind_id<'q, DB, B>(
        query: QueryBuilder<'q, DB>,
        id: &'q Self::Id,
    ) -> QueryBuilder<'q, DB>
    where
        DB: sqlx::Database,
        B: BindAdapter<DB>;

    fn from_row<E: RowExtractor>(
        row: &E::Row,
        ext: &E,
    ) -> Result<Self::Entity, RepositoryError>;
}
