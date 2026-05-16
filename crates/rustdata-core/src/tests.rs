use crate::{
    ColumnDef, SqlDialect,
    column::{InsertStrategy, SqlTypeId, UpdateStrategy},
    query_methods,
    query_methods::QueryMethodParser,
    specification::{Predicate, SqlValue},
    pagination::{Filter, FilterOperator, Sort, Direction, Page, Pageable},
};

// ─── Predicate::to_sql ───────────────────────────────────────────────

#[test]
fn predicate_eq() {
    let p = Predicate::Eq { column: "email".into(), value: SqlValue::Str("a@b.com".into()) };
    let (sql, params, next) = p.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "email = $1");
    assert_eq!(params.len(), 1);
    assert_eq!(next, 2);
}

#[test]
fn predicate_eq_sqlite_placeholder() {
    let p = Predicate::Eq { column: "x".into(), value: SqlValue::I64(42) };
    let (sql, _, _) = p.to_sql(SqlDialect::Sqlite, 1);
    assert_eq!(sql, "x = ?");
}

#[test]
fn predicate_eq_mysql_placeholder() {
    let p = Predicate::Eq { column: "x".into(), value: SqlValue::I64(42) };
    let (sql, _, _) = p.to_sql(SqlDialect::MySql, 1);
    assert_eq!(sql, "x = ?");
}

#[test]
fn predicate_ne() {
    let p = Predicate::Ne { column: "status".into(), value: SqlValue::Str("archived".into()) };
    let (sql, _, _) = p.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "status <> $1");
}

#[test]
fn predicate_gt() {
    let p = Predicate::Gt { column: "age".into(), value: SqlValue::I64(18) };
    let (sql, _, next) = p.to_sql(SqlDialect::Postgres, 3);
    assert_eq!(sql, "age > $3");
    assert_eq!(next, 4);
}

#[test]
fn predicate_lt() {
    let p = Predicate::Lt { column: "price".into(), value: SqlValue::F64(100.0) };
    let (sql, _, _) = p.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "price < $1");
}

#[test]
fn predicate_gte() {
    let p = Predicate::Gte { column: "score".into(), value: SqlValue::I64(50) };
    let (sql, _, _) = p.to_sql(SqlDialect::Postgres, 2);
    assert_eq!(sql, "score >= $2");
}

#[test]
fn predicate_lte() {
    let p = Predicate::Lte { column: "level".into(), value: SqlValue::I64(10) };
    let (sql, _, _) = p.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "level <= $1");
}

#[test]
fn predicate_like() {
    let p = Predicate::Like { column: "name".into(), pattern: "%brew%".into() };
    let (sql, params, next) = p.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "name LIKE $1");
    assert_eq!(params, vec![SqlValue::Str("%brew%".into())]);
    assert_eq!(next, 2);
}

#[test]
fn predicate_in() {
    let p = Predicate::In {
        column: "id".into(),
        values: vec![SqlValue::I64(1), SqlValue::I64(2), SqlValue::I64(3)],
    };
    let (sql, params, next) = p.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "id IN ($1, $2, $3)");
    assert_eq!(params.len(), 3);
    assert_eq!(next, 4);
}

#[test]
fn predicate_between() {
    let p = Predicate::Between {
        column: "price".into(),
        low: SqlValue::I64(10),
        high: SqlValue::I64(100),
    };
    let (sql, params, next) = p.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "price BETWEEN $1 AND $2");
    assert_eq!(params.len(), 2);
    assert_eq!(next, 3);
}

#[test]
fn predicate_is_null() {
    let p = Predicate::IsNull { column: "deleted_at".into() };
    let (sql, params, next) = p.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "deleted_at IS NULL");
    assert!(params.is_empty());
    assert_eq!(next, 1);
}

#[test]
fn predicate_is_not_null() {
    let p = Predicate::IsNotNull { column: "email".into() };
    let (sql, _, next) = p.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "email IS NOT NULL");
    assert_eq!(next, 1);
}

#[test]
fn predicate_and() {
    let p = Predicate::And(vec![
        Predicate::Eq { column: "status".into(), value: SqlValue::Str("active".into()) },
        Predicate::Gt { column: "age".into(), value: SqlValue::I64(18) },
    ]);
    let (sql, params, next) = p.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "(status = $1 AND age > $2)");
    assert_eq!(params.len(), 2);
    assert_eq!(next, 3);
}

