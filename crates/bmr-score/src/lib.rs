//! Block-by-block scoring of a reconstructed world against the original (docs/architecture.md "Scoring").

mod compare;
mod report;

pub use compare::{Cell, ColumnFilter, Scope, score};
pub use report::{Accuracy, Confusion, Hits, Overlap, Report};
