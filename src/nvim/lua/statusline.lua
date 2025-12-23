-- Custom statusline configuration with command feedback

-- Function to get the mode indicator (always shows V)
local function get_mode()
    return 'V'
end

-- Function to get mode highlight group
local function get_mode_hl()
    return 'StatusLineNormal'  -- Always use the yellow/gold background
end

-- Function to get the current mode name
local function get_mode_name()
    local mode_map = {
        n = 'NORMAL',
        i = 'INSERT',
        v = 'VISUAL',
        V = 'V-LINE',
        ['\22'] = 'V-BLOCK',
        c = 'COMMAND',
        s = 'SELECT',
        S = 'S-LINE',
        ['\19'] = 'S-BLOCK',
        R = 'REPLACE',
        r = 'REPLACE',
        ['!'] = 'SHELL',
        t = 'TERMINAL'
    }
    local mode = vim.api.nvim_get_mode().mode
    return mode_map[mode] or mode:upper()
end

-- Build the statusline
function _G.custom_statusline()
    local mode = get_mode()
    local mode_hl = get_mode_hl()
    
    -- Get just the filename (not the full path)
    local filename = vim.fn.expand('%:t')
    if filename == '' then
        filename = '[No Name]'
    end
    
    -- Check if file is modified
    local modified = vim.bo.modified and ' [+]' or ''
    
    -- Get file saved status with highlight
    local saved_status = ''
    if vim.g.statusline_save_msg then
        saved_status = string.format('%%#StatusLineSaved# %s %%*', vim.g.statusline_save_msg)
        -- Clear the message after showing it
        vim.defer_fn(function()
            vim.g.statusline_save_msg = nil
            vim.cmd('redrawstatus')
        end, 2000)
    end
    
    -- Get current mode name
    local mode_name = get_mode_name()
    
    -- Left side: mode indicator and filename
    local left = string.format('%%#%s# %s %%* %s%s', mode_hl, mode, filename, modified)
    
    -- Right side: saved status, mode name, percentage, column, Help
    local right = string.format('%s %s %%p%%%% %%c Help', saved_status, mode_name)
    
    -- Combine with spacing
    return left .. '%=' .. right
end

-- Define highlight groups - V box always uses gold/yellow
vim.api.nvim_set_hl(0, 'StatusLineNormal', { fg = '#191724', bg = '#f6c177', bold = true })

-- Define highlight for saved status - Rosé Pine purple/iris
vim.api.nvim_set_hl(0, 'StatusLineSaved', { fg = '#191724', bg = '#c4a7e7', bold = true })

-- Set the main statusline background
vim.api.nvim_set_hl(0, 'StatusLine', { fg = '#e0def4', bg = '#191724' })
vim.api.nvim_set_hl(0, 'StatusLineNC', { fg = '#6e6a86', bg = '#191724' })

-- Set the custom statusline
vim.opt.statusline = '%!v:lua.custom_statusline()'

-- Enable statusline
vim.opt.laststatus = 2

-- Add autocmd to update statusline on save
vim.api.nvim_create_autocmd('BufWritePost', {
    callback = function()
        vim.g.statusline_save_msg = 'written'
        vim.cmd('redrawstatus')
    end
})

-- Update statusline when mode changes
vim.api.nvim_create_autocmd('ModeChanged', {
    callback = function()
        vim.cmd('redrawstatus')
    end
})
