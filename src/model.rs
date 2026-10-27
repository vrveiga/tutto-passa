use std::collections::{BTreeMap, HashSet};

use anyhow::{Result, bail};
use jiff::civil::Date;
use serde::{Deserialize, Serialize};

pub const DATA_VERSION: u32 = 1;

pub type Entries = BTreeMap<Date, Entry>;

static NO_ENTRIES: Entries = BTreeMap::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Bool,
    Count,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Hue {
    #[default]
    Green,
    Blue,
    Purple,
    Orange,
    Red,
    Yellow,
    Pink,
    Cyan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetChange {
    pub from: Date,
    pub value: u32,
}

/// Half-open interval `[from, to)`; `to == None` means still archived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pause {
    pub from: Date,
    pub to: Option<Date>,
}

impl Pause {
    pub fn contains(&self, date: Date) -> bool {
        self.from <= date && self.to.is_none_or(|to| date < to)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Habit {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default)]
    pub color: Hue,
    pub start: Date,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<TargetChange>,
    #[serde(default)]
    pub pauses: Vec<Pause>,
}

impl Habit {
    /// Target in effect on `date`. Dates before the first change use the first target.
    pub fn target_on(&self, date: Date) -> Option<u32> {
        self.targets
            .iter()
            .rev()
            .find(|t| t.from <= date)
            .or(self.targets.first())
            .map(|t| t.value)
    }

    pub fn pause_containing(&self, date: Date) -> Option<&Pause> {
        self.pauses.iter().find(|p| p.contains(date))
    }

    pub fn open_pause(&self) -> Option<&Pause> {
        self.pauses.iter().find(|p| p.to.is_none())
    }

