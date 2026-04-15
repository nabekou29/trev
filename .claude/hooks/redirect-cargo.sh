#!/bin/bash
# Redirect cargo commands to just tasks in this project
#   cargo clippy → just lint
#   cargo fmt    → just format
#   cargo build  → just build

INPUT=$(cat)
COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty')

# Extract the first command in a pipeline/chain (before |, &&, ;)
# and strip leading whitespace
FIRST_CMD=$(echo "$COMMAND" | sed 's/[|;&].*//' | sed 's/^[[:space:]]*//')

if [[ "$FIRST_CMD" == cargo\ clippy* ]]; then
  echo "Use 'just lint' instead of 'cargo clippy' in this project." >&2
  exit 2
fi

if [[ "$FIRST_CMD" == cargo\ fmt* ]]; then
  echo "Use 'just format' instead of 'cargo fmt' in this project." >&2
  exit 2
fi

if [[ "$FIRST_CMD" == cargo\ build* ]]; then
  echo "Use 'just build' instead of 'cargo build' in this project." >&2
  exit 2
fi

exit 0
