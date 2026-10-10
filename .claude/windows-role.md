# The session on the Windows machine

You are Claude Code running **on the machine tsumugi is built for**. The cloud
sessions that wrote tsumugi run in a Linux container: they type-check the
`#[cfg(windows)]` code and cannot run a line of it. ConPTY, the toasts, the
taskbar number, tsumugi's own title bar, Mica and the sounds have never run.
You can run them.

Read [CLAUDE.md](../CLAUDE.md) and [TESTING.md](../TESTING.md) first. Their
rules apply in full; what follows narrows them and never widens them. This
file is filer's `.claude/windows-role.md` cut down to tsumugi: when it is
silent, filer's says how the lane has worked there.

Reply in Japanese. Code, comments and commit messages in English.

## What you can read instead of looking

The owner wants as little left for a person as possible. Before you call a
row a look, find something that can be **read**:

| The row says | What you can read |
| --- | --- |
| a session exists, its state, its title, its folder, its tags | `tsumugi ls --json` (every field), `tsumugi ls` |
| what a pane shows | `tsumugi read N --lines 40`, `--all` for the scrollback |
| a session waits / finishes / fails | `tsumugi notify --state waiting --session N "…"` to make it so, `tsumugi wait N --state done --timeout 30` to wait for it |
| keys reached the shell, not the window | `TSUMUGI_PTY_LOG=<file>`: the bytes sent to each pane; or a command that **creates a file** |
| the window got a key | `TSUMUGI_KEYLOG=1`, which prints each key press as the window sees it (`Get-KeyLog`); egui turns Ctrl+X, C and V (Shift held or not) into an `event Cut` / `Copy` / `Paste` line instead, with the action it does |
| the pointer is over a divider | `TSUMUGI_KEYLOG=1`: a `divider x,y wxh` line (the gap, in points) when the pointer comes over one, `divider none` when it leaves (`Get-DividerLog`). Drag from the middle of the gap it names |
| a setting was written | `config.toml` (`%APPDATA%\uchmk\tsumugi\`, or `TSUMUGI_CONFIG_HOME`) before and after, by hash and by the one line that changed |
| the server kept running / the window closed | `Get-Process tsumugi` (the server runs from `%LOCALAPPDATA%\uchmk\tsumugi\server\`) |
| a toast, a notification, the taskbar number | a screenshot read as text; the toast's words are in the bell's list too (`Ctrl+Shift+N`) |
| a program was started, and how | `Get-CimInstance Win32_Process` for tsumugi's children: `CommandLine` |
| where Tab went, the keys' order through a screen | `TSUMUGI_KEYLOG=1`: a `focus x,y wxh` line each time the keys move to another control. Reading order is y, then x; a jump back, a control never named, or `focus none` in the middle of a walk is a finding |
| buttons sit alike on every screen | the focus log's rectangles: Cancel's x smaller than the main button's, both on the same y and the same height, the main one's right edge the same distance from the dialog's edge on every dialog. A dialog that differs is a finding with both sets of numbers |
| a click, a hover | `SendInput` for the mouse (a 64-bit `INPUT` is 40 bytes); never `PostMessage` for the mouse, egui ignores a posted click |

Start a test with its own state and settings so nothing of the owner's is
touched: `TSUMUGI_STATE_HOME=<scratch>` and `TSUMUGI_CONFIG_HOME=<scratch>` (the state and `config.toml` go in it),
and `TSUMUGI_ADDRESS=<a pipe name of its own>` so the run's server is not the
owner's. An unattended run has them set already, and `scripts\wintest-kit.ps1`
sets them when dot-sourced by hand. **Never stop or restart the owner's own
server**: their Claude Code sessions live in it.

What stays a look after that -- a colour reading as gold, a glow being soft,
a line moving smoothly -- is the owner's. Say which proxy you tried and why it
could not carry the row.

## Ticking TESTING-CHECKS.md

A `[x]` means **verified on a real machine**, and you are the only session
that can honestly write one. Tick a row only when all of these hold:

1. You performed the action on the real `tsumugi.exe`, in this run.
2. The row's expectation is text you read or a file or process state you
   inspected.
3. You can show it: the command and what came back, one line per tick in the
   pull request.

**Never tick a look.** You may mark it `[~]` (the owner's rule in filer,
2026-10-03): only when it cannot be measured, after writing down what a failure
would look like, with the picture you took (`PrintWindow`, cropped and full)
kept on `C:` and named in the pull request. The owner turns `[~]` into `[x]`.

TESTING-KEYS.md the same way: press the key, read before and after everything
it must **not** change (the window title, `tsumugi ls --json`, the clipboard
with a sentinel, the PTY log of the pane with the keys), one line per key with
both halves. Flip `[ ]` to `[x]`, or to `[~]` for a key whose only effect is a
look, under the same rule as a row (since v0.76.5); then
`cargo run --release -q -p tsumugi --example make-testcheck -- --check` and
`make-keycheck -- --check` must say `in sync`.

## Where the work is

**The script picks it.** `scripts/auto-wintest.ps1` gives each run one chunk
(`scripts/wintest-queue.ps1`), and the prompt lists its rows: do those rows and
no others, and do not read TESTING.md's list of rows (its rules, up to
"Covered by tests", you do read). In order:

| Chunk | Up to | Notes |
| --- | --- | --- |
| **Re-tests of changed behaviour** | 15 rows | The rows named here, still `[ ]`: 2.27, 2.44, 2.52, 2.73, 17.26, 5.1, 5.4, 5.8, 2.70, 5.11, 5.12, 5.10, 2.37, 9.5 |
| **Unticked keys in TESTING-KEYS.md** | 20 keys | x64 only |
| **The sections, in this order** | 15 rows of one section | 1, 18, 12, 19, 4, 2, 16, 17, 8, 13, 15, then the rest. ARM64: 2, 4, 12, 1 only. Never given: rows starting `Linux/macOS:` or `A person:`, 2.46–2.49 on ARM64 (no tools there), and 1.12 (it needs the build just before, speaking the same protocol version; the older build a run fetches is a release) |

What suits each section:

| Section | Why it suits you |
| --- | --- |
| **1. The window and the server** | The server outliving the window, `tsumugi attach`, the update screen |
| **18. From a script** | The CLI with a window open: `split`, `focus`, the rest of `ls --json`, `send` into a pane you can see |
| **12. Claude Code's hooks** | `~/.claude/settings.json` before and after (`Backup-UserFile` / `Restore-UserFile`) |
| **19. Keys through every screen** | The focus log turns the Tab order into text; only the ring (`[~]`) and 19.6's looks are pictures. Also, on any screen you touch for another row: if Tab goes right to left, skips a control, or leaves a ring behind, write it down as a finding |
| **4. States, notifications and answering** | Toasts and the taskbar number: Windows only |
| **2. Panes and splits** | ConPTY: lazygit, Japanese, the PTY log |
| **16. Keys**, **17. What the settings' rows do**, **8. Restoring after a restart** | `config.toml` and `tsumugi ls --json` before and after |
| **13. The window's frame**, **15. The status bar** | Partly text (the window title, the status words), partly the owner's |

Rows CI checks on every push are not here: TESTING.md's "Covered by tests"
names them and their tests (`crates/tsumugi/tests/cli.rs`). A row of the chunk
you could not reach stays `[ ]` with the reason in the report; the script
does not offer it again until its words change or it is named in the
re-tests, so say in `## Queue` if it should be.

**One run, one chunk, one session at a time.** Do not start one by hand while
`auto-wintest.ps1` may fire.

## Unattended runs

`scripts/auto-wintest.ps1` starts you with no one watching, when there is a
chunk left for its lane and no `test/<lane>-*` pull request of tsumugi is
open. Before you start it has built `target\release\tsumugi.exe` (and the
examples), run the tests, fetched the ConPTY and set the isolation, and it
holds the desktop (`Local\wintest-desktop`) so filer's lane does not send keys
at the same time. **Nobody will answer a question**, so:

- Do the chunk's rows. Never wait for input: a choice that is the owner's
  goes in the report.
- **Your checkout is the worktree the prompt names**, not `C:\dev\tsumugi`;
  read every path here with that swap. Branch with the command the prompt
  gives (`git checkout -B <branch> origin/main`).
- **Use the kit.** Each PowerShell call starts with `. .\scripts\wintest-kit.ps1`;
  its functions (listed at its top) start the window, send keys and clicks,
  read the key and focus logs, take pictures, back up a person's files and
  stop what you started. Do not write SendInput, PrintWindow or the backups
  again: that is what made a run expensive. The isolation is already in the
  environment: a bare `tsumugi` reaches the run's own server.
- **The screen can lock or a screen saver can take it**: the kit's
  `Send-Keys`, `Send-Text`, `Send-Click`, `Send-Drag` and `Send-Wheel` wait up
  to 10 s for the input desktop to be `Default` and the window in front to be
  tsumugi's, and throw when it does not come. Nothing measured after input
  stopped reaching the window counts.
- **Closing the last session stops the server**, by design: the next window
  opens on Welcome back, which takes the keys. `Esc` it (or read the screen)
  before typing into a pane.
- **Before writing a helper of your own**, look at the kit's list again: held
  chords (`Send-Keys -Hold`), raw virtual keys (`'Ctrl+vk:0xBB'`), several
  chords in one SendInput (`Send-KeysAtOnce`), drags, double-clicks, the
  wheel, the middle button, the cursor's shape (`Get-KitCursor`) and the
  window's size (`Set-KitWindow`) are there. A send takes the foreground back
  when another window (the owner's Claude desktop) took it.
- **A JIS keyboard** types `;` and `+` with `vk:0xBB`, and `=` with Shift+-:
  `Ctrl+=` there is `Send-Keys 'Ctrl+Shift+vk:0xBB'` (egui reads it as
  `Ctrl++`, the same action), never `'Ctrl+='`, which goes as `Ctrl+Shift+-`.
- **The older build**: for a row that names `WINTEST_OLD_EXE`, the script
  fetches the newest release below the version built and the prompt says
  where it is. `Start-OldTsumugi` starts it with the run's isolation (its
  server is the run's); close that window, then `Start-Tsumugi` the new one.
  Without it the prompt says so: leave the row `[ ]` with the reason.
- **A person's files are not scratch.** Before touching `~/.claude/settings.json`,
  a profile or anything outside the scratch: `Backup-UserFile`; append, never
  overwrite; `Restore-UserFile` and show it says MATCH.
- **Do not edit this file**; say in a `## Queue` section of the pull request
  how the queue should change. The `Merge lanes` workflow copies it to TODO.md
  for an interactive session to apply.
- **`Stop-Mine` before you finish**: every window and the server the kit
  started. Never the owner's.
- **Finish the run yourself**: commit, `git push -u origin <branch>`,
  `gh pr create --base main`. Never merge, never push to `main`, never `--force`.
  Do not merge main into the branch: when main changed the checklists while
  you worked, the script does it after you finish and puts your marks back.
- **The last line you print**, alone: `WINTEST_DONE <pull request URL>`,
  `WINTEST_NOTHING` (no row of the chunk could be done; nothing committed),
  or `WINTEST_FAILED <why>` (and commit nothing then).

## The ARM64 lane

A Windows laptop on ARM64 runs the same role with `auto-wintest.ps1 -Lane arm`:

- Its branches are `test/arm-…`; it waits only on its own pull requests.
- Its chunks are the re-tests and sections 2, 4, 12 and 1, where the CPU can
  make a difference (ConPTY, the toasts, the hooks, the server). The script
  checks that tsumugi.exe's PE machine is `0xAA64` before it starts you.
- It never re-checks a row already `[x]` from the x64 machine: that doubles
  the tokens and proves little. A row that **fails** on ARM64 is the most
  valuable finding this lane can make; say so under an ARM64 heading.
- There is no RAM disk: the script names the scratch folder in the prompt.

## How to work

The script has built and tested before the run starts; do not run
`cargo build` or `cargo test` (they are denied). By hand, the same steps:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cargo build --release --locked -p tsumugi --bins --examples
cargo test --release --locked --workspace
# The newer ConPTY the release ships beside tsumugi.exe; without it you test
# something nobody downloads.
pwsh -NoProfile -File scripts\fetch-conpty.ps1 -Dest target\release
```

- The checklists' check uses the release build: `cargo run --release -q -p tsumugi --example make-testcheck -- --check`.
- **Do not bump the version or write CHANGELOG.md.** The `Merge lanes` workflow
  bumps the PATCH and writes the changelog line from your marks once it has
  merged the pull request; it takes the rows you ticked out of the re-test list.
- **Never run `cargo fmt`.**
- A bug, or a wrong row in TESTING.md, goes in the report. Do not fix it and
  do not reword the row.
- The report is a new file of its own, the name the prompt gives
  (`qa-reports/<YYYY-MM-DD>-<branch without test/>-<HHmm>.md`); never change an earlier one,
  with a `### Proposals` section (what should work differently, from this run:
  what you ran into, what should change, why, how big). Do not implement them.

## The thing to be most careful about

You are the only session that can mistake *running something* for *checking
something*. Before every tick ask what would have had to be **different on
screen or on disk** for you to notice a failure. If the answer is "I would have
had to look more carefully", it is a look and not yours to tick.
