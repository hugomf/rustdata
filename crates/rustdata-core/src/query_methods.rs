use crate::{
    error::RepositoryError,
    specification::{Predicate, SqlValue},
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

        Ok(ParsedQuery {
            conditions,
            conjunction,
        })
    }

    fn parse_field(field_expr: &str) -> (String, String) {
        // Check zero-param suffixes first (they don't consume a value slot)
        if let Some(stem) = field_expr.strip_suffix("_is_not_null") {
            if !stem.is_empty() {
                return (stem.to_string(), "is_not_null".to_string());
            }
        }
        if let Some(stem) = field_expr.strip_suffix("_is_null") {
            if !stem.is_empty() {
                return (stem.to_string(), "is_null".to_string());
            }
        }
        if let Some(stem) = field_expr.strip_suffix("_in") {
            if !stem.is_empty() {
                return (stem.to_string(), "in".to_string());
            }
        }
        for &(suffix, op) in KNOWN_OPERATORS.iter() {
            if let Some(stem) = field_expr.strip_suffix(suffix) {
                if !stem.is_empty() {
                    return (stem.to_string(), op.to_string());
                }
            }
        }
        (field_expr.to_string(), "eq".to_string())
    }

    pub fn build_predicate(
        parsed: ParsedQuery,
        values: Vec<SqlValue>,
    ) -> Result<Predicate, RepositoryError> {
        // Count how many value slots are needed (is_null / is_not_null need 0)
        let value_consuming: Vec<bool> = parsed
            .conditions
            .iter()
            .map(|(_, op)| op != "is_null" && op != "is_not_null")
            .collect();
        let expected_values = value_consuming.iter().filter(|&&v| v).count();
        if expected_values != values.len() {
            return Err(RepositoryError::Database(format!(
                "Expected {} value(s) for query, got {}",
                expected_values,
                values.len()
            )));
        }

        let mut predicates = Vec::new();
        let mut value_iter = values.into_iter();
        for ((column, operator), _needs_value) in parsed.conditions.into_iter().zip(value_consuming)
        {
            let predicate = match operator.as_str() {
                "is_null" => Predicate::IsNull { column },
                "is_not_null" => Predicate::IsNotNull { column },
                "in" => {
                    // For the dynamic API, `in` takes a single SqlValue::Json(array)
                    // or the caller passes multiple values wrapped in a Vec via In.
                    // We take one value and wrap it in a single-element In for now;
                    // callers that need multi-value IN should use find_by_X_in directly.
                    let value = value_iter.next().unwrap();
                    Predicate::In {
                        column,
                        values: vec![value],
                    }
                }
                "eq" => Predicate::Eq {
                    column,
                    value: value_iter.next().unwrap(),
                },
                "ne" => Predicate::Ne {
                    column,
                    value: value_iter.next().unwrap(),
                },
                "lt" => Predicate::Lt {
                    column,
                    value: value_iter.next().unwrap(),
                },
                "lte" => Predicate::Lte {
                    column,
                    value: value_iter.next().unwrap(),
                },
                "gt" => Predicate::Gt {
                    column,
                    value: value_iter.next().unwrap(),
                },
                "gte" => Predicate::Gte {
                    column,
                    value: value_iter.next().unwrap(),
                },
                "like" => {
                    let value = value_iter.next().unwrap();
                    let pattern = match value {
                        SqlValue::Str(s) => s,
                        _ => {
                            return Err(RepositoryError::Database(
                                "LIKE requires string value".to_string(),
                            ))
                        }
                    };
                    Predicate::Like { column, pattern }
                }
                _ => Predicate::Eq {
                    column,
                    value: value_iter.next().unwrap(),
                },
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
