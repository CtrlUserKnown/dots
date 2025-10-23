-- set leader keys before anything else
vim.g.mapleader = " "
vim.g.maplocalleader = "\\"
vim.opt.timeoutlen = 2000 -- better timeout lenght

-- --- config:load
-- highlight current line number
vim.api.nvim_set_hl(0, 'LineNr', { bold = true })

-- load vim options for defaults
require('options')

-- load keymaps for editor
require('keymaps')

-- load lsp for editor
require('lsp-config')

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
