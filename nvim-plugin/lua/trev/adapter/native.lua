--- Native terminal adapter using termopen + manual window management.
--- @class trev.NativeAdapter : trev.Adapter
local M = {}
M.__index = M

--- Create a new native adapter instance.
--- @return trev.NativeAdapter
function M.new()
    return setmetatable({}, M)
end

--- Open a side panel with a terminal.
--- @param cmd string[]
--- @param opts trev.AdapterOpts
--- @return trev.AdapterHandle|nil
function M:open_panel(cmd, opts)
    vim.cmd("topleft " .. (opts.width or 30) .. "vsplit")
    local win = vim.api.nvim_get_current_win()
    local buf = vim.api.nvim_create_buf(false, true)
    vim.api.nvim_win_set_buf(win, buf)

    -- Disable decorations in the panel.
    vim.wo[win].number = false
    vim.wo[win].relativenumber = false
    vim.wo[win].signcolumn = "no"
    vim.wo[win].foldcolumn = "0"
    vim.wo[win].winfixwidth = true

    return self:_start_terminal(cmd, win, opts)
end

--- Open a float with a terminal.
--- @param cmd string[]
--- @param opts trev.AdapterOpts
--- @return trev.AdapterHandle|nil
function M:open_float(cmd, opts)
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

    return self:_start_terminal(cmd, win, opts)
end

--- Start a terminal in the given window.
--- @private
--- @param cmd string[]
--- @param win number
--- @param opts trev.AdapterOpts
--- @return trev.AdapterHandle|nil
function M:_start_terminal(cmd, win, opts)
    local job_id = vim.fn.termopen(cmd, {
        on_exit = function(_, exit_code, _)
            if opts.on_exit then
                vim.schedule(function()
                    opts.on_exit(exit_code)
                end)
            end
        end,
    })

    if job_id <= 0 then
        vim.notify("[trev] failed to start trev daemon", vim.log.levels.ERROR)
        vim.api.nvim_win_close(win, true)
        return nil
    end

    local buf = vim.api.nvim_get_current_buf()
    local pid = vim.fn.jobpid(job_id)

    -- Set custom filetype for identification.
    vim.bo[buf].filetype = "trev"

    -- Enter terminal mode.
    vim.cmd("startinsert")

    -- Auto-enter terminal mode when re-focusing the buffer.
    vim.api.nvim_create_autocmd("BufEnter", {
        buffer = buf,
        callback = function()
            if vim.api.nvim_get_mode().mode == "nt" then
                vim.cmd("startinsert")
            end
        end,
    })

    --- @type trev.AdapterHandle
    local handle = {
        buf = buf,
        win = win,
        pid = pid,
        job_id = job_id,
    }

    if opts.on_ready then
        opts.on_ready(handle)
    end

    return handle
end

--- Close the terminal window and clean up the buffer.
--- @param handle trev.AdapterHandle
function M:close(handle)
    if not handle then
        return
    end
    if handle.win and vim.api.nvim_win_is_valid(handle.win) then
        vim.api.nvim_win_close(handle.win, true)
    end
    if handle.buf and vim.api.nvim_buf_is_valid(handle.buf) then
        vim.api.nvim_buf_delete(handle.buf, { force = true })
    end
    handle.win = nil
    handle.buf = nil
end

--- Check if the terminal window is currently visible.
--- @param handle trev.AdapterHandle
--- @return boolean
function M:is_visible(handle)
    return handle ~= nil and handle.win ~= nil and vim.api.nvim_win_is_valid(handle.win)
end

--- Focus the terminal window.
--- @param handle trev.AdapterHandle
function M:focus(handle)
    if handle and handle.win and vim.api.nvim_win_is_valid(handle.win) then
        vim.api.nvim_set_current_win(handle.win)
    end
end

return M
