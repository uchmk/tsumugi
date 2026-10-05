# 変更履歴

このファイルの形式は [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) に、版のつけ方は
[セマンティックバージョニング](https://semver.org/lang/ja/) に従う。版はルートの `Cargo.toml` の `[workspace.package]` の `version` が正。

## [未リリース]

## [0.0.10] - 2026-10-06

### 追加

- クレート `tsumugi-mux`（v0.2.0 の段 b）。セッションを持つサーバー（`server::start`）と、つなぐ側（`Client`、サーバーの向こうのセッションを `Pane` として扱う `RemotePane`）。
  通信は Unix がドメインソケット（`$XDG_RUNTIME_DIR/tsumugi/sock`、フォルダは 0700）、Windows が名前付きパイプ（`\\.\pipe\tsumugi-<ユーザー名>`、
  持ち主と SYSTEM だけ、ほかの機械からは断る、overlapped I/O で読みと書きを並べる）。形式は長さ＋ bincode（Q4 の推奨。新しい依存は serde と bincode）。
  サーバーは画面が変わったセッションの写しを、見ているクライアントに送る（8 ms ごとにまとめる）。最後のセッションが終わると止まる。
  テストは、クライアントが去ってもセッションが残り、次のクライアントが同じ画面に戻ること、同じ所に 2 つ目のサーバーを立てられないこと。
- `tsumugi-pane` の feature `serde`（`Screen`、`CellView`、`MouseReport`、`Size`）。

## [0.0.9] - 2026-10-06

### 追加

- サーバーとクライアント（v0.2.0）の設計 `docs/mux-design.md` と、通信に使うものの質問（QUESTIONS.md の Q4。回答が無ければ推奨で進める）。
- `tsumugi-pane` に trait `Pane` と `Screen`（見えている画面の写し）。`show` と `input::feed` は `Pane` を相手にするようになり、
  手元の `Terminal` と、サーバーの向こうのセッションの両方を同じ部品で描ける。`Terminal` が `Pane` を実装するので、filer の呼び出しはそのまま。

## [0.0.8] - 2026-10-06

### 追加

- Windows（x64 と ARM64）のビルドを Actions の成果物として残す `build.yml`。`tsumugi.exe` の横に新しい ConPTY（`conpty.dll` と `OpenConsole.exe`）を置く。
  取得は filer と同じ `scripts/fetch-conpty.ps1`（版とハッシュも同じ。変えるときは 2 つのリポジトリで揃える）。

## [0.0.7] - 2026-10-06

### 追加

- 窓を出す本体 `crates/tsumugi`（`cargo run -p tsumugi`）。今いるフォルダで既定のシェル（Windows は pwsh があれば pwsh）を 1 つ開き、窓いっぱいに描く。
  色は filer と同じ。シェルが付けたタイトルを窓のタイトルにし、シェルが終われば窓を閉じる。`TSUMUGI_PTY_LOG` で PTY のログを取れる。
  日本語のために OS のゴシック体を等幅の後ろに足す（Windows は BIZ UDゴシック → MS ゴシック → 游ゴシック → メイリオ、Linux は Noto CJK か IPA ゴシック）。
  GPU は Windows では GL を先に使う（filer と同じ理由）。
- `tsumugi-pane` の `input`（feature `egui`）: egui のキー入力をシェルへのバイトに変える部分を filer の `on_key_event` から写した（win32-input-mode、
  `Ctrl` の制御文字、`Alt` の文字、`Ctrl+C` / `Ctrl+X` が egui のコピー・切り取りに化けるのを戻す）。アプリが自分で取るキーは `claim` で先に抜ける。

## [0.0.6] - 2026-10-06

### 変更

- `docs/pane-extraction.md` の filer の版を直した（v0.0.5 を使い始めたのは filer v0.78.130）。

## [0.0.5] - 2026-10-05

### 修正

- Windows で、起動に失敗したシェルが疑似コンソール（`OpenConsole.exe`、Windows 標準の ConPTY なら `conhost.exe`）を 1 つずつ残し、アプリを閉じるまで消えなかった
  （filer の TODO、x64 の実機の所見 3）。`alacritty_terminal` が疑似コンソールを作ったあとシェルの起動に失敗すると、閉じずに `Err` を返すため。
  PTY を作る前の自分の子のコンソールを覚えておき、失敗したら増えた分を終わらせる。PTY を作るのはプロセスの中で 1 本ずつにした（並んだ spawn の
  コンソールを取り違えないため）。

### 追加

- PTY のログで、win32-input-mode のキーの記録に名前を添える（filer #243 の提案 2）: `\e[66;48;98;1;2;1_  (Alt+b)`。離したキーは `up` を付ける。

## [0.0.4] - 2026-10-05

### 変更

- filer（v0.78.125）がこのクレートを使うようになった（切り出しの段 3）。`docs/pane-extraction.md` に経緯と「これから」を書き、
  filer の `【pane】` の項目を誰が進めるかを QUESTIONS.md の Q3 に出した。

## [0.0.3] - 2026-10-05

### 追加

- 切り出しの段 2。`tsumugi-pane` の feature `egui` に、filer の `src/ui/term.rs` を移した描画の部品 `show` を足した。filer の `App` と `Theme` には頼らず、
  色は `Palette`、フォーカスとホイールは `ViewOptions` で受け、アプリにしかできないこと（キーを渡す、クリップボードに書く、右クリックで貼る）は
  `Shown` で返す。ホイールの行の計算 `wheel_whole` も filer の `ui/mod.rs` から移した（テストも）。
- feature `wgpu` に `gpu`（Windows では GL を先に使う `auto_backends`、`pick_backends`、`has_adapter`。filer の `main.rs` から）。
  名指したバックエンドが無いときの警告の文言はアプリが出す（`pick_backends` は `Err` で `auto` の選択を返す）。
- `restrict_dll_search`（Windows で DLL を exe の隣と System32 からだけ読む。filer の #184）。Windows 以外では何もしない。

## [0.0.2] - 2026-10-05

### 追加

- `tsumugi-pane` に filer の `src/terminal.rs`（v0.78.121）の中身を移した（切り出しの段 1）。egui に依らない層で、モジュールは
  `keys`（キーの変換、win32-input-mode）、`osc`（OSC 7 / 133、win32-input-mode の要求）、`shell`（シェルの選び方と引用）、`log`（PTY のログ）、
  `terminal`（`Terminal` と PTY の読み取り）、`grid`（格子の読み出し、選択、検索）、`sys`（OS ごとの処理: `sys/windows.rs` と `sys/unix.rs`）。
  名前はすべてルートから出すので、filer の `crate::terminal::…` の呼び出しはそのまま `tsumugi_pane::…` になる。
- filer のペインのテスト 44 件を一緒に移した（Linux では 43 件が走り、1 件は Windows だけ）。

### 変更（filer から見て）

- `Terminal::spawn` が PTY のログのファイルを引数で受ける（filer は `FILER_PTY_LOG` を読んで渡す）。クレートは環境変数を読まない。
- テスト用の `testing::term` / `testing::feed` は `#[cfg(test)]` を外した（filer のスクロールのテストが使うため）。

## [0.0.1] - 2026-10-05

### 追加

- Cargo の workspace と、filer のターミナルペインを移すクレート `crates/tsumugi-pane`（まだ空）。計画は `docs/pane-extraction.md`。
- `scripts/verify.sh`（test、clippy を Linux と Windows の 2 ターゲットで `-D warnings`）と CI（Windows と Linux でテスト、Linux で clippy）。
