//! Bind + logical plan tests.

use osi_substrait::model::OsiDocument;
use osi_substrait::parser::from_yaml_str;
use osi_substrait::plan::{LogicalPlan, NamedAggregate};
use osi_substrait::planner::build_logical_plan;
use osi_substrait::query::{FilterSpec, SemanticQuery};
use osi_substrait::resolver::{QueryError, bind_query};

fn minimal_doc() -> OsiDocument {
    let yaml = include_str!("fixtures/minimal.yaml");
    from_yaml_str(yaml).expect("parse fixture")
}

#[test]
fn bind_group_by_only() {
    let doc = minimal_doc();
    let spec = SemanticQuery {
        group_by: vec!["fact.id".into()],
        ..Default::default()
    };
    let bound = bind_query(&doc, "minimal_model", &spec).expect("bind");
    assert_eq!(bound.root_dataset, "fact");
    assert_eq!(bound.group_by.len(), 1);
    assert!(bound.metrics.is_empty());
}

#[test]
fn bind_metrics_only_errors_without_dataset() {
    let doc = minimal_doc();
    let err = bind_query(
        &doc,
        "minimal_model",
        &SemanticQuery {
            metrics: vec!["row_count".into()],
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(matches!(err, QueryError::MissingBaseDataset));
}

#[test]
fn bind_metrics_only_with_dataset() {
    let doc = minimal_doc();
    let spec = SemanticQuery {
        metrics: vec!["row_count".into()],
        dataset: Some("fact".into()),
        ..Default::default()
    };
    let bound = bind_query(&doc, "minimal_model", &spec).expect("bind");
    assert_eq!(bound.root_dataset, "fact");
}

#[test]
fn logical_plan_shape() {
    let doc = minimal_doc();
    let spec = SemanticQuery {
        group_by: vec!["fact.id".into()],
        metrics: vec!["row_count".into()],
        filters: vec![FilterSpec::new("fact.id", serde_json::json!(1))],
        ..Default::default()
    };
    let bound = bind_query(&doc, "minimal_model", &spec).expect("bind");
    let plan = build_logical_plan(&bound).expect("plan");

    let LogicalPlan::Aggregate {
        input,
        group_by,
        aggregates,
    } = plan
    else {
        panic!("expected aggregate root");
    };
    assert_eq!(group_by, vec!["id".to_string()]);
    assert_eq!(
        aggregates,
        vec![NamedAggregate {
            name: "row_count".into(),
            expression_sql: "COUNT(*)".into(),
        }]
    );
    let LogicalPlan::Filter {
        input: filter_input,
        predicate,
    } = *input
    else {
        panic!("expected filter");
    };
    assert_eq!(predicate, "id = 1");
    let LogicalPlan::Scan { source } = *filter_input else {
        panic!("expected scan");
    };
    assert_eq!(source, "warehouse.public.fact_table");
}

fn steelwheels_doc() -> OsiDocument {
    let yaml = include_str!("fixtures/steelwheels.yaml");
    from_yaml_str(yaml).expect("parse steelwheels")
}

#[test]
fn steelwheels_join_orderfact_and_dates() {
    let doc = steelwheels_doc();
    let spec = SemanticQuery {
        group_by: vec!["dates.year_id".into()],
        metrics: vec!["sales".into()],
        dataset: Some("orderfact".into()),
        ..Default::default()
    };
    let bound = bind_query(&doc, "steelwheels", &spec).expect("bind");
    assert_eq!(bound.root_dataset, "orderfact");
    assert_eq!(bound.joins.len(), 1);
    assert_eq!(bound.joins[0].right_dataset, "dates");
    assert!(bound.joins[0].on_predicates[0].contains("time_id"));

    let plan = build_logical_plan(&bound).expect("plan");
    let s = format!("{}", plan.display_indent());
    assert!(s.contains("Join:"), "{s}");
    assert!(s.contains("orderfact") || s.contains("steelwheels.orderfact"), "{s}");
    assert!(s.contains("dates") || s.contains("steelwheels.dates"), "{s}");

    let LogicalPlan::Aggregate { group_by, .. } = plan else {
        panic!("expected aggregate root");
    };
    assert_eq!(group_by, vec!["dates.year_id".to_string()]);
}

#[test]
fn disconnected_datasets_error() {
    let yaml = include_str!("fixtures/disconnected.yaml");
    let doc: OsiDocument = from_yaml_str(yaml).expect("parse");
    let err = bind_query(
        &doc,
        "disconnected_model",
        &SemanticQuery {
            group_by: vec!["alpha.id".into(), "beta.id".into()],
            dataset: Some("alpha".into()),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(matches!(err, QueryError::DisconnectedDatasets(_)), "{err:?}");
}

#[test]
fn multi_dataset_requires_root_dataset() {
    let doc = steelwheels_doc();
    let err = bind_query(
        &doc,
        "steelwheels",
        &SemanticQuery {
            group_by: vec!["orderfact.order_status".into(), "dates.year_id".into()],
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(matches!(err, QueryError::MissingRootDataset), "{err:?}");
}

#[test]
fn semantic_query_accepts_group_by_camel_case_json() {
    let json = r#"{
        "groupBy": ["fact.id"],
        "metrics": [],
        "filters": [],
        "dataset": null
    }"#;
    let q: SemanticQuery = serde_json::from_str(json).expect("deserialize groupBy");
    assert_eq!(q.group_by, vec!["fact.id".to_string()]);
}
