use crate::dialects::{Dialect, TypeMapping, TYPE_MAP};

/// Transpile canonical (Postgres-ish) SQL to a target dialect.
/// Uses annotation-aware multi-pass token substitution.
pub struct Transpiler {
    dialect: Dialect,
}

/// Output of a transpile operation.
pub struct TranspileOutput {
    pub sql: String,
    pub warnings: Vec<String>,
}

/// Errors during transpilation.
#[derive(Debug)]
pub enum TranspileError {
    UnknownDialect(String),
    InvalidAnnotation(String),
}

impl std::fmt::Display for TranspileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownDialect(d) => write!(f, "Unknown dialect: {}", d),
            Self::InvalidAnnotation(a) => write!(f, "Invalid annotation: {}", a),
        }
    }
}

impl std::error::Error for TranspileError {}

impl Transpiler {
    pub fn new(dialect: Dialect) -> Self {
        Self { dialect }
    }

    /// Transpile a canonical migration SQL string to the target dialect.
    /// Returns the transpiled SQL and a list of warnings.
    pub fn transpile(&self, source: &str) -> Result<TranspileOutput, TranspileError> {
        let mut output_lines: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        let mut in_dialect_block: Option<Dialect> = None;
        let mut current_block_matches = false;

        for line in source.lines() {
            let trimmed = line.trim();

            // Annotation: -- @end_dialect — close dialect-specific block
            if trimmed == "-- @end_dialect" {
                in_dialect_block = None;
                current_block_matches = false;
                continue;
            }

            // Annotation: -- @dialect X — start dialect-specific block
            if let Some(rest) = trimmed.strip_prefix("-- @dialect ") {
                let target_str = rest.trim();
                if target_str.ends_with("_only") {
                    let base = target_str.trim_end_matches("_only");
                    let d = parse_dialect(base)?;
                    in_dialect_block = Some(d);
                    current_block_matches = d == self.dialect;
                    continue;
                } else {
                    let d = parse_dialect(target_str)?;
                    in_dialect_block = Some(d);
                    current_block_matches = d == self.dialect;
                    continue;
                }
            }

            // Inside a dialect block — only emit if it matches target dialect
            if in_dialect_block.is_some() {
                if current_block_matches {
                    output_lines.push(line.to_string());
                }
                continue;
            }

            // Skip migration/description/col annotations
            if trimmed.starts_with("-- @migration")
                || trimmed.starts_with("-- @description")
                || trimmed.starts_with("-- @col ")
            {
                continue;
            }

            // Apply transformation passes to regular lines
            let transformed = self.apply_passes(line, &mut warnings);
            output_lines.push(transformed);
        }

        let mut sql = output_lines.join("\n").trim_end().to_string();
        sql.push('\n'); // ensure consistent trailing newline

        // Oracle post-processing: wrap CREATE TABLE statements in PL/SQL blocks
        if self.dialect == Dialect::Oracle {
            sql = post_process_oracle_create_table(&sql);
        }

        Ok(TranspileOutput { sql, warnings })
    }

    fn apply_passes(&self, line: &str, warnings: &mut Vec<String>) -> String {
        let line = self.pass_type_substitution(line, warnings);
        let line = self.pass_default_substitution(&line, warnings);
        let line = self.pass_varchar_substitution(&line, warnings);
        let line = self.pass_structural(&line, warnings);
        line
    }

    /// Pass 2: Type name substitution (UUID → TEXT, JSONB → NVARCHAR(MAX), etc.)
    fn pass_type_substitution(&self, line: &str, _warnings: &mut Vec<String>) -> String {
        let trimmed = line.trim();
        if trimmed.starts_with("--") {
            return line.to_string();
        }

        let mut result = line.to_string();
        for mapping in TYPE_MAP {
            if result.contains(mapping.canonical) {
                let target = dialect_type(self.dialect, mapping);
                if self.is_standalone_token(&result, mapping.canonical) {
                    result = result.replace(mapping.canonical, target);
                }
            }
        }
        result
    }

    /// Check if a token appears as a standalone word (not as part of another word)
    fn is_standalone_token(&self, text: &str, token: &str) -> bool {
        if text.len() < token.len() {
            return false;
        }
        let mut found = false;
        let mut search_start = 0;
        while let Some(pos) = text[search_start..].find(token) {
            let abs_pos = search_start + pos;
            let before = abs_pos
                .checked_sub(1)
                .map(|i| text.as_bytes()[i])
                .unwrap_or(b' ');
            let after = text
                .as_bytes()
                .get(abs_pos + token.len())
                .copied()
                .unwrap_or(b' ');
            let is_start = !before.is_ascii_alphanumeric() && before != b'_';
            let is_end = !after.is_ascii_alphanumeric() && after != b'_';
            if is_start && is_end {
                found = true;
            }
            search_start = abs_pos + 1;
            if search_start >= text.len() {
                break;
            }
        }
        found
    }

