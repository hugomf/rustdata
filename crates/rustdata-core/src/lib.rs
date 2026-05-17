//! # rustdata-core
//!
//! Spring Data–style generic CRUD and query repositories for sqlx.
//!
//! ## Quick start
//!
//! ```ignore
//! use rustdata_core::{CrudRepository, QueryRepository, backends::Sqlite, Entity, QueryMethods};
//!
//! #[derive(Debug, Clone, Entity, QueryMethods)]
//! #[entity(table = "users", order_by = "created_at DESC")]
//! struct User {
//!     #[entity(id)]
//!     id: uuid::Uuid,
//!     username: String,
//!     age: i32,
//! }
//!
//! let repo = CrudRepository::<Sqlite, User>::new(pool);
//! let adults = repo.find_by_age_gt(21).await?;
//! ```
//!
//! ## Macro crate note
//!
//! `rustdata-macros` is a separate Rust crate because proc-macro crates must
//! compile for the build host, not the target.  From your perspective it is
//! invisible: everything is re-exported from this crate.  You only need to
//! add `rustdata-core` to your `Cargo.toml`.

pub mod backend;
pub mod bind;
pub mod column;
pub mod descriptor;
pub mod dialect;
pub mod entity;
pub mod error;
pub mod lifecycle;
pub mod pagination;
pub mod projection;
pub mod query_methods;
pub mod soft_delete;
pub mod specification;
pub mod sql_type;

pub mod backends;
#[cfg(feature = "migration")]
pub mod migration;

// ── DefaultBackend ────────────────────────────────────────────────────────────
//
// A concrete backend type alias resolved from the active Cargo feature.
// This is what the `#[derive(Entity)]` macro uses for the generated `*Repo`
// type alias, keeping feature-gate logic inside rustdata-core (where the
// features are actually declared) rather than inside the proc-macro crate
// (which has no backend features and would always pick the generic fallback).
#[cfg(feature = "sqlite")]
pub type DefaultBackend = backends::Sqlite;

#[cfg(all(feature = "postgres", not(feature = "sqlite")))]
pub type DefaultBackend = backends::Postgres;

#[cfg(all(feature = "mysql", not(feature = "sqlite"), not(feature = "postgres")))]
pub type DefaultBackend = backends::MySql;
pub mod repo;
pub mod row_extractable;

// ── Public API surface ────────────────────────────────────────────────────────

pub use backend::{Backend, DbBound};
pub use bind::BindAdapter;
pub use column::ColumnDef;
pub use descriptor::RowExtractor;
pub use dialect::{SqlDialect, SqlQuery};
pub use entity::EntityDescriptor;
pub use entity::EntityMetadata;
pub use error::{DbError, RepositoryError};
pub use pagination::{Direction, Filter, FilterOperator, Order, Page, Pageable, Sort};
pub use projection::Projection;
pub use repo::CrudRepository;
pub use row_extractable::{bind_values, QueryRepository, RowExtractable};
pub use soft_delete::SoftDeletable;
pub use specification::{AndSpec, NotSpec, OrSpec, Predicate, Specification, SqlValue, ToSqlValue};
pub use sql_type::{SqlBind, SqlExtract};

// Re-export derive macros so users only need `rustdata-core` in Cargo.toml.
// `rustdata-macros` is an implementation detail — never add it directly.
pub use rustdata_macros::{Entity, Projection, QueryMethods, SqlType};

/// Convenience prelude — import everything you need for typical usage.
///
/// ```ignore
/// use rustdata_core::prelude::*;
/// ```
///
/// This re-exports all public traits so generated `*CrudQueryMethods` and
/// `*QueryQueryMethods` traits are in scope without knowing their names.
/// Because Rust trait method resolution only works when the trait is in scope,
/// this is the recommended import style.
pub mod prelude {
    pub use crate::{
        backend::{Backend, DbBound},
        bind::BindAdapter,
        entity::EntityDescriptor,
        error::{DbError, RepositoryError},
        lifecycle::LifecycleHooks,
        pagination::{Direction, Filter, FilterOperator, Order, Page, Pageable, Sort},
        projection::Projection,
        repo::CrudRepository,
        row_extractable::{QueryRepository, RowExtractable},
        soft_delete::SoftDeletable,
        specification::{AndSpec, NotSpec, OrSpec, Predicate, Specification, SqlValue, ToSqlValue},
        sql_type::{SqlBind, SqlExtract},
        Entity, QueryMethods, SqlType,
    };
}

#[cfg(test)]
mod tests;
