//! Rendering: everything here takes a finished [`Scene`] (a grid plus the texts around it).

pub mod ascii;
pub mod font;
#[cfg(feature = "gif")]
pub mod gif;
pub mod svg;

use crate::grid::{Grid, LangId};
use crate::palette::{LangPalette, PotGlaze};
use crate::snapshot::AccountKind;

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
    /// User or organization (an organization gets an ORG tag and its repo count).
    pub kind: AccountKind,
    /// Public repos, shown for organizations.
    pub repos: u32,
}

impl Header {
    /// The stats line under the login.
    pub fn stats(&self) -> String {
        match self.kind {
            AccountKind::User => format!("{} commits · {}y", self.commits, self.age_years),
            AccountKind::Org => format!(
                "{} commits · {} repos · {}y",
                self.commits, self.repos, self.age_years
            ),
        }
    }
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

impl LegendItem {
    /// The legend text, at most `max` characters: a long repo name is cut and ends in `ellipsis`,
    /// and the language goes when even a short name would not fit.
    pub fn label(&self, max: usize, ellipsis: &str) -> String {
        let cut = |name: &str, room: usize| -> String {
            if name.chars().count() <= room {
                name.to_string()
            } else {
                let keep = room.saturating_sub(ellipsis.chars().count());
                name.chars().take(keep).collect::<String>() + ellipsis
            }
        };
        match self {
            LegendItem::Branch {
                repo,
                language,
                commits,
                ..
            } => {
                let tail = format!(" · {language} · {commits}");
                let room = max.saturating_sub(tail.chars().count());
                if room >= 6 {
                    cut(repo, room) + &tail
                } else {
                    let tail = format!(" · {commits}");
                    cut(repo, max.saturating_sub(tail.chars().count())) + &tail
                }
            }
            LegendItem::Pruned { repo } => format!("pruned · {}", cut(repo, max.saturating_sub(9))),
        }
    }
}

/// Everything one image shows.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_legend_labels_fit() {
        let b = LegendItem::Branch {
            repo: "claude-plugins-official".into(),
            language: "python".into(),
            commits: 4658,
            lang: 0,
        };
        let l = b.label(27, "…");
        assert_eq!(l.chars().count(), 27, "{l}");
        assert!(
            l.starts_with("claude-pl") && l.ends_with("· python · 4658"),
            "{l}"
        );
        let short = LegendItem::Branch {
            repo: "mifi".into(),
            language: "kotlin".into(),
            commits: 9,
            lang: 0,
        };
        assert_eq!(short.label(27, "…"), "mifi · kotlin · 9");
    }
}

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
    pub glaze: PotGlaze,
}
