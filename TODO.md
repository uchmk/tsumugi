# TODO

tsumugi のやること。`【人】` は持ち主の作業、`【金】` は 2026-10-09（金）以降に始めるもの
（今週は利用枠の上限に近いため。2026-10-07、持ち主の判断）。

## 金曜日から（2026-10-09 以降）

- [x] （2026-10-09 に x64、2026-10-10 に ARM64 のレーンが回り始めた）【人】【金】Windows 実機のテストを始める。`C:\dev\tsumugi` から worktree（`C:\dev\tsumugi-wintest`）を作り、`-DryRun` で塊を見てから 1 回手で回し、
      スクリプトの冒頭のとおりタスク スケジューラに毎時 :50 で登録する。役割は `.claude/windows-role.md`、チェック表は TESTING-CHECKS.md（303 行）と
      TESTING-KEYS.md（77 個）。CLI とサーバの行は `tests/cli.rs` で CI に移した（TESTING.md の「Covered by tests」）。
  - [x] （2026-10-10、持ち主が用意し、ARM64 のレーンも回っている）【人】ARM64 のノート PC も使うなら `-Lane arm`（`C:\dev\tsumugi-armtest`、タスク名 `tsumugi-auto-wintest-arm`）。
  - [x] （v0.74.4、クラウドのマージの Routine が毎時 :40 にマージする。手順は v0.74.5 の `.claude/merge-routine.md`）実機の PR（`test/win-*`・`test/arm-*`）をマージする側を決める。
  - [ ] 【実機】最初の 3 回の実行時間（`auto-wintest.log` の開始と終了）と使用量の増え方を見て、1 回 1 塊の目安をここに書く。
- [x] （filer v0.79.0 で `rev` を `fc88385`（v0.65.1）に上げた。それ以後 v0.68.0 まで `crates/tsumugi-pane` は変わっていない）【金】filer の `tsumugi-pane` の `rev` を上げる。filer の `Cargo.toml` の `rev` は `d2405616` のままで、それ以後のペインの直し
      （ペインごとの文字コード `Charset` と `encoding_rs`、`all_text`、`sys` のプロセスとポートの検出、`Terminal::shell_pid()` など）が filer に届いていない。
      上げたら filer で `cargo build` して `Cargo.lock` を合わせ、filer の `scripts/verify.sh` を回す。`Palette` などの形が変わっていれば filer の
      `src/ui/term.rs` を合わせる。filer の TESTING.md の 1・19・29・40 節（ペイン）を filer の実機の再テストに積む。
  - [x] （filer v0.79.5。filer には引ける境目が無く（列は yazi の `ratio`、ターミナルは 35% 固定）、置き換えではなく新しく足すかの決定になる。依存の追加なので filer の Q94 で持ち主に聞いた。回答は 1 で、filer v0.80.0 がターミナルペインの上の境目を `dividers` で引けるようにした。列は `ratio` のまま）同じときに、filer の 2 分割の境目を `tsumugi_layout::ui::dividers`（v0.52.0、`egui` 機能）に置き換えられるかを見る。
      filer の分割は「2 つのタブを横に並べる」形なので、`tsumugi_layout::Node` に載せ替えるかどうかは filer の QUESTIONS.md で聞く。
- [x] （v0.68.4 で README に載せた。紹介画像も持ち主が上げた）【人】README の画像を撮る。`docs/images/` に `split.png`・`claude.png`・`tsumugi.gif` を置く（何を写すかと大きさは `docs/images/README.md`）。
      置いたら対話のセッションが README の冒頭に載せる。紹介画像 `docs/social-preview.png` は Settings → General → Social preview から上げる（API が無い）。
- [x] 最初のリリースを切る。v0.52.0 を 2026-10-07 に出した（6 つの成果物と SHA-256 の表が揃った。run 37577264990）。

## uchmk の共通仕様（2026-10-10、持ち主の依頼）

設定の画面・テーマ・時計を uchmk のアプリで揃え、common.toml を変えれば全部のアプリが変わるようにする。仕様は [docs/common-spec.md](docs/common-spec.md)。

