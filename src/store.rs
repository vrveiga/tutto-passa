use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;

use crate::model::{DATA_VERSION, Data};

/// `TP_DATA` overrides the XDG data location. Only called from `main`.
pub fn path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("TP_DATA").filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    dirs::data_dir()
        .map(|d| d.join("tutto-passa").join("habits.json"))
        .ok_or_else(|| anyhow!("cannot determine data directory; set TP_DATA"))
}

/// A missing file is an empty data set.
pub fn load(path: &Path) -> Result<Data> {
    match fs::read_to_string(path) {
        Ok(text) => parse(&text).with_context(|| format!("{}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Data::default()),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

/// Single entry point for decoding, so migrations can be hooked in later.
pub fn parse(text: &str) -> Result<Data> {
    #[derive(Deserialize)]
    struct Version {
        version: u32,
    }
    let v: Version = serde_json::from_str(text).context("invalid data file")?;
    if v.version > DATA_VERSION {
        bail!("data file is from a newer tp (version {})", v.version);
    }
    let data: Data = serde_json::from_str(text).context("invalid data file")?;
    data.validate().context("invalid data file")?;
    Ok(data)
}

pub fn to_json(data: &Data) -> String {
    let mut s = serde_json::to_string_pretty(data).expect("data serializes");
    s.push('\n');
    s
}

/// tmp file → fsync → rename → fsync dir.
pub fn save(path: &Path, data: &Data) -> Result<()> {
    let dir = match path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let mut tmp_name = path.file_name().context("data path has no file name")?.to_owned();
    tmp_name.push(".tmp");
    let tmp = dir.join(tmp_name);
    let write = || -> std::io::Result<()> {
        let mut f = File::create(&tmp)?;
        f.write_all(to_json(data).as_bytes())?;
        f.sync_all()?;
        fs::rename(&tmp, path)?;
        File::open(dir)?.sync_all()
    };
    write().with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Entry;

    const SAMPLE: &str = r#"{
  "version": 1,
  "habits": [
    {
      "id": "read",
      "name": "Read 20 pages",
      "kind": "bool",
      "color": "green",
      "start": "2026-10-01",
      "pauses": []
    },
    {
      "id": "water",
      "name": "water",
      "kind": "count",
      "unit": "glasses",
      "color": "blue",
      "start": "2026-10-10",
      "targets": [
        {
          "from": "2026-10-10",
          "value": 6
        },
        {
          "from": "2026-10-20",
          "value": 8
        }
      ],
      "pauses": [
        {
          "from": "2026-10-12",
          "to": "2026-10-15"
        }
      ]
    }
  ],
  "entries": {
    "read": {
      "2026-10-26": "done",
      "2026-10-27": "skip"
    },
    "water": {
      "2026-10-26": {
        "value": 5,
        "target": 8
      },
      "2026-10-27": "skip"
    }
  }
}
"#;

    #[test]
    fn round_trip() {
        let data = parse(SAMPLE).unwrap();
        assert_eq!(data.habits.len(), 2);
        assert_eq!(
            data.entries["water"][&jiff::civil::date(2026, 10, 26)],
            Entry::Value { value: 5, target: 8 }
        );
        assert_eq!(to_json(&data), SAMPLE);
    }

    #[test]
    fn version_too_new() {
        let err = parse(r#"{"version": 2, "habits": "whatever"}"#).unwrap_err();
        assert!(err.to_string().contains("newer tp"));
    }

    #[test]
    fn corrupt_reports_position() {
        let err = format!("{:#}", parse("{\"version\": 1,\n  \"habits\": [}").unwrap_err());
        assert!(err.contains("line 2"), "{err}");
    }

    #[test]
    fn validation_failures() {
        let cases = [
            (r#""id": "read""#, r#""id": "Read""#, "invalid habit id"),
            (r#""id": "water""#, r#""id": "read""#, "duplicate"),
            (r#""water": {"#, r#""ghost": {"#, "unknown habit"),
            (r#""to": "2026-10-15""#, r#""to": "2026-10-01""#, "ends before"),
            (r#""2026-10-26": "done""#, r#""2026-10-26": {"value": 1, "target": 1}"#, "invalid entry"),
        ];
        for (from, to, msg) in cases {
            let text = SAMPLE.replacen(from, to, 1);
            assert_ne!(text, SAMPLE);
            let err = format!("{:#}", parse(&text).unwrap_err());
            assert!(err.contains(msg), "{msg}: {err}");
        }
        let no_targets = SAMPLE.replace(
            r#""targets": [
        {
          "from": "2026-10-10",
          "value": 6
        },
        {
          "from": "2026-10-20",
          "value": 8
        }
      ],"#,
            "",
        );
        assert!(format!("{:#}", parse(&no_targets).unwrap_err()).contains("no target"));
        let overlap = SAMPLE.replace(
            r#""pauses": [
        {"#,
            r#""pauses": [
        {"from": "2026-10-11", "to": null},
        {"#,
        );
        assert!(parse(&overlap).is_err());
    }

    #[test]
    fn save_and_load() {
        let dir = std::env::temp_dir().join(format!("tp-store-{}", std::process::id()));
        let path = dir.join("sub").join("habits.json");
        assert_eq!(load(&path).unwrap(), Data::default());
        let data = parse(SAMPLE).unwrap();
        save(&path, &data).unwrap();
        assert_eq!(load(&path).unwrap(), data);
        assert!(!path.with_file_name("habits.json.tmp").exists());
        fs::write(&path, "garbage").unwrap();
        assert!(load(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "garbage");
        fs::remove_dir_all(dir).unwrap();
    }
}
