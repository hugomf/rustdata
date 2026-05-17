# CHANGELOG

## [0.1.3] - 2026-05-17

### Added

### Changed

### Fixed


## [0.1.2] - 2026-05-17

### Added

### Changed

### Fixed


## [0.1.1] - 2026-05-17

### Added

### Changed

### Fixed


All notable changes to the `rustdata-*` crates are documented here.

## [0.1.0] — 2025-xx-xx

### Added
- `rustdata-core` v0.1.0 — Spring Data-style CRUD repository for sqlx (PostgreSQL, SQLite, MySQL)
- `rustdata-macros` v0.1.0 — `#[derive(Entity)]`, `#[derive(QueryMethods)]`, `#[derive(Projection)]`, `#[derive(SqlType)]`
- `rustdata-migrations` v0.1.0 — SQL-DDL transpiler (SQLite and PostgreSQL dialects)
- Multi-dialect SQL transpilation engine (Dialect enum, type mappings, multi-pass transpiler)
- `#[derive(QueryMethods)]` — typed async query methods on `CrudRepository` and `QueryRepository`
  - `find_by_*_in()`, `exists_by_*`, `count_by_*`, `delete_by_*`
  - Null checks: `_is_null()`, `_is_not_null()`
  - Compound predicates: `_and_*`, `_or_*`
  - Paginated variants: `_paged()`
- Soft-delete support (`#[derive(SoftDeletable)]`)
- Specification / predicate pattern (`AndSpec`, `OrSpec`, `NotSpec`, `Predicate`)
- Pagination (`Page`, `Pageable`, `Sort`, `Filter`, `FilterOperator`)
- Projection / row-extraction support
- Lifecycle hooks (`BeforeSaveHook`, `AfterSaveHook`)
- Auto-generated ID column support (`#[entity(id)]` + `uuid` / `ULID`)
- Auto-generated timestamp columns (`#[entity(auto_generated)]`)

[0.1.0]: https://github.com/hugomf/rustdata/releases/tag/v0.1.0
