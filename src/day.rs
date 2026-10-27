use jiff::civil::Date;

use crate::model::{Entry, Habit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayState {
    Future,
    BeforeStart,
    Paused,
    Skip,
    Done,
    Partial,
    Pending,
    Missed,
}

impl DayState {
    pub fn name(self) -> &'static str {
        match self {
            DayState::Future => "future",
            DayState::BeforeStart => "before_start",
            DayState::Paused => "paused",
            DayState::Skip => "skip",
            DayState::Done => "done",
            DayState::Partial => "partial",
            DayState::Pending => "pending",
            DayState::Missed => "missed",
        }
    }
}

/// Spec §1.4, in priority order.
pub fn day_state(habit: &Habit, entry: Option<&Entry>, date: Date, today: Date) -> DayState {
    if date > today {
        return DayState::Future;
    }
    if date < habit.start {
        return DayState::BeforeStart;
    }
    if habit.pause_containing(date).is_some() {
        return DayState::Paused;
    }
    match entry {
        Some(Entry::Skip) => return DayState::Skip,
        Some(Entry::Done) => return DayState::Done,
        Some(&Entry::Value { value, target }) if value >= target => return DayState::Done,
        Some(&Entry::Value { value, .. }) if value > 0 && date < today => return DayState::Partial,
        _ => {}
    }
    if date == today { DayState::Pending } else { DayState::Missed }
}

/// Heatmap intensity 0..=4 (spec §3.2).
pub fn level(entry: Option<&Entry>) -> u8 {
    match entry {
        Some(Entry::Done) => 4,
        Some(&Entry::Value { value, target }) => {
            let (v, t) = (u64::from(value), u64::from(target));
            if v == 0 {
                0
            } else if 2 * v < t {
                1
            } else if v < t {
                2
            } else if 2 * v < 3 * t {
                3
            } else {
                4
            }
        }
        Some(Entry::Skip) | None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Pause;
    use crate::model::tests::{bool_habit, count_habit};
    use jiff::civil::date;
    use DayState::*;

    #[test]
    fn states() {
        let today = date(2026, 10, 27);
        let mut h = count_habit("water", date(2026, 10, 1), 8);
        h.pauses.push(Pause { from: date(2026, 10, 10), to: Some(date(2026, 10, 15)) });
        let v = |value| Some(Entry::Value { value, target: 8 });
        let cases = [
            (date(2026, 10, 28), v(8), Future),
            (date(2026, 9, 30), v(8), BeforeStart),
            (date(2026, 10, 1), None, Missed),
            (date(2026, 10, 9), v(8), Done),
            (date(2026, 10, 10), v(8), Paused),
            (date(2026, 10, 14), None, Paused),
            (date(2026, 10, 15), None, Missed),
            (date(2026, 10, 20), Some(Entry::Skip), Skip),
            (date(2026, 10, 20), v(9), Done),
            (date(2026, 10, 20), v(7), Partial),
            (date(2026, 10, 20), v(0), Missed),
            (today, None, Pending),
            (today, v(3), Pending),
            (today, v(8), Done),
            (today, Some(Entry::Skip), Skip),
        ];
        for (d, e, want) in cases {
            assert_eq!(day_state(&h, e.as_ref(), d, today), want, "{d} {e:?}");
        }
        let b = bool_habit("read", date(2026, 10, 1));
        assert_eq!(day_state(&b, Some(&Entry::Done), today, today), Done);
        assert_eq!(day_state(&b, None, date(2026, 10, 26), today), Missed);
        assert_eq!(day_state(&b, Some(&Entry::Done), date(2026, 10, 1), today), Done);
    }

    #[test]
    fn levels() {
        let v = |value| Some(Entry::Value { value, target: 10 });
        for (e, want) in [
            (None, 0),
            (Some(Entry::Skip), 0),
            (Some(Entry::Done), 4),
            (v(0), 0),
            (v(1), 1),
            (v(4), 1),
            (v(5), 2),
            (v(9), 2),
            (v(10), 3),
            (v(14), 3),
            (v(15), 4),
            (Some(Entry::Value { value: u32::MAX, target: u32::MAX }), 3),
        ] {
            assert_eq!(level(e.as_ref()), want, "{e:?}");
        }
    }
}
