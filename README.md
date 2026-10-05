# tsumugi

A terminal built for running many Claude Code and other AI CLI sessions side
by side: vertical tabs, split panes, and a glance at which session is waiting
for you. Rust + egui, Windows first, with macOS and Linux on x86_64 and ARM64.

**Status: early.** A window with one shell in it, nothing more yet. The first
version's scope is in [docs/v1-scope.md](docs/v1-scope.md).

```sh
cargo run -p tsumugi
```

On Windows, a build with the newer ConPTY beside it is on the Actions tab
(the **Build** workflow, artifact `tsumugi-windows-x64-…` or `-arm64-…`). A
local build gets it with `pwsh -File scripts/fetch-conpty.ps1 -Dest target\debug`;
without it the pane runs on the older ConPTY built into Windows, which breaks
`Esc` in lazygit and the like.

Sessions live in a background server (`tsumugi server`, which the window
starts by itself), so closing the window leaves them running; `tsumugi ls`
lists them. The sidebar marks each one: cyan while it works, yellow when it
wants you, a yellow ring when its output has stopped for 10 seconds while a
program runs (`TSUMUGI_QUIET_SECS`, 0 to turn the guess off), green when the
shell is back at its prompt, red for an error. `Ctrl+Shift+U` (Cmd+Shift+U on
macOS) goes to the session that has waited longest.

### Claude Code hooks

An agent can say for itself that it is waiting. With `tsumugi` on the `PATH`,
add to Claude Code's settings (`~/.claude/settings.json`):

```json
{
  "hooks": {
    "Notification": [{ "hooks": [{ "type": "command", "command": "tsumugi notify --stdin" }] }],
    "Stop": [{ "hooks": [{ "type": "command", "command": "tsumugi notify --state done" }] }]
  }
}
```

`tsumugi notify` works inside a tsumugi session only (it reads
`TSUMUGI_SESSION`). Programs that send OSC 9, 99 or 777 notifications are
marked without any setup.

The terminal pane is the crate `tsumugi-pane` (`crates/tsumugi-pane`), shared
with [filer](https://github.com/uchmk/filer).

## Why

[cmux](https://github.com/manaflow-ai/cmux) showed what a terminal for coding
agents looks like: a sidebar of vertical tabs, each with its branch, folder and
latest notification, and a ring around the pane whose agent is waiting. It is
macOS only. tsumugi aims at the same idea on Windows first, then everywhere,
and then past it:

- **Windows first, every platform after.** Windows x64 and ARM64, then macOS
  and Linux, from one Pure Rust code base.
- **Knows a waiting agent without being told.** Agent hooks and OSC 9/99/777
  notifications, as cmux uses, plus what the terminal can see by itself: the
  prompt coming back, the window title, the process tree.
- **A real input box.** Multi-line prompts edited in a proper editor field and
  sent whole, instead of fighting `Shift+Enter` inside a TUI.
- **A multiplexer underneath.** Sessions live in a background server, so
  closing the window, or the window crashing, never ends a running agent;
  open it again and everything is where it was. A CLI (`tsumugi ls`,
  `attach`, `notify`) drives it from scripts and agent hooks. Later, tmux over
  SSH or in WSL drawn as native tabs and splits.
- **Good-looking.** GPU-drawn and smooth, a waiting pane that glows rather
  than blinks, Mica and Acrylic on Windows 11, ligatures and Nerd Font icons.

## Where it comes from

The terminal pane of [filer](https://github.com/uchmk/filer), a yazi-style
file manager, has been driven hard on real Windows machines: ConPTY with a
newer bundled build, win32-input-mode, OSC 7, mouse reporting, a scripted test
harness. That pane is being split out into a shared crate, and tsumugi is
built on it.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option.
