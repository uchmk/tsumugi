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
| the window got a key | `TSUMUGI_KEYLOG=1`, which prints each key press as the window sees it |
| a setting was written | `settings.toml` (`%APPDATA%\tsumugi\`) before and after, by hash and by the one line that changed |
| the server kept running / the window closed | `Get-Process tsumugi` (the server runs from `%LOCALAPPDATA%\tsumugi\server\`) |
| a toast, a notification, the taskbar number | a screenshot read as text; the toast's words are in the bell's list too (`Ctrl+Shift+N`) |
| a program was started, and how | `Get-CimInstance Win32_Process` for tsumugi's children: `CommandLine` |
| a click, a hover | `SendInput` for the mouse (a 64-bit `INPUT` is 40 bytes); never `PostMessage` for the mouse, egui ignores a posted click |

Start a test with its own state and settings so nothing of the owner's is
touched: `TSUMUGI_STATE=<scratch>\state`, `TSUMUGI_SETTINGS=<scratch>\settings.toml`,
and `TSUMUGI_ADDRESS=<a pipe name of its own>` so the run's server is not the
owner's. **Never stop or restart the owner's own server**: their Claude Code
sessions live in it.

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
both halves. Only flip `[ ]` to `[x]`; then
`cargo run -p tsumugi --example make-testcheck -- --check` and
`make-keycheck -- --check` must say `in sync`.

## Where the work is

**A queue, not a menu.** Take the first row that is not done.

| Section | Why it suits you |
| --- | --- |
| **Re-tests of changed behaviour** | First, every run, all of them. Nothing listed yet: every row is `[ ]` until the first runs |
| **Unticked keys in TESTING-KEYS.md** | Second, every run: `cargo run -p tsumugi --example make-keycheck -- --stats` |
| **1. The window and the server** | The server outliving the window, `tsumugi new` / `attach` / `ls`, the update screen: all text |
| **18. From a script** | The CLI: `send`, `read`, `split`, `close`, `wait`, `ls --json` -- all text |
| **12. Claude Code's hooks** | `~/.claude/settings.json` before and after (a copy first, hash afterwards) |
| **4. States, notifications and answering** | Toasts and the taskbar number: Windows only |
| **2. Panes and splits** | ConPTY: lazygit, Japanese, the PTY log |
| **16. Keys**, **17. What the settings' rows do**, **8. Restoring after a restart** | `settings.toml` and `tsumugi ls --json` before and after |
| **13. The window's frame**, **15. The status bar** | Partly text (the window title, the status words), partly the owner's |
| **The rest** | `--stats` names what is short; take a section whose rows read as text |

**Up to three sections per run, one session at a time.** Do not start one by
hand while `auto-wintest.ps1` may fire.

## Unattended runs

`scripts/auto-wintest.ps1` starts you with no one watching, when `main` has
changed this file, TESTING.md or TESTING-CHECKS.md and no `test/win-*` pull
request of tsumugi is open. **Nobody will answer a question**, so:

- Take the first section in the queue. Never wait for input: a choice that is
  the owner's goes in the report.
- **Your checkout is the worktree the prompt names**, not `C:\dev\tsumugi`;
  read every path here with that swap. Branch with
  `git checkout -B test/win-<section> origin/main`.
- **The screen can lock or a screen saver can take it**: check that
  `OpenInputDesktop` names `Default` before `SendInput`, and that the
  foreground window is tsumugi's before a key goes in. Nothing measured after
  input stopped reaching the window counts.
- **A person's files are not scratch.** Before touching `~/.claude/settings.json`,
  a profile or anything outside the scratch: its hash and a copy; append, never
  overwrite; restore it and show the hash matches.
- **Do not edit this file**; say in a `## Queue` section of the pull request
  how the queue should change. Whoever merges applies it.
- **Close every `tsumugi.exe` you started**, and the server you started with
  your own `TSUMUGI_ADDRESS` (`tsumugi close` each session, or stop that
  process by its id). Never the owner's.
- **Finish the run yourself**: commit, `git push -u origin <branch>`,
  `gh pr create --base main`. Never merge, never push to `main`, never `--force`.
- **The last line you print**, alone: `WINTEST_DONE <pull request URL>`,
  `WINTEST_NOTHING`, or `WINTEST_FAILED <why>` (and commit nothing then).

## The ARM64 lane

A Windows laptop on ARM64 runs the same role with `auto-wintest.ps1 -Lane arm`:

- Its branches are `test/arm-<section>`; it waits only on its own pull requests.
- Same queue. Check the binary first: `(Get-Process tsumugi).Path` is your
  build, and its PE machine is `0xAA64` -- a run that tested the x64 build by
  accident proved nothing about ARM64.
- A row already `[x]` from the x64 machine stays as it is; record the ARM64
  result in the report under an ARM64 heading. A row that **fails** on ARM64 is
  the most valuable finding this lane can make. Tick only rows still `[ ]`.
- There is no RAM disk: the script names the scratch folder in the prompt.

## How to work

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cargo build --release -p tsumugi
cargo test --workspace --all-features
# The newer ConPTY the release ships beside tsumugi.exe; without it you test
# something nobody downloads.
pwsh -NoProfile -File scripts\fetch-conpty.ps1 -Dest target\release
```

- Work on `test/win-<section>` from the latest `origin/main`.
- **Do not bump the version or write CHANGELOG.md.** Put the changelog line in
  the pull request body, in English; whoever merges bumps the PATCH.
- **Never run `cargo fmt`.**
- A bug, or a wrong row in TESTING.md, goes in the report. Do not fix it and
  do not reword the row.
- The report is a file of its own: `qa-reports/<YYYY-MM-DD>-<branch without test/>.md`,
  with a `### Proposals` section (what should work differently, from this run:
  what you ran into, what should change, why, how big). Do not implement them.

## The thing to be most careful about

You are the only session that can mistake *running something* for *checking
something*. Before every tick ask what would have had to be **different on
screen or on disk** for you to notice a failure. If the answer is "I would have
had to look more carefully", it is a look and not yours to tick.
