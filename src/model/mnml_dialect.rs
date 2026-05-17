//! Map MNML dialect YAML bodies to planner JSON (`agg`, `col`, `case`, …).

use super::expression::pick_mnml_dialect;
use super::{Expression, MnmlExpressionKind};
use serde_json::Value;

fn is_metric_shape(v: &Value) -> bool {
    v.get("agg").is_some() || v.get("aggregation").is_some()
}

/// JSON expression node for a field MNML dialect entry.
pub fn field_mnml_expr_value(body: &Value) -> Value {
    if let Some(expr) = body.get("expr") {
        if !is_metric_shape(body) {
            return expr.clone();
        }
    }
    body.clone()
}

/// Normalize a metric MNML dialect body to [`MnmlMetricDef`] JSON (`agg`, `field` / `arg`).
pub fn metric_mnml_to_def_value(body: &Value) -> Result<Value, String> {
    let mut obj = body
        .as_object()
        .cloned()
        .ok_or_else(|| "MNML metric dialect must be a mapping".to_string())?;

    if let Some(agg) = obj.remove("aggregation") {
        obj.entry("agg".to_string()).or_insert(agg);
    }

    if obj.get("field").is_none() {
        if let Some(expr) = obj.remove("expr") {
            if obj.get("agg").is_some() {
                obj.insert("arg".to_string(), expr);
            } else {
                return Err(
                    "MNML metric dialect: use `field` for a semantic field ref or `aggregation` with `expr`"
                        .into(),
                );
            }
        }
    }

    if obj.get("agg").is_none() {
        return Err("MNML metric dialect requires `aggregation` or `agg`".into());
    }

    Ok(Value::Object(obj))
}

/// Build envelope from `expression.dialects` MNML entry.
pub fn envelope_from_expression(
    expr: &Expression,
    kind: MnmlExpressionKind,
) -> Result<Option<Value>, String> {
    let Some(body) = pick_mnml_dialect(expr) else {
        return Ok(None);
    };
    let expr_value = match kind {
        MnmlExpressionKind::FieldExpr => field_mnml_expr_value(body),
        MnmlExpressionKind::Metric => metric_mnml_to_def_value(body)?,
    };
    Ok(Some(expr_value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::MnmlMetricDef;
    use crate::parser::from_yaml_str;

    #[test]
    fn metric_normalizes_aggregation_alias() {
        let yaml = r#"
version: "0.1.1"
semantic_model:
  - name: m
    metrics:
      - name: n
        expression:
          dialects:
            - dialect: MNML
              aggregation: sum
              field: d.x
    datasets:
      - name: d
        source: s
        fields:
          - name: x
            expression:
              dialects:
                - dialect: MNML
                  col: x
"#;
        let doc = from_yaml_str(yaml).expect("parse");
        let metric = &doc.semantic_model[0].metrics[0];
        let v = envelope_from_expression(&metric.expression, MnmlExpressionKind::Metric)
            .expect("ok")
            .expect("some");
        let def: MnmlMetricDef = serde_json::from_value(v).expect("metric def");
        assert_eq!(def.agg, "sum");
        assert_eq!(def.field.as_deref(), Some("d.x"));
    }
}