#[test]
fn predicate_and_nested() {
    let p = Predicate::And(vec![
        Predicate::And(vec![
            Predicate::Eq { column: "a".into(), value: SqlValue::I64(1) },
            Predicate::Eq { column: "b".into(), value: SqlValue::I64(2) },
        ]),
        Predicate::Eq { column: "c".into(), value: SqlValue::I64(3) },
    ]);
    let (sql, params, next) = p.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "((a = $1 AND b = $2) AND c = $3)");
    assert_eq!(params.len(), 3);
    assert_eq!(next, 4);
}

#[test]
fn predicate_or() {
    let p = Predicate::Or(vec![
        Predicate::Eq { column: "role".into(), value: SqlValue::Str("admin".into()) },
        Predicate::Eq { column: "role".into(), value: SqlValue::Str("mod".into()) },
    ]);
    let (sql, params, next) = p.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "(role = $1 OR role = $2)");
    assert_eq!(params.len(), 2);
    assert_eq!(next, 3);
}

#[test]
fn predicate_not() {
    let inner = Predicate::Eq { column: "deleted".into(), value: SqlValue::Bool(true) };
    let p = Predicate::Not(Box::new(inner));
    let (sql, params, next) = p.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "NOT (deleted = $1)");
    assert_eq!(params.len(), 1);
    assert_eq!(next, 2);
}

#[test]
fn predicate_raw() {
    let p = Predicate::Raw { sql: "EXTRACT(YEAR FROM created_at) = $1", params: vec![SqlValue::I64(2024)] };
    let (sql, params, next) = p.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "EXTRACT(YEAR FROM created_at) = $1");
    assert_eq!(params.len(), 1);
    assert_eq!(next, 2);
}

#[test]
fn predicate_none() {
    let p = Predicate::None;
    let (sql, params, next) = p.to_sql(SqlDialect::Postgres, 5);
    assert!(sql.is_empty());
    assert!(params.is_empty());
    assert_eq!(next, 5);
}

#[test]
fn predicate_offset_propagation() {
    let p = Predicate::And(vec![
        Predicate::Eq { column: "a".into(), value: SqlValue::I64(1) },
        Predicate::Eq { column: "b".into(), value: SqlValue::I64(2) },
    ]);
    let (sql, _, next) = p.to_sql(SqlDialect::Postgres, 10);
    assert_eq!(sql, "(a = $10 AND b = $11)");
    assert_eq!(next, 12);
}

// ─── QueryMethodParser ──────────────────────────────────────────────

#[test]
fn parse_single_field() {
    let parsed = QueryMethodParser::parse("find_by_email").unwrap();
    assert_eq!(parsed.conditions, vec![("email".to_string(), "eq".to_string())]);
    assert_eq!(parsed.conjunction, query_methods::Conjunction::And);
}

#[test]
fn parse_multi_field_and() {
    let parsed = QueryMethodParser::parse("find_by_organization_id_and_status").unwrap();
    assert_eq!(parsed.conditions, vec![
        ("organization_id".to_string(), "eq".to_string()),
        ("status".to_string(), "eq".to_string()),
    ]);
    assert_eq!(parsed.conjunction, query_methods::Conjunction::And);
}

#[test]
fn parse_multi_field_or() {
    let parsed = QueryMethodParser::parse("find_by_email_or_phone").unwrap();
    assert_eq!(parsed.conditions, vec![
        ("email".to_string(), "eq".to_string()),
        ("phone".to_string(), "eq".to_string()),
    ]);
    assert_eq!(parsed.conjunction, query_methods::Conjunction::Or);
}

#[test]
fn parse_with_operator_ne() {
    let parsed = QueryMethodParser::parse("find_by_email_ne").unwrap();
    assert_eq!(parsed.conditions, vec![("email".to_string(), "ne".to_string())]);
}

#[test]
fn parse_with_operator_gt() {
    let parsed = QueryMethodParser::parse("find_by_age_gt").unwrap();
    assert_eq!(parsed.conditions, vec![("age".to_string(), "gt".to_string())]);
}

#[test]
fn parse_with_operator_like() {
    let parsed = QueryMethodParser::parse("find_by_name_like").unwrap();
    assert_eq!(parsed.conditions, vec![("name".to_string(), "like".to_string())]);
}

