---
paths: "src/**/*.rs"
---

# Rust Conventions

- Rust 2024 edition with strict lints (see Cargo.toml)
- No `mod.rs` - use modern module style (`foo.rs` + `foo/`)
- Prefer functional style: `iter()`, `filter()`, `map()`, `find_map()`

# Testing

## Framework

- **rstest** - `#[rstest]`, `#[case]`, `#[fixture]` (test attribute)
- **googletest** - `assert_that!`, `eq()`, `len()`, `some()`, `none()` (matchers only)

Always use `#[rstest]` as the test attribute. Do not use `#[googletest::test]` or `expect_that!`.

- `#[rstest]` provides `#[case]` parameterization and `#[fixture]`, which `#[googletest::test]` lacks
- `expect_that!` requires `#[googletest::test]` to work correctly; under `#[rstest]` failures are silently ignored

## In-Source Tests

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;
    use super::*;

    #[rstest]
    fn descriptive_test_name() {
        assert_that!(result, eq("expected"));
    }
}
```

## Key Points

- Place tests at file bottom
- Use `super::*` for parent module access
- Use `crate::test_utils::create_translation` for test data
- Name tests descriptively: `truncate_value_short_text`, not `test1`
