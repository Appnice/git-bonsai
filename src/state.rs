//! The tree's memory: `bonsai/state.json` in the profile repo, the only source of the tree's shape.
//! The image is always a function of this state.

use crate::snapshot::{ymd, AccountKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use time::Date;

/// Current state schema. Bump it only together with a `migrate_vN_to_vN+1` and its test.
pub const SCHEMA: u32 = 1;

/// Which side of the trunk a branch grows to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
}

/// The trunk: height and girth in cells, `bends` = `[row, x offset]` control points from the
/// soil up (row 0 is the first row above the soil), interpolated row by row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trunk {
    pub height: u32,
    pub girth: u32,
    pub bends: Vec<[i32; 2]>,
}

/// One branch = one repo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    pub repo_id: String,
    pub name: String,
    /// Language name, `None` when GitHub has none.
    pub lang: Option<String>,
    /// Linguist colour at the time it was last seen (`#dea584`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Trunk row the branch leaves from (0 = just above the soil).
    pub anchor: u32,
    pub side: Side,
    /// Current length in cells (grows by at most `max_growth_per_day` a day).
    pub length: u32,
    /// Pad size 0..=7 (0 = no leaves, as on a pruned branch).
    pub pad: u32,
    pub commits_total: u64,
    /// Day the repo was archived or deleted; a pruned branch keeps a 3-cell stub and a scar forever.
    #[serde(with = "opt_ymd")]
    pub pruned: Option<Date>,
}

/// One line of growth history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// A day (`2026-10-31`) or, for entries older than 400 days, a month (`2025-06`).
    pub date: String,
    pub cells_added: u32,
}

/// The phase on the last grown day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PhaseKind {
    Sapling,
    #[default]
    Growing,
    Bloom,
    Dormant,
}

/// Facts of the last grown day the image needs (phase, colours, pot, moss). Recomputed every day,
/// never accumulated: only the shape (`trunk`, `branches`) is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Vitals {
    pub phase: PhaseKind,
    pub streak_days: u32,
    pub idle_days: u32,
    /// The account's commits in all its public repos (header).
    pub commits_total: u64,
    pub stars_total: u64,
    pub public_repos: u32,
    /// Commits in the last 28 days per branch `repo_id` (pad shade distribution).
    pub activity_28d: BTreeMap<String, u32>,
    /// Cells each branch gained on the last grown day, by `repo_id` (they pulse in the image).
    pub grew: BTreeMap<String, u32>,
}

/// The whole persisted tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeState {
    pub schema: u32,
    pub login: String,
    /// User or organization; organizations get a celadon pot and an ORG tag.
    #[serde(default)]
    pub kind: AccountKind,
    /// 16 hex chars: first 8 bytes of `sha256(login + planted)`.
    pub seed: String,
    #[serde(with = "ymd")]
    pub planted: Date,
    #[serde(with = "opt_ymd")]
    pub last_grown: Option<Date>,
    pub trunk: Trunk,
    /// Sorted by `anchor`, then `repo_id` (render order).
    pub branches: Vec<Branch>,
    /// Commits per repo per finished year: `{"2025": {"R_kgDO…": 412}}`.
    #[serde(default)]
    pub year_cache: BTreeMap<String, BTreeMap<String, u64>>,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
    #[serde(default)]
    pub vitals: Vitals,
}

/// Why a `state.json` cannot be used (exit code 4 in the CLI; the file is never overwritten).
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("state.json is corrupt: {0}")]
    Corrupt(String),
    #[error("state.json has schema {found}, newer than this version understands ({SCHEMA}); upgrade git-bonsai")]
    Newer { found: u32 },
}

/// Parses a `state.json`, migrating older schemas up to [`SCHEMA`].
pub fn load(json: &str) -> Result<TreeState, StateError> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| StateError::Corrupt(e.to_string()))?;
    let found = v
        .get("schema")
        .and_then(|s| s.as_u64())
        .ok_or_else(|| StateError::Corrupt("missing or invalid `schema`".into()))?;
    let found = u32::try_from(found).map_err(|_| StateError::Newer { found: u32::MAX })?;
    if found > SCHEMA {
        return Err(StateError::Newer { found });
    }
    if found == 0 {
        return Err(StateError::Corrupt("schema 0 does not exist".into()));
    }
    // Schema 1 is the first; migrate_v1_to_v2 and friends go here.
    serde_json::from_value(v).map_err(|e| StateError::Corrupt(e.to_string()))
}

