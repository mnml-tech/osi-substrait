//! Emit ANSI-style SQL strings from a [`LogicalPlan`](crate::plan::LogicalPlan).
//!
//! SQL is derived from the structured plan (secondary to Substrait).

use crate::emit::EmitError;
use crate::plan::{format_expr, format_join_on};
use crate::plan::{LogicalPlan, NamedAggregate};

/// Build a single `SELECT` statement for the plan tree.
pub fn to_sql(plan: &LogicalPlan) -> Result<String, EmitError> {
    match plan {
        LogicalPlan::Aggregate {
            input,
            group_by,
            aggregates,
        } => emit_aggregate_root(input.as_ref(), group_by, aggregates, is_multi_table(input)),
        _ => Err(EmitError::Unsupported(
            "root must be Aggregate (use planner output)",
        )),
    }
}

fn is_multi_table(plan: &LogicalPlan) -> bool {
    match plan {
        LogicalPlan::Join { .. } => true,
        LogicalPlan::Filter { input, .. } => is_multi_table(input),
        _ => false,
    }
}

fn emit_aggregate_root(
    input: &LogicalPlan,
    group_by: &[crate::plan::Expr],
    aggregates: &[NamedAggregate],
    qualify: bool,
) -> Result<String, EmitError> {
    let (from_sql, where_sql) = from_and_where(input, qualify)?;

    let mut select_items: Vec<String> = group_by
        .iter()
        .map(|e| format_expr(e, qualify))
        .collect();

    for a in aggregates {
        select_items.push(format!(
            "{} AS {}",
            format_expr(&a.expr, qualify),
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
        sql.push_str(
            &group_by
                .iter()
                .map(|e| format_expr(e, qualify))
                .collect::<Vec<_>>()
                .join(", "),
        );
    }

    Ok(sql)
}

fn from_and_where(
    plan: &LogicalPlan,
    qualify: bool,
) -> Result<(String, Option<String>), EmitError> {
    match plan {
        LogicalPlan::Filter { input, predicate } => {
            let (from, preds) = collect_filters(input.as_ref(), vec![predicate.clone()], qualify)?;
            let where_clause = match preds.len() {
                0 => None,
                1 => Some(format_expr(&preds[0], qualify)),
                _ => Some(
                    preds
                        .iter()
                        .map(|p| format_expr(p, qualify))
                        .collect::<Vec<_>>()
                        .join(" AND "),
                ),
            };
            Ok((from, where_clause))
        }
        _ => {
            let from = from_join_or_scan(plan, qualify)?;
            Ok((from, None))
        }
    }
}

fn collect_filters(
    plan: &LogicalPlan,
    mut preds: Vec<crate::plan::Expr>,
    qualify: bool,
) -> Result<(String, Vec<crate::plan::Expr>), EmitError> {
    match plan {
        LogicalPlan::Filter { input, predicate } => {
            preds.push(predicate.clone());
            collect_filters(input.as_ref(), preds, qualify)
        }
        _ => {
            let from = from_join_or_scan(plan, qualify)?;
            Ok((from, preds))
        }
    }
}

fn from_join_or_scan(plan: &LogicalPlan, qualify: bool) -> Result<String, EmitError> {
    use crate::sql_quote::dotted_osi_name_as_sql_table_expr;
    match plan {
        LogicalPlan::Scan { source, .. } => Ok(dotted_osi_name_as_sql_table_expr(source)),
        LogicalPlan::Join { left, right, on } => {
            let l = from_join_or_scan(left.as_ref(), qualify)?;
            let r = from_join_or_scan(right.as_ref(), qualify)?;
            let on_clause = format_join_on(on, qualify).join(" AND ");
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
    use crate::plan::expr::{AggFunc, Expr, Literal};
    use crate::plan::LogicalPlan;

    fn minimal_aggregate() -> LogicalPlan {
        LogicalPlan::Aggregate {
            input: Box::new(LogicalPlan::Filter {
                input: Box::new(LogicalPlan::Scan {
                    source: "warehouse.public.fact_table".into(),
                    dataset: "fact".into(),
                    columns: vec!["id".into()],
                }),
                predicate: Expr::Eq(
                    Box::new(Expr::column("", "id")),
                    Box::new(Expr::Literal(Literal::Int64(1))),
                ),
            }),
            group_by: vec![Expr::column("", "id")],
            aggregates: vec![crate::plan::NamedAggregate {
                name: "row_count".into(),
                expr: Expr::Agg {
                    func: AggFunc::Count,
                    distinct: false,
                    arg: None,
                },
            }],
        }
    }

    #[test]
    fn aggregate_filter_scan_sql() {
        let sql = to_sql(&minimal_aggregate()).expect("sql");
        assert!(sql.starts_with("SELECT "));
        assert!(sql.contains("FROM \"warehouse.public.fact_table\""));
        assert!(sql.contains("WHERE id = 1"));
        assert!(sql.contains("GROUP BY id"));
        assert!(sql.contains("COUNT(*) AS row_count"));
    }
}
