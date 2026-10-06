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
lists them. `tsumugi new [FOLDER] [--tag TAG] [-- claude]` starts one in a tab
of its own (the server too, if it is not running) and prints its number;
`tsumugi attach NAME` opens a window on one, named by its number, folder,
program or tag. On Windows the server runs from a copy of the exe kept in
`%LOCALAPPDATA%\tsumugi\server\`, so a rebuild or an update can replace
`tsumugi.exe` while sessions run. A window that finds a server of another
version offers to stop it -- the tabs are written down first -- and to start
its own, which brings them back. The sidebar marks each one: cyan while it works, yellow when it
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

### The input box

`Ctrl+I` (`Cmd+I` on macOS) opens a box below the panes to write a prompt in
as in any editor: `Enter` is a new line, `Ctrl+Enter` sends it to the pane
with the keys whole (pasted, then Enter), so there is no fight with
`Shift+Enter`. Files dropped on the window become chips whose paths go with
the prompt. `↑` brings back what was sent (kept between runs), a draft
stays with its session, and choosing a tag under **To** sends the same
prompt to every session wearing it. `Esc` gives the keys back to the pane.

### The status bar

Along the bottom: the server, how many sessions wait, run or failed, and the
folder and branch of the pane with the keys -- with `↑N` for commits not
pushed, `↓N` for commits to pull, and the branch's pull request (`PR #42 ✓`,
green when its checks pass, red when one failed, cyan while they run; a
click opens it). The pull request needs the GitHub CLI (`gh`) signed in.

### Splits

`Alt+Shift+=` splits the pane with the keys to the right, `Alt+Shift+-`
below (`Cmd+D` and `Cmd+Shift+D` on macOS); `Alt+Arrows` move between them
and `Ctrl+Shift+Z` zooms one. Drag a pane by its header onto another: the
middle trades their places, an edge puts it on that side. A pane narrower
than 20 columns or lower than 4 rows folds into a strip with its name and
state.

### New sessions

The **+** beside SESSIONS, or `Ctrl+Shift+T` (`Cmd+T` on macOS), opens the new-session dialog with the
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

### Many sessions

The order button's menu also has **One line each** (a 28px row per tab,
the tab shown still a full card; offered once when there are more than
twelve) and **Narrow rail** (`Ctrl+Shift+B`, `Cmd+Shift+B` on macOS, or
drag the sidebar's edge narrower than 120px): a 60px strip of squares with
the project's first letter, ringed in the state's colour, the card on hover
and how many wait at the foot. Sorted by **Folder**, the tabs come under
headings that close with a click; an open one shows only what wants you
and the tab shown, the rest as "+ N more", which opens it all. The design
asks `Ctrl+B` for the rail, but Claude Code and tmux use it.

### Tags

Put name tags on a session to tell them apart and pick them out: right-click
a tab and type into **Add a tag**, or from inside it run `tsumugi tag review`
(`--remove` takes one off, no tag lists them, `--session N` names another
session). A session has five at most; a tab shows three and `+N`. The tags in
use line up under SESSIONS: click one to show only its tabs, click it again
for all, and right-click it to mute the notifications of every session
wearing it. Folder rules in the settings tag sessions by themselves.

### Settings

`Ctrl+,` (`Cmd+,` on macOS), or **Settings** in the search, opens the
settings screen: General (the clock, restoring without asking), Appearance,
Keys, Notifications (the table of which states tell in which way, quiet
tags, a test), Sessions & profiles, Tags, Theme (the list and a preview,
applied as you pick), Shell & hooks and Advanced. A change there rewrites
only its own line of `settings.toml`, so what you wrote by hand stays.

`settings.toml` is read from `%APPDATA%\tsumugi\` on Windows,
`~/Library/Application Support/tsumugi/` on macOS and `~/.config/tsumugi/`
elsewhere (`TSUMUGI_SETTINGS` names another file). Changes apply within a
couple of seconds, without restarting; a mistake shows above the status bar
with its line, and the last good settings stay in force.

```toml
# A theme's name, or "dark", "light" or "system" (following the OS between
# dark_theme and light_theme).
theme = "dark"
dark_theme = "tsumugi Dark"
light_theme = "tsumugi Light"

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

# The status bar's clock; date_format is YYYY/MM/DD, YYYY-MM-DD, MM/DD/YYYY
# or DD/MM/YYYY.
[clock]
show = true
hour24 = true
date = true
date_format = "YYYY/MM/DD"
weekday = true

# The panes' font: a font file's name (or part of it) or its path, "" for
# the Nerd Font found; its Bold and Italic files beside it are used too.
[font]
family = "JetBrains Mono"
size = 14
line_height = 1.0

# How much a pane without the keys is dimmed, in percent, and whether
# waiting tabs breathe in gold and running ones show a moving cyan line.
[appearance]
dim = 35
animations = true

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

Themes: tsumugi Dark (the default) and Light, Tokyo Night, Catppuccin Mocha
and Latte, Dracula, Nord, Gruvbox Dark and Light, Solarized Dark and Light,
One Dark and Rosé Pine. A theme is twelve colours -- `bg`, `side`, `panel`,
`border`, `fg`, `dim`, the states' `wait`, `run`, `err` and `done`, and the
terminal's `blue` and `magenta` -- written as `"#rrggbb"`, and `light`.
`theme.toml` beside the settings changes any of them in the theme in force;
a file in `themes/` there is a theme of one's own, named by its `name` or
its file, its missing colours taken from tsumugi Dark or Light.

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
