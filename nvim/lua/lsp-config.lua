-- LSP Configuration for Neovim 0.11+
-- Using vim.lsp.config instead of deprecated lspconfig module

-- Use an on_attach function to only map the following keys
-- after the language server attaches to the current buffer
local on_attach = function(client, bufnr)
    local bufopts = { noremap = true, silent = true, buffer = bufnr }

    -- Enable completion triggered by <c-x><c-o>
    vim.api.nvim_buf_set_option(bufnr, 'omnifunc', 'v:lua.vim.lsp.omnifunc')

    -- Enable inlay hints if the server supports it
    if client.server_capabilities.inlayHintProvider then
        vim.lsp.inlay_hint.enable(true, { bufnr = bufnr })
    end

    -- Keybindings
    vim.keymap.set('n', 'gD', vim.lsp.buf.declaration, bufopts)
    vim.keymap.set('n', 'gd', vim.lsp.buf.definition, bufopts)
    vim.keymap.set('n', 'K', vim.lsp.buf.hover, bufopts)
    vim.keymap.set('n', 'gi', vim.lsp.buf.implementation, bufopts)
    vim.keymap.set('n', '<leader>k', vim.lsp.buf.signature_help, bufopts)
    vim.keymap.set('n', '<leader>D', vim.lsp.buf.type_definition, bufopts)
    vim.keymap.set('n', 'mr', vim.lsp.buf.rename, bufopts)
    vim.keymap.set('n', '<leader>ca', vim.lsp.buf.code_action, bufopts)
    vim.keymap.set('n', 'gr', vim.lsp.buf.references, bufopts)
    vim.keymap.set('n', '<leader>f', function() vim.lsp.buf.format { async = true } end, bufopts)

    -- Toggle inlay hints
    vim.keymap.set('n', '<leader>ih', function()
        vim.lsp.inlay_hint.enable(not vim.lsp.inlay_hint.is_enabled({ bufnr = bufnr }), { bufnr = bufnr })
    end, { buffer = bufnr, desc = 'Toggle inlay hints' })

    -- Diagnostic keymaps
    vim.keymap.set('n', '[d', vim.diagnostic.goto_prev, bufopts)
    vim.keymap.set('n', ']d', vim.diagnostic.goto_next, bufopts)
    vim.keymap.set('n', '<leader>d', vim.diagnostic.open_float, bufopts)
    vim.keymap.set('n', '<leader>dl', vim.diagnostic.setloclist, bufopts)
end

-- Format on save
vim.api.nvim_create_autocmd("BufWritePre", {
    pattern = { "*.js", "*.ts", "*.jsx", "*.tsx", "*.css", "*.html", "*.json", "*.py", "*.rs", "*.go", "*.lua" },
    callback = function()
        vim.lsp.buf.format({ async = false })
    end,
})

-- Add additional capabilities supported by nvim-cmp
local capabilities = vim.lsp.protocol.make_client_capabilities()
local cmp_lsp_ok, cmp_nvim_lsp = pcall(require, 'cmp_nvim_lsp')
if cmp_lsp_ok then
    capabilities = cmp_nvim_lsp.default_capabilities(capabilities)
end

-- Configure diagnostic display (updated for Neovim 0.11+)
vim.diagnostic.config({
    virtual_text = true,
    signs = {
        text = {
            [vim.diagnostic.severity.ERROR] = "✘",
            [vim.diagnostic.severity.WARN] = "▲",
            [vim.diagnostic.severity.HINT] = "⚑",
            [vim.diagnostic.severity.INFO] = "»",
        },
    },
    update_in_insert = false,
    underline = true,
    severity_sort = true,
    float = {
        border = 'rounded',
        source = 'always',
    },
})

-- Helper function to setup LSP servers
local function setup_server(name, config)
    config = config or {}
    config.on_attach = on_attach
    config.capabilities = capabilities

    vim.lsp.config(name, config)
    vim.lsp.enable(name)
end

-- Setup language servers using the new API

-- Lua
setup_server('lua_ls', {
    settings = {
        Lua = {
            runtime = {
                version = 'LuaJIT',
            },
            diagnostics = {
                globals = { 'vim' },
            },
            workspace = {
                library = vim.api.nvim_get_runtime_file("", true),
                checkThirdParty = false,
            },
            telemetry = {
                enable = false,
            },
            hint = {
                enable = true,
                setType = true,
                paramType = true,
                paramName = 'All',
                semicolon = 'All',
                arrayIndex = 'Auto',
            },
        },
    },
})

-- Find and replace with popup
vim.keymap.set('n', '<leader>sr', function()
  local word = vim.fn.expand('<cword>')
  vim.ui.input({ prompt = 'Find: ', default = word }, function(find)
    if not find then return end
    vim.ui.input({ prompt = 'Replace with: ' }, function(replace)
      if not replace then return end
      vim.ui.input({ prompt = 'Options (gc for confirm): ', default = 'gc' }, function(opts)
        opts = opts or 'gc'
        vim.cmd(string.format('%%s/%s/%s/%s', find, replace, opts))
      end)
    end)
  end)
end, { noremap = true, silent = true, desc = 'Find and replace with popup' })

