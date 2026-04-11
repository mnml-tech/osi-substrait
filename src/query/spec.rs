//! Query request shape ([`SemanticQuery`]): dimensions, metrics, filters.
//!
//! Which [`SemanticModel`](crate::model::SemanticModel) to use is chosen by the caller (e.g. HTTP path or host API), not embedded here.
//! Field references use `dataset.field` (logical dataset name and field name).

use serde::{Deserialize, Serialize};

/// Query body: `GROUP BY` fields, metrics, filters. Model identity is provided separately to [`crate::resolver::bind_query`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticQuery {
    /// Field references (`dataset.field`) for `GROUP BY`.
    #[serde(default, alias = "groupBy")]
    pub group_by: Vec<String>,
    /// Metric names defined on the semantic model.
    #[serde(default)]
    pub metrics: Vec<String>,
    #[serde(default)]
    pub filters: Vec<FilterSpec>,
    /// When no field references appear in `group_by` or filters, selects the scan target for metrics-only queries.
    #[serde(default)]
    pub dataset: Option<String>,
}

/// Predicate on a field (`dataset.field`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilterSpec {
    /// `dataset.field`
    pub field: String,
    /// Defaults: `in` if `value` is a JSON array, otherwise `eq`.
    #[serde(default)]
    pub operator: Option<String>,
    pub value: serde_json::Value,
}

impl FilterSpec {
    #[must_use]
    pub fn new(field: impl Into<String>, value: serde_json::Value) -> Self {
        Self {
            field: field.into(),
            operator: None,
            value,
        }
    }
}
