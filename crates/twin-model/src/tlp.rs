//! Traffic-light state used everywhere in the twin.
use serde::{Deserialize, Serialize};

/// Green: normal. Yellow: on trend to be impacted (a leading indicator or a forecast).
/// Orange: degraded, running with a limit on power, cooling, network or performance.
/// Red: out. Unknown: no trustworthy data (comms loss, bad quality).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Tlp {
    #[default]
    Green,
    Unknown,
    Yellow,
    Orange,
    Red,
}

impl Tlp {
    pub fn label(self) -> &'static str {
        match self {
            Tlp::Green => "Normal",
            Tlp::Unknown => "No data",
            Tlp::Yellow => "On trend to be impacted",
            Tlp::Orange => "Degraded",
            Tlp::Red => "Out",
        }
    }
    /// Healthy enough to carry load right now.
    pub fn healthy(self) -> bool {
        matches!(self, Tlp::Green | Tlp::Yellow | Tlp::Unknown)
    }
    pub fn available(self) -> bool {
        self != Tlp::Red
    }
}
