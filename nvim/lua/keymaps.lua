-- force Escape key to always work
vim.keymap.set('i', '<Esc>', '<Esc>', { noremap = true, silent = true })
vim.keymap.set('i', 'jk', '<Esc>', { noremap = true, silent = true }) -- Alternative: type 'jk' quickly

-- save and quit with leader keys
vim.keymap.set('n', '<leader>w', ':w<CR>', { noremap = true, silent = true, desc = 'Save file' })
vim.keymap.set('n', '<leader>q', ':wq<CR>', { noremap = true, silent = true, desc = 'Save and quit' })
vim.keymap.set('n', '<leader>Q', ':q!<CR>', { noremap = true, silent = true, desc = 'Quit without saving' })

-- go to beginning and end of file
vim.keymap.set('n', '<leader>gg', 'gg', { noremap = true, silent = true, desc = 'Go to beginning of file' })
vim.keymap.set('n', '<leader>G', 'G', { noremap = true, silent = true, desc = 'Go to end of file' })

-- go to beginning and end of line
vim.keymap.set('n', 'gh', '0', { noremap = true, silent = true, desc = 'Go to beginning of line' })
vim.keymap.set('n', 'gl', '$', { noremap = true, silent = true, desc = 'Go to end of line' })
vim.keymap.set('v', 'gh', '0', { noremap = true, silent = true, desc = 'Go to beginning of line' })
vim.keymap.set('v', 'gl', '$', { noremap = true, silent = true, desc = 'Go to end of line' })

-- paragraph navigation with Option + ( and )
vim.keymap.set('n', '<M-[>', '{', { noremap = true, silent = true, desc = 'Previous paragraph' })
vim.keymap.set('n', '<M-]>', '}', { noremap = true, silent = true, desc = 'Next paragraph' })
vim.keymap.set('v', '<M-[>', '{', { noremap = true, silent = true, desc = 'Previous paragraph' })
vim.keymap.set('v', '<M-]>', '}', { noremap = true, silent = true, desc = 'Next paragraph' })
