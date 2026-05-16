use crate::{column::SqlTypeId, dialect::SqlDialect};

/// Convert a Rust value into a `SqlValue` for query binding.
///
/// The `null_type()` method returns the correct `SqlTypeId` when `Option<Self>`
/// is `None`, so the database receives a properly-typed NULL instead of an
/// incorrect `Null(Uuid)` fallback.
pub trait ToSqlValue {
    fn to_sql_value(self) -> SqlValue;

    /// The `SqlTypeId` used when `Option<Self>` is `None`.
    /// Override this in every concrete impl so nullable bindings are correct.
    fn null_type() -> SqlTypeId where Self: Sized {
        SqlTypeId::Text
    }
}

pub trait Specification<E>: Send + Sync {
    fn predicate(&self) -> Predicate;
}

impl<E> Specification<E> for Predicate {
    fn predicate(&self) -> Predicate {
        self.clone()
    }
}

#[derive(Debug, Clone)]
pub enum Predicate {
    Eq { column: String, value: SqlValue },
    Ne { column: String, value: SqlValue },
    In { column: String, values: Vec<SqlValue> },
    Between { column: String, low: SqlValue, high: SqlValue },
    Like { column: String, pattern: String },
    Gt { column: String, value: SqlValue },
    Lt { column: String, value: SqlValue },
    Gte { column: String, value: SqlValue },
    Lte { column: String, value: SqlValue },
    IsNull { column: String },
    IsNotNull { column: String },
    Not(Box<Predicate>),
    And(Vec<Predicate>),
    Or(Vec<Predicate>),
    Raw { sql: &'static str, params: Vec<SqlValue> },
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Uuid(uuid::Uuid),
    Str(String),
    OptStr(Option<String>),
    I64(i64),
    I32(i32),
    F32(f32),
    F64(f64),
    Bool(bool),
    DateTime(chrono::DateTime<chrono::Utc>),
    Json(serde_json::Value),
    Bytes(Vec<u8>),
    Null(SqlTypeId),
}

// ── ToSqlValue impls ─────────────────────────────────────────────────────────

impl ToSqlValue for String {
    fn to_sql_value(self) -> SqlValue { SqlValue::Str(self) }
    fn null_type() -> SqlTypeId { SqlTypeId::Varchar }
}

impl ToSqlValue for &str {
    fn to_sql_value(self) -> SqlValue { SqlValue::Str(self.to_string()) }
    fn null_type() -> SqlTypeId { SqlTypeId::Varchar }
}

impl ToSqlValue for uuid::Uuid {
    fn to_sql_value(self) -> SqlValue { SqlValue::Uuid(self) }
    fn null_type() -> SqlTypeId { SqlTypeId::Uuid }
}

impl ToSqlValue for i32 {
    fn to_sql_value(self) -> SqlValue { SqlValue::I32(self) }
    fn null_type() -> SqlTypeId { SqlTypeId::Int }
}

impl ToSqlValue for i64 {
    fn to_sql_value(self) -> SqlValue { SqlValue::I64(self) }
    fn null_type() -> SqlTypeId { SqlTypeId::BigInt }
}

impl ToSqlValue for f32 {
    fn to_sql_value(self) -> SqlValue { SqlValue::F32(self) }
    fn null_type() -> SqlTypeId { SqlTypeId::Float }
}

impl ToSqlValue for f64 {
    fn to_sql_value(self) -> SqlValue { SqlValue::F64(self) }
    fn null_type() -> SqlTypeId { SqlTypeId::Float }
}

impl ToSqlValue for bool {
    fn to_sql_value(self) -> SqlValue { SqlValue::Bool(self) }
    fn null_type() -> SqlTypeId { SqlTypeId::Boolean }
}

impl ToSqlValue for chrono::DateTime<chrono::Utc> {
    fn to_sql_value(self) -> SqlValue { SqlValue::DateTime(self) }
    fn null_type() -> SqlTypeId { SqlTypeId::TimestampTz }
}

impl ToSqlValue for serde_json::Value {
    fn to_sql_value(self) -> SqlValue { SqlValue::Json(self) }
    fn null_type() -> SqlTypeId { SqlTypeId::Jsonb }
}

impl ToSqlValue for Vec<u8> {
    fn to_sql_value(self) -> SqlValue { SqlValue::Bytes(self) }
    fn null_type() -> SqlTypeId { SqlTypeId::Bytes }
}

/// `Option<T>` uses `T::null_type()` so the database receives a correctly-typed
/// NULL (e.g. `Null(Int)` for `Option<i32>`, not the old wrong `Null(Uuid)`).
impl<T: ToSqlValue + Clone> ToSqlValue for Option<T> {
    fn to_sql_value(self) -> SqlValue {
        match self {
            Some(v) => v.to_sql_value(),
            None => SqlValue::Null(T::null_type()),
        }
    }
}

// ── Specification combinators ─────────────────────────────────────────────────

pub struct AndSpec<A, B>(pub A, pub B);
pub struct OrSpec<A, B>(pub A, pub B);
pub struct NotSpec<A>(pub A);

impl<E, A: Specification<E>, B: Specification<E>> Specification<E> for AndSpec<A, B> {
    fn predicate(&self) -> Predicate {
        Predicate::And(vec![self.0.predicate(), self.1.predicate()])
    }
}

impl<E, A: Specification<E>, B: Specification<E>> Specification<E> for OrSpec<A, B> {
    fn predicate(&self) -> Predicate {
        Predicate::Or(vec![self.0.predicate(), self.1.predicate()])
    }
}

impl<E, A: Specification<E>> Specification<E> for NotSpec<A> {
    fn predicate(&self) -> Predicate {
        Predicate::Not(Box::new(self.0.predicate()))
    }
}

// ── Predicate → SQL ───────────────────────────────────────────────────────────

impl Predicate {
    pub fn to_sql(
        &self,
        dialect: SqlDialect,
        param_offset: usize,
    ) -> (String, Vec<SqlValue>, usize) {
        match self {
            Predicate::None => (String::new(), Vec::new(), param_offset),
            Predicate::Eq { column, value } => {
                let ph = dialect.ph(param_offset);
                (format!("{} = {}", column, ph), vec![value.clone()], param_offset + 1)
            }
            Predicate::Ne { column, value } => {
                let ph = dialect.ph(param_offset);
                (format!("{} <> {}", column, ph), vec![value.clone()], param_offset + 1)
            }
            Predicate::IsNull { column } => {
                (format!("{} IS NULL", column), Vec::new(), param_offset)
            }
            Predicate::IsNotNull { column } => {
                (format!("{} IS NOT NULL", column), Vec::new(), param_offset)
            }
            Predicate::Gt { column, value } => {
                let ph = dialect.ph(param_offset);
                (format!("{} > {}", column, ph), vec![value.clone()], param_offset + 1)
            }
            Predicate::Lt { column, value } => {
                let ph = dialect.ph(param_offset);
                (format!("{} < {}", column, ph), vec![value.clone()], param_offset + 1)
            }
            Predicate::Gte { column, value } => {
                let ph = dialect.ph(param_offset);
                (format!("{} >= {}", column, ph), vec![value.clone()], param_offset + 1)
            }
            Predicate::Lte { column, value } => {
                let ph = dialect.ph(param_offset);
                (format!("{} <= {}", column, ph), vec![value.clone()], param_offset + 1)
            }
            Predicate::Like { column, pattern } => {
                let ph = dialect.ph(param_offset);
                (
                    format!("{} LIKE {}", column, ph),
                    vec![SqlValue::Str(pattern.clone())],
                    param_offset + 1,
                )
            }
            Predicate::In { column, values } => {
                let phs: Vec<String> = (0..values.len())
                    .map(|i| dialect.ph(param_offset + i))
                    .collect();
                (
                    format!("{} IN ({})", column, phs.join(", ")),
                    values.clone(),
                    param_offset + values.len(),
                )
            }
            Predicate::Between { column, low, high } => {
                let ph1 = dialect.ph(param_offset);
                let ph2 = dialect.ph(param_offset + 1);
                (
                    format!("{} BETWEEN {} AND {}", column, ph1, ph2),
                    vec![low.clone(), high.clone()],
                    param_offset + 2,
                )
            }
            Predicate::And(preds) => {
                let mut parts = Vec::new();
                let mut all_params = Vec::new();
                let mut offset = param_offset;
                for p in preds {
                    let (sql, params, new_offset) = p.to_sql(dialect, offset);
                    if !sql.is_empty() {
                        parts.push(sql);
                        all_params.extend(params);
                    }
                    offset = new_offset;
                }
                if parts.is_empty() {
                    return (String::new(), Vec::new(), param_offset);
                }
                (format!("({})", parts.join(" AND ")), all_params, offset)
            }
            Predicate::Or(preds) => {
                let mut parts = Vec::new();
                let mut all_params = Vec::new();
                let mut offset = param_offset;
                for p in preds {
                    let (sql, params, new_offset) = p.to_sql(dialect, offset);
                    if !sql.is_empty() {
                        parts.push(sql);
                        all_params.extend(params);
                    }
                    offset = new_offset;
                }
                if parts.is_empty() {
                    return (String::new(), Vec::new(), param_offset);
                }
                (format!("({})", parts.join(" OR ")), all_params, offset)
            }
            Predicate::Not(inner) => {
                let (sql, params, offset) = inner.to_sql(dialect, param_offset);
                (format!("NOT ({})", sql), params, offset)
            }
            Predicate::Raw { sql, params } => {
                (sql.to_string(), params.clone(), param_offset + params.len())
            }
        }
    }
}
