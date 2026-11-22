# Codebase map

| Path | What it does |
|---|---|
| `src/main.rs` | Entry point and wiring: startup, opening and closing the inline terminal, panic and signal cleanup, the event loop, carrying out effects |
| `src/cli.rs` | Command-line flags (`-w`, `-b`, `--auto`, `--no-notify`) and the help text |
| `src/pomodoro.rs` | The pure core's public surface |
| `src/pomodoro/app.rs` | The state machine: menu and session, `Action` in, `Effect` out, when the screen next changes |
| `src/pomodoro/plan.rs` | Focus/break phases, plans, the 15/5, 30/5, 60/10 presets, length formatting |
| `src/pomodoro/timer.rs` | A pausable, drift-free countdown |
| `src/tui.rs` | The terminal side's public surface |
| `src/tui/keys.rs` | Key bindings: key → `Action` |
| `src/tui/layout.rs` | Where things go: `HEIGHT`, the side column, folding in narrow panes |
| `src/tui/view.rs` | What each screen shows (menu, session), and whole-screen render tests |
| `src/tui/widgets.rs` | Pieces: clock, layered sparkle progress bar, status chip, presets row, the `i` chip, tally, key hints |
| `src/tui/digits.rs` | The 5-row line font for the big clock |
| `src/tui/theme.rs` | Gruvbox colors, the tint helper and status marks (the only place colors live) |
| `src/system.rs` | The outside world's public surface |
| `src/system/notify.rs` | Terminal bell, `notify-send`, and reaping its processes |
| `src/system/history.rs` | The append-only history file and the "today" count |
| `src/lib.rs` | The library: the graphical side |
| `src/glass.rs` | The glass's public surface and its event loop |
| `src/glass/surface.rs` | The glass's state and surfaces: the visible pill, the invisible drag catcher |
| `src/glass/protocols.rs` | Wayland events, handed to the state |
| `src/glass/paint.rs` | Pure painting of one face into pixels, and the clock text |
| `src/glass/palette.rs` | The smoked and light colors (the only place the glass's colors live) |
| `src/glass/shape.rs` | Size, rounded corners, the blur's area |
| `src/glass/place.rs` | Snapping, staying on screen, the saved position |
| `src/glass/scheme.rs` | Dark or light, from KDE's color file |
| `src/glass/watch.rs` | inotify on that file |
| `src/glass/buffers.rs` | The two shared-memory buffers |
| `src/glass/digits.rs`, `assets/digits.ttf` | The time's characters (11-glyph font, OFL: `assets/LICENSE-digits.txt`) |
| `examples/glass-preview.rs` | The glass with a pretend timer, for trying it |
| `mise.toml` | Pins Rust in `[tools]`; tasks: `mise run check` (all checks), `run`, `install` (static musl build); `mise tasks` lists them |
| `rust-toolchain.toml` | The same Rust pin for rustup: version, clippy, rustfmt, the musl target |

## To change...

| To change | Look at |
|---|---|
| Presets or default lengths | `pomodoro/plan.rs::Plan::presets`, and the fallback in `cli.rs` |
| A key binding | `tui/keys.rs`, then the hints in `tui/view.rs` (`menu_screen`, `session_screen`) |
| Colors | `tui/theme.rs` only |
| The glass's look | `glass/palette.rs`, `glass/paint.rs` |
| The glass's size or corners | `glass/shape.rs` |
| Where the glass snaps | `glass/place.rs::settle` |
| The progress bar or its animation | `tui/widgets.rs::progress_bar` and the constants above it |
| The big digit shapes | `tui/digits.rs` |
| The notification text | `pomodoro/app.rs::finished_notice` |
| What happens when a phase ends | `pomodoro/app.rs::App::tick` |
| A command-line flag | `cli.rs` (parser, `HELP`, tests) |
| The history format | `system/history.rs`, and update `data.md` |
