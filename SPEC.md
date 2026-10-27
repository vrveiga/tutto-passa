# tutto-passa — Terminal Habit Tracker

`tp` is a CLI habit tracker. You create habits, check in each day (or backfill), and view a GitHub-style contribution heatmap per habit.

- Crate name: `tutto-passa`. Binary name: `tp` (`[[bin]] name = "tp"`).
- Interaction model: pure CLI subcommands. No TUI.
- Single user, single machine.

---

## 1. Concepts

### 1.1 Habit kinds

| Kind    | Created with                 | Day value       | "Done" when            |
|---------|------------------------------|-----------------|------------------------|
| `bool`  | `tp add read`                | done / not done | logged as done         |
| `count` | `tp add water --target 8`    | integer ≥ 0     | `value >= target`      |

- A habit's kind is fixed at creation and can't be changed later.
- Count habits **must** have a target, which is a positive integer. They can also have an optional `unit` label (display only, e.g. `glasses`).
- Values are integers only (`u32`).
- Only build-up habits are supported. There are no "limit" habits where less is better. An avoidance habit is modelled as a bool (e.g. `no-smoking`: done).

### 1.2 Frequency

Every habit is **daily**. Exceptions are handled with **skip** (§1.4) or by **archiving** (§1.6).

### 1.3 Days and "today"

- All stored dates are **civil dates** (`YYYY-MM-DD`). No timestamps or timezones are stored, so travelling or changing settings never rewrites history.
- The effective "today" is the local wall-clock time minus `day_starts_at` hours (from config, default `0`). For example, with `day_starts_at = 4`, 01:30 on Oct 28 counts as Oct 27.
- The hidden `TP_TODAY=YYYY-MM-DD` env var overrides "today". All date logic receives `today: jiff::civil::Date` as a parameter and never reads the clock directly.

### 1.4 Day states

For a given habit and date, the state is derived in this priority order:

| State        | Condition                                                                 | Heatmap      | Streak effect            |
|--------------|---------------------------------------------------------------------------|--------------|--------------------------|
| `Future`     | date > today                                                              | blank        | n/a                      |
| `BeforeStart`| date < habit.start                                                        | blank        | n/a                      |
| `Paused`     | date falls inside an archive period                                       | blank        | **breaks** (resets)      |
| `Skip`       | entry is `skip`                                                           | dim `·`      | transparent              |
| `Done`       | bool entry done, or count `value >= entry.target`                         | level 3 / 4  | extends                  |
| `Partial`    | count entry with `0 < value < target`                                     | level 1 / 2  | breaks (unless today)    |
| `Pending`    | date == today and not Done/Skip (includes today-partial)                  | per value + today marker | doesn't break yet |
| `Missed`     | no entry (or value 0), date in [start, today)                             | level 0 `□`  | breaks                   |

Skip and a value are mutually exclusive. Skipping a day replaces any value, and logging on a skipped day replaces the skip.

### 1.5 Targets over time

- Each count entry stores the **target in effect for that date** (snapshot), so raising the bar later doesn't re-grade history.
- The habit keeps a target history: `targets: [{from: date, value}]`, sorted by `from`. The target for date D is the last item with `from <= D`.
- `tp target <habit> N` appends `{from: today, value: N}` (or replaces it if one already exists for today) and re-stamps today's entry if there is one. Older entries are never touched.
- Backfilled entries get the target from the history for that date.

### 1.6 Habit lifecycle

