//! `git-bonsai` CLI. Exit codes: 0 ok, 2 configuration, 3 GitHub API after retries,
//! 4 corrupt or newer state (left untouched).

mod cli;

use anyhow::{anyhow, Context};
use clap::Parser;
use cli::{Cli, Cmd, Common, PhaseArg, ThemeArg};
use git_bonsai::github::{self, ApiError, Client, Token, YearCache};
use git_bonsai::render::{ascii, svg};
use git_bonsai::snapshot::Snapshot;
use git_bonsai::state::{self, PhaseKind, StateError, TreeState};
use git_bonsai::{layout, Config, GrowError, Theme};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;
use time::{Date, UtcOffset};

/// An error with its exit code.
struct Fail {
    code: u8,
    err: anyhow::Error,
}

trait Code<T> {
    fn code(self, code: u8) -> Result<T, Fail>;
}

impl<T, E: Into<anyhow::Error>> Code<T> for Result<T, E> {
    fn code(self, code: u8) -> Result<T, Fail> {
        self.map_err(|e| Fail {
            code,
            err: e.into(),
        })
    }
}

const CONFIG: u8 = 2;
const API: u8 = 3;
const STATE: u8 = 4;
/// I/O failures (writing files) count as configuration problems: wrong paths or permissions.
const IO: u8 = 2;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(f) => {
            eprintln!("git-bonsai: {:#}", f.err);
            ExitCode::from(f.code)
        }
    }
}

fn run(cli: Cli) -> Result<(), Fail> {
    let c = &cli.common;
    match &cli.cmd {
        Cmd::Grow {
            login,
            state,
            out,
            themes,
            from_snapshot,
        } => {
            let cfg = config(c, state)?;
            let old = read_state(state)?;
            let t0 = Instant::now();
            let cache = old
                .as_ref()
                .map(|s| s.year_cache.clone())
                .unwrap_or_default();
            let (snap, queries) = snapshot(c, login, from_snapshot.as_deref(), &cache, &cfg)?;
            let next = match &old {
                Some(s) => git_bonsai::grow(s, &snap, &cfg),
                None => git_bonsai::plant(&snap, &cfg),
            }
            .map_err(grow_fail)?;
            log(
                c,
                &format!(
                    "{} {} → last_grown {}, {} queries, {:.1} s",
                    if old.is_some() { "grew" } else { "planted" },
                    next.login,
                    next.last_grown.map(|d| d.to_string()).unwrap_or_default(),
                    queries,
                    t0.elapsed().as_secs_f64()
                ),
            );
            write_all(c, &next, state, out, themes)
        }
        Cmd::Render {
            state,
            theme,
            out,
            phase,
        } => {
            let mut s = read_state(state)?
                .ok_or_else(|| anyhow!("no state at {}", state.display()))
                .code(CONFIG)?;
            if let Some(p) = phase {
                s.vitals.phase = match p {
                    PhaseArg::Sapling => PhaseKind::Sapling,
                    PhaseArg::Growing => PhaseKind::Growing,
                    PhaseArg::Bloom => PhaseKind::Bloom,
                    PhaseArg::Dormant => PhaseKind::Dormant,
                };
            }
            let scene = layout::build_scene(&s);
            let svg = svg::render(
                &scene,
                &svg::SvgOptions {
                    theme: theme_of(*theme),
                    animate: !c.no_animate,
                },
            );
            write_atomic(out, svg.as_bytes())
        }
        Cmd::Snapshot { login, out } => {
            let t0 = Instant::now();
            let cfg = config(c, Path::new("bonsai/state.json"))?;
            let (snap, queries) = snapshot(c, login, None, &YearCache::new(), &cfg)?;
            log(
                c,
                &format!(
                    "snapshot {} {}: {} repos, {} calendar days, {} queries, {:.1} s",
                    snap.login,
                    snap.date,
                    snap.repos.len(),
                    snap.calendar.len(),
                    queries,
                    t0.elapsed().as_secs_f64()
                ),
            );
            let mut json = serde_json::to_string_pretty(&snap).code(IO)?;
            json.push('\n');
            write_atomic(out, json.as_bytes())
        }
        Cmd::Plant {
            login,
            from_snapshot,
            state,
            out,
            themes,
            force,
        } => {
            if state.exists() && !force {
                return Err(anyhow!(
                    "{} exists; use `grow` to grow it or --force to replace it",
                    state.display()
                ))
                .code(CONFIG);
            }
            let cfg = config(c, state)?;
            let login = match (login, from_snapshot) {
                (Some(l), _) => l.clone(),
                (None, Some(_)) => String::new(),
                (None, None) => {
                    return Err(anyhow!("--login or --from-snapshot is required")).code(CONFIG)
                }
            };
            let t0 = Instant::now();
            let (snap, queries) =
                snapshot(c, &login, from_snapshot.as_deref(), &YearCache::new(), &cfg)?;
            let next = git_bonsai::plant(&snap, &cfg).map_err(grow_fail)?;
            log(
                c,
                &format!(
                    "planted {} ({} → {}), {} branches, {} queries, {:.1} s",
                    next.login,
                    next.planted,
                    snap.date,
                    next.branches.len(),
                    queries,
                    t0.elapsed().as_secs_f64()
                ),
            );
            write_all(c, &next, state, out, themes)
        }
        Cmd::Debug { state } => {
            let s = read_state(state)?
                .ok_or_else(|| anyhow!("no state at {}", state.display()))
                .code(CONFIG)?;
            print!("{}", ascii::print(&layout::build_scene(&s).grid));
            println!("{}", summary(&s));
            Ok(())
        }
    }
}

