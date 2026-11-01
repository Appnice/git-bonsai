//! The binary offline: subcommands on fixture snapshots, exit codes, token hygiene.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_git-bonsai");
const SECRET: &str = "ghp_CliSecretTokenValue0987654321";

fn dir(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn fixture() -> String {
    format!(
        "{}/tests/fixtures/torvalds-2026-10-31.json",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .env_remove("GITHUB_TOKEN")
        .env_remove("GH_TOKEN")
        .output()
        .unwrap()
}

fn code(o: &Output) -> i32 {
    o.status.code().unwrap_or(-1)
}

#[test]
fn plant_grow_render_debug_offline() {
    let d = dir("cli-flow");
    let state = d.join("bonsai/state.json");
    let out = d.join("bonsai");
    let (s, o) = (state.to_str().unwrap(), out.to_str().unwrap());
    let r = run(&[
        "plant",
        "--from-snapshot",
        &fixture(),
        "--state",
        s,
        "--out",
        o,
    ]);
    assert_eq!(code(&r), 0, "{}", String::from_utf8_lossy(&r.stderr));
    for f in ["state.json", "bonsai-dark.svg", "bonsai-light.svg"] {
        assert!(out.join(f).exists(), "{f}");
    }
    // planting again refuses to overwrite
    let r = run(&[
        "plant",
        "--from-snapshot",
        &fixture(),
        "--state",
        s,
        "--out",
        o,
    ]);
    assert_eq!(code(&r), 2);
    // growing on the same snapshot is a no-op
    let before = std::fs::read(&state).unwrap();
    let r = run(&[
        "grow",
        "--login",
        "torvalds",
        "--from-snapshot",
        &fixture(),
        "--state",
        s,
        "--out",
        o,
        "--themes",
        "light",
        "--no-animate",
    ]);
    assert_eq!(code(&r), 0, "{}", String::from_utf8_lossy(&r.stderr));
    assert_eq!(std::fs::read(&state).unwrap(), before);
    let svg = d.join("x.svg");
    let r = run(&[
        "render",
        "--state",
        s,
        "--theme",
        "light",
        "--out",
        svg.to_str().unwrap(),
        "--phase",
        "bloom",
    ]);
    assert_eq!(code(&r), 0);
    assert!(std::fs::read_to_string(&svg).unwrap().starts_with("<svg"));
    let r = run(&["debug", "--state", s]);
    assert_eq!(code(&r), 0);
    assert!(String::from_utf8_lossy(&r.stdout).contains("linux"));
    // wrong login for the snapshot
    let r = run(&[
        "grow",
        "--login",
        "someoneelse",
        "--from-snapshot",
        &fixture(),
        "--state",
        s,
        "--out",
        o,
    ]);
    assert_eq!(code(&r), 2);
}

#[test]
fn corrupt_or_newer_state_exits_4_untouched() {
    let d = dir("cli-state");
    for (name, body) in [("corrupt", "{ not json"), ("newer", r#"{"schema": 99}"#)] {
        let state = d.join(format!("{name}.json"));
        std::fs::write(&state, body).unwrap();
        let r = run(&[
            "grow",
            "--login",
            "torvalds",
            "--from-snapshot",
            &fixture(),
            "--state",
            state.to_str().unwrap(),
            "--out",
            d.to_str().unwrap(),
        ]);
        assert_eq!(code(&r), 4, "{name}");
        assert_eq!(std::fs::read_to_string(&state).unwrap(), body);
        let r = run(&["debug", "--state", state.to_str().unwrap()]);
        assert_eq!(code(&r), 4);
    }
}

#[test]
fn config_errors_exit_2() {
    let d = dir("cli-config");
    let state = d.join("state.json");
    let s = state.to_str().unwrap();
    std::fs::write(d.join("bonsai.toml"), "max_branches = \"seven\"\n").unwrap();
    let r = run(&[
        "plant",
        "--from-snapshot",
        &fixture(),
        "--state",
        s,
        "--out",
        d.to_str().unwrap(),
    ]);
    assert_eq!(code(&r), 2);
    std::fs::write(
        d.join("bonsai.toml"),
        "max_branches = 3\npin = [\"uemacs\"]\nexclude = [\"Guitar*\"]\n",
    )
    .unwrap();
    let r = run(&[
        "plant",
        "--from-snapshot",
        &fixture(),
        "--state",
        s,
        "--out",
        d.to_str().unwrap(),
    ]);
    assert_eq!(code(&r), 0, "{}", String::from_utf8_lossy(&r.stderr));
    let st = std::fs::read_to_string(&state).unwrap();
    assert!(st.contains("\"uemacs\"") && !st.contains("GuitarPedal"));
    let r = run(&[
        "plant",
        "--from-snapshot",
        &fixture(),
        "--state",
        s,
        "--force",
        "--max-branches",
        "0",
    ]);
    assert_eq!(code(&r), 2);
    let r = run(&[
        "render",
        "--state",
        d.join("missing.json").to_str().unwrap(),
        "--out",
        "x.svg",
    ]);
    assert_eq!(code(&r), 2);
}

#[test]
fn token_never_printed() {
    let d = dir("cli-token");
    let r = run(&[
        "snapshot",
        "--login",
        "torvalds",
        "--out",
        d.join("s.json").to_str().unwrap(),
        "--token",
        SECRET,
        "--tz",
        "Not/AZone",
        "-v",
    ]);
    assert_eq!(code(&r), 2);
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    assert!(!all.contains(SECRET), "{all}");
    assert!(all.contains("time zone"));
}
