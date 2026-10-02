//! Minimal safe binding to cubiomes (xpple fork, vendored): overworld structure biome viability.
#![warn(clippy::undocumented_unsafe_blocks)]

use std::ffi::{CString, c_char, c_int, c_void};
use std::ptr::NonNull;

unsafe extern "C" {
    fn bmr_generator_new(mc: c_int) -> *mut c_void;
    fn bmr_generator_free(g: *mut c_void);
    fn bmr_apply_overworld_seed(g: *mut c_void, seed: u64);
    fn bmr_structure_type(set: *const c_char) -> c_int;
    fn str2mc(s: *const c_char) -> c_int;
    fn isViableStructurePos(structure_type: c_int, g: *mut c_void, x: c_int, z: c_int, flags: u32) -> c_int;
}

/// cubiomes' 1.18+ versions (each covers its patch range); picks the newest at or below the target.
const VERSIONS: &[((u32, u32, u32), &str)] = &[
    ((1, 18, 0), "1.18.2"),
    ((1, 19, 0), "1.19.2"),
    ((1, 19, 3), "1.19.4"),
    ((1, 20, 0), "1.20.6"),
    ((1, 21, 0), "1.21.1"),
    ((1, 21, 2), "1.21.3"),
    ((1, 21, 4), "1.21.4"),
    ((1, 21, 5), "1.21.5"),
    ((1, 21, 6), "1.21.6"),
    ((1, 21, 9), "1.21.9"),
    ((1, 21, 11), "1.21.11"),
    ((26, 1, 0), "26.1"),
    ((26, 2, 0), "26.2"),
    ((26, 3, 0), "26.3"),
];

fn mc_id(version: (u32, u32, u32)) -> Option<c_int> {
    let (_, name) = VERSIONS.iter().rev().find(|(v, _)| *v <= version)?;
    let name = CString::new(*name).ok()?;
    // SAFETY: `name` is NUL-terminated and outlives the call; str2mc only strcmp's it against a static table.
    let id = unsafe { str2mc(name.as_ptr()) };
    (id > 0).then_some(id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StructureType(c_int);

impl StructureType {
    /// `None` for sets cubiomes has no overworld biome check for (nether/end sets, unknown names).
    pub fn for_set(set: &str) -> Option<Self> {
        let name = CString::new(set).ok()?;
        // SAFETY: `name` is NUL-terminated and outlives the call; the shim only strcmp's it against a static table.
        let t = unsafe { bmr_structure_type(name.as_ptr()) };
        (t >= 0).then_some(Self(t))
    }
}

/// One overworld generator; not thread-safe to share, so make one per thread.
pub struct Generator(NonNull<c_void>);

// SAFETY: owns its malloc'd Generator exclusively; cubiomes' only mutable global (finders.c `provider`) is never
// written by this crate, so moving the pointer to another thread is sound. Not Sync: queries mutate `g`.
unsafe impl Send for Generator {}

impl Generator {
    /// Unseeded until `apply_seed`; `None` for versions before 1.18.
    pub fn new(version: (u32, u32, u32)) -> Option<Self> {
        let mc = mc_id(version)?;
        // SAFETY: `mc` is a 1.18+ id from str2mc, which setupGenerator handles; NULL (OOM) is checked below.
        let g = unsafe { bmr_generator_new(mc) };
        NonNull::new(g).map(Self)
    }

    pub fn apply_seed(&mut self, seed: i64) {
        // SAFETY: `self.0` is a live Generator from bmr_generator_new, exclusively borrowed for the call.
        unsafe { bmr_apply_overworld_seed(self.0.as_ptr(), seed as u64) }
    }

    /// Whether the biomes at a start chunk allow the structure (cubiomes' approximation of vanilla's check).
    /// Meaningless (but sound) before `apply_seed`.
    pub fn is_viable(&mut self, structure: StructureType, chunk: (i32, i32)) -> bool {
        // SAFETY: `self.0` is live and exclusively borrowed (cubiomes mutates then restores it); `structure.0` comes
        // from the shim's table. Unseeded, g->dim is DIM_UNDEF and genBiomes errors instead of reading unseeded noise.
        unsafe { isViableStructurePos(structure.0, self.0.as_ptr(), chunk.0 * 16, chunk.1 * 16, 0) != 0 }
    }
}

impl Drop for Generator {
    fn drop(&mut self) {
        // SAFETY: `self.0` came from bmr_generator_new's malloc and is freed exactly once, here.
        unsafe { bmr_generator_free(self.0.as_ptr()) }
    }
}
