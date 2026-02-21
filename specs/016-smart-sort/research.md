# Research: Smart Sort

## 1. Natural Sort Algorithm

**Decision**: 自前実装（チャンクベース比較）

**Rationale**:
- 外部 crate（`natord`, `lexical-sort`）は追加依存。constitution IV（YAGNI）の観点から不要
- smart sort は natural sort に加えて接尾辞グルーピングが必要。既存 crate では接尾辞ロジックを組み込めない
- 実装量は ~50 行程度（チャンク分割 + 比較）

**Alternatives considered**:
- `natord` crate: シンプルだが最終更新が古く、case-insensitive の制御が限定的
- `lexical-sort` crate: 高機能だが、接尾辞グルーピングとの統合が煩雑
- `alphanumeric-sort` crate: good API だが追加依存

**Implementation approach**:
```
compare_natural(a, b):
  split each string into chunks: Text("file") | Num(10) | Text(".txt")
  compare chunk-by-chunk:
    Text vs Text: case-insensitive, tie-break case-sensitive
    Num vs Num: compare by value (digit count, then lexicographic)
    Text vs Num: text comes after numbers
```

数値は u128 等にパースせず、桁数比較 → 文字列比較で任意精度をサポートする。

## 2. 接尾辞分解アルゴリズム

**Decision**: 右端から1セグメント分を接尾辞として分解

**Rationale**:
- `xxx.yyy.ext` パターンが最も一般的（`.test.ts`, `.stories.tsx`, `.config.js` 等）
- 3ドット以上は最後の1セグメントのみ接尾辞（clarification で確定済み）
- アンダースコアパターンは `_test` と `_spec` のみ（調査で確定済み）

**Algorithm**:
```
decompose_name(name):
  // 1. Find last dot (extension separator)
  ext_pos = name.rfind('.')
  if no dot: return (name, None)  // no extension, no suffix

  stem = name[..ext_pos]  // e.g., "user.test" from "user.test.ts"
  ext = name[ext_pos..]   // e.g., ".ts"

  // 2. Check underscore patterns in stem
  if stem ends with "_test" or "_spec":
    base = stem[..stem.len()-5 or -5]
    suffix = stem[base.len()..] + ext  // e.g., "_test.go"
    return (base, Some(suffix))

  // 3. Check dot pattern in stem
  dot_pos = stem.rfind('.')
  if dot found:
    base = stem[..dot_pos]     // e.g., "user" from "user.test"
    suffix = stem[dot_pos..] + ext  // e.g., ".test.ts"
    return (base, Some(suffix))

  // 4. No suffix pattern
  return (name, None)  // e.g., "main.rs" → ("main.rs", None)
```

## 3. メニュー汎用化パターン

**Decision**: `MenuState` に `MenuAction` enum を追加

**Rationale**:
- 最小限の変更で既存の CopyMenu と新しい SortMenu を両方サポート
- `AppMode::Menu` を分割するより単純
- ハンドラ側は `match menu_action` で分岐するだけ

**Alternatives considered**:
- 新しい `AppMode::SortMenu(SortMenuState)`: 重複コードが増える
- コールバック関数ポインタ: Rust の所有権モデルと合わない
- trait object: over-engineering
