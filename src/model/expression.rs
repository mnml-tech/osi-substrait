//! Multi-dialect expressions (`$defs.DialectExpression`, `$defs.Expression`).
//!
//! OSI core defines `expression` as a string for SQL dialects. MNML uses structured
//! YAML/JSON keys on the same dialect entry (`col`, `case`, `aggregation`, `expr`, …).

use super::enums::Dialect;
use serde::de::Error as DeError;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

/// Body of a dialect entry: SQL text or structured MNML.
#[derive(Debug, Clone, PartialEq)]
pub enum DialectBody {
    Sql(String),
    Mnml(serde_json::Value),
}

/// Expression in a specific dialect.
#[derive(Debug, Clone, PartialEq)]
pub struct DialectExpression {
    pub dialect: Dialect,
    pub body: DialectBody,
}

impl DialectExpression {
    pub fn sql_expression(&self) -> Option<&str> {
        match &self.body {
            DialectBody::Sql(s) => Some(s.as_str()),
            DialectBody::Mnml(_) => None,
        }
    }

    pub fn mnml_body(&self) -> Option<&serde_json::Value> {
        match &self.body {
            DialectBody::Mnml(v) => Some(v),
            DialectBody::Sql(_) => None,
        }
    }

    pub fn ansi_sql(expression: impl Into<String>) -> Self {
        Self {
            dialect: Dialect::AnsiSql,
            body: DialectBody::Sql(expression.into()),
        }
    }
}

/// Expression definition with multi-dialect support (at least one dialect per schema).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expression {
    pub dialects: Vec<DialectExpression>,
}

/// Structured MNML payload from the `dialect: MNML` entry, if present.
pub fn pick_mnml_dialect(expr: &Expression) -> Option<&serde_json::Value> {
    expr.dialects
        .iter()
        .find(|d| d.dialect == Dialect::Mnml)
        .and_then(|d| d.mnml_body())
}

pub fn count_mnml_dialects(expr: &Expression) -> usize {
    expr.dialects
        .iter()
        .filter(|d| d.dialect == Dialect::Mnml)
        .count()
}

#[derive(Deserialize)]
struct DialectExpressionRaw {
    dialect: Dialect,
    expression: Option<String>,
    #[serde(flatten)]
    mnml: BTreeMap<String, serde_yaml::Value>,
}

impl<'de> Deserialize<'de> for DialectExpression {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = DialectExpressionRaw::deserialize(deserializer)?;
        match raw.dialect {
            Dialect::Mnml => {
                if raw.expression.is_some() {
                    return Err(D::Error::custom(
                        "MNML dialect must use structured keys (col, case, aggregation, …), not expression string",
                    ));
                }
                if raw.mnml.is_empty() {
                    return Err(D::Error::custom("MNML dialect entry is empty"));
                }
                let json = yaml_map_to_json(&raw.mnml).map_err(D::Error::custom)?;
                Ok(Self {
                    dialect: Dialect::Mnml,
                    body: DialectBody::Mnml(json),
                })
            }
            dialect => {
                if !raw.mnml.is_empty() {
                    return Err(D::Error::custom(format!(
                        "{dialect:?} dialect must only use `expression` string, got extra keys: {:?}",
                        raw.mnml.keys().collect::<Vec<_>>()
                    )));
                }
                let sql = raw
                    .expression
                    .ok_or_else(|| D::Error::custom(format!("{dialect:?} dialect requires `expression`")))?;
                Ok(Self {
                    dialect,
                    body: DialectBody::Sql(sql),
                })
            }
        }
    }
}

impl Serialize for DialectExpression {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &self.body {
            DialectBody::Sql(sql) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("dialect", &self.dialect)?;
                map.serialize_entry("expression", sql)?;
                map.end()
            }
            DialectBody::Mnml(v) => {
                let obj = v.as_object().ok_or_else(|| {
                    serde::ser::Error::custom("MNML dialect body must serialize as a JSON object")
                })?;
                let mut map = serializer.serialize_map(Some(1 + obj.len()))?;
                map.serialize_entry("dialect", &self.dialect)?;
                for (k, val) in obj {
                    map.serialize_entry(k, val)?;
                }
                map.end()
            }
        }
    }
}

fn yaml_map_to_json(map: &BTreeMap<String, serde_yaml::Value>) -> Result<serde_json::Value, String> {
    let yaml_val = serde_yaml::Value::Mapping(
        map.iter()
            .map(|(k, v)| (serde_yaml::Value::String(k.clone()), v.clone()))
            .collect(),
    );
    serde_json::to_value(yaml_val).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::from_yaml_str;

    #[test]
    fn parses_mnml_field_col_dialect() {
        let yaml = r#"
version: "0.1.1"
semantic_model:
  - name: m
    datasets:
      - name: d
        source: s.t
        fields:
          - name: x
            expression:
              dialects:
                - dialect: MNML
                  col: x
"#;
        let doc = from_yaml_str(yaml).expect("parse");
        let field = &doc.semantic_model[0].datasets[0].fields[0];
        let body = pick_mnml_dialect(&field.expression).expect("mnml");
        assert_eq!(body.get("col").and_then(|c| c.as_str()), Some("x"));
    }

    #[test]
    fn parses_mnml_metric_aggregation_and_expr() {
        let yaml = r#"
version: "0.1.1"
semantic_model:
  - name: m
    datasets:
      - name: d
        source: s.t
        fields:
          - name: flag
            expression:
              dialects:
                - dialect: MNML
                  case:
                    - when:
                        eq:
                          - col: is_on
                          - lit: 1
                      then:
                        lit: 1
                      else:
                        lit: 0
    metrics:
      - name: total
        expression:
          dialects:
            - dialect: MNML
              aggregation: sum
              field: d.flag
"#;
        let doc = from_yaml_str(yaml).expect("parse");
        let metric = &doc.semantic_model[0].metrics[0];
        let body = pick_mnml_dialect(&metric.expression).expect("mnml");
        assert_eq!(body.get("aggregation").and_then(|a| a.as_str()), Some("sum"));
        assert_eq!(body.get("field").and_then(|f| f.as_str()), Some("d.flag"));
    }
}
