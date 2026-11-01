//! Rendering: everything here takes a finished [`Scene`] (a grid plus the texts around it).

pub mod ascii;
pub mod font;
#[cfg(feature = "gif")]
pub mod gif;
pub mod svg;

use crate::grid::{Grid, LangId};
use crate::palette::LangPalette;

/// The tree's phase, computed daily from the data (never accumulated in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    /// Account under 30 days old or under 10 commits.
    Sapling,
    Growing,
    /// Streak of 21+ days; carries the streak length.
    Bloom {
        streak_days: u32,
    },
    /// 60+ days without activity; carries the idle days.
    Dormant {
        idle_days: u32,
    },
}

impl Phase {
    /// The badge text in the header.
    pub fn badge(&self) -> String {
        match self {
            Phase::Sapling => "sapling".into(),
            Phase::Growing => "growing".into(),
            Phase::Bloom { streak_days } => format!("in bloom · {streak_days}d"),
            Phase::Dormant { idle_days } => format!("dormant · {idle_days}d idle"),
        }
    }
}

/// The header line: who, how much, how old, which phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub login: String,
    pub commits: u64,
    pub age_years: u32,
    pub phase: Phase,
}

/// One line of the legend under the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegendItem {
    /// A living branch: its pad colours, repo, language and commit count.
    Branch {
        repo: String,
        language: String,
        commits: u64,
        lang: LangId,
    },
    /// A pruned (archived or deleted) repo.
    Pruned { repo: String },
}

/// Everything one image shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub grid: Grid,
    /// Pad palettes, indexed by the grid's `LangId`s.
    pub langs: Vec<LangPalette>,
    pub header: Header,
    /// Up to 4 lines (two columns, two rows); extra items are not drawn.
    pub legend: Vec<LegendItem>,
    /// Cells that grew on the last grown day `(x, y)`; they pulse in the animated image.
    pub fresh: Vec<(usize, usize)>,
}