    pub fn is_archived(&self) -> bool {
        self.open_pause().is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    Done,
    Skip,
    Value { value: u32, target: u32 },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum EntryWord {
    Done,
    Skip,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum EntryRepr {
    Word(EntryWord),
    Value { value: u32, target: u32 },
}

impl Serialize for Entry {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match *self {
            Entry::Done => EntryRepr::Word(EntryWord::Done),
            Entry::Skip => EntryRepr::Word(EntryWord::Skip),
            Entry::Value { value, target } => EntryRepr::Value { value, target },
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for Entry {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match EntryRepr::deserialize(d)? {
            EntryRepr::Word(EntryWord::Done) => Entry::Done,
            EntryRepr::Word(EntryWord::Skip) => Entry::Skip,
            EntryRepr::Value { value, target } => Entry::Value { value, target },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Data {
    pub version: u32,
    #[serde(default)]
    pub habits: Vec<Habit>,
    #[serde(default)]
    pub entries: BTreeMap<String, Entries>,
}

impl Default for Data {
    fn default() -> Self {
        Data { version: DATA_VERSION, habits: Vec::new(), entries: BTreeMap::new() }
    }
}

impl Data {
    pub fn entries_of(&self, id: &str) -> &Entries {
        self.entries.get(id).unwrap_or(&NO_ENTRIES)
    }

    pub fn find(&self, id: &str) -> Option<usize> {
        self.habits.iter().position(|h| h.id == id)
    }

    pub fn validate(&self) -> Result<()> {
        let mut seen = HashSet::new();
        for h in &self.habits {
            if !valid_id(&h.id) {
                bail!("invalid habit id '{}'", h.id);
            }
            if !seen.insert(h.id.as_str()) {
                bail!("duplicate habit id '{}'", h.id);
            }
            match h.kind {
                Kind::Count => {
                    if h.targets.is_empty() {
                        bail!("count habit '{}' has no target", h.id);
                    }
                    if h.targets.iter().any(|t| t.value == 0) {
                        bail!("habit '{}' has a zero target", h.id);
                    }
                    if h.targets.windows(2).any(|w| w[0].from >= w[1].from) {
                        bail!("targets of '{}' are not sorted", h.id);
                    }
                }
                Kind::Bool => {
                    if !h.targets.is_empty() {
                        bail!("yes/no habit '{}' has targets", h.id);
                    }
                }
            }
            for (i, p) in h.pauses.iter().enumerate() {
                if p.to.is_some_and(|to| to < p.from) {
                    bail!("pause of '{}' ends before it starts", h.id);
                }
                let last = i + 1 == h.pauses.len();
                if p.to.is_none() && !last {
                    bail!("habit '{}' has more than one open pause", h.id);
                }
                if let Some(next) = h.pauses.get(i + 1)
                    && p.to.is_none_or(|to| to > next.from)
                {
                    bail!("pauses of '{}' overlap", h.id);
                }
            }
        }
        for (id, entries) in &self.entries {
            let Some(i) = self.find(id) else {
                bail!("entries for unknown habit '{id}'");
            };
            let kind = self.habits[i].kind;
            for (date, e) in entries {
                match (kind, e) {
                    (_, Entry::Skip) | (Kind::Bool, Entry::Done) => {}
                    (Kind::Count, Entry::Value { target, .. }) if *target > 0 => {}
                    _ => bail!("invalid entry for '{id}' on {date}"),
                }
            }
        }
        Ok(())
    }
}

/// `^[a-z0-9][a-z0-9-]{0,31}$`
pub fn valid_id(id: &str) -> bool {
    let b = id.as_bytes();
    let ok = |c: &u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    !b.is_empty() && b.len() <= 32 && ok(&b[0]) && b[1..].iter().all(|c| ok(c) || *c == b'-')
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use jiff::civil::date;

    pub fn bool_habit(id: &str, start: Date) -> Habit {
        Habit {
            id: id.into(),
            name: id.into(),
            kind: Kind::Bool,
            unit: None,
            color: Hue::Green,
            start,
            targets: vec![],
            pauses: vec![],
        }
    }

    pub fn count_habit(id: &str, start: Date, target: u32) -> Habit {
        Habit {
            kind: Kind::Count,
            targets: vec![TargetChange { from: start, value: target }],
            ..bool_habit(id, start)
        }
    }

    #[test]
    fn ids() {
        for (id, ok) in [
            ("read", true),
            ("a", true),
            ("0-x", true),
            ("no-smoking", true),
            ("-x", false),
            ("", false),
            ("Read", false),
            ("a_b", false),
            (&"a".repeat(32), true),
            (&"a".repeat(33), false),
        ] {
            assert_eq!(valid_id(id), ok, "{id}");
        }
    }

    #[test]
    fn target_history() {
        let mut h = count_habit("water", date(2026, 10, 10), 6);
        h.targets.push(TargetChange { from: date(2026, 10, 20), value: 8 });
        assert_eq!(h.target_on(date(2026, 10, 1)), Some(6));
        assert_eq!(h.target_on(date(2026, 10, 19)), Some(6));
        assert_eq!(h.target_on(date(2026, 10, 20)), Some(8));
        assert_eq!(h.target_on(date(2026, 12, 1)), Some(8));
    }

    #[test]
    fn pause_bounds() {
        let p = Pause { from: date(2026, 9, 1), to: Some(date(2026, 9, 10)) };
        assert!(!p.contains(date(2026, 8, 31)));
        assert!(p.contains(date(2026, 9, 1)));
        assert!(p.contains(date(2026, 9, 9)));
        assert!(!p.contains(date(2026, 9, 10)));
        let open = Pause { from: date(2026, 9, 1), to: None };
        assert!(open.contains(date(2030, 1, 1)));
    }

    #[test]
    fn entry_serde() {
        let json = r#"{"2026-10-26":"done","2026-10-27":"skip","2026-10-28":{"value":5,"target":8}}"#;
        let e: Entries = serde_json::from_str(json).unwrap();
        assert_eq!(e[&date(2026, 10, 26)], Entry::Done);
        assert_eq!(e[&date(2026, 10, 27)], Entry::Skip);
        assert_eq!(e[&date(2026, 10, 28)], Entry::Value { value: 5, target: 8 });
        assert_eq!(serde_json::to_string(&e).unwrap(), json);
        assert!(serde_json::from_str::<Entry>(r#""maybe""#).is_err());
    }
}
