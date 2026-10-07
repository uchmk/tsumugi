# TODO

tsumugi のやること。`【人】` は持ち主の作業、`【金】` は 2026-10-09（金）以降に始めるもの
（今週は利用枠の上限に近いため。2026-10-07、持ち主の判断）。

## 金曜日から（2026-10-09 以降）

- [ ] 【人】【金】Windows 実機のテストを始める。`C:\dev\tsumugi` を clone（または pull）し、`pwsh -File C:\dev\tsumugi\scripts\auto-wintest.ps1` を 1 回手で回して
      worktree（`C:\dev\tsumugi-wintest`）を作ってから、スクリプトの冒頭のとおりタスク スケジューラに毎時 :50 で登録する。
      filer の実行（:20）が走っている間は待つ。役割は `.claude/windows-role.md`、チェック表は TESTING-CHECKS.md（197 行）と TESTING-KEYS.md（50 個）。
  - [ ] ARM64 のノート PC も使うなら `-Lane arm`（`C:\dev\tsumugi-armtest`）。
  - [ ] 実機の PR（`test/win-*`）をマージする側を決める（filer の `merge-role.md` に当たるもの。今はまだ無い。最初は対話のセッションでマージする）。
- [ ] 【金】filer の `tsumugi-pane` の `rev` を上げる。filer の `Cargo.toml` の `rev` は `d2405616` のままで、それ以後のペインの直し
      （ペインごとの文字コード `Charset` と `encoding_rs`、`all_text`、`sys` のプロセスとポートの検出、`Terminal::shell_pid()` など）が filer に届いていない。
      上げたら filer で `cargo build` して `Cargo.lock` を合わせ、filer の `scripts/verify.sh` を回す。`Palette` などの形が変わっていれば filer の
      `src/ui/term.rs` を合わせる。filer の TESTING.md の 1・19・29・40 節（ペイン）を filer の実機の再テストに積む。
  - [ ] 同じときに、filer の 2 分割の境目を `tsumugi_layout::ui::dividers`（v0.52.0、`egui` 機能）に置き換えられるかを見る。
      filer の分割は「2 つのタブを横に並べる」形なので、`tsumugi_layout::Node` に載せ替えるかどうかは filer の QUESTIONS.md で聞く。
- [x] 最初のリリースを切る。v0.52.0 を 2026-10-07 に出した（6 つの成果物と SHA-256 の表が揃った。run 37577264990）。

## ソースレビュー（2026-10-07）の残り

v0.53.1〜v0.53.3 で重いものと手早いものは直した。残りは次のとおり（どれも中〜小）。

- [ ] Windows のメニューのコマンドが `"` で始まると cmd が引用符を外して壊れる（`menu.rs` の `cmd /C`）。`/S` の形（全体を `"…"` で囲む）にする。`{folder}` の `%VAR%` も。
- [ ] 「Always allow」のルールに `*` が入るとき、または長いコマンドが画面で折り返されたときは作らない（`permit.rs`）。Claude Code の `*` の扱いを実機で確かめる。【実機】
- [ ] 自前の JSON（`json.rs`）: サロゲートペアの `\u` を 1 文字に、`\b` `\f`、知らないエスケープは誤り、`1e400` を書かない、BOM を読み飛ばす。
- [ ] Shift_JIS などのペインで、その文字コードに無い文字（絵文字）が `&#128512;` で送られる（`charset.rs` の `encode`）。`?` にする。
- [ ] AltGr（ドイツ語配列など）の記号に Ctrl+Alt のキーが余計に付く（`input.rs`）。Ctrl と Alt が両方で印字できるキーは、文字だけ送る。【実機】
- [ ] Windows の `end_tree` が PID の使い回しで止まらない／関係ないプロセスを止めうる。プロセス表を 1 回取って `descendants` で辿る。
- [ ] 使用量（`usage.rs`）: `"type":"assistant"` の行だけ読む（サブエージェントの二重計上の疑い）。7 日を過ぎたファイルを忘れる。`get()` を 1 コマ 1 回に。
- [ ] 費用の知らせ（`spend.rs`）: 「その日」を日付で持つ（値段を下げると同じ日にもう一度知らせる）。
- [ ] 検索の列が、小文字にすると長さの変わる文字（`İ`）でずれる（`grid.rs` の `find_lines`）。
- [ ] `view.rs` の `fit` で `row_h` が 0 のときの守り、`window_size` の `u16` への切り詰め。

