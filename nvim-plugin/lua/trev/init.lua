-- trev.nvim - Neovim integration for trev file viewer
--
-- Requires: toggleterm.nvim (for side panel mode)

local M = {}

-- Default configuration
M.config = {
  -- Path to trev executable
  trev_path = "trev",
  -- Working directory for daemon (nil = current directory)
  daemon_cwd = nil,
  -- Width of the side panel (percentage or absolute)
  width = 40,
  -- Automatically reveal file on buffer enter
  auto_reveal = true,
  -- Floating window config
  float = {
    -- Width ratio (0.0-1.0)
    width = 0.8,
    -- Height ratio (0.0-1.0)
    height = 0.8,
    -- Border style: "none", "single", "double", "rounded", "solid", "shadow"
    border = "rounded",
  },
}

-- State
M.state = {
  -- toggleterm Terminal instance (for side panel)
  terminal = nil,
  -- Is side panel open
  is_open = false,
  -- Augroup for auto-reveal
  augroup = nil,
  -- fs_event handle for command file watching
  fs_event = nil,
  -- Command file path
  cmd_file_path = nil,
  -- Floating window state
  float = {
    win = nil,
    buf = nil,
  },
}

-- Get the workspace key (Git root directory name or cwd directory name)
---@param path string
---@return string
local function get_workspace_key(path)
  -- Try to find Git root
  local git_root = vim.fn.systemlist({ "git", "-C", path, "rev-parse", "--show-toplevel" })[1]
  if vim.v.shell_error == 0 and git_root and git_root ~= "" then
    return vim.fn.fnamemodify(git_root, ":t")
  end
  -- Fallback to directory name
  return vim.fn.fnamemodify(path, ":t")
end

-- Get the command file path
---@param workspace_key string
---@return string
local function get_cmd_file_path(workspace_key)
  local runtime_dir = os.getenv("XDG_RUNTIME_DIR") or os.getenv("TMPDIR") or "/tmp"
  return runtime_dir .. "/trev/" .. workspace_key .. ".cmd"
end

-- Handle editor command from trev
---@param cmd_path string
local function handle_editor_command(cmd_path)
  -- Read the command file
  local file = io.open(cmd_path, "r")
  if not file then
    return
  end

  local content = file:read("*a")
  file:close()

  if not content or content == "" then
    return
  end

  -- Parse JSON
  local ok, cmd = pcall(vim.json.decode, content)
  if not ok or not cmd then
    return
  end

  local action = cmd.action
  local path = cmd.path

  if not action or not path then
    return
  end

  -- Focus on non-terminal window first
  local wins = vim.api.nvim_tabpage_list_wins(0)
  for _, win in ipairs(wins) do
    local buf = vim.api.nvim_win_get_buf(win)
    local buftype = vim.bo[buf].buftype
    if buftype ~= "terminal" then
      vim.api.nvim_set_current_win(win)
      break
    end
  end

  -- Execute the action
  if action == "edit" then
    vim.cmd("edit " .. vim.fn.fnameescape(path))
  elseif action == "split" then
    vim.cmd("split " .. vim.fn.fnameescape(path))
  elseif action == "vsplit" then
    vim.cmd("vsplit " .. vim.fn.fnameescape(path))
  elseif action == "tabedit" then
    vim.cmd("tabedit " .. vim.fn.fnameescape(path))
  end
end

