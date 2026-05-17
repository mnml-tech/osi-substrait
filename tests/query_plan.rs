//! Bind + logical plan tests.

use osi_substrait::emit::sql::to_sql;
#[cfg(feature = "substrait")]
use osi_substrait::emit::substrait::to_plan;
use osi_substrait::model::OsiDocument;
use osi_substrait::parser::from_yaml_str;
use osi_substrait::plan::expr::{AggFunc, Expr, Literal};
use osi_substrait::plan::{LogicalPlan, NamedAggregate};
use osi_substrait::planner::build_logical_plan;
use osi_substrait::query::{FilterSpec, SemanticQuery};
use osi_substrait::resolver::{QueryError, bind_query};

fn minimal_doc() -> OsiDocument {
    let yaml = include_str!("fixtures/minimal.yaml");
    from_yaml_str(yaml).expect("parse fixture")
}

fn minimal_model() -> osi_substrait::model::SemanticModel {
    minimal_doc().semantic_model[0].clone()
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
    let model = minimal_model();
    let spec = SemanticQuery {
        group_by: vec!["fact.id".into()],
        metrics: vec!["row_count".into()],
        filters: vec![FilterSpec::new("fact.id", serde_json::json!(1))],
        ..Default::default()
    };
    let bound = bind_query(&doc, "minimal_model", &spec).expect("bind");
    let plan = build_logical_plan(&bound, &model).expect("plan");

    let LogicalPlan::Aggregate {
        input,
        group_by,
        aggregates,
    } = plan
    else {
        panic!("expected aggregate root");
    };
    assert_eq!(group_by, vec![Expr::column("", "id")]);
    assert_eq!(
        aggregates,
        vec![NamedAggregate {
            name: "row_count".into(),
            expr: Expr::Agg {
                func: AggFunc::Count,
                distinct: false,
                arg: None,
            },
        }]
    );
    let LogicalPlan::Filter {
        input: filter_input,
        predicate,
    } = *input
    else {
        panic!("expected filter");
    };
    assert_eq!(
        predicate,
        Expr::Eq(
            Box::new(Expr::column("", "id")),
            Box::new(Expr::Literal(Literal::Int64(1))),
        )
    );
    let LogicalPlan::Scan {
        source,
        dataset,
        columns,
    } = *filter_input
    else {
        panic!("expected scan");
    };
    assert_eq!(source, "warehouse.public.fact_table");
    assert_eq!(dataset, "fact");
    assert!(columns.contains(&"id".to_string()));
}

#[test]
fn eq_field_filter_sql() {
    let doc = minimal_doc();
    let model = minimal_model();
    let spec = SemanticQuery {
        group_by: vec!["fact.id".into()],
        metrics: vec!["row_count".into()],
        filters: vec![FilterSpec {
            field: "fact.day".into(),
            operator: Some("eq_field".into()),
            value: serde_json::json!("fact.month_days"),
        }],
        ..Default::default()
    };
    let bound = bind_query(&doc, "minimal_model", &spec).expect("bind");
    let plan = build_logical_plan(&bound, &model).expect("plan");
    let sql = to_sql(&plan).expect("sql");
    assert!(
        sql.contains("day = month_days"),
        "expected column equality in SQL, got: {sql}"
    );
}

#[cfg(feature = "substrait")]
#[test]
fn minimal_substrait_plan() {
    let doc = minimal_doc();
    let model = minimal_model();
    let spec = SemanticQuery {
        group_by: vec!["fact.id".into()],
        metrics: vec!["row_count".into()],
        filters: vec![FilterSpec::new("fact.id", serde_json::json!(1))],
        ..Default::default()
    };
    let bound = bind_query(&doc, "minimal_model", &spec).expect("bind");
    let plan = build_logical_plan(&bound, &model).expect("plan");
    let p = to_plan(&plan).expect("substrait plan");
    assert_eq!(p.relations.len(), 1);
}

fn steelwheels_doc() -> OsiDocument {
    let yaml = include_str!("fixtures/steelwheels.yaml");
    from_yaml_str(yaml).expect("parse steelwheels")
}

