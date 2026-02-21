# Quickstart: Smart Sort

## テストシナリオ

### 1. Natural Sort の検証

```
テストディレクトリ構成:
  file1.txt, file2.txt, file10.txt, file20.txt, file100.txt

期待される smart sort 順序:
  file1.txt, file2.txt, file10.txt, file20.txt, file100.txt
  (NOT: file1.txt, file10.txt, file100.txt, file2.txt, file20.txt)
```

### 2. 接尾辞グルーピングの検証

```
テストディレクトリ構成:
  admin.ts
  user.ts
  user.test.ts
  user.stories.ts
  user.mock.ts

期待される smart sort 順序:
  admin.ts
  user.ts          ← 接尾辞なし = グループ先頭
  user.mock.ts     ← .mock < .stories < .test (アルファベット順)
  user.stories.ts
  user.test.ts
```

### 3. Go スタイルの検証

```
テストディレクトリ構成:
  handler.go
  handler_test.go
  server.go
  server_test.go

期待される smart sort 順序:
  handler.go
  handler_test.go
  server.go
  server_test.go
```

### 4. 3ドットファイルの検証

```
テストディレクトリ構成:
  foo.bar.ts
  foo.bar.test.ts

期待される smart sort 順序:
  foo.bar.ts       ← ベース "foo.bar", 接尾辞なし
  foo.bar.test.ts  ← ベース "foo.bar", 接尾辞 ".test.ts"
```

### 5. ソートメニューの検証

```
操作手順:
  1. trev でディレクトリを開く
  2. S キーを押す → メニュー表示
  3. 現在のソート方式 (smart) がハイライト表示されていることを確認
  4. j/k でカーソル移動
  5. "name" を選択 → Enter
  6. ツリーが名前順に再ソートされることを確認
  7. S → "smart" → Enter で smart sort に戻ることを確認
```

### 6. デフォルト変更の検証

```
操作手順:
  1. 設定ファイルなしで trev を起動
  2. ファイルが smart sort で表示されることを確認
  3. ~/.config/trev/config.yml に sort.order: name を設定
  4. trev を再起動 → 名前順ソートになることを確認
  5. trev --sort-order smart で起動 → smart sort になることを確認
```

### 7. エッジケースの検証

```
数字のみ:       1, 2, 10 → 1, 2, 10
長い数字:       file99999999999999999.txt → クラッシュしない
Unicode:         あいう.txt, かきく.txt → ソートされる（クラッシュしない）
拡張子なし:      Makefile → 単独で並ぶ（グルーピングなし）
偽陽性テスト:    test_utils.ts → "test_utils.ts" として扱われる（"test" ベースにならない）
ドットファイル:  .gitignore → 通常ファイルと同じ規則
```
