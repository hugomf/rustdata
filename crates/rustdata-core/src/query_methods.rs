use crate::{
    specification::{Predicate, SqlValue},
    error::RepositoryError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conjunction {
    And,
    Or,
}

#[derive(Debug, Clone)]
pub struct ParsedQuery {
    pub conditions: Vec<(String, String)>,
    pub conjunction: Conjunction,
}

/// Parses method names like "find_by_email", "find_by_organization_id_and_status",
/// "find_by_email_ne", or "find_by_email_or_phone" into column/operator pairs.
///
/// Supported operator suffixes: _eq, _ne, _lt, _lte, _gt, _gte, _like
/// Supported conjunctions: _and_, _or_
pub struct QueryMethodParser;

impl QueryMethodParser {
    pub fn parse(method_name: &str) -> Result<ParsedQuery, RepositoryError> {
        if !method_name.starts_with("find_by_") {
            return Err(RepositoryError::Database(format!(
                "Invalid query method: {} must start with 'find_by_'",
                method_name
            )));
        }

        let rest = &method_name[8..];

        if rest.is_empty() {
            return Ok(ParsedQuery {
                conditions: Vec::new(),
                conjunction: Conjunction::And,
            });
        }

        let (conjunction, parts) = if rest.contains("_or_") {
            (Conjunction::Or, rest.split("_or_").collect::<Vec<_>>())
        } else {
            (Conjunction::And, rest.split("_and_").collect::<Vec<_>>())
        };

        let mut conditions = Vec::new();
        for part in parts {
            conditions.push(Self::parse_field(part));
        }

        Ok(ParsedQuery { conditions, conjunction })
    }

    fn parse_field(field_expr: &str) -> (String, String) {
        for &(suffix, op) in KNOWN_OPERATORS.iter() {
            if let Some(stem) = field_expr.strip_suffix(suffix) {
                if !stem.is_empty() {
                    return (stem.to_string(), op.to_string());
                }
            }
        }
        (field_expr.to_string(), "eq".to_string())
    }

    pub fn build_predicate(parsed: ParsedQuery, values: Vec<SqlValue>) -> Result<Predicate, RepositoryError> {
        if parsed.conditions.len() != values.len() {
            return Err(RepositoryError::Database(
                "Number of conditions does not match number of values".to_string()
            ));
        }

        let mut predicates = Vec::new();
        for ((column, operator), value) in parsed.conditions.into_iter().zip(values) {
            let predicate = match operator.as_str() {
                "eq" => Predicate::Eq { column, value },
                "ne" => Predicate::Ne { column, value },
                "lt" => Predicate::Lt { column, value },
                "lte" => Predicate::Lte { column, value },
                "gt" => Predicate::Gt { column, value },
                "gte" => Predicate::Gte { column, value },
                "like" => {
                    let pattern = match value {
                        SqlValue::Str(s) => s,
                        _ => return Err(RepositoryError::Database("LIKE requires string value".to_string())),
                    };
                    Predicate::Like { column, pattern }
                }
                _ => Predicate::Eq { column, value },
            };
            predicates.push(predicate);
        }

        if predicates.is_empty() {
            Ok(Predicate::None)
        } else if predicates.len() == 1 {
            Ok(predicates.remove(0))
        } else {
            match parsed.conjunction {
                Conjunction::And => Ok(Predicate::And(predicates)),
                Conjunction::Or => Ok(Predicate::Or(predicates)),
            }
        }
    }
}

const KNOWN_OPERATORS: &[(&str, &str)] = &[
    ("_like", "like"),
    ("_gte", "gte"),
    ("_lte", "lte"),
    ("_neq", "ne"),
    ("_gt", "gt"),
    ("_lt", "lt"),
    ("_ne", "ne"),
    ("_eq", "eq"),
];
