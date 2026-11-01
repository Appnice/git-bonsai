use super::*;
use std::sync::{Arc, Mutex};
use time::macros::date;

const SECRET: &str = "ghp_SuperSecretTokenValue1234567890";

/// Replies with a scripted list, one per request; the last one repeats.
struct Script {
    replies: Mutex<Vec<Result<(u16, String), String>>>,
}

impl Transport for Script {
    fn post(&self, _body: &str) -> Result<(u16, String), String> {
        let mut r = self.replies.lock().unwrap();
        if r.len() > 1 {
            r.remove(0)
        } else {
            r[0].clone()
        }
    }
}

fn scripted(replies: Vec<Result<(u16, String), String>>) -> (Client, Arc<Mutex<Vec<Duration>>>) {
    let slept = Arc::new(Mutex::new(Vec::new()));
    let s2 = slept.clone();
    let c = Client::with_transport(Box::new(Script {
        replies: Mutex::new(replies),
    }))
    .sleep_with(move |d| s2.lock().unwrap().push(d));
    (c, slept)
}

fn ok(data: &str) -> Result<(u16, String), String> {
    Ok((200, format!(r#"{{"data": {data}}}"#)))
}

#[test]
fn retries_5xx_with_backoff_then_succeeds() {
    let (c, slept) = scripted(vec![
        Ok((502, "bad gateway".into())),
        Err("connection reset".into()),
        Ok((
            200,
            r#"{"errors":[{"type":"RATE_LIMITED","message":"slow down"}]}"#.into(),
        )),
        ok(r#"{"x": 1}"#),
    ]);
    let d = c.query("t", "query{}", serde_json::json!({})).unwrap();
    assert_eq!(d["x"], 1);
    assert_eq!(c.queries(), 4);
    assert_eq!(*slept.lock().unwrap(), BACKOFF.to_vec());
}

#[test]
fn gives_up_after_three_retries() {
    let (c, slept) = scripted(vec![Ok((503, String::new()))]);
    let e = c.query("t", "query{}", serde_json::json!({})).unwrap_err();
    assert!(matches!(e, ApiError::Exhausted { attempts: 4, .. }));
    assert_eq!(c.queries(), 4);
    assert_eq!(slept.lock().unwrap().len(), 3);
}

#[test]
fn client_errors_fail_fast() {
    for (status, body) in [
        (401, r#"{"message":"Bad credentials"}"#),
        (404, r#"{"message":"Not Found"}"#),
        (403, r#"{"message":"Resource not accessible"}"#),
    ] {
        let (c, slept) = scripted(vec![Ok((status, body.into()))]);
        assert!(c.query("t", "q", serde_json::json!({})).is_err());
        assert_eq!(c.queries(), 1, "{status}");
        assert!(slept.lock().unwrap().is_empty());
    }
    let (c, _) = scripted(vec![
        Ok((
            403,
            r#"{"message":"You have exceeded a secondary rate limit"}"#.into(),
        )),
        ok("{}"),
    ]);
    assert!(c.query("t", "q", serde_json::json!({})).is_ok());
    assert_eq!(c.queries(), 2);
    let (c, _) = scripted(vec![Ok((
        200,
        r#"{"data":{"user":null},"errors":[{"type":"NOT_FOUND","message":"no user"}]}"#.into(),
    ))]);
    let e = queries::profile(&c, "nobody").unwrap_err();
    assert_eq!(e, ApiError::UserNotFound("nobody".into()));
    assert_eq!(c.queries(), 1);
}

#[test]
fn token_never_appears_in_errors_or_logs() {
    let token = Token::new(SECRET).unwrap();
    assert!(!format!("{token:?} {token}").contains(SECRET));
    let logs = Arc::new(Mutex::new(Vec::<String>::new()));
    let l2 = logs.clone();
    let echo = format!("token {SECRET} rejected");
    let replies = vec![
        Err(format!("tls error for {SECRET}")),
        Ok((500, echo.clone())),
        Ok((502, echo.clone())),
        Ok((503, echo.clone())),
    ];
    let (c, _) = scripted(replies);
    let c = c
        .scrub(&token)
        .log_with(move |m| l2.lock().unwrap().push(m.to_string()));
    let e = c.query("t", "q", serde_json::json!({})).unwrap_err();
    let mut seen = format!("{e} {e:?}");
    for (status, body) in [
        (422, format!(r#"{{"message":"{echo}"}}"#)),
        (
            200,
            format!(r#"{{"errors":[{{"type":"{SECRET}","message":"{echo}"}}]}}"#),
        ),
        (200, format!("not json {SECRET}")),
    ] {
        let (c, _) = scripted(vec![Ok((status, body))]);
        let c = c.scrub(&token);
        let e = c.query("t", "q", serde_json::json!({})).unwrap_err();
        seen.push_str(&format!("{e} {e:?}"));
    }
    seen.push_str(&logs.lock().unwrap().join("\n"));
    assert!(!logs.lock().unwrap().is_empty());
    assert!(!seen.contains(SECRET), "{seen}");
    assert!(seen.contains("***"));
}

/// Answers query 1 (two pages) and query 2 by the aliases in the request.
struct FakeGitHub {
    requests: Mutex<Vec<String>>,
}

impl Transport for FakeGitHub {
    fn post(&self, body: &str) -> Result<(u16, String), String> {
        self.requests.lock().unwrap().push(body.to_string());
        let v: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
        let q = v["query"].as_str().unwrap_or("");
        if q.contains("repositories(") {
            let page2 = v["variables"]["after"].as_str() == Some("c1");
            let data = if page2 {
                r##"{"user":{"login":"Octo","createdAt":"2023-06-10T12:00:00Z","repositories":{
                  "pageInfo":{"hasNextPage":false,"endCursor":null},
                  "nodes":[{"id":"R_old","name":"old","isFork":false,"isArchived":true,
                    "archivedAt":"2025-02-01T08:00:00Z","stargazerCount":3,"pushedAt":null,"createdAt":null,
                    "primaryLanguage":null}]}}}"##
            } else {
                r##"{"user":{"login":"Octo","createdAt":"2023-06-10T12:00:00Z","repositories":{
                  "pageInfo":{"hasNextPage":true,"endCursor":"c1"},
                  "nodes":[{"id":"R_a","name":"alpha","isFork":false,"isArchived":false,
                    "archivedAt":null,"stargazerCount":120,"pushedAt":null,"createdAt":null,
                    "primaryLanguage":{"name":"Rust","color":"#dea584"}},
                    {"id":"R_f","name":"fork","isFork":true,"isArchived":false,
                    "archivedAt":null,"stargazerCount":0,"pushedAt":null,"createdAt":null,
                    "primaryLanguage":null}]}}}"##
            };
            return Ok((200, format!(r#"{{"data":{data}}}"#)));
        }
        let mut user = serde_json::Map::new();
        for (alias, a, old, day) in [
            ("y2023", 10, 5, "2023-12-30"),
            ("y2024", 20, 7, "2024-12-30"),
            ("y2025", 30, 1, "2025-01-15"),
            ("current", 40, 0, "2026-10-30"),
            ("last365", 50, 0, "2026-10-30"),
            ("last28", 6, 0, ""),
            ("today", 2, 0, ""),
        ] {
            if q.contains(&format!("{alias}: contributionsCollection")) {
                let mut node = serde_json::json!({
                    "commitContributionsByRepository": [
                        {"repository": {"id": "R_a"}, "contributions": {"totalCount": a}},
                        {"repository": {"id": "R_old"}, "contributions": {"totalCount": old}},
                        {"repository": {"id": "R_someone_elses"}, "contributions": {"totalCount": 99}}
                    ]
                });
                if !day.is_empty() {
                    node["contributionCalendar"] = serde_json::json!({"weeks": [{"contributionDays": [
                        {"date": "2023-06-01", "contributionCount": 9},
                        {"date": day, "contributionCount": 4}
                    ]}]});
                }
                user.insert(alias.into(), node);
            }
        }
        Ok((200, serde_json::json!({"data": {"user": user}}).to_string()))
    }
}

#[test]
fn fetch_builds_a_snapshot_and_uses_the_year_cache() {
    let fake = Arc::new(FakeGitHub {
        requests: Mutex::new(Vec::new()),
    });
    struct Shared(Arc<FakeGitHub>);
    impl Transport for Shared {
        fn post(&self, body: &str) -> Result<(u16, String), String> {
            self.0.post(body)
        }
    }
    let c = Client::with_transport(Box::new(Shared(fake.clone())));
    let date = date!(2026 - 10 - 31);
    let s = fetch(&c, "octo", date, UtcOffset::UTC, &YearCache::new()).unwrap();
    // 2 profile pages + 3 finished years + 1 daily
    assert_eq!(c.queries(), 6);
    assert_eq!(s.login, "Octo");
    assert_eq!(s.account_created, date!(2023 - 06 - 10));
    assert_eq!(s.public_repos, 3);
    assert_eq!(s.years, vec![2023, 2024, 2025, 2026]);
    let names: Vec<&str> = s.repos.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["alpha", "fork", "old"]);
    let a = &s.repos[0];
    assert_eq!(a.commits_total, 10 + 20 + 30 + 40);
    assert_eq!((a.commits_365d, a.commits_28d, a.commits_today), (50, 6, 2));
    assert_eq!(a.color.as_deref(), Some("#dea584"));
    let old = &s.repos[2];
    assert_eq!(old.archived_at, Some(date!(2025 - 02 - 01)));
    assert_eq!(old.commits_total, 13);
    // calendar: before account creation dropped, sorted, deduped
    let days: Vec<String> = s.calendar.iter().map(|d| d.date.to_string()).collect();
    assert_eq!(
        days,
        ["2023-12-30", "2024-12-30", "2025-01-15", "2026-10-30"]
    );
    // the daily query asks for exactly the four windows
    let reqs = fake.requests.lock().unwrap();
    let last = reqs.last().unwrap();
    for alias in ["current:", "last365:", "last28:", "today:"] {
        assert!(last.contains(alias), "{alias}");
    }
    assert!(last.contains("2025-11-01T00:00:00Z"));
    drop(reqs);

    // daily run: finished years come from the cache
    let mut cache = YearCache::new();
    for y in ["2023", "2024", "2025"] {
        cache.insert(y.into(), [("R_a".to_string(), 1u64)].into());
    }
    let c = Client::with_transport(Box::new(Shared(fake.clone())));
    let s = fetch(&c, "octo", date, UtcOffset::UTC, &cache).unwrap();
    assert_eq!(c.queries(), 3);
    assert_eq!(s.repos[0].commits_total, 3 + 40);
    assert_eq!(s.repos[2].commits_total, 0);
}

#[test]
fn windows_respect_the_offset_and_one_year_limit() {
    let off = UtcOffset::from_hms(2, 0, 0).unwrap();
    let w = year_window(2025, off);
    assert_eq!(
        w.from
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap(),
        "2025-01-01T00:00:00+02:00"
    );
    assert!(w.to - w.from < time::Duration::days(366));
    for w in daily_windows(date!(2026 - 03 - 01), off) {
        assert!(w.to - w.from < time::Duration::days(366), "{}", w.alias);
    }
    let q = queries::contributions_query(&daily_windows(date!(2026 - 03 - 01), off));
    assert!(q.contains("fragment WithCalendar") && q.contains("fragment ByRepo"));
}
