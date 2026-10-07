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
