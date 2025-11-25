-- force Escape key to always work
vim.keymap.set('i', '<Esc>', '<Esc>', { noremap = true, silent = true })
vim.keymap.set('i', 'jk', '<Esc>', { noremap = true, silent = true }) -- Alternative: type 'jk' quickly

-- save and quit with leader keys
vim.keymap.set('n', '<leader>w', ':w<CR>', { noremap = true, silent = true, desc = 'Save file' })
vim.keymap.set('n', '<leader>q', ':wq<CR>', { noremap = true, silent = true, desc = 'Save and quit' })
vim.keymap.set('n', '<leader>qq', ':q!<CR>', { noremap = true, silent = true, desc = 'Quit without saving' })

-- go to beginning and end of file
vim.keymap.set('n', '[[', 'gg', { noremap = true, silent = true, desc = 'Go to beginning of file' })
vim.keymap.set('n', ']]', 'G', { noremap = true, silent = true, desc = 'Go to end of file' })

-- go to beginning and end of line
vim.keymap.set('n', 'el', '0', { noremap = true, silent = true, desc = 'Go to beginning of line' })
vim.keymap.set('n', 'ea', '$', { noremap = true, silent = true, desc = 'Go to end of line' })
vim.keymap.set('v', 'el', '0', { noremap = true, silent = true, desc = 'Go to beginning of line' })
vim.keymap.set('v', 'ea', '$', { noremap = true, silent = true, desc = 'Go to end of line' })

-- paragraph navigation with Option + ( and )
vim.keymap.set('n', '<M-[>', '{', { noremap = true, silent = true, desc = 'Previous paragraph' })
vim.keymap.set('n', '<M-]>', '}', { noremap = true, silent = true, desc = 'Next paragraph' })
vim.keymap.set('v', '<M-[>', '{', { noremap = true, silent = true, desc = 'Previous paragraph' })
vim.keymap.set('v', '<M-]>', '}', { noremap = true, silent = true, desc = 'Next paragraph' })

-- change undo key to R
vim.keymap.set('n', 'R', 'u', { desc = 'Undo' })

-- Move line up and down with option/alt + j/k
vim.keymap.set('n', '<A-k>', ':m .-2<CR>==', { desc = 'Move line up' })
vim.keymap.set('n', '<A-j>', ':m .+1<CR>==', { desc = 'Move line down' })
vim.keymap.set('v', '<A-k>', ":m '<-2<CR>gv=gv", { desc = 'Move selection up' })
vim.keymap.set('v', '<A-j>', ":m '>+1<CR>gv=gv", { desc = 'Move selection down' })
vim.keymap.set('i', '<C-k>', '<Esc>:m .-2<CR>==gi', { desc = 'Move line up' })
vim.keymap.set('i', '<C-j>', '<Esc>:m .+1<CR>==gi', { desc = 'Move line down' })

-- Fix navigation when not inside tmux
vim.g.tmux_navigator_no_mappings = 1

-- Always allow Vim split navigation with CTRL + h/j/k/l
vim.keymap.set('n', '<C-h>', '<C-w>h', { silent = true })
vim.keymap.set('n', '<C-j>', '<C-w>j', { silent = true })
vim.keymap.set('n', '<C-k>', '<C-w>k', { silent = true })
vim.keymap.set('n', '<C-l>', '<C-w>l', { silent = true })
