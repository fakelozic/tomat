# tomat: notes for AI agents

A small KDE Wayland app: three focus/break slots and a frosted glass timer over everything. **Nothing more: no TUI, no tracking, no counts, no extras.** Read `docs/architecture.md` and `docs/codebase-map.md` first, and `PLAN.md` if it exists.

Something broke? Read `~/.local/state/tomat/logs/` first (`docs/operations.md`).

## Commands
- Check everything: `mise run check` (format, clippy pedantic with `-D warnings`, tests)
- Build both programs: `mise run build`
- Install: `mise run install`. A rebuild alone changes nothing for the installed copy.
- Toolchain: Rust 1.99.1 in both `rust-toolchain.toml` and `mise.toml`. Change both together.
- Never run it on the real screen or state to test: use a private virtual KWin with its own HOME and XDG_RUNTIME_DIR.

## Layout
- `src/timer.rs`, `src/plan.rs`, `src/link.rs`: pure. Time is passed in.
- `src/host*`: the only place that decides. `src/glass*`: shows. `window/`: shows and reports.
- The glass's colors live only in `src/glass/palette.rs`, the window's only in `window/ui/theme.slint`.

## Rules
- **Never log or save a timer's start, finish, pause or length**, or count anything. So a chime or popup that fails is never logged. The one exception is the crash copy in the runtime folder, erased on Stop.
- The glass is only the colored square and the time: no bar, no ring, no accent edge. The grab cursor is the cue that it moves.
- Both window screens are the same size and nothing shifts. Digits are monospaced.
- Memory matters: timer plus glass stay under 3 MiB. Never link the toolkit, an async runtime or a D-Bus client into `tomat`.
- No `unwrap()` outside tests. No unsafe, except Slint's generated code in `window/src/ui.rs`.
- The app never blocks on the window.

## Traps
- The glass's layer-shell scope must stay `"dock"` (Show Desktop hides any other) and its layer Overlay (Top goes under fullscreen apps).
- The glass is moved under a catcher surface that never moves. Don't read drag positions from the moving surface.
- Keep the outline around smoked text: blur doesn't darken, so cream on a white page needs it.
- Keep the thin edge: it covers the blur region's stepped corners.
- `kdeglobals` is replaced by a rename: watch its folder, not the file.
- The digit font's family is "JetBrainsMonoNL Nerd Font" (its typographic name). A wrong name silently falls back to a proportional font.
- The app echoes slot edits to the window. Writing them back into the fields would move the cursor mid-typing (`show` in `window/src/main.rs`).
- KWin gives the glass the same window class as the window. In scripts, tell them apart by `dock`.
- `signal_hook`'s pipe is how signals wake poll. A flag alone wouldn't.
