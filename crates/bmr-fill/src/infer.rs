//! A map's render settings, which sites do not publish: BlueMap's defaults for the dimension (`Profile`),
//! corrected where the drawn faces contradict them. Only contradictions are acted on; the absence of faces
//! never moves a setting, since unrendered terrain looks the same as culled terrain.
//! - a cave face (`DrawnHeights::cave`) at y below the cave cut-off → caves are drawn down to y
//! - faces inside the render mask → it starts above the highest of them, or there is none
//! - faces outside the build height → it reaches them (whole 16-block sections)

use bmr_invert::drawn::DrawnHeights;

use crate::profile::Profile;

/// Triangles at one height that count as evidence: a lone stray quad does not (one dark bedrock underside
/// on the default-rendered `context` fixture); a drawn roof or cave layer has thousands.
const MIN_TRIANGLES: u64 = 16;

/// Settings given on the command line; `None` = infer.
#[derive(Debug, Default, Clone, Copy)]
pub struct Given {
    pub min_y: Option<i32>,
    pub max_y: Option<i32>,
    pub cave_y: Option<i32>,
    /// `Some(None)` = no mask.
    pub mask: Option<Option<(i32, i32)>>,
}

impl Given {
    pub fn complete(&self) -> bool {
        self.min_y.is_some() && self.max_y.is_some() && self.cave_y.is_some() && self.mask.is_some()
    }

    pub fn apply(&self, p: Profile) -> Profile {
        Profile {
            min_y: self.min_y.unwrap_or(p.min_y),
            max_y: self.max_y.unwrap_or(p.max_y),
            cave_y: self.cave_y.unwrap_or(p.cave_y),
            mask: self.mask.unwrap_or(p.mask),
            ..p
        }
    }
}

pub struct Inferred {
    pub profile: Profile,
    /// One line: every setting in use, the inferred ones marked.
    pub summary: String,
    /// The inferred settings that differ from BlueMap's defaults.
    pub changed: Vec<&'static str>,
}

pub fn infer(defaults: Profile, drawn: &DrawnHeights, given: &Given) -> Inferred {
    let mut p = given.apply(defaults);
    let mut changed = Vec::new();
    if let Some((lo, hi)) = drawn.range() {
        if given.min_y.is_none() && lo < p.min_y {
            p.min_y = lo.div_euclid(16) * 16;
            changed.push("min-y");
        }
        if given.max_y.is_none() && hi > p.max_y {
            p.max_y = hi.div_euclid(16) * 16 + 15;
            changed.push("max-y");
        }
    }
    if given.mask.is_none()
        && let Some((lo, hi)) = p.mask
        && let Some(top) = drawn.faces.range(lo..=hi).filter(|e| *e.1 >= MIN_TRIANGLES).map(|(&y, _)| y).next_back()
    {
        p.mask = (top < hi).then_some((top + 1, hi));
        changed.push("mask-y");
    }
    if given.cave_y.is_none()
        && let Some((&lowest, _)) = drawn.cave.iter().find(|e| *e.1 >= MIN_TRIANGLES)
        && lowest < p.cave_y
    {
        p.cave_y = lowest;
        changed.push("cave-y");
    }
    let summary = describe(&p, given);
    Inferred { profile: p, summary, changed }
}

fn describe(p: &Profile, given: &Given) -> String {
    let mark = |given: bool| if given { " (given)" } else { "" };
    let caves =
        if p.cave_y <= p.min_y { "caves drawn".to_string() } else { format!("caves hidden below y {}", p.cave_y) };
    let mask = p.mask.map_or("no render mask".to_string(), |(lo, hi)| format!("y {lo}..{hi} masked"));
    format!(
        "y {}..{}{}, {caves}{}, {mask}{}",
        p.min_y,
        p.max_y,
        mark(given.min_y.is_some() && given.max_y.is_some()),
        mark(given.cave_y.is_some()),
        mark(given.mask.is_some())
    )
}

#[cfg(test)]
#[path = "infer_tests.rs"]
mod tests;
