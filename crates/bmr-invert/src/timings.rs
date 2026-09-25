//! Named stage durations, collected by the pipeline and reported by the CLI (`--timings`).

use std::time::{Duration, Instant};

#[derive(Debug, Default, Clone)]
pub struct Timings(pub Vec<(String, Duration)>);

impl Timings {
    /// Run `f`, record its duration under `name`.
    pub fn time<T>(&mut self, name: &str, f: impl FnOnce() -> T) -> T {
        let t = Instant::now();
        let out = f();
        self.0.push((name.to_owned(), t.elapsed()));
        out
    }

    pub fn extend(&mut self, prefix: &str, other: Timings) {
        self.0.extend(other.0.into_iter().map(|(n, d)| (format!("{prefix}.{n}"), d)));
    }
}
