//! Mutations on `Data`. Pure: no I/O, `today` is always a parameter.

use anyhow::{Result, bail};
use jiff::civil::Date;

use crate::config::Config;
use crate::day::{DayState, day_state};
use crate::model::{Data, Entry, Habit, Hue, Kind, Pause, TargetChange, valid_id};
use crate::stats::current_streak;

/// What to print: `message` on stdout, the rest on stderr.
#[derive(Debug, Default)]
pub struct Report {
    pub message: String,
    pub notes: Vec<String>,
    pub warnings: Vec<String>,
}

impl Report {
    fn new(message: impl Into<String>) -> Self {
        Report { message: message.into(), ..Default::default() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoneMode {
    /// Add N (default 1). `None` means no N was given.
    Add(Option<u32>),
    Set(u32),
    Full,
}

#[derive(Debug, Clone)]
pub struct NewHabit {
    pub id: String,
    pub name: Option<String>,
    pub target: Option<u32>,
    pub unit: Option<String>,
    pub color: Hue,
    pub since: Option<Date>,
}

pub fn add(data: &mut Data, new: NewHabit, today: Date) -> Result<Report> {
    if !valid_id(&new.id) {
        bail!(
            "invalid id '{}': use lowercase letters, digits and '-', starting with a letter or digit (max 32)",
            new.id
        );
    }
    if data.find(&new.id).is_some() {
        bail!("habit '{}' already exists", new.id);
    }
    let start = new.since.unwrap_or(today);
    if start > today {
        bail!("{start} is in the future");
    }
    if new.target == Some(0) {
        bail!("target must be a positive integer");
    }
    if new.unit.is_some() && new.target.is_none() {
        bail!("--unit requires --target");
    }
    let mut message = format!("added {}", new.id);
    if let Some(t) = new.target {
        message += &format!(" (target {t}{})", unit_suffix(new.unit.as_deref()));
    }
    data.habits.push(Habit {
        name: new.name.unwrap_or_else(|| new.id.clone()),
        id: new.id,
        kind: if new.target.is_some() { Kind::Count } else { Kind::Bool },
        unit: new.unit,
        color: new.color,
        start,
        targets: new.target.map(|value| TargetChange { from: start, value }).into_iter().collect(),
        pauses: vec![],
    });
    Ok(Report::new(message))
}

/// Shared checks for any write to `date`: future, archived, paused, backfill warning.
fn check_date(habit: &Habit, date: Date, today: Date, cfg: &Config, report: &mut Report) -> Result<()> {
    if date > today {
        bail!("{date} is in the future");
    }
    if let Some(p) = habit.open_pause() {
        bail!("{} is archived since {}; run 'tp unarchive {}'", habit.id, p.from, habit.id);
    }
    if let Some(p) = habit.pause_containing(date) {
        let to = p.to.map_or_else(String::new, |d| d.to_string());
        bail!("{date} is inside a pause of {} ({}..{to})", habit.id, p.from);
    }
    let ago = i64::from((today - date).get_days());
    if ago > i64::from(cfg.backfill_warn_days) {
        report.warnings.push(format!("editing {date} ({ago} days ago)"));
    }
    Ok(())
}

/// Logging before the start moves the start back; it never moves forward.
fn extend_start(habit: &mut Habit, date: Date, report: &mut Report) {
    if date < habit.start {
        habit.start = date;
        report.notes.push(format!("{} start moved to {date}", habit.id));
    }
}

pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn unit_suffix(unit: Option<&str>) -> String {
    unit.map_or_else(String::new, |u| format!(" {u}"))
}

/// `✓ water — 2026-10-27 · 8/8 glasses · streak 4`
fn status_line(data: &Data, idx: usize, date: Date, today: Date) -> String {
    let h = &data.habits[idx];
    let entries = data.entries_of(&h.id);
    let entry = entries.get(&date);
    let done = day_state(h, entry, date, today) == DayState::Done;
    let mut s = format!("{} {} — {date}", if done { "✓" } else { " " }, h.id);
    if h.kind == Kind::Count {
        let (value, target) = match entry {
            Some(&Entry::Value { value, target }) => (value, target),
            _ => (0, h.target_on(date).unwrap_or(0)),
        };
        s += &format!(" · {value}/{target}{}", unit_suffix(h.unit.as_deref()));
    }
    s + &format!(" · streak {}", current_streak(h, entries, today))
}

pub fn done(
    data: &mut Data,
    idx: usize,
    mode: DoneMode,
    date: Date,
    today: Date,
    cfg: &Config,
) -> Result<Report> {
    let mut report = Report::default();
    let habit = &data.habits[idx];
    if habit.kind == Kind::Bool && mode != DoneMode::Add(None) {
        bail!("{} is a yes/no habit", habit.id);
    }
    check_date(habit, date, today, cfg, &mut report)?;
    let id = habit.id.clone();
    let snapshot = habit.target_on(date);
    extend_start(&mut data.habits[idx], date, &mut report);
    let entries = data.entries.entry(id.clone()).or_default();
    let old = entries.get(&date).copied();
    match snapshot {
        None => {
            if old == Some(Entry::Done) {
                report.message = format!("✓ {id} — {date} · already done");
                return Ok(report);
            }
            entries.insert(date, Entry::Done);
        }
        Some(target_on) => {
            let (current, target) = match old {
                Some(Entry::Value { value, target }) => (value, target),
                _ => (0, target_on),
            };
            let value = match mode {
                DoneMode::Add(n) => current.saturating_add(n.unwrap_or(1)),
                DoneMode::Set(n) => n,
                DoneMode::Full => target,
            };
            if value == 0 {
                entries.remove(&date);
            } else {
                entries.insert(date, Entry::Value { value, target });
            }
        }
    }
    prune(data, &id);
    report.message = status_line(data, idx, date, today);
    Ok(report)
}

pub fn skip(data: &mut Data, idx: usize, date: Date, today: Date, cfg: &Config) -> Result<Report> {
    let mut report = Report::default();
    check_date(&data.habits[idx], date, today, cfg, &mut report)?;
    let id = data.habits[idx].id.clone();
    extend_start(&mut data.habits[idx], date, &mut report);
    data.entries.entry(id.clone()).or_default().insert(date, Entry::Skip);
    report.message = format!("· {id} skipped — {date}");
    Ok(report)
}

pub fn undo(data: &mut Data, idx: usize, date: Date, today: Date, cfg: &Config) -> Result<Report> {
    let mut report = Report::default();
    check_date(&data.habits[idx], date, today, cfg, &mut report)?;
    let id = data.habits[idx].id.clone();
    let removed = data.entries.get_mut(&id).and_then(|e| e.remove(&date));
    prune(data, &id);
    report.message = match removed {
        Some(_) => format!("{id} — {date} cleared"),
        None => "nothing to undo".into(),
    };
    Ok(report)
}

/// Drop an empty per-habit entries map so the file stays tidy.
fn prune(data: &mut Data, id: &str) {
    if data.entries.get(id).is_some_and(|e| e.is_empty()) {
        data.entries.remove(id);
    }
}

pub fn target(data: &mut Data, idx: usize, value: u32, today: Date) -> Result<Report> {
    let habit = &mut data.habits[idx];
    if habit.kind == Kind::Bool {
        bail!("{} is a yes/no habit", habit.id);
    }
    if value == 0 {
        bail!("target must be a positive integer");
    }
    habit.targets.retain(|t| t.from < today);
    habit.targets.push(TargetChange { from: today, value });
    let msg = format!("{} target {value}{} from {today}", habit.id, unit_suffix(habit.unit.as_deref()));
    if let Some(Entry::Value { target, .. }) =
        data.entries.get_mut(&habit.id).and_then(|e| e.get_mut(&today))
    {
        *target = value;
    }
    Ok(Report::new(msg))
}

pub fn rename(data: &mut Data, idx: usize, new_id: &str, name: Option<String>) -> Result<Report> {
    if !valid_id(new_id) {
        bail!("invalid id '{new_id}'");
    }
    let old_id = data.habits[idx].id.clone();
    if new_id != old_id && data.find(new_id).is_some() {
        bail!("habit '{new_id}' already exists");
    }
    let habit = &mut data.habits[idx];
    match name {
        Some(n) => habit.name = n,
        None if habit.name == old_id => habit.name = new_id.to_string(),
        None => {}
    }
    habit.id = new_id.to_string();
    if let Some(e) = data.entries.remove(&old_id) {
        data.entries.insert(new_id.to_string(), e);
    }
    Ok(Report::new(format!("renamed {old_id} → {new_id}")))
}

pub fn archive(data: &mut Data, idx: usize, today: Date) -> Result<Report> {
    let habit = &mut data.habits[idx];
    if let Some(p) = habit.open_pause() {
        bail!("{} is already archived since {}", habit.id, p.from);
    }
    if habit.pauses.last().and_then(|p| p.to).is_some_and(|to| to > today) {
        bail!("{} has a pause ending after {today}", habit.id);
    }
    habit.pauses.push(Pause { from: today, to: None });
    Ok(Report::new(format!("archived {}", habit.id)))
}

pub fn unarchive(data: &mut Data, idx: usize, today: Date) -> Result<Report> {
    let habit = &mut data.habits[idx];
    let Some(p) = habit.pauses.last_mut().filter(|p| p.to.is_none()) else {
        bail!("{} is not archived", habit.id);
    };
    if p.from >= today {
        habit.pauses.pop();
    } else {
        p.to = Some(today);
    }
    Ok(Report::new(format!("unarchived {}", habit.id)))
}

pub fn delete(data: &mut Data, idx: usize) -> Report {
    let habit = data.habits.remove(idx);
    let n = data.entries.remove(&habit.id).map_or(0, |e| e.len());
    Report::new(format!("deleted {} ({})", habit.id, plural(n, "entry", "entries")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    const TODAY: Date = date(2026, 10, 27);
    const CFG: Config = Config { day_starts_at: 0, backfill_warn_days: 7 };

    fn new(id: &str, target: Option<u32>) -> NewHabit {
        NewHabit {
            id: id.into(),
            name: None,
            target,
            unit: target.map(|_| "glasses".into()),
            color: Hue::Green,
            since: Some(date(2026, 10, 1)),
        }
    }

    fn setup() -> Data {
        let mut d = Data::default();
        add(&mut d, new("read", None), TODAY).unwrap();
        add(&mut d, new("water", Some(8)), TODAY).unwrap();
        d
    }

    fn get(d: &Data, id: &str, date: Date) -> Option<Entry> {
        d.entries_of(id).get(&date).copied()
    }

    fn val(value: u32) -> Option<Entry> {
        Some(Entry::Value { value, target: 8 })
    }

    #[test]
    fn add_validates() {
        let mut d = setup();
        assert!(add(&mut d, new("read", None), TODAY).is_err());
        assert!(add(&mut d, new("Bad", None), TODAY).is_err());
        let future = NewHabit { since: Some(date(2026, 10, 28)), ..new("x", None) };
        assert!(add(&mut d, future, TODAY).unwrap_err().to_string().contains("in the future"));
        let h = &d.habits[1];
        assert_eq!((h.kind, h.start), (Kind::Count, date(2026, 10, 1)));
        assert_eq!(h.targets, vec![TargetChange { from: date(2026, 10, 1), value: 8 }]);
        d.validate().unwrap();
    }

    #[test]
    fn done_count_modes() {
        let mut d = setup();
        let w = 1;
        done(&mut d, w, DoneMode::Add(None), TODAY, TODAY, &CFG).unwrap();
        assert_eq!(get(&d, "water", TODAY), val(1));
        let r = done(&mut d, w, DoneMode::Add(Some(3)), TODAY, TODAY, &CFG).unwrap();
        assert_eq!(get(&d, "water", TODAY), val(4));
        assert_eq!(r.message, "  water — 2026-10-27 · 4/8 glasses · streak 0");
        done(&mut d, w, DoneMode::Set(6), TODAY, TODAY, &CFG).unwrap();
        assert_eq!(get(&d, "water", TODAY), val(6));
        let r = done(&mut d, w, DoneMode::Full, TODAY, TODAY, &CFG).unwrap();
        assert_eq!(get(&d, "water", TODAY), val(8));
        assert_eq!(r.message, "✓ water — 2026-10-27 · 8/8 glasses · streak 1");
        done(&mut d, w, DoneMode::Set(0), TODAY, TODAY, &CFG).unwrap();
        assert_eq!(get(&d, "water", TODAY), None);
        assert!(!d.entries.contains_key("water"));
        done(&mut d, w, DoneMode::Set(u32::MAX), TODAY, TODAY, &CFG).unwrap();
        done(&mut d, w, DoneMode::Add(Some(5)), TODAY, TODAY, &CFG).unwrap();
        assert_eq!(get(&d, "water", TODAY), val(u32::MAX));
    }

    #[test]
    fn done_bool() {
        let mut d = setup();
        let r = done(&mut d, 0, DoneMode::Add(None), TODAY, TODAY, &CFG).unwrap();
        assert_eq!(r.message, "✓ read — 2026-10-27 · streak 1");
        let r = done(&mut d, 0, DoneMode::Add(None), TODAY, TODAY, &CFG).unwrap();
        assert!(r.message.contains("already done"));
        for mode in [DoneMode::Add(Some(3)), DoneMode::Set(1), DoneMode::Full] {
            let err = done(&mut d, 0, mode, TODAY, TODAY, &CFG).unwrap_err();
            assert_eq!(err.to_string(), "read is a yes/no habit");
        }
    }

    #[test]
    fn skip_and_done_replace_each_other() {
        let mut d = setup();
        done(&mut d, 1, DoneMode::Add(Some(5)), TODAY, TODAY, &CFG).unwrap();
        let r = skip(&mut d, 1, TODAY, TODAY, &CFG).unwrap();
        assert_eq!(r.message, "· water skipped — 2026-10-27");
        assert_eq!(get(&d, "water", TODAY), Some(Entry::Skip));
        done(&mut d, 1, DoneMode::Add(Some(2)), TODAY, TODAY, &CFG).unwrap();
        assert_eq!(get(&d, "water", TODAY), val(2));
        skip(&mut d, 0, TODAY, TODAY, &CFG).unwrap();
        done(&mut d, 0, DoneMode::Add(None), TODAY, TODAY, &CFG).unwrap();
        assert_eq!(get(&d, "read", TODAY), Some(Entry::Done));
    }

    #[test]
    fn undo_entry() {
        let mut d = setup();
        assert_eq!(undo(&mut d, 0, TODAY, TODAY, &CFG).unwrap().message, "nothing to undo");
        done(&mut d, 0, DoneMode::Add(None), TODAY, TODAY, &CFG).unwrap();
        undo(&mut d, 0, TODAY, TODAY, &CFG).unwrap();
        assert_eq!(get(&d, "read", TODAY), None);
    }

    #[test]
    fn backfill_before_start_moves_start() {
        let mut d = setup();
        let early = date(2026, 9, 20);
        let r = done(&mut d, 1, DoneMode::Add(Some(2)), early, TODAY, &CFG).unwrap();
        assert_eq!(d.habits[1].start, early);
        assert_eq!(r.notes, vec!["water start moved to 2026-09-20"]);
        assert_eq!(r.warnings, vec!["editing 2026-09-20 (37 days ago)"]);
        assert_eq!(get(&d, "water", early), val(2)); // first target applies
        undo(&mut d, 1, early, TODAY, &CFG).unwrap();
        assert_eq!(d.habits[1].start, early);
        d.validate().unwrap();
    }

    #[test]
    fn backfill_warning_threshold() {
        let mut d = setup();
        let r = done(&mut d, 0, DoneMode::Add(None), date(2026, 10, 20), TODAY, &CFG).unwrap();
        assert!(r.warnings.is_empty());
        let r = done(&mut d, 0, DoneMode::Add(None), date(2026, 10, 19), TODAY, &CFG).unwrap();
        assert_eq!(r.warnings.len(), 1);
    }

    #[test]
    fn future_rejected() {
        let mut d = setup();
        let err = done(&mut d, 0, DoneMode::Add(None), date(2026, 10, 28), TODAY, &CFG).unwrap_err();
        assert_eq!(err.to_string(), "2026-10-28 is in the future");
    }

    #[test]
    fn pauses_reject_writes() {
        let mut d = setup();
        archive(&mut d, 0, date(2026, 10, 10)).unwrap();
        let err = done(&mut d, 0, DoneMode::Add(None), TODAY, TODAY, &CFG).unwrap_err();
        assert_eq!(err.to_string(), "read is archived since 2026-10-10; run 'tp unarchive read'");
        assert!(skip(&mut d, 0, date(2026, 10, 5), TODAY, &CFG).is_err());
        assert!(archive(&mut d, 0, TODAY).is_err());
        unarchive(&mut d, 0, date(2026, 10, 20)).unwrap();
        assert_eq!(d.habits[0].pauses, vec![Pause { from: date(2026, 10, 10), to: Some(date(2026, 10, 20)) }]);
        let err = done(&mut d, 0, DoneMode::Add(None), date(2026, 10, 12), TODAY, &CFG).unwrap_err();
        assert_eq!(err.to_string(), "2026-10-12 is inside a pause of read (2026-10-10..2026-10-20)");
        done(&mut d, 0, DoneMode::Add(None), date(2026, 10, 20), TODAY, &CFG).unwrap();
        assert!(unarchive(&mut d, 0, TODAY).is_err());
        archive(&mut d, 0, TODAY).unwrap();
        unarchive(&mut d, 0, TODAY).unwrap();
        assert_eq!(d.habits[0].pauses.len(), 1);
        d.validate().unwrap();
    }

    #[test]
    fn target_snapshot() {
        let mut d = setup();
        let y = date(2026, 10, 26);
        done(&mut d, 1, DoneMode::Add(Some(6)), y, TODAY, &CFG).unwrap();
        done(&mut d, 1, DoneMode::Add(Some(6)), TODAY, TODAY, &CFG).unwrap();
        target(&mut d, 1, 10, TODAY).unwrap();
        target(&mut d, 1, 12, TODAY).unwrap();
        assert_eq!(get(&d, "water", y), val(6));
        assert_eq!(get(&d, "water", TODAY), Some(Entry::Value { value: 6, target: 12 }));
        assert_eq!(d.habits[1].targets.len(), 2);
        assert_eq!(d.habits[1].target_on(TODAY), Some(12));
        assert!(target(&mut d, 0, 3, TODAY).is_err());
        d.validate().unwrap();
    }

    #[test]
    fn target_on_start_day_replaces() {
        let mut d = Data::default();
        add(&mut d, NewHabit { since: None, ..new("water", Some(8)) }, TODAY).unwrap();
        target(&mut d, 0, 5, TODAY).unwrap();
        assert_eq!(d.habits[0].targets, vec![TargetChange { from: TODAY, value: 5 }]);
    }

    #[test]
    fn rename_and_delete() {
        let mut d = setup();
        done(&mut d, 0, DoneMode::Add(None), TODAY, TODAY, &CFG).unwrap();
        assert!(rename(&mut d, 0, "water", None).is_err());
        rename(&mut d, 0, "books", None).unwrap();
        assert_eq!((d.habits[0].id.as_str(), d.habits[0].name.as_str()), ("books", "books"));
        assert_eq!(get(&d, "books", TODAY), Some(Entry::Done));
        rename(&mut d, 0, "books", Some("Read 20 pages".into())).unwrap();
        assert_eq!(d.habits[0].name, "Read 20 pages");
        assert_eq!(delete(&mut d, 0).message, "deleted books (1 entry)");
        assert!(d.entries.is_empty());
        d.validate().unwrap();
    }
}
