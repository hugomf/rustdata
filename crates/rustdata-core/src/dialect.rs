use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum SqlDialect {
    Postgres = 0,
    Sqlite = 1,
    MySql = 2,
    MsSql = 3,
}

pub(crate) const DIALECT_COUNT: usize = 4;

impl SqlDialect {
    pub fn ph(self, n: usize) -> String {
        match self {
            Self::Postgres => format!("${n}"),
            Self::Sqlite => "?".to_string(),
            Self::MySql => "?".to_string(),
            Self::MsSql => format!("@P{n}"),
        }
    }

    pub fn ph_list(self, count: usize) -> String {
        (1..=count)
            .map(|n| self.ph(n))
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn render(self, template: &str) -> String {
        let mut result = String::with_capacity(template.len() + 16);
        let chars: Vec<char> = template.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '{' {
                if let Some(offset) = chars[i + 1..].iter().position(|&c| c == '}') {
                    let num_str: String = chars[i + 1..i + 1 + offset].iter().collect();
                    if let Ok(n) = num_str.parse::<usize>() {
                        result.push_str(&self.ph(n));
                        i += 2 + offset;
                        continue;
                    }
                }
            }
            result.push(chars[i]);
            i += 1;
        }
        result
    }

    /// Wrap `sql` (which must already include an `ORDER BY` clause) with
    /// dialect-appropriate pagination syntax.
    ///
    /// The `ORDER BY` clause must be part of `sql` before calling this —
    /// use `select_sql` + `build_sort_clause` in the repo to produce it.
    pub fn render_pagination(&self, sql: &str, offset: i64, limit: i64) -> String {
        match self {
            Self::Postgres | Self::Sqlite | Self::MySql => {
                format!("{} LIMIT {} OFFSET {}", sql, limit, offset)
            }
            Self::MsSql => {
                format!(
                    "{} OFFSET {} ROWS FETCH NEXT {} ROWS ONLY",
                    sql, offset, limit
                )
            }
        }
    }

    pub fn render_filter(&self, column: &str, operator: &str, _value: &str) -> String {
        match operator {
            "eq" => format!("{} = {}", column, self.ph(1)),
            "ne" => format!("{} != {}", column, self.ph(1)),
            "lt" => format!("{} < {}", column, self.ph(1)),
            "lte" => format!("{} <= {}", column, self.ph(1)),
            "gt" => format!("{} > {}", column, self.ph(1)),
            "gte" => format!("{} >= {}", column, self.ph(1)),
            "like" => format!("{} LIKE {}", column, self.ph(1)),
            "contains" => format!("{} LIKE {}", column, self.ph(1)),
            "starts_with" => format!("{} LIKE {}", column, self.ph(1)),
            "ends_with" => format!("{} LIKE {}", column, self.ph(1)),
            "in" => format!("{} IN ({})", column, self.ph(1)),
            _ => format!("{} = {}", column, self.ph(1)),
        }
    }

    pub fn current_timestamp(self) -> &'static str {
        match self {
            Self::Postgres | Self::MySql => "NOW()",
            Self::Sqlite => "CURRENT_TIMESTAMP",
            Self::MsSql => "GETUTCDATE()",
        }
    }
}

pub struct SqlQuery {
    template: &'static str,
    mssql_override: Option<&'static str>,
    cache: OnceLock<[String; DIALECT_COUNT]>,
}

impl SqlQuery {
    pub const fn new(template: &'static str, mssql_override: Option<&'static str>) -> Self {
        Self {
            template,
            mssql_override,
            cache: OnceLock::new(),
        }
    }

    pub fn for_dialect(&self, dialect: SqlDialect) -> &str {
        let cache = self.cache.get_or_init(|| {
            let mssql_tpl = self.mssql_override.unwrap_or(self.template);
            [
                SqlDialect::Postgres.render(self.template),
                SqlDialect::Sqlite.render(self.template),
                SqlDialect::MySql.render(self.template),
                SqlDialect::MsSql.render(mssql_tpl),
            ]
        });
        &cache[dialect as usize]
    }
}

#[macro_export]
macro_rules! sql_query {
    ($template:expr) => {
        $crate::dialect::SqlQuery::new($template, None)
    };
}

#[macro_export]
macro_rules! sql_query_mssql {
    (default: $tpl:expr, mssql: $mssql:expr) => {
        $crate::dialect::SqlQuery::new($tpl, Some($mssql))
    };
}