/// Serialises a state as pretty JSON with a trailing newline (stable bytes for git diffs).
pub fn to_json(state: &TreeState) -> String {
    let mut s = serde_json::to_string_pretty(state).unwrap_or_default();
    s.push('\n');
    s
}

/// `Option<Date>` as `"YYYY-MM-DD"` or `null`.
pub mod opt_ymd {
    use serde::{Deserialize, Deserializer, Serializer};
    use time::Date;

    pub fn serialize<S: Serializer>(d: &Option<Date>, s: S) -> Result<S::Ok, S::Error> {
        match d {
            Some(d) => crate::snapshot::ymd::serialize(d, s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Date>, D::Error> {
        #[derive(Deserialize)]
        struct W(#[serde(with = "crate::snapshot::ymd")] Date);
        Ok(Option::<W>::deserialize(d)?.map(|w| w.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc_example_parses() {
        let json = r#"{
          "schema": 1, "login": "mira-codes", "seed": "9f2c41a07be3d5e1",
          "planted": "2016-03-14", "last_grown": "2026-10-31",
          "trunk": { "height": 24, "girth": 4, "bends": [[0,0],[6,-1],[11,1],[16,0]] },
          "branches": [
            { "repo_id": "R_1", "name": "ferrite", "lang": "Rust", "anchor": 12, "side": "right",
              "length": 9, "pad": 5, "commits_total": 1204, "pruned": null },
            { "repo_id": "R_2", "name": "old-android-lib", "lang": "Kotlin", "anchor": 5, "side": "left",
              "length": 3, "pad": 0, "commits_total": 310, "pruned": "2024-05-02" }
          ],
          "year_cache": { "2025": { "R_1": 412 } },
          "history": [ { "date": "2026-10-31", "cells_added": 2 } ]
        }"#;
        let s: TreeState = serde_json::from_str(json).unwrap();
        assert_eq!(
            s.branches[1].pruned.map(|d| d.to_string()),
            Some("2024-05-02".into())
        );
        let back: TreeState = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
        assert_eq!(load(&to_json(&s)).unwrap(), s);
    }

    #[test]
    fn kind_defaults_to_user_and_round_trips() {
        use crate::snapshot::AccountKind;
        let json = r#"{"schema": 1, "login": "a", "seed": "0123456789abcdef",
          "planted": "2020-01-01", "last_grown": null,
          "trunk": {"height": 8, "girth": 2, "bends": [[0,0]]}, "branches": []}"#;
        let mut s = load(json).unwrap();
        assert_eq!(s.kind, AccountKind::User);
        s.kind = AccountKind::Org;
        let out = to_json(&s);
        assert!(out.contains(r#""kind": "org""#));
        assert_eq!(load(&out).unwrap().kind, AccountKind::Org);
        let snap: crate::snapshot::Snapshot = serde_json::from_str(
            r#"{"login":"a","date":"2026-01-02","account_created":"2020-01-01",
                "public_repos":0,"repos":[],"calendar":[],"kind":"org"}"#,
        )
        .unwrap();
        assert_eq!(snap.kind, AccountKind::Org);
        let back: crate::snapshot::Snapshot =
            serde_json::from_str(&serde_json::to_string(&snap).unwrap()).unwrap();
        assert_eq!(back, snap);
    }

    #[test]
    fn load_rejects_corrupt_and_newer() {
        assert!(matches!(load("{"), Err(StateError::Corrupt(_))));
        assert!(matches!(load("{}"), Err(StateError::Corrupt(_))));
        assert!(matches!(
            load(r#"{"schema": 2, "whatever": true}"#),
            Err(StateError::Newer { found: 2 })
        ));
        assert!(matches!(
            load(r#"{"schema": 1, "login": 5}"#),
            Err(StateError::Corrupt(_))
        ));
    }
}
