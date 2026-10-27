use jiff::ToSpan;
use jiff::civil::Date;

use super::Cell;
use super::color::{ColorMode, Style, paint, ramp};
use crate::day::{DayState, day_state};
use crate::model::{Data, Entry, Habit, Kind};
use crate::stats::current_streak;

/// Today's status column: `✓`, `·`, `skip`, `-` (paused) or `v/t`.
pub fn status(habit: &Habit, entry: Option<&Entry>, today: Date) -> (String, DayState) {
    let state = day_state(habit, entry, today, today);
    let text = match (state, habit.kind) {
        (DayState::Skip, _) => "skip".to_string(),
        (DayState::Paused | DayState::BeforeStart, _) => "-".to_string(),
        (DayState::Done, Kind::Bool) => "✓".to_string(),
        (_, Kind::Bool) => "·".to_string(),
        (_, Kind::Count) => match entry {
            Some(&Entry::Value { value, target }) => format!("{value}/{target}"),
            _ => format!("0/{}", habit.target_on(today).unwrap_or(0)),
        },
    };
    (text, state)
}

pub fn strip_len(width: u16, fixed: usize) -> usize {
    usize::from(width).saturating_sub(fixed).clamp(7, 30)
}

pub fn render(data: &Data, today: Date, all: bool, width: u16, mode: ColorMode) -> String {
    let habits: Vec<&Habit> = data
        .habits
        .iter()
        .filter(|h| all || !h.is_archived())
        .collect();
    if habits.is_empty() {
        return "no habits yet — try: tp add read\n".into();
    }
    struct Row<'a> {
        habit: &'a Habit,
        status: String,
        state: DayState,
        streak: String,
    }
    let rows: Vec<Row> = habits
        .iter()
        .map(|&h| {
            let entries = data.entries_of(&h.id);
            let (status, state) = status(h, entries.get(&today), today);
            let streak = format!("streak {}", current_streak(h, entries, today));
            Row {
                habit: h,
                status,
                state,
                streak,
            }
        })
        .collect();
    let id_w = rows
        .iter()
        .map(|r| r.habit.id.chars().count())
        .max()
        .unwrap_or(0);
    let st_w = rows
        .iter()
        .map(|r| r.status.chars().count())
        .max()
        .unwrap_or(0)
        .max(4);
    let streak_w = rows.iter().map(|r| r.streak.len()).max().unwrap_or(0);
    let n = strip_len(width, 2 + id_w + 2 + st_w + 2 + 2 + streak_w + 1);

    let mut out = String::new();
    for r in rows {
        let h = r.habit;
        let entries = data.entries_of(&h.id);
        let archived = h.is_archived();
        let id = format!("{:<id_w$}", h.id);
        let st = format!("{:<st_w$}", r.status);
        let (id, st) = if archived {
            (
                paint(mode, &id, Style::dim()),
                paint(mode, &st, Style::dim()),
            )
        } else if r.state == DayState::Done {
            let s = Style {
                bold: true,
                ..Style::fg(ramp(h.color)[3])
            };
            (id, paint(mode, &st, s))
        } else {
            (id, st)
        };
        let mut strip = String::new();
        for i in (0..n as i64).rev() {
            let date = today.saturating_sub(i.days());
            let entry = entries.get(&date);
            let cell = Cell::of(day_state(h, entry, date, today), entry);
            strip += &cell.paint(h.color, mode, date == today && cell != Cell::Blank);
        }
        out += &format!("  {id}  {st}  {strip}  {}", r.streak);
        if archived {
            out += &format!("  {}", paint(mode, "archived", Style::dim()));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Pause;
    use crate::model::tests::{bool_habit, count_habit};
    use jiff::civil::date;

    const TODAY: Date = date(2026, 10, 27);

    #[test]
    fn strip_bounds() {
        assert_eq!(strip_len(10, 30), 7);
        assert_eq!(strip_len(50, 30), 20);
        assert_eq!(strip_len(200, 30), 30);
    }

    #[test]
    fn rows() {
        let mut d = Data::default();
        d.habits.push(bool_habit("read", date(2026, 10, 1)));
        d.habits.push(count_habit("water", date(2026, 10, 1), 8));
        let mut gym = bool_habit("gym", date(2026, 10, 1));
        gym.pauses.push(Pause {
            from: date(2026, 10, 20),
            to: None,
        });
        d.habits.push(gym);
        d.entries
            .entry("read".into())
            .or_default()
            .insert(TODAY, Entry::Done);
        d.entries
            .entry("read".into())
            .or_default()
            .insert(date(2026, 10, 26), Entry::Skip);
        d.entries.entry("water".into()).or_default().insert(
            TODAY,
            Entry::Value {
                value: 4,
                target: 8,
            },
        );
        let got = render(&d, TODAY, false, 0, ColorMode::None);
        assert_eq!(
            got,
            "  read   ✓     □□□□□·█  streak 1\n  water  4/8   □□□□□□▒  streak 0\n"
        );
        let all = render(&d, TODAY, true, 0, ColorMode::None);
        assert!(
            all.lines().nth(2).unwrap().ends_with("streak 0  archived"),
            "{all}"
        );
        assert_eq!(
            render(&Data::default(), TODAY, false, 80, ColorMode::None),
            "no habits yet — try: tp add read\n"
        );
    }
}