- [x] （v0.81.0）共有のクレート `tsumugi-common`（common.toml・設定の 1 キーの書き換え・見張り・時計の文字）、`tsumugi-theme`（テーマ）、
      `tsumugi-prefs`（設定の画面の枠と部品、Language・CLOCK・Theme の共通のページ）を切り出し、tsumugi の設定の画面をその上に作り直した。
      テーマと時計は common.toml を読み書きする（settings.toml の古い値は common.toml に無ければ使う）。`uchmk/themes/` も読む。
- [ ] （mimamori v0.32.0 で）mimamori に設定の画面・テーマ・時計を足す。
- [ ] filer に持ち帰る: Ctrl+, の設定の画面を `tsumugi-prefs` で作り、ステータスバーの右端に時計を出し、common.toml のテーマと時計を読む。
      yazi の `theme.toml` との関係は filer の QUESTIONS.md で持ち主に聞く。filer の TODO.md の「設定の画面（uchmk の共通仕様）」で進める。
- [ ] tsumugi 自身のメニューを `tsumugi-i18n` の表に載せ、設定の画面の文字を `Words::of(language)` で日本語にも出す（今は英語だけ、TESTING.md 17.38）。

## 実機のレーンから

マージの Routine（`.claude/merge-routine.md`）が、実機の PR の `### Proposals`・見つけた不具合・`## Queue` の頼みをここに積む。対話のセッションが拾う。

- [x] （v0.76.0）実機 #1〜#5 の不具合と提案を入れた。仕切りのドラッグ（2.3・2.15）、`Ctrl+Shift+-`（2.24）、コピーモードの選択とバッジ（2.25・2.26）、
      `PromptLinks`（pwsh・Starship のプロンプトジャンプとコマンドのバー、2.27・2.42・2.44）、MSYS の `/c/...`（2.28）、`Ctrl+Shift+X` と
      パレットの入れ替え・均等割り（2.37）、検索バーの `Alt+矢印`（2.31）、ConPTY の `Ctrl+C`（2.58）と `Shift+Enter`（2.55）、`CSI 16 t`（2.48）、
      フォーカスの `\e[O`（2.53）、設定の検索の索引、`TERM_PROGRAM`（2.54）。キットに `Send-Keys -Hold`・`vk:0x..`・`Send-Drag`・`Send-Wheel`・
      `Send-Click -Middle`・入力待ち、`Invoke-Tsumugi` は位置引数を受ける。キューは `Linux/macOS:`・`A person:` の行と ARM64 の 2.46〜2.49 を渡さない。
      2.10・2.18・2.19 は人の行にし、2.28 の zsh は「入っていれば」、2.29 は `291`、2.30 は「Wrapped」、2.40 は `"r"` イベント、2.45 は「2.42 のあと」。
      直した行は `.claude/windows-role.md` の再テストの列に積んだ。
