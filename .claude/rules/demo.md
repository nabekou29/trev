---
paths: "demo/**"
---

# Demo Tape Rules

## Debug-first workflow

When modifying demo.tape, always follow this workflow:

1. **Add debug screenshots at each step**: `Screenshot demo/debug-XX-description.png`
2. **Run `mise run demo`** to generate the recording
3. **Verify each screenshot with the Read tool**: check cursor position, tree expansion state, and preview content
4. **Fix and re-run** if anything is off
5. **Remove all debug screenshot lines and image files** once every step is confirmed correct

## trev key behavior gotchas

- `l` (Expand): Does **nothing** on an already-expanded directory — use `j` to move into children
- `E` (Expand All): Async loading means it may **only expand one level**. Expand deeper levels step by step
- After expansion, use **Sleep 600ms or more** to wait for async directory loads
- Cursor position can be verified via the `N/M` indicator in the bottom-right corner
