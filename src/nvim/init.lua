-- set leader keys before anything else
vim.g.mapleader = " "
vim.g.maplocalleader = "\\"

-- --- config:load
-- highlight current line number
vim.api.nvim_set_hl(0, 'CursorLineNr', {
  fg = '#f6c177',  -- Rosé Pine gold (yellow-orange)
  bold = true
})

-- load statusline configuration
require('statusline')

-- load neovim options
require('options')

-- load keymaps for editor
require('keymaps')


-- load telescope find & replace configuration
-- require('telescope-replace-config').setup()

-- load vim autoclose for closing quotes and paretheses
require('autoclose').setup()

-- --- config:plugins ---
-- bootstrap lazy.nvim
local lazypath = vim.fn.stdpath("data") .. "/lazy/lazy.nvim"
if not vim.loop.fs_stat(lazypath) then
    vim.fn.system({ "git", "clone", "--filter=blob:none", "https://github.com/folke/lazy.nvim.git", "--branch=stable",
        lazypath })
end
vim.opt.rtp:prepend(lazypath)

-- setup plugins via lazy.nvim
require('lazy').setup('plugins', {
    change_detection = { enabled = true, notify = false },
})

-- load treesitter configuration
require('treesitter-config')

-- load completion configuration (only if nvim-cmp is installed)
local cmp_ok, _ = pcall(require, 'cmp')
if cmp_ok then
    require('completion-config')
end

vim.api.nvim_set_hl(0, "Normal", { bg = "none" })
vim.api.nvim_set_hl(0, "NormalFloat", { bg = "none" })

-- Fallback for Crystal syntax highlighting if Tree-sitter is still failing
vim.api.nvim_create_autocmd("FileType", {
    pattern = "crystal",
    callback = function()
        -- Only set if current syntax highlighting is generic (no filetype)
        if vim.bo.syntax == 'on' or vim.bo.syntax == 'crystal' then
            vim.cmd("set syntax=ruby")
        end
    end
})
