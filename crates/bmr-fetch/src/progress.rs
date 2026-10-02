//! Progress events for the caller to render: the library never prints.

use std::sync::Arc;

pub enum Progress<'a> {
    /// A request failed and will be retried after a pause.
    Retry { url: &'a str, attempt: u32, of: u32, error: &'a anyhow::Error },
    /// Probing of one map's layer advanced (lod 0 = hires).
    Layer { map: &'a str, lod: u32, present: usize, empty: usize },
}

pub type OnProgress = Arc<dyn Fn(&Progress<'_>) + Send + Sync>;

pub(crate) fn emit(on: Option<&OnProgress>, p: Progress<'_>) {
    if let Some(f) = on {
        f(&p);
    }
}
