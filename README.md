# tutto-passa

> *Tutto Passa* - everything passes, your habits shouldn't.

A tiny habit tracker for your terminal. Check in daily, watch the heatmap fill up.

## Install

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/vrveiga/tutto-passa/releases/latest/download/tutto-passa-installer.sh | sh
```

Windows:

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/vrveiga/tutto-passa/releases/latest/download/tutto-passa-installer.ps1 | iex"
```

Got Rust? `cargo install --git https://github.com/vrveiga/tutto-passa`

## Use

```sh
tp add read                 # yes/no habit
tp add water --target 8     # count habit
tp done read                # check in
tp done water 3             # +3
tp done read -d yesterday   # forgot? backfill
tp skip read                # day off, streak stays alive
tp                          # today at a glance
tp show read                # heatmap + streaks
```

`tp --help` has the rest: `undo`, `target`, `rename`, `archive`, `delete`.

## Update

```sh
tp update
```

## Uninstall

```sh
rm ~/.local/bin/tp
rm -rf ~/.config/tutto-passa        # config + install receipt
rm -rf ~/.local/share/tutto-passa   # your habit history — only if you mean it
```

On macOS, data lives in `~/Library/Application Support/tutto-passa`. On Windows, look in `%APPDATA%\tutto-passa` (config, data) and `%LOCALAPPDATA%\tutto-passa` (receipt).

## Config

Optional, in `~/.config/tutto-passa/config.toml`:

```toml
day_starts_at = 4       # night owl? the day flips at 4am
backfill_warn_days = 7  # warn when editing older days
```

Your habits are plain JSON in `~/.local/share/tutto-passa/habits.json`. Set `TP_DATA` to keep them elsewhere.

## License

MIT
