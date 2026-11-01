//! The two GraphQL queries of the RFC and their response types.

use super::{ApiError, Client};
use serde::Deserialize;
use std::collections::BTreeMap;
use time::format_description::well_known::Rfc3339;
use time::{Date, Duration, Month, OffsetDateTime, UtcOffset};

/// Query 1: profile and owned public repos, 100 per page (RFC fields plus `login` and
/// `archivedAt`, the day pruning happened).
pub const PROFILE: &str = r#"query($login: String!, $after: String) {
  user(login: $login) {
    login
    createdAt
    repositories(first: 100, after: $after, ownerAffiliations: OWNER,
                 privacy: PUBLIC, orderBy: {field: PUSHED_AT, direction: DESC}) {
      pageInfo { hasNextPage endCursor }
      nodes {
        id name isFork isArchived archivedAt stargazerCount pushedAt createdAt
        primaryLanguage { name color }
      }
    }
  }
}"#;

/// Pages of query 1 at most (10 000 repos).
const MAX_PAGES: usize = 100;

/// One `contributionsCollection` window (≤ 1 year) under a GraphQL alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub alias: String,
    pub from: OffsetDateTime,
    pub to: OffsetDateTime,
    /// Also fetch the contribution calendar of the window.
    pub calendar: bool,
}

fn start_of(d: Date, offset: UtcOffset) -> OffsetDateTime {
    d.midnight().assume_offset(offset)
}

fn end_of(d: Date, offset: UtcOffset) -> OffsetDateTime {
    start_of(d, offset) + Duration::seconds(86_399)
}

/// The whole calendar year `year` (alias `y2025`), with its calendar.
pub fn year_window(year: i32, offset: UtcOffset) -> Window {
    let jan1 = Date::from_calendar_date(year, Month::January, 1).unwrap_or(Date::MIN);
    let dec31 = Date::from_calendar_date(year, Month::December, 31).unwrap_or(Date::MIN);
    Window {
        alias: format!("y{year}"),
        from: start_of(jan1, offset),
        to: end_of(dec31, offset),
        calendar: true,
    }
}

/// The daily windows ending on `date`: `current` (1 Jan..date, with calendar), `last365`
/// (with calendar), `last28` and `today`.
pub fn daily_windows(date: Date, offset: UtcOffset) -> Vec<Window> {
    let jan1 = Date::from_calendar_date(date.year(), Month::January, 1).unwrap_or(date);
    let w = |alias: &str, from: Date, calendar: bool| Window {
        alias: alias.into(),
        from: start_of(from, offset),
        to: end_of(date, offset),
        calendar,
    };
    vec![
        w("current", jan1, true),
        w("last365", date - Duration::days(364), true),
        w("last28", date - Duration::days(27), false),
        w("today", date, false),
    ]
}

/// Query 2 with one aliased `contributionsCollection` per window.
pub fn contributions_query(windows: &[Window]) -> String {
    let mut q = String::from("query($login: String!) {\n  user(login: $login) {\n");
    for w in windows {
        let from = w.from.format(&Rfc3339).unwrap_or_default();
        let to = w.to.format(&Rfc3339).unwrap_or_default();
        let frag = if w.calendar { "WithCalendar" } else { "ByRepo" };
        q.push_str(&format!(
            "    {}: contributionsCollection(from: \"{from}\", to: \"{to}\") {{ ...{frag} }}\n",
            w.alias
        ));
    }
    q.push_str("  }\n}\n");
    let any_cal = windows.iter().any(|w| w.calendar);
    let any_plain = windows.iter().any(|w| !w.calendar);
    let by_repo = "commitContributionsByRepository(maxRepositories: 100) {\n    repository { id }\n    contributions { totalCount }\n  }";
    if any_cal {
        q.push_str(&format!(
            "fragment WithCalendar on ContributionsCollection {{\n  contributionCalendar {{ weeks {{ contributionDays {{ date contributionCount }} }} }}\n  {by_repo}\n}}\n"
        ));
    }
    if any_plain {
        q.push_str(&format!(
            "fragment ByRepo on ContributionsCollection {{\n  {by_repo}\n}}\n"
        ));
    }
    q
}

/// A repo from query 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub id: String,
    pub name: String,
    pub is_fork: bool,
    pub is_archived: bool,
    pub archived_at: Option<OffsetDateTime>,
    pub stars: u64,
    pub language: Option<String>,
    pub color: Option<String>,
}

/// Query 1, all pages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub login: String,
    pub created: OffsetDateTime,
    pub repos: Vec<Repo>,
}

/// One window's result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Contribs {
    pub calendar: Vec<(Date, u32)>,
    pub by_repo: BTreeMap<String, u64>,
}