-- Start watching the command file
local function start_command_watcher()
  local cwd = M.config.daemon_cwd or vim.fn.getcwd()
  local workspace_key = get_workspace_key(cwd)
  local cmd_path = get_cmd_file_path(workspace_key)

  M.state.cmd_file_path = cmd_path

  -- Ensure directory exists
  local dir = vim.fn.fnamemodify(cmd_path, ":h")
  vim.fn.mkdir(dir, "p")

  -- Create fs_event
  local fs_event = vim.uv.new_fs_event()
  if not fs_event then
    vim.notify("trev.nvim: Failed to create fs_event", vim.log.levels.WARN)
    return
  end

  M.state.fs_event = fs_event

  -- Start watching
  local ok, err = fs_event:start(cmd_path, {}, function(err2, filename, events)
    if err2 then
      return
    end
    -- Schedule to run in main thread
    vim.schedule(function()
      handle_editor_command(cmd_path)
    end)
  end)

  if not ok then
    -- File might not exist yet, that's OK - trev will create it
    -- We'll try to watch the directory instead and create watch on first write
    fs_event:stop()
    M.state.fs_event = nil

    -- Watch the directory instead
    local dir_event = vim.uv.new_fs_event()
    if dir_event then
      local dir_ok, dir_err = dir_event:start(dir, {}, function(err2, filename, events)
        if err2 then
          return
        end
        vim.schedule(function()
          if filename == workspace_key .. ".cmd" then
            handle_editor_command(cmd_path)
          end
        end)
      end)
      if dir_ok then
        M.state.fs_event = dir_event
      else
        vim.notify("trev.nvim: failed to watch directory: " .. tostring(dir_err), vim.log.levels.WARN)
      end
    end
  end
end

-- Stop watching the command file
local function stop_command_watcher()
  if M.state.fs_event then
    M.state.fs_event:stop()
    M.state.fs_event = nil
  end
  M.state.cmd_file_path = nil
end

