//! Open Semantic Interchange dialect and vendor enumerations.

use serde::{Deserialize, Serialize};

/// Supported SQL and expression language dialects (`$defs.Dialect`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Dialect {
    AnsiSql,
    Snowflake,
    Mdx,
    Tableau,
    Databricks,
}

/// Supported vendors for custom extensions (`$defs.Vendor`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Vendor {
    Common,
    Snowflake,
    Salesforce,
    Dbt,
    Databricks,
}
