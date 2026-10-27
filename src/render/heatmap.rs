use jiff::ToSpan;
use jiff::civil::Date;

use super::Cell;
use super::color::ColorMode;
use crate::day::day_state;
use crate::model::{Entries, Habit, Kind};
use crate::stats::{current_streak, longest_streak};

/// Weekday label column ("Mon ") plus the gap before the first cell.
const PREFIX: usize = 5;
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

pub fn weeks_for_width(width: u16) -> usize {
    ((i32::from(width) - 6) / 2).clamp(4, 53) as usize
}

/// Weeks from the one holding the 1st of the start month through today's
/// week, capped by what fits the terminal.
pub fn default_weeks(start: Date, today: Date, width: u16) -> usize {
    let from = first_monday(start.first_of_month(), 1);
    let to = first_monday(today, 1);
    let span = ((to - from).get_days() / 7 + 1).max(1) as usize;
    span.min(weeks_for_width(width))
}

/// Monday of the first displayed week; the last column holds `today`.
pub fn first_monday(today: Date, weeks: usize) -> Date {
    let offset = i64::from(today.weekday().to_monday_zero_offset()) + 7 * (weeks as i64 - 1);
    today.saturating_sub(offset.days())
}

/// `(column, label)` for each month whose 1st falls in a column, dropping
/// labels that would touch the previous one.
pub fn month_labels(first_monday: Date, weeks: usize) -> Vec<(usize, &'static str)> {
    let mut out: Vec<(usize, &'static str)> = Vec::new();
    for col in 0..weeks {
        let monday = first_monday.saturating_add((7 * col as i64).days());
        let Some(first) = (0..7)
            .map(|i| monday.saturating_add(i.days()))
            .find(|d| d.day() == 1)
        else {
            continue;
        };
        if out.last().is_some_and(|&(prev, _)| 2 * col < 2 * prev + 4) {
            continue;
        }
        out.push((col, MONTHS[first.month() as usize - 1]));
    }
    out
}

pub fn render(
    habit: &Habit,
    entries: &Entries,
    today: Date,
    weeks: usize,
    mode: ColorMode,
) -> String {
    let start = first_monday(today, weeks);
    let mut out = habit.id.clone();
    if habit.name != habit.id {
        out += &format!(" — {}", habit.name);
    }
    if habit.is_archived() {
        out += " (archived)";
    }
    out.push('\n');

    let mut labels = vec![' '; PREFIX + 2 * weeks + 3];
    for (col, name) in month_labels(start, weeks) {
        for (i, ch) in name.chars().enumerate() {
            labels[PREFIX + 2 * col + i] = ch;
        }
    }
    out += labels.iter().collect::<String>().trim_end();
    out.push('\n');

    for row in 0..7 {
        let mut line = format!(
            "{:<4}",
            ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"][row]
        );
        for col in 0..weeks {
            let date = start.saturating_add((7 * col as i64 + row as i64).days());
            let entry = entries.get(&date);
            let cell = Cell::of(day_state(habit, entry, date, today), entry);
            let is_today = date == today && cell != Cell::Blank;
            line.push(if is_today && mode == ColorMode::None {
                '>'
            } else {
                ' '
            });
            line += &cell.paint(habit.color, mode, is_today);
        }
        out += line.trim_end();
        out.push('\n');
    }

    let pad = " ".repeat(PREFIX);
    let mut legend = format!("\n{pad}less");
    for cell in [
        Cell::Missed,
        Cell::Level(1),
        Cell::Level(2),
        Cell::Level(3),
        Cell::Level(4),
    ] {
        legend += &format!(" {}", cell.paint(habit.color, mode, false));
    }
    legend += &format!(
        " more   {} skip",
        Cell::Skip.paint(habit.color, mode, false)
    );
    if habit.kind == Kind::Count
        && let Some(t) = habit.target_on(today)
    {
        legend += &format!("   target {t}");
        if let Some(u) = &habit.unit {
            legend += &format!(" {u}");
        }
    }
    out += &legend;
    out += &format!(
        "\n{pad}streak {} · longest {}\n",
        current_streak(habit, entries, today),
        longest_streak(habit, entries, today)
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Entry;
    use crate::model::tests::{bool_habit, count_habit};
    use jiff::civil::date;

    #[test]
    fn weeks_from_width() {
        for (w, want) in [
            (0, 4),
            (10, 4),
            (14, 4),
            (16, 5),
            (80, 37),
            (81, 37),
            (112, 53),
            (300, 53),
        ] {
            assert_eq!(weeks_for_width(w), want, "{w}");
        }
    }

    #[test]
    fn weeks_from_start_month() {
        for (start, today, width, want) in [
            (date(2026, 10, 27), date(2026, 10, 27), 200, 5),
            (date(2026, 10, 1), date(2026, 10, 1), 200, 1),
            (date(2026, 10, 5), date(2026, 10, 27), 200, 5),
            (date(2025, 1, 10), date(2026, 10, 27), 80, 37),
            (date(2026, 10, 27), date(2026, 10, 27), 10, 4),
            (date(2026, 10, 27), date(2026, 10, 27), 14, 4),
            (date(2026, 10, 20), date(2026, 10, 21), 10, 4),
        ] {
            assert_eq!(
                default_weeks(start, today, width),
                want,
                "{start} {today} {width}"
            );
        }
        assert_eq!(default_weeks(date(2026, 10, 1), date(2026, 10, 1), 10), 1);
    }

    #[test]
    fn starts_at_start_month() {
        let today = date(2026, 10, 27);
        let h = bool_habit("read", today);
        let weeks = default_weeks(h.start, today, 200);
        assert_eq!(first_monday(today, weeks), date(2026, 9, 28));
        let got = render(&h, &Entries::new(), today, weeks, ColorMode::None);
        let lines: Vec<&str> = got.lines().collect();
        assert_eq!(lines[1], "     Oct     Nov");
        assert_eq!(lines[3], "Tue         >□");
    }

    #[test]
    fn first_column() {
        // 2026-10-27 is a Tuesday.
        assert_eq!(first_monday(date(2026, 10, 27), 1), date(2026, 10, 26));
        assert_eq!(first_monday(date(2026, 10, 27), 4), date(2026, 10, 5));
        assert_eq!(first_monday(date(2026, 11, 1), 1), date(2026, 10, 26));
    }

    #[test]
    fn labels() {
        // Columns start 2026-08-31, 09-07, …; Sep 1 is in column 0.
        let start = date(2026, 8, 31);
        assert_eq!(
            month_labels(start, 10),
            vec![(0, "Sep"), (4, "Oct"), (8, "Nov")]
        );
        // Jun 1 2026 is a Monday: col 0 = Jun 1 week; Jul 1 in col 4.
        assert_eq!(
            month_labels(date(2026, 6, 1), 6),
            vec![(0, "Jun"), (4, "Jul")]
        );
        // Adjacent columns: 2 apart in text, label is 3 wide → drop.
        assert_eq!(month_labels(date(2026, 1, 26), 2), vec![(0, "Feb")]);
        // Feb 2027 starts Monday Feb 1; Mar 1 2027 is 4 columns later → kept (gap of 5).
        assert_eq!(
            month_labels(date(2027, 2, 1), 5),
            vec![(0, "Feb"), (4, "Mar")]
        );
        assert!(month_labels(date(2026, 10, 5), 3).is_empty());
    }

    #[test]
    fn no_color_grid() {
        let today = date(2026, 10, 28); // Wednesday
        let mut h = count_habit("water", date(2026, 10, 13), 8);
        h.unit = Some("glasses".into());
        let mut e = Entries::new();
        e.insert(
            date(2026, 10, 13),
            Entry::Value {
                value: 8,
                target: 8,
            },
        );
        e.insert(date(2026, 10, 14), Entry::Skip);
        e.insert(
            date(2026, 10, 20),
            Entry::Value {
                value: 3,
                target: 8,
            },
        );
        e.insert(
            date(2026, 10, 27),
            Entry::Value {
                value: 12,
                target: 8,
            },
        );
        let got = render(&h, &e, today, 4, ColorMode::None);
        let want = "\
water
           Nov
Mon      □ □
Tue    ▓ ░ █
Wed    · □>□
Thu    □ □
Fri    □ □
Sat    □ □
Sun    □ □

     less □ ░ ▒ ▓ █ more   · skip   target 8 glasses
     streak 1 · longest 1
";
        assert_eq!(got, want);
    }

    #[test]
    fn today_marker_after_label() {
        let today = date(2026, 10, 26); // Monday, first row of last column
        let h = bool_habit("read", date(2026, 10, 26));
        let got = render(&h, &Entries::new(), today, 1, ColorMode::None);
        assert_eq!(got.lines().nth(2), Some("Mon >□"));
        let colored = render(&h, &Entries::new(), today, 1, ColorMode::TrueColor);
        assert!(
            colored
                .lines()
                .nth(2)
                .unwrap()
                .starts_with("Mon  \x1b[1;4;38;2;")
        );
    }
}