-- Find and replace in visual selection with popup
vim.keymap.set('v', '<leader>sr', function()
  vim.ui.input({ prompt = 'Find: ' }, function(find)
    if not find then return end
    vim.ui.input({ prompt = 'Replace with: ' }, function(replace)
      if not replace then return end
      vim.ui.input({ prompt = 'Options (gc for confirm): ', default = 'gc' }, function(opts)
        opts = opts or 'gc'
        vim.cmd(string.format("'<,'>s/%s/%s/%s", find, replace, opts))
      end)
    end)
  end)
end, { noremap = true, silent = true, desc = 'Find and replace in selection with popup' })

-- LSP rename with popup (for refactoring variables)
vim.keymap.set('n', '<leader>rn', function()
  local current_name = vim.fn.expand('<cword>')
  vim.ui.input({ 
    prompt = string.format('Rename "%s" to: ', current_name),
    default = current_name 
  }, function(new_name)
    if new_name and new_name ~= '' and new_name ~= current_name then
      vim.lsp.buf.rename(new_name)
    end
  end)
end, { noremap = true, silent = true, desc = 'Rename symbol (LSP)' })

-- Python
setup_server('pyright', {
    settings = {
        python = {
            analysis = {
                inlayHints = {
                    variableTypes = true,
                    functionReturnTypes = true,
                },
            },
        },
    },
})

-- JavaScript/TypeScript
setup_server('ts_ls', {
    settings = {
        typescript = {
            inlayHints = {
                includeInlayParameterNameHints = 'all',
                includeInlayParameterNameHintsWhenArgumentMatchesName = true,
                includeInlayFunctionParameterTypeHints = true,
                includeInlayVariableTypeHints = true,
                includeInlayPropertyDeclarationTypeHints = true,
                includeInlayFunctionLikeReturnTypeHints = true,
                includeInlayEnumMemberValueHints = true,
            },
        },
        javascript = {
            inlayHints = {
                includeInlayParameterNameHints = 'all',
                includeInlayParameterNameHintsWhenArgumentMatchesName = true,
                includeInlayFunctionParameterTypeHints = true,
                includeInlayVariableTypeHints = true,
                includeInlayPropertyDeclarationTypeHints = true,
                includeInlayFunctionLikeReturnTypeHints = true,
                includeInlayEnumMemberValueHints = true,
            },
        },
    },
})

-- Rust
setup_server('rust_analyzer', {
    settings = {
        ['rust-analyzer'] = {
            checkOnSave = {
                command = "clippy",
            },
            inlayHints = {
                bindingModeHints = {
                    enable = true,
                },
                chainingHints = {
                    enable = true,
                },
                closingBraceHints = {
                    minLines = 25,
                },
                closureReturnTypeHints = {
                    enable = "always",
                },
                lifetimeElisionHints = {
                    enable = "always",
                    useParameterNames = true,
                },
                maxLength = 25,
                parameterHints = {
                    enable = true,
                },
                reborrowHints = {
                    enable = "always",
                },
                renderColons = true,
                typeHints = {
                    enable = true,
                    hideClosureInitialization = false,
                    hideNamedConstructor = false,
                },
            },
        },
    },
})

-- Go
setup_server('gopls', {
    settings = {
        gopls = {
            hints = {
                assignVariableTypes = true,
                compositeLiteralFields = true,
                compositeLiteralTypes = true,
                constantValues = true,
                functionTypeParameters = true,
                parameterNames = true,
                rangeVariableTypes = true,
            },
        },
    },
})

-- C/C++
setup_server('clangd', {
    filetypes = { 'c', 'cpp', 'objc', 'objcpp', 'cuda', 'proto' },
    cmd = {
        "clangd",
        "--background-index",
        "--clang-tidy",
        "--header-insertion=iwyu",
        "--completion-style=detailed",
        "--function-arg-placeholders",
        "--fallback-style=llvm",
        "--inlay-hints",
    },
    settings = {
        clangd = {
            InlayHints = {
                Designators = true,
                Enabled = true,
                ParameterNames = true,
                DeducedTypes = true,
            },
        },
    },
})

-- HTML
setup_server('html')

-- CSS
setup_server('cssls')

-- JSON
setup_server('jsonls')

-- Java
setup_server('jdtls', {
    filetypes = { 'java' },
    on_attach = function(client, bufnr)
        on_attach(client, bufnr)
        -- Force enable inlay hints for jdtls
        vim.lsp.inlay_hint.enable(true, { bufnr = bufnr })
    end,
    settings = {
        java = {
            inlayHints = {
                parameterNames = {
                    enabled = "all",
                },
            },
            implementationsCodeLens = {
                enabled = true,
            },
            referencesCodeLens = {
                enabled = true,
            },
        },
    },
})

-- Bash
setup_server('bashls')

-- Swift
setup_server('sourcekit', {
    cmd = { 'sourcekit-lsp' },
    settings = {
        sourcekit = {
            inlayHints = {
                enabled = true,
            },
        },
    },
})

-- Crystal
setup_server('crystalline', {
    cmd = { 'crystalline' },
    filetypes = { 'crystal' },
})
