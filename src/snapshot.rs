//! One day's normalised view of an account's public activity. `fetch` builds it from GitHub;
//! every test builds it from a fixture. Nothing downstream ever touches the network.

use serde::{Deserialize, Serialize};
use time::Date;

time::serde::format_description!(pub ymd, Date, "[year]-[month]-[day]");

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
}

/// One day of the contribution calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarDay {
    #[serde(with = "ymd")]
    pub date: Date,
    pub count: u32,
}
