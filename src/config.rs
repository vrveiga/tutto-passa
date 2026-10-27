use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub day_starts_at: u8,
    pub backfill_warn_days: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            day_starts_at: 0,
            backfill_warn_days: 7,
        }
    }
}

/// Raw file contents, wide integer types so range errors can name the key.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct RawConfig {
    day_starts_at: Option<i64>,
    backfill_warn_days: Option<i64>,
}

pub fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("tutto-passa").join("config.toml"))
}

/// Missing file means defaults.
pub fn load(path: &Path) -> Result<Config> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text).with_context(|| format!("config {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

pub fn parse(text: &str) -> Result<Config> {
    let raw: RawConfig = toml::from_str(text)?;
    let mut cfg = Config::default();
    if let Some(v) = raw.day_starts_at {
        if !(0..=23).contains(&v) {
            bail!("day_starts_at must be in 0..=23 (got {v})");
        }
        cfg.day_starts_at = v as u8;
    }
    if let Some(v) = raw.backfill_warn_days {
        cfg.backfill_warn_days = u32::try_from(v)
            .map_err(|_| anyhow::anyhow!("backfill_warn_days must be ≥ 0 (got {v})"))?;
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses() {
        assert_eq!(parse("").unwrap(), Config::default());
        assert_eq!(
            parse("day_starts_at = 4\nbackfill_warn_days = 3").unwrap(),
            Config {
                day_starts_at: 4,
                backfill_warn_days: 3
            }
        );
    }

    #[test]
    fn errors_name_the_key() {
        for (text, key) in [
            ("day_starts_at = 24", "day_starts_at"),
            ("day_starts_at = -1", "day_starts_at"),
            ("backfill_warn_days = -2", "backfill_warn_days"),
            ("colour = 1", "colour"),
            ("day_starts_at = \"x\"", "day_starts_at"),
        ] {
            let err = format!("{:#}", parse(text).unwrap_err());
            assert!(err.contains(key), "{text}: {err}");
        }
    }
}
