//! Block-by-block scoring of a reconstructed world against the original (docs/plan.md "Scoring").

mod compare;
mod report;

pub use compare::{ColumnFilter, score};
pub use report::{Accuracy, Confusion, Hits, Overlap, Report};