#[derive(Deserialize)]
struct ProfileData {
    user: Option<UserNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserNode {
    login: String,
    created_at: String,
    repositories: RepoConn,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepoConn {
    page_info: PageInfo,
    nodes: Vec<Option<RepoNode>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RepoNode {
    id: String,
    name: String,
    is_fork: bool,
    is_archived: bool,
    archived_at: Option<String>,
    stargazer_count: u64,
    primary_language: Option<LangNode>,
}

#[derive(Deserialize)]
struct LangNode {
    name: String,
    color: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CollNode {
    contribution_calendar: Option<CalNode>,
    commit_contributions_by_repository: Vec<ByRepoNode>,
}

#[derive(Deserialize)]
struct CalNode {
    weeks: Vec<WeekNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WeekNode {
    contribution_days: Vec<DayNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DayNode {
    date: String,
    contribution_count: u32,
}

#[derive(Deserialize)]
struct ByRepoNode {
    repository: IdNode,
    contributions: TotalNode,
}

#[derive(Deserialize)]
struct IdNode {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TotalNode {
    total_count: u64,
}

fn decode<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> Result<T, ApiError> {
    serde_json::from_value(v).map_err(|e| ApiError::Decode(e.to_string()))
}

fn timestamp(s: &str) -> Result<OffsetDateTime, ApiError> {
    OffsetDateTime::parse(s, &Rfc3339).map_err(|e| ApiError::Decode(format!("date {s:?}: {e}")))
}

fn not_found(e: ApiError, login: &str) -> ApiError {
    match e {
        ApiError::GraphQl { kind, .. } if kind == "NOT_FOUND" => {
            ApiError::UserNotFound(login.to_string())
        }
        other => other,
    }
}

/// Runs query 1 over all pages.
pub fn profile(client: &Client, login: &str) -> Result<Profile, ApiError> {
    let mut after: Option<String> = None;
    let mut out: Option<Profile> = None;
    for page in 0..MAX_PAGES {
        let data = client
            .query(
                &format!("profile p{}", page + 1),
                PROFILE,
                serde_json::json!({ "login": login, "after": after }),
            )
            .map_err(|e| not_found(e, login))?;
        let d: ProfileData = decode(data)?;
        let user = d
            .user
            .ok_or_else(|| ApiError::UserNotFound(login.to_string()))?;
        let p = match &mut out {
            Some(p) => p,
            None => out.insert(Profile {
                login: user.login.clone(),
                created: timestamp(&user.created_at)?,
                repos: Vec::new(),
            }),
        };
        for n in user.repositories.nodes.into_iter().flatten() {
            p.repos.push(Repo {
                id: n.id,
                name: n.name,
                is_fork: n.is_fork,
                is_archived: n.is_archived,
                archived_at: n.archived_at.as_deref().map(timestamp).transpose()?,
                stars: n.stargazer_count,
                language: n.primary_language.as_ref().map(|l| l.name.clone()),
                color: n.primary_language.and_then(|l| l.color),
            });
        }
        if !user.repositories.page_info.has_next_page {
            break;
        }
        after = user.repositories.page_info.end_cursor;
        if after.is_none() {
            break;
        }
    }
    out.ok_or_else(|| ApiError::UserNotFound(login.to_string()))
}

/// Runs query 2 for the given windows (one request), keyed by alias.
pub fn contributions(
    client: &Client,
    login: &str,
    windows: &[Window],
) -> Result<BTreeMap<String, Contribs>, ApiError> {
    let names: Vec<&str> = windows.iter().map(|w| w.alias.as_str()).collect();
    let data = client
        .query(
            &format!("contributions {}", names.join("+")),
            &contributions_query(windows),
            serde_json::json!({ "login": login }),
        )
        .map_err(|e| not_found(e, login))?;
    let user = data
        .get("user")
        .filter(|u| !u.is_null())
        .ok_or_else(|| ApiError::UserNotFound(login.to_string()))?;
    let mut out = BTreeMap::new();
    for w in windows {
        let node = user
            .get(&w.alias)
            .cloned()
            .ok_or_else(|| ApiError::Decode(format!("missing window {}", w.alias)))?;
        let c: CollNode = decode(node)?;
        let mut calendar = Vec::new();
        for week in c.contribution_calendar.map(|c| c.weeks).unwrap_or_default() {
            for day in week.contribution_days {
                let d = Date::parse(&day.date, crate::snapshot::YMD)
                    .map_err(|e| ApiError::Decode(format!("day {:?}: {e}", day.date)))?;
                calendar.push((d, day.contribution_count));
            }
        }
        let by_repo = c
            .commit_contributions_by_repository
            .into_iter()
            .map(|r| (r.repository.id, r.contributions.total_count))
            .collect();
        out.insert(w.alias.clone(), Contribs { calendar, by_repo });
    }
    Ok(out)
}
