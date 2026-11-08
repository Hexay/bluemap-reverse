//! Dimension ids, parsed once at the edge (CLI flag, map guess) and passed around typed.

use std::convert::Infallible;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dimension {
    Overworld,
    Nether,
    End,
    /// A datapack dimension, keeping its id as given.
    Other(String),
}

impl Dimension {
    /// A bare name takes the `minecraft` namespace, as in Minecraft's own resource locations.
    pub fn parse(id: &str) -> Self {
        match id.strip_prefix("minecraft:").unwrap_or(id) {
            "overworld" => Self::Overworld,
            "the_nether" => Self::Nether,
            "the_end" => Self::End,
            _ => Self::Other(id.to_owned()),
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Overworld => "minecraft:overworld",
            Self::Nether => "minecraft:the_nether",
            Self::End => "minecraft:the_end",
            Self::Other(id) => id,
        }
    }

    /// (namespace, name) of the id.
    pub fn namespaced(&self) -> (&str, &str) {
        let id = self.id();
        id.split_once(':').unwrap_or(("minecraft", id))
    }
}

impl FromStr for Dimension {
    type Err = Infallible;

    fn from_str(id: &str) -> Result<Self, Infallible> {
        Ok(Self::parse(id))
    }
}

impl From<&str> for Dimension {
    fn from(id: &str) -> Self {
        Self::parse(id)
    }
}

impl fmt::Display for Dimension {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.id())
    }
}

#[cfg(test)]
#[path = "dimension_tests.rs"]
mod tests;
