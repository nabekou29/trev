--- Snacks.nvim terminal adapter.
--- Delegates terminal and window management to Snacks.terminal.
--- @class trev.SnacksAdapter : trev.Adapter
local M = {}
M.__index = M

--- Create a new snacks adapter instance.
--- @return trev.SnacksAdapter
function M.new()
    return setmetatable({}, M)
end

--- Build common snacks.terminal options.
--- @private
--- @param cmd string[]
--- @param opts trev.AdapterOpts
--- @param win_opts table snacks.win.Config overrides
--- @return table snacks.terminal.Opts
function M:_build_snacks_opts(cmd, opts, win_opts)
    local handle = { buf = nil, win = nil, pid = nil, job_id = nil, _snacks_term = nil }

    local snacks_opts = {
        auto_insert = true,
        auto_close = false, -- We manage lifecycle via IPC quit.
        start_insert = true,
        win = vim.tbl_deep_extend("force", {
            bo = { filetype = "trev" },
            on_buf = function(self)
                handle.buf = self.buf
            end,
            on_win = function(self)
                handle.win = self.win
            end,
            on_close = function(_)
                handle.win = nil
            end,
        }, win_opts),
    }

    -- Deferred: extract pid/job_id after terminal starts, then call on_ready.
    handle._on_buf_deferred = function(term)
        vim.defer_fn(function()
            if handle.buf and vim.api.nvim_buf_is_valid(handle.buf) then
                handle.job_id = vim.b[handle.buf].terminal_job_id
                if handle.job_id and handle.job_id > 0 then
                    handle.pid = vim.fn.jobpid(handle.job_id)
                end
            end
            if opts.on_ready then
                opts.on_ready(handle)
            end
        end, 50)

        -- Wire up process exit.
        if term and term.buf and vim.api.nvim_buf_is_valid(term.buf) then
            vim.api.nvim_create_autocmd("TermClose", {
                buffer = term.buf,
                once = true,
                callback = function()
                    local exit_code = (vim.v.event and vim.v.event.status) or 0
                    if opts.on_exit then
                        vim.schedule(function()
                            opts.on_exit(exit_code)
                        end)
                    end
                end,
            })
        end
    end

    return snacks_opts, handle
end

--- Open a side panel via Snacks.terminal.
--- @param cmd string[]
--- @param opts trev.AdapterOpts
--- @return trev.AdapterHandle|nil
function M:open_panel(cmd, opts)
    local snacks_opts, handle = self:_build_snacks_opts(cmd, opts, {
        position = "left",
        width = opts.width or 30,
        wo = {
            number = false,
            relativenumber = false,
            signcolumn = "no",
            foldcolumn = "0",
            winfixwidth = true,
        },
    })

    local term = Snacks.terminal.open(cmd, snacks_opts)
    handle._snacks_term = term

    if handle._on_buf_deferred then
        handle._on_buf_deferred(term)
        handle._on_buf_deferred = nil
    end

    return handle
end

--- Open a float via Snacks.terminal.
--- @param cmd string[]
--- @param opts trev.AdapterOpts
--- @return trev.AdapterHandle|nil
function M:open_float(cmd, opts)
    local snacks_opts, handle = self:_build_snacks_opts(cmd, opts, {
        position = "float",
        width = 0.6,
        height = 0.7,
        border = "rounded",
    })

    local term = Snacks.terminal.open(cmd, snacks_opts)
    handle._snacks_term = term

    if handle._on_buf_deferred then
        handle._on_buf_deferred(term)
        handle._on_buf_deferred = nil
    end

    return handle
end

--- Close the terminal.
--- @param handle trev.AdapterHandle
function M:close(handle)
    if not handle then
        return
    end
    local term = handle._snacks_term
    if term then
        term:close()
    end
    handle.win = nil
    handle.buf = nil
    handle._snacks_term = nil
end

--- Check if the terminal window is currently visible.
--- @param handle trev.AdapterHandle
--- @return boolean
function M:is_visible(handle)
    if not handle or not handle._snacks_term then
        return false
    end
    return handle._snacks_term:win_valid()
end

--- Focus the terminal window.
--- @param handle trev.AdapterHandle
function M:focus(handle)
    if not handle or not handle._snacks_term then
        return
    end
    if handle._snacks_term:win_valid() then
        vim.api.nvim_set_current_win(handle._snacks_term.win)
    end
end

return M
