--- Zellij terminal adapter: runs trev in Zellij panes.
--- @class trev.ZellijAdapter : trev.Adapter
local M = {}
M.__index = M

--- Create a new Zellij adapter instance.
--- @return trev.ZellijAdapter
function M.new()
    if not vim.env.ZELLIJ or vim.env.ZELLIJ == "" then
        error("[trev] zellij adapter requires running inside Zellij (ZELLIJ not set)")
    end
    return setmetatable({}, M)
end

--- Open a side panel via zellij run --direction left.
--- @param cmd string[]
--- @param opts trev.AdapterOpts
--- @return trev.AdapterHandle|nil
function M:open_panel(cmd, opts)
    -- --direction left: create pane on the left side
    -- --close-on-exit: auto-close when trev exits
    -- --name: identify the pane
    -- Note: --width is floating-only in Zellij, so panel uses default split width.
    local zellij_cmd = {
        'zellij', 'run',
        '--direction', 'left',
        '--close-on-exit',
        '--name', 'trev',
        '--',
    }
    for _, arg in ipairs(cmd) do
        table.insert(zellij_cmd, arg)
    end

    local output = vim.fn.system(zellij_cmd)

    if vim.v.shell_error ~= 0 then
        vim.notify("[trev] zellij run failed: " .. output, vim.log.levels.ERROR)
        return nil
    end

    -- zellij run moves focus to the new pane; move back to Neovim.
    vim.fn.system({ 'zellij', 'action', 'move-focus', 'right' })

    --- @type trev.AdapterHandle
    local handle = {
        buf = nil,
        win = nil,
        pid = nil,
        job_id = nil,
        is_popup = false,
        _closed = false,
    }

    if opts.on_ready then
        opts.on_ready(handle)
    end

    return handle
end

--- Open a float via zellij run --floating.
--- @param cmd string[]
--- @param opts trev.AdapterOpts
--- @return trev.AdapterHandle|nil
function M:open_float(cmd, opts)
    -- --floating: create a floating pane
    -- --close-on-exit: auto-close when trev exits
    -- --width/--height: size of the floating pane
    local zellij_cmd = {
        'zellij', 'run',
        '--floating',
        '--close-on-exit',
        '--name', 'trev',
        '--width', '60%',
        '--height', '70%',
        '--',
    }
    for _, arg in ipairs(cmd) do
        table.insert(zellij_cmd, arg)
    end

    local output = vim.fn.system(zellij_cmd)

    if vim.v.shell_error ~= 0 then
        vim.notify("[trev] zellij run (floating) failed: " .. output, vim.log.levels.ERROR)
        return nil
    end

    --- @type trev.AdapterHandle
    local handle = {
        buf = nil,
        win = nil,
        pid = nil,
        job_id = nil,
        is_popup = true,
        _closed = false,
    }

    if opts.on_ready then
        opts.on_ready(handle)
    end

    return handle
end

--- Close the Zellij pane.
--- Pane auto-closes via --close-on-exit when trev exits.
--- _close_instance() sends IPC quit first, triggering trev exit.
--- @param handle trev.AdapterHandle
function M:close(handle)
    if not handle or handle._closed then
        return
    end
    handle._closed = true
end

--- Check if the Zellij pane is currently visible.
--- Zellij CLI has no way to query a specific pane's visibility,
--- so we assume visible unless explicitly closed.
--- @param handle trev.AdapterHandle
--- @return boolean
function M:is_visible(handle)
    if not handle or handle._closed then
        return false
    end
    return true
end

--- Focus the Zellij pane (no-op for floating panes).
--- @param handle trev.AdapterHandle
function M:focus(handle)
    if not handle or handle._closed or handle.is_popup then
        return
    end
    vim.fn.system({ 'zellij', 'action', 'move-focus', 'left' })
end

--- Switch Zellij focus back to Neovim's pane.
function M:focus_editor()
    vim.fn.system({ 'zellij', 'action', 'move-focus', 'right' })
end

return M
