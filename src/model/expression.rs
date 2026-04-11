//! Multi-dialect expressions (`$defs.DialectExpression`, `$defs.Expression`).

use super::enums::Dialect;
use serde::{Deserialize, Serialize};

/// Expression in a specific dialect.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialectExpression {
    pub dialect: Dialect,
    pub expression: String,
}

/// Expression definition with multi-dialect support (at least one dialect per schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expression {
    pub dialects: Vec<DialectExpression>,
}
