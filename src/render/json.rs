//! `--json` output. Never contains ANSI codes.

use jiff::ToSpan;
use jiff::civil::Date;
use serde_json::{Value, json};

use crate::day::{DayState, day_state, level};
use crate::model::{Data, Entries, Entry, Habit, Kind};
use crate::stats::{current_streak, longest_streak};

fn value_target(habit: &Habit, entry: Option<&Entry>, date: Date) -> (Value, Value) {
    if habit.kind == Kind::Bool {
        return (Value::Null, Value::Null);
    }
    match entry {
        Some(&Entry::Value { value, target }) => (json!(value), json!(target)),
        _ => (json!(0), json!(habit.target_on(date))),
    }
}

pub fn habit(habit: &Habit, entries: &Entries, today: Date) -> Value {
    let entry = entries.get(&today);
    let (value, target) = value_target(habit, entry, today);
    json!({
        "id": habit.id,
        "name": habit.name,
        "kind": habit.kind,
        "unit": habit.unit,
        "target": habit.target_on(today),
        "color": habit.color,
        "start": habit.start,
        "archived": habit.is_archived(),
        "today": {
            "state": day_state(habit, entry, today, today).name(),
            "value": value,
            "target": target,
        },
        "streak": {
            "current": current_streak(habit, entries, today),
            "longest": longest_streak(habit, entries, today),
        },
    })
}

pub fn list(data: &Data, today: Date, all: bool) -> Value {
    let habits: Vec<Value> = data
        .habits
        .iter()
        .filter(|h| all || !h.is_archived())
        .map(|h| habit(h, data.entries_of(&h.id), today))
        .collect();
    json!({ "today": today, "habits": habits })
}

/// The habit object plus every in-window day from `start` up to today.
pub fn show(h: &Habit, entries: &Entries, today: Date, first: Date) -> Value {
    let mut obj = habit(h, entries, today);
    let days: Vec<Value> = first
        .series(1.day())
        .take_while(|d| *d <= today)
        .filter_map(|d| {
            let entry = entries.get(&d);
            let state = day_state(h, entry, d, today);
            if matches!(state, DayState::BeforeStart | DayState::Future) {
                return None;
            }
            let (value, target) = value_target(h, entry, d);
            Some(json!({
                "date": d,
                "state": state.name(),
                "value": value,
                "target": target,
                "level": level(entry),
            }))
        })
        .collect();
    obj["days"] = Value::Array(days);
    obj
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::count_habit;
    use jiff::civil::date;

    #[test]
    fn shape() {
        let today = date(2026, 10, 27);
        let mut h = count_habit("water", date(2026, 10, 25), 8);
        h.unit = Some("glasses".into());
        let mut e = Entries::new();
        e.insert(
            date(2026, 10, 26),
            Entry::Value {
                value: 8,
                target: 8,
            },
        );
        e.insert(
            today,
            Entry::Value {
                value: 4,
                target: 8,
            },
        );
        let v = show(&h, &e, today, date(2026, 10, 19));
        assert_eq!(
            v["today"],
            json!({"state": "pending", "value": 4, "target": 8})
        );
        assert_eq!(v["streak"], json!({"current": 1, "longest": 1}));
        assert_eq!(v["unit"], "glasses");
        let days = v["days"].as_array().unwrap();
        assert_eq!(days.len(), 3);
        assert_eq!(
            days[0],
            json!({"date": "2026-10-25", "state": "missed", "value": 0, "target": 8, "level": 0})
        );
        assert_eq!(days[1]["level"], 3);
    }
}
