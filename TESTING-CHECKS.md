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

## 3. The sidebar

- [ ] **3.1** Open three tabs in different git repositories → Each card: name, folder and branch, the state and how long
- [ ] **3.2** The **+** beside SESSIONS, and the one at the top of the rail → The new-session dialog opens with the focused pane's folder
- [ ] **3.3** Sort button → each of the five orders → The cards reorder accordingly; **Manual** keeps a dragged order across restarts
- [ ] **3.4** Drag a card in Manual order → The grip shows on hover; the card lands where it is dropped
- [ ] **3.5** The state and folder filters at the foot → Only matching cards; the count says `N of M`
- [ ] **3.6** Right-click a card: rename, tag, mute, pin, restart, duplicate, new window, open in editor / filer, copy path, close → Each does what it says; close is red and last; Rename, Duplicate and Close show their keys on the right, in grey
- [ ] **3.7** Sort → One line each; then `Ctrl+Shift+B` → One line per tab; then the 60px rail with first letters and state rings
- [ ] **3.8** Rest the pointer on a card of another tab → Its last 12 lines in a box, the waiting pane's in a tab of several
- [ ] **3.9** A tab in a repository with uncommitted changes → `+N −M` (or `N new`) on the card's second line; on hover, the files as `git status` lists them
- [ ] **3.10** Drag the sidebar's edge left past 120px, and back → It becomes the rail; dragged back out, the sidebar
- [ ] **3.11** One session of each state (notify them) and a plain shell → Each card ringed in its state's colour with its own mark -- a clock (waiting, breathing), a turning arc (running), a dotted circle (probably waiting), a triangle (error), a tick (done, its ground sunken) -- and the shell grey with a small dot and no words; the foot's filters and the status bar count the shell as Shell, not Running
- [ ] **3.12** Two waiting, then `[keys] next_waiting = "F8"` → Under a line at the foot: the key as a cap (`F8` once changed) and **Jump to waiting · 2** in grey; a click on either jumps
- [ ] **3.13** Theme tsumugi Light (and each light theme) → The waiting and error words and counts read clearly on white (WCAG 4.5 or more)
- [ ] **3.14** A tab on a branch with an open pull request (`gh` signed in) → A line on its card: `PR #N · checks passing` (green), `failing` (red), `running` (gold) or `no checks`
- [ ] **3.15** The card of the tab with the keys, Claude Code talked to a few times → A line `N prompts · 12m · 3.4k tokens`; the other cards have none. A running card's third line ends with `· N tokens`
- [ ] **3.16** `F2` → A field on the card with the name selected; Enter renames, Esc leaves it as it was
- [ ] **3.17** `Ctrl+Shift+D` (macOS `Cmd+Option+D`) in a Claude Code pane, then in a shell → A new tab in the same folder: `claude` typed in the first, a shell in the second
- [ ] **3.18** Look at the sidebar on first start → 288px wide
- [ ] **3.19** Right-click a card → Note…, type `the release`, Enter; restart the machine (or the server) → `“the release”` on the card under the folder; Edit the note… and an empty Enter takes it off; after the restart the note is back

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
- [ ] **4.10** The bell → The list of notices, newest first; a click goes to the session and marks it read
- [ ] **4.11** `printf '\e]9;hello\a'` in a pane → Marked waiting with `hello`, no hooks needed
- [ ] **4.12** `Ctrl+Shift+U` with two waiting → The one waiting longest first, then the other
- [ ] **4.13** Two Claude Code sessions asking permission, then `Ctrl+Shift+Y` (or **List ›** at the sidebar's foot) → A panel: each waiting session with what it said, how long, its choices as buttons and a tick; a choice's button types its number there
- [ ] **4.14** In that panel, **Yes to 2** → Both go on as if `1` were typed in each; untick one first and only the other is answered; **No to N** types each menu's "No" choice
- [ ] **4.15** A session waiting with a menu of other words (no "Yes"/"No") → Not counted in Yes to N / No to N; its own buttons still work
- [ ] **4.16** `exit` a Claude Code session after some work; then the panel's **Recently closed** (or search `recently`) → It is listed: today at …, done, its tokens; **Last output** shows its last lines; **Resume** opens a new tab in its folder typing `claude --resume <id>`; the list survives a restart of tsumugi

## 5. Search

- [ ] **5.1** `Ctrl+Shift+P`, type part of a session's name → That session first; Enter goes there
- [ ] **5.2** Type a folder another session is in → **Folder** entries: Enter starts a session there
- [ ] **5.3** Type `split` → The commands, with their keys beside them
- [ ] **5.4** Type three letters printed long ago in another tab's scrollback → **IN THE SCROLLBACK** lines below the rest; picking one goes to that tab, scrolls back to the line, the match selected
- [ ] **5.5** `Esc`, and a click outside → Closes without doing anything

## 6. New sessions

- [ ] **6.1** `Ctrl+Shift+T`, Enter → Claude Code starts in a new tab in the focused pane's folder
- [ ] **6.2** `Ctrl+Shift+T`, Shell, `Alt+Enter` → A shell split to the right instead of a tab
- [ ] **6.3** Type part of a folder, `Tab` → Completes from the folders listed, including **closed** ones from sessions that ended; the cursor at its end; a second `Tab` moves on
- [ ] **6.4** Add a tag, and take a dashed (folder rule) tag off → The new session wears exactly what the dialog showed
- [ ] **6.5** Beside it → + Pane twice (Claude Code, Shell), Create → One tab of three panes: the first, one to its right, one below that
- [ ] **6.6** Tick Save as a profile, name it; next time Profile… → it → The folder, start, tags and panes come back
- [ ] **6.7** Tick In a new git worktree in a repository, Create → A folder `<repo>-tsumugi-MMDD-HHMM` beside the repository on its own branch, and the session in it; a toast says so
- [ ] **6.8** `exit` the last session in that worktree → **Remove the worktree?**; Remove takes the folder (the branch stays); with uncommitted changes git refuses and the toast says why
- [ ] **6.9** `Ctrl+Shift+T`, then only the keyboard: `Tab` through the dialog, `←`/`→` on the ways to start, `Space` on a button, `Shift+Tab` back → A cyan ring shows where the keys are; Tab completes the folder once and then moves on, through the start buttons, + Pane, the tag, the two ticks, Create and Cancel; `Enter` on Cancel cancels, `Enter` in a field creates

## 7. The input box

- [ ] **7.1** `Ctrl+I`, write two lines (Enter between), `Ctrl+Enter` → The box opens inside the focused pane's card, at its foot, the terminal shortened above it; both lines reach the session as one prompt, then Enter
- [ ] **7.2** `↑` in the empty box → The last prompt sent; `↓` back
- [ ] **7.3** Drop two files on the window → Chips in the box; sent, their paths follow the text, quoted when they have spaces
- [ ] **7.4** Pick a tag as **To**, send → Every session wearing the tag gets it
- [ ] **7.5** While a session runs, **When done** (or `Ctrl+Shift+Enter`) → `1 queued` on the card and in the box; when the session is done, the prompt goes and a toast says so
- [ ] **7.6** Queue to a session showing a permission menu → It waits until the menu is answered and the session is done
- [ ] **7.7** `Esc` → The box closes and the keys go back to the pane; the draft is kept for next time
- [ ] **7.8** Take a screenshot (`Win+Shift+S`), `Ctrl+V` in the box → A chip `▣ paste-<date>-<time>.png · N KB`; the PNG is in the settings folder's `attachments`; sent, its path follows the text
- [ ] **7.9** `Ctrl+V` with text on the clipboard → The text is pasted; no chip
- [ ] **7.10** Drop a file of a few hundred KB → Its chip says its size, `▤ name · 214 KB`
- [ ] **7.11** Split the tab, open the box, move the keys with `Alt+Arrows` → The box follows the pane with the keys; a pane narrower than 20 columns has none
- [ ] **7.12** **+ Sessions** in the box, tick another session, `Esc`, write a prompt, `Ctrl+Enter` → `+ name` beside the pane under To; the menu's Esc does not reach the shell; both sessions get the prompt; a click on `+ name` takes it off

## 8. Restoring after a restart

- [ ] **8.1** Split tabs with Claude conversations, then reboot (or stop the server) and start tsumugi → **Welcome back** lists the tabs; Restore brings their splits and folders back
- [ ] **8.2** A restored Claude Code pane → `claude --resume <id>` was typed: the conversation is back
- [ ] **8.3** Settings → General → On start → Restore the last sessions, then 8.1 → No Welcome back; the tabs simply return
- [ ] **8.4** 8.1 with fourteen or more tabs → The list scrolls; Restore and Start fresh stay in sight; the time reads `today at …` or `yesterday at …`

## 9. The settings screen

- [ ] **9.1** `Ctrl+,` → The screen with its nine pages and the search field over them; the band says Settings in the middle with an X; no sidebar or status bar; `Esc` and the X close it
- [ ] **9.2** Change something on each page → Only that line of `settings.toml` changes (diff the file); comments stay
- [ ] **9.3** Write a mistake into `settings.toml` by hand → A red line above the status bar says what and where, until it is fixed; nothing else changes
- [ ] **9.4** Open settings.toml / Open the settings folder → The system's editor / file manager opens
- [ ] **9.5** Type `scroll` in Search settings → Only Advanced in the list, with a count; its Scrollback row lit; Enter goes there
- [ ] **9.6** Next to the design's "Settings: every page" → The same pages, sections and rows in the same order
- [ ] **9.7** Only the keyboard: `Ctrl+Tab` / `Ctrl+Shift+Tab` through the pages, `Tab` through a page, `Space` on a switch, `Esc` twice → A cyan ring on the control with the keys; Tab never stops on the top band or the list of pages; Space flips the switch (the file changes); the first Esc leaves the control, the second closes the screen; nothing typed reaches the shell behind it
- [ ] **9.8** Notifications → WHEN A SESSION… → A line between rows; the three state columns the same width, each switch in the middle of its column and row, under its heading's dot

## 10. Themes

- [ ] **10.1** Settings → Theme, walk the thirteen → The window and the panes recolour at once; states stay readable (gold, cyan, red, green)
- [ ] **10.2** `theme = "system"`, then switch Windows between dark and light → tsumugi follows within seconds
- [ ] **10.3** Save a Windows Terminal scheme `.json` in `themes\` → It is in the list under its name; chosen, `ls` colours match Windows Terminal's with that scheme
- [ ] **10.4** The same with an iTerm2 `.itermcolors` → As 10.3
- [ ] **10.5** `theme.toml` with one colour → Only that colour changes in the theme in force
- [ ] **10.6** Settings → Theme → Mode (Dark, Light, System) on the right of the heading; PREVIEW names the theme in force; a line with the font, the window and motion, a click goes to Appearance; each theme's swatches bordered and rounded

## 11. Fonts

- [ ] **11.1** Settings → Appearance → Font list → The installed monospace fonts (Cascadia, Consolas, any Nerd Font)
- [ ] **11.2** Pick Cascadia Code; `printf '\e[1mbold\e[0m \e[3mitalic\e[0m'` → The pane in Cascadia; bold and italic in their own faces; the line under the list names the files found
- [ ] **11.3** Consolas → Its bold and italic found too (`consolab.ttf`, `consolai.ttf`)
- [ ] **11.4** Size and line height: type a value, Enter → The grid re-fits; text in the middle of taller rows; `tsumugi ls` shows the new columns × rows after a moment; `Esc` in the field leaves it as it was
- [ ] **11.5** Cascadia Code or Fira Code, `echo '-> != == >= => |> www'` → Each shown as one sign, on the grid; Ligatures off draws them as plain characters
- [ ] **11.6** Japanese text and a Nerd Font icon in a prompt → Japanese from the system font, icons from the Nerd Font, neither as boxes

## 12. Claude Code's hooks

- [ ] **12.1** Without the hooks in `~/.claude/settings.json`, start tsumugi → On the first-run screen, the gold-ringed card with the two hooks under the folders; with sessions there already, a short card at the bottom right, above the input box when it is open
- [ ] **12.2** Add the hooks → Both in the file beside any there, `settings.json.tsumugi-backup` next to it, and a toast
- [ ] **12.3** Not now / Don't ask again → Asked again at the next start / never again
- [ ] **12.4** Settings → Shell & hooks → Claude Code hooks → Add → Added, the row says Installed; Remove takes only tsumugi's out, the backup beside it

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

## 16. Keys

- [ ] **16.1** Settings → Keys → click New session, press `Ctrl+Shift+N` → `[keys] new_tab = "Ctrl+Shift+N"` in the file; `Ctrl+Shift+N` opens the dialog; `Ctrl+Shift+T` goes to the shell
- [ ] **16.2** Its own → The key goes back; the search box and the dialog name the key in force
- [ ] **16.3** `zoom = "none"` → `Ctrl+Shift+Z` reaches the shell
- [ ] **16.4** Press only Ctrl while a key is being changed → Nothing is written until a real key comes

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
- [ ] **17.17** Offer compact rows above 3; four tabs → The offer of one line each comes
- [ ] **17.18** Profiles → Edit: another folder and a pane on the right; Add a new one → `profiles.toml` has them; the new-session dialog's profile opens both panes
- [ ] **17.19** Tab menu: hide Pin, move Close the session up, add `lazygit -p {folder}` → The right-click menu follows; the item runs lazygit in the session's folder
- [ ] **17.20** Open with → Editor command `code {folder}` → Open in the editor opens VS Code in the session's folder
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