    /// Pass: VARCHAR(n) → NVARCHAR(n) (MSSQL) or VARCHAR2(n) (Oracle)
    fn pass_varchar_substitution(&self, line: &str, _warnings: &mut Vec<String>) -> String {
        match self.dialect {
            Dialect::Sqlite => {
                let mut result = String::with_capacity(line.len());
                let mut i = 0;
                let bytes = line.as_bytes();
                while i < line.len() {
                    if i + 7 < line.len()
                        && bytes[i..i + 7].eq_ignore_ascii_case(b"VARCHAR")
                        && (i == 0 || !bytes[i - 1].is_ascii_alphabetic())
                        && (i + 7 >= line.len() || !bytes[i + 7].is_ascii_alphabetic())
                    {
                        result.push_str("TEXT");
                        i += 7;
                        if i < line.len() && bytes[i] == b' ' {
                            i += 1;
                        }
                        if i < line.len() && bytes[i] == b'(' {
                            while i < line.len() && bytes[i] != b')' {
                                i += 1;
                            }
                            if i < line.len() {
                                i += 1;
                            }
                        }
                    } else {
                        result.push(bytes[i] as char);
                        i += 1;
                    }
                }
                result
            }
            Dialect::MsSql => {
                let mut result = String::with_capacity(line.len());
                let mut i = 0;
                let bytes = line.as_bytes();
                while i < line.len() {
                    if i + 7 < line.len()
                        && bytes[i..i + 7].eq_ignore_ascii_case(b"VARCHAR")
                        && (i == 0 || !bytes[i - 1].is_ascii_alphabetic())
                        && (i + 7 >= line.len() || !bytes[i + 7].is_ascii_alphabetic())
                    {
                        let prev_is_n = i > 0 && bytes[i - 1].eq_ignore_ascii_case(&b'N');
                        if !prev_is_n {
                            result.push_str("NVARCHAR");
                            i += 7;
                        } else {
                            result.push(bytes[i] as char);
                            i += 1;
                        }
                    } else {
                        result.push(bytes[i] as char);
                        i += 1;
                    }
                }
                result
            }
            Dialect::Oracle => line.replace("VARCHAR(", "VARCHAR2("),
            _ => line.to_string(),
        }
    }

    /// Pass: DEFAULT values substitution
    fn pass_default_substitution(&self, line: &str, _warnings: &mut Vec<String>) -> String {
        let mut result = line.to_string();

        match self.dialect {
            Dialect::Sqlite => {
                result = result.replace("DEFAULT NOW()", "DEFAULT (datetime('now'))");
            }
            Dialect::MsSql => {
                result = result.replace("DEFAULT NOW()", "DEFAULT GETDATE()");
            }
            Dialect::Oracle => {
                result = result.replace("DEFAULT NOW()", "DEFAULT CURRENT_TIMESTAMP");
            }
            _ => {}
        }

        match self.dialect {
            Dialect::Sqlite => {
                result = result.replace(
                    "DEFAULT gen_random_uuid()",
                    "DEFAULT (lower(hex(randomblob(16))))",
                );
            }
            Dialect::MySql => {
                result = result.replace("DEFAULT gen_random_uuid()", "DEFAULT (UUID())");
            }
            Dialect::MsSql => {
                result = result.replace("DEFAULT gen_random_uuid()", "DEFAULT NEWID()");
            }
            Dialect::Oracle => {
                result = result.replace(" DEFAULT gen_random_uuid()", "");
                result = result.replace("DEFAULT gen_random_uuid() ", "");
                result = result.replace("DEFAULT gen_random_uuid()", "");
            }
            Dialect::Postgres => {}
        }

        match self.dialect {
            Dialect::Sqlite | Dialect::MsSql | Dialect::Oracle => {
                result = result.replace(" DEFAULT TRUE", " DEFAULT 1");
                result = result.replace(" DEFAULT FALSE", " DEFAULT 0");
                result = result.replace(" DEFAULT true", " DEFAULT 1");
                result = result.replace(" DEFAULT false", " DEFAULT 0");
            }
            _ => {}
        }

        result
    }