#[test]
fn parse_with_operator_and_conjunction() {
    let parsed = QueryMethodParser::parse("find_by_email_ne_and_age_gt").unwrap();
    assert_eq!(parsed.conditions, vec![
        ("email".to_string(), "ne".to_string()),
        ("age".to_string(), "gt".to_string()),
    ]);
    assert_eq!(parsed.conjunction, query_methods::Conjunction::And);
}

#[test]
fn parse_invalid_prefix() {
    let result = QueryMethodParser::parse("findAll");
    assert!(result.is_err());
}

#[test]
fn parse_empty_suffix() {
    let parsed = QueryMethodParser::parse("find_by_").unwrap();
    assert!(parsed.conditions.is_empty());
}

#[test]
fn build_predicate_single() {
    let parsed = query_methods::ParsedQuery {
        conditions: vec![("email".to_string(), "eq".to_string())],
        conjunction: query_methods::Conjunction::And,
    };
    let values = vec![SqlValue::Str("a@b.com".into())];
    let pred = QueryMethodParser::build_predicate(parsed, values).unwrap();
    match pred {
        Predicate::Eq { column, .. } => assert_eq!(column, "email"),
        _ => panic!("Expected Eq"),
    }
}

#[test]
fn build_predicate_multi_and() {
    let parsed = query_methods::ParsedQuery {
        conditions: vec![
            ("email".to_string(), "eq".to_string()),
            ("status".to_string(), "eq".to_string()),
        ],
        conjunction: query_methods::Conjunction::And,
    };
    let values = vec![SqlValue::Str("a@b.com".into()), SqlValue::Str("active".into())];
    let pred = QueryMethodParser::build_predicate(parsed, values).unwrap();
    match pred {
        Predicate::And(preds) => assert_eq!(preds.len(), 2),
        _ => panic!("Expected And"),
    }
}

#[test]
fn build_predicate_multi_or() {
    let parsed = query_methods::ParsedQuery {
        conditions: vec![
            ("role".to_string(), "eq".to_string()),
            ("role".to_string(), "eq".to_string()),
        ],
        conjunction: query_methods::Conjunction::Or,
    };
    let values = vec![SqlValue::Str("admin".into()), SqlValue::Str("mod".into())];
    let pred = QueryMethodParser::build_predicate(parsed, values).unwrap();
    match pred {
        Predicate::Or(preds) => assert_eq!(preds.len(), 2),
        _ => panic!("Expected Or"),
    }
}

#[test]
fn build_predicate_mismatch_count() {
    let parsed = query_methods::ParsedQuery {
        conditions: vec![("email".to_string(), "eq".to_string())],
        conjunction: query_methods::Conjunction::And,
    };
    let values = vec![];
    let result = QueryMethodParser::build_predicate(parsed, values);
    assert!(result.is_err());
}

// ─── SqlDialect ──────────────────────────────────────────────────────

#[test]
fn ph_postgres() {
    assert_eq!(SqlDialect::Postgres.ph(1), "$1");
    assert_eq!(SqlDialect::Postgres.ph(10), "$10");
}

#[test]
fn ph_sqlite() {
    assert_eq!(SqlDialect::Sqlite.ph(1), "?");
    assert_eq!(SqlDialect::Sqlite.ph(5), "?");
}

#[test]
fn ph_mysql() {
    assert_eq!(SqlDialect::MySql.ph(1), "?");
}

#[test]
fn ph_mssql() {
    assert_eq!(SqlDialect::MsSql.ph(1), "@P1");
    assert_eq!(SqlDialect::MsSql.ph(3), "@P3");
}

#[test]
fn ph_list_postgres() {
    assert_eq!(SqlDialect::Postgres.ph_list(3), "$1, $2, $3");
}

#[test]
fn ph_list_sqlite() {
    assert_eq!(SqlDialect::Sqlite.ph_list(3), "?, ?, ?");
}

#[test]
fn ph_list_mssql() {
    assert_eq!(SqlDialect::MsSql.ph_list(2), "@P1, @P2");
}

#[test]
fn render_template_postgres() {
    let result = SqlDialect::Postgres.render("SELECT * FROM t WHERE id = {1} AND name = {2}");
    assert_eq!(result, "SELECT * FROM t WHERE id = $1 AND name = $2");
}

