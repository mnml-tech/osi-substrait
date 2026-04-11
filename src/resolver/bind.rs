//! Bind a [`SemanticQuery`](crate::query::SemanticQuery) to a concrete OSI semantic model named `model_name` in `doc`.

use std::collections::{HashMap, HashSet};

use crate::model::{Dataset, Field, Metric, OsiDocument, Relationship, SemanticModel};
use crate::query::SemanticQuery;
use crate::resolver::QueryError;

/// Field resolved with its logical [`Dataset`].
#[derive(Debug, Clone)]
pub struct ResolvedField {
    pub dataset: String,
    pub field: Field,
}

/// Filter with a [`ResolvedField`].
#[derive(Debug, Clone)]
pub struct BoundFilter {
    pub field: ResolvedField,
    pub operator: String,
    pub value: serde_json::Value,
}

/// One INNER JOIN step (left-deep): `left_dataset` already in the tree, add `right_dataset`.
#[derive(Debug, Clone)]
pub struct BoundJoin {
    pub left_dataset: String,
    pub right_dataset: String,
    /// `left.col = right.col` fragments for `ON`.
    pub on_predicates: Vec<String>,
}

/// Fully bound query: one or more logical datasets joined by relationships.
#[derive(Debug, Clone)]
pub struct BoundQuery {
    pub model_name: String,
    /// Anchor scan / join root.
    pub root_dataset: String,
    /// Logical dataset name → physical `source` for every table in the join.
    pub sources: HashMap<String, String>,
    /// Ordered INNER JOINs after the root scan.
    pub joins: Vec<BoundJoin>,
    pub group_by: Vec<ResolvedField>,
    pub metrics: Vec<Metric>,
    pub filters: Vec<BoundFilter>,
}

/// Resolve `spec` against the semantic model named `model_name` in `doc`.
pub fn bind_query(
    doc: &OsiDocument,
    model_name: &str,
    spec: &SemanticQuery,
) -> Result<BoundQuery, QueryError> {
    if model_name.is_empty() {
        return Err(QueryError::ModelNotFound(String::new()));
    }

    let model = doc
        .semantic_model
        .iter()
        .find(|m| m.name == model_name)
        .ok_or_else(|| QueryError::ModelNotFound(model_name.to_string()))?;

    let mut paths: Vec<String> = Vec::new();
    paths.extend(spec.group_by.iter().cloned());
    paths.extend(spec.filters.iter().map(|f| f.field.clone()));

    let path_datasets: HashSet<String> = paths
        .iter()
        .map(|p| {
            let (ds, _) = parse_field_ref(p)?;
            Ok::<_, QueryError>(ds.to_string())
        })
        .collect::<Result<_, _>>()?;

    let root = if path_datasets.is_empty() {
        if spec.metrics.is_empty() {
            return Err(QueryError::EmptyQuery);
        }
        spec.dataset.clone().ok_or(QueryError::MissingBaseDataset)?
    } else if path_datasets.len() == 1 {
        spec.dataset.clone().unwrap_or_else(|| path_datasets.iter().next().unwrap().clone())
    } else {
        spec.dataset.clone().ok_or(QueryError::MissingRootDataset)?
    };

    let mut required: HashSet<String> = path_datasets;
    required.insert(root.clone());

    let dataset_by_name: HashMap<String, &Dataset> = model
        .datasets
        .iter()
        .map(|d| (d.name.clone(), d))
        .collect();

    for name in &required {
        if !dataset_by_name.contains_key(name) {
            return Err(QueryError::DatasetNotFound(name.clone()));
        }
    }

    let resolve_field = |path: &str| -> Result<ResolvedField, QueryError> {
        let (ds_name, field_name) = parse_field_ref(path)?;
        if !required.contains(ds_name) {
            return Err(QueryError::DatasetNotFound(ds_name.to_string()));
        }
        let dataset = dataset_by_name
            .get(ds_name)
            .cloned()
            .ok_or_else(|| QueryError::DatasetNotFound(ds_name.to_string()))?;
        let field = dataset
            .fields
            .iter()
            .find(|f| f.name == field_name)
            .cloned()
            .ok_or_else(|| QueryError::FieldNotFound(format!("{ds_name}.{field_name}")))?;
        Ok(ResolvedField {
            dataset: ds_name.to_string(),
            field,
        })
    };

    let group_by: Vec<ResolvedField> = spec
        .group_by
        .iter()
        .map(|p| resolve_field(p))
        .collect::<Result<_, _>>()?;

    let mut filters = Vec::new();
    for f in &spec.filters {
        let field = resolve_field(&f.field)?;
        let operator = f.operator.clone().unwrap_or_else(|| {
            if f.value.is_array() {
                "in".to_string()
            } else {
                "eq".to_string()
            }
        });
        filters.push(BoundFilter {
            field,
            operator,
            value: f.value.clone(),
        });
    }

    let metrics: Vec<Metric> = spec
        .metrics
        .iter()
        .map(|name| {
            model
                .metrics
                .iter()
                .find(|m| m.name == *name)
                .cloned()
                .ok_or_else(|| QueryError::MetricNotFound(name.clone()))
        })
        .collect::<Result<_, _>>()?;

    let joins = if required.len() <= 1 {
        Vec::new()
    } else {
        build_join_order(model, &required, &root)?
    };

    let mut sources: HashMap<String, String> = HashMap::new();
    for name in &required {
        let ds = dataset_by_name.get(name).unwrap();
        sources.insert((*name).clone(), ds.source.clone());
    }

    Ok(BoundQuery {
        model_name: model.name.clone(),
        root_dataset: root,
        sources,
        joins,
        group_by,
        metrics,
        filters,
    })
}

