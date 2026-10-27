use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use jiff::ToSpan;
use jiff::civil::Date;

use crate::model::Hue;

#[derive(Parser, Debug)]
#[command(name = "tp", version, about = "Terminal habit tracker")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Today's summary
    List {
        /// Include archived habits
        #[arg(long)]
        all: bool,
        #[arg(long)]
        json: bool,
    },
    /// Heatmap and stats for a habit (summary without one)
    Show {
        habit: Option<String>,
        /// Number of weeks to display
        #[arg(long, value_parser = clap::value_parser!(u16).range(1..=520))]
        weeks: Option<u16>,
        #[arg(long)]
        json: bool,
    },
    /// Create a habit (a count habit when --target is given)
    Add {
        id: String,
        /// Display name
        #[arg(long)]
        name: Option<String>,
        /// Daily target; makes this a count habit
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
        target: Option<u32>,
        /// Unit label for count habits
        #[arg(long, requires = "target")]
        unit: Option<String>,
        #[arg(long, value_enum, default_value_t = Hue::Green)]
        color: Hue,
        /// Start date
        #[arg(long, value_parser = parse_date_spec, allow_hyphen_values = true)]
        since: Option<DateSpec>,
    },
    /// Log a day (count habits: add N, default 1)
    Done {
        habit: String,
        #[arg(conflicts_with_all = ["set", "full"])]
        n: Option<u32>,
        /// Replace the day's value (0 clears it)
        #[arg(long, conflicts_with = "full")]
        set: Option<u32>,
        /// Set the value to the day's target
        #[arg(long)]
        full: bool,
        #[command(flatten)]
        date: DateArg,
    },
    /// Mark a day as skipped
    Skip {
        habit: String,
        #[command(flatten)]
        date: DateArg,
    },
    /// Remove a day's entry
    Undo {
        habit: String,
        #[command(flatten)]
        date: DateArg,
    },
    /// Change a count habit's target from today on
    Target {
        habit: String,
        #[arg(value_parser = clap::value_parser!(u32).range(1..))]
        n: u32,
    },
    /// Rename a habit, keeping its history
    Rename {
        habit: String,
        new_id: String,
        #[arg(long)]
        name: Option<String>,
    },
    /// Pause a habit from today
    Archive { habit: String },
    /// Resume an archived habit from today
    Unarchive { habit: String },
    /// Delete a habit and all its entries
    Delete {
        habit: String,
        /// Don't ask for confirmation
        #[arg(long)]
        force: bool,
    },
    /// Update tp to the latest release
    Update,
}

#[derive(clap::Args, Debug)]
pub struct DateArg {
    /// YYYY-MM-DD, today, yesterday or -N (N days ago)
    #[arg(short = 'd', long = "date", value_parser = parse_date_spec, allow_hyphen_values = true)]
    pub date: Option<DateSpec>,
}

impl DateArg {
    pub fn resolve(&self, today: Date) -> Result<Date> {
        self.date.map_or(Ok(today), |d| d.resolve(today))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateSpec {
    On(Date),
    DaysAgo(u32),
}

impl DateSpec {
    pub fn resolve(self, today: Date) -> Result<Date> {
        match self {
            DateSpec::On(d) => Ok(d),
            DateSpec::DaysAgo(n) => today
                .checked_sub(i64::from(n).days())
                .map_err(|_| anyhow!("date out of range")),
        }
    }
}

pub fn parse_date_spec(s: &str) -> Result<DateSpec, String> {
    let err = || format!("invalid date '{s}' (use YYYY-MM-DD, today, yesterday or -N)");
    match s {
        "today" => return Ok(DateSpec::DaysAgo(0)),
        "yesterday" => return Ok(DateSpec::DaysAgo(1)),
        _ => {}
    }
    if let Some(n) = s.strip_prefix('-') {
        if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
            return Err(err());
        }
        return n.parse().map(DateSpec::DaysAgo).map_err(|_| err());
    }
    let b = s.as_bytes();
    let shape = b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
    if !shape {
        return Err(err());
    }
    s.parse()
        .map(DateSpec::On)
        .map_err(|e| format!("invalid date '{s}': {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use jiff::civil::date;

    #[test]
    fn date_specs() {
        let today = date(2026, 10, 27);
        for (s, want) in [
            ("today", Some(today)),
            ("yesterday", Some(date(2026, 10, 26))),
            ("-2", Some(date(2026, 10, 25))),
            ("-0", Some(today)),
            ("2026-10-01", Some(date(2026, 10, 1))),
            ("2026-02-30", None),
            ("2026-1-01", None),
            ("-x", None),
            ("-", None),
            ("tomorrow", None),
        ] {
            let got = parse_date_spec(s).ok().map(|d| d.resolve(today).unwrap());
            assert_eq!(got, want, "{s}");
        }
    }

    #[test]
    fn cli_shapes() {
        Cli::command().debug_assert();
        let parse = |args: &[&str]| Cli::try_parse_from([&["tp"], args].concat());
        let cli = parse(&["done", "water", "3", "-d", "-2"]).unwrap();
        match cli.command {
            Some(Command::Done {
                n: Some(3),
                date:
                    DateArg {
                        date: Some(DateSpec::DaysAgo(2)),
                    },
                ..
            }) => {}
            other => panic!("{other:?}"),
        }
        assert!(parse(&["done", "water", "--date=-1"]).is_ok());
        assert!(parse(&["done", "water", "3", "--set", "4"]).is_err());
        assert!(parse(&["done", "water", "--set", "4", "--full"]).is_err());
        assert!(parse(&["add", "water", "--unit", "glasses"]).is_err());
        assert!(parse(&["add", "water", "--target", "0"]).is_err());
        assert!(parse(&["add", "water", "--color", "teal"]).is_err());
        assert!(parse(&[]).unwrap().command.is_none());
    }
}
