//! Progress output, silenced by `--quiet`. Results, warnings and errors always print.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bmr_fetch::{OnProgress, Progress};

static QUIET: AtomicBool = AtomicBool::new(false);

pub fn set_quiet(quiet: bool) {
    QUIET.store(quiet, Ordering::Relaxed);
}

pub fn quiet() -> bool {
    QUIET.load(Ordering::Relaxed)
}

/// `println!` unless `--quiet`.
macro_rules! progress {
    ($($arg:tt)*) => {
        if !$crate::ui::quiet() {
            println!($($arg)*);
        }
    };
}
pub(crate) use progress;

/// bmr-fetch's progress events rendered on stderr, or `None` under `--quiet`.
pub fn fetch_progress() -> Option<OnProgress> {
    (!quiet()).then(|| Arc::new(render_fetch) as OnProgress)
}

fn render_fetch(p: &Progress<'_>) {
    match p {
        Progress::Retry { url, attempt, of, error } => eprintln!("retry {attempt}/{of} {url}: {error:#}"),
        Progress::Layer { map, lod, present, empty } => {
            eprintln!("  {map} lod {lod}: {present} present, {empty} empty")
        }
    }
}