- [ ] 【実機】2.52（SGR の下線・打ち消し線・隠し文字）は Linux では再現しなかった。再テストで落ちたら PTY ログの `out` 行を報告に貼ってもらい、それから直す。
- [ ] （実機 #7）`tsumugi new` が argv を受ける（または語ごとに引用する）ようにする。`Program Files` の bash.exe を渡すと pwsh が `ParserError` になる（`qa-reports/2026-10-10-arm-2-28.md`）。
- [ ] （実機 #7）TESTING.md 2.45 に「フック付きの bash のペインでも成り立つ」と書く（同レポート）。
- [ ] （実機 #7）最後のセッションを閉じるとサーバーも止まり、次の起動の Welcome back ダイアログがキーを取る。入力が吸われた（同レポート。不具合ではない）。
- [ ] 【実機】`Ctrl+=`（`font_bigger`）を `'Ctrl+vk:0xBB'` で確かめる（JIS 配列のレーン）。
- [ ] （実機 #9）**Last-output copy and prompt jumps with multi-line prompts** (2.27, 2.44): start at the row after the *input* mark (OSC 133 `B`/`C`), or the row after the last prompt row, so a two-line prompt does not leak the command row into the copy. Small (one function in the OSC handling plus a test with a two-line prompt).（qa-reports/2026-10-10-arm-retest-1-7.md）
- [ ] （実機 #9）**Log the keys egui turns into events** (Cut/Copy/Paste): add a `key X` line, or an `event Cut` line, to `TSUMUGI_KEYLOG`, so `Ctrl+Shift+X` and its siblings can be read as text. Small.（qa-reports/2026-10-10-arm-retest-1-7.md）
- [ ] （実機 #9）**A CLI or log line for the divider under the pointer** (for example a `hover divider x,y` line in the key log): rows 2.3 and 2.15 are pictures now and could be read. Small.（qa-reports/2026-10-10-arm-retest-1-7.md）
- [ ] （実機 #9）**Older-server fixture**: keep an older `tsumugi.exe` (0.46.x) in the repo's releases or the scratch cache so 1.7, 1.11 and 1.12 can be run by the unattended lane. Medium.（qa-reports/2026-10-10-arm-retest-1-7.md）
- [ ] （実機 #9・キュー）Keep 1.7, 1.11, 1.12 in the re-tests, but they need the older build first (see the last proposal); until then the lane keeps offering rows it cannot do. Move them out of the ARM64 re-tests, or give them to a run with the old build.（qa-reports/2026-10-10-arm-retest-1-7.md）
- [ ] （実機 #9・キュー）Keep 2.3, 2.15 in the re-tests (retry with the divider's exact coordinates, or after the hover log exists).（qa-reports/2026-10-10-arm-retest-1-7.md）
- [ ] （実機 #9・キュー）Keep 2.27, 2.44, 2.37 in the re-tests; they are not done, and the first two will fail until the multi-line prompt is handled.（qa-reports/2026-10-10-arm-retest-1-7.md）
- [ ] （実機 #9・キュー）Drop 2.24 from the ARM64 lane (the bash path is not available here).（qa-reports/2026-10-10-arm-retest-1-7.md）
- [ ] （実機 #9・キュー）2.25, 2.26, 2.42, 2.53 can leave the re-tests (now `[x]`); 2.29 and 2.31 are `[~]`, the owner's to confirm.（qa-reports/2026-10-10-arm-retest-1-7.md）
- [ ] （実機 #10）**Kit: double-click, window resize, cursor-shape and "same-second" helpers.** I wrote `GetCursorInfo`, a double-click and `SetWindowPos` by hand for 2.3/2.15/2.70. A `Send-DoubleClick`, `Get-KitCursor` and `Set-KitWindow` in `wintest-kit.ps1` would stop every lane from doing it again. Small.（qa-reports/2026-10-10-win-retest-1-7.md）
- [ ] （実機 #10）**Kit: keep the foreground.** The owner's Claude desktop takes it between two sends; for a two-input race (2.70's `-2`) the kit should send both inputs inside one `SendInput` batch or hold the desktop lock with `LockSetForegroundWindow`. Small–medium.（qa-reports/2026-10-10-win-retest-1-7.md）
- [ ] （実機 #10）**Make 2.70's `-2` a test** in `crates/tsumugi/tests/cli.rs` (two `log` requests with the same path) instead of a real-machine row; it is server logic with a file as the result. Small.（qa-reports/2026-10-10-win-retest-1-7.md）
- [ ] （実機 #10）**Read the log keys.** `Ctrl+Shift+X` (and Cut/Copy/Paste) never show in `TSUMUGI_KEYLOG`; a `key X` or `event Cut` line would let 2.37 be read as text. Small.（qa-reports/2026-10-10-win-retest-1-7.md）
- [ ] （実機 #10）**Large OSC 1337 images are slow** (37–91 KB: 10–15 s before the prompt returns). Decoding or the escape parser looks quadratic. Medium; worth a profile.（qa-reports/2026-10-10-win-retest-1-7.md）
- [ ] （実機 #10）**A `tsumugi log N [PATH]` CLI** would let a lane start and finish work logs without the key or the menu, and make 2.70/2.71 text rows. Small.（qa-reports/2026-10-10-win-retest-1-7.md）
- [ ] （実機 #10）**Older-server fixture** for 1.7/1.11/1.12: keep one old `tsumugi.exe` (0.46.x) next to the scratch cache, or let `scripts/auto-wintest.ps1` fetch the previous release's `.zip` from GitHub into `kit\old\`. Medium.（qa-reports/2026-10-10-win-retest-1-7.md）
- [ ] （実機 #10・キュー）Take 2.58, 2.24, 2.44, 2.27, 2.3, 2.15, 2.71, 4.16 out of the re-tests (now `[x]`), and 2.54, 2.48 (`[~]`, the owner's to confirm).（qa-reports/2026-10-10-win-retest-1-7.md）
- [ ] （実機 #10・キュー）Keep 2.70 in the re-tests, but only for the `-2` file; it is better moved to `cli.rs` (see Proposals) and the row's last clause then lives in "Covered by tests". Everything else in it passed on the real exe, key and menu.（qa-reports/2026-10-10-win-retest-1-7.md）
- [ ] （実機 #10・キュー）Keep 2.37 in the re-tests until `Ctrl+Shift+X` can be seen reaching tsumugi (key-log line, or a machine where the chord is not swallowed). Do not offer it to this machine's lane again before then.（qa-reports/2026-10-10-win-retest-1-7.md）
- [ ] （実機 #10・キュー）Keep 1.7, 1.11, 1.12 out of the lanes until an older server build can be fetched (they were offered here and could not be done).（qa-reports/2026-10-10-win-retest-1-7.md）
- [ ] （実機 #10・キュー）`TESTING-KEYS.md` needs regenerating on `main` (see Findings) before the keys chunk is offered again; `Ctrl+Shift+S` and `Ctrl+Shift+O` are then unticked keys for the next x64 run.（qa-reports/2026-10-10-win-retest-1-7.md）
- [ ] （実機 #10・キュー）Keep 2.70 for the `-2` file only; better moved to `cli.rs` as a test, then "Covered by tests".（PR 本文）
- [ ] （実機 #10・キュー）Keep 2.37 until `Ctrl+Shift+X` can be seen reaching tsumugi; do not offer it to this machine's lane before then.（PR 本文）
- [ ] （実機 #10・キュー）Keep 1.7, 1.11, 1.12 out of the lanes until an older server build can be fetched.（PR 本文）
- [ ] （実機 #10・キュー）Regenerate TESTING-KEYS.md on `main` before the keys chunk is offered again.（PR 本文）
- [ ] （実機 #10・キュー）🤖 Generated with [Claude Code](https://claude.com/claude-code)（PR 本文）

