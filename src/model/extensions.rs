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

/// MNML dataset storage metadata (vendor extension payload).
///
/// The JSON payload is stored in [`CustomExtension::data`] when `vendor_name` is [`Vendor::Mnml`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MnmlDatasetStorage {
    pub version: u32,
    pub format: MnmlStorageFormat,
    pub location: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partitioning: Option<MnmlPartitioning>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MnmlStorageFormat {
    Parquet,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MnmlPartitioning {
    pub style: MnmlPartitioningStyle,
    #[serde(default)]
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MnmlPartitioningStyle {
    Hive,
}

/// Parse all MNML storage extension payloads from a `custom_extensions` list.
pub fn parse_mnml_dataset_storage_extensions(
    extensions: &[CustomExtension],
) -> Result<Vec<MnmlDatasetStorage>, serde_json::Error> {
    extensions
        .iter()
        .filter(|ext| ext.vendor_name == Vendor::Mnml)
        .filter(|ext| super::mnml_expression::is_mnml_storage_json(&ext.data))
        .map(|ext| serde_json::from_str::<MnmlDatasetStorage>(&ext.data))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mnml_dataset_storage_extensions_ignores_non_mnml() {
        let exts = vec![CustomExtension {
            vendor_name: Vendor::Common,
            data: "{}".to_string(),
        }];
        let parsed = parse_mnml_dataset_storage_extensions(&exts).expect("parse");
        assert!(parsed.is_empty());
    }

    #[test]
    fn parse_mnml_dataset_storage_extensions_parses_payload() {
        let exts = vec![CustomExtension {
            vendor_name: Vendor::Mnml,
            data: r#"{"version":1,"format":"parquet","location":"people_daily/","partitioning":{"style":"hive","keys":["dt"]}}"#
                .to_string(),
        }];
        let parsed = parse_mnml_dataset_storage_extensions(&exts).expect("parse");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].version, 1);
        assert_eq!(parsed[0].format, MnmlStorageFormat::Parquet);
        assert_eq!(parsed[0].location, "people_daily/");
    }
}
