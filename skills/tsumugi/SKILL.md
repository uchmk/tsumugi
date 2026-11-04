---
name: tsumugi
description: Use when running inside tsumugi (the terminal for several AI CLI sessions, TSUMUGI_SESSION is set) to look at, drive or wait for the other sessions, or to mark this session for the user
---

# tsumugi

This skill is for [tsumugi](https://github.com/uchmk/tsumugi), a terminal that runs several AI CLI sessions side by side.
If `TSUMUGI_SESSION` is not set, you are not inside tsumugi and this skill does not apply.
`tsumugi skill setup` installs this skill together with the MCP server it relies on.

Each session is numbered; this one is `$TSUMUGI_SESSION`.

Prefer the MCP tools (`tsumugi_attention`, `tsumugi_sessions`, `tsumugi_screen`, `tsumugi_wait`, `tsumugi_send`, `tsumugi_split`, `tsumugi_close`); the same things are on the command line:

- `tsumugi ls [--json]` lists the sessions (number, state, command, folder, title, tags).
- `tsumugi read N` shows what session N displays (`tsumugi_screen`). Reading needs no permission.
- `tsumugi wait N` blocks until session N is waiting or done.
- `tsumugi send N TEXT` types a line into session N. The user must press Allow in a box in their window, and may refuse: say what you send and why, and do not retry a refusal.
- `tsumugi notify --state waiting|done|error [MESSAGE]` marks this session in the sidebar; `tsumugi tag TAG...` tags it.

Sessions belong to the user. `tsumugi_split` opens a shell pane for you to work in and `tsumugi_close` closes only such a pane; a session you did not open is never closed from here.
