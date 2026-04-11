//! Emit SQL or Substrait from a [`LogicalPlan`](crate::plan::LogicalPlan).
//!
//! - **SQL** ([`sql`]): dependency-free string emission; predicates and aggregates are passed through
//!   as SQL text (same representation as in the planner).
//! - **Substrait** ([`substrait`]): optional; enable with the `substrait` Cargo feature. v1 only
//!   maps a bare [`LogicalPlan::Scan`] to a minimal [`substrait::proto::Plan`]; other shapes return
//!   [`EmitError::Unsupported`] until expressions are structured beyond opaque SQL strings.

mod error;
pub mod sql;

#[cfg(feature = "substrait")]
pub mod substrait;

pub use error::EmitError;
