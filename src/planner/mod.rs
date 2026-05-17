//! Build a [`LogicalPlan`](crate::plan::LogicalPlan) from a [`BoundQuery`](crate::resolver::BoundQuery) (verb).

use crate::model::{Metric, SemanticModel};
use crate::plan::mnml::{field_to_expr, metric_to_expr, physical_column_for_field};
use crate::plan::{
    Expr, Literal, LogicalPlan, NamedAggregate, parse_metric_sql, pick_expression_sql,
};
use crate::resolver::{BoundQuery, QueryError, ResolvedField};

fn field_expr(rf: &ResolvedField, model: &SemanticModel) -> Result<Expr, QueryError> {
    if let Some(expr) = field_to_expr(&rf.field, &rf.dataset, model)? {
        return Ok(expr);
    }
    let sql = pick_expression_sql(&rf.field.expression).ok_or(QueryError::MissingExpressionSql)?;
    Ok(Expr::column(rf.dataset.clone(), sql))
}

fn metric_expr(m: &Metric, model: &SemanticModel) -> Result<Expr, QueryError> {
    if let Some(expr) = metric_to_expr(m, model)? {
        return Ok(expr);
    }
    let sql = pick_expression_sql(&m.expression).ok_or(QueryError::MissingExpressionSql)?;
    Ok(parse_metric_sql(&sql))
}

fn dataset_columns(model: &SemanticModel, dataset_name: &str) -> Result<Vec<String>, QueryError> {
    let ds = model
        .datasets
        .iter()
        .find(|d| d.name == dataset_name)
        .ok_or_else(|| QueryError::DatasetNotFound(dataset_name.to_string()))?;
    let mut cols = Vec::new();
    for f in &ds.fields {
        if let Ok(name) = physical_column_for_field(model, dataset_name, &f.name) {
            if !cols.contains(&name) {
                cols.push(name);
            }
        }
    }
    if cols.is_empty() {
        return Err(QueryError::InvalidMnmlExpression(format!(
            "dataset {dataset_name:?} has no physical columns for scan"
        )));
    }
    Ok(cols)
}

fn filter_to_expr(
    f: &crate::resolver::BoundFilter,
    qualify: bool,
    model: &SemanticModel,
) -> Result<Expr, QueryError> {
    let left = field_expr(&f.field, model)?;
    if matches!(f.operator.as_str(), "eq_field" | "eq_column") {
        let rhs = f.rhs_field.as_ref().ok_or_else(|| {
            QueryError::InvalidFieldRef(
                "eq_field / eq_column filter missing RHS field binding".to_string(),
            )
        })?;
        return Ok(Expr::Eq(
            Box::new(qualify_column(left, qualify)),
            Box::new(qualify_column(field_expr(rhs, model)?, qualify)),
        ));
    }
    let op = f.operator.as_str();
    let left_q = qualify_column(left, qualify);
    match op {
        "in" if f.value.is_array() => {
            let values: Vec<Literal> = f
                .value
                .as_array()
                .expect("is_array")
                .iter()
                .map(Literal::from_json)
                .collect();
            Ok(Expr::In {
                expr: Box::new(left_q),
                values,
            })
        }
        "eq" | "=" => Ok(Expr::Eq(
            Box::new(left_q),
            Box::new(Expr::Literal(Literal::from_json(&f.value))),
        )),
        "ne" | "!=" => Ok(Expr::Ne(
            Box::new(left_q),
            Box::new(Expr::Literal(Literal::from_json(&f.value))),
        )),
        _ => Ok(Expr::Eq(
            Box::new(left_q),
            Box::new(Expr::Literal(Literal::from_json(&f.value))),
        )),
    }
}

fn qualify_column(expr: Expr, qualify: bool) -> Expr {
    if qualify {
        expr
    } else if let Expr::Column(c) = expr {
        Expr::column("", c.sql)
    } else {
        expr
    }
}

/// Build a plan: scan (and joins) → optional combined filter → aggregate.
pub fn build_logical_plan(
    bound: &BoundQuery,
    model: &SemanticModel,
) -> Result<LogicalPlan, QueryError> {
    let qualify = bound.sources.len() > 1;

    let root_src = bound
        .sources
        .get(&bound.root_dataset)
        .cloned()
        .ok_or_else(|| QueryError::DatasetNotFound(bound.root_dataset.clone()))?;

    let root_cols = dataset_columns(model, &bound.root_dataset)?;

    let mut plan = LogicalPlan::Scan {
        source: root_src,
        dataset: bound.root_dataset.clone(),
        columns: root_cols,
    };

    for j in &bound.joins {
        let right_src = bound
            .sources
            .get(&j.right_dataset)
            .cloned()
            .ok_or_else(|| QueryError::DatasetNotFound(j.right_dataset.clone()))?;
        let right_cols = dataset_columns(model, &j.right_dataset)?;
        let right = LogicalPlan::Scan {
            source: right_src,
            dataset: j.right_dataset.clone(),
            columns: right_cols,
        };
        plan = LogicalPlan::Join {
            left: Box::new(plan),
            right: Box::new(right),
            on: j.on.clone(),
        };
    }

    let after_filter = if bound.filters.is_empty() {
        plan
    } else {
        let parts: Vec<Expr> = bound
            .filters
            .iter()
            .map(|f| filter_to_expr(f, qualify, model))
            .collect::<Result<_, _>>()?;
        LogicalPlan::Filter {
            input: Box::new(plan),
            predicate: Expr::and(parts),
        }
    };

    let mut group_by = Vec::with_capacity(bound.group_by.len());
    for f in &bound.group_by {
        group_by.push(qualify_column(field_expr(f, model)?, qualify));
    }

    let aggregates: Vec<NamedAggregate> = bound
        .metrics
        .iter()
        .map(|m| {
            Ok(NamedAggregate {
                name: m.name.clone(),
                expr: metric_expr(m, model)?,
            })
        })
        .collect::<Result<_, _>>()?;

    Ok(LogicalPlan::Aggregate {
        input: Box::new(after_filter),
        group_by,
        aggregates,
    })
}