    /// Pass: Structural rewrite rules (CREATE TABLE guards, ADD COLUMN, etc.)
    fn pass_structural(&self, line: &str, _warnings: &mut Vec<String>) -> String {
        let trimmed = line.trim_start();

        if matches!(
            self.dialect,
            Dialect::Postgres | Dialect::Sqlite | Dialect::MySql
        ) {
            if trimmed.starts_with("CREATE TABLE ") && !trimmed.contains("IF NOT EXISTS") {
                return line.replacen("CREATE TABLE ", "CREATE TABLE IF NOT EXISTS ", 1);
            }
        }

        if self.dialect == Dialect::MsSql && trimmed.starts_with("CREATE TABLE IF NOT EXISTS ") {
            let table_name = extract_table_name(trimmed);
            let inner = line.replacen("CREATE TABLE IF NOT EXISTS ", "CREATE TABLE ", 1);
            return format!(
                "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name = '{}' AND xtype = 'U')\n{}",
                table_name, inner
            );
        }

        if self.dialect == Dialect::MsSql
            && trimmed.starts_with("CREATE TABLE ")
            && !trimmed.contains("IF NOT EXISTS")
        {
            let table_name = extract_table_name(trimmed);
            return format!(
                "IF NOT EXISTS (SELECT * FROM sysobjects WHERE name = '{}' AND xtype = 'U')\n{}",
                table_name, line
            );
        }

        if self.dialect == Dialect::MsSql {
            if let Some(stripped) = trimmed.strip_prefix("ALTER TABLE ") {
                if let Some(after_add) = stripped.find(" ADD COLUMN ") {
                    let table_name = stripped[..after_add].trim();
                    let rest = &stripped[after_add + " ADD COLUMN ".len()..];
                    let col_name = rest.split_whitespace().next().unwrap_or("");
                    let add_stmt = line.replace(" ADD COLUMN ", " ADD ");
                    return format!(
                        "IF NOT EXISTS (SELECT * FROM sys.columns WHERE object_id = OBJECT_ID('{}') AND name = '{}')\n{}",
                        table_name, col_name, add_stmt
                    );
                }
            }
        }

        if self.dialect == Dialect::MsSql && trimmed.starts_with("CREATE INDEX IF NOT EXISTS ") {
            let (idx_name, table_name) = extract_index_info(trimmed);
            let inner = line.replacen("CREATE INDEX IF NOT EXISTS ", "CREATE INDEX ", 1);
            return format!(
                "IF NOT EXISTS (SELECT 1 FROM sys.indexes WHERE object_id = OBJECT_ID(N'{}') AND name = N'{}')\n{}",
                table_name, idx_name, inner
            );
        }

        if self.dialect == Dialect::Oracle {
            if trimmed.contains(" ADD COLUMN ") {
                return line.replace(" ADD COLUMN ", " ADD ");
            }
        }

        if self.dialect == Dialect::Oracle && trimmed.starts_with("CREATE INDEX IF NOT EXISTS ") {
            return line.replacen("CREATE INDEX IF NOT EXISTS ", "CREATE INDEX ", 1);
        }

        line.to_string()
    }
}

fn dialect_type(dialect: Dialect, mapping: &TypeMapping) -> &'static str {
    match dialect {
        Dialect::Postgres => mapping.postgres,
        Dialect::Sqlite => mapping.sqlite,
        Dialect::MySql => mapping.mysql,
        Dialect::MsSql => mapping.mssql,
        Dialect::Oracle => mapping.oracle,
    }
}

fn parse_dialect(s: &str) -> Result<Dialect, TranspileError> {
    match s.trim().to_lowercase().as_str() {
        "postgres" => Ok(Dialect::Postgres),
        "sqlite" => Ok(Dialect::Sqlite),
        "mysql" => Ok(Dialect::MySql),
        "mssql" => Ok(Dialect::MsSql),
        "oracle" => Ok(Dialect::Oracle),
        _ => Err(TranspileError::UnknownDialect(s.to_string())),
    }
}

fn extract_index_info(stmt: &str) -> (String, String) {
    let after_ci = stmt
        .trim_start()
        .strip_prefix("CREATE INDEX IF NOT EXISTS ")
        .or_else(|| stmt.trim_start().strip_prefix("CREATE INDEX "))
        .unwrap_or("");
    let parts: Vec<&str> = after_ci.splitn(2, " ON ").collect();
    if parts.len() != 2 {
        return ("unknown".to_string(), "unknown".to_string());
    }
    let index_name = parts[0].trim();
    let table_name = parts[1].split('(').next().unwrap_or("").trim();
    (index_name.to_string(), table_name.to_string())
}

