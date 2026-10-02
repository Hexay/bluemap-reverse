//! Named stage durations + process working set after each stage, collected by the pipeline and reported
//! by the CLI (`--timings`). Memory is sampled, so it shows what a stage leaves resident, not its peak.

use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Stage {
    pub name: String,
    pub duration: Duration,
    /// Working set right after the stage (bytes), 0 where unsupported.
    pub resident: u64,
}

#[derive(Debug, Default, Clone)]
pub struct Timings(pub Vec<Stage>);

impl Timings {
    /// Run `f`, record its duration and the resident memory afterwards under `name`.
    pub fn time<T>(&mut self, name: &str, f: impl FnOnce() -> T) -> T {
        let t = Instant::now();
        let out = f();
        self.record(name, t.elapsed());
        out
    }

    pub fn record(&mut self, name: &str, duration: Duration) {
        self.0.push(Stage { name: name.to_owned(), duration, resident: resident_bytes() });
    }

    pub fn extend(&mut self, prefix: &str, other: Timings) {
        self.0.extend(other.0.into_iter().map(|s| Stage { name: format!("{prefix}.{}", s.name), ..s }));
    }

    /// Add `other`'s stages into same-named ones (durations summed, resident = max); new names appended.
    pub fn accumulate(&mut self, other: Timings) {
        for s in other.0 {
            match self.0.iter_mut().find(|m| m.name == s.name) {
                Some(m) => {
                    m.duration += s.duration;
                    m.resident = m.resident.max(s.resident);
                }
                None => self.0.push(s),
            }
        }
    }
}

#[cfg(windows)]
pub fn resident_bytes() -> u64 {
    #[repr(C)]
    struct Counters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set: usize,
        working_set: usize,
        rest: [usize; 6],
    }
    unsafe extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(process: isize, counters: *mut Counters, cb: u32) -> i32;
    }
    let mut c = Counters {
        cb: size_of::<Counters>() as u32,
        page_fault_count: 0,
        peak_working_set: 0,
        working_set: 0,
        rest: [0; 6],
    };
    // SAFETY: plain Win32 call writing into a correctly sized PROCESS_MEMORY_COUNTERS
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    if ok != 0 { c.working_set as u64 } else { 0 }
}

#[cfg(not(windows))]
pub fn resident_bytes() -> u64 {
    0
}
