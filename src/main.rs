mod cli;
mod clock;
mod config;
mod day;
mod model;
mod ops;
mod render;
mod resolve;
mod stats;
mod store;

use std::io::{BufRead, IsTerminal, Write};
use std::process::ExitCode;

use anyhow::{Result, bail};
use clap::Parser;

use cli::{Cli, Command};
use ops::{DoneMode, NewHabit, Report};
use render::color::ColorMode;
use resolve::resolve;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    let config = match config::path() {
        Some(p) => config::load(&p)?,
        None => config::Config::default(),
    };
    let today = clock::today(&config)?;
    let path = store::path()?;
    let mut data = store::load(&path)?;

    let stdout_tty = std::io::stdout().is_terminal();
    let mode = ColorMode::detect(
        stdout_tty,
        std::env::var("NO_COLOR").ok().as_deref(),
        std::env::var("COLORTERM").ok().as_deref(),
    );
    let width = terminal_size::terminal_size().map_or(80, |(w, _)| w.0);

    let command = cli.command.unwrap_or(Command::List { all: false, json: false });
    let report = match command {
        Command::List { all, json } => return list(&data, today, all, json, width, mode),
        Command::Show { habit: None, json, .. } => return list(&data, today, false, json, width, mode),
        Command::Show { habit: Some(arg), weeks, json } => {
            let h = &data.habits[resolve(&data, &arg)?];
            let entries = data.entries_of(&h.id);
            let weeks = weeks.map_or_else(|| render::heatmap::weeks_for_width(width), usize::from);
            if json {
                let first = render::heatmap::first_monday(today, weeks);
                print_json(&render::json::show(h, entries, today, first));
            } else {
                print!("{}", render::heatmap::render(h, entries, today, weeks, mode));
            }
            return Ok(());
        }
        Command::Add { id, name, target, unit, color, since } => {
            let since = since.map(|s| s.resolve(today)).transpose()?;
            ops::add(&mut data, NewHabit { id, name, target, unit, color, since }, today)?
        }
        Command::Done { habit, n, set, full, date } => {
            let idx = resolve(&data, &habit)?;
            let mode = match (n, set, full) {
                (_, Some(v), _) => DoneMode::Set(v),
                (_, _, true) => DoneMode::Full,
                (n, ..) => DoneMode::Add(n),
            };
            ops::done(&mut data, idx, mode, date.resolve(today)?, today, &config)?
        }
        Command::Skip { habit, date } => {
            let idx = resolve(&data, &habit)?;
            ops::skip(&mut data, idx, date.resolve(today)?, today, &config)?
        }
        Command::Undo { habit, date } => {
            let idx = resolve(&data, &habit)?;
            ops::undo(&mut data, idx, date.resolve(today)?, today, &config)?
        }
        Command::Target { habit, n } => {
            let idx = resolve(&data, &habit)?;
            ops::target(&mut data, idx, n, today)?
        }
        Command::Rename { habit, new_id, name } => {
            let idx = resolve(&data, &habit)?;
            ops::rename(&mut data, idx, &new_id, name)?
        }
        Command::Archive { habit } => {
            let idx = resolve(&data, &habit)?;
            ops::archive(&mut data, idx, today)?
        }
        Command::Unarchive { habit } => {
            let idx = resolve(&data, &habit)?;
            ops::unarchive(&mut data, idx, today)?
        }
        Command::Delete { habit, force } => {
            let idx = resolve(&data, &habit)?;
            if !force && !confirm_delete(&data, idx)? {
                println!("aborted");
                return Ok(());
            }
            ops::delete(&mut data, idx)
        }
    };
    store::save(&path, &data)?;
    print_report(&report);
    Ok(())
}

fn list(data: &model::Data, today: jiff::civil::Date, all: bool, json: bool, width: u16, mode: ColorMode) -> Result<()> {
    if json {
        print_json(&render::json::list(data, today, all));
    } else {
        print!("{}", render::summary::render(data, today, all, width, mode));
    }
    Ok(())
}

fn print_json(v: &serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(v).expect("json serializes"));
}

fn print_report(r: &Report) {
    for w in &r.warnings {
        eprintln!("warning: {w}");
    }
    for n in &r.notes {
        eprintln!("note: {n}");
    }
    println!("{}", r.message);
}

/// Prompts on a TTY; otherwise `--force` is required.
fn confirm_delete(data: &model::Data, idx: usize) -> Result<bool> {
    let id = &data.habits[idx].id;
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        bail!("refusing to delete '{id}' without confirmation; use --force");
    }
    let n = data.entries_of(id).len();
    eprint!("Delete '{id}' and all {}? [y/N] ", ops::plural(n, "entry", "entries"));
    std::io::stderr().flush()?;
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    Ok(matches!(line.trim(), "y" | "Y" | "yes"))
}
