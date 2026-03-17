#!/bin/bash
# Redirect cargo commands to mise tasks in this project
#   cargo clippy → mise run lint
#   cargo fmt    → mise run format
#   cargo build  → mise run build

INPUT=$(cat)
COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty')

# Extract the first command in a pipeline/chain (before |, &&, ;)
# and strip leading whitespace
FIRST_CMD=$(echo "$COMMAND" | sed 's/[|;&].*//' | sed 's/^[[:space:]]*//')

if [[ "$FIRST_CMD" == cargo\ clippy* ]]; then
  echo "Use 'mise run lint' instead of 'cargo clippy' in this project." >&2
  exit 2
fi

if [[ "$FIRST_CMD" == cargo\ fmt* ]]; then
  echo "Use 'mise run format' instead of 'cargo fmt' in this project." >&2
  exit 2
fi

if [[ "$FIRST_CMD" == cargo\ build* ]]; then
  echo "Use 'mise run build' instead of 'cargo build' in this project." >&2
  exit 2
fi

exit 0
