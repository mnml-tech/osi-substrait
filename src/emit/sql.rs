//! Emit ANSI-style SQL strings from a [`LogicalPlan`](crate::plan::LogicalPlan).
//!
//! Inputs are trusted OSI-derived fragments (predicates and expressions are already SQL text).
//! This is a best-effort formatter, not a full SQL parser or validator.

use crate::emit::EmitError;
use crate::plan::{LogicalPlan, NamedAggregate};

/// Build a single `SELECT` statement for the plan tree.
///
/// Expects the same shapes produced by [`crate::planner::build_logical_plan`]: root is
/// [`LogicalPlan::Aggregate`], with optional [`LogicalPlan::Filter`] and [`LogicalPlan::Join`] /
/// [`LogicalPlan::Scan`] below.
pub fn to_sql(plan: &LogicalPlan) -> Result<String, EmitError> {
    match plan {
        LogicalPlan::Aggregate {
            input,
            group_by,
            aggregates,
        } => emit_aggregate_root(input.as_ref(), group_by, aggregates),
        _ => Err(EmitError::Unsupported(
            "root must be Aggregate (use planner output)",
        )),
    }
}

fn emit_aggregate_root(
    input: &LogicalPlan,
    group_by: &[String],
    aggregates: &[NamedAggregate],
) -> Result<String, EmitError> {
    let (from_sql, where_sql) = from_and_where(input)?;

    let mut select_items: Vec<String> = group_by.to_vec();

    for a in aggregates {
        select_items.push(format!(
            "{} AS {}",
            a.expression_sql,
            quote_ident(&a.name)
        ));
    }

    if select_items.is_empty() {
        return Err(EmitError::InvalidPlan("empty SELECT"));
    }

    let select_list = select_items.join(", ");
    let mut sql = format!("SELECT {select_list} FROM {from_sql}");

    if let Some(w) = where_sql {
        sql.push_str(" WHERE ");
        sql.push_str(&w);
    }

    if !group_by.is_empty() {
        sql.push_str(" GROUP BY ");
        sql.push_str(&group_by.join(", "));
    }

    Ok(sql)
}

/// Peel `Filter` nodes; build `FROM` from `Scan` / `Join` chain.
fn from_and_where(plan: &LogicalPlan) -> Result<(String, Option<String>), EmitError> {
    match plan {
        LogicalPlan::Filter { input, predicate } => {
            let (from, preds) = collect_filters(input.as_ref(), vec![predicate.clone()])?;
            let where_clause = match preds.len() {
                0 => None,
                1 => Some(preds[0].clone()),
                _ => Some(preds.join(" AND ")),
            };
            Ok((from, where_clause))
        }
        _ => {
            let from = from_join_or_scan(plan)?;
            Ok((from, None))
        }
    }
}

fn collect_filters(
    plan: &LogicalPlan,
    mut preds: Vec<String>,
) -> Result<(String, Vec<String>), EmitError> {
    match plan {
        LogicalPlan::Filter { input, predicate } => {
            preds.push(predicate.clone());
            collect_filters(input.as_ref(), preds)
        }
        _ => {
            let from = from_join_or_scan(plan)?;
            Ok((from, preds))
        }
    }
}

fn from_join_or_scan(plan: &LogicalPlan) -> Result<String, EmitError> {
    match plan {
        LogicalPlan::Scan { source } => Ok(source.clone()),
        LogicalPlan::Join { left, right, on } => {
            let l = from_join_or_scan(left.as_ref())?;
            let r = from_join_or_scan(right.as_ref())?;
            let on_clause = on.join(" AND ");
            Ok(format!("{l} INNER JOIN {r} ON {on_clause}"))
        }
        LogicalPlan::Filter { .. } => Err(EmitError::InvalidPlan(
            "Filter must be peeled before FROM",
        )),
        LogicalPlan::Aggregate { .. } => Err(EmitError::Unsupported(
            "nested Aggregate under FROM",
        )),
    }
}

fn quote_ident(name: &str) -> String {
    if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        name.to_string()
    } else {
        format!("\"{}\"", name.replace('"', "\"\""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::LogicalPlan;

    fn minimal_aggregate() -> LogicalPlan {
        LogicalPlan::Aggregate {
            input: Box::new(LogicalPlan::Filter {
                input: Box::new(LogicalPlan::Scan {
                    source: "warehouse.public.fact_table".into(),
                }),
                predicate: "id = 1".into(),
            }),
            group_by: vec!["id".into()],
            aggregates: vec![NamedAggregate {
                name: "row_count".into(),
                expression_sql: "COUNT(*)".into(),
            }],
        }
    }

    #[test]
    fn aggregate_filter_scan_sql() {
        let sql = to_sql(&minimal_aggregate()).expect("sql");
        assert!(sql.starts_with("SELECT "));
        assert!(sql.contains("FROM warehouse.public.fact_table"));
        assert!(sql.contains("WHERE id = 1"));
        assert!(sql.contains("GROUP BY id"));
        assert!(sql.contains("COUNT(*) AS row_count"));
    }

    #[test]
    fn scan_only_not_valid_root() {
        let plan = LogicalPlan::Scan {
            source: "t".into(),
        };
        assert!(to_sql(&plan).is_err());
    }

    #[test]
    fn aggregate_with_join_in_from() {
        let plan = LogicalPlan::Aggregate {
            input: Box::new(LogicalPlan::Join {
                left: Box::new(LogicalPlan::Scan {
                    source: "a".into(),
                }),
                right: Box::new(LogicalPlan::Scan {
                    source: "b".into(),
                }),
                on: vec!["a.k = b.k".into()],
            }),
            group_by: vec!["a.x".into()],
            aggregates: vec![NamedAggregate {
                name: "m".into(),
                expression_sql: "SUM(a.y)".into(),
            }],
        };
        let sql = to_sql(&plan).expect("sql");
        assert!(sql.contains("a INNER JOIN b ON a.k = b.k"), "{sql}");
        assert!(sql.contains("GROUP BY a.x"), "{sql}");
    }
}
