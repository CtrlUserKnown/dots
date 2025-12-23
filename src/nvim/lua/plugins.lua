return
{
    -- Nvim Surround
    {
        "kylechui/nvim-surround",
        version = "*",
        event = "VeryLazy",
        config = function()
            require("nvim-surround").setup({})
        end
    },

    -- Multi-cursor support
    {
        "mg979/vim-visual-multi",
        branch = "master",
        lazy = false,
    },

    -- Vim-Tmux-Navigator
    {
        "christoomey/vim-tmux-navigator",
        cmd = {
            "TmuxNavigateLeft",
            "TmuxNavigateDown",
            "TmuxNavigateUp",
            "TmuxNavigateRight",
            "TmuxNavigatePrevious",
        },
        keys = {
            { "<c-h>", "<cmd>TmuxNavigateLeft<cr>" },
            { "<c-j>", "<cmd>TmuxNavigateDown<cr>" },
            { "<c-k>", "<cmd>TmuxNavigateUp<cr>" },
            { "<c-l>", "<cmd>TmuxNavigateRight<cr>" },
            { "<c-\\>", "<cmd>TmuxNavigatePrevious<cr>" },
        },
    },

    -- Harpoon2
    {
        'ThePrimeagen/harpoon',
        branch = 'harpoon2',
        dependencies = { 'nvim-lua/plenary.nvim' },
        config = function()
            require('harpoon-config')
        end
    },

    -- Rosé Pine
    {
        'rose-pine/neovim',
        name = 'rose-pine',
        priority = 1000,
        config = function()
            require('rose-pine').setup({
                variant = 'main', -- 'main', 'moon', or 'dawn'
                disable_background = true,
                disable_float_background = true,
                disable_italics = false,
                highlight_groups = {
                        CursorLineNr = { fg = 'gold', bold = true }
                }
            })
            vim.cmd('colorscheme rose-pine')

        end
    },

    -- Treesitter
    {
        'nvim-treesitter/nvim-treesitter',
        build = ':TSUpdate',
    },

    -- Mason (LSP/DAP/Linter installer)
    {
        'williamboman/mason.nvim',
        config = function()
            require('mason').setup({
                ui = {
                    icons = {
                        package_installed = "✓",
                        package_pending = "➜",
                        package_uninstalled = "✗"
                    }
                }
            })
        end,
    },

    {
        'williamboman/mason-lspconfig.nvim',
        dependencies = { 'williamboman/mason.nvim', 'neovim/nvim-lspconfig' },
        config = function()
            local lsp_config = require('lsp-config')
            require('mason-lspconfig').setup({
                ensure_installed = {
                    'lua_ls',
                    'pyright',
                    'ts_ls',
                    'jdtls',
                },
                automatic_installation = true,
                handlers = lsp_config.handlers,
            })
        end,
    },


    -- Refactoring
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
            vim.keymap.set({ "n", "x" }, "<leader>R", refactoring.select_refactor, { desc = "Select Refactor" })

            -- Example specific refactor operations:
            vim.keymap.set({ "x" }, "<leader>Rc", function()
                return refactoring.refactor('Extract Function')
            end, { expr = true, desc = "Extract Function" })

            vim.keymap.set({ "x" }, "<leader>Rv", function()
                return refactoring.refactor('Extract Variable')
            end, { expr = true, desc = "Extract Variable" })

            vim.keymap.set({ "n", "x" }, "<leader>Ri", function()
                return refactoring.refactor('Inline Variable')
            end, { expr = true, desc = "Inline Variable" })

            -- debugger functionality
            vim.keymap.set({ "n", "x" }, "<leader>Rp", refactor('Printf'), { desc = "Insert Printf" })
            vim.keymap.set({ "n", "x" }, "<leader>RP", refactor('Print Var'), { desc = "Insert Print Var" })
            vim.keymap.set({ "n" }, "<leader>Rc", refactor('Cleanup'), { desc = "Cleanup Debug" })
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
                    prompt_prefix = ' 🔎  ',
                    selection_caret = '▸',
                },
                extensions = {
                    file_browser = {
                        hidden = true,
                        hijack_netrw = true, -- Telescope instead of netrw
                    },
                },
            })
            -- load file browser extension
            pcall(telescope.load_extension, 'file_browser')
        end,
    },

    -- Which-key
    {
        "folke/which-key.nvim",
        event = "VeryLazy",
        opts = {
            preset = "modern",
        },
        config = function()
            local wk = require("which-key")
            wk.setup({
                preset = "modern",
            })

            -- Add key descriptions
            wk.add({
                { "<leader>R", group = "Refactor" },
                { "<leader>w", desc = "Save file" },
                { "<leader>q", desc = "Save and quit" },
                { "<leader>Q", desc = "Quit without saving" },
                { "<leader>e", desc = "File explorer" },
                { "<leader>v", desc = "File explorer" },
            })
        end,
    },

    -- noice.nvim - Better UI for cmdline and popups
    {
        "folke/noice.nvim",
        event = "VeryLazy",
        dependencies = {
            "MunifTanjim/nui.nvim",
        },
        opts = {
            cmdline = {
                enabled = true,
                view = "cmdline_popup",
                format = {
                    cmdline = { icon = ":" },
                    search_down = { icon = "🔍 ⌄" },
                    search_up = { icon = "🔍 ⌃" },
                    filter = { icon = "$" },
                    lua = { icon = "☾" },
                    help = { icon = "?" },
                },
            },
            messages = {
                enabled = false,  -- Disable messages to avoid needing nvim-notify
            },
            popupmenu = {
                enabled = true,
                backend = "nui",
            },
            lsp = {
                override = {
                    ["vim.lsp.util.convert_input_to_markdown_lines"] = true,
                    ["vim.lsp.util.stylize_markdown"] = true,
                    ["cmp.entry.get_documentation"] = true,
                },
            },
            presets = {
                bottom_search = true,
                command_palette = true,
                long_message_to_split = true,
            },
        },
    },

    -- GitHub Copilot
    {
        "zbirenbaum/copilot.lua",
        cmd = "Copilot",
        event = "InsertEnter",
        config = function()
            require("copilot").setup({
                panel = {
                    enabled = true,
                    auto_refresh = false,
                    keymap = {
                        jump_prev = "[[",
                        jump_next = "]]",
                        accept = "<CR>",
                        refresh = "gr",
                        open = "<M-CR>"
                    },
                    layout = {
                        position = "bottom", -- | top | left | right
                        ratio = 0.4
                    },
                },
                suggestion = {
                    enabled = true,
                    auto_trigger = true,
                    debounce = 75,
                    keymap = {
                        accept = "<M-l>",
                        accept_word = false,
                        accept_line = false,
                        next = "<M-k>",
                        prev = "<M-j>",
                        dismiss = "<C-]>",
                    },
                },
                filetypes = {
                    yaml = false,
                    markdown = false,
                    help = false,
                    gitcommit = false,
                    gitrebase = false,
                    hgcommit = false,
                    svn = false,
                    cvs = false,
                    ["."] = false,
                },
                copilot_node_command = 'node', -- Node.js version must be > 18.x
                server_opts_overrides = {},
            })
        end,
    },

    -- Copilot CMP source (optional - integrates Copilot with nvim-cmp)
    {
        "zbirenbaum/copilot-cmp",
        dependencies = { "zbirenbaum/copilot.lua" },
        config = function()
            require("copilot_cmp").setup()
        end
    },

    -- alpha-nvim dashboard
    {
        "goolord/alpha-nvim",
        config = function ()
            require('alpha-config')
        end
    },

    -- Typst Preview (browser-based preview with low latency)
    {
        'chomosuke/typst-preview.nvim',
        ft = 'typst',
        version = '1.*',
        build = function()
            require('typst-preview').update()
        end,
        opts = {},
        config = function()
            -- Create custom commands for Typst preview
            vim.api.nvim_create_user_command('TP', 'TypstPreview', { desc = 'Start Typst preview' })
            vim.api.nvim_create_user_command('TS', 'TypstPreviewStop', { desc = 'Stop Typst preview' })
            vim.api.nvim_create_user_command('TU', 'TypstPreviewUpdate', { desc = 'Update Typst preview' })
        end
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
