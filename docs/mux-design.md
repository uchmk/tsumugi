# サーバーとクライアント（`tsumugi-mux`）— 設計と進み具合

v1-scope.md の「順番」の 3（v0.2.0）。窓を閉じても、GUI が落ちても、シェルと Claude Code が動き続け、次に開いた窓がつなぎ直す。
後から分けると作り直しになるので、タブより先にやる。2026-10-06 に書いた。

## 形

```
tsumugi（GUI、クライアント）──┐
tsumugi ls / attach / new ────┼── 名前付きパイプ / ドメインソケット ──  tsumugi server（セッションを持つ）
tsumugi notify（フックから）──┘                                         └─ Terminal × N（ito-pane、PTY と格子）
```

- **1 つの exe に全部入れる。**`tsumugi` は GUI、`tsumugi server` がサーバー、`tsumugi ls` などが CLI。配る物が 1 つで済み、版のずれも起きない。
- **サーバーは GUI が要るときに自分で起こす。**つながらなければ `tsumugi server` を親から切り離して起動し、つながるまで待つ。
- **サーバーは最後のセッションが終わったら止まる**（v1-scope.md の「決めたこと」）。窓が 1 つもなくても、セッションがあれば動き続ける。
- **格子はサーバーが持つ。**PTY の出力の解釈（alacritty の `Term`）、選択、検索、スクロールバックはサーバーの側。クライアントは見えている画面の写し（セルの列）を受けて描くだけ。
  クライアントで同じ解釈をやり直す形（生のバイトを流す）より、つなぎ直したときに画面がそのまま戻り、クライアントが軽い。WezTerm の mux も画面の行を送る形。
- **描画の部品（`show`）とキー入力（`input::feed`）は、手元の `Terminal` とサーバーの向こうのペインの両方に使う。**`ito-pane`（v0.82.0 までは `tsumugi-pane`）に trait `Pane` を置き、
  `Terminal` と、クライアントの `RemotePane` がそれを実装する。filer は今のまま `Terminal` を使う。

## 通信

- **Unix はドメインソケット**（`std::os::unix::net`）。場所は `$XDG_RUNTIME_DIR/tsumugi/sock`、無ければ `/tmp/tsumugi-<uid>/sock`（フォルダは 0700）。
- **Windows は名前付きパイプ** `\\.\pipe\tsumugi-<ユーザー名>`。`windows` クレート（filer も ito-pane も使っている）で `CreateNamedPipeW` と `CreateFileW` を呼び、
  あとは `std::fs::File` として読み書きする。ほかのユーザーからつながれないよう、既定のセキュリティ（作ったユーザーと管理者だけ）のままにし、`PIPE_REJECT_REMOTE_CLIENTS` を付ける。
- **形式は「長さ（u32、リトルエンディアン）＋ postcard」**の枠。型は serde で書く。先頭で版を言い合い、違えばクライアントがそう言って止まる。
  新しい依存は `serde` と `postcard`（どちらも Pure Rust）。最初は bincode にしたが、bincode は 2025 年に保守が止まっていた（v0.4.1 で替えた）。QUESTIONS.md の Q4。
  200 × 50 の画面 1 枚が約 79 KB（1 セル 8 バイト）、書くのも読むのも約 0.25 ms（release、クラウドの機械）。重くなるのは形式より「毎回画面を丸ごと送る」こと。
  v0.4.3 で、変わった行だけを送る形にした（`diff.rs`。1 文字打つと約 1.6 KB）。
- **1 つのつなぎに 2 本のスレッド**（読む・書く）。サーバーはクライアントごとに。画面の写しは、そのセッションが変わったときに送る（毎秒 60 回まで）。

### 送るもの（最初の版）

| 向き | 中身 |
| --- | --- |
| C → S | `Hello { version }`、`List`、`Spawn { cwd, shell, size, cell }`、`Attach { id }`、`Detach { id }`、`Input { id, bytes }`、`Paste { id, text }`、`Resize { id, size, cell }`、`Scroll { id, by }`、`Select { id, … }`、`Kill { id }` |
| S → C | `Hello { version }`、`Sessions(Vec<Info>)`、`Screen { id, rows, cursor, modes, title, scrolled_back, … }`、`Exited { id }`、`Clipboard(String)`、`Error(String)` |

`Info` はセッションの番号、作業フォルダ、タイトル、起動したコマンド、動いているか。状態の印（v0.4.0）はここに足していく。

## 段

| 段 | 中身 | 状態 |
| --- | --- | --- |
| a | `tsumugi-pane` に trait `Pane`。`show` と `input::feed` をそれに合わせ、`Terminal` が実装する（filer はそのまま） | 済み（v0.0.9） |
| b | `crates/tsumugi-mux`: 送る型、枠、通信（Unix / Windows）、サーバー、クライアント（`RemotePane`）。テストはプロセス内でサーバーとクライアントをつなぐ | 済み（v0.0.10） |
| c | `tsumugi` をクライアントにする。`tsumugi server`、`tsumugi ls`、サーバーが無ければ起こす。窓を閉じてもシェルが残り、開き直すと同じ画面に戻ることを Xvfb で確かめる | 済み（v0.1.0） |

## 決めていないこと

- `tsumugi attach <名前>` の名前（今は番号。タブの名前が入ったら名前でも）。
- 1 つのセッションを 2 つの窓で同時に見たときの大きさ（tmux は小さいほうに合わせる。最初の版は、最後に大きさを言った窓に合わせる）。

## 進み具合

（段が終わるたびに 1 行足す）

- 2026-10-06 v0.0.10: 段 b。サーバーとクライアントのテストが Linux で通る（Windows は CI で名前付きパイプを通る）。
- 2026-10-06 v0.1.0: 段 c。Xvfb で、窓で `echo first-window` → 窓を閉じる → `tsumugi ls` に残る → 2 つ目の窓が同じ画面で開く → `exit` で窓が閉じ、サーバーも止まる、を確かめた。
  最初の版では、サーバーが止まる直前の `Exited` が届かず窓が「サーバーが去った」と言っていたので、サーバーが 0.3 秒待ってから終わるようにした。
  実機（Windows）ではまだ動かしていない。
