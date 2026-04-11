//! Logical dataset fields (`$defs.Field`, field-level dimension metadata).

use super::ai_context::AiContext;
use super::expression::Expression;
use super::extensions::CustomExtension;
use serde::{Deserialize, Serialize};

/// Dimension metadata on a field (`$defs.Dimension`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldDimension {
    /// Indicates if this is a time-based dimension for temporal filtering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_time: Option<bool>,
}

/// Row-level attribute for grouping, filtering, and metric expressions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub name: String,
    pub expression: Expression,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimension: Option<FieldDimension>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_context: Option<AiContext>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_extensions: Vec<CustomExtension>,
}
