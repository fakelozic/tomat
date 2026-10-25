# tomat: notes for AI agents

A ratatui pomodoro TUI drawn inline under the shell prompt. Read `docs/architecture.md` and `docs/codebase-map.md` first.

## Commands
- Check everything: `mise run check` (fmt check, clippy pedantic with `-D warnings`, tests)
- Run quickly: `mise run run` (3-second phases, history kept in `target/dev-data`)
- Install: `mise run install` (static musl) or `mise run install-glibc`
- Toolchain: pinned to Rust 1.99.1 in both `rust-toolchain.toml` and `mise.toml` `[tools]`. Change both together. `rust-version` in Cargo.toml is the MSRV, which is separate.
- Look at the UI without touching the user's tmux: `tmux -L tomat-test new-session -d -x 80 -y 11 './target/debug/tomat --no-notify'`, then `tmux -L tomat-test capture-pane -p` (add `-e` for colors), then `tmux -L tomat-test kill-server`.
- Keep manual runs out of the real history: prefix with `XDG_DATA_HOME=<scratch dir>`.

## Layout
- `src/pomodoro/`: pure logic. No terminal, disk, processes or clock reads (`SystemTime::now()`). Time is passed in, and tests use fixed `SystemTime`s. Work for the outside goes out as `pomodoro::Effect`.
- `src/tui/`: keys in, frames out. Never changes app state.
- `src/system/`: notifications and the history file. Never decides anything.
- `src/main.rs`, `src/cli.rs`: wiring only.

## Rules
- Colors are defined only in `tui/theme.rs`.
- Key bindings live in `tui/keys.rs`, and the hints in `tui/view.rs` (`menu_screen`, `session_screen`) describe them. Change both together.
- Design rules:
  - No painted backgrounds except chips and the progress bar.
  - Every state label is exactly five letters (TOMAT, FOCUS, BREAK, PAUSE), so nothing beside it shifts.
  - The menu and session screens use the same rows.
  - The key list stays hidden until `i` is pressed. The `i` chip sits at the end of the presets/progress row on every screen, and the bottom row stays empty until then.
- After any visual change, run `cargo test`. The render tests in `tui/view.rs` compare whole screens. Paste the printed screen only when the change was intended.
- Memory matters: don't add a dependency that stays resident (async runtimes, D-Bus clients) without measuring it (see `docs/decisions.md`).
- Clippy pedantic is on. Use `#[expect(..., reason = "...")]` for a justified exception, never a bare `allow`.
- No `unwrap()` outside tests. Never print to stdout while the TUI runs (except the BEL in `system/notify.rs`).
- The history file is append-only. Don't rewrite or truncate the user's file.

## Traps
- `"pause "` in the hints has a trailing space on purpose: it keeps the hints from shifting when the label becomes `resume`.
- The clock rounds seconds *up* (`tui/widgets.rs::clock_text`), so a fresh timer reads `30:00`, not `29:59`.
- The digit font (`tui/digits.rs`) mixes `█` with `▀`/`▄` on purpose: a cell is twice as tall as wide, so a full-width column and a half-height row are the same stroke thickness.
- The bar's animation step comes from the timer's elapsed time (`tui/view.rs::animation_step`), not a frame counter, so a paused bar is still and tests are deterministic.
- `lexopt::Parser::from_args` (not `from_iter`): `main` already drops the program name.
- Don't use `ratatui::init`/`restore`/`run`. They assume the alternate screen, and leaving it moves the cursor in tmux. `main.rs` has `open_terminal`/`close_terminal`/`restore` for the inline block.
- Time is `SystemTime` (it counts through suspend), not `Instant`. Keep it that way in the timer.
- In the loop, `App::tick` runs before `App::handle`, and `Effect::Quit` is handled last. Reordering loses finished phases.
- Lengths from `-w`/`-b` are whole seconds (1 s to 24 h), so the clock, captions and history agree.