fn log(c: &Common, msg: &str) {
    if c.verbose > 0 {
        eprintln!("{msg}");
    }
}

/// One line per branch, for `debug` while the layout is young.
fn summary(s: &TreeState) -> String {
    let v = &s.vitals;
    let mut out = format!(
        "{} planted {} last_grown {} · trunk h{} g{} · {:?} streak {} idle {} · {} commits {} stars {} repos\n",
        s.login,
        s.planted,
        s.last_grown.map(|d| d.to_string()).unwrap_or_else(|| "-".into()),
        s.trunk.height,
        s.trunk.girth,
        v.phase,
        v.streak_days,
        v.idle_days,
        v.commits_total,
        v.stars_total,
        v.public_repos
    );
    for b in &s.branches {
        out.push_str(&format!(
            "  {:>2} {:<5} len {:>2} pad {} {:<24} {:>6} commits {}{}\n",
            b.anchor,
            format!("{:?}", b.side).to_lowercase(),
            b.length,
            b.pad,
            b.name,
            b.commits_total,
            b.lang.as_deref().unwrap_or("-"),
            b.pruned.map(|d| format!(" pruned {d}")).unwrap_or_default()
        ));
    }
    out
}

fn theme_of(t: ThemeArg) -> Theme {
    match t {
        ThemeArg::Dark => Theme::Dark,
        ThemeArg::Light => Theme::Light,
    }
}

fn grow_fail(e: GrowError) -> Fail {
    let code = match e {
        GrowError::Schema(_) => STATE,
        _ => CONFIG,
    };
    Fail {
        code,
        err: e.into(),
    }
}

fn api_fail(e: ApiError) -> Fail {
    let code = match e {
        ApiError::Unauthorized | ApiError::UserNotFound(_) => CONFIG,
        _ => API,
    };
    Fail {
        code,
        err: e.into(),
    }
}

/// Defaults ← bonsai.toml ← flags.
fn config(c: &Common, state: &Path) -> Result<Config, Fail> {
    let (path, explicit) = match &c.config {
        Some(p) => (p.clone(), true),
        None => (
            state
                .parent()
                .map(|p| p.join("bonsai.toml"))
                .unwrap_or_else(|| PathBuf::from("bonsai.toml")),
            false,
        ),
    };
    let mut cfg = if path.exists() {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))
            .code(CONFIG)?;
        toml::from_str::<Config>(&text)
            .with_context(|| format!("parsing {}", path.display()))
            .code(CONFIG)?
    } else if explicit {
        return Err(anyhow!("config {} not found", path.display())).code(CONFIG);
    } else {
        Config::default()
    };
    if let Some(n) = c.max_branches {
        cfg.max_branches = n;
    }
    if let Some(ex) = &c.exclude {
        cfg.exclude = ex
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    cfg.validate().code(CONFIG)?;
    Ok(cfg)
}

/// `None` when there is no state file yet; exit 4 when it is corrupt or too new.
fn read_state(path: &Path) -> Result<Option<TreeState>, Fail> {
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))
        .code(STATE)?;
    match state::load(&text) {
        Ok(s) => Ok(Some(s)),
        Err(e @ (StateError::Corrupt(_) | StateError::Newer { .. })) => Err(Fail {
            code: STATE,
            err: anyhow::Error::new(e).context(format!("{} left untouched", path.display())),
        }),
    }
}

