//! Root OSI document (core metadata file).

use super::enums::{Dialect, Vendor};
use super::semantic_model::SemanticModel;
use serde::{Deserialize, Serialize};

/// Root document for OSI Core Metadata (`osi-schema.json` root object).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OsiDocument {
    /// OSI specification version (schema const `0.1.1`).
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialects: Option<Vec<Dialect>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendors: Option<Vec<Vendor>>,
    pub semantic_model: Vec<SemanticModel>,
}
