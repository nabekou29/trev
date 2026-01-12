# trev.nvim

Neovim integration for [trev](https://github.com/nabekou29/trev) file viewer.

## Requirements

- Neovim 0.10+
- [toggleterm.nvim](https://github.com/akinsho/toggleterm.nvim)
- trev with daemon mode support

## Installation

### lazy.nvim

```lua
{
  dir = "/path/to/trev/nvim-plugin",
  dependencies = { "akinsho/toggleterm.nvim" },
  config = function()
    require("trev").setup({
      width = 40,
      auto_reveal = true,
    })
  end,
  keys = {
    { "<leader>e", "<cmd>TrevToggle<cr>", desc = "Toggle trev" },
  },
}
```

## Commands

| Command | Description |
|---------|-------------|
| `:TrevToggle` | Toggle trev side panel |
| `:TrevOpen` | Open trev side panel |
| `:TrevClose` | Close trev side panel |
| `:TrevReveal [path]` | Reveal specific file |

## Configuration

```lua
require("trev").setup({
  -- Path to trev executable
  trev_path = "trev",

  -- Working directory (nil = current directory)
  daemon_cwd = nil,

  -- Width of the side panel
  width = 40,

  -- Automatically reveal file on buffer enter
  auto_reveal = true,
})
```

## How it works

1. `:TrevToggle` opens trev with `--daemon` flag in a toggleterm vertical split
2. When `auto_reveal = true`, switching buffers sends `trev ctl reveal <path>`
3. Closing the terminal sends `trev ctl quit` to stop the daemon