fn parse_field_ref(s: &str) -> Result<(&str, &str), QueryError> {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(QueryError::InvalidFieldRef(s.to_string()));
    }
    Ok((parts[0], parts[1]))
}

fn join_predicate_sql(left: &str, right: &str, rel: &Relationship) -> Option<Vec<String>> {
    if left == rel.from && right == rel.to {
        if rel.from_columns.len() != rel.to_columns.len() {
            return None;
        }
        return Some(
            rel
                .from_columns
                .iter()
                .zip(rel.to_columns.iter())
                .map(|(a, b)| format!("{left}.{a} = {right}.{b}"))
                .collect(),
        );
    }
    if left == rel.to && right == rel.from {
        if rel.from_columns.len() != rel.to_columns.len() {
            return None;
        }
        return Some(
            rel
                .from_columns
                .iter()
                .zip(rel.to_columns.iter())
                .map(|(a, b)| format!("{left}.{b} = {right}.{a}"))
                .collect(),
        );
    }
    None
}

/// Prim-style: grow from `root` until `required` is covered.
fn build_join_order(
    model: &SemanticModel,
    required: &HashSet<String>,
    root: &str,
) -> Result<Vec<BoundJoin>, QueryError> {
    let mut included: HashSet<String> = std::iter::once(root.to_string()).collect();
    let mut joins = Vec::new();

    while included != *required {
        let mut step: Option<(String, String, BoundJoin)> = None;
        for rel in &model.relationships {
            let a = rel.from.as_str();
            let b = rel.to.as_str();
            if included.contains(a) && required.contains(b) && !included.contains(b) {
                if let Some(on) = join_predicate_sql(a, b, rel) {
                    step = Some((
                        a.to_string(),
                        b.to_string(),
                        BoundJoin {
                            left_dataset: a.to_string(),
                            right_dataset: b.to_string(),
                            on_predicates: on,
                        },
                    ));
                    break;
                }
            }
            if included.contains(b) && required.contains(a) && !included.contains(a) {
                if let Some(on) = join_predicate_sql(b, a, rel) {
                    step = Some((
                        b.to_string(),
                        a.to_string(),
                        BoundJoin {
                            left_dataset: b.to_string(),
                            right_dataset: a.to_string(),
                            on_predicates: on,
                        },
                    ));
                    break;
                }
            }
        }
        let (left, right, join) = step.ok_or_else(|| {
            QueryError::DisconnectedDatasets(
                required
                    .difference(&included)
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        })?;
        included.insert(right.clone());
        joins.push(join);
        let _ = left;
    }

    Ok(joins)
}
