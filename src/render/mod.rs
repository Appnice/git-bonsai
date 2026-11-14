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

/// One language a legend row names, biggest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegendLang {
    /// Palette index, for the swatch.
    pub lang: LangId,
    /// Language name, lower case, as shown.
    pub name: String,
    /// Bytes of that language in the repo (0 when the repo has only one language).
    pub bytes: u64,
}

/// One line of the legend under the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegendItem {
    /// A living branch: its pad colours, repo, languages and commit count.
    Branch {
        repo: String,
        commits: u64,
        /// The languages its pad carries, biggest first; a single entry means a single colour.
        langs: Vec<LegendLang>,
    },
    /// A pruned (archived or deleted) repo.
    Pruned { repo: String },
}

impl LegendItem {
    /// The legend text, at most `max` characters: a long repo name is cut and ends in `ellipsis`,
    /// and the languages shorten (`html +2`, then `3 langs`) before the name does.
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
                commits,
                langs,
            } => {
                let mut tails: Vec<String> = Vec::new();
                if let Some(first) = langs.first() {
                    if langs.len() > 1 {
                        let all: Vec<&str> = langs.iter().map(|l| l.name.as_str()).collect();
                        tails.push(format!(" · {} · {commits}", all.join(", ")));
                        tails.push(format!(" · {} +{} · {commits}", first.name, langs.len() - 1));
                        tails.push(format!(" · {} langs · {commits}", langs.len()));
                    }
                    tails.push(format!(" · {} · {commits}", first.name));
                }
                tails.push(format!(" · {commits}"));
                for tail in tails {
                    let room = max.saturating_sub(tail.chars().count());
                    if room >= 6 {
                        return cut(repo, room) + &tail;
                    }
                }
                cut(repo, max)
            }
            LegendItem::Pruned { repo } => format!("pruned · {}", cut(repo, max.saturating_sub(9))),
        }
    }

    /// The tooltip: the wording the row has no room for, with the share of every language.
    pub fn title(&self) -> String {
        match self {
            LegendItem::Branch {
                repo,
                commits,
                langs,
            } => {
                let total: u64 = langs.iter().map(|l| l.bytes).sum();
                if langs.len() > 1 && total > 0 {
                    let parts: Vec<String> = langs
                        .iter()
                        .map(|l| format!("{} {}%", l.name, l.bytes * 100 / total))
                        .collect();
                    format!("{repo} — {} · {commits} commits", parts.join(", "))
                } else if let Some(l) = langs.first() {
                    format!("{repo} · {} · {commits} commits", l.name)
                } else {
                    format!("{repo} · {commits} commits")
                }
            }
            LegendItem::Pruned { repo } => format!("{repo} — pruned"),
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
            commits: 4658,
            langs: vec![LegendLang {
                lang: 0,
                name: "python".into(),
                bytes: 0,
            }],
        };
        let l = b.label(27, "…");
        assert_eq!(l.chars().count(), 27, "{l}");
        assert!(
            l.starts_with("claude-pl") && l.ends_with("· python · 4658"),
            "{l}"
        );
        let short = LegendItem::Branch {
            repo: "mifi".into(),
            commits: 9,
            langs: vec![LegendLang {
                lang: 0,
                name: "kotlin".into(),
                bytes: 0,
            }],
        };
        assert_eq!(short.label(27, "…"), "mifi · kotlin · 9");
    }

    #[test]
    fn polyglot_labels_shorten_before_the_repo_name() {
        let roam = LegendItem::Branch {
            repo: "roam".into(),
            commits: 2253,
            langs: vec![
                LegendLang {
                    lang: 0,
                    name: "html".into(),
                    bytes: 2_934_737,
                },
                LegendLang {
                    lang: 1,
                    name: "typescript".into(),
                    bytes: 2_120_709,
                },
                LegendLang {
                    lang: 2,
                    name: "plpgsql".into(),
                    bytes: 1_812_788,
                },
            ],
        };
        // Three names do not fit in 27 characters, so the row counts the rest instead.
        assert_eq!(roam.label(27, "…"), "roam · html +2 · 2253");
        // Wider rows name all of them.
        assert_eq!(
            roam.label(45, "…"),
            "roam · html, typescript, plpgsql · 2253"
        );
        assert_eq!(
            roam.title(),
            "roam — html 42%, typescript 31%, plpgsql 26% · 2253 commits"
        );
        let two = LegendItem::Branch {
            repo: "skyhook".into(),
            commits: 45,
            langs: vec![
                LegendLang {
                    lang: 0,
                    name: "javascript".into(),
                    bytes: 419_530,
                },
                LegendLang {
                    lang: 1,
                    name: "css".into(),
                    bytes: 41_077,
                },
            ],
        };
        assert_eq!(two.label(45, "…"), "skyhook · javascript, css · 45");
        assert_eq!(two.label(27, "…"), "skyhook · javascript +1 · 45");
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
