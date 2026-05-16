//! Backend implementations for each supported database.
//!
//! Each backend bundles three things:
//! - A `sqlx::Database` type (the wire protocol)
//! - A `BindAdapter` (type-safe parameter binding)
//! - A `RowExtractor` (type-safe column reading)
//!
//! ## Usage
//!
//! ```ignore
//! use rustdata_core::{CrudRepository, backends::Sqlite};
//! let repo = CrudRepository::<Sqlite, User>::new(pool);
//! ```
//!
//! ## Adding a new backend
//!
//! 1. Create `src/backends/my_db.rs`
//! 2. Implement `BindAdapter<MyDb>`, `RowExtractor`, `Backend`, `DbBound`
//! 3. Add `#[cfg(feature = "my_db")] pub mod my_db; pub use my_db::*;` here
//! 4. Add a `my_db = ["sqlx/my_db"]` feature in `Cargo.toml`

#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(feature = "sqlite")]
pub use sqlite::*;

#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(feature = "postgres")]
pub use postgres::*;

#[cfg(feature = "mysql")]
pub mod mysql;
#[cfg(feature = "mysql")]
pub use mysql::*;
