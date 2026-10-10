# Testing on a real machine

Everything here is something `cargo test` cannot reach. tsumugi was written in
a Linux container: the Windows code was type-checked and its logic
unit-tested, and the window was driven on a virtual X display, but **the
Windows-only half -- ConPTY, the toasts, the taskbar, the title bar's
resizing, Mica, the sounds -- has never run.** That is what this list is
for, first; the rest is there so that what was seen once stays seen.

What `cargo test` already covers, so it is not repeated below: the protocol
and the server (sessions outliving their client, tags, restoring, the
cross-session search, stopping), the layout tree (splits, moves, swaps),
settings parsing and one-line rewriting, themes and imported schemes, the
key bindings and their parsing, the git and token parsers, the hooks'
JSON, the worktree commands, ligature shaping and the menu of numbered
choices. It runs on Windows CI on every push.

## The keys

[TESTING-KEYS.md](TESTING-KEYS.md) is a tickable line per key, generated
from `crates/tsumugi/src/keys.rs`:

```sh
cargo run -p tsumugi --example make-keycheck
```

## Working through it

[TESTING-CHECKS.md](TESTING-CHECKS.md) is this file as a tickable list, one
line per row below:

```sh
cargo run -p tsumugi --example make-testcheck
```

**This file stays the source of truth.** The generator takes the ids, the
sections and the words from here on every run; ticks (`[x]`, and `[~]` for
a look judged from a screenshot) survive. `--check` fails when either list
is out of date (CI and `scripts/verify.sh` run it), and `--stats` prints how
far each section has got -- the lists carry no totals, so two pull requests
ticking rows do not collide.

A row is ticked when it did what **Expect** says and nothing else. Ask
before ticking: *had this failed, what on the screen or on disk would have
been different?* If the answer is "I would have had to look closer", it is
a look, and gets `[~]` with the screenshot, never `[x]`.

## What you need

- A build with the newer ConPTY beside it: the **Build** workflow's
  `tsumugi-windows-x64-…` artifact, or `cargo build` and
  `pwsh -File scripts/fetch-conpty.ps1 -Dest target\debug`.
- `claude` (Claude Code) and `git` on the `PATH`; `gh` signed in for the
  pull-request rows; `lazygit` for the full-screen row.
- A git repository with a pull request open on its branch, and one with
  changes not committed.
- `TSUMUGI_KEYLOG=1` prints every key the window gets, for a key that does
  nothing (`key …`; Ctrl+X/C/V, with Shift too, come as `event Cut`/`Copy`/
  `Paste`), and `divider x,y wxh` (`divider none`) as the pointer goes onto a
  divider and off it.

## How to report

One file per run, named by the prompt (`qa-reports/<date>-<branch>-<HHmm>.md`): the rows ticked, each
with what was seen (a value read, a file's contents, a screenshot's path),
and anything that surprised. A row that failed gets what was expected,
what happened, and the steps.

## Covered by tests

These rows needed no window, so `crates/tsumugi/tests/cli.rs` runs them
against a server of its own on every push, Windows (ConPTY) and Linux, and
they are not ticked by hand any more. A change that breaks one turns CI red.

| Was | What | Test |
| --- | --- | --- |
| 1.2 | `tsumugi ls`: one line per session, its seven columns | `new_starts_a_server_and_ls_lists_the_session` |
| 1.4 (half) | `tsumugi new . -- echo hi` with no server: starts one, prints a number, `hi` runs | `new_starts_a_server_and_ls_lists_the_session` |
| 1.10 | `tsumugi new . --tag a --tag b --tag c`: all three tags | `new_starts_a_server_and_ls_lists_the_session` |
| 1.4 (words) | `tsumugi new . -- "<a folder with a space>\run.cmd" "a  b" "it's"` in pwsh (sh elsewhere): the program runs and gets each word as it was | `new_gives_the_words_to_the_program_as_they_are` |
| 2.70 (`-2`) | `tsumugi log N logs/work.txt`, `--stop`, the same again: the second file is `work-2.txt`, the first is whole; a second log while one runs and `--stop` with none are refused | `log_starts_and_finishes_a_work_log_and_never_writes_over_one` |
| 18.1 | `tsumugi ls --json`: one array, every key | `ls_json_is_one_array_of_sessions` |
| 18.2 | `tsumugi send N "…"`: typed and run | `send_runs_a_line_and_read_gives_it_back` |
| 18.3 | `tsumugi read N --lines 5` and `--all`, Japanese whole | `send_runs_a_line_and_read_gives_it_back` |
| 18.5 | `tsumugi wait`: exit 0 on the state, 1 on the timeout, 3 for a session gone (`notify` standing in for Claude Code) | `wait_and_close` |
| 18.6 (half) | `tsumugi close N`: the session ends | `wait_and_close` |
| 18.10 (half) | `tsumugi mcp`: answers `initialize` and `tools/list`; `tsumugi_sessions` says tsumugi is not running without a server, lists the session with one; `tsumugi_screen` by number and by tag | `mcp_lists_sessions_and_reads_a_screen` |

A row that only needs text belongs here, not in a section below: add a test
to `cli.rs` and a line to this table instead of a row.

## 1. The window and the server