## ほかのターミナルにあるもの（2026-10-08、持ち主の依頼で全部取り込む）

軽いものから順に入れる。重いものと、範囲を決めてからのものは後ろ。

- 軽いもの
  - [x] プロンプト間のジャンプ（OSC 133;A、`Ctrl+Shift+Up` / `Down`）。シェルフックと pwsh の自動のフックが印を出す。
  - [x] ペインの中の検索（`Ctrl+Shift+F`、右上のバー、Enter / Shift+Enter、Esc はシェルに届かない）。
  - [x] 貼り付けの警告（bracketed paste でない複数行と 5 KB 以上。設定で切れる）。
  - [x] OSC 8 のハイパーリンク（Ctrl+クリックで開く、点線の下線）。
  - [x] ペインの入れ替え・均等割り・別のタブへ移す（キーで）。
  - [x] SSH / WSL のプロファイル（新しいセッションのダイアログから選ぶ）。
- 中くらい
  - [x] 背景の不透明度・背景画像。
  - [x] セッションの録画（asciinema v2 の `.cast`）。
  - [x] Quake モード（全体のホットキーで上から出し入れ）。
- 重いもの
  - [x] 画像の表示（sixel・kitty・iTerm2）。
  - [x] コマンドブロック（OSC 133 の B・C・D でコマンドと出力をまとめる）。
