return {
    {
        "kylechui/nvim-surround",
        version = "*",
        event = "VeryLazy",
        config = function()
            require("nvim-surround").setup({})
        end
    },

    -- Treesitter for better syntax highlighting and code understanding
    {
        'nvim-treesitter/nvim-treesitter',
        build = ':TSUpdate',
        config = function()
            require('treesitter-config')
        end,
    },

    {
        'ThePrimeagen/refactoring.nvim',
        dependencies = {
            'nvim-lua/plenary.nvim', -- refactoring.nvim requires plenary.nvim
            'nvim-treesitter/nvim-treesitter',
        },
        config = function()
            local refactoring = require('refactoring')
            local function refactor(name)
                return function()
                    refactoring.refactor(name)
                end
            end

            -- Select refactor operation with vim.ui.select (or Telescope if configured)
            vim.keymap.set({ "n", "x" }, "<leader>rr", refactoring.select_refactor, { desc = "Select Refactor" })

            -- Example specific refactor operations:
            vim.keymap.set({ "x" }, "<leader>re", function()
                return refactoring.refactor('Extract Function')
            end, { expr = true, desc = "Extract Function" })

            vim.keymap.set({ "x" }, "<leader>rv", function()
                return refactoring.refactor('Extract Variable')
            end, { expr = true, desc = "Extract Variable" })

            vim.keymap.set({ "n", "x" }, "<leader>ri", function()
                return refactoring.refactor('Inline Variable')
            end, { expr = true, desc = "Inline Variable" })

            -- debugger functionality
            vim.keymap.set({ "n", "x" }, "<leader>rp", refactor('Printf'), { desc = "Insert Printf" })
            vim.keymap.set({ "n", "x" }, "<leader>rP", refactor('Print Var'), { desc = "Insert Print Var" })
            vim.keymap.set({ "n" }, "<leader>rc", refactor('Cleanup'), { desc = "Cleanup Debug" })
        end
    },

    -- Telescope and file browser extension
    {
        'nvim-telescope/telescope.nvim',
        dependencies = {
            'nvim-lua/plenary.nvim',
            'nvim-telescope/telescope-file-browser.nvim',
        },
        config = function()
            local telescope = require('telescope')
            telescope.setup({
                defaults = {
                    prompt_prefix = '   ',
                    selection_caret = '❯',
                },
                extensions = {
                    file_browser = {
                        hidden = true,
                        hijack_netrw = true, -- Telescope instead of
                    },
                },
            })
            -- load file browser extension
            pcall(telescope.load_extension, 'file_browser')
        end,
    },

    -- Completion plugins
    'hrsh7th/nvim-cmp',     -- The completion plugin
    'hrsh7th/cmp-buffer',   -- buffer completions
    'hrsh7th/cmp-path',     -- path completions
    'hrsh7th/cmp-cmdline',  -- cmdline completions
    'hrsh7th/cmp-nvim-lsp', -- LSP completions
    'hrsh7th/cmp-nvim-lua', -- Lua completions

    -- Snippet engine (required for nvim-cmp)
    'L3MON4D3/LuaSnip',         -- snippet engine
    'saadparwaiz1/cmp_luasnip', -- snippet completions
}
