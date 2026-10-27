use jiff::ToSpan;
use jiff::civil::Date;

use crate::day::{DayState, day_state};
use crate::model::{Entries, Habit};

fn state(habit: &Habit, entries: &Entries, date: Date, today: Date) -> DayState {
    day_state(habit, entries.get(&date), date, today)
}

/// Spec §4.1.
pub fn current_streak(habit: &Habit, entries: &Entries, today: Date) -> u32 {
    let mut n = 0;
    let mut d = today;
    loop {
        match state(habit, entries, d, today) {
            DayState::Done => n += 1,
            DayState::Skip => {}
            DayState::Pending if d == today => {}
            _ => break,
        }
        match d.checked_sub(1.day()) {
            Ok(prev) => d = prev,
            Err(_) => break,
        }
    }
    n
}

/// Spec §4.2.
pub fn longest_streak(habit: &Habit, entries: &Entries, today: Date) -> u32 {
    let (mut best, mut run) = (0, 0);
    for d in habit.start.series(1.day()).take_while(|d| *d <= today) {
        match state(habit, entries, d, today) {
            DayState::Done => {
                run += 1;
                best = best.max(run);
            }
            DayState::Skip | DayState::Pending => {}
            _ => run = 0,
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::{bool_habit, count_habit};
    use crate::model::{Entry, Pause, TargetChange};
    use jiff::civil::date;

    const TODAY: Date = date(2026, 10, 27);

    /// One char per day ending today: `x` done, `s` skip, `.` missed, `p` partial.
    fn entries(pattern: &str, target: u32) -> Entries {
        let n = pattern.len() as i64;
        pattern
            .chars()
            .enumerate()
            .filter_map(|(i, c)| {
                let d = TODAY.checked_sub((n - 1 - i as i64).days()).unwrap();
                let e = match c {
                    'x' if target == 0 => Entry::Done,
                    'x' => Entry::Value {
                        value: target,
                        target,
                    },
                    'p' => Entry::Value {
                        value: 1,
                        target: target.max(2),
                    },
                    's' => Entry::Skip,
                    _ => return None,
                };
                Some((d, e))
            })
            .collect()
    }

    #[test]
    fn streaks() {
        let h = bool_habit("read", date(2026, 10, 1));
        for (pattern, cur, long) in [
            ("xxx", 3, 3),
            ("xx.", 2, 2), // today pending doesn't break
            ("xsx", 2, 2), // skip transparent
            ("xxsxx", 4, 4),
            ("xxx.xx", 2, 3),
            ("xxxs.", 3, 3),
            ("ss", 0, 0),
            ("", 0, 0),
        ] {
            let e = entries(pattern, 0);
            assert_eq!(current_streak(&h, &e, TODAY), cur, "current {pattern}");
            assert_eq!(longest_streak(&h, &e, TODAY), long, "longest {pattern}");
        }
    }

    #[test]
    fn partial_breaks_except_today() {
        let h = count_habit("water", date(2026, 10, 1), 8);
        let e = entries("xxpxx", 8);
        assert_eq!(current_streak(&h, &e, TODAY), 2);
        assert_eq!(longest_streak(&h, &e, TODAY), 2);
        let e = entries("xxxp", 8);
        assert_eq!(current_streak(&h, &e, TODAY), 3);
    }

    #[test]
    fn stops_at_start() {
        let h = bool_habit("read", date(2026, 10, 25));
        let e = entries("xxxxx", 0);
        assert_eq!(current_streak(&h, &e, TODAY), 3);
        assert_eq!(longest_streak(&h, &e, TODAY), 3);
    }

    #[test]
    fn pause_resets() {
        let mut h = bool_habit("gym", date(2026, 10, 1));
        h.pauses.push(Pause {
            from: date(2026, 10, 24),
            to: Some(date(2026, 10, 26)),
        });
        let e = entries("xxxxxxx", 0);
        assert_eq!(current_streak(&h, &e, TODAY), 2);
        assert_eq!(longest_streak(&h, &e, TODAY), 3);
    }

    #[test]
    fn raised_target_keeps_history() {
        let mut h = count_habit("water", date(2026, 10, 1), 6);
        h.targets.push(TargetChange {
            from: TODAY,
            value: 8,
        });
        let mut e = entries("xxx", 6); // stamped with target 6
        e.insert(
            TODAY,
            Entry::Value {
                value: 6,
                target: 8,
            },
        );
        assert_eq!(current_streak(&h, &e, TODAY), 2); // today pending at 6/8
        e.insert(
            TODAY,
            Entry::Value {
                value: 8,
                target: 8,
            },
        );
        assert_eq!(current_streak(&h, &e, TODAY), 3);
    }
}