- [x] 設定のスクリプト（Lua か Rhai か）と、SSH 先のサーバーにつなぐ mux は QUESTIONS.md で範囲を決めてから（Q14・Q15 に書いた）。
  - [x] 【人】Q14・Q15 の回答（Q14 は保留、Q15 は 1）。
- Q15: SSH 先の mux（OS の `ssh` で SSH 先の `tsumugi proxy` につなぐ）
  - [x] `tsumugi proxy` と、外からの操作の `--host H`（v0.66.0）。
  - [x] 窓: 複数のサーバーにつなぐ。新しいセッションのダイアログで「SSH 先」を選び、その機械のセッションをサイドバーの別の組に並べる。切れたら組に印を付け、つなぎ直す。
    窓は一度に 1 台を見せ、ほかはサイドバーの MACHINES に数だけ出す（v0.67.0）。worktree・git・閉じたフォルダ・知らせは見せている機械のものだけ。
  - [x] つないだ機械を覚えて、窓を開き直したときにつなぎ直す。見せていない機械の待ちも知らせる（v0.68.0。`[remote] hosts`、行の右クリックで Forget）。
  - [x] 遅い回線: 画面の更新を間引き、画像は送る大きさに上限を付ける（v0.68.0。ssh 越しは 100 ms ごとに差分、画像は 1 MB まで）。
  - [x] SSH 先に tsumugi が無いときは入れ方を出す。SSH 先のコマンド（PATH に無いとき）を設定で変えられるようにし、版が違うときは何をすればよいかを言う（v0.68.0。`[remote] command` / `commands`、Restart its server）。

## ほかのターミナルにあるもの・その 2（2026-10-09、持ち主の依頼。上から順に）

ペインの直しは `tsumugi-pane` に入れるので、filer のターミナルペインにも `rev` を上げれば届く。

- [x] 下線と取り消し線を描く（SGR 4・4:2〜4:5 の二重・波線・点線・破線、58 の下線の色、9 の取り消し線、8 の隠し文字）。今は太字・斜体・薄字・反転しか描いていない。
- [x] （v0.70.0）フォーカスの通知（DECSET 1004）。ペインがキーを持つ・離すときに `ESC[I` / `ESC[O` を送る。
- [x] （v0.70.1）端末の名乗りに答える（DA1・DA2・XTVERSION・DECRQM）。足りない返事を足し、テストで確かめる。同期出力（DECSET 2026）が `alacritty_terminal` の中で効いているかもテストで確かめる。
- [x] （v0.71.0）kitty のキーボードの取り決め（CSI u。プログラムが頼んだ段だけ）。Shift+Enter・Ctrl+Enter などを区別して送れるようにする。キー変換の作り直しになる。
- [x] （v0.72.0）quick-select（WezTerm の Quick Select・kitty の hints）。画面の URL・パス・ハッシュなどに文字の印を付け、押した印のものをコピー（Shift で開く）。
- [x] （v0.73.0）選んだらコピー（設定）・矩形選択（Alt+ドラッグ）・Linux の PRIMARY と中クリックの貼り付け。
- [x] （v0.74.0）トリガー（iTerm2）: 正規表現が出力に当たったら、色を付ける・知らせる。
- [ ] 録画（`Ctrl+Shift+R`）の最初に、始めた時点の画面を 1 つ目の出力として書く。今は始めた後の出力だけなので、何も出さずに止めるとヘッダーだけの `.cast` になり、再生しても始めの画面が無い（2026-10-10、持ち主の質問から）。
- 後で（範囲を決めてから）
  - [ ] プロファイルごとのテーマとフォント、ペインの読み取り専用、視覚ベル。
  - [ ] 窓を複数持つ・タブを窓の外へ引き出す、スクリーンリーダー（AccessKit）、右から左の文字（bidi）。
  - [ ] シェーダー・カーソルのアニメーション、tmux の control mode、シリアル接続、macOS の Secure Keyboard Entry（macOS の窓と一緒に）。

## ソースレビュー（2026-10-07）の残り

