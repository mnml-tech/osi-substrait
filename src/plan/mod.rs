//! Logical plan AST (nouns) for OSI-backed queries.

mod display;

use crate::model::{Dialect, Expression};

pub use display::DisplayIndent;

/// Root logical plan node (scan / join → optional filter → aggregate).
#[derive(Debug, Clone)]
pub enum LogicalPlan {
    Scan {
        /// Underlying relation id ([`Dataset::source`](crate::model::Dataset::source)).
        source: String,
    },
    Join {
        left: Box<LogicalPlan>,
        right: Box<LogicalPlan>,
        /// `ON` equality fragments (`left.col = right.col`).
        on: Vec<String>,
    },
    Filter {
        input: Box<LogicalPlan>,
        /// Boolean SQL expression.
        predicate: String,
    },
    Aggregate {
        input: Box<LogicalPlan>,
        /// SQL scalar expressions for `GROUP BY` (field expressions in chosen dialect).
        group_by: Vec<String>,
        /// Named aggregate expressions (metric bodies).
        aggregates: Vec<NamedAggregate>,
    },
}

/// Aggregate with output label (metric name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedAggregate {
    pub name: String,
    pub expression_sql: String,
}

/// Prefer `ANSI_SQL`, else first dialect entry.
pub fn pick_expression_sql(expr: &Expression) -> Option<String> {
    expr.dialects
        .iter()
        .find(|d| d.dialect == Dialect::AnsiSql)
        .or_else(|| expr.dialects.first())
        .map(|d| d.expression.clone())
}
