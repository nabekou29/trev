--- trev.nvim — Neovim integration for trev file viewer.
--- Provides side panel, float picker, reveal, and external command handling
--- via JSON-RPC 2.0 over Unix Domain Socket.
local M = {}

---------------------------------------------------------------------------
-- Types
---------------------------------------------------------------------------

--- @class trev.Adapter
--- @field open_panel fun(self, cmd: string[], opts: trev.AdapterOpts): trev.AdapterHandle|nil
--- @field open_float fun(self, cmd: string[], opts: trev.AdapterOpts): trev.AdapterHandle|nil
--- @field close fun(self, handle: trev.AdapterHandle)
--- @field is_visible fun(self, handle: trev.AdapterHandle): boolean
--- @field focus fun(self, handle: trev.AdapterHandle)

--- @class trev.AdapterOpts
--- @field width? number
--- @field on_exit fun(exit_code: number)
--- @field on_ready fun(handle: trev.AdapterHandle)

--- @class trev.AdapterHandle
--- @field buf number|nil
--- @field win number|nil
--- @field pid number|nil
--- @field job_id number|nil

--- @class trev.Config
--- @field trev_path string Path to trev binary
--- @field width number Side panel width (columns)
--- @field auto_reveal boolean Auto-reveal on BufEnter
--- @field action string Default editor action (edit/split/vsplit/tabedit)
--- @field adapter "auto"|"native"|"snacks"|"toggleterm"|"tmux"|"zellij" Terminal backend
--- @field handlers table<string, function> Notification handlers

--- @type trev.Config
local config = {
    trev_path = "trev",
    width = 30,
    auto_reveal = true,
    action = "edit",
    adapter = "auto",
    handlers = {},
}

---------------------------------------------------------------------------
-- State
---------------------------------------------------------------------------

--- @class trev.State
--- @field handle trev.AdapterHandle|nil adapter handle
--- @field mode string|nil "panel" | "float" | nil
--- @field prev_win number|nil window before float opened
--- @field pipe userdata|nil vim.uv pipe handle
--- @field socket_path string|nil path to UDS socket
--- @field read_buf string partial read buffer for line framing
--- @field request_id number next JSON-RPC request ID
--- @field pending table<number, function> pending request callbacks

--- @type trev.State
local state = {
    handle = nil,
    mode = nil,
    prev_win = nil,
    pipe = nil,
    socket_path = nil,
    read_buf = "",
    request_id = 1,
    pending = {},
}

--- @type trev.Adapter|nil
local adapter = nil

--- @type number|nil
local augroup = nil

---------------------------------------------------------------------------
-- State helpers
---------------------------------------------------------------------------

--- Get the current buffer number from the adapter handle.
--- @return number|nil
local function get_buf()
    return state.handle and state.handle.buf
end

--- Get the current window number from the adapter handle.
--- @return number|nil
local function get_win()
    return state.handle and state.handle.win
end

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
    local had_pipe = state.pipe ~= nil

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

    -- For external panes (tmux split, Zellij): pipe EOF means trev exited.
    -- Trigger cleanup since there is no on_exit callback from a Neovim job.
    -- Only fires when: had an active pipe, handle exists, no Neovim job_id,
    -- and no tmux popup job (_popup_job_id has its own on_exit callback).
    if had_pipe and state.handle and not state.handle.job_id and not state.handle._popup_job_id then
        vim.schedule(function()
            if state.handle then
                M._on_daemon_exit(0)
            end
        end)
    end
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

---------------------------------------------------------------------------
-- Message handling
---------------------------------------------------------------------------

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
    elseif method == "close" then
        M._handle_close()
    elseif method == "external_command" then
        M._handle_external_command(params)
    elseif config.handlers[method] then
        config.handlers[method](params)
    end
end

--- Close the trev instance and restore focus to the previous window.
function M._close_and_restore()
    local prev_win = state.prev_win
    M._close_instance()
    if prev_win and vim.api.nvim_win_is_valid(prev_win) then
        vim.api.nvim_set_current_win(prev_win)
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

    if state.mode == "float" then
        M._close_and_restore()
    else
        -- Panel mode: focus the editor window (not the trev panel).
        local target_win = M._find_editor_window()
        if target_win then
            vim.api.nvim_set_current_win(target_win)
        end
        -- For tmux: switch tmux focus back to Neovim's pane.
        if adapter and adapter.focus_editor then
            adapter:focus_editor()
        end
    end

    vim.cmd(action .. " " .. escape_path(path))
end

--- Handle close notification: close the trev panel or float.
function M._handle_close()
    M._close_and_restore()
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

