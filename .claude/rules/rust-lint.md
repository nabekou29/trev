---
paths: "src/**/*.rs, Cargo.toml"
---

# Rust Lint Rules

This project uses very strict clippy rules.

## Key Constraints

- `unsafe_code = "forbid"` - No unsafe code
- `unwrap_used = "deny"` - Use `?` or proper error handling
- `expect_used = "deny"` - Use `?` or proper error handling
- `indexing_slicing = "deny"` - Use `.get()` instead
- `print_stdout/stderr = "deny"` - Use `tracing` for logging
- `missing_docs_in_private_items = "deny"` - Document everything

## Patterns

```rust
// BAD
let x = vec[0];
let y = opt.unwrap();

// GOOD
let x = vec.get(0);
let y = opt.ok_or(Error)?;
```