fn extract_table_name(stmt: &str) -> String {
    let after_create = stmt
        .trim_start()
        .strip_prefix("CREATE TABLE ")
        .and_then(|s| s.strip_prefix("IF NOT EXISTS "))
        .or_else(|| stmt.trim_start().strip_prefix("CREATE TABLE "))
        .unwrap_or("");

    after_create
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches('(')
        .to_string()
}

fn post_process_oracle_create_table(sql: &str) -> String {
    let mut result = String::with_capacity(sql.len() + 512);
    let mut in_create = false;
    let mut create_lines: Vec<&str> = Vec::new();

    for line in sql.lines() {
        let trimmed = line.trim();

        if !in_create && trimmed.starts_with("CREATE TABLE ") {
            in_create = true;
            create_lines.push(line);
            if trimmed.ends_with(';') {
                in_create = false;
                result.push_str(&wrap_oracle_plsql(&create_lines.join("\n")));
                create_lines.clear();
            }
        } else if in_create {
            create_lines.push(line);
            if trimmed.ends_with(';') {
                in_create = false;
                result.push_str(&wrap_oracle_plsql(&create_lines.join("\n")));
                create_lines.clear();
            }
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }

    if in_create {
        let stmt = create_lines.join("\n");
        result.push_str(&wrap_oracle_plsql(&stmt));
    }

    result
}

fn wrap_oracle_plsql(create_stmt: &str) -> String {
    let inner = create_stmt.trim_end_matches(';');
    let inner = if inner.starts_with("CREATE TABLE IF NOT EXISTS ") {
        inner.replacen("CREATE TABLE IF NOT EXISTS ", "CREATE TABLE ", 1)
    } else {
        inner.to_string()
    };
    let inner_escaped = inner.replace('\'', "''");

    format!(
        "BEGIN\n\
         EXECUTE IMMEDIATE '\n\
         {}\n\
         ';\n\
         EXCEPTION\n\
         WHEN OTHERS THEN\n\
         IF SQLCODE = -955 THEN NULL; END IF;\n\
         END;\n\
         /\n",
        inner_escaped
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mysql_type_substitution() {
        let sql = "CREATE TABLE users (\n    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),\n    permissions JSONB NOT NULL DEFAULT '[]'\n);";
        let output = Transpiler::new(Dialect::MySql).transpile(sql).unwrap();
        assert!(output.sql.contains("VARCHAR(36)"));
        assert!(output.sql.contains("JSON"));
        assert!(output.sql.contains("DEFAULT (UUID())"));
        assert!(!output.sql.contains("UNIQUEIDENTIFIER"));
    }

    #[test]
    fn test_oracle_type_substitution() {
        let sql = "CREATE TABLE test (\n    id UUID PRIMARY KEY,\n    enabled BOOLEAN NOT NULL DEFAULT FALSE,\n    counter BIGINT NOT NULL DEFAULT 0,\n    description TEXT\n);";
        let output = Transpiler::new(Dialect::Oracle).transpile(sql).unwrap();
        assert!(output.sql.contains("RAW(16)"));
        assert!(output.sql.contains("NUMBER(1)"));
        assert!(output.sql.contains("INTEGER"));
        assert!(output.sql.contains("CLOB"));
        assert!(output.sql.contains("DEFAULT 0"));
    }

    #[test]
    fn test_timestamptz_substitution() {
        let sql = "CREATE TABLE t (created_at TIMESTAMPTZ NOT NULL DEFAULT NOW());";
        let pg = Transpiler::new(Dialect::Postgres).transpile(sql).unwrap();
        assert!(pg.sql.contains("TIMESTAMPTZ"));
        let sq = Transpiler::new(Dialect::Sqlite).transpile(sql).unwrap();
        assert!(sq.sql.contains("TEXT"));
        let ms = Transpiler::new(Dialect::MsSql).transpile(sql).unwrap();
        assert!(ms.sql.contains("DATETIME2"));
        assert!(ms.sql.contains("DEFAULT GETDATE()"));
        let or = Transpiler::new(Dialect::Oracle).transpile(sql).unwrap();
        assert!(or.sql.contains("TIMESTAMP WITH TIME ZONE"));
        assert!(or.sql.contains("DEFAULT CURRENT_TIMESTAMP"));
    }

    #[test]
    fn test_mssql_create_index_guard() {
        let sql = "CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);";
        let output = Transpiler::new(Dialect::MsSql).transpile(sql).unwrap();
        assert!(output.sql.contains("IF NOT EXISTS (SELECT 1 FROM sys.indexes WHERE object_id = OBJECT_ID(N'users') AND name = N'idx_users_email')"));
        assert!(output
            .sql
            .contains("CREATE INDEX idx_users_email ON users(email)"));
    }

    #[test]
    fn test_oracle_create_index_strips_if_not_exists() {
        let sql = "CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);";
        let output = Transpiler::new(Dialect::Oracle).transpile(sql).unwrap();
        assert!(!output.sql.contains("IF NOT EXISTS"));
        assert!(output
            .sql
            .contains("CREATE INDEX idx_users_email ON users(email)"));
    }

    #[test]
    fn test_oracle_alter_add_column_strips_column() {
        let sql = "ALTER TABLE users ADD COLUMN display_name VARCHAR(255);";
        let output = Transpiler::new(Dialect::Oracle).transpile(sql).unwrap();
        assert!(!output.sql.contains("ADD COLUMN"));
        assert!(output.sql.contains("ADD display_name VARCHAR2(255)"));
    }

    #[test]
    fn test_sqlite_varchar_to_text() {
        let sql = "CREATE TABLE users (name VARCHAR(255) NOT NULL);";
        let output = Transpiler::new(Dialect::Sqlite).transpile(sql).unwrap();
        assert!(output.sql.contains("TEXT"));
        assert!(!output.sql.contains("VARCHAR"));
    }

    #[test]
    fn test_dialect_only_suffix() {
        let sql = "\
-- @dialect postgres_only
SELECT pg_only_function();
-- @end_dialect";
        let pg = Transpiler::new(Dialect::Postgres).transpile(sql).unwrap();
        assert!(pg.sql.contains("pg_only_function"));
        let sq = Transpiler::new(Dialect::Sqlite).transpile(sql).unwrap();
        assert!(!sq.sql.contains("pg_only_function"));
    }

    #[test]
    fn test_standalone_token_not_in_function_name() {
        let sql = "id UUID PRIMARY KEY DEFAULT gen_random_uuid()";
        let output = Transpiler::new(Dialect::Sqlite).transpile(sql).unwrap();
        assert!(output.sql.contains("id TEXT"));
        assert!(output.sql.contains("gen_random_uuid()") || output.sql.contains("randomblob"));
    }

    #[test]
    fn test_col_annotation_stripped() {
        let sql = "-- @col id UUID\nCREATE TABLE users (id UUID PRIMARY KEY);";
        let output = Transpiler::new(Dialect::Postgres).transpile(sql).unwrap();
        assert!(!output.sql.contains("-- @col"));
        assert!(output.sql.contains("CREATE TABLE"));
    }

    #[test]
    fn test_create_table_postgres_passthrough() {
        let sql = "CREATE TABLE IF NOT EXISTS users (\n    id UUID PRIMARY KEY DEFAULT gen_random_uuid()\n);";
        let output = Transpiler::new(Dialect::Postgres).transpile(sql).unwrap();
        assert!(output.sql.contains("UUID"));
        assert!(output.sql.contains("gen_random_uuid()"));
        assert!(output.warnings.is_empty());
    }

    #[test]
    fn test_create_table_sqlite() {
        let sql = "CREATE TABLE IF NOT EXISTS users (\n    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),\n    permissions JSONB NOT NULL DEFAULT '[]'\n);";
        let output = Transpiler::new(Dialect::Sqlite).transpile(sql).unwrap();
        assert!(output.sql.contains("TEXT"));
        assert!(output.warnings.is_empty());
    }

    #[test]
    fn test_create_table_mssql() {
        let sql = "CREATE TABLE IF NOT EXISTS users (\n    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),\n    permissions JSONB NOT NULL DEFAULT '[]'\n);";
        let output = Transpiler::new(Dialect::MsSql).transpile(sql).unwrap();
        assert!(output.sql.contains("UNIQUEIDENTIFIER"));
        assert!(output.sql.contains("NVARCHAR(MAX)"));
        assert!(output
            .sql
            .contains("IF NOT EXISTS (SELECT * FROM sysobjects"));
        assert!(output.sql.contains("DEFAULT NEWID()"));
    }

    #[test]
    fn test_dialect_block_filtering() {
        let sql = "\
-- @dialect postgres
SELECT pg_function();
-- @end_dialect
-- @dialect sqlite
SELECT sqlite_function();
-- @end_dialect";

        let output = Transpiler::new(Dialect::Postgres).transpile(sql).unwrap();
        assert!(output.sql.contains("pg_function"));
        assert!(!output.sql.contains("sqlite_function"));
    }

    #[test]
    fn test_varchar_to_nvarchar_mssql() {
        let sql = "CREATE TABLE test (name VARCHAR(255) NOT NULL);";
        let output = Transpiler::new(Dialect::MsSql).transpile(sql).unwrap();
        assert!(output.sql.contains("NVARCHAR(255)"));
    }

    #[test]
    fn test_varchar_to_varchar2_oracle() {
        let sql = "CREATE TABLE test (name VARCHAR(255) NOT NULL);";
        let output = Transpiler::new(Dialect::Oracle).transpile(sql).unwrap();
        assert!(output.sql.contains("VARCHAR2(255)"));
    }

    #[test]
    fn test_boolean_substitution_sqlite() {
        let sql = "ALTER TABLE users ADD COLUMN mfa_enabled BOOLEAN NOT NULL DEFAULT FALSE;";
        let output = Transpiler::new(Dialect::Sqlite).transpile(sql).unwrap();
        assert!(output.sql.contains("INTEGER"));
        assert!(output.sql.contains("DEFAULT 0"));
    }

    #[test]
    fn test_oracle_create_table_wrapping() {
        let sql = "CREATE TABLE users (\n    id UUID PRIMARY KEY\n);";
        let output = Transpiler::new(Dialect::Oracle).transpile(sql).unwrap();
        assert!(output.sql.contains("BEGIN"));
        assert!(output.sql.contains("EXECUTE IMMEDIATE"));
        assert!(output.sql.contains("RAW(16)"));
        assert!(output.sql.contains("SQLCODE = -955"));
        assert!(output.sql.contains("END;\n/"));
    }

    #[test]
    fn test_oracle_create_table_if_not_exists_stripped() {
        let sql = "CREATE TABLE IF NOT EXISTS users (\n    id UUID PRIMARY KEY\n);";
        let output = Transpiler::new(Dialect::Oracle).transpile(sql).unwrap();
        assert!(!output.sql.contains("IF NOT EXISTS"));
        assert!(output.sql.contains("EXECUTE IMMEDIATE"));
    }

    #[test]
    fn test_oracle_create_table_with_default_escaped_quotes() {
        let sql = "CREATE TABLE test (\n    permissions JSONB NOT NULL DEFAULT '[]'\n);";
        let output = Transpiler::new(Dialect::Oracle).transpile(sql).unwrap();
        assert!(output.sql.contains("CLOB"));
        assert!(output.sql.contains("''[]''"));
    }

    #[test]
    fn test_mssql_alter_add_column_guard() {
        let sql = "ALTER TABLE users ADD COLUMN locked_at TIMESTAMPTZ;";
        let output = Transpiler::new(Dialect::MsSql).transpile(sql).unwrap();
        assert!(output.sql.contains("IF NOT EXISTS (SELECT * FROM sys.columns WHERE object_id = OBJECT_ID('users') AND name = 'locked_at')"));
        assert!(output
            .sql
            .contains("ALTER TABLE users ADD locked_at DATETIME2"));
    }

    #[test]
    fn test_pg_adds_if_not_exists_from_bare_create() {
        let sql = "CREATE TABLE users (\n    id UUID PRIMARY KEY\n);";
        let output = Transpiler::new(Dialect::Postgres).transpile(sql).unwrap();
        assert!(output.sql.contains("CREATE TABLE IF NOT EXISTS users"));
    }

    #[test]
    fn test_sqlite_adds_if_not_exists_from_bare_create() {
        let sql = "CREATE TABLE users (\n    id UUID PRIMARY KEY\n);";
        let output = Transpiler::new(Dialect::Sqlite).transpile(sql).unwrap();
        assert!(output.sql.contains("CREATE TABLE IF NOT EXISTS users"));
    }

    #[test]
    fn test_mysql_adds_if_not_exists_from_bare_create() {
        let sql = "CREATE TABLE users (\n    id UUID PRIMARY KEY\n);";
        let output = Transpiler::new(Dialect::MySql).transpile(sql).unwrap();
        assert!(output.sql.contains("CREATE TABLE IF NOT EXISTS users"));
    }
}
