--- trev.nvim — Neovim integration for trev file viewer.
--- Provides side panel, float picker, reveal, and external command handling
--- via JSON-RPC 2.0 over Unix Domain Socket.
local M = {}

--- @class trev.Config
--- @field trev_path string Path to trev binary
--- @field width number Side panel width (columns)
--- @field auto_reveal boolean Auto-reveal on BufEnter
--- @field action string Default editor action (edit/split/vsplit/tabedit)
--- @field handlers table<string, function> External command handlers

--- @type trev.Config
local config = {
  trev_path = "trev",
  width = 30,
  auto_reveal = true,
  action = "edit",
  handlers = {},
}

--- Plugin state.
--- @class trev.State
--- @field pipe userdata|nil vim.uv pipe handle
--- @field pid number|nil trev daemon process ID
--- @field job_id number|nil terminal job ID
--- @field buf number|nil terminal buffer number
--- @field win number|nil terminal window number
--- @field socket_path string|nil path to UDS socket
--- @field read_buf string partial read buffer for line framing
--- @field request_id number next JSON-RPC request ID
--- @field pending table<number, function> pending request callbacks

--- @type trev.State
local state = {
  pipe = nil,
  pid = nil,
  job_id = nil,
  buf = nil,
  win = nil,
  socket_path = nil,
  read_buf = "",
  request_id = 1,
  pending = {},
}

--- @type number|nil
local augroup = nil

---------------------------------------------------------------------------
-- Helpers
---------------------------------------------------------------------------

--- Escape a file path for use in a Vim command.
--- @param path string
--- @return string
local function escape_path(path)
  return vim.fn.fnameescape(path)
end

--- Check if a buffer is a "special" buffer that should not trigger auto-reveal.
--- @param bufnr number
--- @return boolean
local function is_special_buffer(bufnr)
  local bt = vim.bo[bufnr].buftype
  if bt == "terminal" or bt == "quickfix" or bt == "help" or bt == "nofile" or bt == "prompt" then
    return true
  end
  return false
end

---------------------------------------------------------------------------
-- JSON-RPC client (vim.uv pipe)
---------------------------------------------------------------------------

--- Connect the JSON-RPC pipe to the daemon socket.
--- @param socket string path to UDS
--- @param on_connect function|nil callback on successful connection
local function ipc_connect(socket, on_connect)
  if state.pipe then
    return
  end

  local pipe = vim.uv.new_pipe(false)
  if not pipe then
    vim.notify("[trev] failed to create pipe", vim.log.levels.ERROR)
    return
  end

  pipe:connect(socket, function(err)
    if err then
      vim.notify("[trev] connect error: " .. err, vim.log.levels.ERROR)
      pipe:close()
      return
    end

    state.pipe = pipe
    state.socket_path = socket
    state.read_buf = ""

    pipe:read_start(function(read_err, data)
      if read_err then
        vim.notify("[trev] read error: " .. read_err, vim.log.levels.WARN)
        M._disconnect()
        return
      end

      if not data then
        -- EOF — daemon disconnected.
        M._disconnect()
        return
      end

      -- Line-buffered JSON parsing.
      state.read_buf = state.read_buf .. data
      while true do
        local nl = state.read_buf:find("\n")
        if not nl then
          break
        end
        local line = state.read_buf:sub(1, nl - 1)
        state.read_buf = state.read_buf:sub(nl + 1)
        if #line > 0 then
          vim.schedule(function()
            M._handle_message(line)
          end)
        end
      end
    end)

    if on_connect then
      vim.schedule(on_connect)
    end
  end)
end

--- Disconnect the JSON-RPC pipe.
function M._disconnect()
  if state.pipe then
    if not state.pipe:is_closing() then
      state.pipe:read_stop()
      state.pipe:close()
    end
    state.pipe = nil
  end
  state.socket_path = nil
  state.read_buf = ""
  state.pending = {}
end

