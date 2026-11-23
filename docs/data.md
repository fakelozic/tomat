# Data

Everything is owner-only and stays on the PC. There is no history and no count.

| File | What | Lives |
|---|---|---|
| `$XDG_STATE_HOME/tomat/slots` | `15/5 30/5 60/10`: focus/break minutes of the three slots | Until edited |
| `$XDG_STATE_HOME/tomat/glass-position` | `X Y MONITOR`: fractions of the room it has to move in, so it holds when the resolution changes | Until dragged again |
| `$XDG_STATE_HOME/tomat/logs/tomat-YYYY-MM-DD.log` | What went wrong, and the window opening and closing | 7 days, 2 MB a day at most |
| `$XDG_RUNTIME_DIR/tomat/timer` | `SLOT DONE_MS SEEN_MS running\|paused`: lets a crash resume the timer | Erased when the round ends, on Stop, and at logout |
| `$XDG_RUNTIME_DIR/tomat/lock`, `door` | Single instance | While running |

`$XDG_STATE_HOME` is usually `~/.local/state`. Files are read with a size limit and without following a pipe, and replaced atomically. A bad file means the default.

A running timer that was gone for over two whole rounds isn't resumed. A paused one is.

tomat 0.1 kept a history in `~/.local/share/tomat/log.csv`. 0.2 never reads or writes it, and doesn't delete it.
