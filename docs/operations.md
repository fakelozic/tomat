# Operations: logs and fixes

```sh
cd ~/.local/state/tomat/logs
tail -n 50 "$(ls tomat-*.log | tail -1)"               # the latest lines
grep -hE '"level":"(warn|error)"' tomat-*.log          # everything that went wrong this week
```

One JSON object a line, UTC. The home folder is written as `~`. **No event may reveal a timer's start, finish, pause or length.**

| Event | Means | Look at |
|---|---|---|
| `window.opened`, `window.closed` | The window | Nothing |
| `window.start_failed` | It couldn't start | Is `tomat-window` beside `tomat`? Reinstall |
| `window.failed` | It started, then gave up | The detail: usually the display |
| `window.crashed` | It died by itself | `process.panic` just before it |
| `window.killed` | It didn't leave within a second | A hung display |
| `window.bad_message`, `window.send_failed`, `window.lost`, `window.quit_failed`, `window.hide_failed` | App and window lost touch | Reopen. Both from the same build? |
| `glass.failed` | No glass: retried every 2 s | Wayland session? Layer-shell support? |
| `glass.closed` | The display took it (a monitor went). It comes back | Nothing, unless it repeats |
| `glass.blur_off` | Blur was switched off | KDE's Desktop Effects, Blur |
| `glass.place_not_saved`, `slots.save_failed`, `timer.save_failed` | A write failed | Disk space, permissions |
| `slots.reset` | Saved slots were unreadable: defaults used | The slots file |
| `chime.missing` | No sound when a phase ends, said once at start | Install `pipewire-utils` or the Ocean sound theme |
| `popup.missing` | No popup when a phase ends, said once at start | Install `libnotify` (`notify-send`) |
| `display.gone` | No display for a minute: saved and left | Normal at logout |
| `app.failed`, `app.wait_failed` | The app couldn't run | The detail |
| `process.panic` | A bug | The place in the detail |
| `log.suppressed` | One event over 30 times a minute | That event |
| `log.unavailable`, `log.prune_failed` | The log itself is in trouble | Permissions |

| Symptom | Fix |
|---|---|
| The glass isn't frosted | Turn on KDE's Blur effect. KWin also pauses it during Overview |
| Can't drag the glass | Open tomat first |
| Glass in an odd place | Open tomat and drag it, or delete `glass-position` |
