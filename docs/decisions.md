# Decisions

Newest first.

## 2026-11-23: Less curve and a quieter edge on the glass
- **Decision:** corner radius 9 px (was 12). The edge is 1 px at about a quarter opacity (was 1.3 px at 40%).
- **Why:** the glass should be noticed for its time, not its outline. The thin edge stays, because it covers the blur's stepped corners.

## 2026-11-23: Two programs
- **Decision:** `tomat` (timer and glass, static musl) and `tomat-window` (Slint, dynamic), installed side by side.
- **Why:** measured about 1 MiB RSS for what stays, and 17 MiB for the window only while it is open. One binary would map the toolkit into the part that stays, and a static one can't load the toolkit's Wayland and keyboard libraries.
- **License:** tomat stays MIT under Slint's royalty-free license, which needs its badge on a public page: the README.

## 2026-11-23: How the app behaves
- **Decision:** focus and break alternate until Stop. No popups and no sound. Opening tomat again is the way to pause, stop and move: no shortcuts and no tray.
- **Logs:** what went wrong, plus the window opening and closing. Never a timer's start, finish or length, which would be tracking.
- **The crash copy of the timer** lives in the runtime folder, so logout erases it and no history builds up.

## 2026-11-23: The glass is 108 px wide
- **Decision:** 108 x 42, down from 144 x 42. The room beside `MM:SS` now matches the room above and below it.
- **Why:** the wide sides were empty space covering what's behind.
- **Limit:** times of 100 minutes and more (`120:00`) still fit, with less room at the sides. The width is fixed, so it never jumps while counting.

## 2026-11-23: No orange ring while the glass is movable
- **Decision:** the glass looks the same whether it can be dragged or not. The grab cursor over it is the cue. The thin neutral edge stays.
- **Why:** an orange ring competes with the square, whose color is the state.

## 2026-11-23: The glass is only a square and the time
- **Decision:** no progress bar on the glass. The square alone carries the state: orange focus, green break, yellow paused.
- **Why:** the bar was too much for something that is always on screen.

## 2026-11-23: A desktop app with a glass timer, and no TUI
- **Decision:** tomat becomes three editable focus/break slots and a small frosted timer above every window. The TUI, the history, the session count, the popups and the bell go when the app replaces it.
- **Why:** a timer inside a terminal is out of sight as soon as another window covers it. And nothing about past sessions is wanted: no tracking.
- **The glass** (built first):
  - wlr-layer-shell, Overlay layer, scope `dock`: above fullscreen apps and kept during Show Desktop.
  - An empty input region and no keyboard interest: clicks pass through and it never takes focus.
  - `ext-background-effect-v1` for the blur. Without blur the body turns nearly solid.
  - Smoked on a dark KDE, light on a light one, live, through inotify on `kdeglobals` (no D-Bus client).
  - Painted in software into two small shared-memory buffers, once a second at most.
- **Moving:** KWin can't drag layer surfaces. While movable, an invisible catcher over the monitor (one stretched pixel) takes clicks only where the glass is, and the glass follows the pointer.
- **Measured:** the static preview uses under 1 MiB RSS, dragging included, and about 0.2% of a core.
- **Rejected:** a terminal with a window rule (blocks clicks, fades the text, takes focus), a panel widget (hidden by fullscreen apps), Qt or GTK for the glass (many times the memory).

## 2026-10-25: tomat 0.1, a terminal timer
- Replaced by the app above. Its decisions are in this file's git history.