--- Send a JSON-RPC notification (no response expected).
--- @param method string
--- @param params table|nil
function M._send_notification(method, params)
  if not state.pipe then
    return
  end
  local msg = vim.json.encode({
    jsonrpc = "2.0",
    method = method,
    params = params,
  }) .. "\n"
  state.pipe:write(msg)
end

--- Send a JSON-RPC request (expects a response).
--- @param method string
--- @param params table|nil
--- @param callback function|nil called with (result, error)
function M._send_request(method, params, callback)
  if not state.pipe then
    if callback then
      callback(nil, "not connected")
    end
    return
  end

  local id = state.request_id
  state.request_id = state.request_id + 1

  if callback then
    state.pending[id] = callback
  end

  local msg = vim.json.encode({
    jsonrpc = "2.0",
    method = method,
    params = params,
    id = id,
  }) .. "\n"
  state.pipe:write(msg)
end

--- Handle an incoming JSON-RPC message (notification or response).
--- @param line string raw JSON line
function M._handle_message(line)
  local ok, msg = pcall(vim.json.decode, line)
  if not ok or type(msg) ~= "table" then
    return
  end

  -- Response to a pending request.
  if msg.id ~= nil then
    local cb = state.pending[msg.id]
    if cb then
      state.pending[msg.id] = nil
      cb(msg.result, msg.error)
    end
    return
  end

  -- Notification from daemon.
  local method = msg.method
  local params = msg.params or {}

  if method == "open_file" then
    M._handle_open_file(params)
  elseif method == "external_command" then
    M._handle_external_command(params)
  end
end

--- Handle open_file notification: open a file in Neovim.
--- @param params table {action: string, path: string}
function M._handle_open_file(params)
  local path = params.path
  local action = params.action or config.action
  if not path then
    return
  end

  -- Focus the editor window (not the trev panel).
  local target_win = M._find_editor_window()
  if target_win then
    vim.api.nvim_set_current_win(target_win)
  end

  vim.cmd(action .. " " .. escape_path(path))
end

--- Handle external_command notification: dispatch to user handler.
--- @param params table {command: string}
function M._handle_external_command(params)
  local command = params.command
  if not command then
    return
  end
  local handler = config.handlers[command]
  if handler then
    handler()
  end
end

--- Find an editor window (not the trev side panel).
--- @return number|nil window ID
function M._find_editor_window()
  local current = vim.api.nvim_get_current_win()
  if current ~= state.win then
    return current
  end
  -- Find the first non-trev window.
  for _, win in ipairs(vim.api.nvim_tabpage_list_wins(0)) do
    if win ~= state.win then
      return win
    end
  end
  return nil
end

---------------------------------------------------------------------------
-- Socket discovery
---------------------------------------------------------------------------

--- Build the expected socket path for a given pid and workspace dir.
--- @param pid number
--- @param workspace string
--- @return string|nil
local function find_socket_for_pid(pid, workspace)
  -- Socket is at $XDG_RUNTIME_DIR/trev/<key>-<pid>.sock or $TMPDIR/trev/<key>-<pid>.sock
  local runtime_dir = vim.env.XDG_RUNTIME_DIR
  local base
  if runtime_dir and runtime_dir ~= "" then
    base = runtime_dir
  else
    base = vim.fn.getenv("TMPDIR")
    if not base or base == vim.NIL or base == "" then
      base = "/tmp"
    end
  end
  local trev_dir = base .. "/trev"
  local pattern = trev_dir .. "/*-" .. pid .. ".sock"
  local matches = vim.fn.glob(pattern, false, true)
  if #matches > 0 then
    return matches[1]
  end

  -- Wait briefly and retry (daemon may not have created socket yet).
  vim.wait(500, function()
    matches = vim.fn.glob(pattern, false, true)
    return #matches > 0
  end, 50)
  if #matches > 0 then
    return matches[1]
  end

  return nil
end

---------------------------------------------------------------------------
-- Side panel (terminal)
---------------------------------------------------------------------------

