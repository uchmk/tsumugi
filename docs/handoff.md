# 引き継ぎ（2026-10-10 時点）

会話を `/clear` する前に書いた、進み具合と次の手順。細かいやることは [TODO.md](../TODO.md)、変わったことは [CHANGELOG.md](../CHANGELOG.md)。

## 今どこか

- **v0.82.0（2026-10-11）で、共有のクレートを [ito](https://github.com/uchmk/ito) に移した**（`tsumugi-pane` → `ito-pane` など 10 個）。
  filer は kura（v0.95.0）、mimamori は yagura（v0.33.0）に名前が変わり、どちらも ito に切り替えた。下の項目は移す前のもの。
- v0.83.0 で実機 #7・#9・#10 の提案を片付けた（`tsumugi new` の語の引用、`tsumugi log`、キーのログの `event`・`divider` の行）。
  残りは TODO.md の「実機のレーンから」（複数行のプロンプト、道具の補助、OSC 1337 の遅さ、古いサーバーの版）。
- 版は **v0.76.1**（`main`）。最後に出たリリースは **v0.52.0**（2026-10-07）。v0.76.0 の `release.yml` は Windows の Test で落ちた
  （下の CI の赤）。v0.76.1 で直したので、CI が緑になったら `tag` に `v0.76.1` を渡して投げ直す。
- **Windows の CI は v0.55.0（run #97）から赤だった。**`tsumugi-mux` の `a_restart_brings_the_tabs_back` と `folder_rules_tag_sessions` の 2 件だけ。
  ランナーの `TEMP` が 8.3 の短い名前（`C:\Users\RUNNER~1`）で、シェルが報告するフォルダは長い名前（`runneradmin`）なので、タグのルールのフォルダが合わなかった。
  v0.76.1 で `TagRule::tag_for` が両方を `GetLongPathNameW` で長い名前にしてから比べる（利用者の `TEMP` も同じ形になりうるので、テストだけの話ではない）。
  もう 1 件は `claude --resume …` の行がプロンプトの長さで折り返していただけ（テストを直した）。
- v1 の範囲（[v1-scope.md](v1-scope.md)）の機能は出そろい、TODO.md の「ほかのターミナルにあるもの」も「後で」の 3 行を除いて済み。
- **実機のレーンは x64（2026-10-09）と ARM64（2026-10-10）の両方が回っている。**v0.76.0 で最初の実行の不具合を直した
  （仕切りのドラッグが戻る、ConPTY で Ctrl+C が止めない、Shift+Enter の CSI-u、CSI 16 t、pwsh / Starship のプロンプトの印、MSYS のパス、Ctrl+Shift+- など）。
  直した行は `.claude/windows-role.md` の「Re-tests of changed behaviour」に積んである。残りは TODO.md の `【実機】` 2 行（2.52 の PTY ログ、JIS の `Ctrl+=`）。
- **filer は v0.86.0 で `tsumugi-pane` / `tsumugi-layout` / `tsumugi-ipc` / `tsumugi-mcp` を全部 `085c421`（v0.76.0）に揃えた。**
  filer 側は `chords_back`（Ctrl+Shift+C / X）・素の `\x03`・Shift+Enter の CSI-u を合わせ、ペインの節（1・19・29・40・49）を両方のレーンの再テストに積んだ。
- **LLM との連携**（計画は filer の `docs/llm-integration.md`）: 段 1〜4 は済み（filer の 50 節は x64 #303・ARM64 #302 で通った）。
  tsumugi の TESTING.md 18.10（`tsumugi mcp` を Claude Code に登録して呼ぶ）はまだ `[ ]` で、x64 のレーンの節の順（1 → 18）で回ってくる。
  段 5（書く道具。窓の確認の箱つき）は filer の QUESTIONS.md **Q97** で持ち主に聞いている（推奨は 1: filer のファイル操作とセッションへの入力、1 回ごとの確認の箱）。

## 次の手順

1. v0.76.1 の CI（`ci.yml`）が Windows でも緑か見る。緑なら `release.yml` を `tag=v0.76.1` で投げる（GitHub の MCP の `actions_run_trigger`）。
   まだ赤なら、落ちたテストの画面の出力から直す。
2. 実機のレーンの報告（`test/win-*`・`test/arm-*` の PR、マージの Routine が入れる）が出す所見を TODO.md から直す。
3. Q97 の回答が来たら段 5 を作る（`tsumugi-mcp` に「確認の箱が要る道具」の印を足し、窓の側で箱を出す）。
4. 18.10 が通ったら TODO.md の LLM の段 4 を tsumugi 側でも閉じる。

## 覚えておくこと

- コミットメッセージは heredoc で書く（v0.52.3 は二重引用符の中のバッククォートがシェルで実行され、件名の語が欠けた。直していない）。
- egui 0.36 の落とし穴: `scope_builder` は親のカーソルを進めるが `new_child` は進めない。`consume_key(NONE, Tab)` は Shift+Tab にも当たるので Shift を先に取る。
  begin_pass で egui が Tab と矢印をフォーカス移動として読むので、自分で動かすときは `move_focus(FocusDirection::None)` してから `request_focus`。
- フォーカスの動きは `TSUMUGI_KEYLOG=1` で `focus x,y wxh` が出る。
- クラウドのコンテナの書ける容量は小さい。別の作業ツリーで `cargo build` すると `target` がもう 1 つでき、リンクで `No space left on device` になる。
  `CARGO_TARGET_DIR` を元の `target` に向ける。
- filer の `scripts/push-main.sh` は、`main` が進んでいて自分のコミットが `Cargo.toml` の版以外も変えていると止まる（「replay it by hand」）。
  自動モードでは `git rebase` が拒まれるので、`origin/main` から別の作業ツリーを作って cherry-pick し、版・CHANGELOG・`Cargo.lock` の衝突を解いて、そこから `push-main.sh` を回す。