- **Start date**: defaults to the creation day, or set explicitly with `tp add … --since DATE`. If an entry is logged before the start date, the start moves back to that date and a notice is printed: `note: read start moved to 2026-10-01`. Undo never moves the start forward.
- **Archive**: `tp archive <habit>` opens a pause period starting today: `pauses: [{from: today, to: null}]`. Archived habits are hidden from the default list and can't be logged to.
- **Unarchive**: `tp unarchive <habit>` closes the open pause with `to: today` (exclusive), so the habit is active again from today. Paused days render blank, and the **streak resets** across the pause.
- Logging into an archived habit, or backfilling a date inside any pause period, is **rejected**: `error: gym is archived since 2026-09-01; run 'tp unarchive gym'` (or `error: 2026-09-03 is inside a pause of gym (2026-09-01..2026-09-10)`).
- **Rename**: `tp rename <habit> <new-id> [--name "…"]` keeps all history.
- **Delete**: `tp delete <habit>` prompts `Delete 'gym' and all 143 entries? [y/N]`. When stdin isn't a TTY, `--force` is required, otherwise it errors.

---

## 2. Command-line interface

### 2.1 Habit identification

- Each habit has an `id` slug matching `^[a-z0-9][a-z0-9-]{0,31}$`, plus an optional free-text display `name` (defaults to the id).
- Habit arguments resolve as follows: **exact id match** wins; otherwise a **unique prefix** match; otherwise an error.
  - Ambiguous: `error: 'g' matches gym, guitar`.
  - None: `error: no habit 'gmy' (did you mean gym?)`. The suggestion uses edit distance ≤ 2.
- Archived habits can be resolved (for `unarchive`, `show`, `rename`, `delete`).

### 2.2 Date arguments (`--date` / `-d`)

The following forms are accepted:
- `YYYY-MM-DD`
- `today`, `yesterday`
- `-N`, meaning N days before today (e.g. `-d -2`). Needs clap `allow_negative_numbers`/`allow_hyphen_values` on this arg.

Rules:
- **Future dates are rejected**: `error: 2026-10-28 is in the future`.
- Dates more than `backfill_warn_days` (default 7) before today trigger a warning on stderr. The write still happens: `warning: editing 2026-10-01 (26 days ago)`.

### 2.3 Commands

```
tp                                   # same as `tp list`
tp list [--all] [--json]             # today summary (--all includes archived)
tp show [<habit>] [--weeks N] [--json]
                                     # with habit: full heatmap + stats
                                     # without habit: same as `tp list`
tp add <id> [--name TEXT] [--target N [--unit TEXT]] [--color HUE] [--since DATE]
tp done <habit> [N] [--set N | --full] [-d DATE]
tp skip <habit> [-d DATE]
tp undo <habit> [-d DATE]
tp target <habit> <N>
tp rename <habit> <new-id> [--name TEXT]
tp archive <habit>
tp unarchive <habit>
tp delete <habit> [--force]
```

#### `tp done` semantics

| Habit  | Invocation                  | Effect                                                          |
|--------|-----------------------------|-----------------------------------------------------------------|
| bool   | `tp done read`              | mark done (idempotent; prints `already done` if it was)         |
| bool   | `tp done read 3` / `--set` / `--full` | error: `read is a yes/no habit`                       |
| count  | `tp done water`             | add 1                                                           |
| count  | `tp done water 3`           | add 3                                                           |
| count  | `tp done water --set 6`     | replace the day's value with 6; `--set 0` clears the entry (= undo) |
| count  | `tp done water --full`      | set the value to that day's target                              |