#[test]
fn render_template_sqlite() {
    let result = SqlDialect::Sqlite.render("SELECT * FROM t WHERE x = {1} AND y = {2}");
    assert_eq!(result, "SELECT * FROM t WHERE x = ? AND y = ?");
}

#[test]
fn render_template_mssql() {
    let result = SqlDialect::MsSql.render("SELECT * FROM t WHERE x = {1}");
    assert_eq!(result, "SELECT * FROM t WHERE x = @P1");
}

#[test]
fn render_template_no_placeholders() {
    let result = SqlDialect::Postgres.render("SELECT 1");
    assert_eq!(result, "SELECT 1");
}

#[test]
fn render_template_with_braces_not_placeholder() {
    let result = SqlDialect::Postgres.render("SELECT {1} AS val");
    assert_eq!(result, "SELECT $1 AS val");
}

#[test]
fn current_timestamp() {
    assert_eq!(SqlDialect::Postgres.current_timestamp(), "NOW()");
    assert_eq!(SqlDialect::Sqlite.current_timestamp(), "CURRENT_TIMESTAMP");
    assert_eq!(SqlDialect::MySql.current_timestamp(), "NOW()");
    assert_eq!(SqlDialect::MsSql.current_timestamp(), "GETUTCDATE()");
}

#[test]
fn render_pagination_limit_offset() {
    let sql = "SELECT * FROM users ORDER BY id ASC";
    let result = SqlDialect::Postgres.render_pagination(sql, 10, 20);
    assert_eq!(result, "SELECT * FROM users ORDER BY id ASC LIMIT 20 OFFSET 10");
}

#[test]
fn render_pagination_mssql() {
    let sql = "SELECT * FROM users ORDER BY id ASC";
    let result = SqlDialect::MsSql.render_pagination(sql, 10, 20);
    assert_eq!(result, "SELECT * FROM users ORDER BY id ASC OFFSET 10 ROWS FETCH NEXT 20 ROWS ONLY");
}

// ─── ColumnDef builder ──────────────────────────────────────────────

#[test]
fn column_def_new() {
    let c = ColumnDef::new("id", SqlTypeId::Uuid);
    assert_eq!(c.name, "id");
    assert_eq!(c.sql_type, SqlTypeId::Uuid);
    assert!(!c.nullable);
    assert!(!c.is_id);
    assert!(!c.is_json);
    assert_eq!(c.insert_strategy, InsertStrategy::Provided);
    assert_eq!(c.update_strategy, UpdateStrategy::Updatable);
}

#[test]
fn column_def_id_chain() {
    let c = ColumnDef::new("id", SqlTypeId::BigInt).id();
    assert!(c.is_id);
    assert_eq!(c.update_strategy, UpdateStrategy::Immutable);
}

#[test]
fn column_def_nullable() {
    let c = ColumnDef::new("email", SqlTypeId::Varchar).nullable();
    assert!(c.nullable);
}

#[test]
fn column_def_json() {
    let c = ColumnDef::new("data", SqlTypeId::Jsonb).json();
    assert!(c.is_json);
}

#[test]
fn column_def_insert_strategy() {
    let c = ColumnDef::new("ts", SqlTypeId::TimestampTz).insert(InsertStrategy::ServerTimestamp);
    assert_eq!(c.insert_strategy, InsertStrategy::ServerTimestamp);
}

#[test]
fn column_def_update_strategy() {
    let c = ColumnDef::new("ts", SqlTypeId::TimestampTz).update(UpdateStrategy::Immutable);
    assert_eq!(c.update_strategy, UpdateStrategy::Immutable);
}

#[test]
fn column_def_chained() {
    let c = ColumnDef::new("id", SqlTypeId::Uuid).id().json();
    assert!(c.is_id);
    assert!(c.is_json);
    assert_eq!(c.update_strategy, UpdateStrategy::Immutable);
}

#[test]
fn column_def_is_inserted() {
    let provided = ColumnDef::new("a", SqlTypeId::Varchar);
    let auto = ColumnDef::new("b", SqlTypeId::Varchar).insert(InsertStrategy::AutoGenerated);
    let server = ColumnDef::new("c", SqlTypeId::Varchar).insert(InsertStrategy::ServerTimestamp);
    assert!(provided.is_inserted());
    assert!(!auto.is_inserted());
    assert!(!server.is_inserted());
}

