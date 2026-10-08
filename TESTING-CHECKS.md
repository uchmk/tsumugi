# 実機チェックリスト

`TESTING.md` から `cargo run -p tsumugi --example make-testcheck` で生成している。**正は TESTING.md** で、
このファイルはその表を 1 行ずつ印を付けられる形に並べたもの。食い違ったら TESTING.md を信じること。

**チェック（`[x]` と `[~]`）だけは手で書いてよく、生成し直しても残る。**それ以外を書き換えても次の生成で消える。
`[x]` は実機で確かめた印、`[~]` は見た目の行を画面の画像で判断した印（済みには数えない。持ち主が同じ画像を見て `[x]` にする）。
**件数はこのファイルに書かない**（印を付けた PR が並ぶと必ずぶつかるため）。`-- --stats` で出る。

付ける前に「失敗していたら、画面かディスクの何が違ったはずか」を自問すること。キーは [TESTING-KEYS.md](TESTING-KEYS.md)。

## 1. The window and the server

- [ ] **1.1** Start `tsumugi` with no session and nothing to restore → **Start your first session**: the folder it was started from first, the default folder, the last ones closed, **Choose another folder…**; Enter (or a click) starts the settings' program there. The window opens in under a second
- [ ] **1.2** `tsumugi ls` from another terminal → One line per session: number, state, program, folder, title, what it said, its tags
- [ ] **1.3** Close the window, then start `tsumugi` again → The same tab and the same shell, its output still there: the server kept it
- [ ] **1.4** With the window closed, `tsumugi new . -- echo hi` → Prints a number; the next window has a tab with `hi` in it
- [ ] **1.5** `tsumugi attach <that number>`, and `tsumugi attach <folder name>` → The window opens on that session. A name two sessions share says so and names their numbers
- [ ] **1.6** With tsumugi running, `cargo build` (Windows) → The build replaces `tsumugi.exe` -- no `アクセスが拒否されました` -- because the server runs from its copy in `%LOCALAPPDATA%\tsumugi\server\`
- [ ] **1.7** Start the newly built window while the older server runs (a build of 0.45.0 or 0.46.0 too) → **tsumugi was updated**, what that means in a sentence, **Restart the server** lit; Enter (no Tab or click) restarts it and every tab comes back at once, with no Welcome back; Esc closes the window and the old sessions go on
- [ ] **1.8** Task Manager after 1.3 → One `tsumugi-<version>-<hash>.exe` server, and no console window anywhere
- [ ] **1.9** The taskbar, Alt+Tab, and the window's corner → The logo (two threads, cyan and gold, on a dark tile) as the window's icon
- [ ] **1.10** `tsumugi new . --tag a --tag b --tag c -- bash` → All three tags on the new session (`tsumugi ls`'s last column)
- [ ] **1.11** Settings → General → Restart the server after an update without asking on; then 1.7 → No question: the older server is restarted at once and every tab comes back

## 2. Panes and splits

- [ ] **2.1** `Alt+Shift++`, then `Alt+Shift+-` → A shell to the right, then one below it, each in the folder of the pane it split
- [ ] **2.2** `Alt+Arrows` → The keys move to the pane on that side; its cursor fills, the others' go hollow, and the others are dimmed
- [ ] **2.3** Bring the pointer to a gap between panes, drag → A cyan line appears, the split follows the pointer; a double-click halves it
- [ ] **2.4** `Ctrl+Shift+Z` with a hidden pane waiting (notify it) → One pane fills the tab, its heading says ZOOM with a small split; a gold-ringed note at the bottom right says how many wait behind and the key, and a click goes there; `Ctrl+Shift+Z` again restores the split
- [ ] **2.5** Drag a pane by its header onto the middle of another → A cyan outline and **Swap** while dragging; on release the two trade places
- [ ] **2.6** Drag a header to another pane's edge → **Move here** on that half; on release the pane goes to that side and the two share the room
- [ ] **2.7** Split right until a pane is narrower than 20 columns → It folds into a strip with its state's dot and its name (on its side when tall); a click gives it the keys, a drag carries it
- [ ] **2.8** `exit` in one pane of a split → The pane goes and its neighbour takes its room
- [ ] **2.9** `lazygit` in a pane, move with `j`/`k`, `?` then `Esc`, then `q` → It draws, takes the keys, its menu closes on `Esc`, and quitting leaves a working prompt
- [ ] **2.10** Type Japanese with the IME in a pane → The candidate window sits at the cursor and the committed text arrives once
- [ ] **2.11** On a JIS keyboard: `Alt+Shift+;` (`+`), then `Alt+Shift+-` → To the right, then below: neither is taken for the other
- [ ] **2.12** Drag a pane of a split by its header onto the sidebar → The sidebar lights up, **A tab of its own**, the pane's name with the pointer; dropped, the pane is a tab of its own after the one it left
- [ ] **2.13** One pane alone, and each pane of a split → Every pane is a card with room round it and a 30px heading: the state's mark, the name, the folder, short words on the right (none for a shell); its ring in the state's colour, the one with the keys too; the heading is not lit for the keys
- [ ] **2.14** A split tab, `Ctrl+Shift+I`, type `echo hi`, Enter; `Ctrl+Shift+I` again → **TYPING INTO ALL** on every pane's heading; `hi` in each; after the second press only the pane with the keys gets keys; the window's own keys (`Ctrl+Shift+T`) act once
- [ ] **2.15** Three panes split both ways: hover each divider, drag it, double-click it → Near it the cyan line and the resize pointer; the split follows the drag and stays where dropped (and after a restart); a double-click halves it. Unchanged from before the dividers moved into `tsumugi-layout` (v0.52.0)
- [ ] **2.16** Copy `echo one` + Esc `[201~` + `echo two` (e.g. `printf 'echo one\033[201~echo two' \| clip` / `pbcopy`) and paste it into bash or pwsh with bracketed paste → It lands as one line held on the prompt (`echo one[201~echo two`), not run: the Esc is dropped from a paste
- [ ] **2.17** Drag a divider and, still holding the button, press `Ctrl+Shift+Z` (or switch tabs with the keyboard); let go; split again → The tab shows its real split afterwards; the new split appears
- [ ] **2.18** A German (or other AltGr) keyboard layout: `AltGr+Q` and `AltGr+7` in bash and in pwsh → `@` and `{` typed, nothing else before them (`TSUMUGI_PTY_LOG` shows only the character); `Ctrl+Alt+X` with no character still reaches emacs as C-M-x
- [ ] **2.19** A pane in Shift_JIS (the status bar's charset), type or paste `日本😀` → `日本?` reaches the program, not `&#128512;`
- [ ] **2.20** Close a tab whose shell runs a deep tree (`cmd /c "cmd /c ping -t localhost"` three levels) → The window closes it within a second; every process of the tree is gone (`Get-Process ping`); nothing else is
- [ ] **2.21** `lazygit` in a pane: `Tab` and `Shift+Tab` a few times, then `j`/`k`; the same in bash (`Tab` completes) → Each Tab moves lazygit to its next panel and the keys stay with it; no button of the window takes them (`TSUMUGI_KEYLOG=1` prints no `focus` line)
- [ ] **2.22** `echo https://example.com/a?b=1 src/main.rs:3:5 ~/.bashrc`; hold `Ctrl` and move over each, click each → Each underlined only while Ctrl is held and the pointer is on it; the address opens in the browser; `src/main.rs` (from the session's folder) opens in VS Code at line 3, column 5; a missing path says **No such file**
- [ ] **2.23** `echo 'https://x.example/?a=1&b=2'`, Ctrl+click it; `[open] file = ""`, Ctrl+click a path → The address is not opened and a toast says why; the file opens in the system's program for it
- [ ] **2.24** `Ctrl+=` three times, `Ctrl+-` once, `Ctrl+0`; `Ctrl+Shift+-` in bash → Every pane's letters grow, shrink and come back to the settings' size, the grid re-fitting each time and a toast naming the size; `Ctrl+Shift+-` undoes in bash
- [ ] **2.25** `seq 200`, then `Ctrl+Shift+M`; `k` past the top, `v`, `j` three times, `y`; paste in Notepad → A gold box and **COPY MODE** in the pane; the output scrolls back under the box; the selection follows it; four lines on the clipboard; the mode ends and keys reach the shell again
- [ ] **2.26** `Ctrl+Shift+M`, `G`, `0`, `Enter`; again, `Esc`; again, then `Alt+Arrows` to another pane → The cursor's line copied; `Esc` leaves with nothing selected; moving away ends the mode
- [ ] **2.27** pwsh started by tsumugi (no hook in the profile, then with `tsumugi shell-hook pwsh` and Starship): run `dir` five times; `Ctrl+Shift+Up` three times, `Ctrl+Shift+Down` twice → Each press puts the next prompt line above (below) at the top of the pane; the prompt looks as before (Starship's included), with no stray characters
- [ ] **2.28** The same in bash and zsh with `tsumugi shell-hook bash` / `zsh` in the rc file → The same jumps; `cd` still updates the pane's folder
- [ ] **2.29** `seq 300`, `Ctrl+Shift+F`, type `1`, then `Enter` a few times, `Shift+Enter`, then `Err` → A bar at the pane's top right; `299` (the newest `1`) selected and on screen, then older ones, then newer; **No match** in red for `Err`; letters typed reach the bar, not the shell
- [ ] **2.30** In the bar: `Enter` until it passes the oldest match; then `Esc`; then type in the pane → **From the end again** once it starts over; `Esc` closes the bar, drops the selection and is not sent to the shell (Claude Code is not interrupted); keys reach the shell again
- [ ] **2.31** Open the bar, click into the pane, `Ctrl+Shift+F` again; `Alt+Arrows` to another pane → The second press puts the keys back in the bar; moving to another pane closes it

## 3. The sidebar

- [ ] **3.1** Open three tabs in different git repositories → Each card: the name in bold sans-serif (IBM Plex Sans JP, else Segoe UI / the system's), then `~/folder · branch` in the terminal's font, then the state and how long in the state's colour (waiting gold, running cyan, error red, done green, probably waiting grey); one state mark, on the left
- [ ] **3.2** The **+** beside SESSIONS, and the one at the top of the rail → The new-session dialog opens with the focused pane's folder
- [ ] **3.3** Sort button → each of the five orders → The cards reorder accordingly; **Manual** keeps a dragged order across restarts
- [ ] **3.4** Drag a card in Manual order → The grip shows on hover; the card lands where it is dropped
- [ ] **3.5** The state and folder filters at the foot → Only matching cards; the heading reads `SESSIONS  N of M` (also `5 of 5` with no filter), then the order button, then **+**
- [ ] **3.6** Right-click a card: rename, tag, mute, pin, restart, duplicate, new window, open in editor / filer, copy path, close → Each does what it says; close is red and last; Rename, Duplicate and Close show their keys on the right, in grey
- [ ] **3.7** Sort → One line each; then `Ctrl+Shift+B` → One line per tab; then the 60px rail with first letters and state rings
- [ ] **3.8** Rest the pointer on a card of another tab → Its last 12 lines in a box, the waiting pane's in a tab of several
- [ ] **3.9** A tab in a repository with uncommitted changes → `+N −M` (or `N new`) on the card's second line; on hover, the files as `git status` lists them
- [ ] **3.10** Drag the sidebar's edge left past 120px, and back → It becomes the rail; dragged back out, the sidebar
- [ ] **3.11** One session of each state (notify them) and a plain shell → Each card ringed in its state's colour with its own mark -- a clock (waiting, breathing), a turning arc (running), a dotted circle (probably waiting), a triangle (error), a tick (done, its ground sunken) -- and the shell grey with a small dot and no words; the foot's filters and the status bar count the shell as Shell, not Running
- [ ] **3.12** Two waiting, then `[keys] next_waiting = "F8"` → Under a line at the foot: the key as a cap (`F8` once changed) and **Jump to waiting · 2** in grey; a click on either jumps
- [ ] **3.13** Theme tsumugi Light (and each light theme) → The waiting and error words and counts read clearly on white (WCAG 4.5 or more)
- [ ] **3.14** A tab on a branch with an open pull request (`gh` signed in) → `· PR #N` after the branch on the second line, green when its checks pass, red when they fail, cyan while they run, grey with none; its tooltip says which
- [ ] **3.15** The card of the tab with the keys, Claude Code talked to a few times → A line `N prompts · 12m · 3.4k tokens`; the other cards have none. A running card's third line ends with `· N tokens`
- [ ] **3.16** `F2` → A field on the card with the name selected; Enter renames, Esc leaves it as it was
- [ ] **3.17** `Ctrl+Shift+D` (macOS `Cmd+Option+D`) in a Claude Code pane, then in a shell → A new tab in the same folder: `claude` typed in the first, a shell in the second
- [ ] **3.18** Look at the sidebar on first start → 288px wide
- [ ] **3.19** Right-click a card → Note…, type `the release`, Enter; restart the machine (or the server) → `“the release”` on the card under the folder; Edit the note… and an empty Enter takes it off; after the restart the note is back
- [ ] **3.20** A tab with uncommitted changes: click its `+N −M` (or search `changes`) → The diff over the window: each file under its name, added lines green, removed red, hunks cyan, new files at the end; Esc or a click outside closes it; a lock file's large diff is cut at 512 KB
- [ ] **3.21** Right-click a card → Save the output to a file (or search `save the pane`) → A toast names `Downloads\tsumugi-<name>-<date>-<time>.txt`; it holds the whole scrollback as the program wrote it, Japanese whole, long lines unbroken
- [ ] **3.22** A tab on a branch other than main (a worktree's), right-click → Create a pull request (`gh` signed in) → The branch pushed, a pull request made from its commits and opened in the browser; a toast with its address; on `main` the item is not there; without `gh`, a toast says why
- [ ] **3.23** Run `codex` (or `gemini`, `opencode`) in a session → The card is an agent's, not a grey shell: its name (`codex`) on a line under the folder, its states shown; on Windows, Gemini CLI (run by node) stays a shell
- [ ] **3.24** `npm run dev` (or `python -m http.server 8123`) in a session → `:3000` (`:8123`) on its card within a few seconds; a click opens `http://localhost:3000` in the browser; gone when the server stops
- [ ] **3.25** A card with tags and a PR (and a note), a card with neither → The extra lines (note, agent and ports, numbers) under the tags, or under the third line; none over another
- [ ] **3.26** Claude Code working (its title `✳ …` or a braille spinner), and pwsh → The card's and pane heading's name without the `✳` or spinner: one mark only, the card's own on the left
- [ ] **3.27** Click a waiting card, then a running one, then a shell → The card's ground stays dark (no fill): its ring in its state's colour, brighter, with a soft glow past it (cyan for a shell); a running card's line still runs along its top edge
- [ ] **3.28** A plain shell's card beside an agent's → The shell's card is a line shorter: no empty third line
- [ ] **3.29** Claude Code working in a tab: `F2`, type, `Esc`; right-click → Note…, type, `Esc`; and `F2` on a tab, then pick a tag filter that hides it → Each field closes as it was and Claude Code keeps working (no Esc reached it, `TSUMUGI_PTY_LOG` has no `in key` line); a hidden card's field is let go and the pane gets keys again
- [ ] **3.30** With the Japanese IME on: right-click a card → Note…, type `にほんご`, convert with Space, Enter to commit, Enter again; the same in `F2`'s field, the menu's Rename… and the search box (`Ctrl+Shift+P`) → The reading stays underlined as it is typed and the candidates come up; the first Enter only commits the conversion, the second keeps the note (or name); `日本語` arrives whole, nothing lost or doubled
- [ ] **3.31** Sort → Folder with three running, one waiting and one done tab in a folder → Every tab under its heading as a full card, none as one line, no "+ N more"; a click on the heading closes and opens it
- [ ] **3.32** Open tabs until the sidebar is full (about 15), in each order; then make the window shorter → All full cards while they fit; past that the last ones turn into one-line rows from the bottom up, the tab shown always a card; no scrollbar; taller again, they are cards again
- [ ] **3.33** Windows, a profile with no tsumugi hook: rules `c:\dev\filer` → `filer` and `c:\dev\tsumugi` → `tsumugi`; in a pwsh tab `cd c:\dev\filer`, then `cd c:\dev\tsumugi`, then back; add a tag `mine` by hand → The card wears `filer`, then `tsumugi` in its place, then `filer` again; `mine` stays throughout; a profile's own hook (mise, `tsumugi shell-hook`) still runs

## 4. States, notifications and answering

- [ ] **4.1** `claude` in a pane, ask it to do something that needs permission → The card turns gold with **Waiting for you** and Claude's words
- [ ] **4.2** With the hooks (12.x), Claude finishing a reply → The card turns green, **Done**
- [ ] **4.3** The waiting card's `1 Yes` button, without going to the tab → Claude goes on, as if `1` were typed there; the card leaves the waiting state
- [ ] **4.4** A card waiting with no menu on screen → No buttons
- [ ] **4.5** Another window in front while a session starts waiting → A Windows toast titled `<folder> is waiting for you`, the work's name and words below, with **Open** and **Later**; a second notice from the same session replaces it in the Action Center
- [ ] **4.6** Click the toast, or Open; then Later on another → tsumugi comes to the front on that pane; Later only puts the toast away
- [ ] **4.7** The taskbar button while two wait, then one fails → A gold `2` with dark digits; red with white once one has failed; it clears when the window is looked at
- [ ] **4.8** Settings → Notifications: flash and sound on for waiting, then 4.5 again → The taskbar button flashes and the system's message sound plays once (the error sound for an error)
- [ ] **4.9** Right-click a tag chip → mute → Sessions with the tag tell only in the bell
- [ ] **4.10** The bell, right of the search box at the top; then `Ctrl+Shift+N` (macOS `Cmd+Shift+N`) twice in a shell → The list of notices under it, newest first, its names without Claude Code's `✳`; a click goes to the session and marks it read; no bell in the sidebar's heading. The key opens the same list and closes it, and nothing reaches the shell
- [ ] **4.11** `printf '\e]9;hello\a'` in a pane → Marked waiting with `hello`, no hooks needed
- [ ] **4.12** `Ctrl+Shift+U` with two waiting → The one waiting longest first, then the other
- [ ] **4.13** Two Claude Code sessions asking permission, then `Ctrl+Shift+Y` (or **List ›** at the sidebar's foot) → A panel: each waiting session with what it said, how long, its choices as buttons and a tick; a choice's button types its number there
- [ ] **4.14** In that panel, **Yes to 2** → Both go on as if `1` were typed in each; untick one first and only the other is answered; **No to N** types each menu's "No" choice
- [ ] **4.15** A session waiting with a menu of other words (no "Yes"/"No") → Not counted in Yes to N / No to N; its own buttons still work
- [ ] **4.16** `exit` a Claude Code session after some work; then the panel's **Recently closed** (or search `recently`) → It is listed: today at …, done, its tokens; **Last output** shows its last lines; **Resume** opens a new tab in its folder typing `claude --resume <id>`; the list survives a restart of tsumugi
- [ ] **4.17** A Claude Code session asking to run a Bash command; `Ctrl+Shift+Y` → Under what it said: "Bash command", the command and its description, "Do you want to proceed?", in the terminal's font, before the buttons
- [ ] **4.18** A Claude Code session asking to run one Bash command → `Ctrl+Shift+Y` → **Always allow…** → **Add and say yes** → Before: the rule `Bash(<command>)` and the file named; after: the project's `.claude/settings.local.json` has it under `permissions.allow` (the old file as `.tsumugi-backup`), `1` typed, a toast; next time Claude Code runs it without asking. No **Always allow…** for an edit or a command of several lines
- [ ] **4.19** `[notify] webhook = "https://ntfy.sh/<a topic>"`, `webhook_after = 30`; a session waiting; minimize (or close) the window → Within a minute the phone (ntfy app on the topic) shows `<folder> is waiting for you` and what it said; once, not again for the same wait; nothing while the window is looked at; a muted session never
- [ ] **4.20** `webhook_format = "slack"` with a Slack incoming webhook → The message in the channel, the title in bold
- [ ] **4.21** Settings → Notifications → WHAT IT COSTS → Tell when the day costs → Past $5, with Claude Code's day already past $5 (or `[prices]` raised to get there) → `spend_day = 5` in the file; at once a toast `Today's Claude Code use passed $5: about $N at API prices`, and with the window not looked at the system's notification too; not again that day, again the next
- [ ] **4.22** Tell when a 5-hour block costs → Past $5, the block past it → The same once for the block (`This 5-hour block passed $5`); again only in the next block
- [ ] **4.23** Start `claude` in a session and type nothing for half a minute (with a plugin such as claude-mem printing at start too) → After about 10 s (`[sessions] quiet`) the card reads **Quiet for … · probably waiting**, not **Running**; typing a prompt makes it **Running** again
- [ ] **4.24** With a webhook set, a waiting session whose note starts with `@` (`tsumugi notify --session N "@C:\Windows\win.ini"`), the window not looked at → The phone gets the text `@C:\Windows\win.ini`, not the file's contents

## 5. Search

- [ ] **5.1** `Ctrl+Shift+P`, type part of a session's name → That session first; Enter goes there
- [ ] **5.2** Type a folder another session is in → **Folder** entries: Enter starts a session there
- [ ] **5.3** Type `split` → The commands, with their keys beside them
- [ ] **5.4** Type three letters printed long ago in another tab's scrollback → **IN THE SCROLLBACK** lines below the rest; picking one goes to that tab, scrolls back to the line, the match selected
- [ ] **5.5** `Esc`, and a click outside → Closes without doing anything
- [ ] **5.6** Keep a prompt `Say {project}` in the input box's **Prompts…**; in the search box type `send`, pick **Send prompt: …**; then `Ctrl+Shift+I` in a split tab and pick it again → It is sent to the pane with the keys, the project's name in it; the second time every pane of the tab gets it
- [ ] **5.7** A tab of three panes (Claude Code, a shell beside, one below), dividers moved; **Save this tab's layout**; close it; **Open layout: …** from another pane → `layouts.toml` beside the settings has it; a new tab opens in that pane's folder with the same splits and shares, Claude Code and the shells started as they were
- [ ] **5.8** `Ctrl+Shift+O` with sessions waiting, running and done; arrows, `Enter`; a click on another; `Ctrl+Shift+O` again → **All sessions** with the waiting first, each row's state, folder, branch, tags, tokens and last two lines; Enter and the click go to that session; the key closes it
- [ ] **5.9** Give the tab with the keys five tags, `[tags] shown = 3`; make the window narrower step by step; move the keys to a tab without tags → The search box and the bell stay in the middle of the band; three tags and `+2` on the right, fewer and a bigger `+N` as it narrows, never over the box; the box shrinks to its magnifier last
- [ ] **5.10** `F1` on a 1280 × 800 window; then a narrower one; `F1` again, `Esc`, a click outside → Every key by kind in three columns (two when narrower), all on one screen with no scrolling; a key moved in the settings shows its new key; each closes it

## 6. New sessions

- [ ] **6.1** `Ctrl+Shift+T`, Enter → Claude Code starts in a new tab in the focused pane's folder
- [ ] **6.2** `Ctrl+Shift+T`, Shell, `Alt+Enter` → A shell split to the right instead of a tab
- [ ] **6.3** Type part of a folder, `Tab` → Completes from the folders listed, including **closed** ones from sessions that ended; the cursor at its end; a second `Tab` moves on
- [ ] **6.4** Add a tag, and take a dashed (folder rule) tag off → The new session wears exactly what the dialog showed
- [ ] **6.5** Beside it → + Pane twice (Claude Code, Shell), Create → One tab of three panes: the first, one to its right, one below that
- [ ] **6.6** Tick Save as a profile, name it; next time Profile… → it → The folder, start, tags and panes come back
- [ ] **6.7** Tick In a new git worktree in a repository, Create → A folder `<repo>-tsumugi-MMDD-HHMM` beside the repository on its own branch, and the session in it; a toast says so
- [ ] **6.8** `exit` the last session in that worktree → **Remove the worktree?**; Remove takes the folder (the branch stays); with uncommitted changes git refuses and the toast says why
- [ ] **6.9** `Ctrl+Shift+T`, then only the keyboard: `Tab` through the dialog, `←`/`→` on the ways to start, `Space` on a button, `Shift+Tab` back → A cyan ring shows where the keys are; Tab completes the folder once and then moves on to the way picked in START (one stop for the whole row), then + Pane, the tag, the two ticks, Create and Cancel; `←`/`→` walk the row round, Profile… included, and Shift+Tab from + Pane comes back on the way picked; `Enter` on Cancel cancels, `Enter` in a field creates
- [ ] **6.10** Search `parallel` → Start in parallel; two prompts; Start 2 → Two folders `<repo>-tsumugi-MMDD-HHMM-1`/`-2` beside the repository on branches `tsumugi/MMDD-HHMM-1`/`-2`, two tabs, Claude Code started in each on its own prompt (quotes in a prompt kept); Esc or Cancel starts nothing
- [ ] **6.11** `Ctrl+Shift+T`, `Tab` to Shell (the ring on it), `Enter` → A shell starts, as Alt+Enter would split one: Enter on a way to start picks it and creates

## 7. The input box

- [ ] **7.1** `Ctrl+I`, write two lines (Enter between), `Ctrl+Enter` → The box opens inside the focused pane's card, at its foot, the terminal shortened above it; both lines reach the session as one prompt, then Enter
- [ ] **7.2** `↑` in the empty box → The last prompt sent; `↓` back
- [ ] **7.3** Drop two files on the window → Chips in the box; sent, their paths follow the text, quoted when they have spaces
- [ ] **7.4** Pick a tag as **To**, send → Every session wearing the tag gets it
- [ ] **7.5** While a session runs, **When done** (or `Ctrl+Shift+Enter`) → `1 queued` on the card and in the box; when the session is done, the prompt goes and a toast says so
- [ ] **7.6** Queue to a session showing a permission menu → It waits until the menu is answered and the session is done
- [ ] **7.7** `Esc`, then type `echo ok` and Enter → The box closes and the keys go back to the pane; the draft is kept for next time; the Esc does not reach the shell (`echo ok` runs whole), nor Claude Code (it is not stopped)
- [ ] **7.8** Take a screenshot (`Win+Shift+S`), `Ctrl+V` in the box → A chip `▣ paste-<date>-<time>.png · N KB`; the PNG is in the settings folder's `attachments`; sent, its path follows the text
- [ ] **7.9** `Ctrl+V` with text on the clipboard → The text is pasted; no chip
- [ ] **7.10** Drop a file of a few hundred KB → Its chip says its size, `▤ name · 214 KB`
- [ ] **7.11** Split the tab, open the box, move the keys with `Alt+Arrows` → The box follows the pane with the keys; a pane narrower than 20 columns has none
- [ ] **7.12** **+ Sessions** in the box, tick another session, `Esc`, write a prompt, `Ctrl+Enter` → `+ name` beside the pane under To; the menu's Esc does not reach the shell; both sessions get the prompt; a click on `+ name` takes it off
- [ ] **7.13** Write `Run the tests in {project} on {branch}`, **Prompts…** → name it, Save; empty the box, **Prompts…** → it, `Ctrl+Enter` → The menu stays open while the name is typed; `prompts.toml` beside the settings holds it; picked, it fills the box and the keys are back in it; the session gets the project's and branch's names in place of the braces; with **+ Sessions**, each its own
- [ ] **7.14** Open **Prompts…** or **+ Sessions**, press `Esc`, type → The menu closes, the box stays open with the keys; nothing reaches the shell
- [ ] **7.15** Queue a prompt for a session in another tab that is running Claude Code; let it stop on a permission question (`1. Yes / 2. No`) → The prompt stays queued while the question is up; it is sent only once the question is answered and the session waits with no question
- [ ] **7.16** In the input box of session A press `↑` (a past prompt shows), switch to session B and press `↑` / `↓`; then `Shift+↑` in a two-line draft → B's own draft is never replaced by A's; Shift+↑ selects in the draft, it does not walk the history

## 8. Restoring after a restart

- [ ] **8.1** Split tabs with Claude conversations, then reboot (or stop the server) and start tsumugi → **Welcome back** lists the tabs; Restore brings their splits and folders back
- [ ] **8.2** A restored Claude Code pane → `claude --resume <id>` was typed: the conversation is back
- [ ] **8.3** Settings → General → On start → Restore the last sessions, then 8.1 → No Welcome back; the tabs simply return
- [ ] **8.4** 8.1 with fourteen or more tabs → The list scrolls; Restore and Start fresh stay in sight; the time reads `today at …` or `yesterday at …`
- [ ] **8.5** A session where `codex` ran; restart the machine (or the server) → The restored tab types `codex resume --last`; with `[agents.gemini] resume = "…"`, a Gemini session types that

## 9. The settings screen

- [ ] **9.1** `Ctrl+,` → The screen with its nine pages and the search field over them; the band says Settings in the middle with an X; no sidebar or status bar; `Esc` and the X close it
- [ ] **9.2** Change something on each page → Only that line of `settings.toml` changes (diff the file); comments stay
- [ ] **9.3** Write a mistake into `settings.toml` by hand → A red line above the status bar says what and where, until it is fixed; nothing else changes
- [ ] **9.4** Open settings.toml / Open the settings folder → The system's editor / file manager opens
- [ ] **9.5** Type `scroll` in Search settings → Only Advanced in the list, with a count; its Scrollback row lit; Enter goes there
- [ ] **9.6** Next to the design's "Settings: every page" → The same pages, sections and rows in the same order
- [ ] **9.7** Only the keyboard: `Ctrl+Tab` / `Ctrl+Shift+Tab` through the pages, `Tab` through a page, `Space` on a switch, `Esc` twice → A cyan ring on the control with the keys; Tab never stops on the top band or the list of pages; Space flips the switch (the file changes); the first Esc leaves the control, the second closes the screen; nothing typed reaches the shell behind it
- [ ] **9.8** Notifications → WHEN A SESSION… → A line between rows; the three state columns the same width, each switch in the middle of its column and row, under its heading's dot
- [ ] **9.9** `Ctrl+,` (macOS `Cmd+,`) with the settings open → They close and stay closed (not reopened on their first page)

## 10. Themes

- [ ] **10.1** Settings → Theme, walk the thirteen → The window and the panes recolour at once; states stay readable (gold, cyan, red, green)
- [ ] **10.2** `theme = "system"`, then switch Windows between dark and light → tsumugi follows within seconds
- [ ] **10.3** Save a Windows Terminal scheme `.json` in `themes\` → It is in the list under its name; chosen, `ls` colours match Windows Terminal's with that scheme
- [ ] **10.4** The same with an iTerm2 `.itermcolors` → As 10.3
- [ ] **10.5** `theme.toml` with one colour → Only that colour changes in the theme in force
- [ ] **10.6** Settings → Theme, in Follow OS → Mode (Follow OS, Light, Dark) at the left under the heading; only the theme shown now is lit, it and the other kind's pick are named `when dark` / `when light`; PREVIEW names the theme in force; a line with the font, the window and motion, a click goes to Appearance; each theme's swatches bordered and rounded

## 11. Fonts

- [ ] **11.1** Settings → Appearance → Font list → The installed monospace fonts (Cascadia, Consolas, any Nerd Font)
- [ ] **11.2** Pick Cascadia Code; `printf '\e[1mbold\e[0m \e[3mitalic\e[0m'` → The pane in Cascadia; bold and italic in their own faces; the line under the list names the files found
- [ ] **11.3** Consolas → Its bold and italic found too (`consolab.ttf`, `consolai.ttf`)
- [ ] **11.4** Size and line height: type a value, Enter → The grid re-fits; text in the middle of taller rows; `tsumugi ls` shows the new columns × rows after a moment; `Esc` in the field leaves it as it was
- [ ] **11.5** Cascadia Code or Fira Code, `echo '-> != == >= => |> www'` → Each shown as one sign, on the grid; Ligatures off draws them as plain characters
- [ ] **11.6** Japanese text and a Nerd Font icon in a prompt → Japanese from the system font, icons from the Nerd Font, neither as boxes

## 12. Claude Code's hooks

- [ ] **12.1** Without the hooks in `~/.claude/settings.json`, start tsumugi → On the first-run screen, the gold-ringed card with the two hooks under the folders; with sessions there already, a short card at the bottom right, above the input box when it is open
- [ ] **12.2** Add the hooks → Both in the file beside any there, each the full path of this tsumugi (`C:/…/tsumugi.exe notify …`, in `'…'` when the path has a space), `settings.json.tsumugi-backup` next to it, and a toast
- [ ] **12.3** Not now / Don't ask again → Asked again at the next start / never again
- [ ] **12.4** Settings → Shell & hooks → Claude Code hooks → Add → Added, the row says Installed; Remove takes only tsumugi's out, the backup beside it
- [ ] **12.5** `notify = ["tsumugi", "notify", "--state", "done"]` in `~/.codex/config.toml`; ask Codex something in a session → When its turn ends the card turns done (green) with the first line of its answer
- [ ] **12.6** Windows PowerShell 5.1: a `$PROFILE` saved as UTF-16 (`"# mine" \| Out-File $PROFILE`), then Settings → Shell & hooks → Shell integration → Install → A toast says the profile is not UTF-8 text and is left as it is; the profile is unchanged (its hash the same)
- [ ] **12.7** In `~/.claude/settings.json`, tsumugi's hooks as the bare `tsumugi notify --stdin` / `tsumugi notify --state done`, with tsumugi not on the `PATH`; start tsumugi → A toast says the hooks could not find tsumugi and run this one now; both commands are this tsumugi's full path, the old file is `settings.json.tsumugi-backup`; Claude Code in a session ends its replies with no "Stop hook error" and the card turns done
- [ ] **12.8** With the hooks in, Claude Code in a terminal that is not tsumugi (Windows Terminal); let it finish a reply → No "Stop hook error", and Claude does not carry on by itself

## 13. The window's frame

- [ ] **13.1** Look at the window (Windows) → No system title bar: the band carries minimize, maximize and close on the right
- [ ] **13.2** Drag the band's empty part → The window moves; drag it to the screen's top: it maximizes (Aero Snap)
- [ ] **13.3** Double-click the band → Maximizes; again, restores
- [ ] **13.4** Each of the three buttons → Minimize, maximize/restore (its icon changes), close; close turns red under the pointer
- [ ] **13.5** Each edge and corner → The pointer changes and dragging resizes; not while maximized
- [ ] **13.6** Hover the maximize button (Windows 11) → Note whether Snap Layouts appear (they may not: the button is tsumugi's own)
- [ ] **13.7** Settings → Appearance → tsumugi's own title bar off → The system's frame comes back at once; on again, gone
- [ ] **13.8** Material → mica, restart → The desktop's colour through the band, sidebar and status bar; the panes stay solid
- [ ] **13.9** Material → acrylic, restart → A blurred desktop instead; moving the window may lag (Windows' own limit)
- [ ] **13.10** macOS: Material → vibrancy, restart → The sidebar's frosted material behind the band and sidebar; the sidebar runs up to the window's top and the traffic lights sit on it; the band starts to its right, and its empty top drags the window
- [ ] **13.11** Linux on Wayland: tsumugi's own title bar off → The system's frame (Adwaita) with its buttons, moving and resizing the window; no title text on it

## 14. Motion

- [ ] **14.1** A session waiting → Its card's gold ring brightens and fades every 2.4 s, a glow outside it
- [ ] **14.2** A session running → A thin cyan line crossing the top of its card every 1.8 s
- [ ] **14.3** Switch tabs; split → The panes fade in over a moment rather than appearing at once
- [ ] **14.4** Settings → Appearance → Animations off → All of it still; CPU idle
- [ ] **14.5** Task Manager with one waiting card, window focused and then behind another → A few percent CPU at most in front, less behind (the window repaints 30 times a second only while something moves, 10 when not looked at)

## 15. The status bar

- [ ] **15.1** Look at it → 28px tall: server uptime and counts per state, a divider, the folder (monospace) and branch of the pane with the keys, a divider, its program and size, the clock; the clock's tooltip gives the day and date, the program's says what each part is
- [ ] **15.2** Commit without pushing in the focused repository → `↑1` in gold within half a minute
- [ ] **15.3** A branch with an open pull request (`gh` signed in) → `PR #N` coloured by its checks; a click opens it in the browser
- [ ] **15.4** A Claude Code session focused → `N tokens · Today M`; the tooltip breaks it down
- [ ] **15.5** Rest the pointer on `UTF-8` → Says the panes read and write UTF-8, and that ConPTY turns any console program's output into it; `chcp 932` and a Japanese `dir` in cmd still read right
- [ ] **15.6** Claude Code used in the last hours → `5h N · resets HH:MM` beside the tokens; the tooltip gives the window's start and end, what is left, and that the limit is the plan's
- [ ] **15.7** Linux or macOS: click `UTF-8` → Shift_JIS; `cat` a Shift_JIS file; type Japanese into `cat > x.txt`, then `nkf -g x.txt` (or `file`) → The menu of eight; the file reads right; `Shift_JIS` on the pane's heading and in gold in the status bar; what was typed is Shift_JIS in the file; restart the machine: still Shift_JIS
- [ ] **15.8** Windows: the `UTF-8` in the status bar → Only a label with its tooltip: no menu
- [ ] **15.9** Claude Code used today (Opus 5.5, say) → `≈$0.39` after the conversation's tokens and today's, and in the 5h window; the tooltip says it is the API's price; `[prices."claude-opus-5-5"] input = 8.0` doubles the input's share; a closed session's line in Recently closed has its own

## 16. Keys

- [ ] **16.1** Settings → Keys → click New session, press `Ctrl+Shift+K` → `[keys] new_tab = "Ctrl+Shift+K"` in the file; `Ctrl+Shift+K` opens the dialog; `Ctrl+Shift+T` goes to the shell
- [ ] **16.2** Its own → The key goes back; the search box and the dialog name the key in force
- [ ] **16.3** `zoom = "none"` → `Ctrl+Shift+Z` reaches the shell
- [ ] **16.4** Press only Ctrl while a key is being changed → Nothing is written until a real key comes
- [ ] **16.5** Settings → Keys, each section → Every row's key button lines up in one column at the right, also after a key is changed; the VIEW section has Keys (F1), the overview and the font sizes

## 17. What the settings' rows do

- [ ] **17.1** General → Start the server at sign-in on; sign out and in → `tsumugi ls` answers before the window is opened; the `tsumugi` value under `HKCU\…\Run` (macOS: the LaunchAgent; Linux: `~/.config/autostart/tsumugi-server.desktop`); off takes it away
- [ ] **17.2** Keep sessions running off; close the window with a shell open → `tsumugi ls` is empty afterwards
- [ ] **17.3** Ask before closing on; `sleep 100` in a pane; close the window → "A session is still running"; Cancel keeps the window; Close closes it
- [ ] **17.4** Check for updates on, with a build older than the latest release → A toast names the newer version, once a start
- [ ] **17.5** Default folder `~/dev`; start with no session → The first shell starts in `~/dev`
- [ ] **17.6** Appearance → Cursor, each of the six → The focused pane's cursor is a block, a bar or a line under, blinking when asked; a pane without the keys shows an outline
- [ ] **17.7** Nerd Font icons off, with a Nerd Font installed → The branch mark is drawn, not the font's icon
- [ ] **17.8** Keys → New session → press `Ctrl+Shift+W`, then `Ctrl+C` → The row says whose each is; nothing written; another key, or Use it anyway, writes it
- [ ] **17.9** `Alt+Shift+Arrows` in a split → The nearest divider moves that way a step at a time; the split is kept after a restart
- [ ] **17.10** Notifications → ▶ beside each sound, each of the four → Four different system sounds
- [ ] **17.11** Focus mode on in Windows; a session waits while the window is behind → No sound, flash or toast; the taskbar number still comes
- [ ] **17.12** Tell about a finish → 5 min; a 1-minute command finishes behind → No notice for it
- [ ] **17.13** Sessions → Default program → Shell → The new-session dialog opens with Shell picked
- [ ] **17.14** Claude Code command: a full path to `claude` → New Claude Code sessions run that program
- [ ] **17.15** Resume conversations off; restart with a Claude Code pane → It comes back as a plain shell, nothing typed
- [ ] **17.16** "Probably waiting" after 3; a program that prints nothing → Its card turns to probably waiting after about 3 s
- [ ] **17.18** Profiles → Edit: another folder and a pane on the right; Add a new one → `profiles.toml` has them; the new-session dialog's profile opens both panes
- [ ] **17.19** Tab menu: hide Pin, move Close the session up, add `lazygit -p {folder}` → The right-click menu follows; the item runs lazygit in the session's folder
- [ ] **17.20** Open with → Editor command `code {folder}` → Open in the editor opens VS Code in the session's folder; with one command the menu item has no ▶
- [ ] **17.21** Tags → New rule → Branch `claude/*`, tag `claude` → A session on a `claude/…` branch gets the tag
- [ ] **17.22** Edit a tag: rename it, pick a colour, Quiet → Renamed on the sessions and in the rules; the chip recoloured everywhere; its notices go to the bell only
- [ ] **17.23** Shell → Default shell, Arguments `-NoLogo`, Environment `FOO=1` → A new shell session runs that shell with the argument; `echo $env:FOO` prints 1
- [ ] **17.24** Shell integration → Install, then Remove (pwsh) → A marked block appears in `$PROFILE`, and goes; a new session with it reports its folder
- [ ] **17.25** Windows → ConPTY → Bundled, with the zip's build; the system's, with a bare exe
- [ ] **17.26** Advanced → Restart the server → The window says it is restarting; the tabs come back (or Welcome back); the uptime starts again
- [ ] **17.27** Graphics backend → Vulkan, then a name the machine has no adapter for, restart → Draws with it; for the missing one, the automatic choice and a line on stderr
- [ ] **17.28** Scrollback 200; a new session; `seq 1000` → Only about 200 lines to scroll back
- [ ] **17.29** Log what each pane sends and receives on; a new session → `pane-logs/session-N.log` beside the saved tabs grows as it runs
- [ ] **17.30** Export…, then Import on another machine (or after changing things) → One file in Downloads; importing brings the settings, themes and profiles back, the old ones kept as `.bak`
- [ ] **17.31** Keys → set a key to `Ctrl+[` by hand in `settings.toml`, then change another key and a font size in Settings → `settings.toml` still has every table and comment it had; only the changed lines differ
- [ ] **17.32** Put a mistake in `settings.toml` (`[broken`), then change anything in Settings → A toast says the file does not read and is left as it is; the file is unchanged
- [ ] **17.33** `[[menu.session]] command = "\"C:\\Program Files\\Microsoft VS Code\\Code.exe\" {folder}"` (Windows), then the card menu item → VS Code opens on the tab's folder (cmd took the line whole)
- [ ] **17.34** Open with → Editor: type `sakura {folder}` into Add another; right-click a card → `[open] editor = ["code {folder}", "sakura {folder}"]`; Open in the editor has ▶; a click on it opens VS Code, hovering opens `code` and `sakura` to its right and each opens its own; × on the second row (or emptying its field) leaves one again
- [ ] **17.35** Open with → take out every filer command; right-click a card → Open the folder in filer is greyed, its hover says where to set it
- [ ] **17.36** Tags → Tags shown 1, then 5 → The band and the cards show one tag and `+N`, then all five
- [ ] **17.37** Tags → a rule's Edit: change its folder, then add a branch, Save; Edit another and take it out → `settings.toml` has the rule changed in place (one rule with both, then neither); the sessions it tagged before lose the tag, those it now matches get it

## 18. From a script

- [ ] **18.1** `tsumugi ls --json` (PowerShell: `tsumugi ls --json \| ConvertFrom-Json`) → One array, a session an object: id, state, command, agent, cwd, project, branch, title, note, tags, ports
- [ ] **18.2** `tsumugi send N "echo hi"` → `echo hi` typed into session N and run
- [ ] **18.3** `tsumugi read N --lines 5`; `tsumugi read N --all > out.txt` → Its last five lines; the whole scrollback in the file, Japanese whole
- [ ] **18.4** `tsumugi split N --down -- claude` → A pane below session N in its folder with Claude Code started; its number printed; the window shows the split
- [ ] **18.5** `tsumugi wait N --state done --timeout 600` while Claude Code works there → Returns `done` (exit 0) when it finishes; with a short timeout, exit 1; for a closed session, exit 3
- [ ] **18.6** `tsumugi close N` → The session ends and its pane goes

## 19. Keys through every screen

- [ ] **19.1** The new-session dialog: `Tab` all the way round, then `Shift+Tab` all the way back → The folder, the way picked in START (one stop), + Pane, the tag, the two ticks, Cancel, Create, the folder again; the log's positions go left to right, top to bottom; back the same in reverse
- [ ] **19.2** In START, `→` four times, then `←` four times → Claude Code, Resume last, Shell, Profile…, round to Claude Code; one button lit at a time: the way picked, filled, with the ring; on Profile… it is filled with the ring, as the others, and nothing else is lit; back on Shell it is filled again
- [ ] **19.3** Settings, each page in turn: `Tab` through it → Every switch, list, field and button on the page, in reading order; inside a row of several (Tags' New rule, a key and Its own, a sound and ▶, the menu's ↑ ↓ and switch) left to right
- [ ] **19.4** The search box (`Ctrl+Shift+P`), the waiting list (`Ctrl+Shift+Y`), the bell's list (`Ctrl+Shift+N`), Start in parallel → Each opens with the keys in its first field or line; `Tab` walks it in reading order; `Esc` closes it and the keys go back to the pane they came from
- [ ] **19.5** Every dialog and list in 19.1 to 19.4: `Enter` on a field, and on each button → As the dialog's foot says: Enter in a field does the main thing; on a button it presses that button; nothing reaches the shell behind
- [ ] **19.7** Every dialog and form with buttons: New session, Start in parallel, Remove the worktree?, closing a running tab, Settings' profile and environment editors, the waiting list's Always allow… → The same everywhere: the buttons at the right, Cancel (or Keep) on the left of the main one, on one line, the same height; the focus log's x for Cancel smaller than the main one's, and the main one's right edge at the dialog's right edge less its margin; Tab reaches Cancel before the main one
- [ ] **19.8** The same screens: their headings, and the key named beside a heading → A dialog's title at its top left, its key (where it has one) at the top right in the terminal's font; a section's heading in small capitals above its card; the same gaps between them on every screen (a screenshot each, `[~]`)
- [ ] **19.6** The same screens at 150% (Windows display scale) and with the window 1000 px wide → Nothing cut, nothing on top of something else, no row's words touching the line under it (a screenshot each, `[~]`)
