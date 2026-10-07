# 引き継ぎ（2026-10-07 時点）

会話を `/clear` する前に書いた、進み具合と次の手順。細かいやることは [TODO.md](../TODO.md)、変わったことは [CHANGELOG.md](../CHANGELOG.md)。

## 今どこか

- 版は **v0.53.5**（`main`）。最後のリリースは **v0.52.0**（2026-10-07、6 つの成果物と SHA-256 の表。run 37577264990）。
- v1 の範囲（[v1-scope.md](v1-scope.md)）の機能は出そろった。v0.52.2〜v0.53.0 で設定画面・新しいセッションの画面・Tab の順番とボタンの位置を揃え、
  v0.53.1〜v0.53.4 でソースレビュー（2026-10-07）の指摘を全部直した（TODO.md の「ソースレビュー（2026-10-07）の残り」はすべて `[x]`）。
- 実機ではまだ 1 行も確かめていない。TESTING-CHECKS.md と TESTING-KEYS.md は全部 `[ ]`。

## 次の手順

1. **金曜（2026-10-09）から実機のテスト**（TODO.md の「金曜日から」）。持ち主が `scripts/auto-wintest.ps1` を 1 回手で回し、タスク スケジューラに毎時 :50 で登録する。
   役割は `.claude/windows-role.md`。Tab の順番・ボタンの位置などの UI の整合は TESTING.md の 19 節。ソースレビューの直しは 2.16〜2.20、3.29、4.24、7.15、7.16、9.9、12.6、17.31〜17.33。
2. 実機の PR（`test/win-*`）をマージする側を決める（最初は対話のセッションで。filer の `merge-role.md` に当たるものはまだ無い）。
3. **filer の `tsumugi-pane` の `rev` を上げる**（今は `d2405616`）。filer で `cargo build` して `Cargo.lock` を合わせ、filer の `scripts/verify.sh`。
   同じときに filer の 2 分割の境目を `tsumugi_layout::ui::dividers` に置き換えられるかを見る。
4. 実機で直したものが溜まったら次のリリース（`release.yml` を `workflow_dispatch`、`tag` に `vX.Y.Z`）。GitHub の MCP の `actions_run_trigger` から投げられる
   （このコンテナの `gh` はトークンが通らない）。

## 覚えておくこと

- コミットメッセージは heredoc で書く（v0.52.3 は二重引用符の中のバッククォートがシェルで実行され、件名の語が欠けた。直していない）。
- egui 0.36 の落とし穴: `scope_builder` は親のカーソルを進めるが `new_child` は進めない。`consume_key(NONE, Tab)` は Shift+Tab にも当たるので Shift を先に取る。
  begin_pass で egui が Tab と矢印をフォーカス移動として読むので、自分で動かすときは `move_focus(FocusDirection::None)` してから `request_focus`。
- フォーカスの動きは `TSUMUGI_KEYLOG=1` で `focus x,y wxh` が出る。