#[test]
fn column_def_is_updated() {
    let updatable = ColumnDef::new("a", SqlTypeId::Varchar);
    let immutable = ColumnDef::new("b", SqlTypeId::Varchar).update(UpdateStrategy::Immutable);
    assert!(updatable.is_updated());
    assert!(!immutable.is_updated());
}

// ─── sql_gen helpers ─────────────────────────────────────────────────

#[test]
fn sql_gen_select_cols() {
    let cols = vec![
        ColumnDef::new("id", SqlTypeId::Uuid).id(),
        ColumnDef::new("email", SqlTypeId::Varchar),
        ColumnDef::new("data", SqlTypeId::Jsonb).json(),
    ];
    assert_eq!(crate::column::sql_gen::select_cols(&cols), "id, email, data");
}

#[test]
fn sql_gen_insert_cols() {
    let cols = vec![
        ColumnDef::new("id", SqlTypeId::Uuid).id(),
        ColumnDef::new("email", SqlTypeId::Varchar),
        ColumnDef::new("ts", SqlTypeId::TimestampTz).insert(InsertStrategy::ServerTimestamp),
    ];
    assert_eq!(crate::column::sql_gen::insert_cols(&cols), "id, email");
}

#[test]
fn sql_gen_insert_param_count() {
    let cols = vec![
        ColumnDef::new("id", SqlTypeId::Uuid).id(),
        ColumnDef::new("email", SqlTypeId::Varchar),
        ColumnDef::new("ts", SqlTypeId::TimestampTz).insert(InsertStrategy::ServerTimestamp),
    ];
    assert_eq!(crate::column::sql_gen::insert_param_count(&cols), 2);
}

#[test]
fn sql_gen_update_set_postgres() {
    let cols = vec![
        ColumnDef::new("id", SqlTypeId::Uuid).id(),
        ColumnDef::new("email", SqlTypeId::Varchar),
        ColumnDef::new("name", SqlTypeId::Varchar),
    ];
    let result = crate::column::sql_gen::update_set(&cols, SqlDialect::Postgres);
    assert_eq!(result, "email = $1, name = $2");
}

#[test]
fn sql_gen_update_set_sqlite() {
    let cols = vec![
        ColumnDef::new("id", SqlTypeId::Uuid).id(),
        ColumnDef::new("email", SqlTypeId::Varchar),
    ];
    let result = crate::column::sql_gen::update_set(&cols, SqlDialect::Sqlite);
    assert_eq!(result, "email = ?");
}

#[test]
fn sql_gen_update_param_count() {
    let cols = vec![
        ColumnDef::new("id", SqlTypeId::Uuid).id(),
        ColumnDef::new("email", SqlTypeId::Varchar),
        ColumnDef::new("ts", SqlTypeId::TimestampTz).update(UpdateStrategy::Immutable),
    ];
    assert_eq!(crate::column::sql_gen::update_param_count(&cols), 1);
}

#[test]
fn sql_gen_default_order_by() {
    let cols = vec![
        ColumnDef::new("id", SqlTypeId::Uuid).id(),
        ColumnDef::new("email", SqlTypeId::Varchar),
    ];
    assert_eq!(crate::column::sql_gen::default_order_by(&cols), "id ASC");
}

#[test]
fn sql_gen_default_order_by_no_id() {
    let cols = vec![
        ColumnDef::new("email", SqlTypeId::Varchar),
    ];
    assert_eq!(crate::column::sql_gen::default_order_by(&cols), "1 ASC");
}

// ─── Filter ─────────────────────────────────────────────────────────

#[test]
fn filter_eq() {
    let f = Filter::new("status", FilterOperator::Eq, SqlValue::Str("active".into()));
    let (sql, params, next) = f.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "status = $1");
    assert_eq!(next, 2);
    assert_eq!(params.len(), 1);
}

#[test]
fn filter_ne() {
    let f = Filter::new("status", FilterOperator::Ne, SqlValue::Str("deleted".into()));
    let (sql, _, _) = f.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "status != $1");
}

#[test]
fn filter_is_null() {
    let f = Filter::new("deleted_at", FilterOperator::IsNull, SqlValue::Null(SqlTypeId::TimestampTz));
    let (sql, _, _) = f.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "deleted_at IS NULL");
}

#[test]
fn filter_like() {
    let f = Filter::new("name", FilterOperator::Like, SqlValue::Str("%brew%".into()));
    let (sql, _, _) = f.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "name LIKE $1");
}