- `N` and `--set`/`--full` are mutually exclusive (enforced by clap).
- Any `done` on a skipped day replaces the skip.
- Values saturate at `u32::MAX` (unrealistic, but don't panic).

**Output** (one line, on stdout):
```
✓ read — 2026-10-27 · streak 5
  water — 2026-10-27 · 5/8 glasses · streak 3
✓ water — 2026-10-27 · 8/8 glasses · streak 4
```

#### `tp skip`
Marks the date as skipped, clearing any value. Output: `· gym skipped — 2026-10-26`.

#### `tp undo`
Removes the entry for that date, so the day becomes Missed or Pending. If there was no entry, prints `nothing to undo` and exits 0.

### 2.4 Exit codes

- `0` success
- `1` runtime/domain error (unknown habit, future date, corrupt data, …)
- `2` usage error (clap)

Errors go to stderr, prefixed with `error:`. Warnings and notices go to stderr, prefixed with `warning:` / `note:`.

---

## 3. Output

### 3.1 Colors and terminal detection

- Color is enabled only when stdout is a TTY (`std::io::IsTerminal`) **and** `NO_COLOR` is unset or empty.
- Color depth: truecolor if `COLORTERM` is `truecolor` or `24bit`, otherwise the nearest xterm-256 color.
- Every habit has a `color` hue (default `green`). Supported hues: `green`, `blue`, `purple`, `orange`, `red`, `yellow`, `pink`, `cyan`. Each hue has a fixed 4-step ramp (levels 1–4), hard-coded in a table. The green ramp follows GitHub's dark palette: `#0e4429 #006d32 #26a641 #39d353`.
- Missed (level 0) is gray (`#2d333b`-ish). Skip is dim gray.

### 3.2 Levels

| Level | Bool       | Count (ratio = value / entry.target) |
|-------|------------|--------------------------------------|
| 0     | missed     | 0 / no entry                         |
| 1     | —          | 0 < ratio < 0.5                      |
| 2     | —          | 0.5 ≤ ratio < 1.0                    |
| 3     | —          | 1.0 ≤ ratio < 1.5                    |
| 4     | done       | ratio ≥ 1.5                          |

Bool done uses level 4 so that a done day is the full, satisfying color.

### 3.3 Glyphs

| Cell               | Color mode                     | No-color mode |
|--------------------|--------------------------------|---------------|
| blank              | ` `                            | ` `           |
| missed (L0)        | `□` gray                       | `□`           |
| L1–L4              | `■` in ramp color              | `░ ▒ ▓ █`     |
| skip               | `·` dim                        | `·`           |
| today              | cell rendered **bold + underline** | `>` in the gap column immediately before the cell |

Every cell is **2 columns wide**: the glyph followed by a gap space. The today marker in no-color mode uses the gap that comes before today's cell, which is the trailing space of the previous column or the space after the weekday label. This keeps the grid aligned.

> Note: `■`, `□`, and `·` are East Asian *ambiguous-width* characters. They are assumed to be 1 column wide. This is documented as a known limitation for CJK-width terminal settings.

### 3.4 Heatmap (`tp show <habit>`)

```
read — Read 20 pages                         (name line; id — name, if they differ)
      Jun     Jul      Aug     Sep      Oct
Mon   ■ ■ ■ □ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■
Tue   ■ ■ · ■ ■ ■ ■ ■ □ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■
Wed   ■ ■ ■ ■ ■ □ ■ ■ ■ ■ ■ ■ · ■ ■ ■ ■ ■ ■ ■
Thu   ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■
Fri   ■ □ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■
Sat   ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■
Sun   ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■ ■

      less □ ■ ■ ■ ■ more   · skip
      streak 12 · longest 31
```

Layout rules:
- **Rows are weekdays, Monday to Sunday.** Weeks start on Monday (ISO). Every row is labeled (Mon–Sun); the label column is 4 characters wide plus 1 gap (`"Mon "` + gap).
- **Columns are weeks.** The last column is the week containing today. Days after today in that week are blank.
- **Number of weeks**: `--weeks N` if given; otherwise `clamp((term_width − 6) / 2, 4, 53)`. Terminal width comes from `terminal_size`, falling back to 80 when it isn't a TTY.
- If the terminal is too narrow for 4 weeks, render 4 weeks anyway and let the terminal wrap. There is no vertical layout.
- **Month labels**: a month's 3-letter label is placed above the first column whose week contains the 1st of that month. A label is dropped if it would overlap the previous label's text (needs ≥ 1 space between labels).
- The legend shows the habit's ramp (L0–L4). Count habits add the unit and current target: `target 8 glasses`.
- **Stats line**: current streak and longest streak.

### 3.5 Summary (`tp`, `tp list`, `tp show` with no habit)

```
  read    ✓      ■■■■□■■■■■■■■■  streak 9
  gym     ·      ■□■■·■■□■■■□■□  streak 0
  water   4/8    ▒▓■■▒■▓■■░■■▓▒  streak 3
  run     skip   ■■■■■■■■■■■■■·  streak 14
```

- One row per **active** habit, in creation order. `--all` adds archived habits, dimmed and with an `archived` tag.
- Columns: id (padded) · today status (`✓`, `·` pending, `skip`, `v/t` for count habits) · strip of the last N days (1 column per day, no gaps, oldest to newest, today last) · current streak.
- Strip length: `clamp(term_width − fixed_columns, 7, 30)`.
- When there are no habits: `no habits yet — try: tp add read`.

### 3.6 JSON output (`--json`)

`tp list --json`:
```json
{
  "today": "2026-10-27",
  "habits": [
    {
      "id": "water", "name": "water", "kind": "count", "unit": "glasses",
      "target": 8, "archived": false,
      "today": { "state": "pending", "value": 4, "target": 8 },
      "streak": { "current": 3, "longest": 21 }
    }
  ]
}
```

`tp show water --json` contains the same habit object plus `"days": [{"date": "...", "state": "done|partial|missed|skip|pending|paused", "value": n, "target": n, "level": 0..4}]`. It covers the displayed window, includes in-window days from `start` onward, and omits blank `before_start`/`future` days.

JSON output never contains ANSI codes.

---

## 4. Stats

All stats are pure functions of `(habit, entries, today)`.

### 4.1 Current streak

Walk backwards from today:
1. If today is `Pending`, move to yesterday without breaking the streak and without counting today. If today is `Done`, count it.
2. `Done` → +1, continue. `Skip` → continue without counting it.
3. Stop at `Missed`, `Partial`, `Paused`, or `BeforeStart`.

### 4.2 Longest streak

The same rules applied over the whole history from `start` to today. This is the maximum run of `Done` days, where `Skip` days are transparent and `Missed`/`Partial`/`Paused` days break the run. Today counts only if it is `Done`.

---

## 5. Storage

### 5.1 Locations

| What   | Default path                                    | Override        |
|--------|-------------------------------------------------|-----------------|
| Data   | `$XDG_DATA_HOME/tutto-passa/habits.json` (`~/.local/share/…`) | `TP_DATA` env (path to file) |
| Config | `$XDG_CONFIG_HOME/tutto-passa/config.toml` (`~/.config/…`)    | —               |

- A missing data file is treated as empty. The file (and its parent directory) is created on the first write.
- A missing config file means all defaults are used.

### 5.2 Config (`config.toml`, hand-edited)

```toml
day_starts_at = 4         # 0..=23, default 0
backfill_warn_days = 7    # default 7
```
Unknown keys or out-of-range values produce an error that names the key.

### 5.3 Data file format

```json
{
  "version": 1,
  "habits": [
    {
      "id": "read",
      "name": "Read 20 pages",
      "kind": "bool",
      "color": "green",
      "start": "2026-10-01",
      "pauses": []
    },
    {
      "id": "water",
      "name": "water",
      "kind": "count",
      "unit": "glasses",
      "color": "blue",
      "start": "2026-10-10",
      "targets": [{ "from": "2026-10-10", "value": 6 }, { "from": "2026-10-20", "value": 8 }],
      "pauses": [{ "from": "2026-10-12", "to": "2026-10-15" }]
    }
  ],
  "entries": {
    "read":  { "2026-10-26": "done", "2026-10-27": "skip" },
    "water": { "2026-10-26": { "value": 5, "target": 8 }, "2026-10-27": "skip" }
  }
}
```

- `habits` is kept in creation order. `entries` is `BTreeMap<id, BTreeMap<Date, Entry>>`, which gives sorted, stable output.
- `Entry` is a serde enum: `"done"` | `"skip"` | `{ "value": u32, "target": u32 }`.
- Pause intervals are half-open `[from, to)`, with `to: null` meaning "still archived". At most one pause can be open.
- `targets` is present only for `count` habits. `unit` is optional.
- The file is pretty-printed with 2-space indent and a trailing newline.

### 5.4 Integrity

- **Atomic writes**: serialize to `habits.json.tmp` in the same directory → `fsync` the file → `rename` over `habits.json` → `fsync` the directory.
- **Version check**: if `version` is greater than the supported version, error out (`data file is from a newer tp`). Migration hooks are not needed in v1, but loading goes through one `load()` function so they can be added later.
- **Corrupt or invalid JSON**: error with path and serde position. **Never overwrite** a file that failed to parse.
- Validation on load: ids are unique and valid, every `entries` key has a habit, count habits have ≥ 1 target, pauses don't overlap. Violations are errors.
- No file locking and no backups (single user, single machine).

---

## 6. Implementation

### 6.1 Dependencies

- `clap` (derive) for CLI parsing
- `jiff` for civil dates and local time
- `serde`, `serde_json` for the data file
- `toml` for the config
- `dirs` for XDG paths
- `anyhow` for error propagation
- `terminal_size` for terminal width
- `strsim` (optional, can be replaced by a hand-rolled Levenshtein) for "did you mean"

Not used: SQLite, a TUI library, or a coloring crate. ANSI escapes are written directly in a small module, since only fg color, bold, underline, and dim are needed.

### 6.2 Module layout

```
src/
  main.rs        # parse args, load config + data, dispatch, map errors → exit codes
  cli.rs         # clap definitions, date-arg parsing (today/yesterday/-N/ISO)
  config.rs      # Config { day_starts_at, backfill_warn_days }, load/validate
  clock.rs       # effective_today(config) honoring TP_TODAY
  model.rs       # Habit, Kind, Entry, Pause, TargetChange, Data; validation
  store.rs       # load/save, atomic write, version check
  resolve.rs     # id → habit (exact, prefix, suggestions)
  ops.rs         # add/done/skip/undo/target/rename/archive/unarchive/delete
                 # (pure: &mut Data + args + today → Result<Outcome>)
  day.rs         # day_state(habit, entries, date, today) -> DayState; level()
  stats.rs       # current_streak, longest_streak
  render/
    color.rs     # ColorMode detection, hue ramps, ANSI helpers
    heatmap.rs   # grid layout, month labels, legend
    summary.rs   # list view
    json.rs      # --json serializers
```

`ops`, `day`, and `stats` don't do any I/O. `main` is the only place that touches the filesystem, the env, or the clock.

### 6.3 Testing

- **Injectable clock**: `TP_TODAY` env var. Internally everything takes `today` as a parameter.
- **Unit tests** (table-driven) for:
  - `day_state`: every row of §1.4, including the boundaries (start date, pause `from`/`to`, today).
  - Streaks: skips being transparent, a pause resetting the streak, today pending vs done, a partial day breaking the streak, a target raised mid-streak (old entries keep their old target).
  - Levels: ratio boundaries 0, 0.5, 1.0, 1.5.
  - `ops`: add-vs-set, `--set 0` clears, `--full`, done replacing skip, skip clearing value, backfill before start moving the start, logging into a pause rejected, future date rejected.
  - Effective today with `day_starts_at` around midnight and DST transitions.
  - Prefix resolution: exact beats prefix, ambiguity, suggestions.
  - Month-label placement and collision dropping; week-count computation from width.
  - Store: round-trip, version too new, validation failures.

---

## 7. Out of scope for v1

- TUI / interactive mode
- Weekly or N-per-week frequencies, specific-weekday schedules
- Limit habits (less is better), decimal values
- Multi-machine sync and merge, file locking, backups
- Per-day notes, CSV export, shell completions, manual reordering
- Configurable themes or glyphs
- Changing a habit's kind
