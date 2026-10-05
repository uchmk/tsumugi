# 変更履歴

このファイルの形式は [Keep a Changelog](https://keepachangelog.com/ja/1.1.0/) に、版のつけ方は
[セマンティックバージョニング](https://semver.org/lang/ja/) に従う。版はルートの `Cargo.toml` の `[workspace.package]` の `version` が正。

## [未リリース]

## [0.0.1] - 2026-10-05

### 追加

- Cargo の workspace と、filer のターミナルペインを移すクレート `crates/tsumugi-pane`（まだ空）。計画は `docs/pane-extraction.md`。
- `scripts/verify.sh`（test、clippy を Linux と Windows の 2 ターゲットで `-D warnings`）と CI（Windows と Linux でテスト、Linux で clippy）。
