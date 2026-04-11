//! Build a [`LogicalPlan`](crate::plan::LogicalPlan) from a [`BoundQuery`](crate::resolver::BoundQuery) (verb).

use crate::model::Expression;
use crate::plan::{LogicalPlan, NamedAggregate, pick_expression_sql};
use crate::resolver::{BoundQuery, QueryError, ResolvedField};

fn require_expression_sql(expr: &Expression) -> Result<String, QueryError> {
    pick_expression_sql(expr).ok_or(QueryError::MissingExpressionSql)
}

fn qualified_field_sql(rf: &ResolvedField, qualify: bool) -> Result<String, QueryError> {
    let sql = require_expression_sql(&rf.field.expression)?;
    if qualify {
        Ok(format!("{}.{}", rf.dataset, sql))
    } else {
        Ok(sql)
    }
}

fn json_to_sql_literal(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::Bool(b) => {
            if *b {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            }
        }
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::to_string(v).unwrap_or_else(|_| "NULL".to_string())
        }
    }
}

fn filter_predicate_sql(expr_sql: &str, operator: &str, value: &serde_json::Value) -> String {
    match operator {
        "in" if value.is_array() => {
            let vals: Vec<String> = value
                .as_array()
                .expect("is_array")
                .iter()
                .map(json_to_sql_literal)
                .collect();
            format!("{expr_sql} IN ({})", vals.join(", "))
        }
        "eq" | "=" => format!("{expr_sql} = {}", json_to_sql_literal(value)),
        "ne" | "!=" => format!("{expr_sql} <> {}", json_to_sql_literal(value)),
        _ => format!("{expr_sql} = {}", json_to_sql_literal(value)),
    }
}

/// Build a minimal plan: scan (and joins) → optional combined filter → aggregate.
pub fn build_logical_plan(bound: &BoundQuery) -> Result<LogicalPlan, QueryError> {
    let qualify = bound.sources.len() > 1;

    let root_src = bound
        .sources
        .get(&bound.root_dataset)
        .cloned()
        .ok_or_else(|| QueryError::DatasetNotFound(bound.root_dataset.clone()))?;

    let mut plan = LogicalPlan::Scan { source: root_src };

    for j in &bound.joins {
        let right_src = bound
            .sources
            .get(&j.right_dataset)
            .cloned()
            .ok_or_else(|| QueryError::DatasetNotFound(j.right_dataset.clone()))?;
        let right = LogicalPlan::Scan {
            source: right_src,
        };
        plan = LogicalPlan::Join {
            left: Box::new(plan),
            right: Box::new(right),
            on: j.on_predicates.clone(),
        };
    }

    let after_filter = if bound.filters.is_empty() {
        plan
    } else {
        let parts: Vec<String> = bound
            .filters
            .iter()
            .map(|f| {
                let expr_sql = qualified_field_sql(&f.field, qualify)?;
                Ok(filter_predicate_sql(&expr_sql, &f.operator, &f.value))
            })
            .collect::<Result<_, QueryError>>()?;
        let predicate = parts.join(" AND ");
        LogicalPlan::Filter {
            input: Box::new(plan),
            predicate,
        }
    };

    let group_by: Vec<String> = bound
        .group_by
        .iter()
        .map(|f| qualified_field_sql(f, qualify))
        .collect::<Result<_, _>>()?;

    let aggregates: Vec<NamedAggregate> = bound
        .metrics
        .iter()
        .map(|m| {
            Ok(NamedAggregate {
                name: m.name.clone(),
                expression_sql: require_expression_sql(&m.expression)?,
            })
        })
        .collect::<Result<_, _>>()?;

    Ok(LogicalPlan::Aggregate {
        input: Box::new(after_filter),
        group_by,
        aggregates,
    })
}
