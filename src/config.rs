//! Growth settings: defaults, overridden by `bonsai/bonsai.toml`, overridden by CLI flags.

use serde::Deserialize;
use std::collections::BTreeMap;

use crate::state::PadMix;

/// Configuration errors (exit code 2 in the CLI).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("max_branches must be 1..=20, got {0}")]
    MaxBranches(usize),
    #[error("max_growth_per_day must be 1..=8, got {0}")]
    MaxGrowth(u32),
    #[error("{0} must be at least 1")]
    ZeroDays(&'static str),
    #[error("colour for {lang} must be #rrggbb, got {value:?}")]
    Color { lang: String, value: String },
    #[error("max_pad_langs must be 1..={MAX_PAD_LANGS}, got {0}")]
    MaxPadLangs(u32),
    #[error("min_lang_share must be 1..=50, got {0}")]
    MinLangShare(u32),
}

/// Most colours one pad can be split into. Above that a leaf is thinner than the trunk.
pub const MAX_PAD_LANGS: u32 = 4;

/// What `bonsai.toml` may set. Every field is optional in the file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Living branches at most (top repos by commits in the last 365 days).
    pub max_branches: usize,
    /// Repo name globs (`*`, `?`, case-insensitive) that never get a branch.
    pub exclude: Vec<String>,
    /// Repo names that always get a branch, even outside the top `max_branches`.
    pub pin: Vec<String>,
    /// Cells a branch may gain per day (anti-farming).
    pub max_growth_per_day: u32,
    pub bloom_streak_days: u32,
    pub dormant_after_days: u32,
    /// Language colour overrides, `{"Kotlin": "#a97bff"}`.
    pub colors: BTreeMap<String, String>,
    /// Colours a polyglot repo's pad may be split into (1 keeps one colour per branch).
    pub max_pad_langs: u32,
    /// Smallest share of a repo's bytes that earns its own colour on the pad, in percent.
    pub min_lang_share: u32,
    /// How a pad showing several languages is painted: `patches`, `scatter` or `bands`.
    pub pad_mix: PadMix,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            max_branches: 16,
            exclude: Vec::new(),
            pin: Vec::new(),
            max_growth_per_day: 1,
            bloom_streak_days: 21,
            dormant_after_days: 60,
            colors: BTreeMap::new(),
            max_pad_langs: 3,
            min_lang_share: 8,
            pad_mix: PadMix::Patches,
        }
    }
}

impl Config {
    /// Checks ranges; call after merging file and flags.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if !(1..=crate::rules::MAX_BRANCHES).contains(&self.max_branches) {
            return Err(ConfigError::MaxBranches(self.max_branches));
        }
        if !(1..=8).contains(&self.max_growth_per_day) {
            return Err(ConfigError::MaxGrowth(self.max_growth_per_day));
        }
        if self.bloom_streak_days == 0 {
            return Err(ConfigError::ZeroDays("bloom_streak_days"));
        }
        if self.dormant_after_days == 0 {
            return Err(ConfigError::ZeroDays("dormant_after_days"));
        }
        for (lang, value) in &self.colors {
            let ok = value.len() == 7
                && value.starts_with('#')
                && value[1..].chars().all(|c| c.is_ascii_hexdigit());
            if !ok {
                return Err(ConfigError::Color {
                    lang: lang.clone(),
                    value: value.clone(),
                });
            }
        }
        if !(1..=MAX_PAD_LANGS).contains(&self.max_pad_langs) {
            return Err(ConfigError::MaxPadLangs(self.max_pad_langs));
        }
        if !(1..=50).contains(&self.min_lang_share) {
            return Err(ConfigError::MinLangShare(self.min_lang_share));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        assert_eq!(Config::default().validate(), Ok(()));
    }

    #[test]
    fn rejects_bad_values() {
        let c = Config {
            max_branches: 0,
            ..Config::default()
        };
        assert_eq!(c.validate(), Err(ConfigError::MaxBranches(0)));
        let mut c = Config::default();
        c.colors.insert("Kotlin".into(), "a97bff".into());
        assert!(matches!(c.validate(), Err(ConfigError::Color { .. })));
    }

    #[test]
    fn partial_json_fills_defaults() {
        let c: Config = serde_json::from_str(r#"{"pin":["ferrite"]}"#).unwrap();
        assert_eq!(c.pin, vec!["ferrite".to_string()]);
        assert_eq!(c.max_branches, 16);
    }
}
