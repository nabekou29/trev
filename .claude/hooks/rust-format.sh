#!/bin/bash
# Auto-format Rust files after edit

INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')

if [[ "$FILE_PATH" != *.rs ]]; then
  exit 0
fi

cd "$(echo "$INPUT" | jq -r '.cwd')"
cargo fmt --all 2>&1
