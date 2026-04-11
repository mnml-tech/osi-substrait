//! Vendor custom extensions (`$defs.CustomExtension`).

use super::enums::Vendor;
use serde::{Deserialize, Serialize};

/// Vendor-specific attributes for extensibility. `data` is often a JSON document as a string.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomExtension {
    pub vendor_name: Vendor,
    pub data: String,
}
