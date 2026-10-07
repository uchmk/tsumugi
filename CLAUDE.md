# tsumugi — Claude 向けメモ

Claude Code などの AI CLI のセッションを何本も並べて動かすためのターミナル。Rust + egui、Windows 優先。
姉妹プロジェクトの [filer](https://github.com/uchmk/filer)（yazi 風のファイルマネージャー）のターミナルペインを土台にする。

最初の版の範囲は [docs/v1-scope.md](docs/v1-scope.md)。今は filer のターミナルペインを `crates/tsumugi-pane` に切り出している（[docs/pane-extraction.md](docs/pane-extraction.md)。段階と進み具合もそこ）。

- Cargo の workspace。クレートは `crates/` の下に置く。版はルートの `Cargo.toml` の `[workspace.package]` で 1 つ。
- **push の前に `scripts/verify.sh` を 1 回回す。**最後の行が `ALL OK: …` なら push してよい（test、clippy を Linux と `x86_64-pc-windows-msvc` の両方で `-D warnings`）。

## 作業ルール

- 返答は日本語。コード・コメント・コミットメッセージは英語。
- 頼まれるまでコミットしない。
- 改行は LF。スクリプトで書き換えるときは改行を変えない（Python なら `newline=''`）。
- やることは [TODO.md](TODO.md) に書く（`【人】` は持ち主の作業、`【金】` は 2026-10-09 以降に始めるもの）。
- 人への確認事項は QUESTIONS.md に書く（形式は filer の CLAUDE.md の「確認事項」と同じ。選択肢に推奨を 1 つ付ける）。
- **`cargo fmt` は走らせない。**filer と同じく手で整形する（`Self { a, b, c }` を 1 行に収める書き方）。整形の確認は `cargo fmt --check` で見るだけ。
- clippy は `--all-targets -- -D warnings` で警告ゼロを保つ。検証は CI と同じ stable で回す。

## 実機のテスト（TESTING.md）

- 画面の無い環境では確かめられないもの（Windows の ConPTY・トースト・タスクバー・タイトルバー・Mica・音、見た目）は [TESTING.md](TESTING.md) の表に積む。
  **機能を足したら、そこに行を足してから完了にする。**正は TESTING.md（英語）で、`cargo run -p tsumugi --example make-testcheck` が
  TESTING-CHECKS.md（印を付ける表）を、`make-keycheck` が keys.rs から TESTING-KEYS.md を作る。印（`[x]`・`[~]`）は作り直しても残る。
- **動きを変えた行は、印を外して（`[ ]` に戻して）から作り直す。**前の動きを確かめた印なので、残すと嘘になる。
- `scripts/verify.sh` と CI（`checklists.yml`）が `--check` で、表が TESTING.md と keys.rs に追いついているかを見る。
- `[x]` は実機で確かめた印で、付けてよいのは実機のセッション（Agent）か持ち主だけ。見た目の行は `[~]`（画面の画像で判断した）まで。

## 版と変更ログ（コードが入ったら）

filer と同じ。`Cargo.toml` の `version` が正、版の繰り上げと CHANGELOG.md（日本語、Keep a Changelog、日付は JST）はセットでコミットする。
`main` への直接の push を許す。マージは merge コミットで行う（rebase / squash を使わない）。

## リリース

- `.github/workflows/release.yml` を Actions タブから `workflow_dispatch` で回し、`tag` に `vX.Y.Z` を渡す（タグが無ければ作られる。
  クラウドのセッションからタグは push できない）。`Cargo.toml` の版と違えば落ちる。成果物は 6 つ（Windows は ConPTY を同梱した `.zip`、
  macOS と Linux は `.tar.gz`）、最後に `release-sums.yml` が SHA-256 の表をノートに足す。
- ノートは前のタグからの `vX.Y.Z:` のコミットの最初の段落から作る。英語で、それだけで意味が通るように書く。前のリリースのタグができてから次を投げる。

## Windows 実機のセッション

- 役割は [.claude/windows-role.md](.claude/windows-role.md)。`scripts/auto-wintest.ps1` をタスク スケジューラで毎時 :50 に回すと、
  TESTING.md / TESTING-CHECKS.md / 役割の定義が `main` で変わっていて `test/win-*` の PR が開いていないときに 1 本起動する（filer のものの写し。
  filer の実行が走っている間は待つ）。報告は `qa-reports/<日付>-<ブランチ>.md`。
- `[x]` を付けてよいのはこのセッションと持ち主だけ。版と CHANGELOG は触らず、PR 本文に 1 行書く。マージは merge コミット。
- 始めるのは 2026-10-09（金）から（TODO.md）。

## 設計の約束事

- **マルチ OS**: Windows（ConPTY、Win32、UNC）、macOS（Cmd キー）、Linux（X11 / Wayland）の違いは `#[cfg(...)]` のモジュールに閉じ込め、
  使う側から見えない API にする。
- **マルチアーキテクチャ**: x86_64 と ARM64 の 6 ターゲットへのクロスコンパイルを軽く保つ。Pure Rust のクレートを優先し、C のライブラリを抱えるものは避ける。
- **修飾キーの抽象化**: macOS では Cmd、Windows / Linux では Ctrl に差し替えられる形にする。
- **描画とロジックを分ける**: PTY・キー変換・OSC の解釈・セッションの状態は egui に依らない層に置く。
- **ディスクやプロセスに触る処理は UI スレッドで実行しない**（ワーカースレッドとチャネル、古い依頼は捨てる）。
- **ターミナルペインは filer と共有のクレート**（仮名 `tsumugi-pane`）。ペインの直しはクレートに入れ、両方に効かせる。
  画面の分割も共有のクレート（仮名 `tsumugi-layout`）にし、先に tsumugi で作って実機で揉んでから filer に持ち帰る。
