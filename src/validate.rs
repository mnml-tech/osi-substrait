//! Semantic validation beyond Serde (uniqueness, references, schema constraints).

use crate::error::Error;
use crate::model::{Expression, OsiDocument};
use std::collections::{HashMap, HashSet};

const EXPECTED_VERSION: &str = "0.1.1";

/// Validate an [`OsiDocument`] (version, uniqueness, references, expression shape).
pub fn validate(doc: &OsiDocument) -> Result<(), Error> {
    let mut issues = Vec::new();

    if doc.version != EXPECTED_VERSION {
        issues.push(format!(
            "[Version] expected {:?}, got {:?}",
            EXPECTED_VERSION, doc.version
        ));
    }

    for model in &doc.semantic_model {
        validate_semantic_model(model, &mut issues);
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(Error::Validation(issues))
    }
}

fn validate_semantic_model(model: &crate::model::SemanticModel, issues: &mut Vec<String>) {
    let model_name = &model.name;

    if model.datasets.is_empty() {
        issues.push(format!(
            "[SemanticModel] model {:?} must have at least one dataset",
            model_name
        ));
    }

    let dataset_name_list: Vec<&str> = model.datasets.iter().map(|d| d.name.as_str()).collect();
    for dup in duplicate_strings(&dataset_name_list) {
        issues.push(format!(
            "[Unique] duplicate dataset name {:?} in model {:?}",
            dup, model_name
        ));
    }

    for dataset in &model.datasets {
        let field_names: Vec<&str> = dataset.fields.iter().map(|f| f.name.as_str()).collect();
        for dup in duplicate_strings(&field_names) {
            issues.push(format!(
                "[Unique] duplicate field name {:?} in dataset {:?} (model {:?})",
                dup, dataset.name, model_name
            ));
        }

        for field in &dataset.fields {
            validate_expression_nonempty(
                &format!("field {:?}.{:?}", dataset.name, field.name),
                &field.expression,
                issues,
            );
        }
    }

    let metric_names: Vec<&str> = model.metrics.iter().map(|m| m.name.as_str()).collect();
    for dup in duplicate_strings(&metric_names) {
        issues.push(format!(
            "[Unique] duplicate metric name {:?} in model {:?}",
            dup, model_name
        ));
    }

    for metric in &model.metrics {
        validate_expression_nonempty(
            &format!("metric {:?}", metric.name),
            &metric.expression,
            issues,
        );
    }

    let rel_names: Vec<&str> = model
        .relationships
        .iter()
        .map(|r| r.name.as_str())
        .collect();
    for dup in duplicate_strings(&rel_names) {
        issues.push(format!(
            "[Unique] duplicate relationship name {:?} in model {:?}",
            dup, model_name
        ));
    }

    let known: HashSet<&str> = dataset_name_list.iter().copied().collect();

    for rel in &model.relationships {
        if rel.from_columns.len() != rel.to_columns.len() {
            issues.push(format!(
                "[Relationship] {:?}: from_columns length {} does not match to_columns length {}",
                rel.name,
                rel.from_columns.len(),
                rel.to_columns.len()
            ));
        }
        if !known.contains(rel.from.as_str()) {
            issues.push(format!(
                "[Reference] relationship {:?} references unknown dataset {:?} (model {:?})",
                rel.name, rel.from, model_name
            ));
        }
        if !known.contains(rel.to.as_str()) {
            issues.push(format!(
                "[Reference] relationship {:?} references unknown dataset {:?} (model {:?})",
                rel.name, rel.to, model_name
            ));
        }
    }
}

fn validate_expression_nonempty(context: &str, expr: &Expression, issues: &mut Vec<String>) {
    if expr.dialects.is_empty() {
        issues.push(format!(
            "[Expression] {} must have at least one dialect entry",
            context
        ));
    }
}

/// Names that appear more than once (stable order of first occurrence of each duplicated value).
fn duplicate_strings(names: &[&str]) -> Vec<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for n in names {
        *counts.entry(*n).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .filter(|(_, c)| *c > 1)
        .map(|(k, _)| k.to_string())
        .collect()
}
