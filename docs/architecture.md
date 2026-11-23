# Architecture

Two programs, so the one that stays is tiny:

```
 menu / terminal ─► tomat ── already running? knock on its door and exit
                      │
                      ▼
      ┌──────────── tomat (static, about 1 MiB) ────────────┐
      │ host ── timer (pure)                                │
      │   ├──── glass ─► KWin: layer surface + blur         │
      │   ├──── door (unix socket): "show the window"       │
      │   ├──── cue: pw-play + notify-send, never waited on │
      │   └──── log, saved slots, crash copy of the timer   │
      └───────────────┬─────────────────────────────────────┘
               socketpair, one line per message (link.rs)
                      ▼
        tomat-window (Slint, about 17 MiB, only while open)
```

| Area | Job | May touch |
|---|---|---|
| `timer`, `plan`, `link` | The rules, the slots, the messages | Nothing: pure |
| `host` | Owns the timer, decides what opens and closes | Files, processes, sockets |
| `glass` | Shows what it's given, decides where it sits | Wayland, its saved place |
| `theme` | Dark or light | Reads KDE's color file |
| `window/` | Shows what the app says, reports presses | Its socket, the theme |

Dependencies point one way: the window uses the library, and the library knows nothing of the toolkit.

## Life cycle
- **Open:** the first tomat takes a lock and becomes the app. Later ones knock on its socket and exit. From a terminal it first starts itself detached, in its own session.
- **Start:** the window closes and gives its memory back. The timer and the glass stay.
- **Open again:** a new window replaces any old one, because Wayland can't raise another program's window. While a window is open the glass can be dragged.
- **Stop:** the glass goes and the crash copy is erased. Closing the window then ends everything.
- **The app dies:** the window sees its socket close and leaves. The next open restores the timer.
- **The display goes:** the glass retries every 2 seconds. After a minute with neither, the app saves and leaves.

## The loop (`host.rs`)
One thread, asleep in poll on the display, the theme's inotify, the window's socket, the door and the signals. It wakes by itself once a second while counting, and not while paused. The glass redraws only when its face changes.

## Key choices
- **Pure core.** Time is passed in, so the timer is tested without a clock.
- **Wall-clock time.** A break due at 12:30 ends at 12:30, even through sleep. A clock set backwards counts as no time.
- **Nothing waits on the window.** Its socket is nonblocking on the app's side, and a window that won't leave is killed after a second.
