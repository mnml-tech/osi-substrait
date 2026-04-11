//! Emit Substrait [`Plan`](substrait::proto::Plan) from a [`LogicalPlan`](crate::plan::LogicalPlan).
//!
//! # v1 scope
//!
//! Only [`LogicalPlan::Scan`] is mapped to a [`ReadRel`](substrait::proto::ReadRel) with a
//! [`NamedTable`](substrait::proto::read_rel::NamedTable) and a minimal placeholder schema (one
//! nullable string column). Filters, joins, and aggregates carry **opaque SQL strings** in the IR;
//! turning them into Substrait expressions requires a structured expression tree or SQL parsing, so
//! they return [`EmitError::Unsupported`] until extended.

use substrait::proto::{
    self,
    plan_rel::RelType as PlanRelType,
    read_rel::{NamedTable, ReadType},
    rel::RelType,
    r#type::{Kind, Nullability},
};

use crate::emit::EmitError;
use crate::plan::LogicalPlan;

/// Build a Substrait plan from `plan`.
///
/// Returns [`EmitError::Unsupported`] unless `plan` is a single [`LogicalPlan::Scan`].
pub fn to_plan(plan: &LogicalPlan) -> Result<proto::Plan, EmitError> {
    let rel = match plan {
        LogicalPlan::Scan { source } => emit_read_rel_scan(source)?,
        LogicalPlan::Join { .. } => {
            return Err(EmitError::Unsupported(
                "Join: use SQL emitter or add structured join keys",
            ))
        }
        LogicalPlan::Filter { .. } => {
            return Err(EmitError::Unsupported(
                "Filter: predicate is opaque SQL, not a Substrait expression",
            ))
        }
        LogicalPlan::Aggregate { .. } => {
            return Err(EmitError::Unsupported(
                "Aggregate: groupings and measures are opaque SQL",
            ))
        }
    };

    let names = vec!["_col".to_string()];

    Ok(proto::Plan {
        version: Some(proto::Version {
            major_number: 0,
            minor_number: 62,
            patch_number: 0,
            ..Default::default()
        }),
        relations: vec![proto::PlanRel {
            rel_type: Some(PlanRelType::Root(proto::RelRoot {
                input: Some(rel),
                names,
            })),
        }],
        ..Default::default()
    })
}

fn emit_read_rel_scan(source: &str) -> Result<proto::Rel, EmitError> {
    let table_names: Vec<String> = source.split('.').map(String::from).collect();

    let base_schema = proto::NamedStruct {
        names: vec!["_col".to_string()],
        r#struct: Some(proto::r#type::Struct {
            types: vec![proto::Type {
                kind: Some(Kind::String(proto::r#type::String {
                    type_variation_reference: 0,
                    nullability: Nullability::Nullable as i32,
                })),
            }],
            type_variation_reference: 0,
            nullability: Nullability::Nullable as i32,
        }),
    };

    Ok(proto::Rel {
        rel_type: Some(RelType::Read(Box::new(proto::ReadRel {
            read_type: Some(ReadType::NamedTable(NamedTable {
                names: table_names,
                advanced_extension: None,
            })),
            base_schema: Some(base_schema),
            ..Default::default()
        }))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::LogicalPlan;

    #[test]
    fn scan_to_plan_has_read_rel() {
        let plan = LogicalPlan::Scan {
            source: "warehouse.public.t".into(),
        };
        let p = to_plan(&plan).expect("plan");
        assert_eq!(p.relations.len(), 1);
    }

    #[test]
    fn aggregate_unsupported() {
        let plan = LogicalPlan::Aggregate {
            input: Box::new(LogicalPlan::Scan {
                source: "t".into(),
            }),
            group_by: vec![],
            aggregates: vec![],
        };
        assert!(to_plan(&plan).is_err());
    }
}
