local harpoon = require("harpoon")
harpoon:setup()

-- Add file to harpoon
vim.keymap.set("n", "<leader>a", function() harpoon:list():add() end, { desc = "Add file to Harpoon" })

-- Remove current file from harpoon
vim.keymap.set("n", "<leader>ar", function() harpoon:list():remove() end, { desc = "Remove from Harpoon" })

-- Clear all harpoon marks
vim.keymap.set("n", "<leader>ac", function() harpoon:list():clear() end, { desc = "Clear all Harpoon marks" })

-- Toggle harpoon menu
vim.keymap.set("n", "<leader>h", function() harpoon.ui:toggle_quick_menu(harpoon:list()) end, { desc = "Toggle Harpoon menu" })

-- Navigate with Option (Alt) + number
vim.keymap.set("n", "<M-1>", function() harpoon:list():select(1) end, { desc = "Harpoon file 1" })
vim.keymap.set("n", "<M-2>", function() harpoon:list():select(2) end, { desc = "Harpoon file 2" })
vim.keymap.set("n", "<M-3>", function() harpoon:list():select(3) end, { desc = "Harpoon file 3" })
vim.keymap.set("n", "<M-4>", function() harpoon:list():select(4) end, { desc = "Harpoon file 4" })
vim.keymap.set("n", "<M-5>", function() harpoon:list():select(5) end, { desc = "Harpoon file 5" })

-- Navigate to next/previous harpoon file
vim.keymap.set("n", "<M-n>", function() harpoon:list():next() end, { desc = "Next Harpoon file" })
vim.keymap.set("n", "<M-p>", function() harpoon:list():prev() end, { desc = "Previous Harpoon file" })
