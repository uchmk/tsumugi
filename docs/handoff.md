# 引き継ぎ（2026-10-09 時点）

会話を `/clear` する前に書いた、進み具合と次の手順。細かいやることは [TODO.md](../TODO.md)、変わったことは [CHANGELOG.md](../CHANGELOG.md)。

## 今どこか

- 版は **v0.75.2**（`main`）。最後のリリースは **v0.52.0**（2026-10-07）のままで、それ以後の v0.53〜v0.75 はまだ出していない。
- v1 の範囲（[v1-scope.md](v1-scope.md)）の機能は出そろい、TODO.md の「ほかのターミナルにあるもの」その 1・その 2 も「後で」の 3 行を除いて済み
  （v0.69.0〜v0.74.3: 下線と取り消し線、DECSET 1004、DA・XTVERSION、kitty のキーボードの取り決め、quick-select、選んだらコピー・矩形選択・PRIMARY、トリガー）。
- **LLM との連携**（filer の QUESTIONS.md Q95・Q96、持ち主の回答はどちらも 1、計画は filer の `docs/llm-integration.md`）:
  - v0.75.0 で `tsumugi mcp`（道具 `tsumugi_sessions` / `tsumugi_screen`、読むだけ）と共有のクレート `tsumugi-ipc`（口: Unix のソケット /
    Windows の名前付きパイプと枠）・`tsumugi-mcp`（JSON-RPC を `serde_json` だけで手書き、`tokio` 無し）を入れた。サーバーの口は `tsumugi-mux` から `tsumugi-ipc` に移しただけで動きは同じ。
  - filer v0.85.0 が同じクレートで `filer mcp`（`filer_state` / `filer_reveal`）を足した。filer は `tsumugi-ipc` と `tsumugi-mcp` を `rev = 8139010…`（v0.75.0）で読む。
  - 文字で確かめられる半分は `tests/cli.rs` の `mcp_lists_sessions_and_reads_a_screen`。実機の行は TESTING.md 18.10（filer は 50 節）。
- **Windows 実機のテストは 2026-10-09 に始まった。** TESTING-CHECKS.md は 304 行中 7 行、TESTING-KEYS.md は 77 個中 19 個が `[x]`。
  実機の PR（`test/win-*`・`test/arm-*`）はクラウドのマージの Routine（毎時 :40、[.claude/merge-routine.md](../.claude/merge-routine.md)）がマージし、PATCH と CHANGELOG を上げる。

## 次の手順

1. **実機が見つけた不具合を直す**（TODO.md の「実機のレーンから」）。一番大きいのは、仕切りをドラッグして離すと元の比率に戻るもの
   （`crates/tsumugi/src/main.rs` の ~:4190 が離したフレームで `self.dragging` を消し、~:4521 の `Moved::Released` で `set_layout` が呼ばれない）。
   直したら 2.3・2.15 を再テストに積む。キットの提案（`Send-Drag`、生の仮想キー、`--check` の CRLF）もそこにある。
2. **filer の `tsumugi-pane` / `tsumugi-layout` の `rev` を上げる。**filer はまだ `fc88385`（v0.65.1）で、v0.69.0〜v0.74.3 のペインの直し
   （`crates/tsumugi-pane` に 13 ファイル・約 1,400 行）が filer に届いていない。上げたら filer で `cargo build` して `Cargo.lock` を合わせ、
   `src/terminal.rs`・`src/ui/term.rs` を新しい API に合わせ、filer の `scripts/verify.sh`。filer の TESTING.md のペインの節（1・19・29・40・49）を再テストに積み、
   新しい機能の行を足す。`tsumugi-ipc` / `tsumugi-mcp` の `rev` も同じ値に揃える。
3. **次のリリース。**v0.52.0 から間が空いた。実機の直しが一段落したら `release.yml` を `workflow_dispatch`（`tag` に `vX.Y.Z`）。
   GitHub の MCP の `actions_run_trigger` から投げられる（このコンテナの `gh` はトークンが通らない）。
4. LLM の段 4（実機で Claude Code に登録して呼ぶ: TESTING.md 18.10、filer は 50 節）は実機のレーン待ち。段 5（書く道具。窓の確認の箱つき）は使ってみてから範囲を聞く。
5. 【人】ARM64 のレーン（`-Lane arm`）と、最初の 3 回の実行時間の目安（TODO.md の「金曜日から」）。

## 覚えておくこと

- コミットメッセージは heredoc で書く（v0.52.3 は二重引用符の中のバッククォートがシェルで実行され、件名の語が欠けた。直していない）。
- egui 0.36 の落とし穴: `scope_builder` は親のカーソルを進めるが `new_child` は進めない。`consume_key(NONE, Tab)` は Shift+Tab にも当たるので Shift を先に取る。
  begin_pass で egui が Tab と矢印をフォーカス移動として読むので、自分で動かすときは `move_focus(FocusDirection::None)` してから `request_focus`。
- フォーカスの動きは `TSUMUGI_KEYLOG=1` で `focus x,y wxh` が出る。
- クラウドのコンテナの書ける容量は小さい。別の作業ツリーで `cargo build` すると `target` がもう 1 つでき、リンクで `No space left on device` になる。
  `CARGO_TARGET_DIR` を元の `target` に向ける。
- filer の `scripts/push-main.sh` は、`main` が進んでいて自分のコミットが `Cargo.toml` の版以外も変えていると止まる（「replay it by hand」）。
  自動モードでは `git rebase` が拒まれるので、`origin/main` から別の作業ツリーを作って cherry-pick し、版・CHANGELOG・`Cargo.lock` の衝突を解いて、そこから `push-main.sh` を回す。
