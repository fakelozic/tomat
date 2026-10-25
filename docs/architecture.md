# Architecture

tomat is one small binary with three areas, plus the wiring that connects them:

```
                       ┌──────────── src/main.rs (wiring) ─────────────┐
 keys ─► tui::keys ─Action─► pomodoro::App ─Effect─► notify / record / quit
                                  │                       │
                    tui::view ◄───┘ reads App every frame  ├─► system::Notifier (bell + notify-send)
                         │                                 └─► system::History  (log.csv)
                         ▼
               terminal (ratatui + crossterm, inline viewport)
```

| Area | Folder | Job | May touch |
|---|---|---|---|
| Pomodoro | `src/pomodoro/` | Decides everything: plans, the countdown, what happens on each key and when a phase ends | Nothing: pure functions of their inputs |
| TUI | `src/tui/` | Keys in, frames out: key bindings, layout, screens, widgets, font, colors | The frame it is given |
| System | `src/system/` | Carries out effects: desktop notifications, the history file | Processes and disk |
| Wiring | `src/main.rs`, `src/cli.rs` | Reads flags, builds the app, runs the loop, routes effects | Everything, but decides nothing |

Dependencies point one way: `tui` and `system` use `pomodoro`; `pomodoro` uses neither.

## The loop (`main.rs::event_loop`)
1. Draw the frame from the current `App`.
2. Sleep until something on screen will change, or a key arrives. While a phase runs, that is the next clock second or the next 200 ms animation step, whichever comes first (`App::next_change`). Otherwise it is one second, so a stop signal is noticed and finished `notify-send` processes are collected.
3. Call `App::tick(now)` **first**: a phase that ran out before the key arrived is finished, recorded and announced before the key acts.
4. Map a key to an `Action` (`tui::action_for`) and call `App::handle`.
5. Collect finished `notify-send` processes, then carry out the effects, with `Quit` last so a finish in the same pass is still recorded.
6. Stop when the app quits or a signal (SIGTERM, SIGHUP, SIGINT, SIGQUIT) sets the stop flag.

## The terminal (`main.rs`)
- `open_terminal` turns on raw mode and creates an inline viewport of `tui::HEIGHT` rows. Without a usable terminal it returns an error ("no interactive terminal") instead of panicking.
- `close_terminal` clears the block, puts the cursor where tomat started, and turns raw mode off.
- tomat doesn't use `ratatui::init`/`restore`: they assume the alternate screen, and leaving it moves the cursor in tmux and xterm.
- A panic hook restores the terminal too. Release builds abort on panic, so the hook is the only cleanup a panic gets.

## Where state lives
- **In memory:** all of it, inside `App`. A `Session` holds the `Plan`, the current `Phase` and the `Timer`.
- **On disk:** only the history (`system/history.rs`, see `data.md`). It is read once at startup for the "today" count.

## Key choices
- **Pure core, thin edges.** The logic takes the current time (`SystemTime`) as an argument and returns effects, so it is tested without a terminal, a clock or mocks. Even the bar's animation is a pure function of the elapsed time.
- **Wall-clock time.** The timer counts `SystemTime` from start and pause moments, not counted ticks, so a slow frame can't make it drift. It also keeps counting while the laptop is suspended: a break due at 12:30 ends at 12:30. A clock set backwards counts as no time.
- **Small at runtime.** No async runtime, no linked D-Bus client, no CLI framework. See `decisions.md` for the measurements behind each choice.