#[test]
fn steelwheels_join_orderfact_and_dates() {
    let doc = steelwheels_doc();
    let model = doc.semantic_model[0].clone();
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
    assert!(bound.joins[0].on[0].left_sql.contains("time_id"));

    let plan = build_logical_plan(&bound, &model).expect("plan");
    let s = format!("{}", plan.display_indent());
    assert!(s.contains("Join:"), "{s}");
    assert!(s.contains("orderfact") || s.contains("steelwheels.orderfact"), "{s}");
    assert!(s.contains("dates") || s.contains("steelwheels.dates"), "{s}");

    let LogicalPlan::Aggregate { group_by, .. } = plan else {
        panic!("expected aggregate root");
    };
    assert_eq!(
        group_by,
        vec![Expr::column("dates", "year_id")]
    );
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
fn people_daily_tenant_model_validates_and_plans() {
    use osi_substrait::validate::validate;
    let yaml = include_str!("fixtures/people_daily_tenant.yaml");
    let doc = from_yaml_str(yaml).expect("parse");
    validate(&doc).expect("validate");
    let model = &doc.semantic_model[0];
    let spec = SemanticQuery {
        metrics: vec!["starters".into(), "turnover".into()],
        group_by: vec![
            "people_daily.year_mm".into(),
            "people_daily.month_desc".into(),
            "people_daily.employment_type_name".into(),
        ],
        dataset: Some("people_daily".into()),
        filters: vec![FilterSpec::new("people_daily.year", serde_json::json!([2024, 2025]))],
    };
    let bound = bind_query(&doc, "people_daily", &spec).expect("bind");
    build_logical_plan(&bound, model).expect("plan");
    let p = to_plan(&build_logical_plan(&bound, model).unwrap()).expect("substrait");
    assert!(!p.relations.is_empty());
}

#[test]
fn people_daily_mnml_distinct_and_tenure_metrics() {
    use osi_substrait::validate::validate;
    let yaml = include_str!("fixtures/people_daily_tenant.yaml");
    let doc = from_yaml_str(yaml).expect("parse");
    validate(&doc).expect("validate");
    let model = &doc.semantic_model[0];
    for name in ["num_days", "num_employees", "tenure"] {
        let spec = SemanticQuery {
            metrics: vec![name.into()],
            group_by: vec!["people_daily.year".into()],
            dataset: Some("people_daily".into()),
            ..Default::default()
        };
        let bound = bind_query(&doc, "people_daily", &spec).expect("bind");
        let plan = build_logical_plan(&bound, model).expect("plan");
        let sql = to_sql(&plan).expect("sql");
        if name == "tenure" {
            assert!(sql.contains("CURRENT_DATE"), "{sql}");
            assert!(sql.contains("employment_start_date"), "{sql}");
        } else {
            assert!(sql.contains("COUNT(DISTINCT"), "{sql}");
        }
        to_plan(&plan).expect("substrait");
    }
}

#[test]
fn mnml_starters_metric_plan_and_substrait() {
    let yaml = include_str!("fixtures/mnml_expressions.yaml");
    let doc = from_yaml_str(yaml).expect("parse");
    let model = &doc.semantic_model[0];
    let spec = SemanticQuery {
        group_by: vec!["fact.id".into()],
        metrics: vec!["starters".into()],
        dataset: Some("fact".into()),
        ..Default::default()
    };
    let bound = bind_query(&doc, "mnml_model", &spec).expect("bind");
    let plan = build_logical_plan(&bound, model).expect("plan");
    let sql = to_sql(&plan).expect("sql");
    assert!(
        sql.contains("CASE") && sql.contains("is_starter") && sql.contains("starters"),
        "expected CASE aggregate from MNML, got: {sql}"
    );
    #[cfg(feature = "substrait")]
    {
        let p = to_plan(&plan).expect("substrait");
        assert_eq!(p.relations.len(), 1);
    }
}

#[test]
fn mnml_row_count_metric() {
    let yaml = include_str!("fixtures/mnml_expressions.yaml");
    let doc = from_yaml_str(yaml).expect("parse");
    let model = &doc.semantic_model[0];
    let spec = SemanticQuery {
        metrics: vec!["row_count".into()],
        dataset: Some("fact".into()),
        ..Default::default()
    };
    let bound = bind_query(&doc, "mnml_model", &spec).expect("bind");
    let plan = build_logical_plan(&bound, model).expect("plan");
    let LogicalPlan::Aggregate { aggregates, .. } = plan else {
        panic!("aggregate root");
    };
    assert!(matches!(
        aggregates[0].expr,
        Expr::Agg {
            func: AggFunc::Count,
            ..
        }
    ));
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
