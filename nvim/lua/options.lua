-- show line numbers
vim.wo.number = true

-- enable line trails
vim.opt.list = true
vim.opt.listchars = { tab = '▸ ', trail = '·', extends = '»', precedes = '«', nbsp = '␣' }

-- remove statusline
vim.opt.laststatus = 0
vim.opt.showcmd = true

-- change the key for the file explorer (use Telescope file browser)
vim.api.nvim_create_user_command('E', 'Telescope file_browser path=%:p:h select_buffer=true hidden=true', {})
vim.api.nvim_create_user_command('Tree', 'Telescope file_browser path=%:p:h select_buffer=true hidden=true', {})

vim.keymap.set('n', '<leader>e', ':Telescope file_browser path=%:p:h select_buffer=true hidden=true<CR>')
vim.keymap.set('n', '<leader>v', ':Telescope file_browser path=%:p:h select_buffer=true hidden=true<CR>')

-- vim character encoding (utf8)
vim.scriptencoding = "utf-8"
vim.opt.encoding = "utf-8"
vim.opt.fileencoding = "utf-8"

-- disable backups
vim.opt.backup = false

-- show 10 lines are visable abbove/below when scrolling
vim.scrolloff = 10

-- change tabs to spaces
vim.opt.expandtab = true
vim.opt.smarttab = true
vim.opt.smartindent = true
vim.opt.shiftwidth = 4
vim.opt.tabstop = 4
vim.opt.softtabstop = 4

-- swap file (set to false by default)
vim.opt.swapfile = false

-- use system clipboard
vim.opt.clipboard = "unnamedplus"

-- highlight the current line number
vim.opt.cursorline = true
vim.opt.cursorlineopt = 'number'

-- cursor shape in different modes
vim.opt.guicursor = "n-v-c:block,i-ci-ve:ver25,r-cr:hor20,o:hor50"
