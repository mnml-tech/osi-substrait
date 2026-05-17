//! MNML structured expression payloads in [`CustomExtension::data`](super::extensions::CustomExtension).

use super::extensions::CustomExtension;
use super::enums::Vendor;
use serde::{Deserialize, Serialize};

/// Parsed MNML expression envelope (`version` + `kind` + `expr`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MnmlExpressionEnvelope {
    pub version: u32,
    pub kind: MnmlExpressionKind,
    pub expr: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MnmlExpressionKind {
    FieldExpr,
    Metric,
}

/// Shorthand metric aggregate definition inside `expr`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MnmlMetricDef {
    pub agg: String,
    #[serde(default)]
    pub distinct: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arg: Option<serde_json::Value>,
}

/// Returns true when JSON looks like MNML dataset storage (not an expression).
pub fn is_mnml_storage_json(data: &str) -> bool {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(data) else {
        return false;
    };
    v.get("format").is_some() && v.get("kind").is_none()
}

/// Parse the single MNML expression extension on a field or metric, if present.
pub fn parse_mnml_expression_extension(
    extensions: &[CustomExtension],
) -> Result<Option<MnmlExpressionEnvelope>, serde_json::Error> {
    let mut found: Option<MnmlExpressionEnvelope> = None;
    for ext in extensions {
        if ext.vendor_name != Vendor::Mnml {
            continue;
        }
        if is_mnml_storage_json(&ext.data) {
            continue;
        }
        let parsed: MnmlExpressionEnvelope = serde_json::from_str(&ext.data)?;
        if found.is_some() {
            continue;
        }
        found = Some(parsed);
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_storage_from_expression() {
        assert!(is_mnml_storage_json(
            r#"{"version":1,"format":"parquet","location":"x/"}"#
        ));
        assert!(!is_mnml_storage_json(
            r#"{"version":1,"kind":"metric","expr":{"agg":"count"}}"#
        ));
    }

    #[test]
    fn parse_metric_envelope() {
        let exts = vec![CustomExtension {
            vendor_name: Vendor::Mnml,
            data: r#"{"version":1,"kind":"metric","expr":{"agg":"sum","field":"fact.id"}}"#
                .to_string(),
        }];
        let env = parse_mnml_expression_extension(&exts)
            .expect("parse")
            .expect("env");
        assert_eq!(env.kind, MnmlExpressionKind::Metric);
    }
}
