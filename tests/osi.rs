//! Integration tests: fixtures, TPC-DS example, round-trip.

use osi_substrait::model::{
    AiContext, Dialect, DialectExpression, Expression, FieldDimension, OsiDocument, SemanticModel,
};
use osi_substrait::{parser, validate};
use std::path::PathBuf;

#[test]
fn steelwheels_fixture_parses_and_validates() {
    let yaml = include_str!("fixtures/steelwheels.yaml");
    let doc = parser::from_yaml_str(yaml).expect("parse steelwheels fixture");
    assert_eq!(doc.version, "0.1.1");
    assert_eq!(doc.semantic_model.len(), 1);
    let m = &doc.semantic_model[0];
    assert_eq!(m.name, "steelwheels");
    assert_eq!(m.datasets.len(), 3);
    assert_eq!(m.relationships.len(), 2);
    assert_eq!(m.metrics.len(), 5);
    validate::validate(&doc).expect("validate steelwheels");
}

#[test]
fn minimal_fixture_parses_and_validates() {
    let yaml = include_str!("fixtures/minimal.yaml");
    let doc = parser::from_yaml_str(yaml).expect("parse minimal fixture");
    assert_eq!(doc.version, "0.1.1");
    assert_eq!(doc.semantic_model.len(), 1);
    let m = &doc.semantic_model[0];
    assert_eq!(m.name, "minimal_model");
    assert_eq!(m.datasets.len(), 1);
    assert_eq!(m.datasets[0].name, "fact");
    validate::validate(&doc).expect("validate minimal");
}

#[test]
#[ignore = "clone github.com/open-semantic-interchange/OSI as ../OSI (sibling of this repo), then: cargo test -- --include-ignored"]
fn tpcds_example_parses_and_validates() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../OSI/examples/tpcds_semantic_model.yaml");
    assert!(
        path.exists(),
        "expected OSI example at {} (clone github.com/open-semantic-interchange/OSI next to this repo)",
        path.display()
    );
    let text = std::fs::read_to_string(&path).expect("read tpcds");
    let doc = parser::from_yaml_str(&text).expect("parse tpcds");
    assert_eq!(doc.version, "0.1.1");
    assert_eq!(doc.semantic_model.len(), 1);
    let m = &doc.semantic_model[0];
    assert_eq!(m.name, "tpcds_retail_model");
    assert!(m.datasets.len() >= 5);
    assert_eq!(m.relationships.len(), 4);
    assert!(m.metrics.len() >= 3);
    validate::validate(&doc).expect("validate tpcds");
}

#[test]
fn roundtrip_yaml_document() {
    let doc = OsiDocument {
        version: "0.1.1".to_string(),
        dialects: None,
        vendors: None,
        semantic_model: vec![SemanticModel {
            name: "rt".to_string(),
            description: None,
            ai_context: Some(AiContext::Text("hello".to_string())),
            datasets: vec![osi_substrait::model::Dataset {
                name: "d".to_string(),
                source: "a.b.c".to_string(),
                primary_key: vec![],
                unique_keys: vec![],
                description: None,
                ai_context: None,
                fields: vec![osi_substrait::model::Field {
                    name: "x".to_string(),
                    expression: Expression {
                        dialects: vec![DialectExpression {
                            dialect: Dialect::AnsiSql,
                            expression: "x".to_string(),
                        }],
                    },
                    dimension: Some(FieldDimension {
                        is_time: Some(false),
                    }),
                    label: None,
                    description: None,
                    ai_context: None,
                    custom_extensions: vec![],
                }],
                custom_extensions: vec![],
            }],
            relationships: vec![],
            metrics: vec![],
            custom_extensions: vec![],
        }],
    };
    validate::validate(&doc).expect("valid");
    let yaml = serde_yaml::to_string(&doc).expect("serialize");
    let back: OsiDocument = parser::from_yaml_str(&yaml).expect("parse roundtrip");
    assert_eq!(back.version, doc.version);
    assert_eq!(back.semantic_model[0].name, "rt");
}

#[test]
fn ai_context_string_on_dataset_from_json() {
    let v = serde_json::json!({
        "version": "0.1.1",
        "semantic_model": [{
            "name": "m",
            "datasets": [{
                "name": "d",
                "source": "s",
                "ai_context": "plain text",
                "fields": [{
                    "name": "f",
                    "expression": { "dialects": [{ "dialect": "ANSI_SQL", "expression": "1" }] }
                }]
            }]
        }]
    });
    let doc: OsiDocument = serde_json::from_value(v).expect("from_value");
    let ds = &doc.semantic_model[0].datasets[0];
    assert!(matches!(&ds.ai_context, Some(AiContext::Text(s)) if s == "plain text"));
}
