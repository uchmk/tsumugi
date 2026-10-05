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

### After a restart

The server writes the tabs down as they change: each tab's splits, and per
pane its folder, its shell and the Claude Code conversation running in it.
After a restart of the machine, the first window starts them all again and
types `claude --resume <conversation>` where Claude Code ran. The file is
`%LOCALAPPDATA%\tsumugi\state` on Windows, `~/.local/state/tsumugi/state`
on Linux, `~/Library/Application Support/tsumugi/state` on macOS
(`TSUMUGI_STATE` overrides).

On Windows a shell's folder can only be known when the shell says it, so add
the hook to PowerShell 7's profile; without it a tab comes back in the folder
it was opened in:

```powershell
tsumugi shell-hook pwsh >> $PROFILE
```

(`tsumugi shell-hook bash` and `zsh` exist too; on Linux and macOS the folder
is read from the system anyway.)

### When you are not looking

While the window does not have the keyboard, a session that starts waiting,
hits an error, or finishes after running a minute or more shows the system's
notification, and the ones waiting or in error are counted on the taskbar
button (on Windows a red number over the icon; elsewhere the window title
starts with it, `(2) …`). Coming back to the window clears the number. The bell
beside SESSIONS keeps the same list.

Clicking the notification goes to that session's pane and brings the window
to the front (Windows and Linux; macOS's AppleScript notifications cannot
say they were clicked). With several windows open, only the one that had the
keyboard last tells, and none does while you are at any of them. Right-click
a tab and choose **Mute notifications** to keep it to the bell: no system
notification and no number for it, and the tab shows a struck-through bell.

On Windows the notification needs the app's name registered for the current
user; tsumugi writes it at the first notification
(`HKEY_CURRENT_USER\Software\Classes\AppUserModelId\uchmk.tsumugi`). On
Linux it goes through `notify-send` (a click is seen with libnotify 0.7.12 or
later), on macOS through AppleScript. Which
states tell in which way is `[notify]` in the settings (below); by default
the taskbar does not flash.

### New sessions

`Ctrl+Shift+T` (`Cmd+T` on macOS) opens the new-session dialog with the
folder of the pane you are in already filled in and Claude Code chosen, so
one more Claude Code beside this one is that key and `Enter`. Type another
folder or pick one of the folders the other sessions are in (the arrows and
`Tab` complete); choose **Resume last** (`claude --continue`) or **Shell**
instead; the folder rules' tags are there, dashed, and more can be added.
`Enter` opens it in a new tab, `Alt+Enter` splits it to the right of the
pane with the keys. **Save as a profile** keeps the choices under a name in
`profiles.toml` beside the settings, for **Profile…** to fill in next time.

### Searching

`Ctrl+Shift+P` (`Cmd+Shift+P` on macOS), or the box in the band along the
top, searches the sessions (by title, folder, branch and tags), the folders
they are in (to start a new session there) and the window's commands. Type
a few letters in order, move with the arrows, `Enter` to go, `Esc` to close.
The band also shows the tags of the session with the keys.

### Sorting and filtering

The button beside SESSIONS orders the tabs: **Manual** (the default, where a
tab is dragged into place and stays there for every window), **Needs me
first** (waiting, then errors, running, done; the longest waiting first),
**Recent activity**, **Folder** (the repository a tab is in) or **Name**.
The foot of the sidebar shows only the tabs in one state, or in one
repository, and the two combine with the tag filter; the heading then says
how many of all are shown.

### Tags

Put name tags on a session to tell them apart and pick them out: right-click
a tab and type into **Add a tag**, or from inside it run `tsumugi tag review`
(`--remove` takes one off, no tag lists them, `--session N` names another
session). A session has five at most; a tab shows three and `+N`. The tags in
use line up under SESSIONS: click one to show only its tabs, click it again
for all, and right-click it to mute the notifications of every session
wearing it. Folder rules in the settings tag sessions by themselves.

### Settings

`settings.toml` is read from `%APPDATA%\tsumugi\` on Windows,
`~/Library/Application Support/tsumugi/` on macOS and `~/.config/tsumugi/`
elsewhere (`TSUMUGI_SETTINGS` names another file). Changes apply within a
couple of seconds, without restarting; a mistake shows above the status bar
with its line, and the last good settings stay in force.

```toml
# Tag a session by the folder it is in, or anywhere under it.
[[tags.rule]]
folder = "~/dev/filer"
tag = "filer"

# Every folder under ~/dev, by its own name.
[[tags.rule]]
folder = "~/dev/*"
tag = "{name}"

# Which states tell you in which way while you are not at the window:
# waiting, error, done (done only after a run of a minute or more).
[notify]
system = ["waiting", "error", "done"]   # the system's notification
taskbar = ["waiting", "error"]          # the number on the taskbar
flash = []                              # flash the taskbar button

# What the tab's menu opens its folder with; the system's shell runs it.
[open]
editor = "code {folder}"
filer = "filer {folder}"

# The tab's menu: items left out, and your own ({folder}, {session}).
[menu]
hide = ["new-window"]

[[menu.session]]
name = "Open lazygit here"
command = "wt -d {folder} lazygit"
```

The tab's right-click menu: **Rename…**, tags, **Mute notifications**, **Pin
to top**; **Restart** (a fresh shell in its place, resuming the Claude Code
conversation), **Duplicate in the same folder**, **Move to a new window**;
**Open the folder in filer**, **Open in the editor** (the commands in
`[open]` above), **Copy the folder path**; your own items from
`[[menu.session]]`; and **Close the session**, which asks a second click
while something is running in it. `[menu] hide` leaves out any of rename,
tags, mute, pin, restart, duplicate, new-window, filer, editor, copy-path,
close.

A folder rule adds its tag when a session starts or moves into the folder;
it never takes one off, so a tag removed by hand stays off until the session
moves again.

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

The `Notification` hook's `--stdin` also hands over Claude Code's
conversation id, which is what `claude --resume` takes after a restart.
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