/// The snapshot from a file, or fetched for `--date` (default: yesterday in `--tz`).
fn snapshot(
    c: &Common,
    login: &str,
    from: Option<&Path>,
    cache: &YearCache,
    cfg: &Config,
) -> Result<(Snapshot, u32), Fail> {
    if let Some(p) = from {
        let text = std::fs::read_to_string(p)
            .with_context(|| format!("reading {}", p.display()))
            .code(CONFIG)?;
        let snap: Snapshot = serde_json::from_str(&text)
            .with_context(|| format!("parsing snapshot {}", p.display()))
            .code(CONFIG)?;
        if !login.is_empty() && !login.eq_ignore_ascii_case(&snap.login) {
            return Err(anyhow!(
                "snapshot {} is for {}, not {login}",
                p.display(),
                snap.login
            ))
            .code(CONFIG);
        }
        return Ok((snap, 0));
    }
    let (date, offset) = day(c)?;
    let token = token(c)?;
    let verbose = c.verbose > 0;
    let client = Client::new(token).map_err(api_fail)?.log_with(move |m| {
        if verbose {
            eprintln!("  {m}");
        }
    });
    let opts = github::FetchOptions {
        offset,
        force_org: c.org,
        config: cfg.clone(),
    };
    let snap = github::fetch(&client, login, date, &opts, cache).map_err(api_fail)?;
    Ok((snap, client.queries()))
}

/// The day to grow and the zone's UTC offset on it.
fn day(c: &Common) -> Result<(Date, UtcOffset), Fail> {
    let tz = match &c.tz {
        Some(name) => jiff::tz::TimeZone::get(name)
            .with_context(|| format!("unknown time zone {name:?}"))
            .code(CONFIG)?,
        None => jiff::tz::TimeZone::UTC,
    };
    let civil = match &c.date {
        Some(d) => d
            .parse::<jiff::civil::Date>()
            .with_context(|| format!("--date {d:?} is not YYYY-MM-DD"))
            .code(CONFIG)?,
        None => jiff::Timestamp::now()
            .to_zoned(tz.clone())
            .date()
            .yesterday()
            .code(CONFIG)?,
    };
    let noon = civil.at(12, 0, 0, 0).to_zoned(tz.clone()).code(CONFIG)?;
    let offset = UtcOffset::from_whole_seconds(noon.offset().seconds()).code(CONFIG)?;
    let month = time::Month::try_from(civil.month() as u8).code(CONFIG)?;
    let date =
        Date::from_calendar_date(civil.year() as i32, month, civil.day() as u8).code(CONFIG)?;
    Ok((date, offset))
}

/// `--token`, `GITHUB_TOKEN`, `GH_TOKEN`, then `gh auth token`.
fn token(c: &Common) -> Result<Token, Fail> {
    let from_env = |k: &str| std::env::var(k).ok().and_then(Token::new);
    c.token
        .clone()
        .and_then(Token::new)
        .or_else(|| from_env("GITHUB_TOKEN"))
        .or_else(|| from_env("GH_TOKEN"))
        .or_else(|| {
            std::process::Command::new("gh")
                .args(["auth", "token"])
                .stderr(std::process::Stdio::null())
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(Token::new)
        })
        .ok_or_else(|| anyhow!("no GitHub token: pass --token, set GITHUB_TOKEN or GH_TOKEN, or log in with `gh auth login`"))
        .code(CONFIG)
}

/// Writes `state.json` into `state` and the images into `out`.
fn write_all(
    c: &Common,
    s: &TreeState,
    state_path: &Path,
    out: &Path,
    themes: &[ThemeArg],
) -> Result<(), Fail> {
    write_atomic(state_path, state::to_json(s).as_bytes())?;
    let scene = layout::build_scene(s);
    let mut seen = Vec::new();
    for t in themes {
        if seen.contains(t) {
            continue;
        }
        seen.push(*t);
        let theme = theme_of(*t);
        let svg = svg::render(
            &scene,
            &svg::SvgOptions {
                theme,
                animate: !c.no_animate,
            },
        );
        write_atomic(
            &out.join(format!("bonsai-{}.svg", theme.name())),
            svg.as_bytes(),
        )?;
    }
    if c.gif {
        write_gif(&scene, out)?;
    }
    log(
        c,
        &format!(
            "wrote {} and {} image(s) in {}",
            state_path.display(),
            seen.len(),
            out.display()
        ),
    );
    Ok(())
}

#[cfg(feature = "gif")]
fn write_gif(scene: &git_bonsai::Scene, out: &Path) -> Result<(), Fail> {
    let bytes = git_bonsai::render::gif::render(scene, Theme::Dark).code(IO)?;
    write_atomic(&out.join("bonsai.gif"), &bytes)
}

#[cfg(not(feature = "gif"))]
fn write_gif(_scene: &git_bonsai::Scene, _out: &Path) -> Result<(), Fail> {
    Err(anyhow!(
        "this build has no GIF support (crate feature `gif`)"
    ))
    .code(CONFIG)
}

/// Write to a temp file next to `path`, then rename (a crash never leaves half a file).
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Fail> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating {}", dir.display()))
            .code(IO)?;
    }
    let tmp = path.with_extension("tmp~");
    std::fs::write(&tmp, bytes)
        .with_context(|| format!("writing {}", tmp.display()))
        .code(IO)?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("writing {}", path.display()))
        .code(IO)
}
