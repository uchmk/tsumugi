# tsumugi — Claude 向けメモ

Claude Code などの AI CLI のセッションを何本も並べて動かすためのターミナル。Rust + egui、Windows 優先。
姉妹プロジェクトの [filer](https://github.com/uchmk/filer)（yazi 風のファイルマネージャー）のターミナルペインを土台にする。

**今は計画の段階で、コードはまだ無い。**最初の版の範囲は [docs/v1-scope.md](docs/v1-scope.md)。

## 作業ルール

- 返答は日本語。コード・コメント・コミットメッセージは英語。
- 頼まれるまでコミットしない。
- 改行は LF。スクリプトで書き換えるときは改行を変えない（Python なら `newline=''`）。
- 人への確認事項は QUESTIONS.md に書く（形式は filer の CLAUDE.md の「確認事項」と同じ。選択肢に推奨を 1 つ付ける）。
- **`cargo fmt` は走らせない。**filer と同じく手で整形する（`Self { a, b, c }` を 1 行に収める書き方）。整形の確認は `cargo fmt --check` で見るだけ。
- clippy は `--all-targets -- -D warnings` で警告ゼロを保つ。検証は CI と同じ stable で回す。

## 版と変更ログ（コードが入ったら）

filer と同じ。`Cargo.toml` の `version` が正、版の繰り上げと CHANGELOG.md（日本語、Keep a Changelog、日付は JST）はセットでコミットする。
`main` への直接の push を許す。マージは merge コミットで行う（rebase / squash を使わない）。

## 設計の約束事

- **マルチ OS**: Windows（ConPTY、Win32、UNC）、macOS（Cmd キー）、Linux（X11 / Wayland）の違いは `#[cfg(...)]` のモジュールに閉じ込め、
  使う側から見えない API にする。
- **マルチアーキテクチャ**: x86_64 と ARM64 の 6 ターゲットへのクロスコンパイルを軽く保つ。Pure Rust のクレートを優先し、C のライブラリを抱えるものは避ける。
- **修飾キーの抽象化**: macOS では Cmd、Windows / Linux では Ctrl に差し替えられる形にする。
- **描画とロジックを分ける**: PTY・キー変換・OSC の解釈・セッションの状態は egui に依らない層に置く。
- **ディスクやプロセスに触る処理は UI スレッドで実行しない**（ワーカースレッドとチャネル、古い依頼は捨てる）。
- **ターミナルペインは filer と共有のクレート**（仮名 `tsumugi-pane`）。ペインの直しはクレートに入れ、両方に効かせる。
  画面の分割も共有のクレート（仮名 `tsumugi-layout`）にし、先に tsumugi で作って実機で揉んでから filer に持ち帰る。
