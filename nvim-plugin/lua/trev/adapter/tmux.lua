--- Tmux terminal adapter: runs trev in tmux panes/popups.
--- @class trev.TmuxAdapter : trev.Adapter
--- @field _neovim_pane_id string tmux pane ID where Neovim runs
local M = {}
M.__index = M

--- Create a new tmux adapter instance.
--- @return trev.TmuxAdapter
function M.new()
    local pane_id = vim.env.TMUX_PANE
    if not pane_id or pane_id == "" then
        error("[trev] tmux adapter requires running inside tmux (TMUX_PANE not set)")
    end
    return setmetatable({ _neovim_pane_id = pane_id }, M)
end

--- Build a shell-safe command string from a command table.
--- @param cmd string[]
--- @return string
local function build_cmd_str(cmd)
    local parts = {}
    for _, arg in ipairs(cmd) do
        table.insert(parts, vim.fn.shellescape(arg))
    end
    return table.concat(parts, " ")
end

--- Open a side panel via tmux split-window.
--- @param cmd string[]
--- @param opts trev.AdapterOpts
--- @return trev.AdapterHandle|nil
function M:open_panel(cmd, opts)
    local width = opts.width or 30

    -- Use table form to avoid shell quoting issues.
    -- -h: horizontal split (side-by-side panes)
    -- -b: place new pane before (left side)
    -- -l: width in columns
    -- -P -F: print pane info (pane_id and pid)
    local tmux_cmd = {
        'tmux', 'split-window',
        '-h', '-b',
        '-l', tostring(width),
        '-P', '-F', '#{pane_id} #{pane_pid}',
    }
    for _, arg in ipairs(cmd) do
        table.insert(tmux_cmd, arg)
    end

    local output = vim.fn.system(tmux_cmd)

    if vim.v.shell_error ~= 0 then
        vim.notify("[trev] tmux split-window failed: " .. output, vim.log.levels.ERROR)
        return nil
    end

    local pane_id, pid_str = output:match("^(%%[%d]+)%s+(%d+)")
    local pid = pid_str and tonumber(pid_str) or nil

    -- Return focus to Neovim pane (split-window moves focus to new pane).
    vim.fn.system({ 'tmux', 'select-pane', '-t', self._neovim_pane_id })

    --- @type trev.AdapterHandle
    local handle = {
        buf = nil,
        win = nil,
        pid = pid,
        job_id = nil,
        pane_id = pane_id,
        is_popup = false,
        _closed = false,
    }

    if opts.on_ready then
        opts.on_ready(handle)
    end

    return handle
end

--- Open a float via tmux display-popup.
--- @param cmd string[]
--- @param opts trev.AdapterOpts
--- @return trev.AdapterHandle|nil
function M:open_float(cmd, opts)
    -- Build shell-safe command string for tmux's internal shell execution.
    local cmd_str = build_cmd_str(cmd)

    -- Use table form for jobstart to avoid double shell interpretation.
    -- Pass the shell-command as a single arg (tmux runs it through $SHELL).
    -- -E: close popup when command exits
    -- -w/-h: 60%/70% of terminal
    local popup_job_id = vim.fn.jobstart(
        { 'tmux', 'display-popup', '-E', '-w', '60%', '-h', '70%', cmd_str },
        {
            on_exit = function(_, exit_code, _)
                if opts.on_exit then
                    vim.schedule(function()
                        opts.on_exit(exit_code)
                    end)
                end
            end,
        }
    )

    if popup_job_id <= 0 then
        vim.notify("[trev] tmux display-popup failed", vim.log.levels.ERROR)
        return nil
    end

    --- @type trev.AdapterHandle
    local handle = {
        buf = nil,
        win = nil,
        pid = nil,
        job_id = nil,
        pane_id = nil,
        is_popup = true,
        _popup_job_id = popup_job_id,
        _closed = false,
    }

    if opts.on_ready then
        opts.on_ready(handle)
    end

    return handle
end

--- Close the tmux pane or popup.
--- @param handle trev.AdapterHandle
function M:close(handle)
    if not handle or handle._closed then
        return
    end
    handle._closed = true

    if handle.is_popup then
        if handle._popup_job_id then
            pcall(vim.fn.jobstop, handle._popup_job_id)
            handle._popup_job_id = nil
        end
    else
        if handle.pane_id then
            vim.fn.system({ 'tmux', 'kill-pane', '-t', handle.pane_id })
            handle.pane_id = nil
        end
    end
end

--- Check if the tmux pane is currently visible.
--- @param handle trev.AdapterHandle
--- @return boolean
function M:is_visible(handle)
    if not handle or handle._closed then
        return false
    end

    if handle.is_popup then
        return handle._popup_job_id ~= nil
    end

    if not handle.pane_id then
        return false
    end
    vim.fn.system({ 'tmux', 'display-message', '-t', handle.pane_id, '-p', '#{pane_id}' })
    return vim.v.shell_error == 0
end

--- Focus the tmux pane (no-op for popups).
--- @param handle trev.AdapterHandle
function M:focus(handle)
    if not handle or handle._closed or handle.is_popup then
        return
    end
    if handle.pane_id then
        vim.fn.system({ 'tmux', 'select-pane', '-t', handle.pane_id })
    end
end

--- Switch tmux focus back to Neovim's pane.
function M:focus_editor()
    vim.fn.system({ 'tmux', 'select-pane', '-t', self._neovim_pane_id })
end

return M