--- Get the absolute path of the current editor buffer (or nil).
--- @return string|nil
function M._current_editor_path()
    local bufnr = vim.api.nvim_get_current_buf()
    if is_special_buffer(bufnr) then
        return nil
    end
    local name = vim.api.nvim_buf_get_name(bufnr)
    if name == "" then
        return nil
    end
    return vim.fn.fnamemodify(name, ":p")
end

--- Find an editor window (not the trev side panel).
--- @return number|nil window ID
function M._find_editor_window()
    local trev_win = get_win()
    local current = vim.api.nvim_get_current_win()
    if current ~= trev_win then
        return current
    end
    -- Find the first non-trev window.
    for _, win in ipairs(vim.api.nvim_tabpage_list_wins(0)) do
        if win ~= trev_win then
            return win
        end
    end
    return nil
end

---------------------------------------------------------------------------
-- Socket discovery
---------------------------------------------------------------------------

--- Get the trev runtime directory (matching Rust's runtime_dir()).
--- @return string
local function get_trev_runtime_dir()
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
    return base .. "/trev"
end

--- Build the expected socket path for a given pid.
--- @param pid number
--- @return string|nil
local function find_socket_for_pid(pid)
    local trev_dir = get_trev_runtime_dir()
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

--- Compute the workspace key matching Rust's workspace_key() function.
--- @param path string
--- @return string
local function compute_workspace_key(path)
    local dir_name = vim.fn.fnamemodify(path, ":t")
    if dir_name == "" then
        dir_name = "trev"
    end
    local canonical = vim.fn.resolve(path)
    local hash = vim.fn.sha256(canonical):sub(1, 8)
    return dir_name .. "-" .. hash
end

--- Find a socket by workspace key (single check, no retry).
--- @param workspace_key string
--- @return string|nil
local function find_socket_for_workspace(workspace_key)
    local trev_dir = get_trev_runtime_dir()
    local pattern = trev_dir .. "/" .. workspace_key .. "-*.sock"
    local matches = vim.fn.glob(pattern, false, true)
    if #matches > 0 then
        return matches[1]
    end
    return nil
end

---------------------------------------------------------------------------
-- Command builder & IPC connect
---------------------------------------------------------------------------

--- Build the trev daemon command array.
--- @param reveal string|nil file path to reveal on startup
--- @return string[]
local function build_cmd(reveal)
    local cmd = {
        config.trev_path,
        "--daemon",
        vim.fn.getcwd(),
    }
    if reveal and reveal ~= "" then
        table.insert(cmd, "--reveal")
        table.insert(cmd, reveal)
    end
    return cmd
end

--- Connect IPC after a short delay (daemon needs time to bind socket).
--- @param pid number|nil daemon process ID (nil for workspace-key discovery)
--- @param mode string "panel" | "float"
local function connect_ipc(pid, mode)
    local on_connect = function()
        if mode == "panel" then
            M._setup_auto_reveal()
        end
    end

    if pid then
        -- PID-based discovery: short delay + built-in retry.
        vim.defer_fn(function()
            local socket = find_socket_for_pid(pid)
            if socket then
                ipc_connect(socket, on_connect)
            else
                vim.notify("[trev] socket not found for pid " .. pid, vim.log.levels.WARN)
            end
        end, 300)
    else
        -- Workspace-key-based discovery (e.g., tmux popup where PID is unknown).
        -- Non-blocking retry loop: trev in tmux popup may take longer to start.
        local ws_key = compute_workspace_key(vim.fn.getcwd())
        local attempts = 0
        local max_attempts = 20
        local interval = 250

        local function try_connect()
            attempts = attempts + 1
            local socket = find_socket_for_workspace(ws_key)
            if socket then
                ipc_connect(socket, on_connect)
            elseif attempts < max_attempts then
                vim.defer_fn(try_connect, interval)
            else
                vim.notify("[trev] socket not found for workspace", vim.log.levels.WARN)
            end
        end

        vim.defer_fn(try_connect, 500)
    end
end

---------------------------------------------------------------------------
-- Adapter callback helpers
---------------------------------------------------------------------------

--- Create the on_exit callback for the adapter.
--- @return fun(exit_code: number)
local function make_on_exit()
    return function(exit_code)
        M._on_daemon_exit(exit_code)
    end
end

--- Create the on_ready callback for the adapter.
--- @param mode string "panel" | "float"
--- @return fun(handle: trev.AdapterHandle)
local function make_on_ready(mode)
    return function(handle)
        state.handle = handle
        -- pid may be nil (e.g., tmux popup); connect_ipc handles both cases.
        connect_ipc(handle.pid, mode)
    end
end

---------------------------------------------------------------------------
-- Side panel
---------------------------------------------------------------------------

--- Open the trev side panel.
function M.open()
    if state.handle and adapter and adapter:is_visible(state.handle) then
        adapter:focus(state.handle)
        return
    end

    local reveal = M._current_editor_path()
    local cmd = build_cmd(reveal)

    state.mode = "panel"
    state.handle = adapter:open_panel(cmd, {
        width = config.width,
        on_exit = make_on_exit(),
        on_ready = make_on_ready("panel"),
    })
end

--- Close the trev instance (panel or float).
function M.close()
    M._close_instance()
end

--- Internal close: teardown IPC, stop job, close window, reset state.
function M._close_instance()
    M._teardown_auto_reveal()

    if state.pipe then
        -- Ask trev to quit gracefully (saves session before exiting).
        M._send_request("quit", nil, nil)
        M._disconnect()
    -- Don't jobstop — trev will exit on its own after saving.
    elseif state.handle and state.handle.job_id then
        -- No IPC connection — force kill.
        vim.fn.jobstop(state.handle.job_id)
    end

    -- Close window immediately for responsive UX.
    if state.handle and adapter then
        adapter:close(state.handle)
    end

    state.handle = nil
    state.mode = nil
    state.prev_win = nil
end

--- Toggle the side panel.
--- @param mode string|nil "float" for float picker, nil for side panel
function M.toggle(mode)
    if mode == "float" then
        M.float_pick()
        return
    end

    if state.handle and adapter and adapter:is_visible(state.handle) then
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

    if state.handle and adapter then
        adapter:close(state.handle)
    end

    state.handle = nil
    state.mode = nil
    state.prev_win = nil

    if exit_code ~= 0 then
        vim.notify("[trev] daemon exited with code " .. exit_code, vim.log.levels.WARN)
    end
end

---------------------------------------------------------------------------
-- Float picker
---------------------------------------------------------------------------

--- Open a float picker for quick file selection.
function M.float_pick()
    -- Close existing instance if any.
    if state.handle and adapter and adapter:is_visible(state.handle) then
        M.close()
    end

    local reveal = M._current_editor_path()
    state.prev_win = vim.api.nvim_get_current_win()

    local cmd = build_cmd(reveal)

    state.mode = "float"
    state.handle = adapter:open_float(cmd, {
        on_exit = make_on_exit(),
        on_ready = make_on_ready("float"),
    })
end

---------------------------------------------------------------------------
-- Public API for custom handlers
---------------------------------------------------------------------------

--- Close the float window if in float mode.
--- Returns the previous window ID (or nil).
--- No-op if not in float mode.
--- @return number|nil prev_win
function M.close_float()
    if state.mode ~= "float" then
        return nil
    end
    local prev_win = state.prev_win
    M._close_instance()
    return prev_win
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

    M._send_request("reveal", { path = path }, function(result, _)
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
            -- Skip if entering the trev buffer itself.
            if ev.buf == get_buf() then
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
-- Adapter resolution
---------------------------------------------------------------------------

--- Resolve and instantiate the terminal adapter.
local function resolve_adapter()
    local choice = config.adapter

    if choice == "auto" then
        local has_snacks, snacks = pcall(require, "snacks")
        if has_snacks and snacks and snacks.terminal then
            choice = "snacks"
        elseif pcall(require, "toggleterm") then
            choice = "toggleterm"
        else
            choice = "native"
        end
    end

    if choice == "zellij" then
        if not vim.env.ZELLIJ or vim.env.ZELLIJ == "" then
            vim.notify("[trev] adapter 'zellij' requires running inside Zellij", vim.log.levels.ERROR)
            adapter = require("trev.adapter.native").new()
        else
            adapter = require("trev.adapter.zellij").new()
        end
    elseif choice == "tmux" then
        if not vim.env.TMUX or vim.env.TMUX == "" then
            vim.notify("[trev] adapter 'tmux' requires running inside tmux", vim.log.levels.ERROR)
            adapter = require("trev.adapter.native").new()
        else
            adapter = require("trev.adapter.tmux").new()
        end
    elseif choice == "snacks" then
        adapter = require("trev.adapter.snacks").new()
    elseif choice == "toggleterm" then
        adapter = require("trev.adapter.toggleterm").new()
    else
        adapter = require("trev.adapter.native").new()
    end
end

---------------------------------------------------------------------------
-- Setup
---------------------------------------------------------------------------

--- Initialize the trev plugin.
--- @param opts trev.Config|nil
function M.setup(opts)
    config = vim.tbl_deep_extend("force", config, opts or {})

    resolve_adapter()

    -- User commands.
    vim.api.nvim_create_user_command("TrevToggle", function(cmd_opts)
        local args = vim.split(cmd_opts.args, "%s+", { trimempty = true })
        local mode = args[1] -- "float" or nil
        M.toggle(mode)
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
