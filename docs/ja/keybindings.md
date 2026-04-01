# デフォルトキーバインド

英語版（ソースから自動生成）をベースに手動翻訳しています。

## ナビゲーション

| キー      | アクション                     | 説明                       |
| --------- | ------------------------------ | -------------------------- |
| `j`       | `tree.move_down`               | 下に移動                   |
| `k`       | `tree.move_up`                 | 上に移動                   |
| `l`       | `tree.expand`                  | 展開 / 開く                |
| `h`       | `tree.collapse`                | 折りたたみ / 親へ          |
| `-`       | `tree.toggle_expand`           | 展開の切替                 |
| `g`       | `tree.jump_first`              | 先頭にジャンプ             |
| `G`       | `tree.jump_last`               | 末尾にジャンプ             |
| `<C-d>`   | `tree.half_page_down`          | 半ページ下                 |
| `<C-u>`   | `tree.half_page_up`            | 半ページ上                 |
| `E`       | `tree.expand_all`              | すべて展開                 |
| `W`       | `tree.collapse_all`            | すべて折りたたみ           |
| `R`       | `tree.refresh`                 | ツリーを更新               |
| `<Enter>` | `tree.change_root`             | 選択項目をルートに変更     |
| `<BS>`    | `tree.change_root_up`          | 親をルートに変更           |
| `zz`      | `tree.center_cursor`           | カーソルを中央に配置       |
| `zt`      | `tree.scroll_cursor_to_top`    | カーソルを上端にスクロール |
| `zb`      | `tree.scroll_cursor_to_bottom` | カーソルを下端にスクロール |

## フィルター

| キー | アクション       | 説明                       |
| ---- | ---------------- | -------------------------- |
| `.`  | `filter.hidden`  | 隠しファイルの表示切替     |
| `I`  | `filter.ignored` | ignored ファイルの表示切替 |

## プレビュー

| キー      | アクション                    | 説明                 |
| --------- | ----------------------------- | -------------------- |
| `J`       | `preview.scroll_down`         | 下にスクロール       |
| `K`       | `preview.scroll_up`           | 上にスクロール       |
| `L`       | `preview.scroll_right`        | 右にスクロール       |
| `H`       | `preview.scroll_left`         | 左にスクロール       |
| `-`       | `preview.half_page_down`      | 半ページ下           |
| `-`       | `preview.half_page_up`        | 半ページ上           |
| `<Tab>`   | `preview.cycle_next_provider` | 次のプロバイダ       |
| `<S-Tab>` | `preview.cycle_prev_provider` | 前のプロバイダ       |
| `P`       | `preview.toggle_preview`      | プレビューの表示切替 |
| `w`       | `preview.toggle_wrap`         | 折り返しの切替       |

## ファイル操作

| キー      | アクション                     | 説明                             |
| --------- | ------------------------------ | -------------------------------- |
| `y`       | `file_op.yank`                 | ファイルをコピー（ヤンク）       |
| `x`       | `file_op.cut`                  | ファイルを切り取り               |
| `p`       | `file_op.paste`                | ファイルを貼り付け               |
| -         | `file_op.paste_from_clipboard` | クリップボードから貼り付け       |
| `a`       | `file_op.create_file`          | ファイルを作成                   |
| `A`       | `file_op.create_directory`     | ディレクトリを作成               |
| `r`       | `file_op.rename`               | リネーム                         |
| `d`       | `file_op.delete`               | 削除                             |
| `D`       | `file_op.system_trash`         | ゴミ箱に移動                     |
| `u`       | `file_op.undo`                 | 元に戻す                         |
| `<C-r>`   | `file_op.redo`                 | やり直す                         |
| `<Space>` | `file_op.toggle_mark`          | マークの切替                     |
| `<Esc>`   | `file_op.clear_selections`     | 選択をクリア                     |
| `<C-p>`   | `file_op.paste_menu`           | 貼り付けオプション               |
| `-`       | `file_op.paste_as_symlink`     | シンボリックリンクとして貼り付け |
| `-`       | `file_op.paste_as_hardlink`    | ハードリンクとして貼り付け       |

## 検索

| キー | アクション    | 説明       |
| ---- | ------------- | ---------- |
| `/`  | `search.open` | 検索を開く |

## 一般

| キー | アクション    | 説明           |
| ---- | ------------- | -------------- |
| `q`  | `quit`        | 終了           |
| `e`  | `open_editor` | エディタで開く |
| `?`  | `help`        | ヘルプを表示   |
| `-`  | `noop`        | 操作なし       |
