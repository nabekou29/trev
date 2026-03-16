#!/bin/bash
# Auto-format Markdown files after edit

INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')

if [[ "$FILE_PATH" != *.md ]]; then
  exit 0
fi

oxfmt "$FILE_PATH" 2>&1
