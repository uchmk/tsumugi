# 変更履歴

このファイルの形式は [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) に、版のつけ方は
[セマンティックバージョニング](https://semver.org/lang/ja/) に従う。版はルートの `Cargo.toml` の `[workspace.package]` の `version` が正。

## [未リリース]

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