--- Open the trev side panel.
function M.open()
  if state.win and vim.api.nvim_win_is_valid(state.win) then
    -- Already open — focus it.
    vim.api.nvim_set_current_win(state.win)
    return
  end

  local workspace = vim.fn.getcwd()
  local cmd = {
    config.trev_path,
    "--daemon",
    "--action",
    config.action,
    workspace,
  }

  -- Create a vertical split on the left.
  vim.cmd("topleft " .. config.width .. "vsplit")
  local win = vim.api.nvim_get_current_win()
  local buf = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_win_set_buf(win, buf)

  -- Disable line numbers and other decorations in the panel.
  vim.wo[win].number = false
  vim.wo[win].relativenumber = false
  vim.wo[win].signcolumn = "no"
  vim.wo[win].foldcolumn = "0"
  vim.wo[win].winfixwidth = true

  -- Start the terminal.
  local job_id = vim.fn.termopen(cmd, {
    on_exit = function(_, exit_code, _)
      vim.schedule(function()
        M._on_daemon_exit(exit_code)
      end)
    end,
  })

  if job_id <= 0 then
    vim.notify("[trev] failed to start trev daemon", vim.log.levels.ERROR)
    vim.api.nvim_win_close(win, true)
    return
  end

  state.win = win
  state.buf = vim.api.nvim_get_current_buf()
  state.job_id = job_id
  state.pid = vim.fn.jobpid(job_id)

  -- Enter terminal mode for immediate interaction.
  vim.cmd("startinsert")

  -- Connect IPC after a short delay (daemon needs time to bind socket).
  vim.defer_fn(function()
    if state.pid then
      local socket = find_socket_for_pid(state.pid, workspace)
      if socket then
        ipc_connect(socket, function()
          M._setup_auto_reveal()
        end)
      else
        vim.notify("[trev] socket not found for pid " .. state.pid, vim.log.levels.WARN)
      end
    end
  end, 300)
end

--- Close the trev side panel.
function M.close()
  M._teardown_auto_reveal()
  M._disconnect()

  if state.job_id then
    vim.fn.jobstop(state.job_id)
    state.job_id = nil
  end
  if state.win and vim.api.nvim_win_is_valid(state.win) then
    vim.api.nvim_win_close(state.win, true)
  end
  state.win = nil
  state.buf = nil
  state.pid = nil
end

--- Toggle the side panel.
--- @param mode string|nil "float" for float picker, nil for side panel
--- @param action string|nil override editor action
function M.toggle(mode, action)
  if mode == "float" then
    M.float_pick(action)
    return
  end

  if state.win and vim.api.nvim_win_is_valid(state.win) then
    M.close()
  else
    M.open()
  end
end

--- Handle daemon process exit.
--- @param exit_code number
function M._on_daemon_exit(exit_code)
  M._teardown_auto_reveal()
  M._disconnect()
  state.job_id = nil
  state.pid = nil

  if state.win and vim.api.nvim_win_is_valid(state.win) then
    vim.api.nvim_win_close(state.win, true)
  end
  state.win = nil
  state.buf = nil

  if exit_code ~= 0 then
    vim.notify("[trev] daemon exited with code " .. exit_code, vim.log.levels.WARN)
  end
end

---------------------------------------------------------------------------
-- Float picker (--emit mode)
---------------------------------------------------------------------------

