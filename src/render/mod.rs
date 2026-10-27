pub mod color;
pub mod heatmap;
pub mod json;
pub mod summary;

use self::color::{ColorMode, MISSED, SKIP, Style, paint, ramp};
use crate::day::{DayState, level};
use crate::model::{Entry, Hue};

/// What a single day looks like, independent of color mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Blank,
    Missed,
    Level(u8),
    Skip,
}

impl Cell {
    pub fn of(state: DayState, entry: Option<&Entry>) -> Self {
        match state {
            DayState::Future | DayState::BeforeStart | DayState::Paused => Cell::Blank,
            DayState::Skip => Cell::Skip,
            DayState::Missed => Cell::Missed,
            DayState::Done | DayState::Partial | DayState::Pending => match level(entry) {
                0 => Cell::Missed,
                l => Cell::Level(l),
            },
        }
    }

    /// One terminal column. `today` adds bold+underline in color mode; the
    /// no-color today marker is the caller's job.
    pub fn paint(self, hue: Hue, mode: ColorMode, today: bool) -> String {
        let (glyph, mut style) = match (self, mode) {
            (Cell::Blank, _) => return " ".into(),
            (Cell::Missed, _) => ("□", Style::fg(MISSED)),
            (Cell::Skip, _) => (
                "·",
                Style {
                    dim: true,
                    ..Style::fg(SKIP)
                },
            ),
            (Cell::Level(l), ColorMode::None) => {
                (["░", "▒", "▓", "█"][usize::from(l - 1)], Style::default())
            }
            (Cell::Level(l), _) => ("■", Style::fg(ramp(hue)[usize::from(l - 1)])),
        };
        if today {
            style.bold = true;
            style.underline = true;
        }
        paint(mode, glyph, style)
    }
}
