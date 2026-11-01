//! One day's normalised view of an account's public activity. `fetch` builds it from GitHub;
//! every test builds it from a fixture. Nothing downstream ever touches the network.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use time::Date;

time::serde::format_description!(pub ymd, Date, "[year]-[month]-[day]");

/// A GitHub user or organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    #[default]
    User,
    Org,
}

/// `YYYY-MM-DD`, for parsing dates outside serde.
pub const YMD: &[time::format_description::BorrowedFormatItem<'static>] =
    time::macros::format_description!("[year]-[month]-[day]");

/// A repo as seen on the snapshot day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoSignal {
    /// GitHub node id (`R_kgDO…`), stable across renames.
    pub id: String,
    pub name: String,
    pub is_fork: bool,
    pub is_archived: bool,
    pub stars: u64,
    /// Primary language name (`Rust`), if GitHub knows one.
    pub language: Option<String>,
    /// Linguist colour of that language (`#dea584`).
    pub color: Option<String>,
    /// The owner's commits in this repo over the whole history (summed year by year).
    pub commits_total: u64,
    /// The owner's commits in this repo over the 28 days up to the snapshot date.
    pub commits_28d: u32,
    /// Commits on the snapshot date itself.
    pub commits_today: u32,
    /// The owner's commits in this repo over the 365 days up to the snapshot date (branch ranking).
    #[serde(default)]
    pub commits_365d: u64,
    /// The owner's commits per calendar year (`{"2025": 412}`); finished years go to
    /// `TreeState::year_cache`, `plant` spreads them over the calendar to replay history.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub commits_by_year: BTreeMap<String, u64>,
    /// The day GitHub says the repo was archived (`archivedAt`), when archived.
    #[serde(
        default,
        with = "crate::state::opt_ymd",
        skip_serializing_if = "Option::is_none"
    )]
    pub archived_at: Option<Date>,
}

/// Everything `grow` needs for one day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub login: String,
    /// The day this snapshot describes (a full UTC day, or a local day with `--tz`).
    #[serde(with = "ymd")]
    pub date: Date,
    #[serde(with = "ymd")]
    pub account_created: Date,
    /// Public repos owned by the account (forks and archives included; they decide pot width).
    pub public_repos: u32,
    pub repos: Vec<RepoSignal>,
    /// Contribution calendar, ascending dates, at least the last 400 days when available.
    pub calendar: Vec<CalendarDay>,
    /// Calendar years whose per-repo counts `RepoSignal::commits_by_year` holds in full (a year
    /// listed here with no entry for a repo means 0 commits). Finished ones go to the year cache.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub years: Vec<i32>,
    /// User or organization. An organization has no contribution calendar: `calendar` is
    /// synthesised from default-branch commits of its top repos over the last 70 days.
    #[serde(default)]
    pub kind: AccountKind,
}

/// One day of the contribution calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarDay {
    #[serde(with = "ymd")]
    pub date: Date,
    pub count: u32,
}
