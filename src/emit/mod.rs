//! Emit SQL or Substrait from a [`LogicalPlan`](crate::plan::LogicalPlan).
//!
//! - **SQL** ([`sql`]): dependency-free string emission; predicates and aggregates are passed through
//!   as SQL text (same representation as in the planner).
//! - **Substrait** ([`substrait`]): primary; enable with the `substrait` Cargo feature. Emits
//!   scan / join / filter / aggregate from the structured [`LogicalPlan`](crate::plan::LogicalPlan).
//! - **SQL** ([`sql`]): secondary; derived from the same structured plan.

mod error;
pub mod sql;

#[cfg(feature = "substrait")]
pub mod substrait;

pub use error::EmitError;