#[test]
fn filter_contains() {
    let f = Filter::new("name", FilterOperator::Contains, SqlValue::Str("brew".into()));
    let (sql, params, _) = f.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "name LIKE $1");
    assert_eq!(params, vec![SqlValue::Str("%brew%".into())]);
}

#[test]
fn filter_starts_with() {
    let f = Filter::new("name", FilterOperator::StartsWith, SqlValue::Str("brew".into()));
    let (sql, params, _) = f.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "name LIKE $1");
    assert_eq!(params, vec![SqlValue::Str("brew%".into())]);
}

#[test]
fn filter_ends_with() {
    let f = Filter::new("name", FilterOperator::EndsWith, SqlValue::Str("brew".into()));
    let (sql, params, _) = f.to_sql(SqlDialect::Postgres, 1);
    assert_eq!(sql, "name LIKE $1");
    assert_eq!(params, vec![SqlValue::Str("%brew".into())]);
}

#[test]
fn filter_offset_propagation() {
    let f = Filter::new("x", FilterOperator::Eq, SqlValue::I64(1));
    let (sql, _, next) = f.to_sql(SqlDialect::Postgres, 5);
    assert_eq!(sql, "x = $5");
    assert_eq!(next, 6);
}

// ─── Sort ────────────────────────────────────────────────────────────

#[test]
fn sort_ascending() {
    let s = Sort::ascending("email");
    assert_eq!(s.to_sql(), "email ASC");
}

#[test]
fn sort_descending() {
    let s = Sort::descending("created_at");
    assert_eq!(s.to_sql(), "created_at DESC");
}

#[test]
fn sort_unsorted() {
    let s = Sort::unsorted();
    assert!(s.to_sql().is_empty());
}

#[test]
fn sort_multi_column() {
    let s = Sort::ascending("name").and(Sort::descending("age"));
    assert_eq!(s.to_sql(), "name ASC, age DESC");
}

#[test]
fn sort_is_sorted() {
    assert!(Sort::ascending("x").is_sorted());
    assert!(!Sort::unsorted().is_sorted());
}

// ─── Direction ───────────────────────────────────────────────────────

#[test]
fn direction_as_str() {
    assert_eq!(Direction::Asc.as_str(), "ASC");
    assert_eq!(Direction::Desc.as_str(), "DESC");
}

// ─── Page ────────────────────────────────────────────────────────────

#[test]
fn page_new() {
    let pageable = Pageable::of(0, 10);
    let page = Page::new(vec![1, 2, 3], 30, &pageable);
    assert_eq!(page.content, vec![1, 2, 3]);
    assert_eq!(page.total_elements, 30);
    assert_eq!(page.total_pages, 3);
    assert_eq!(page.page, 0);
    assert_eq!(page.size, 10);
}

#[test]
fn page_empty() {
    let pageable = Pageable::of(0, 10);
    let page: Page<i32> = Page::new(vec![], 0, &pageable);
    assert!(page.is_empty());
}

#[test]
fn page_map() {
    let pageable = Pageable::of(0, 10);
    let page = Page::new(vec![1, 2, 3], 3, &pageable);
    let mapped = page.map(|n| n * 2);
    assert_eq!(mapped.content, vec![2, 4, 6]);
}

#[test]
fn page_is_first() {
    let pageable = Pageable::of(0, 10);
    let page: Page<i32> = Page::new(vec![], 10, &pageable);
    assert!(page.is_first());
}

#[test]
fn page_is_last() {
    let pageable = Pageable::of(0, 1);
    let page: Page<i32> = Page::new(vec![], 1, &pageable);
    assert!(page.is_last());
}

#[test]
fn page_has_next() {
    let pageable = Pageable::of(0, 10);
    let page: Page<i32> = Page::new(vec![], 30, &pageable);
    assert!(page.has_next());
}

// ─── Pageable ────────────────────────────────────────────────────────

#[test]
fn pageable_default_page_zero() {
    let p = Pageable::default();
    assert_eq!(p.page, 0);
    assert_eq!(p.size, 20);
}

#[test]
fn pageable_offset() {
    let p = Pageable::of(2, 10);
    assert_eq!(p.offset(), 20);
}

#[test]
fn pageable_size_min_one() {
    let p = Pageable::of(0, 0);
    assert_eq!(p.size, 1);
}
