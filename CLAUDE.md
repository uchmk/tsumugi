# tsumugi — Claude 向けメモ

Claude Code などの AI CLI のセッションを何本も並べて動かすためのターミナル。Rust + egui、Windows 優先。
姉妹プロジェクトの [kura](https://github.com/uchmk/kura)（旧 filer、yazi 風のファイルマネージャー）のターミナルペインを土台にする。

前の会話からの引き継ぎ（今の版と次の手順）は [docs/handoff.md](docs/handoff.md)。最初の版の範囲は [docs/v1-scope.md](docs/v1-scope.md)。共有のクレート（ペイン・分割・言語・設定の画面・テーマなど、`ito-*`）は [ito](https://github.com/uchmk/ito) にある（v0.82.0 で移した）。
ペインを filer から切り出した経緯は [docs/pane-extraction.md](docs/pane-extraction.md)。

- **uchmk のアプリの共通仕様**（common.toml・設定の画面・テーマ・時計、共有のクレート `ito-common`・`ito-theme`・`ito-prefs`）は
  ito の [docs/common-spec.md](https://github.com/uchmk/ito/blob/main/docs/common-spec.md)。kura（旧 filer）と yagura（旧 mimamori）もこれに従う。仕様を変えたら ito の文書も直す。
- Cargo の workspace。クレートは `crates/` の下に置く。版はルートの `Cargo.toml` の `[workspace.package]` で 1 つ。
- **push の前に `scripts/verify.sh` を 1 回回す。**最後の行が `ALL OK: …` なら push してよい（test、clippy を Linux と `x86_64-pc-windows-msvc` の両方で `-D warnings`）。

## 作業ルール

- 返答は日本語。コード・コメント・コミットメッセージは英語。
- 頼まれるまでコミットしない。
- 改行は LF。スクリプトで書き換えるときは改行を変えない（Python なら `newline=''`）。
- やることは [TODO.md](TODO.md) に書く（`【人】` は持ち主の作業、`【金】` は 2026-10-09 以降に始めるもの）。
- 人への確認事項は QUESTIONS.md に書く（形式は kura の CLAUDE.md の「確認事項」と同じ。選択肢に推奨を 1 つ付ける）。
- **`cargo fmt` は走らせない。**kura と同じく手で整形する（`Self { a, b, c }` を 1 行に収める書き方）。整形の確認は `cargo fmt --check` で見るだけ。
- clippy は `--all-targets -- -D warnings` で警告ゼロを保つ。検証は CI と同じ stable で回す。

## 共有のクレート（ito）

- `crates/tsumugi/Cargo.toml` と `crates/tsumugi-mux/Cargo.toml` が `ito-* = { git = "https://github.com/uchmk/ito", rev = "…" }` で読む。
  **全部のクレートを同じ `rev` にそろえる。**
- ペインなどの直しは ito に入れて push し、ここの `rev` を上げる（`cargo build` で `Cargo.lock` も合わせる）。ito の CHANGELOG に使う側がやることが書いてある。
- 2 つを手元で一緒に直すときは、`rev` を書き換えずにローカルの ito を差し込む（クレートごとに 1 つ `--config`）:

  ```sh
  cargo build --config 'patch."https://github.com/uchmk/ito".ito-pane.path="../ito/crates/ito-pane"'
  ```

  確かめ終えたら ito を push し、ここの `rev` を上げてから push する。
- ito は `cargo fmt` で整形する（ここと違う）。ito のファイルを直すときは ito の CLAUDE.md に従う。

## 実機のテスト（TESTING.md）

- 画面の無い環境では確かめられないもの（Windows の ConPTY・トースト・タスクバー・タイトルバー・Mica・音、見た目）は [TESTING.md](TESTING.md) の表に積む。
  **機能を足したら、そこに行を足してから完了にする。**正は TESTING.md（英語）で、`cargo run -p tsumugi --example make-testcheck` が
  TESTING-CHECKS.md（印を付ける表）を、`make-keycheck` が keys.rs から TESTING-KEYS.md を作る。印（`[x]`・`[~]`）は作り直しても残る。
- **動きを変えた行は、印を外して（`[ ]` に戻して）から作り直す。**前の動きを確かめた印なので、残すと嘘になる。
- `scripts/verify.sh` と CI（`checklists.yml`）が `--check` で、表が TESTING.md と keys.rs に追いついているかを見る。
- `[x]` は実機で確かめた印で、付けてよいのは実機のセッション（Agent）か持ち主だけ。見た目の行は `[~]`（画面の画像で判断した）まで。
- **文字で確かめられる行（CLI・サーバの出力）は CI に回す。**`crates/tsumugi/tests/cli.rs` にテストを書き、行は TESTING.md の
  「Covered by tests」の表（元の番号・内容・テスト名）に移す。実機の Claude に残すのは窓・キー・トーストなど画面の要る行だけ。

## 版と変更ログ（コードが入ったら）

kura と同じ。`Cargo.toml` の `version` が正、版の繰り上げと CHANGELOG.md（日本語、Keep a Changelog、日付は JST）はセットでコミットする。
`main` への直接の push を許す。マージは merge コミットで行う（rebase / squash を使わない）。

## リリース

- `.github/workflows/release.yml` を Actions タブから `workflow_dispatch` で回し、`tag` に `vX.Y.Z` を渡す（タグが無ければ作られる。
  クラウドのセッションからタグは push できない）。`Cargo.toml` の版と違えば落ちる。成果物は 6 つ（Windows は ConPTY を同梱した `.zip`、
  macOS と Linux は `.tar.gz`）、最後に `release-sums.yml` が SHA-256 の表をノートに足す。
- ノートは前のタグからの `vX.Y.Z:` のコミットの最初の段落から作る。英語で、それだけで意味が通るように書く。前のリリースのタグができてから次を投げる。

## Windows 実機のセッション

- 役割は [.claude/windows-role.md](.claude/windows-role.md)。`scripts/auto-wintest.ps1` をタスク スケジューラで毎時 :50 に回す
  （x64 と ARM64 の `-Lane arm`）。スクリプトが先に `cargo build` / `cargo test` / ConPTY の取得を済ませ、TESTING-CHECKS.md と TESTING-KEYS.md から
  1 回分の塊（再テスト → キー（x64 だけ）→ 節の順、最大 15 行）を選び、その行だけをプロンプトに入れて Claude（既定は Sonnet 5.5）を起動する。
  塊が無い・自分の PR が開いている・ビルドが落ちたときは Claude を起動しない。道具は `scripts/wintest-kit.ps1`、塊の選び方は `scripts/wintest-queue.ps1`
  （`scripts/check-ps1.ps1` が CI で確かめる）。報告は `qa-reports/<日付>-<ブランチ>.md`。
- 同じ机で kura のレーンとキーがぶつからないよう、両方のスクリプトが `Local\wintest-desktop` のロックを取る（最大 20 分待って次回へ）。
- 実機の PR（`test/win-*`・`test/arm-*`）は、ワークフロー `Merge lanes`（`.github/workflows/merge-lanes.yml`、中身は kura と同じ `scripts/merge-lanes.py`）が
  規則（触ってよいファイル・印の変わり方・印ごとの証拠の行）と `check` の緑を確かめて、head を固定した merge コミットでマージする（v0.76.4 から）。
  チェック表だけのぶつかりは main の表に PR の印を入れ直して自分でマージし、マージ済みの PR の分け前（PATCH・Cargo.lock・CHANGELOG・再テストの表・
  報告の Proposals と Queue を TODO.md へ）も main に push して CI を起こす（v0.76.6 から）。
  クラウドのマージの Routine（手順は [.claude/merge-routine.md](.claude/merge-routine.md)）はマージも分け前もせず、ワークフローが止めた PR
  （規則・赤・チェック表以外のぶつかり）の QUESTIONS.md への質問と解決だけをする。レーンは前の PR の `#N` が CHANGELOG に入るまで次を始めない。
- `[x]` を付けてよいのはこのセッションと持ち主だけ。版と CHANGELOG は触らず、PR 本文に 1 行書く。

## 設計の約束事

- **マルチ OS**: Windows（ConPTY、Win32、UNC）、macOS（Cmd キー）、Linux（X11 / Wayland）の違いは `#[cfg(...)]` のモジュールに閉じ込め、
  使う側から見えない API にする。
- **マルチアーキテクチャ**: x86_64 と ARM64 の 6 ターゲットへのクロスコンパイルを軽く保つ。Pure Rust のクレートを優先し、C のライブラリを抱えるものは避ける。
- **修飾キーの抽象化**: macOS では Cmd、Windows / Linux では Ctrl に差し替えられる形にする。
- **描画とロジックを分ける**: PTY・キー変換・OSC の解釈・セッションの状態は egui に依らない層に置く。
- **ディスクやプロセスに触る処理は UI スレッドで実行しない**（ワーカースレッドとチャネル、古い依頼は捨てる）。
- **ターミナルペイン（`ito-pane`）と画面の分割（`ito-layout`）は kura と共有**。直しは ito に入れ、両方に効かせる。
  分割は先に tsumugi で作って実機で揉んでから kura に持ち帰る。