--- Open a float picker for quick file selection.
--- @param action string|nil editor action override
function M.float_pick(action)
  action = action or config.action
  local workspace = vim.fn.getcwd()
  local cmd = {
    config.trev_path,
    "--emit",
    workspace,
  }

  -- Create a centered floating window.
  local ui = vim.api.nvim_list_uis()[1]
  local width = math.floor(ui.width * 0.6)
  local height = math.floor(ui.height * 0.7)
  local row = math.floor((ui.height - height) / 2)
  local col = math.floor((ui.width - width) / 2)

  local buf = vim.api.nvim_create_buf(false, true)
  local win = vim.api.nvim_open_win(buf, true, {
    relative = "editor",
    width = width,
    height = height,
    row = row,
    col = col,
    style = "minimal",
    border = "rounded",
  })

  vim.fn.termopen(cmd, {
    on_exit = function(_, _, _)
      vim.schedule(function()
        -- Read output from terminal buffer (selected file paths).
        local lines = {}
        if vim.api.nvim_buf_is_valid(buf) then
          lines = vim.api.nvim_buf_get_lines(buf, 0, -1, false)
        end

        -- Close the float window.
        if vim.api.nvim_win_is_valid(win) then
          vim.api.nvim_win_close(win, true)
        end

        -- Open selected files.
        for _, line in ipairs(lines) do
          local path = vim.trim(line)
          if path ~= "" and vim.fn.filereadable(path) == 1 then
            vim.cmd(action .. " " .. escape_path(path))
          end
        end
      end)
    end,
  })

  vim.cmd("startinsert")
end

---------------------------------------------------------------------------
-- Reveal
---------------------------------------------------------------------------

--- Reveal a file in the trev tree.
--- @param path string|nil file path (defaults to current buffer)
--- @param callback function|nil called with (ok: boolean)
function M.reveal(path, callback)
  path = path or vim.api.nvim_buf_get_name(0)
  if path == "" then
    return
  end
  -- Resolve to absolute path.
  path = vim.fn.fnamemodify(path, ":p")

  M._send_request("reveal", { path = path }, function(result, err)
    if callback then
      local ok = result and result.ok or false
      callback(ok)
    end
  end)
end

--- Auto-reveal: send notification on BufEnter.
--- @param path string
local function auto_reveal(path)
  if path == "" then
    return
  end
  path = vim.fn.fnamemodify(path, ":p")
  M._send_notification("reveal", { path = path })
end

--- Set up BufEnter autocmd for auto-reveal.
function M._setup_auto_reveal()
  if not config.auto_reveal then
    return
  end
  if augroup then
    return
  end

  augroup = vim.api.nvim_create_augroup("TrevAutoReveal", { clear = true })
  vim.api.nvim_create_autocmd("BufEnter", {
    group = augroup,
    callback = function(ev)
      if is_special_buffer(ev.buf) then
        return
      end
      -- Skip if entering the trev panel itself.
      if ev.buf == state.buf then
        return
      end
      local bufname = vim.api.nvim_buf_get_name(ev.buf)
      auto_reveal(bufname)
    end,
  })
end

--- Tear down auto-reveal autocmd.
function M._teardown_auto_reveal()
  if augroup then
    vim.api.nvim_del_augroup_by_id(augroup)
    augroup = nil
  end
end

---------------------------------------------------------------------------
-- Setup
---------------------------------------------------------------------------

--- Initialize the trev plugin.
--- @param opts trev.Config|nil
function M.setup(opts)
  config = vim.tbl_deep_extend("force", config, opts or {})

  -- User commands.
  vim.api.nvim_create_user_command("TrevToggle", function(cmd_opts)
    local args = vim.split(cmd_opts.args, "%s+", { trimempty = true })
    local mode = args[1] -- "float" or nil
    local action = args[2] -- editor action override
    M.toggle(mode, action)
  end, {
    nargs = "*",
    desc = "Toggle trev side panel or float picker",
  })

  vim.api.nvim_create_user_command("TrevOpen", function()
    M.open()
  end, {
    desc = "Open trev side panel",
  })

  vim.api.nvim_create_user_command("TrevClose", function()
    M.close()
  end, {
    desc = "Close trev side panel",
  })

  vim.api.nvim_create_user_command("TrevReveal", function(cmd_opts)
    local path = cmd_opts.args ~= "" and cmd_opts.args or nil
    M.reveal(path)
  end, {
    nargs = "?",
    desc = "Reveal file in trev tree",
    complete = "file",
  })
end

return M
