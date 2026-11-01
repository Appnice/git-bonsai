//! GitHub GraphQL client (feature `network`): the only module that touches the network.
//!
//! A day's snapshot is query 1 (profile + repos, 100 per page) plus contribution windows: one
//! query per finished year missing from the state's `year_cache` (run 4 at a time), and one
//! daily query with the current year, the last 365 days, the last 28 days and the day itself
//! as aliases of query 2. A daily run of an account with ≤ 300 repos costs ≤ 4 queries.

mod queries;

use crate::snapshot::{CalendarDay, RepoSignal, Snapshot};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use time::{Date, UtcOffset};

pub use queries::{daily_windows, year_window, Window};

/// GitHub GraphQL endpoint.
pub const ENDPOINT: &str = "https://api.github.com/graphql";
/// Waits between attempts on 5xx / `RATE_LIMITED` / transport errors (3 retries).
pub const BACKOFF: [Duration; 3] = [
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
];
/// Year windows fetched in parallel.
const PARALLEL: usize = 4;

/// An API token. `Debug` and `Display` never show it.
#[derive(Clone)]
pub struct Token(String);

impl Token {
    /// `None` for an empty or blank string.
    pub fn new(s: impl Into<String>) -> Option<Token> {
        let s: String = s.into().trim().to_string();
        (!s.is_empty()).then_some(Token(s))
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(***)")
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

/// Why a fetch failed. Messages never contain the token.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ApiError {
    #[error("GitHub rejected the token (HTTP 401)")]
    Unauthorized,
    #[error("GitHub user {0:?} not found")]
    UserNotFound(String),
    #[error("GitHub answered HTTP {status}: {message}")]
    Http { status: u16, message: String },
    #[error("GitHub GraphQL error {kind}: {message}")]
    GraphQl { kind: String, message: String },
    #[error("GitHub still failing after {attempts} attempts: {last}")]
    Exhausted { attempts: u32, last: String },
    #[error("unexpected GitHub response: {0}")]
    Decode(String),
}

/// One HTTP POST of a GraphQL body. `Err` = transport failure (DNS, TLS, timeout).
pub trait Transport: Send + Sync {
    fn post(&self, body: &str) -> Result<(u16, String), String>;
}

/// The real transport: blocking reqwest with rustls.
pub struct HttpTransport {
    http: reqwest::blocking::Client,
    token: Token,
}

impl HttpTransport {
    pub fn new(token: Token) -> Result<HttpTransport, ApiError> {
        let http = reqwest::blocking::Client::builder()
            .user_agent(concat!("git-bonsai/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| ApiError::Decode(format!("HTTP client: {e}")))?;
        Ok(HttpTransport { http, token })
    }
}

impl Transport for HttpTransport {
    fn post(&self, body: &str) -> Result<(u16, String), String> {
        let resp = self
            .http
            .post(ENDPOINT)
            .bearer_auth(&self.token.0)
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .map_err(|e| e.without_url().to_string())?;
        let status = resp.status().as_u16();
        let text = resp.text().map_err(|e| e.without_url().to_string())?;
        Ok((status, text))
    }
}

type Sleep = Box<dyn Fn(Duration) + Send + Sync>;
type Log = Box<dyn Fn(&str) + Send + Sync>;

/// GraphQL client with retry, a query counter and an optional log sink.
pub struct Client {
    transport: Box<dyn Transport>,
    /// Scrubbed from every message as a second line of defence.
    secret: Option<String>,
    sleep: Sleep,
    log: Option<Log>,
    queries: AtomicU32,
}

impl Client {
    /// A client over HTTPS with the given token.
    pub fn new(token: Token) -> Result<Client, ApiError> {
        let secret = token.0.clone();
        let mut c = Client::with_transport(Box::new(HttpTransport::new(token)?));
        c.secret = Some(secret);
        Ok(c)
    }

    /// A client over any transport (tests use a fake one), sleeping for real between retries.
    pub fn with_transport(transport: Box<dyn Transport>) -> Client {
        Client {
            transport,
            secret: None,
            sleep: Box::new(std::thread::sleep),
            log: None,
            queries: AtomicU32::new(0),
        }
    }

    /// Replaces the sleep between retries (tests).
    pub fn sleep_with(mut self, f: impl Fn(Duration) + Send + Sync + 'static) -> Client {
        self.sleep = Box::new(f);
        self
    }

    /// Sends progress lines (query names, retries; never the token) to `f`.
    pub fn log_with(mut self, f: impl Fn(&str) + Send + Sync + 'static) -> Client {
        self.log = Some(Box::new(f));
        self
    }

    /// Marks the token to scrub from messages when a custom transport holds it.
    pub fn scrub(mut self, secret: &Token) -> Client {
        self.secret = Some(secret.0.clone());
        self
    }

    /// HTTP requests sent so far, retries included.
    pub fn queries(&self) -> u32 {
        self.queries.load(Ordering::Relaxed)
    }

    fn log(&self, msg: &str) {
        if let Some(l) = &self.log {
            l(&self.clean(msg));
        }
    }

    fn clean(&self, msg: &str) -> String {
        match &self.secret {
            Some(s) if !s.is_empty() => msg.replace(s.as_str(), "***"),
            _ => msg.to_string(),
        }
    }

    fn clean_err(&self, e: ApiError) -> ApiError {
        match e {
            ApiError::Http { status, message } => ApiError::Http {
                status,
                message: self.clean(&message),
            },
            ApiError::GraphQl { kind, message } => ApiError::GraphQl {
                kind: self.clean(&kind),
                message: self.clean(&message),
            },
            ApiError::Exhausted { attempts, last } => ApiError::Exhausted {
                attempts,
                last: self.clean(&last),
            },
            ApiError::Decode(m) => ApiError::Decode(self.clean(&m)),
            ApiError::UserNotFound(m) => ApiError::UserNotFound(self.clean(&m)),
            ApiError::Unauthorized => ApiError::Unauthorized,
        }
    }

    /// Runs one GraphQL query; returns its `data`. Retries 5xx, `RATE_LIMITED` and transport
    /// errors 3 times (2/4/8 s); any other 4xx or GraphQL error fails at once.
    pub fn query(
        &self,
        name: &str,
        query: &str,
        vars: serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
        let body = serde_json::json!({ "query": query, "variables": vars }).to_string();
        let mut last = String::new();
        for attempt in 0..=BACKOFF.len() {
            if attempt > 0 {
                let wait = BACKOFF[attempt - 1];
                self.log(&format!(
                    "{name}: retry {attempt} in {}s ({last})",
                    wait.as_secs()
                ));
                (self.sleep)(wait);
            }
            self.queries.fetch_add(1, Ordering::Relaxed);
            self.log(&format!("{name}: query"));
            match self.transport.post(&body) {
                Err(e) => last = format!("transport: {e}"),
                Ok((status, text)) => match classify(status, &text) {
                    Outcome::Data(d) => return Ok(d),
                    Outcome::Retry(why) => last = why,
                    Outcome::Fail(e) => return Err(self.clean_err(e)),
                },
            }
        }
        Err(self.clean_err(ApiError::Exhausted {
            attempts: BACKOFF.len() as u32 + 1,
            last,
        }))
    }
}

enum Outcome {
    Data(serde_json::Value),
    Retry(String),
    Fail(ApiError),
}

fn short(s: &str) -> String {
    s.chars().take(300).collect()
}

fn classify(status: u16, text: &str) -> Outcome {
    let json: Option<serde_json::Value> = serde_json::from_str(text).ok();
    let field = |j: &serde_json::Value, k: &str| -> String {
        short(j.get(k).and_then(|m| m.as_str()).unwrap_or(""))
    };
    if status >= 500 {
        return Outcome::Retry(format!("HTTP {status}"));
    }
    if (status == 403 || status == 429) && text.to_ascii_lowercase().contains("rate limit") {
        return Outcome::Retry(format!("HTTP {status} rate limited"));
    }
    if status == 401 {
        return Outcome::Fail(ApiError::Unauthorized);
    }
    if status >= 400 {
        let message = json
            .as_ref()
            .map(|j| field(j, "message"))
            .unwrap_or_default();
        return Outcome::Fail(ApiError::Http { status, message });
    }
    let Some(json) = json else {
        return Outcome::Fail(ApiError::Decode(format!("HTTP {status}, body is not JSON")));
    };
    if let Some(errs) = json
        .get("errors")
        .and_then(|e| e.as_array())
        .filter(|e| !e.is_empty())
    {
        if errs.iter().any(|e| field(e, "type") == "RATE_LIMITED") {
            return Outcome::Retry("RATE_LIMITED".into());
        }
        let first = &errs[0];
        return Outcome::Fail(ApiError::GraphQl {
            kind: field(first, "type"),
            message: field(first, "message"),
        });
    }
    match json.get("data") {
        Some(d) if !d.is_null() => Outcome::Data(d.clone()),
        _ => Outcome::Fail(ApiError::Decode("no data".into())),
    }
}

/// Per finished year, per repo id, the owner's commits (`TreeState::year_cache`).
pub type YearCache = BTreeMap<String, BTreeMap<String, u64>>;

/// Fetches the snapshot of `login` for `date` (a full day at `offset`). Years in `year_cache`
/// are taken from it; the others are queried, with their calendars.
pub fn fetch(
    client: &Client,
    login: &str,
    date: Date,
    offset: UtcOffset,
    year_cache: &YearCache,
) -> Result<Snapshot, ApiError> {
    let profile = queries::profile(client, login)?;
    let created = profile.created.to_offset(offset).date();
    let this_year = date.year();
    let missing: Vec<i32> = (created.year()..this_year)
        .filter(|y| !year_cache.contains_key(&y.to_string()))
        .collect();

    let mut windows: BTreeMap<String, queries::Contribs> = BTreeMap::new();
    for chunk in missing.chunks(PARALLEL) {
        let results: Vec<Result<(i32, queries::Contribs), ApiError>> = std::thread::scope(|s| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|y| {
                    let y = *y;
                    s.spawn(move || {
                        let w = year_window(y, offset);
                        let alias = w.alias.clone();
                        queries::contributions(client, login, &[w])
                            .map(|mut m| (y, m.remove(&alias).unwrap_or_default()))
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| {
                    h.join()
                        .unwrap_or_else(|_| Err(ApiError::Decode("worker thread panicked".into())))
                })
                .collect()
        });
        for r in results {
            let (y, c) = r?;
            windows.insert(format!("y{y}"), c);
        }
    }
    windows.extend(queries::contributions(
        client,
        login,
        &daily_windows(date, offset),
    )?);

    // Calendar: every queried window's days within [created, date].
    let mut cal: BTreeMap<Date, u32> = BTreeMap::new();
    for c in windows.values() {
        for (d, n) in &c.calendar {
            if *d >= created && *d <= date {
                cal.insert(*d, *n);
            }
        }
    }

    let count = |alias: &str, id: &str| -> u64 {
        windows
            .get(alias)
            .and_then(|c| c.by_repo.get(id))
            .copied()
            .unwrap_or(0)
    };
    let mut repos: Vec<RepoSignal> = profile
        .repos
        .iter()
        .map(|r| {
            let mut by_year = BTreeMap::new();
            for y in created.year()..=this_year {
                let key = y.to_string();
                let c = if y == this_year {
                    count("current", &r.id)
                } else if let Some(cache) = year_cache.get(&key) {
                    cache.get(&r.id).copied().unwrap_or(0)
                } else {
                    count(&format!("y{y}"), &r.id)
                };
                if c > 0 {
                    by_year.insert(key, c);
                }
            }
            RepoSignal {
                id: r.id.clone(),
                name: r.name.clone(),
                is_fork: r.is_fork,
                is_archived: r.is_archived,
                stars: r.stars,
                language: r.language.clone(),
                color: r.color.clone(),
                commits_total: by_year.values().sum(),
                commits_28d: u32::try_from(count("last28", &r.id)).unwrap_or(u32::MAX),
                commits_today: u32::try_from(count("today", &r.id)).unwrap_or(u32::MAX),
                commits_365d: count("last365", &r.id),
                commits_by_year: by_year,
                archived_at: r.archived_at.map(|t| t.to_offset(offset).date()),
            }
        })
        .collect();
    repos.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.id.cmp(&b.id))
    });
    Ok(Snapshot {
        login: profile.login,
        date,
        account_created: created,
        public_repos: profile.repos.len() as u32,
        repos,
        calendar: cal
            .into_iter()
            .map(|(date, count)| CalendarDay { date, count })
            .collect(),
        years: (created.year()..=this_year).collect(),
    })
}

#[cfg(test)]
mod tests;
