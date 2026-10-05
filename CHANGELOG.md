# 変更履歴

このファイルの形式は [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) に、版のつけ方は
[セマンティックバージョニング](https://semver.org/lang/ja/) に従う。版はルートの `Cargo.toml` の `[workspace.package]` の `version` が正。

## [未リリース]

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
