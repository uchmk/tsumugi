# 確認事項

人の判断が要ることをここに書く。形式は filer の CLAUDE.md の「確認事項」と同じ（番号は通しで再利用しない、選択肢に推奨を 1 つ）。

## Q1: 既定のキー
- 状態: 回答済み（2026-10-05、持ち主: 「いい案があれば」。下の案で進め、使ってみて直す）
- タスク: docs/v1-scope.md「決めたこと」
- 背景: Windows を第一にするので、Windows Terminal の既定に寄せる（Windows の人の指がもう覚えている）。macOS は iTerm2 と cmux の
  慣習（Cmd）に寄せる。**Claude Code 自身が使うキー（`Ctrl+C`、`Esc`、`Shift+Tab`、`Shift+Enter`、`Ctrl+R` など）は既定で取らない。**
  素の `Ctrl+英字` もシェルやエディタが使うので取らず、`Ctrl+Shift` か `Alt+Shift` にする。
- 案:

  | 動き | Windows / Linux | macOS | 元にしたもの |
  | --- | --- | --- | --- |
  | 新しいタブ | `Ctrl+Shift+T` | `Cmd+T` | Windows Terminal / iTerm2 |
  | ペインかタブを閉じる | `Ctrl+Shift+W` | `Cmd+W` | Windows Terminal / iTerm2 |
  | 右に分割 | `Alt+Shift+=` | `Cmd+D` | Windows Terminal / iTerm2・cmux |
  | 下に分割 | `Alt+Shift+-` | `Cmd+Shift+D` | Windows Terminal / iTerm2・cmux |
  | ペインの移動 | `Alt+矢印` | `Cmd+Option+矢印` | Windows Terminal / iTerm2 |
  | 次 / 前のタブ | `Ctrl+Tab` / `Ctrl+Shift+Tab` | 同じ | 共通 |
  | N 番目のタブ | `Ctrl+Alt+1`〜`9` | `Cmd+1`〜`9` | Windows Terminal / 共通 |
  | **入力待ちのタブへ飛ぶ** | `Ctrl+Shift+U` | `Cmd+Shift+U` | 独自（Unread の U）。押すたびに、待っている時間の長い順に回る |
  | コマンドパレット | `Ctrl+Shift+P` | `Cmd+Shift+P` | Windows Terminal / VS Code |

- 回答: 案で進める。使ってみて直す。

## Q2: 「入力待ち」と判断する条件
- 状態: 回答済み（2026-10-05、持ち主: 「他のターミナルの推奨で」）
- タスク: docs/v1-scope.md「v1 に入れるもの」2
- 背景: 他のターミナルのやり方は 2 つに分かれる。cmux はエージェントから知らせてもらう（フックと OSC 9 / 99 / 777）。
  Konsole や iTerm2、tmux（`monitor-silence`）は「出力が N 秒止まった」で知らせる（Konsole の既定は 10 秒）。
  前者は確かだが仕込みが要り、後者は仕込み不要だが「考え中で黙っている」のと区別できない。
- 案（3 段に重ね、確かなものほど強く出す）:
  1. **知らせてもらったら、すぐ「入力待ち」。** Claude Code のフック（`Notification` / `Stop`）から `tsumugi notify`、または OSC 9 / 99 / 777。
     最初に開いたとき、Claude Code のフックの設定を足すかを 1 回だけ聞く。
  2. **シェルのプロンプトが戻ったら「終わった」。** OSC 133（シェルの統合）。filer の `shell-hook` と同じ仕組み。
  3. **10 秒、出力が止まったら「たぶん入力待ち」**（Konsole の既定に合わせる）。ただし、最後のキー入力より後に出力があり、子プロセスが生きているときだけ。
     印は 1 より薄くして、確かな印と見分けられるようにする。秒数は設定で変えられ、0 で止められる。
- 回答: 案で進める。
