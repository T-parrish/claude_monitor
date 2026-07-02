# Claude Monitor

A tiny macOS menu bar widget that shows how much of your Claude plan usage
you've burned through, as a live percentage bar:

```
▰▰▱▱▱ 34%
```

Clicking the item opens a dropdown with every limit on your plan, each with
its own bar and reset time:

```
Claude Plan Usage
▰▰▰▱▱▱▱▱▱▱   34%   Session (5h) · resets 5:30 PM
▰▱▱▱▱▱▱▱▱▱   12%   Week · all models · resets Thu 12:00 AM
─────────────────
Updated 2:41 PM
Refresh Now
─────────────────
Quit Claude Monitor
```

## How it works

The widget calls the same OAuth usage endpoint that Claude Code's `/usage`
command uses (`https://api.anthropic.com/api/oauth/usage`), authenticating
with the token Claude Code already stores in the macOS Keychain (read via
the `security` CLI). There is nothing to configure: as long as you're logged
in to Claude Code, the widget works, and Claude Code keeps the token fresh.


Metrics refresh automatically every 60 seconds (configurable via
`REFRESH_INTERVAL` in `src/main.rs`). If a fetch fails — offline, logged
out — the title shows `⚠ Claude` and the dropdown shows the error; it keeps
retrying and recovers on its own.

Note: Claude plans don't have a daily token cap. The real limits are a
rolling 5-hour session window plus weekly caps, which is what's displayed.
The menu bar title tracks the session limit.

## Requirements

- macOS
- [Claude Code](https://claude.com/claude-code) installed and logged in
  (that's where the credentials come from)
- Rust toolchain to build (`rustup`)

## Build & run

```sh
cargo build --release
./target/release/claude_monitor &
```

To start it at login, add `target/release/claude_monitor` in
System Settings → General → Login Items.

To quit, use "Quit Claude Monitor" in the dropdown.

## Adding new metrics

The app is organized around a small `MetricSource` trait so it can display
more than plan usage:

- `src/metrics.rs` — the `Metric` struct (label, percent, reset time) and
  the `MetricSource` trait: `fetch() -> Result<Vec<Metric>, String>`.
- `src/sources/` — one module per source. `plan_usage.rs` is the only one
  today (Keychain + usage API).
- `src/main.rs` — register sources in the `sources()` function at the top.
  The dropdown automatically renders a section per source; whichever metric
  is marked `emphasized` drives the menu bar title.

So a source for, say, local token counts from `~/.claude/projects` or API
spend is: implement the trait in a new file under `src/sources/`, add one
line to `sources()`. New limit kinds Anthropic adds to the usage API show up
automatically, since the generic `limits` array is parsed as-is.

## License

[MIT](LICENSE)