-- Setup the plugin with user configuration
---@param opts table|nil
function M.setup(opts)
  M.config = vim.tbl_deep_extend("force", M.config, opts or {})

  -- Create augroup
  M.state.augroup = vim.api.nvim_create_augroup("TrevNvim", { clear = true })

  -- Parse arguments helper (order doesn't matter)
  local function parse_args(args_str)
    local opts = {
      float = false,
      reveal = false,
      action = nil,
    }
    for word in args_str:gmatch("%S+") do
      if word == "float" then
        opts.float = true
      elseif word == "reveal" then
        opts.reveal = true
      elseif word == "edit" or word == "split" or word == "vsplit" or word == "tabedit" then
        opts.action = word
      end
    end
    return opts
  end

  -- Create user commands
  vim.api.nvim_create_user_command("TrevToggle", function(args)
    local opts = parse_args(args.args)
    if opts.float then
      -- Float mode: toggle by closing if open, opening if closed
      if M.state.float.win and vim.api.nvim_win_is_valid(M.state.float.win) then
        M.close_float()
      else
        M.float(opts.action, opts.reveal)
      end
    else
      M.toggle(opts.reveal)
    end
  end, {
    nargs = "*",
    complete = function()
      return { "float", "reveal", "edit", "split", "vsplit", "tabedit" }
    end,
    desc = "Toggle trev (:TrevToggle [float] [reveal] [action])",
  })

  vim.api.nvim_create_user_command("TrevOpen", function(args)
    local opts = parse_args(args.args)
    if opts.float then
      M.float(opts.action, opts.reveal)
    else
      M.open(opts.reveal)
    end
  end, {
    nargs = "*",
    complete = function()
      return { "float", "reveal", "edit", "split", "vsplit", "tabedit" }
    end,
    desc = "Open trev (:TrevOpen [float] [reveal] [action])",
  })

  vim.api.nvim_create_user_command("TrevClose", function()
    M.close()
    M.close_float()
  end, { desc = "Close trev (side panel and float)" })

  vim.api.nvim_create_user_command("TrevReveal", function(args)
    local path = args.args
    if path == "" then
      path = vim.fn.expand("%:p")
    end
    M.reveal(path)
  end, {
    nargs = "?",
    complete = "file",
    desc = "Reveal file in trev",
  })

  -- Setup auto-reveal (only for side panel mode)
  if M.config.auto_reveal then
    vim.api.nvim_create_autocmd({ "BufEnter" }, {
      group = M.state.augroup,
      callback = function(ev)
        -- Skip if terminal is not open
        if not M.state.is_open then
          return
        end

        -- Skip special buffers
        local buftype = vim.bo[ev.buf].buftype
        if buftype ~= "" then
          return
        end

        -- Skip empty buffer names
        local bufname = vim.api.nvim_buf_get_name(ev.buf)
        if bufname == "" then
          return
        end

        -- Reveal the file
        vim.defer_fn(function()
          M.reveal(bufname)
        end, 100)
      end,
    })
  end
end

-- Create or get the terminal instance (for side panel)
---@param reveal_path string|nil Path to reveal on startup (only used for new terminal)
---@return table|nil terminal, boolean is_new
local function get_terminal(reveal_path)
  if M.state.terminal then
    return M.state.terminal, false
  end

  -- Check if toggleterm is available
  local ok, toggleterm = pcall(require, "toggleterm.terminal")
  if not ok then
    vim.notify("trev.nvim: toggleterm.nvim is required for side panel mode", vim.log.levels.ERROR)
    return nil, false
  end

  local Terminal = toggleterm.Terminal
  local cwd = M.config.daemon_cwd or vim.fn.getcwd()
  local cmd = string.format("%s --daemon", M.config.trev_path)

  -- Add reveal option if path is provided
  if reveal_path and reveal_path ~= "" then
    cmd = cmd .. " --reveal " .. vim.fn.shellescape(reveal_path)
  end

  M.state.terminal = Terminal:new({
    cmd = cmd,
    dir = cwd,
    direction = "vertical",
    close_on_exit = true,
    hidden = false,
    on_open = function(term)
      M.state.is_open = true

      -- Move window to the left
      vim.cmd("wincmd H")
      vim.api.nvim_win_set_width(term.window, M.config.width)

      -- Start command file watcher
      vim.defer_fn(function()
        start_command_watcher()
      end, 100)
    end,
    on_close = function()
      M.state.is_open = false
      stop_command_watcher()
    end,
  })

  return M.state.terminal, true
end

-- Open trev side panel
---@param reveal boolean|nil Whether to reveal the current file after opening
function M.open(reveal)
  -- Get current file BEFORE opening terminal (terminal changes current buffer)
  local current_file = nil
  if reveal then
    current_file = vim.fn.expand("%:p")
    if current_file == "" or vim.fn.filereadable(current_file) ~= 1 then
      current_file = nil
    end
  end

  local term, is_new = get_terminal(current_file)
  if term then
    term:open()
    -- If terminal already existed and reveal requested, use IPC
    if not is_new and current_file then
      vim.defer_fn(function()
        M.reveal(current_file)
      end, 100)
    end
  end
end

-- Close trev side panel
function M.close()
  if M.state.terminal then
    -- Send quit command first
    vim.system({ M.config.trev_path, "ctl", "quit" }, {}, function() end)
    M.state.terminal:close()
  end
end

-- Toggle trev side panel
---@param reveal boolean|nil Whether to reveal the current file after opening
function M.toggle(reveal)
  -- Get current file BEFORE toggling terminal (terminal changes current buffer)
  local current_file = nil
  if reveal then
    current_file = vim.fn.expand("%:p")
    if current_file == "" or vim.fn.filereadable(current_file) ~= 1 then
      current_file = nil
    end
  end

  local was_open = M.state.is_open
  local term, is_new = get_terminal(current_file)
  if term then
    term:toggle(M.config.width)
    -- If terminal already existed and reveal requested and panel was just opened, use IPC
    if not is_new and not was_open and current_file then
      vim.defer_fn(function()
        M.reveal(current_file)
      end, 100)
    end
  end
end

-- Calculate floating window dimensions
local function get_float_dimensions()
  local width = math.floor(vim.o.columns * M.config.float.width)
  local height = math.floor(vim.o.lines * M.config.float.height)
  local col = math.floor((vim.o.columns - width) / 2)
  local row = math.floor((vim.o.lines - height) / 2)

  return {
    width = width,
    height = height,
    col = col,
    row = row,
  }
end

-- Close floating window
local function close_float()
  if M.state.float.win and vim.api.nvim_win_is_valid(M.state.float.win) then
    vim.api.nvim_win_close(M.state.float.win, true)
  end
  if M.state.float.buf and vim.api.nvim_buf_is_valid(M.state.float.buf) then
    vim.api.nvim_buf_delete(M.state.float.buf, { force = true })
  end
  M.state.float.win = nil
  M.state.float.buf = nil
end

-- Open trev in a floating window
---@param action string|nil Action to perform: "edit", "split", "vsplit", "tabedit" (default: "edit")
---@param reveal boolean|nil Whether to reveal the current file (default: false)
function M.float(action, reveal)
  action = action or "edit"

  -- Get current file BEFORE creating floating window (nvim_open_win enters the new window)
  local current_file = nil
  if reveal then
    current_file = vim.fn.expand("%:p")
  end

  -- Close existing float if any
  close_float()

  local dims = get_float_dimensions()
  local cwd = M.config.daemon_cwd or vim.fn.getcwd()

  -- Create buffer
  local buf = vim.api.nvim_create_buf(false, true)
  M.state.float.buf = buf

  -- Create floating window
  local win = vim.api.nvim_open_win(buf, true, {
    relative = "editor",
    width = dims.width,
    height = dims.height,
    col = dims.col,
    row = dims.row,
    style = "minimal",
    border = M.config.float.border,
    title = " trev ",
    title_pos = "center",
  })
  M.state.float.win = win

  -- Build command with emit flag
  local cmd = string.format("%s --emit --action %s", M.config.trev_path, action)

  -- Add reveal flag if requested
  if reveal and current_file and current_file ~= "" and vim.fn.filereadable(current_file) == 1 then
    cmd = cmd .. " --reveal " .. vim.fn.shellescape(current_file)
  end

  -- Start terminal with trev
  vim.fn.termopen(cmd, {
    cwd = cwd,
    on_exit = function(job_id, exit_code, event)
      vim.schedule(function()
        -- Get output from buffer before closing
        if exit_code == 0 and vim.api.nvim_buf_is_valid(buf) then
          local lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
          -- Find the last non-empty line (the selected path)
          local selected_path = nil
          for i = #lines, 1, -1 do
            local line = lines[i]:gsub("^%s*(.-)%s*$", "%1") -- trim
            if line ~= "" then
              selected_path = line
              break
            end
          end

          -- Close float first
          close_float()

          -- Open the file if path was selected
          if selected_path and selected_path ~= "" and vim.fn.filereadable(selected_path) == 1 then
            if action == "edit" then
              vim.cmd("edit " .. vim.fn.fnameescape(selected_path))
            elseif action == "split" then
              vim.cmd("split " .. vim.fn.fnameescape(selected_path))
            elseif action == "vsplit" then
              vim.cmd("vsplit " .. vim.fn.fnameescape(selected_path))
            elseif action == "tabedit" then
              vim.cmd("tabedit " .. vim.fn.fnameescape(selected_path))
            end
          end
        else
          close_float()
        end
      end)
    end,
  })

  -- Start insert mode for terminal
  vim.cmd("startinsert")

  -- Set up keymaps for the floating window
  vim.api.nvim_buf_set_keymap(buf, "t", "<Esc><Esc>", "<cmd>lua require('trev').close_float()<CR>", {
    noremap = true,
    silent = true,
    desc = "Close trev floating window",
  })
end

-- Close floating window (public API)
function M.close_float()
  close_float()
end

-- Reveal a file in trev (side panel mode only)
---@param path string
function M.reveal(path)
  if not M.state.is_open then
    return
  end

  -- Normalize path
  local abs_path = vim.fn.fnamemodify(path, ":p")

  -- Send reveal command
  vim.system({ M.config.trev_path, "ctl", "reveal", abs_path }, {}, function() end)
end

-- Reveal current file
function M.reveal_current()
  M.reveal(vim.fn.expand("%:p"))
end

return M