v0.53.1〜v0.53.3 で重いものと手早いものを、v0.53.4 で残りを直した。実機で確かめるのは TESTING.md の 2.16〜2.20、3.29、4.24、7.15、7.16、9.9、12.6、17.31〜17.33。

- [x] Windows のメニューのコマンドが `"` で始まると cmd が引用符を外して壊れる（`menu.rs` の `cmd /C`）。`/S` の形（全体を `"…"` で囲む）にする。`{folder}` の `%VAR%` も。
- [x] 「Always allow」のルールに `*` が入るとき、または長いコマンドが画面で折り返されたときは作らない（`permit.rs`）。`*` を含むものと、2 行目が説明に見えない（大文字で始まらない）ものは作らない。
- [x] 自前の JSON（`json.rs`）: サロゲートペアの `\u` を 1 文字に、`\b` `\f`、知らないエスケープは誤り、`1e400` を書かない、BOM を読み飛ばす。
- [x] Shift_JIS などのペインで、その文字コードに無い文字（絵文字）が `&#128512;` で送られる（`charset.rs` の `encode`）。`?` にする。
- [x] AltGr（ドイツ語配列など）の記号に Ctrl+Alt のキーが余計に付く（`input.rs`）。Ctrl と Alt が両方で印字できるキーは、次に文字が来るなら文字だけ送る（2.18 で実機）。
- [x] Windows の `end_tree` が PID の使い回しで止まらない／関係ないプロセスを止めうる。プロセス表を 1 回取って `descendants` で辿る。
- [x] 使用量（`usage.rs`）: `"type":"assistant"` の行だけ読む（サブエージェントの二重計上の疑い）。7 日を過ぎたファイルを忘れる。`get()` を 1 コマ 1 回に。
- [x] 費用の知らせ（`spend.rs`）: 「その日」を日付で持つ（値段を下げると同じ日にもう一度知らせる）。
- [x] 検索の列が、小文字にすると長さの変わる文字（`İ`）でずれる（`grid.rs` の `find_lines`）。
- [x] `view.rs` の `fit` で `row_h` が 0 のときの守り、`window_size` の `u16` への切り詰め。


## ソースレビュー（2026-10-09）の残り

v0.74.2 で 8 件のうち 7 件を、v0.74.3 で残りを直した（kitty のキーの離し、全画面の行を読まない、トリガーの色の `^`/`$`、壊れた設定で規則を戻さない、クイック選択の Shift、ESC/CAN/SUB）。実機は TESTING.md の 2.65〜2.69。

- [x] （v0.74.3）`Ctrl+Shift+C` でコピーするかを画面に見えている選択だけで決めている（`main.rs`）。選んだところをスクロールで画面の外に出すと `Ctrl+C` が送られる。サーバーのペインは選択の有無を手元に持たないので、mux の画面の知らせに「選択あり」を足してから（取り決めの版が上がる）。
- （実機 #6）不具合: ARM64 で `Ctrl+C` が走っているプログラム（`sleep 30`）を止めない。`^C` は PTY に届く。2.69 の後半はこれが直るまで `[ ]`（`qa-reports/2026-10-10-arm-2-61.md`）。
- （実機 #6）不具合: notify トリガーが、入力したコマンド行（PSReadLine の再描画）にも一致して 10 秒の間隔を使い切るため、2.62〜2.65 は書かれたとおりだと通知が出ないことが多い（`qa-reports/2026-10-10-arm-2-61.md`）。
- （実機 #6）不具合: ベルの一覧で長いタイトルが時刻と **trigger** の印に重なって読めない（`qa-reports/2026-10-10-arm-2-61.md`）。
- （実機 #6）提案: 2.62〜2.65 の文言を、入力したコマンド行がトリガーの文字列を含まない形（`echo Build" "succeeded`）に変える。キットに `Send-Keys -Hold`・`Send-Wheel`・`Send-Drag`・`Set-WindowSize` を足す。2.67 は `kitten` が要るので kitty を入れるか人に回す。2.62〜2.65 の点滅・音・タスクバーの数・trigger の印は人が確かめる（`qa-reports/2026-10-10-arm-2-61.md`）。
