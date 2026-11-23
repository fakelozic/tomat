# Codebase map

| Path | What it does |
|---|---|
| `src/main.rs` | `tomat`: arguments, letting go of the terminal |
| `src/lib.rs` | The library both programs share |
| `src/host.rs` | The app's loop: timer, glass, window, saving |
| `src/host/door.rs` | Single instance: the lock and the "show the window" socket |
| `src/host/window.rs` | The window process as the app sees it |
| `src/host/cue.rs` | The chime and popup when a phase ends: their own processes, never waited for |
| `src/timer.rs` | The pure timer: one focus, one break, then over; pause, sleep, crash restore |
| `src/plan.rs` | Slots and their limits (1 to 999 minutes) |
| `src/link.rs` | The messages between app and window |
| `src/paths.rs` | Every file tomat touches |
| `src/private_file.rs` | Safe reads and atomic writes of its own files |
| `src/log.rs`, `src/log/` | The 7-day JSON-line log |
| `src/theme.rs`, `src/theme/` | Dark or light from `kdeglobals`, watched with inotify |
| `src/glass.rs` | The glass's public surface and its wait |
| `src/glass/surface.rs` | The visible pill and the invisible drag catcher |
| `src/glass/protocols.rs` | Wayland events, handed to the state |
| `src/glass/paint.rs` | Pure painting of one face |
| `src/glass/palette.rs` | The glass's colors (the only place they live) |
| `src/glass/shape.rs` | Size, corners, blur area |
| `src/glass/place.rs` | Snapping and the saved place |
| `src/glass/buffers.rs` | Two shared-memory buffers |
| `src/glass/digits.rs`, `assets/digits.ttf` | The time's characters (OFL: `assets/LICENSE-digits.txt`) |
| `window/src/main.rs` | The window process |
| `window/src/fields.rs` | What may be typed |
| `window/ui/theme.slint` | The window's colors and sizes (the only place they live) |
| `window/ui/app.slint`, `window/ui/controls.slint` | Its two screens and their pieces |
| `packaging/`, `scripts/install.sh` | Menu entry, icon, per-user install |
| `examples/glass-preview.rs` | The glass alone, with a pretend timer |

## To change...

| To change | Look at |
|---|---|
| The glass's look | `glass/palette.rs`, `glass/paint.rs` |
| Its size or corners | `glass/shape.rs` |
| Where it snaps | `glass/place.rs::settle` |
| The window's look | `window/ui/theme.slint`, then the two other `.slint` files |
| The default slots | `plan.rs::Slots::default` |
| What happens at a phase's end | `timer.rs::Timer::tick`, `host.rs::Host::tick`, `host/cue.rs` |
| A button | `link.rs::Ask`, `host.rs::hear_window`, `window/src/main.rs::wire` |
| What is logged | `log.rs`'s rule first, then `docs/operations.md` |
