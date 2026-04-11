//! Indented tree display for [`LogicalPlan`], similar in spirit to DataFusion's `display_indent()`.

use std::fmt::{self, Display, Formatter};

use super::{LogicalPlan, NamedAggregate};

/// Indented, multi-line plan text (use with `format!("{}", plan.display_indent())`).
pub struct DisplayIndent<'a>(pub(super) &'a LogicalPlan);

impl Display for DisplayIndent<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write_node(f, self.0, 0)
    }
}

impl LogicalPlan {
    /// Return a value that formats like DataFusion's logical plan tree (operator per line, children indented).
    pub fn display_indent(&self) -> DisplayIndent<'_> {
        DisplayIndent(self)
    }
}

impl Display for LogicalPlan {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.display_indent().fmt(f)
    }
}

fn write_node(f: &mut Formatter<'_>, plan: &LogicalPlan, depth: usize) -> fmt::Result {
    let pad = "  ".repeat(depth);
    match plan {
        LogicalPlan::Scan { source } => {
            writeln!(f, "{pad}Scan: {source}")
        }
        LogicalPlan::Join { left, right, on } => {
            let on_s = on.join(" AND ");
            writeln!(f, "{pad}Join: inner on={on_s}")?;
            write_node(f, left, depth + 1)?;
            write_node(f, right, depth + 1)
        }
        LogicalPlan::Filter { input, predicate } => {
            writeln!(f, "{pad}Filter: {predicate}")?;
            write_node(f, input, depth + 1)
        }
        LogicalPlan::Aggregate {
            input,
            group_by,
            aggregates,
        } => {
            let gb = if group_by.is_empty() {
                "[]".to_string()
            } else {
                format!("[{}]", group_by.join(", "))
            };
            let ag = format_aggregates(aggregates);
            writeln!(f, "{pad}Aggregate: groupBy={gb}, aggr=[{ag}]")?;
            write_node(f, input, depth + 1)
        }
    }
}

fn format_aggregates(aggregates: &[NamedAggregate]) -> String {
    if aggregates.is_empty() {
        return String::new();
    }
    aggregates
        .iter()
        .map(|a| format!("{}: {}", a.name, a.expression_sql))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use crate::plan::{LogicalPlan, NamedAggregate};

    #[test]
    fn display_indent_matches_datafusion_style_tree() {
        let plan = LogicalPlan::Aggregate {
            input: Box::new(LogicalPlan::Filter {
                input: Box::new(LogicalPlan::Scan {
                    source: "warehouse.public.f".into(),
                }),
                predicate: "id = 1".into(),
            }),
            group_by: vec!["id".into()],
            aggregates: vec![NamedAggregate {
                name: "row_count".into(),
                expression_sql: "COUNT(*)".into(),
            }],
        };
        let s = format!("{}", plan.display_indent());
        assert!(
            s.contains("Aggregate:")
                && s.contains("Filter:")
                && s.contains("Scan: warehouse.public.f"),
            "{s}"
        );
    }

    #[test]
    fn display_join_under_aggregate() {
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
            aggregates: vec![],
        };
        let s = format!("{}", plan.display_indent());
        assert!(s.contains("Join:") && s.contains("inner on=a.k = b.k"), "{s}");
    }
}
