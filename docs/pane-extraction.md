# ターミナルペインの切り出し（`tsumugi-pane`）— 計画と進み具合

2026-10-05 に着手（持ち主の判断: filer の TODO が片付くのを待たずに始める）。v1-scope.md の「順番」の 1。

## 目的

filer のターミナルペイン（`src/terminal.rs` 約 2,400 行、`src/ui/term.rs` 約 430 行、`src/shellhook.rs`、`main.rs` の
`wgpu_options` / `pick_backends` / `restrict_dll_search`）を、このリポジトリの Cargo workspace のクレート `tsumugi-pane` に移す。
filer は git の依存（`rev` で固定）で読み、tsumugi の窓（v0.1.0）は同じクレートを使う。

## 約束

- **OS ごとの処理は `sys/` の `#[cfg]` のモジュールに閉じ込める**（`sys/windows.rs`、`sys/unix.rs`）。使う側から OS の違いが見えない API にする。
- **egui に依らない層と描画の層を分ける。**既定の機能は egui を引かない。描画は `egui` の feature、GPU の選び方は `wgpu` の feature（eframe を引く）。
- **Pure Rust。**新しい依存は filer がすでに持っているもの（`alacritty_terminal`、`crossbeam-channel`、`windows`、`egui` / `eframe`）だけ。
- **filer のテストを一緒に持っていく。**ペインのテストは全部クレートの中で走る。filer の `crate::util` に頼っていた所は、クレートの小さな `util` に写す。
- **filer の名前をクレートに残さない。**`FILER_PTY_LOG` のような環境変数の名前、シェルの設定の文言は使う側が渡す。
- 検証: `scripts/verify.sh`（test、clippy を Linux と `x86_64-pc-windows-msvc` の両方で `-D warnings`、`--locked` のビルド）。`cargo fmt` は走らせない。

## 段階

| 段 | 中身 | 終わりの条件 | 状態 |
| --- | --- | --- | --- |
| 0 | workspace の骨組み、`crates/tsumugi-pane`（空）、`scripts/verify.sh`、CI | verify が緑 | 済み（v0.0.1） |
| 1 | egui に依らない層を移す: キーの変換（`keys`）、OSC の読み取り（`osc`）、シェルの選び方と引用（`shell`）、PTY のログ（`log`）、`Terminal` と格子の読み出し（`terminal`）、OS の処理（`sys`） | filer の `terminal.rs` のテストが全部クレートで通る | 済み（v0.0.2） |
| 2 | 描画の層: `ui/term.rs` を filer の `App` と `Theme` から外し、色・文字・クリップボードを引数で受ける部品にする（feature `egui`）。`wgpu_options` / `pick_backends`（feature `wgpu`）、`restrict_dll_search`（`sys/windows.rs`） | filer の `ui/term.rs` と `main.rs` のテストに当たるものが通る | 済み（v0.0.3） |
| 3 | filer を切り替える: `tsumugi-pane` を git の依存で読み、filer の `terminal.rs` などを消す。`【pane】` の印を外し、ペインの実機の行（TESTING.md の 1、19、29、40 節）を filer の再テストの順番表（`windows-role.md` の x64 と ARM64）に積む | filer の `scripts/verify.sh` が緑 | |

`shellhook.rs` は filer の CLI（`filer shell-hook`）の文言で、関数名やコメントに filer の名前が入っているので、段 3 では filer に残す。
tsumugi は自分の `shell-hook` を持つ（OSC 7 と OSC 133 の両方を出す形。v0.4.0 の状態の印で要る）。共有するのは「OSC 7 を読む側」だけ。

## 段 1 の API の形

filer の呼び出し（`app.rs`、`envreport.rs`、`main.rs`、`ui/term.rs`）が今の名前のまま動くよう、ルートで今の関数と型をそのまま出す（`tsumugi_pane::Terminal`、
`tsumugi_pane::encode` など）。filer は段 3 で `use tsumugi_pane as terminal;` と書けば呼び出しを直さずに済む。名前の整理は、filer が切り替わってから。

PTY のログの環境変数は `Terminal::spawn` に渡す（filer は `FILER_PTY_LOG`、tsumugi は `TSUMUGI_PTY_LOG`）。

## 進み具合

（段が終わるたびにここに 1 行足す。上限などで止まったときは、どこで止まったかを書く）

- 2026-10-05 v0.0.1: 段 0。workspace、空のクレート、`scripts/verify.sh`、CI。
- 2026-10-05 v0.0.2: 段 1。filer の `src/terminal.rs`（v0.78.121 の時点）を 7 つのモジュールに分けて移した。テスト 44 件（Linux で 43 件）が通る。
  filer の側は、まだ自分の `terminal.rs` を使っている。段 3 までの間に filer の `terminal.rs` が変わったら、同じ直しをこちらにも入れる（`【pane】` で止めてあるので、起きないはず）。
  macOS のコード（`sys/unix.rs` の `hang_up_children`）は型検査もしていない（macOS のターゲットを入れていない）。
- 2026-10-05 v0.0.3: 段 2。`show`（feature `egui`）、`gpu`（feature `wgpu`）、`restrict_dll_search`。テスト 51 件。
  filer の側で残るもの: 枠の上の線（`focus_rule`、filer の見た目）は filer が `show` の後に描く。クリップボードの読み書きと、読めなかったときのトーストも filer。
