# Decisions

Newest first.

## 2026-10-25: Hardening after a full code review
- **Decision:** a review of all of `src/` found 31 issues (3 reproduced). All are fixed except one, noted below.
  - **Time is wall-clock (`SystemTime`), not `Instant`.** A monotonic clock stops while the laptop is suspended, so a break would end late.
  - **tomat opens and closes the terminal itself.** `ratatui::restore` leaves an alternate screen tomat never entered, which made tmux put the next prompt over old output. `ratatui::init` panicked without a TTY. A panic hook now restores the terminal and shows the cursor.
  - **SIGTERM, SIGHUP, SIGINT and SIGQUIT** stop tomat cleanly, through `signal-hook`, which crossterm already pulls in.
  - **The loop ticks the clock before handling a key**, and quits last. Before, a key at a phase's last moment could swallow its completion.
  - **The history reader skips non-UTF-8 lines**, where before they failed the whole count.
  - **`-w`/`-b` round to whole seconds, at least 1.** Before, `-w 1e-12` made zero-length phases that kept recording.
  - Help and version output ignore a closed pipe.
  - Smaller fixes: the clock rounds at nanoseconds; the layout is sized to the phase's full clock; the selected preset stays in view; the tally fits any count; there is no duplicate custom preset; `notify-send` processes are always collected; the test temp dir is cleaned on failure.
- **Kept on purpose:** the loop still wakes once a second when idle. That is how it notices a stop signal and collects `notify-send` processes; a wake that changes nothing redraws nothing.

## 2026-10-25: mise for tools and tasks, plus rust-toolchain.toml
- **Decision:** `mise.toml` pins Rust 1.99.1 in `[tools]` (minimal profile, clippy, rustfmt, the musl target) and holds the tasks (`mise run check`, `run`, `install`, `install-glibc`). `rust-toolchain.toml` pins the same toolchain for rustup.
- **Why:** the user already manages tools with mise, so one file covers both the toolchain and the tasks. `rust-toolchain.toml` is Rust's own pin: plain-rustup users and editor tooling get the same compiler and the musl target installed automatically, so the install task needs no `rustup target add`.
- **Rejected:** a justfile (it would need a second tool just for tasks) and a Makefile.

## 2026-10-25: Keys hidden behind an `i` chip
- **Decision:** an `[i]` chip sits at the end of the presets/progress row on every screen. The presets show no numbers (the `1`–`9` keys still pick them). The bottom row stays empty until `i` (or `?`) is pressed. Then the chip lights up and the keys appear there, so nothing else moves. Notices still use that row when there is one.
- **Why:** the user wanted a quieter screen. Two layouts were mocked; the user picked this one over swapping the row's content.

## 2026-10-25: The progress bar is two layers: a solid streak with sparkles on top
- **Decision:** the done part is solid accent color. Over its front, a layer of cream braille dots gets denser toward the leading edge, like the sparkle trail of something flying. The dots drift backward every 200 ms while the timer runs, and freeze when it is paused. The percentage sits in a tinted chip.
- **Why:** picked by the user from a gallery of four bars (stipple, dot matrix, glow, segments) rendered in real Alacritty, then refined to "dots layered over the bar, densest at the front".
- **Note:** braille comes from the terminal's fallback font, so the dots sit a little high in each cell. Cream dots won over dark ones (which read as holes) and yellow or light-orange ones (too little contrast on orange).

## 2026-10-25: Orange focus, aqua break, status chips
- **Decision:** focus is gruvbox orange `#fe8019` and breaks are aqua `#8ec07c`. The phase label is a chip on a raised surface with a colored square (hollow while waiting to start). Today's sessions are spaced orange dots.
- **Why:** inspired by a reference design the user shared (a budget bar with a status chip and tinted badges). Orange with the cream digits is warmer than red; aqua reads as rest next to it, where gruvbox green looked too close to yellow.

## 2026-10-25: Built to stay under ~1 MiB of memory
- **Decision:**
  - Desktop notifications run the system's `notify-send` per popup instead of linking a D-Bus client.
  - Flags are parsed with `lexopt` instead of `clap`.
  - ratatui uses only its `crossterm` feature.
  - The history is read line by line.
  - Release builds use `opt-level = "s"` and `panic = "abort"`.
  - `mise run install` builds a static musl binary.
- **Why:** measured with `/proc/PID/smaps_rollup` after one focus phase. For comparison, a build with `notify-rust` + `clap` + default features used **3.6 MiB RSS / 1.2 MiB PSS**. After its first popup it grew to **4.6 / 2.1**, and kept two D-Bus threads alive for good. Now:

  | Build | RSS | PSS |
  |---|---|---|
  | glibc, opt 3 | 2.7 | 1.1 |
  | glibc, opt "s" + abort | 2.6 | 0.9 |
  | **musl static, opt "s" + abort** | **0.9** | **0.9** |

  The static build stops mapping the shared C library, which is where most of the remaining RSS was.
- **Rejected:** `opt-level = "z"`: no gain over "s". Keeping `notify-rust`: it holds about 1 MiB and two threads to send one popup every half hour.
- **Trade-off:** desktop notifications need `notify-send` (libnotify), which ships with GNOME and most Linux desktops. Without it, only the terminal bell rings.

## 2026-10-25: Code in three areas: pomodoro, tui, system
- **Decision:** `src/pomodoro/` (pure logic), `src/tui/` (terminal), `src/system/` (notifications, history), with `main.rs` and `cli.rs` as wiring. Whole-screen render tests draw into ratatui's `TestBackend`.
- **Why:** each folder has one job and a small public surface. Layout regressions now fail `cargo test` instead of needing a manual look.

## 2026-10-25: A plain pomodoro with three presets
- **Decision:** focus and break simply alternate. There are no rounds and no long break. The presets are 15/5, 30/5 and 60/10, and `-w`/`-b` give a custom pair.
- **Why:** the user asked for "a simple normal pomodoro" with exactly these times.

## 2026-10-25: One compact block, inline, on the terminal's own background
- **Decision:** a 10-row block drawn under the prompt (ratatui `Viewport::Inline`): a thin line font for the clock, a side column with the status chip, captions and tally, then the bar or presets row and the key hints. It is cleared on quit. The only fills are on chips and the bar.
- **Why:** a full-screen app with a painted background didn't match the user's terminals (both use `#17191A`). Designs were compared as rendered screenshots in the user's own font and background.
- **Rejected:** a bordered box (it left a big empty area beside the clock), and thick pixel digits (too loud for a 30-minute stare).

## 2026-10-25: Wait for Enter between phases by default
- **Why:** a break that starts by itself while you're away isn't a real break. `--auto` is there for people who want it.

## 2026-10-25: Skip starts the next phase at once and doesn't count
- **Why:** skipping is a deliberate choice, so asking again would be friction. A skipped focus wasn't finished, so it isn't recorded.
