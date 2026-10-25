# Data

## History: `$XDG_DATA_HOME/tomat/log.csv`
Usually `~/.local/share/tomat/log.csv`. It is created when the first focus session finishes. The code is `src/system/history.rs`.

One line per finished focus session (skipped ones are not written):

```
2026-10-25T12:42:13+05:30,1800
```

| Field | Meaning |
|---|---|
| 1 | When the session finished: RFC 3339 local time, with its UTC offset |
| 2 | Focus length in whole seconds |

- **Append only.** tomat never rewrites or deletes lines. Each line is written in one call, so two running copies can't mix up each other's lines.
- **Reading:** only at startup, line by line (memory stays flat however long the file gets), to count today's sessions. Lines that don't parse are skipped.
- **Retention:** none. The file grows by about 32 bytes per session (about 300 KB after 10,000 sessions). Delete or trim it by hand whenever you like.
- **Known limit:** the "today" count doesn't reset at midnight while tomat stays open. It's correct again on the next start.