| # | Do | Expect |
| --- | --- | --- |
| 1.1 | Start `tsumugi` with no session and nothing to restore | **Start your first session**: the folder it was started from first, the default folder, the last ones closed, **Choose another folder…**; Enter (or a click) starts the settings' program there. The window opens in under a second |
| 1.3 | Close the window, then start `tsumugi` again | The same tab and the same shell, its output still there: the server kept it |
| 1.4 | With the window closed, `tsumugi new . -- echo hi`, then start `tsumugi` | The window has a tab with `hi` in it (the number printed and the session itself: `cli.rs`) |
| 1.5 | `tsumugi attach <that number>`, and `tsumugi attach <folder name>` | The window opens on that session. A name two sessions share says so and names their numbers |
| 1.6 | With tsumugi running, `cargo build` (Windows) | The build replaces `tsumugi.exe` -- no `アクセスが拒否されました` -- because the server runs from its copy in `%LOCALAPPDATA%\tsumugi\server\` |
| 1.7 | Start the newly built window while the older server runs (an unattended run: the older release in `$env:WINTEST_OLD_EXE`, started with `Start-OldTsumugi` and its window closed; by hand, a build of 0.45.0 or 0.46.0 too) | **tsumugi was updated**, what that means in a sentence, **Restart the server** lit; Enter (no Tab or click) restarts it and every tab comes back at once, with no Welcome back, never "the server did not answer" (try it with several Claude Code sessions running); Esc closes the window and the old sessions go on |
| 1.8 | Task Manager after 1.3 | One `tsumugi-<version>-<hash>.exe` server, and no console window anywhere |
| 1.9 | The taskbar, Alt+Tab, and the window's corner | The logo (two threads, cyan and gold, on a dark tile) as the window's icon |
| 1.11 | Settings → General → Restart the server after an update without asking on; then 1.7 (an unattended run: with `$env:WINTEST_OLD_EXE`) | No question: the older server is restarted at once and every tab comes back |
| 1.12 | Build and start a newer window while the server of the build before runs, when the two speak the same version (no **tsumugi was updated**); then Settings → Advanced → Restart the server | A toast "The server is still tsumugi <old> (this is <new>): Settings → Advanced → Restart the server…", once; Settings → Advanced shows "Running · <old> · <up>"; after the restart it shows the new version and no toast comes |

## 2. Panes and splits

| # | Do | Expect |
| --- | --- | --- |
| 2.1 | `Alt+Shift++`, then `Alt+Shift+-` | A shell to the right, then one below it, each in the folder of the pane it split |
| 2.2 | `Alt+Arrows` | The keys move to the pane on that side; its cursor fills, the others' go hollow, and the others are dimmed |
| 2.3 | Bring the pointer to a gap between panes, drag | A cyan line appears, the split follows the pointer; a double-click halves it |
| 2.4 | `Ctrl+Shift+Z` with a hidden pane waiting (notify it) | One pane fills the tab, its heading says ZOOM with a small split; a gold-ringed note at the bottom right says how many wait behind and the key, and a click goes there; `Ctrl+Shift+Z` again restores the split |
| 2.5 | Drag a pane by its header onto the middle of another | A cyan outline and **Swap** while dragging; on release the two trade places |
| 2.6 | Drag a header to another pane's edge | **Move here** on that half; on release the pane goes to that side and the two share the room |
| 2.7 | Split right until a pane is narrower than 20 columns | It folds into a strip with its state's dot and its name (on its side when tall); a click gives it the keys, a drag carries it |
| 2.8 | `exit` in one pane of a split | The pane goes and its neighbour takes its room |
| 2.9 | `lazygit` in a pane, move with `j`/`k`, `?` then `Esc`, then `q` | It draws, takes the keys, its menu closes on `Esc`, and quitting leaves a working prompt |
| 2.10 | A person: type Japanese with the IME in a pane | The candidate window sits at the cursor and the committed text arrives once |
| 2.11 | On a JIS keyboard: `Alt+Shift+;` (`+`), then `Alt+Shift+-` | To the right, then below: neither is taken for the other |
| 2.12 | Drag a pane of a split by its header onto the sidebar | The sidebar lights up, **A tab of its own**, the pane's name with the pointer; dropped, the pane is a tab of its own after the one it left |
| 2.13 | One pane alone, and each pane of a split | Every pane is a card with room round it and a 30px heading: the state's mark, the name, the folder, short words on the right (none for a shell); its ring in the state's colour, the one with the keys too; the heading is not lit for the keys |
| 2.14 | A split tab, `Ctrl+Shift+I`, type `echo hi`, Enter; `Ctrl+Shift+I` again | **TYPING INTO ALL** on every pane's heading; `hi` in each; after the second press only the pane with the keys gets keys; the window's own keys (`Ctrl+Shift+T`) act once |
| 2.15 | Three panes split both ways: hover each divider, drag it, double-click it | Near it the cyan line and the resize pointer; the split follows the drag and stays where dropped (and after a restart); a double-click halves it. Unchanged from before the dividers moved into `tsumugi-layout` (v0.52.0) |
| 2.16 | Copy `echo one` + Esc `[201~` + `echo two` (e.g. `printf 'echo one\033[201~echo two' \| clip` / `pbcopy`) and paste it into bash or pwsh with bracketed paste | It lands as one line held on the prompt (`echo one[201~echo two`), not run: the Esc is dropped from a paste |
| 2.17 | Drag a divider and, still holding the button, press `Ctrl+Shift+Z` (or switch tabs with the keyboard); let go; split again | The tab shows its real split afterwards; the new split appears |
| 2.18 | A person: a German (or other AltGr) keyboard layout: `AltGr+Q` and `AltGr+7` in bash and in pwsh | `@` and `{` typed, nothing else before them (`TSUMUGI_PTY_LOG` shows only the character); `Ctrl+Alt+X` with no character still reaches emacs as C-M-x |
| 2.19 | A person: a pane in Shift_JIS (the status bar's charset), type or paste `日本😀` | `日本?` reaches the program, not `&#128512;` |
| 2.20 | Close a tab whose shell runs a deep tree (`cmd /c "cmd /c ping -t localhost"` three levels) | The window closes it within a second; every process of the tree is gone (`Get-Process ping`); nothing else is |
| 2.21 | `lazygit` in a pane: `Tab` and `Shift+Tab` a few times, then `j`/`k`; the same in bash (`Tab` completes) | Each Tab moves lazygit to its next panel and the keys stay with it; no button of the window takes them (`TSUMUGI_KEYLOG=1` prints no `focus` line) |
| 2.22 | `echo https://example.com/a?b=1 src/main.rs:3:5 ~/.bashrc`; hold `Ctrl` and move over each, click each | Each underlined only while Ctrl is held and the pointer is on it; the address opens in the browser; `src/main.rs` (from the session's folder) opens in VS Code at line 3, column 5; a missing path says **No such file** |
| 2.23 | `echo 'https://x.example/?a=1&b=2'`, Ctrl+click it; `[open] file = ""`, Ctrl+click a path | The address is not opened and a toast says why; the file opens in the system's program for it |
| 2.24 | `Ctrl+=` three times, `Ctrl+-` once, `Ctrl+0`; `Ctrl+Shift+-` in bash | Every pane's letters grow, shrink and come back to the settings' size, the grid re-fitting each time and a toast naming the size; `Ctrl+Shift+-` undoes in bash |
| 2.25 | `seq 200`, then `Ctrl+Shift+M`; `k` past the top, `v`, `j` three times, `y`; paste in Notepad | A gold box and **COPY MODE** in the pane; the output scrolls back under the box; the selection follows it; four lines on the clipboard; the mode ends and keys reach the shell again |
| 2.26 | `Ctrl+Shift+M`, `G`, `0`, `Enter`; again, `Esc`; again, then `Alt+Arrows` to another pane | The cursor's line copied; `Esc` leaves with nothing selected; moving away ends the mode |
| 2.27 | pwsh started by tsumugi (no hook in the profile, then with `tsumugi shell-hook pwsh` and Starship, whose prompt is two lines; once more in a pane narrow enough to wrap it): run `dir` five times; `Ctrl+Shift+Up` three times, `Ctrl+Shift+Down` twice | Each press moves one command: the first line of the next prompt above (below) at the top of the pane; the prompt looks as before (Starship's included), with no stray characters |
| 2.28 | The same in bash (Git Bash on Windows) with `tsumugi shell-hook bash` in the rc file, and in zsh with `zsh` where it is installed | The same jumps; `cd` still updates the pane's folder |
| 2.29 | `seq 300`, `Ctrl+Shift+F`, type `1`, then `Shift+Enter` a few times, `Enter`, then `Err` | A bar at the pane's top right; `291` (the newest `1`) selected and on screen, then older ones (up), then a newer one (down); **No match** in red for `Err`; letters typed reach the bar, not the shell |
| 2.30 | In the bar: `Enter` until it passes the oldest match; then `Esc`; then type in the pane | **Wrapped** once it starts over; `Esc` closes the bar, drops the selection and is not sent to the shell (Claude Code is not interrupted); keys reach the shell again |
| 2.31 | Open the bar, click into the pane, `Ctrl+Shift+F` again; `Alt+Arrows` to another pane | The second press puts the keys back in the bar; moving to another pane closes it |
| 2.32 | Copy three lines (`echo a`, `echo b`, `echo c`); paste with `Ctrl+V` and with a right click into `cat` (no bracketed paste) and into pwsh 7 | **Paste 3 lines?** with the lines shown; `Esc` sends nothing (nothing reaches `cat`); `Enter` (or **Paste**) sends all three |
| 2.33 | Paste the same three lines into Claude Code and into bash 5.1+; paste one line ending in a line break into `cat` | No dialog: they arrive as one paste (Claude Code shows "[Pasted text]" or the lines); the one line goes straight in |
| 2.34 | Paste 10 KB of text into Claude Code; turn off both switches in Settings → General → Copy and paste and paste again; typing into all panes, paste three lines into `cat` panes | **Paste 10 KB?** even there; with the switches off, no dialog; with all panes, the dialog says it goes to all of them and **Paste** sends it to each |
| 2.35 | `printf '\e]8;;https://example.com\e\\a link\e]8;;\e\\ and text\n'`; hold `Ctrl` over **a link**, click | A dotted line under **a link** only; with Ctrl, a solid line and a hand; the click opens example.com in the browser |
| 2.36 | Linux/macOS: `ls --hyperlink=auto` in a folder with `a b.txt`; Ctrl+click it. Then scroll the output back a few lines and Ctrl+click it again | Dotted lines under the names; the file opens (the space read right); after scrolling, the line still sits under the name and the click opens the same file |
| 2.37 | Split a tab into three: right, then right again from the new pane, then down; `Ctrl+Shift+X` twice | The pane with the keys trades places with the next one each press and keeps the keys (its ring moves with it); the shape stays; `TSUMUGI_KEYLOG=1` logs each press as `event Cut … -> Some(SwapPane)` (egui gives Ctrl+Shift+X as Cut, not as a key) |
| 2.38 | In the same tab, `Ctrl+Shift+E` | The three columns get a third of the width each; the two stacked panes half the height each |
| 2.39 | `Ctrl+Shift+J` on one of the panes; then on a tab with a single pane | The pane leaves the split for a new tab just after this one, which is shown with the keys in it; the old tab keeps the others. With one pane, nothing happens |
| 2.40 | `Ctrl+Shift+R` in a pane; run `dir` / `ls`, type some Japanese (`echo あいう`), resize the window; `Ctrl+Shift+R` again | A toast says where it records and the pane's heading says ● REC; the second press toasts "Saved …" and the mark goes. The file in `Videos\tsumugi` (Movies on macOS) holds the Japanese whole and, for the resize, an `"r"` event with the new size (`Select-String '"r"'` on it); a person can also play it with `asciinema play` |
| 2.41 | Start a recording, close the window (the server keeps running), open it again, stop it | The pane still says ● REC after reopening; the file has what happened while the window was closed |
| 2.42 | Windows, PowerShell 7 started by tsumugi: `dir`, then `dir nothing-here`, then `cmd /c exit 3` | A green bar in the left margin beside `dir` and its output; red bars beside the other two; none beside the prompt still waiting |
| 2.43 | Linux/macOS: `tsumugi shell-hook bash >> ~/.bashrc` (zsh: `~/.zshrc`), open a new pane, `ls`, `false`, `ls /nope` | Green bar for `ls`, red for the other two; scrolling moves the bars with the lines; `vim` or `less` (the alternate screen) shows no bars |
| 2.44 | After 2.42 or 2.43, and again in pwsh with Starship's prompt of two lines: `echo one; echo two`, then `Ctrl+Shift+L`, paste somewhere | The paste is `one` and `two` on two lines, without the command line or any line of the prompt |
| 2.45 | Through the mux: close the window after 2.42 or 2.43 (either: on Windows a Git Bash pane with `tsumugi shell-hook bash` in its rc does as well) and open it again | The bars come back on the same lines; `Ctrl+Shift+L` still copies the last output |
| 2.46 | Linux/macOS (WSL on Windows): `img2sixel some.png` (or `chafa -f sixel some.png`) | The picture shows below the command, at its own size (cut to the pane's width), and the prompt comes under it |
| 2.47 | `kitten icat some.png` (kitty installed), then `kitten icat --place 20x10@5x2 some.png` | The first under the command, fitted to the pane; the second at column 5, row 2 in a 20 x 10 cell box; `kitten icat --clear` takes them away |
| 2.48 | `imgcat some.jpg` (iTerm2's script) or `wezterm imgcat some.jpg` | The picture under the command, its shape kept |
| 2.49 | Windows, PowerShell 7 in tsumugi: `wsl img2sixel some.png`, and yazi in a pane with a picture selected | Both show the picture (ConPTY passes the sequences through); yazi's preview changes with the selection and leaves nothing behind |
| 2.50 | After 2.46: scroll the pane up and down, then `clear` (`cls`) | The picture moves with its lines, half shown at the top edge, cut at the pane's edges; `clear` takes it away |
| 2.51 | After 2.46: close the window (the server keeps running), open it again | The picture is back where it was; a second pane or a split shows its own |
| 2.52 | In PowerShell 7: ``Write-Host "a `e[4mu`e[0m `e[4:2mdd`e[0m `e[4:3;58;2;255;0;0mcurl`e[0m `e[4:4mdots`e[0m `e[4:5mdash`e[0m `e[9mstrike`e[0m [`e[8mhid`e[0m]"``; then in Git Bash: `printf 'a \e[4mu\e[0m \e[4:2mdd\e[0m \e[4:3;58;2;255;0;0mcurl\e[0m \e[4:4mdots\e[0m \e[4:5mdash\e[0m \e[9mstrike\e[0m [\e[8mhid\e[0m]\n'`; then close the window and open it again | In both shells: a single and a double line, a red wave under **curl**, dots and dashes under the next two, a line through **strike**, and `[   ]` with nothing between; the lines run unbroken under each word and come back the same after reopening. If either is not so, turn on **Log what each pane sends and receives** (Settings → Advanced), run both again and put the log's `out` lines for each shell in the report, with whether `OpenConsole.exe` sits beside the `tsumugi.exe` the server runs from (the ConPTY that rewrote them) |
| 2.53 | In a split, run `nvim` (or `vim` with `set autoread` and `au FocusGained * checktime`) on a file in the left pane; in the right pane `echo x >> thatfile`; click the left pane again. Then switch to another app and back (Alt+Tab) | vim reloads the file as soon as its pane is clicked, without a keypress; no stray `^[[I` / `^[[O` appears in the right pane's shell or in vim's text |
| 2.54 | In bash: `printf '\e[>q\e[c'; read -rs -d c r; echo "${r@Q}"`; then in nvim `:checkhealth` (its terminal section); then start `claude` and let it stream a long answer | The reply holds `tsumugi` and the version (`\EP>|tsumugi X.Y.Z\E\\`) and then `\E[?62;4;22`; nvim names the terminal; Claude's long answer scrolls without tearing or half-drawn lines |
| 2.55 | In bash: `printf '\e[>1u'; IFS= read -rsn7 a; printf '\e[<u'; echo "${a@Q}"`, then press `Shift+Enter`. Then in nvim (0.10 or later) insert mode `:imap <S-CR> X`, press `Shift+Enter`; quit and press `Shift+Enter` at the shell prompt. On Windows also try it in WSL and in a pane on an SSH machine | bash prints `$'\E[13;2u'`; nvim types `X` (not a new line); at the prompt `Shift+Enter` runs the line as `Enter` does. Windows's local pane gives the code too (ConPTY made it a plain Enter before); note whether WSL does |
| 2.56 | `git log --oneline -3; echo https://example.com src/main.rs:3 127.0.0.1:8080 12345`, then `Ctrl+Shift+Space` (`Cmd+Shift+Space`); type the label on the address; paste in Notepad. Again, `Shift` and the label on `src/main.rs:3` (in a folder that has it). Again, `Esc` | Gold labels on the hashes, the address, the path, the IP and the number, the bottom ones `a`, `s`, …; the address on the clipboard and the labels gone; the file opens in the editor at line 3; `Esc` leaves and keys reach the shell again |
| 2.57 | `echo hello world`; drag over `hello` and paste in Notepad. Then turn off **Copy a selection when the mouse lets go** (Settings → General → Copy and paste), drag over `world`, paste; then `Ctrl+Shift+C` (`Cmd+C`), paste | `hello` on the clipboard as soon as the button lets go; with the switch off the clipboard still holds `hello` after the drag, and `world` after `Ctrl+Shift+C` |
| 2.58 | Run `sleep 30`, select some text in the pane and press `Ctrl+C`; run `sleep 30` again, click to clear the selection and press `Ctrl+Shift+C` | Plain `Ctrl+C` stops `sleep` (`^C`) even with a selection on; with nothing selected `Ctrl+Shift+C` reaches the program as `Ctrl+C` does and stops it too |
| 2.59 | `ls -l` (`Get-ChildItem`) in a folder of a few files; hold `Alt` and drag from the size column of the first file down to the last; paste in Notepad | A rectangle is highlighted, not whole lines; the pasted text is that column, one row a line |
| 2.60 | Linux (X11 or a Wayland desktop with XWayland): select a word in a pane, middle-click in a text editor; then select a word in the editor and middle-click in the pane (in `cat`); then select three lines elsewhere and middle-click in `cat`. Windows and macOS: copy a word elsewhere, middle-click in the pane | The editor gets the pane's word, `cat` gets the editor's word (the clipboard is not changed by either); three lines ask **Paste 3 lines?** first; on Windows and macOS the middle-click pastes the clipboard |
| 2.61 | Put the README's two `[[triggers]]` in `settings.toml`; in a pane, `echo an error here; echo ERROR; echo errors` and `echo Build succeeded`; then select across `error` with the mouse | `error` and `ERROR` in red, `errors` not; `Build succeeded` on a dark green background, its text as it was; the selected part takes the selection's colour, the rest stays red |
| 2.62 | With the second trigger, `sleep 3; echo Build succeeded` in a session and switch to another app at once | A system notification **<folder>: a trigger matched** with `Build succeeded` below; the bell's list has it marked **trigger**; no flash, sound or number on the taskbar |
| 2.63 | With the second trigger, `for i in 1 2 3; do echo Build succeeded; done` in a session while another app is in front; 15 seconds later the same again | One notification for the three lines; a second after the 15 seconds |
| 2.64 | With the second trigger, `printf 'Build succeeded\n' > /tmp/t.txt; less /tmp/t.txt` (`more`) while another app is in front; then a trigger `regex = "a*"`, then `regex = "("`, then one with neither colour nor notify | `less` shows the line coloured but sends no notification; each bad trigger is reported when the settings are read (empty line, does not read, does nothing) and the old settings stay |
| 2.65 | Windows: with the second trigger, `echo Build succeeded` in a session while another app is in front; after the notification, drag the window's edge to make it narrower and wider several times | One notification only; the redraw ConPTY sends on a resize does not notify again |
| 2.66 | A trigger `regex = "^done$"` with a colour; in a pane, `echo '  done'` and `echo done.` | The first `done` is coloured (the blanks either side are not part of the line), `done.` is not |
| 2.67 | `kitten show-key -m kitty` (kitty installed) in one pane of a split; hold `a`, and while holding it `Ctrl+Tab` (or click) to the other pane, let go of `a`, then come back. Again, holding `a` and switching to another app | `show-key` shows `a` released when the keys left it (no `a` held for ever, no repeat) in both cases |
| 2.68 | Quick select with more than 26 addresses on the screen (`for i in $(seq 30); do echo https://example.com/$i; done`): type `Shift`+the first letter of a two-letter label, `Backspace`, then the label without `Shift` | The address is copied (the clipboard), no browser opens |
| 2.69 | Settings → General → Copy and paste: **Copy a selection when the mouse lets go** off. `seq 200`, drag over `1` to `3` near the top of the scrollback, wheel down to the bottom so none of it shows, `Ctrl+Shift+C` (`Cmd+C`); paste in Notepad. Then click to clear it, `sleep 30`, `Ctrl+Shift+C` | `1` to `3` on the clipboard and the shell gets no `^C`; with nothing selected `Ctrl+Shift+C` stops `sleep` |
| 2.70 | `Ctrl+Shift+S` in a shell pane; `seq 200`, `echo あいう`, a line longer than the pane; wait a second; `Ctrl+Shift+S` again. Then right-click the card → **Write a work log**, and again | A toast names `Downloads\tsumugi-<name>-<date>-<time>.txt` and the heading says LOG; while it runs, opening the file shows everything up to now, the screen included (open it again after typing: it has the new lines); `ls -l` twice, then make the window narrower and taller: nothing of the first listing is missing and nothing comes twice; the second press toasts "Saved …", LOG goes, and the file has every line once, in order, the long line whole, Japanese whole, ending with the screen; the menu does the same |
| 2.71 | Start a work log, close the window (the server keeps running), `seq 300` in that pane, open it again, `vim` something and quit it, finish the log; then close the session while another log runs | The heading still says LOG after reopening; the file has the lines printed while the window was closed, none of vim's screen; closing the session with a log running leaves a whole file with its last screen |
| 2.72 | `seq 300`, then drag from a line in the middle of the pane up past the pane's top edge and hold the button there; then drag down past the bottom edge; let go and paste in Notepad | While the pointer is above the pane the view scrolls back through older output and the selection grows with it; the farther from the edge the faster; below the pane it scrolls toward the newest; held still inside the pane nothing scrolls; the pasted text is every line from the start to where it ended |
| 2.73 | In pwsh, a picture of 100 KB or more (a `Save-Shot` PNG will do) through OSC 1337, sent with `tsumugi send N`: ``$b = [Convert]::ToBase64String([IO.File]::ReadAllBytes('big.png')); Write-Host -NoNewline "`e]1337;File=inline=1:$b`a"; 'after'``; then `tsumugi read N --lines 5` every quarter of a second | `after` and the next prompt are there within 2 s of the send (they took 10–15 s before v0.84.0); the picture shows as in 2.48, its shape kept |
| 2.74 | In pwsh, `Get-ChildItem` (some coloured names) so the prompt waits below them; `Ctrl+Shift+R`, then at once `Ctrl+Shift+R` again without typing; `Get-Content` the file and play it with `asciinema play` | The file's second line is `[0.000000, "o", …]` and holds the names and the prompt (it was the header alone before v0.85.0); played back, the screen is there from the first moment with its colours and the cursor after the prompt |

## 3. The sidebar

| # | Do | Expect |
| --- | --- | --- |
| 3.1 | Open three tabs in different git repositories | Each card: the name in bold sans-serif (IBM Plex Sans JP, else Segoe UI / the system's), then `~/folder · branch` in the terminal's font, then the state and how long in the state's colour (waiting gold, running cyan, error red, done green, probably waiting grey); one state mark, on the left |
| 3.2 | The **+** beside SESSIONS, and the one at the top of the rail | The new-session dialog opens with the focused pane's folder |
| 3.3 | Sort button → each of the five orders | The cards reorder accordingly; **Manual** keeps a dragged order across restarts |
| 3.4 | Drag a card in Manual order | The grip shows on hover; the card lands where it is dropped |
| 3.5 | The state and folder filters at the foot | Only matching cards; the heading reads `SESSIONS  N of M` (also `5 of 5` with no filter), then the order button, then **+** |
| 3.6 | Right-click a card: rename, tag, mute, pin, restart, duplicate, new window, open in editor / filer, copy path, close | Each does what it says; close is red and last; Rename, Duplicate and Close show their keys on the right, in grey |
| 3.7 | Sort → One line each; then `Ctrl+Shift+B` | One line per tab; then the 60px rail with first letters and state rings |
| 3.8 | Rest the pointer on a card of another tab | Its last 12 lines in a box, the waiting pane's in a tab of several |
| 3.9 | A tab in a repository with uncommitted changes | `+N −M` (or `N new`) on the card's second line; on hover, the files as `git status` lists them |
| 3.10 | Drag the sidebar's edge left past 120px, and back | It becomes the rail; dragged back out, the sidebar |
| 3.11 | One session of each state (notify them) and a plain shell | Each card ringed in its state's colour with its own mark -- a clock (waiting, breathing), a turning arc (running), a dotted circle (probably waiting), a triangle (error), a tick (done, its ground sunken) -- and the shell grey with a small dot and no words; the foot's filters and the status bar count the shell as Shell, not Running |
| 3.12 | Two waiting, then `[keys] next_waiting = "F8"` | Under a line at the foot: the key as a cap (`F8` once changed) and **Jump to waiting · 2** in grey; a click on either jumps |
| 3.13 | Theme tsumugi Light (and each light theme) | The waiting and error words and counts read clearly on white (WCAG 4.5 or more) |
| 3.14 | A tab on a branch with an open pull request (`gh` signed in) | `· PR #N` after the branch on the second line, green when its checks pass, red when they fail, cyan while they run, grey with none; its tooltip says which |
| 3.15 | The card of the tab with the keys, Claude Code talked to a few times | A line `N prompts · 12m · 3.4k tokens`; the other cards have none. A running card's third line ends with `· N tokens` |
| 3.16 | `F2` | A field on the card with the name selected; Enter renames, Esc leaves it as it was |
| 3.17 | `Ctrl+Shift+D` (macOS `Cmd+Option+D`) in a Claude Code pane, then in a shell | A new tab in the same folder: `claude` typed in the first, a shell in the second |
| 3.18 | Look at the sidebar on first start | 288px wide |
| 3.19 | Right-click a card → Note…, type `the release`, Enter; restart the machine (or the server) | `“the release”` on the card under the folder; Edit the note… and an empty Enter takes it off; after the restart the note is back |
| 3.20 | A tab with uncommitted changes: click its `+N −M` (or search `changes`) | The diff over the window: each file under its name, added lines green, removed red, hunks cyan, new files at the end; Esc or a click outside closes it; a lock file's large diff is cut at 512 KB |
| 3.21 | Right-click a card → Save the output to a file (or search `save the pane`) | A toast names `Downloads\tsumugi-<name>-<date>-<time>.txt`; it holds the whole scrollback as the program wrote it, Japanese whole, long lines unbroken |
| 3.22 | A tab on a branch other than main (a worktree's), right-click → Create a pull request (`gh` signed in) | The branch pushed, a pull request made from its commits and opened in the browser; a toast with its address; on `main` the item is not there; without `gh`, a toast says why |
| 3.23 | Run `codex` (or `gemini`, `opencode`) in a session | The card is an agent's, not a grey shell: its name (`codex`) on a line under the folder, its states shown; on Windows, Gemini CLI (run by node) stays a shell |
| 3.24 | `npm run dev` (or `python -m http.server 8123`) in a session | `:3000` (`:8123`) on its card within a few seconds; a click opens `http://localhost:3000` in the browser; gone when the server stops |
| 3.25 | A card with tags and a PR (and a note), a card with neither | The extra lines (note, agent and ports, numbers) under the tags, or under the third line; none over another |
| 3.26 | Claude Code working (its title `✳ …` or a braille spinner), and pwsh | The card's and pane heading's name without the `✳` or spinner: one mark only, the card's own on the left |
| 3.27 | Click a waiting card, then a running one, then a shell | The card's ground stays dark (no fill): its ring in its state's colour, brighter, with a soft glow past it (cyan for a shell); a running card's line still runs along its top edge |
| 3.28 | A plain shell's card beside an agent's | The shell's card is a line shorter: no empty third line |
| 3.29 | Claude Code working in a tab: `F2`, type, `Esc`; right-click → Note…, type, `Esc`; and `F2` on a tab, then pick a tag filter that hides it | Each field closes as it was and Claude Code keeps working (no Esc reached it, `TSUMUGI_PTY_LOG` has no `in key` line); a hidden card's field is let go and the pane gets keys again |
| 3.30 | With the Japanese IME on: right-click a card → Note…, type `にほんご`, convert with Space, Enter to commit, Enter again; the same in `F2`'s field, the menu's Rename… and the search box (`Ctrl+Shift+P`) | The reading stays underlined as it is typed and the candidates come up; the first Enter only commits the conversion, the second keeps the note (or name); `日本語` arrives whole, nothing lost or doubled |
| 3.31 | Sort → Folder with three running, one waiting and one done tab in a folder | Every tab under its heading as a full card, none as one line, no "+ N more"; a click on the heading closes and opens it |
| 3.32 | Open tabs until the sidebar is full (about 15), in each order; then make the window shorter | All full cards while they fit; past that the last ones turn into one-line rows from the bottom up, the tab shown always a card; no scrollbar; taller again, they are cards again |
| 3.33 | Windows, a profile with no tsumugi hook: rules `c:\dev\filer` → `filer` and `c:\dev\tsumugi` → `tsumugi`; in a pwsh tab `cd c:\dev\filer`, then `cd c:\dev\tsumugi`, then back; add a tag `mine` by hand | The card wears `filer`, then `tsumugi` in its place, then `filer` again; `mine` stays throughout; a profile's own hook (mise, `tsumugi shell-hook`) still runs |

## 4. States, notifications and answering

| # | Do | Expect |
| --- | --- | --- |
| 4.1 | `claude` in a pane, ask it to do something that needs permission | The card turns gold with **Waiting for you** and Claude's words |
| 4.2 | With the hooks (12.x), Claude finishing a reply | The card turns green, **Done** |
| 4.3 | The waiting card's `1 Yes` button, without going to the tab | Claude goes on, as if `1` were typed there; the card leaves the waiting state |
| 4.4 | A card waiting with no menu on screen | No buttons |
| 4.5 | Another window in front while a session starts waiting | A Windows toast titled `<folder> is waiting for you`, the work's name and words below, with **Open** and **Later**; a second notice from the same session replaces it in the Action Center |
| 4.6 | Click the toast, or Open; then Later on another | tsumugi comes to the front on that pane; Later only puts the toast away |
| 4.7 | The taskbar button while two wait, then one fails | A gold `2` with dark digits; red with white once one has failed; it clears when the window is looked at |
| 4.8 | Settings → Notifications: flash and sound on for waiting, then 4.5 again | The taskbar button flashes and the system's message sound plays once (the error sound for an error) |
| 4.9 | Right-click a tag chip → mute | Sessions with the tag tell only in the bell |
| 4.10 | The bell, right of the search box at the top; then `Ctrl+Shift+N` (macOS `Cmd+Shift+N`) twice in a shell | The list of notices under it, newest first, its names without Claude Code's `✳`; a click goes to the session and marks it read; no bell in the sidebar's heading. The key opens the same list and closes it, and nothing reaches the shell |
| 4.11 | `printf '\e]9;hello\a'` in a pane | Marked waiting with `hello`, no hooks needed |
| 4.12 | `Ctrl+Shift+U` with two waiting | The one waiting longest first, then the other |
| 4.13 | Two Claude Code sessions asking permission, then `Ctrl+Shift+Y` (or **List ›** at the sidebar's foot) | A panel: each waiting session with what it said, how long, its choices as buttons and a tick; a choice's button types its number there |
| 4.14 | In that panel, **Yes to 2** | Both go on as if `1` were typed in each; untick one first and only the other is answered; **No to N** types each menu's "No" choice |
| 4.15 | A session waiting with a menu of other words (no "Yes"/"No") | Not counted in Yes to N / No to N; its own buttons still work |
| 4.16 | `exit` a Claude Code session after some work; then the panel's **Recently closed** (or search `recently`) | It is listed: today at …, done, its tokens, and its last lines without a click (all of them on the picked one, two on the rest); `Up` / `Down` move the pick; `Enter` (and a double click) opens a new tab in its folder typing `claude --resume <id>`; `Delete` takes the picked one off and the pick stays in place, and a note at the bottom offers **Undo** for eight seconds (the button or `Ctrl+Z`), which puts it back in its place (several deletes, or **Clear the list**, come back together); the list survives a restart of tsumugi |
| 4.17 | A Claude Code session asking to run a Bash command; `Ctrl+Shift+Y` | Under what it said: "Bash command", the command and its description, "Do you want to proceed?", in the terminal's font, before the buttons |
| 4.18 | A Claude Code session asking to run one Bash command → `Ctrl+Shift+Y` → **Always allow…** → **Add and say yes** | Before: the rule `Bash(<command>)` and the file named; after: the project's `.claude/settings.local.json` has it under `permissions.allow` (the old file as `.tsumugi-backup`), `1` typed, a toast; next time Claude Code runs it without asking. No **Always allow…** for an edit or a command of several lines |
| 4.19 | `[notify] webhook = "https://ntfy.sh/<a topic>"`, `webhook_after = 30`; a session waiting; minimize (or close) the window | Within a minute the phone (ntfy app on the topic) shows `<folder> is waiting for you` and what it said; once, not again for the same wait; nothing while the window is looked at; a muted session never |
| 4.20 | `webhook_format = "slack"` with a Slack incoming webhook | The message in the channel, the title in bold |
| 4.21 | Settings → Notifications → WHAT IT COSTS → Tell when the day costs → Past $5, with Claude Code's day already past $5 (or `[prices]` raised to get there) | `spend_day = 5` in the file; at once a toast `Today's Claude Code use passed $5: about $N at API prices`, and with the window not looked at the system's notification too; not again that day, again the next |
| 4.22 | Tell when a 5-hour block costs → Past $5, the block past it | The same once for the block (`This 5-hour block passed $5`); again only in the next block |
| 4.23 | Start `claude` in a session and type nothing for half a minute (with a plugin such as claude-mem printing at start too) | After about 10 s (`[sessions] quiet`) the card reads **Quiet for … · probably waiting**, not **Running**; typing a prompt makes it **Running** again |
| 4.24 | With a webhook set, a waiting session whose note starts with `@` (`tsumugi notify --session N "@C:\Windows\win.ini"`), the window not looked at | The phone gets the text `@C:\Windows\win.ini`, not the file's contents |

## 5. Search

| # | Do | Expect |
| --- | --- | --- |
| 5.1 | `Ctrl+Shift+O`, type part of a session's name (then a tag as `#tag`, then a word from a card's last lines) | Only the cards that match, that session first; `Down` / `Up` move, Enter goes there |
| 5.2 | Type a folder another session is in | **Folder** entries: Enter starts a session there |
| 5.3 | Type `split` | The commands, with their keys beside them |
| 5.4 | `Ctrl+Shift+O`, type three letters printed long ago in another tab's scrollback | **IN THE SCROLLBACK** lines under the cards a moment later; `Down` walks the cards then the lines; Enter (or a click) on one goes to that tab, scrolls back to the line, the match selected |
| 5.5 | `Esc`, and a click outside | Closes without doing anything |
| 5.6 | Keep a prompt `Say {project}` in the input box's **Prompts…**; in the search box type `send`, pick **Send prompt: …**; then `Ctrl+Shift+I` in a split tab and pick it again | It is sent to the pane with the keys, the project's name in it; the second time every pane of the tab gets it |
| 5.7 | A tab of three panes (Claude Code, a shell beside, one below), dividers moved; **Save this tab's layout**; close it; **Open layout: …** from another pane | `layouts.toml` beside the settings has it; a new tab opens in that pane's folder with the same splits and shares, Claude Code and the shells started as they were |

| 5.8 | `Ctrl+Shift+O` with sessions waiting, running and done, on a 1280 × 800 window; arrows, `Enter`; a click on another; `Ctrl+Shift+O` again; then `Ctrl+Tab` twice, `Ctrl+Shift+Tab`, `Right`, `Left` | **All sessions** across most of the window, several rows at once, with the waiting first, each row's state, folder, branch, tags, tokens and last two lines; Enter and the click go to that session; the key closes it; the keys turn to Waiting, Recently closed, back to Waiting, and so on, round the three |
| 5.9 | Give the tab with the keys five tags, `[tags] shown = 3`; make the window narrower step by step; move the keys to a tab without tags | The search box and the bell stay in the middle of the band; three tags and `+2` on the right, fewer and a bigger `+N` as it narrows, never over the box; the box shrinks to its magnifier last |
| 5.10 | `F1` on a 1280 × 800 window; then a 1000 × 700 one; `F1` again, `Esc`, a click outside | Every key by kind, in the middle of the window, in three columns or more (smaller letters on the smaller window), all of it with no scrollbar; a key moved in the settings shows its new key; each closes it |
| 5.11 | `Ctrl+Shift+O`, type `abc`, then `Left` and `Right`; clear the box, `Left` and `Right` again; type `zzzzqqq` | While typing the arrows move in the box and the page stays All; with it empty they turn the page; the last says Nothing matches |
| 5.12 | `Ctrl+Shift+P`, type part of a session's name, then `work` | No session or scrollback lines in the palette (its hint points to Ctrl+Shift+O); `work` shows **Work log …** with `Ctrl+Shift+S` beside it |

## 6. New sessions

| # | Do | Expect |
| --- | --- | --- |
| 6.1 | `Ctrl+Shift+T`, Enter | Claude Code starts in a new tab in the focused pane's folder |
| 6.2 | `Ctrl+Shift+T`, Shell, `Alt+Enter` | A shell split to the right instead of a tab |
| 6.3 | Type part of a folder, `Tab` | Completes from the folders listed, including **closed** ones from sessions that ended; the cursor at its end; a second `Tab` moves on |
| 6.4 | Add a tag, and take a dashed (folder rule) tag off | The new session wears exactly what the dialog showed |
| 6.5 | Beside it → + Pane twice (Claude Code, Shell), Create | One tab of three panes: the first, one to its right, one below that |
| 6.6 | Tick Save as a profile, name it; next time Profile… → it | The folder, start, tags and panes come back |
| 6.7 | Tick In a new git worktree in a repository, Create | A folder `<repo>-tsumugi-MMDD-HHMM` beside the repository on its own branch, and the session in it; a toast says so |
| 6.8 | `exit` the last session in that worktree | **Remove the worktree?**; Remove takes the folder (the branch stays); with uncommitted changes git refuses and the toast says why |
| 6.9 | `Ctrl+Shift+T`, then only the keyboard: `Tab` through the dialog, `←`/`→` on the ways to start, `Space` on a button, `Shift+Tab` back | A cyan ring shows where the keys are; Tab completes the folder once and then moves on to the way picked in START (one stop for the whole row), then + Pane, the tag, the two ticks, Create and Cancel; `←`/`→` walk the row round, Profile… included, and Shift+Tab from + Pane comes back on the way picked; `Enter` on Cancel cancels, `Enter` in a field creates |
| 6.10 | Search `parallel` → Start in parallel; two prompts; Start 2 | Two folders `<repo>-tsumugi-MMDD-HHMM-1`/`-2` beside the repository on branches `tsumugi/MMDD-HHMM-1`/`-2`, two tabs, Claude Code started in each on its own prompt (quotes in a prompt kept); Esc or Cancel starts nothing |
| 6.11 | `Ctrl+Shift+T`, `Tab` to Shell (the ring on it), `Enter` | A shell starts, as Alt+Enter would split one: Enter on a way to start picks it and creates |
| 6.12 | With WSL installed: `Ctrl+Shift+T`, Runs on → WSL: Ubuntu, Shell, Create | A Linux shell in the same folder (`/mnt/c/…`); Docker Desktop's distributions are not offered; no console window flashes when the dialog opens |
| 6.13 | With a `Host box` in `~/.ssh/config`: Runs on → SSH: box, Claude Code, tick Save as a profile, Create | `ssh box` starts in the pane (asks for a password or key if it needs one) and `claude` is typed once logged in; `Host *` lines are not offered; Profile… → it next time picks SSH: box again |
| 6.14 | With tsumugi on `box`'s `PATH`: `Ctrl+Shift+T`, Runs on → SSH: box, kept there, Shell, Create | A shell on box's own server in its home folder; the sidebar says `on box`, the title starts `[box]`, MACHINES lists This machine and box with their counts |
| 6.15 | Then click This machine in MACHINES, and box again | This machine's sessions (or the first-run screen, with none of box's folders in it) and title `tsumugi`; box's come back as they were |
| 6.16 | While box is shown: `Ctrl+Shift+T`, Runs on → This machine, Create | The window goes to this machine and starts the session there |
| 6.17 | While box is shown, cut the network (or stop its `sshd`) | A toast says the line dropped; box's row turns red with **Reconnect**; hovering it while ssh fails shows ssh's error; once box answers, Reconnect brings the same sessions back with what they printed meanwhile |
| 6.18 | Leave the window on its first-run screen over two minutes, then go to box and back | This machine's server is still running (no "not connected" mark); if it was stopped, going back to This machine starts it again |
| 6.19 | After 6.14, close the window and open it again | MACHINES lists box again (reached, not shown) with its counts; the settings file has `hosts = ["box"]` under `[remote]`; right-click box → Forget takes it off the list and out of the file |
| 6.20 | With box not shown, make one of its sessions wait (a Claude Code question), then put the window behind another | A toast "box: 1 waiting. Show it from MACHINES in the sidebar", box's row says `1 waiting`, and a Windows notification says the same while the window is behind |
| 6.21 | With a host where tsumugi is not installed: Runs on → SSH: it, kept there, Create; hover its red row | The tooltip says tsumugi is not there, gives the releases page and Settings → Advanced → Command there; filling that with tsumugi's full path there and Reconnect reaches it |
| 6.22 | With an older tsumugi running as box's server: reach box; right-click → Restart its server | Before: the tooltip says the versions differ and to install the same one, then restart; after: box comes up on the new server with its tabs back (its sessions restarted) |
| 6.23 | Over a slow line (a phone's hotspot or a far host), run `yes` and show a picture in a session on box | The window stays responsive, the pane updates in steps of about a tenth of a second, the picture comes through (shrunk if large) |

## 7. The input box

| # | Do | Expect |
| --- | --- | --- |
| 7.1 | `Ctrl+I`, write two lines (Enter between), `Ctrl+Enter` | The box opens inside the focused pane's card, at its foot, the terminal shortened above it; both lines reach the session as one prompt, then Enter |
| 7.2 | `↑` in the empty box | The last prompt sent; `↓` back |
| 7.3 | Drop two files on the window | Chips in the box; sent, their paths follow the text, quoted when they have spaces |
| 7.4 | Pick a tag as **To**, send | Every session wearing the tag gets it |
| 7.5 | While a session runs, **When done** (or `Ctrl+Shift+Enter`) | `1 queued` on the card and in the box; when the session is done, the prompt goes and a toast says so |
| 7.6 | Queue to a session showing a permission menu | It waits until the menu is answered and the session is done |
| 7.7 | `Esc`, then type `echo ok` and Enter | The box closes and the keys go back to the pane; the draft is kept for next time; the Esc does not reach the shell (`echo ok` runs whole), nor Claude Code (it is not stopped) |
| 7.8 | Take a screenshot (`Win+Shift+S`), `Ctrl+V` in the box | A chip `▣ paste-<date>-<time>.png · N KB`; the PNG is in the settings folder's `attachments`; sent, its path follows the text |
| 7.9 | `Ctrl+V` with text on the clipboard | The text is pasted; no chip |
| 7.10 | Drop a file of a few hundred KB | Its chip says its size, `▤ name · 214 KB` |
| 7.11 | Split the tab, open the box, move the keys with `Alt+Arrows` | The box follows the pane with the keys; a pane narrower than 20 columns has none |
| 7.12 | **+ Sessions** in the box, tick another session, `Esc`, write a prompt, `Ctrl+Enter` | `+ name` beside the pane under To; the menu's Esc does not reach the shell; both sessions get the prompt; a click on `+ name` takes it off |
| 7.13 | Write `Run the tests in {project} on {branch}`, **Prompts…** → name it, Save; empty the box, **Prompts…** → it, `Ctrl+Enter` | The menu stays open while the name is typed; `prompts.toml` beside the settings holds it; picked, it fills the box and the keys are back in it; the session gets the project's and branch's names in place of the braces; with **+ Sessions**, each its own |
| 7.14 | Open **Prompts…** or **+ Sessions**, press `Esc`, type | The menu closes, the box stays open with the keys; nothing reaches the shell |
| 7.15 | Queue a prompt for a session in another tab that is running Claude Code; let it stop on a permission question (`1. Yes / 2. No`) | The prompt stays queued while the question is up; it is sent only once the question is answered and the session waits with no question |
| 7.16 | In the input box of session A press `↑` (a past prompt shows), switch to session B and press `↑` / `↓`; then `Shift+↑` in a two-line draft | B's own draft is never replaced by A's; Shift+↑ selects in the draft, it does not walk the history |

## 8. Restoring after a restart

| # | Do | Expect |
| --- | --- | --- |
| 8.1 | Split tabs with Claude conversations, then reboot (or stop the server) and start tsumugi | **Welcome back** lists the tabs; Restore brings their splits and folders back |
| 8.2 | A restored Claude Code pane | `claude --resume <id>` was typed: the conversation is back |
| 8.3 | Settings → General → On start → Restore the last sessions, then 8.1 | No Welcome back; the tabs simply return |
| 8.4 | 8.1 with fourteen or more tabs | The list scrolls; Restore and Start fresh stay in sight; the time reads `today at …` or `yesterday at …` |
| 8.5 | A session where `codex` ran; restart the machine (or the server) | The restored tab types `codex resume --last`; with `[agents.gemini] resume = "…"`, a Gemini session types that |
| 8.6 | 8.1 with some sessions that had finished (`exit` in them first); click **Select all**, then again, then tick one row by hand | The box above the rows is half-filled while some are ticked; the first click ticks every row (the finished ones now say `new shell in …`) and Restore counts them all; the second clears them all and Restore is greyed; one row ticked makes the box half-filled again |

## 9. The settings screen

| # | Do | Expect |
| --- | --- | --- |
| 9.1 | `Ctrl+,` | The screen with its nine pages and the search field over them; the band says Settings in the middle with an X; no sidebar or status bar; `Esc` and the X close it |
| 9.2 | Change something on each page | Only that line of `settings.toml` changes (diff the file); comments stay. Language, the clock and the theme change that line of `common.toml` in the uchmk folder instead |
| 9.3 | Write a mistake into `settings.toml` by hand | A red line above the status bar says what and where, until it is fixed; nothing else changes |
| 9.4 | Open settings.toml / Open the settings folder | The system's editor / file manager opens |
| 9.5 | Type `scroll` in Search settings | Only Advanced in the list, with a count; its Scrollback row lit; Enter goes there. `copy` lists General (**Copy a selection when the mouse lets go**) |
| 9.6 | Next to the design's "Settings: every page" | The same pages, sections and rows in the same order |
| 9.7 | Only the keyboard: `Ctrl+Tab` / `Ctrl+Shift+Tab` through the pages, `Tab` through a page, `Space` on a switch, `Esc` twice | A cyan ring on the control with the keys; Tab never stops on the top band or the list of pages; Space flips the switch (the file changes); the first Esc leaves the control, the second closes the screen; nothing typed reaches the shell behind it |
| 9.8 | Notifications → WHEN A SESSION… | A line between rows; the three state columns the same width, each switch in the middle of its column and row, under its heading's dot |
| 9.9 | `Ctrl+,` (macOS `Cmd+,`) with the settings open | They close and stay closed (not reopened on their first page) |

## 10. Themes

| # | Do | Expect |
| --- | --- | --- |
| 10.1 | Settings → Theme, walk the thirteen | The window and the panes recolour at once; states stay readable (gold, cyan, red, green) |
| 10.2 | `theme = "system"` in `common.toml` (`%APPDATA%\uchmk`), then switch Windows between dark and light | tsumugi follows within seconds |
| 10.3 | Save a Windows Terminal scheme `.json` in `themes\` | It is in the list under its name; chosen, `ls` colours match Windows Terminal's with that scheme |
| 10.4 | The same with an iTerm2 `.itermcolors` | As 10.3 |
| 10.5 | `theme.toml` with one colour | Only that colour changes in the theme in force |
| 10.6 | Settings → Theme, in Follow OS | Mode (Follow OS, Light, Dark) at the left under the heading; only the theme shown now is lit, it and the other kind's pick are named `when dark` / `when light`; PREVIEW names the theme in force; a line with the font, the window and motion, a click goes to Appearance; each theme's swatches bordered and rounded |

| 10.7 | With tsumugi and mimamori open, write `theme = "Nord"` into `common.toml` by hand; then pick Dracula in tsumugi's Settings → Theme | Both apps turn Nord within a couple of seconds, then both turn Dracula; `settings.toml` is unchanged |
| 10.8 | An older `settings.toml` with `theme = "Nord"` and no `theme` in `common.toml`; start tsumugi | Nord, as before; Settings → Theme shows Nord lit. Picking another writes it to `common.toml` |
| 10.9 | Save a theme `.toml` (with `name = "Mine"`) in `%APPDATA%\uchmk\themes\` | Mine is in Settings → Theme's list within a couple of seconds, in tsumugi and mimamori alike |

## 11. Fonts

| # | Do | Expect |
| --- | --- | --- |
| 11.1 | Settings → Appearance → Font list | The installed monospace fonts (Cascadia, Consolas, any Nerd Font) |
| 11.2 | Pick Cascadia Code; `printf '\e[1mbold\e[0m \e[3mitalic\e[0m'` | The pane in Cascadia; bold and italic in their own faces; the line under the list names the files found |
| 11.3 | Consolas | Its bold and italic found too (`consolab.ttf`, `consolai.ttf`) |
| 11.4 | Size and line height: type a value, Enter | The grid re-fits; text in the middle of taller rows; `tsumugi ls` shows the new columns × rows after a moment; `Esc` in the field leaves it as it was |
| 11.5 | Cascadia Code or Fira Code, `echo '-> != == >= => |> www'` | Each shown as one sign, on the grid; Ligatures off draws them as plain characters |
| 11.6 | Japanese text and a Nerd Font icon in a prompt | Japanese from the system font, icons from the Nerd Font, neither as boxes |

## 12. Claude Code's hooks

| # | Do | Expect |
| --- | --- | --- |
| 12.1 | Without the hooks in `~/.claude/settings.json`, start tsumugi | On the first-run screen, the gold-ringed card with the two hooks under the folders; with sessions there already, a short card at the bottom right, above the input box when it is open |
| 12.2 | Add the hooks | Both in the file beside any there, each the full path of this tsumugi (`C:/…/tsumugi.exe notify …`, in `'…'` when the path has a space), `settings.json.tsumugi-backup` next to it, and a toast |
| 12.3 | Not now / Don't ask again | Asked again at the next start / never again |
| 12.4 | Settings → Shell & hooks → Claude Code hooks → Add | Added, the row says Installed; Remove takes only tsumugi's out, the backup beside it |
| 12.5 | `notify = ["tsumugi", "notify", "--state", "done"]` in `~/.codex/config.toml`; ask Codex something in a session | When its turn ends the card turns done (green) with the first line of its answer |
| 12.6 | Windows PowerShell 5.1: a `$PROFILE` saved as UTF-16 (`"# mine" \| Out-File $PROFILE`), then Settings → Shell & hooks → Shell integration → Install | A toast says the profile is not UTF-8 text and is left as it is; the profile is unchanged (its hash the same) |
| 12.7 | In `~/.claude/settings.json`, tsumugi's hooks as the bare `tsumugi notify --stdin` / `tsumugi notify --state done`, with tsumugi not on the `PATH`; start tsumugi | A toast says the hooks could not find tsumugi and run this one now; both commands are this tsumugi's full path, the old file is `settings.json.tsumugi-backup`; Claude Code in a session ends its replies with no "Stop hook error" and the card turns done |
| 12.8 | With the hooks in, Claude Code in a terminal that is not tsumugi (Windows Terminal); let it finish a reply | No "Stop hook error", and Claude does not carry on by itself |

## 13. The window's frame

| # | Do | Expect |
| --- | --- | --- |
| 13.1 | Look at the window (Windows) | No system title bar: the band carries minimize, maximize and close on the right |
| 13.2 | Drag the band's empty part | The window moves; drag it to the screen's top: it maximizes (Aero Snap) |
| 13.3 | Double-click the band | Maximizes; again, restores |
| 13.4 | Each of the three buttons | Minimize, maximize/restore (its icon changes), close; close turns red under the pointer |
| 13.5 | Each edge and corner | The pointer changes and dragging resizes; not while maximized |
| 13.6 | Hover the maximize button (Windows 11) | Note whether Snap Layouts appear (they may not: the button is tsumugi's own) |
| 13.7 | Settings → Appearance → tsumugi's own title bar off | The system's frame comes back at once; on again, gone |
| 13.8 | Material → mica, restart | The desktop's colour through the band, sidebar and status bar; the panes stay solid |
| 13.9 | Material → acrylic, restart | A blurred desktop instead; moving the window may lag (Windows' own limit) |
| 13.10 | macOS: Material → vibrancy, restart | The sidebar's frosted material behind the band and sidebar; the sidebar runs up to the window's top and the traffic lights sit on it; the band starts to its right, and its empty top drags the window |
| 13.11 | Linux on Wayland: tsumugi's own title bar off | The system's frame (Adwaita) with its buttons, moving and resizing the window; no title text on it |
| 13.12 | Settings → Appearance → Opacity 80, restart; then 60 without restarting | The desktop shows through the whole window -- band, sidebar, panes and the gaps between them -- evenly, no darker bands where the panes are; text stays solid (a reversed status line in vim or htop too); 60 thins it at once |
| 13.13 | Opacity 80 with Material mica (Windows 11) or vibrancy (macOS), restart | The material behind the chrome as before, the panes see-through to it; nothing goes fully clear at a pane's corners |
| 13.14 | Background image → a large png or jpeg (`~/…` and a name beside settings.toml); Image strength 25, then 60 | The picture fills each pane cut to its shape (not stretched), faint behind the text; 60 brings it forward; a pane split or resized refits it; a name that is not a picture toasts why; empty takes it away |
| 13.15 | Settings → Appearance → Quake mode key `` Ctrl+` `` (no restart); go to another program and press it; press it again; again from another program | The window comes down at the top of the screen, the screen's width and half its height, with the keys; the second press hides it, off the taskbar (Dock stays on macOS); the third brings it back with the sessions as they were |
| 13.16 | Quake mode key set to a key another program holds (or `Ctrl+Nothing`) | A toast says it could not be taken (or is not a key); the window works on |

## 14. Motion

| # | Do | Expect |
| --- | --- | --- |
| 14.1 | A session waiting | Its card's gold ring brightens and fades every 2.4 s, a glow outside it |
| 14.2 | A session running | A thin cyan line crossing the top of its card every 1.8 s |
| 14.3 | Switch tabs; split | The panes fade in over a moment rather than appearing at once |
| 14.4 | Settings → Appearance → Animations off | All of it still; CPU idle |
| 14.5 | Task Manager with one waiting card, window focused and then behind another | A few percent CPU at most in front, less behind (the window repaints 30 times a second only while something moves, 10 when not looked at) |

## 15. The status bar

| # | Do | Expect |
| --- | --- | --- |
| 15.1 | Look at it | 28px tall: server uptime and counts per state, a divider, the folder (monospace) and branch of the pane with the keys, a divider, its program and size, the clock; the clock's tooltip gives the day and date, the program's says what each part is |
| 15.2 | Commit without pushing in the focused repository | `↑1` in gold within half a minute |
| 15.3 | A branch with an open pull request (`gh` signed in) | `PR #N` coloured by its checks; a click opens it in the browser |
| 15.4 | A Claude Code session focused | `N tokens · Today M`; the tooltip breaks it down |
| 15.5 | Rest the pointer on `UTF-8` | Says the panes read and write UTF-8, and that ConPTY turns any console program's output into it; `chcp 932` and a Japanese `dir` in cmd still read right |
| 15.6 | Claude Code used in the last hours | `5h N · resets HH:MM` beside the tokens; the tooltip gives the window's start and end, what is left, and that the limit is the plan's |
| 15.7 | Linux or macOS: click `UTF-8` → Shift_JIS; `cat` a Shift_JIS file; type Japanese into `cat > x.txt`, then `nkf -g x.txt` (or `file`) | The menu of eight; the file reads right; `Shift_JIS` on the pane's heading and in gold in the status bar; what was typed is Shift_JIS in the file; restart the machine: still Shift_JIS |
| 15.8 | Windows: the `UTF-8` in the status bar | Only a label with its tooltip: no menu |
| 15.9 | Claude Code used today (Opus 5.5, say) | `≈$0.39` after the conversation's tokens and today's, and in the 5h window; the tooltip says it is the API's price; `[prices."claude-opus-5-5"] input = 8.0` doubles the input's share; a closed session's line in Recently closed has its own |
| 15.10 | Settings → General → CLOCK: Show the date off, then 12-hour; with mimamori open | The status bar's clock drops the date, then reads `2:32 PM`; `common.toml` has `[clock] date = false` and `hour24 = false`; mimamori's clock does the same |

## 16. Keys

| # | Do | Expect |
| --- | --- | --- |
| 16.1 | Settings → Keys → click New session, press `Ctrl+Shift+K` | `[keys] new_tab = "Ctrl+Shift+K"` in the file; `Ctrl+Shift+K` opens the dialog; `Ctrl+Shift+T` goes to the shell |
| 16.2 | Its own | The key goes back; the search box and the dialog name the key in force |
| 16.3 | `zoom = "none"` | `Ctrl+Shift+Z` reaches the shell |
| 16.4 | Press only Ctrl while a key is being changed | Nothing is written until a real key comes |
| 16.5 | Settings → Keys, each section | Every row's key button lines up in one column at the right, also after a key is changed; the VIEW section has Keys (F1), the overview and the font sizes |

## 17. What the settings' rows do

| # | Do | Expect |
| --- | --- | --- |
| 17.1 | General → Start the server at sign-in on; sign out and in | `tsumugi ls` answers before the window is opened; the `tsumugi` value under `HKCU\…\Run` (macOS: the LaunchAgent; Linux: `~/.config/autostart/tsumugi-server.desktop`); off takes it away |
| 17.2 | Keep sessions running off; close the window with a shell open | `tsumugi ls` is empty afterwards |
| 17.3 | Ask before closing on; `sleep 100` in a pane; close the window | "A session is still running"; Cancel keeps the window; Close closes it |
| 17.4 | Check for updates on, with a build older than the latest release | A toast names the newer version, once a start |
| 17.5 | Default folder `~/dev`; start with no session | The first shell starts in `~/dev` |
| 17.6 | Appearance → Cursor, each of the six | The focused pane's cursor is a block, a bar or a line under, blinking when asked; a pane without the keys shows an outline |
| 17.7 | Nerd Font icons off, with a Nerd Font installed | The branch mark is drawn, not the font's icon |
| 17.8 | Keys → New session → press `Ctrl+Shift+W`, then `Ctrl+C` | The row says whose each is; nothing written; another key, or Use it anyway, writes it |
| 17.9 | `Alt+Shift+Arrows` in a split | The nearest divider moves that way a step at a time; the split is kept after a restart |
| 17.10 | Notifications → ▶ beside each sound, each of the four | Four different system sounds |
| 17.11 | Focus mode on in Windows; a session waits while the window is behind | No sound, flash or toast; the taskbar number still comes |
| 17.12 | Tell about a finish → 5 min; a 1-minute command finishes behind | No notice for it |
| 17.13 | Sessions → Default program → Shell | The new-session dialog opens with Shell picked |
| 17.14 | Claude Code command: a full path to `claude` | New Claude Code sessions run that program |
| 17.15 | Resume conversations off; restart with a Claude Code pane | It comes back as a plain shell, nothing typed |
| 17.16 | "Probably waiting" after 3; a program that prints nothing | Its card turns to probably waiting after about 3 s |
| 17.18 | Profiles → Edit: another folder and a pane on the right; Add a new one | `profiles.toml` has them; the new-session dialog's profile opens both panes |
| 17.19 | Tab menu: hide Pin, move Close the session up, add `lazygit -p {folder}` | The right-click menu follows; the item runs lazygit in the session's folder |
| 17.20 | Open with → Editor command `code {folder}` | Open in the editor opens VS Code in the session's folder; with one command the menu item has no ▶ |
| 17.21 | Tags → New rule → Branch `claude/*`, tag `claude` | A session on a `claude/…` branch gets the tag |
| 17.22 | Edit a tag: rename it, pick a colour, Quiet | Renamed on the sessions and in the rules; the chip recoloured everywhere; its notices go to the bell only |
| 17.23 | Shell → Default shell, Arguments `-NoLogo`, Environment `FOO=1` | A new shell session runs that shell with the argument; `echo $env:FOO` prints 1 |
| 17.24 | Shell integration → Install, then Remove (pwsh) | A marked block appears in `$PROFILE`, and goes; a new session with it reports its folder |
| 17.25 | Windows → ConPTY | Bundled, with the zip's build; the system's, with a bare exe |
| 17.26 | Advanced → Restart the server | The window says it is restarting; the tabs come back (or Welcome back); the uptime starts again |
| 17.27 | Graphics backend → Vulkan, then a name the machine has no adapter for, restart | Draws with it; for the missing one, the automatic choice and a line on stderr |
| 17.28 | Scrollback 200; a new session; `seq 1000` | Only about 200 lines to scroll back |
| 17.29 | Log what each pane sends and receives on; a new session | `pane-logs/session-N.log` beside the saved tabs grows as it runs |
| 17.30 | Export…, then Import on another machine (or after changing things) | One file in Downloads; importing brings the settings, themes and profiles back, the old ones kept as `.bak` |
| 17.31 | Keys → set a key to `Ctrl+[` by hand in `settings.toml`, then change another key and a font size in Settings | `settings.toml` still has every table and comment it had; only the changed lines differ |
| 17.32 | Put a mistake in `settings.toml` (`[broken`), then change anything in Settings | A toast says the file does not read and is left as it is; the file is unchanged |
| 17.33 | `[[menu.session]] command = "\"C:\\Program Files\\Microsoft VS Code\\Code.exe\" {folder}"` (Windows), then the card menu item | VS Code opens on the tab's folder (cmd took the line whole) |
| 17.34 | Open with → Editor: type `sakura {folder}` into Add another; right-click a card | `[open] editor = ["code {folder}", "sakura {folder}"]`; Open in the editor has ▶; a click on it opens VS Code, hovering opens `code` and `sakura` to its right and each opens its own; × on the second row (or emptying its field) leaves one again |
| 17.35 | Open with → take out every kura command; right-click a card | Open the folder in kura is greyed, its hover says where to set it |
| 17.36 | Tags → Tags shown 1, then 5 | The band and the cards show one tag and `+N`, then all five |
| 17.37 | Tags → a rule's Edit: change its folder, then add a branch, Save; Edit another and take it out | `settings.toml` has the rule changed in place (one rule with both, then neither); the sessions it tagged before lose the tag, those it now matches get it |
| 17.38 | General → Language → 日本語; then start mimamori; then write `language = "en"` into `common.toml` by hand | `common.toml` in the uchmk folder (`%APPDATA%\uchmk`) has `language = "ja"`, its other lines kept; mimamori comes up in Japanese; tsumugi's menus stay English; the select shows English within a couple of seconds of the hand edit |

## 18. From a script

| # | Do | Expect |
| --- | --- | --- |
| 18.4 | `tsumugi split N --down -- claude` | A pane below session N in its folder with Claude Code started; its number printed; the window shows the split |
| 18.6 | `tsumugi close N` with the window open on N | Its pane goes from the window (the session ending: `cli.rs`) |
| 18.7 | With tsumugi on PATH on another machine you reach by `ssh HOST` with a key: `tsumugi ls --host HOST`, then `tsumugi split --host HOST N` and `tsumugi read --host HOST N` | That machine's sessions listed (its server started if none ran); the split and read work there; no console window flashes on Windows |
| 18.8 | Pull the network while `tsumugi wait --host HOST N` waits, then `tsumugi ls --host HOST` again once it is back | The wait fails within about 30 s; the sessions on HOST are still there afterwards |
| 18.9 | `tsumugi ls --host HOST` where HOST has no tsumugi, and where the key is refused | One line naming HOST and what ssh said (`command not found`, `Permission denied`); no password prompt |
| 18.10 | In a terminal, `claude mcp add tsumugi -- "<full path>\tsumugi.exe" mcp`; start `claude`, type `/mcp`, then ask "Which tsumugi sessions are waiting, and what does the first one show?" with two sessions open in tsumugi | `/mcp` shows tsumugi connected with 2 tools; Claude Code calls `tsumugi_sessions` and `tsumugi_screen` and answers with the real sessions; no console window flashes. With tsumugi's server stopped, the answer says tsumugi is not running |

## 19. Keys through every screen

With `TSUMUGI_KEYLOG=1` the window writes `focus x,y wxh` to stderr each time
the keys move to another control (`focus none` when no control has them), so
the order Tab takes is read, not looked at. Read each walk against the screen:
**left to right within a row, then top to bottom**; every control reached; the
cyan ring on the one the log names and on no other (a screenshot, `[~]`).

| # | Do | Expect |
| --- | --- | --- |
| 19.1 | The new-session dialog: `Tab` all the way round, then `Shift+Tab` all the way back | The folder, the way picked in START (one stop), + Pane, the tag, the two ticks, Cancel, Create, the folder again; the log's positions go left to right, top to bottom; back the same in reverse |
| 19.2 | In START, `→` four times, then `←` four times | Claude Code, Resume last, Shell, Profile…, round to Claude Code; one button lit at a time: the way picked, filled, with the ring; on Profile… it is filled with the ring, as the others, and nothing else is lit; back on Shell it is filled again |
| 19.3 | Settings, each page in turn: `Tab` through it | Every switch, list, field and button on the page, in reading order; inside a row of several (Tags' New rule, a key and Its own, a sound and ▶, the menu's ↑ ↓ and switch) left to right |
| 19.4 | The search box (`Ctrl+Shift+P`), the waiting list (`Ctrl+Shift+Y`), the bell's list (`Ctrl+Shift+N`), Start in parallel | Each opens with the keys in its first field or line; `Tab` walks it in reading order; `Esc` closes it and the keys go back to the pane they came from |
| 19.5 | Every dialog and list in 19.1 to 19.4: `Enter` on a field, and on each button | As the dialog's foot says: Enter in a field does the main thing; on a button it presses that button; nothing reaches the shell behind |
| 19.7 | Every dialog and form with buttons: New session, Start in parallel, Remove the worktree?, closing a running tab, Settings' profile and environment editors, the waiting list's Always allow… | The same everywhere: the buttons at the right, Cancel (or Keep) on the left of the main one, on one line, the same height; the focus log's x for Cancel smaller than the main one's, and the main one's right edge at the dialog's right edge less its margin; Tab reaches Cancel before the main one |
| 19.8 | The same screens: their headings, and the key named beside a heading | A dialog's title at its top left, its key (where it has one) at the top right in the terminal's font; a section's heading in small capitals above its card; the same gaps between them on every screen (a screenshot each, `[~]`) |
| 19.6 | The same screens at 150% (Windows display scale) and with the window 1000 px wide | Nothing cut, nothing on top of something else, no row's words touching the line under it (a screenshot each, `[~]`) |

