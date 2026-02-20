--- toggleterm.nvim terminal adapter.
--- Delegates terminal and window management to toggleterm.terminal.Terminal.
--- @class trev.ToggletrmAdapter : trev.Adapter
local M = {}
M.__index = M

local Terminal = require("toggleterm.terminal").Terminal

--- Create a new toggleterm adapter instance.
--- @return trev.ToggletrmAdapter
function M.new()
    return setmetatable({ _term = nil }, M)
end

--- Open a side panel via toggleterm.
--- @param cmd string[]
--- @param opts trev.AdapterOpts
--- @return trev.AdapterHandle|nil
function M:open_panel(cmd, opts)
    return self:_open(cmd, opts, {
        direction = "vertical",
        size = opts.width or 30,
    })
end

--- Open a float via toggleterm.
--- @param cmd string[]
--- @param opts trev.AdapterOpts
--- @return trev.AdapterHandle|nil
function M:open_float(cmd, opts)
    local ui = vim.api.nvim_list_uis()[1]
    return self:_open(cmd, opts, {
        direction = "float",
        float_opts = {
            border = "rounded",
            width = math.floor(ui.width * 0.6),
            height = math.floor(ui.height * 0.7),
        },
    })
end

--- Common open logic.
--- @private
--- @param cmd string[]
--- @param opts trev.AdapterOpts
--- @param term_opts table toggleterm.Terminal options
--- @return trev.AdapterHandle|nil
function M:_open(cmd, opts, term_opts)
    --- @type trev.AdapterHandle
    local handle = { buf = nil, win = nil, pid = nil, job_id = nil }

    local term = Terminal:new(vim.tbl_deep_extend("force", term_opts, {
        cmd = table.concat(cmd, " "),
        hidden = true,
        close_on_exit = false,
        start_in_insert = true,
        on_create = function(t)
            vim.bo[t.bufnr].filetype = "trev"

            -- Auto-enter terminal mode when re-focusing the buffer.
            vim.api.nvim_create_autocmd("BufEnter", {
                buffer = t.bufnr,
                callback = function()
                    if vim.api.nvim_get_mode().mode == "nt" then
                        vim.cmd("startinsert")
                    end
                end,
            })
        end,
        on_open = function(t)
            handle.buf = t.bufnr
            handle.win = t.window
            handle.job_id = t.job_id
            if t.job_id and t.job_id > 0 then
                handle.pid = vim.fn.jobpid(t.job_id)
            end
            if opts.on_ready then
                opts.on_ready(handle)
            end
        end,
        on_close = function(_)
            handle.win = nil
        end,
        on_exit = function(_, _, exit_code, _)
            if opts.on_exit then
                vim.schedule(function()
                    opts.on_exit(exit_code)
                end)
            end
        end,
    }))

    self._term = term
    term:open()

    return handle
end

--- Close the terminal window and clean up.
--- @param handle trev.AdapterHandle
function M:close(handle)
    if not handle then
        return
    end
    if self._term then
        self._term:shutdown()
        self._term = nil
    end
    handle.win = nil
    handle.buf = nil
end

--- Check if the terminal window is currently visible.
--- @param handle trev.AdapterHandle
--- @return boolean
function M:is_visible(handle)
    if not handle or not self._term then
        return false
    end
    return self._term:is_open()
end

--- Focus the terminal window.
--- @param handle trev.AdapterHandle
function M:focus(handle)
    if handle and self._term and self._term:is_open() then
        self._term:focus()
    end
end

return M
