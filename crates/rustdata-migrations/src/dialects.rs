/// Supported SQL dialects
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialect {
    Postgres,
    Sqlite,
    MySql,
    MsSql,
    Oracle,
}

impl Dialect {
    pub fn all() -> &'static [Dialect] {
        &[Self::Postgres, Self::Sqlite, Self::MySql, Self::MsSql, Self::Oracle]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Sqlite => "sqlite",
            Self::MySql => "mysql",
            Self::MsSql => "mssql",
            Self::Oracle => "oracle",
        }
    }
}

/// Complete type mapping table derived from all 105 migration files.
/// Maps canonical (Postgres-ish) type names to each dialect's equivalent.
#[derive(Debug, Clone)]
pub struct TypeMapping {
    pub canonical: &'static str,
    pub postgres: &'static str,
    pub sqlite: &'static str,
    pub mysql: &'static str,
    pub mssql: &'static str,
    pub oracle: &'static str,
}

pub const TYPE_MAP: &[TypeMapping] = &[
    // UUID — for PKs and FKs
    TypeMapping {
        canonical: "UUID",
        postgres: "UUID",
        sqlite: "TEXT",
        mysql: "VARCHAR(36)",
        mssql: "UNIQUEIDENTIFIER",
        oracle: "RAW(16)",
    },
    // JSONB — all JSON columns
    TypeMapping {
        canonical: "JSONB",
        postgres: "JSONB",
        sqlite: "TEXT",
        mysql: "JSON",
        mssql: "NVARCHAR(MAX)",
        oracle: "CLOB",
    },
    // TIMESTAMPTZ — all timestamp columns
    TypeMapping {
        canonical: "TIMESTAMPTZ",
        postgres: "TIMESTAMPTZ",
        sqlite: "TEXT",
        mysql: "TEXT",
        mssql: "DATETIME2",
        oracle: "TIMESTAMP WITH TIME ZONE",
    },
    // BOOLEAN — all boolean columns
    TypeMapping {
        canonical: "BOOLEAN",
        postgres: "BOOLEAN",
        sqlite: "INTEGER",
        mysql: "BOOLEAN",
        mssql: "BIT",
        oracle: "NUMBER(1)",
    },
    // TEXT — unbounded strings
    TypeMapping {
        canonical: "TEXT",
        postgres: "TEXT",
        sqlite: "TEXT",
        mysql: "TEXT",
        mssql: "NVARCHAR(MAX)",
        oracle: "CLOB",
    },
    // BIGINT
    TypeMapping {
        canonical: "BIGINT",
        postgres: "BIGINT",
        sqlite: "INTEGER",
        mysql: "BIGINT",
        mssql: "BIGINT",
        oracle: "INTEGER",
    },
    // INTEGER
    TypeMapping {
        canonical: "INTEGER",
        postgres: "INTEGER",
        sqlite: "INTEGER",
        mysql: "INTEGER",
        mssql: "INT",
        oracle: "INTEGER",
    },
];
