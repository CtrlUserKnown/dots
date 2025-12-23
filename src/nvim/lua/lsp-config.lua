-- Function to add words to dictionary for different LSP servers
local function add_word_to_dictionary(word, client_name)
    -- For ltex LSP
    if client_name == "ltex" then
        local params = {
            command = "ltex.addToDictionary",
            arguments = {
                {
                    words = {
                        ["en-US"] = { word }  -- Change language code if needed
                    }
                }
            }
        }
        vim.lsp.buf.execute_command(params)
    end
    
    -- For typos LSP
    if client_name == "typos_lsp" then
        local config_path = vim.fn.expand("~/.config/typos.toml")
        vim.notify("Add custom typos config at " .. config_path, vim.log.levels.INFO)
    end
end

local on_attach = function(client, bufnr)
    local opts = { noremap = true, silent = true, buffer = bufnr }
    
    -- Enable inlay hints if the server supports it
    if client.server_capabilities.inlayHintProvider then
        vim.lsp.inlay_hint.enable(true, { bufnr = bufnr })
    end
    
    vim.keymap.set('n', 'K', vim.lsp.buf.hover, opts)
    vim.keymap.set('n', 'gd', vim.lsp.buf.definition, opts)
    vim.keymap.set('n', 'gD', vim.lsp.buf.declaration, opts)
    vim.keymap.set('n', 'gi', vim.lsp.buf.implementation, opts)
    vim.keymap.set('n', 'go', vim.lsp.buf.type_definition, opts)
    vim.keymap.set('n', 'gr', vim.lsp.buf.references, opts)
    vim.keymap.set('n', 'gs', vim.lsp.buf.signature_help, opts)
    vim.keymap.set('n', '<leader>rn', vim.lsp.buf.rename, opts)
    vim.keymap.set('n', '<leader>ca', vim.lsp.buf.code_action, opts)
    vim.keymap.set('n', '<leader>e', vim.diagnostic.open_float, opts)
    vim.keymap.set('n', '[d', vim.diagnostic.goto_prev, opts)
    vim.keymap.set('n', ']d', vim.diagnostic.goto_next, opts)
    
    -- Toggle inlay hints with a keybinding
    vim.keymap.set('n', '<leader>ih', function()
        vim.lsp.inlay_hint.enable(not vim.lsp.inlay_hint.is_enabled({ bufnr = bufnr }), { bufnr = bufnr })
    end, { buffer = bufnr, desc = 'Toggle inlay hints' })
    
    -- Add keybinding to ignore word under cursor (add to dictionary)
    vim.keymap.set('n', '<leader>ig', function()
        local word = vim.fn.expand('<cword>')
        add_word_to_dictionary(word, client.name)
        vim.notify("Added '" .. word .. "' to dictionary", vim.log.levels.INFO)
    end, vim.tbl_extend('force', opts, { desc = 'Ignore word (add to dictionary)' }))
    
    -- Add keybinding to inspect diagnostics (useful for debugging)
    vim.keymap.set('n', '<leader>di', function()
        local diag = vim.diagnostic.get(0, { lnum = vim.fn.line('.') - 1 })
        print(vim.inspect(diag))
    end, vim.tbl_extend('force', opts, { desc = 'Inspect diagnostics at cursor' }))
end

-- Filter diagnostics to remove comment-related checks and spelling errors
local function filter_diagnostics(diagnostics)
    return vim.tbl_filter(function(diagnostic)
        -- Filter out sentence/paragraph length warnings
        if diagnostic.message:match("sentence is %d+ words long") then
            return false
        end
        if diagnostic.message:match("paragraph is %d+ words long") then
            return false
        end
        -- Filter out other common comment linting messages
        if diagnostic.message:match("This sentence") then
            return false
        end
        
        -- Filter out spelling errors (uncomment if you want to hide all spelling errors)
        -- if diagnostic.message:match("Unknown word") then
        --     return false
        -- end
        -- if diagnostic.message:match("Spelling") then
        --     return false
        -- end
        -- if diagnostic.source == "ltex" or diagnostic.source == "typos" or diagnostic.source == "cspell" then
        --     return false
        -- end
        
        return true
    end, diagnostics)
end

-- Override diagnostic handler to filter unwanted diagnostics
local original_set = vim.diagnostic.set
vim.diagnostic.set = function(namespace, bufnr, diagnostics, opts)
    diagnostics = filter_diagnostics(diagnostics)
    original_set(namespace, bufnr, diagnostics, opts)
end

-- Configure diagnostics display
vim.diagnostic.config({
    virtual_text = {
        source = "if_many",
    },
    signs = true,
    underline = true,
    update_in_insert = false,
    severity_sort = true,
    float = {
        border = "rounded",
        source = "if_many",
        header = "",
        prefix = "",
    },
})

-- Only proceed if lspconfig is available
local lspconfig_ok, lspconfig = pcall(require, 'lspconfig')
if not lspconfig_ok then
    return {
        on_attach = on_attach,
        handlers = {},
    }
end

-- Safely load capabilities
local capabilities = vim.lsp.protocol.make_client_capabilities()
local cmp_nvim_lsp_ok, cmp_nvim_lsp = pcall(require, 'cmp_nvim_lsp')
if cmp_nvim_lsp_ok then
    capabilities = cmp_nvim_lsp.default_capabilities()
end

-- Configure sourcekit-lsp using the new vim.lsp API
vim.lsp.config('sourcekit-lsp', {
    cmd = { 'sourcekit-lsp' },
    filetypes = { 'swift', 'c', 'cpp', 'objective-c', 'objective-cpp' },
    root_markers = { 'Package.swift', 'compile_commands.json', '.git' },
    capabilities = capabilities,
})

-- Auto-start sourcekit-lsp for Swift files
vim.api.nvim_create_autocmd('FileType', {
    pattern = { 'swift', 'c', 'cpp', 'objective-c', 'objective-cpp' },
    callback = function(args)
        vim.lsp.enable('sourcekit-lsp')
        -- Call on_attach manually since we're using the new API
        local clients = vim.lsp.get_clients({ bufnr = args.buf, name = 'sourcekit-lsp' })
        if #clients > 0 then
            on_attach(clients[1], args.buf)
        end
    end,
})

local handlers = {
    -- Default handler for servers without a specific override
    function(server_name)
        lspconfig[server_name].setup {
            on_attach = on_attach,
            capabilities = capabilities,
        }
    end,

    -- Custom handler for gopls (Go)
    ['gopls'] = function()
        lspconfig.gopls.setup {
            on_attach = on_attach,
            capabilities = capabilities,
            settings = {
                gopls = {
                    analyses = {
                        unusedparams = true,
                        shadow = true,
                    },
                    staticcheck = true,
                    gofumpt = true,
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
        }
    end,

    -- Custom handler for tinymist (Typst)
    ['tinymist'] = function()
        lspconfig.tinymist.setup {
            on_attach = on_attach,
            capabilities = capabilities,
            settings = {
                exportPdf = "onSave",
                outputPath = "$root/$dir/$name",
            },
        }
    end,

    -- Custom handler for jdtls (Java)
    ['jdtls'] = function()
        -- Function to generate a unique workspace name based on the project root
        local function get_jdtls_workspace(root_dir)
            local project_name = vim.fn.fnamemodify(root_dir, ':p:h:t')
            local workspace_dir = vim.fn.stdpath('cache') .. '/jdtls/' .. project_name
            return workspace_dir
        end

        lspconfig.jdtls.setup(vim.tbl_deep_extend('force', {
            on_attach = on_attach,
            capabilities = capabilities,
        }, {
            filetypes = { 'java' },
            -- Root directory detection for Maven, Gradle, Ant, or Git
            root_dir = lspconfig.util.root_pattern(
                'build.xml',      -- Ant
                'pom.xml',        -- Maven
                'build.gradle',   -- Gradle
                'build.gradle.kts', -- Gradle Kotlin DSL
                'settings.gradle', -- Gradle multi-project
                'settings.gradle.kts',
                '.git'
            ),
            -- Override cmd to use unique workspace per project
            cmd_env = {
                JDTLS_WORKSPACE = function()
                    local root = vim.fs.root(0, {
                        'build.xml',
                        'pom.xml',
                        'build.gradle',
                        'build.gradle.kts',
                        'settings.gradle',
                        'settings.gradle.kts',
                        '.git'
                    })
                    if root then
                        return get_jdtls_workspace(root)
                    end
                    return vim.fn.stdpath('cache') .. '/jdtls/default'
                end
            },
            on_attach = function(client, bufnr)
                on_attach(client, bufnr)
                -- Force enable inlay hints for Java
                vim.lsp.inlay_hint.enable(true, { bufnr = bufnr })
            end,
            settings = {
                java = {
                    -- Eclipse JDT Language Server Configuration
                    eclipse = {
                        downloadSources = true,
                    },
                    configuration = {
                        updateBuildConfiguration = "interactive",
                    },
                    maven = {
                        downloadSources = true,
                        updateSnapshots = false,
                    },
                    -- Referenced libraries for all build systems
                    project = {
                        referencedLibraries = {
                            "lib/**/*.jar",
                            "**/lib/*.jar",
                            vim.fn.expand("~/.m2/repository/**/*.jar"),
                            vim.fn.expand("~/.gradle/caches/**/*.jar"),
                            "target/**/*.jar",
                            "build/libs/**/*.jar",
                            "dist/**/*.jar",
                        },
                    },
                    -- Inlay hints
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
                    -- Import organization
                    sources = {
                        organizeImports = {
                            starThreshold = 9999,
                            staticStarThreshold = 9999,
                        },
                    },
                    -- Code generation
                    codeGeneration = {
                        toString = {
                            template = "${object.className}{${member.name()}=${member.value}, ${otherMembers}}",
                        },
                        hashCodeEquals = {
                            useJava7Objects = true,
                        },
                        useBlocks = true,
                    },
                    -- Completion settings
                    completion = {
                        favoriteStaticMembers = {
                            "org.junit.Assert.*",
                            "org.junit.Assume.*",
                            "org.junit.jupiter.api.Assertions.*",
                            "org.junit.jupiter.api.Assumptions.*",
                            "org.junit.jupiter.api.DynamicContainer.*",
                            "org.junit.jupiter.api.DynamicTest.*",
                            "org.mockito.Mockito.*",
                            "org.mockito.ArgumentMatchers.*",
                            "org.mockito.Answers.*",
                        },
                        filteredTypes = {
                            "com.sun.*",
                            "io.micrometer.shaded.*",
                            "java.awt.*",
                            "jdk.*",
                            "sun.*",
                        },
                        importOrder = {
                            "java",
                            "javax",
                            "com",
                            "org",
                        },
                    },
                    -- Format settings
                    format = {
                        enabled = true,
                        settings = {
                            url = vim.fn.stdpath("config") .. "/java-format.xml",
                            profile = "GoogleStyle",
                        },
                    },
                },
            },
        }))
    end,
}

-- Auto-detect build system and set appropriate commands
vim.api.nvim_create_autocmd("FileType", {
    pattern = "java",
    callback = function()
        local root = vim.fs.root(0, {
            'build.xml',
            'pom.xml',
            'build.gradle',
            'build.gradle.kts',
            'settings.gradle',
            'settings.gradle.kts',
        })
        
        if root then
            -- Detect which build system is present
            local build_file = nil
            local build_system = nil
            
            if vim.fn.filereadable(root .. '/pom.xml') == 1 then
                build_system = 'Maven'
                build_file = 'pom.xml'
            elseif vim.fn.filereadable(root .. '/build.gradle') == 1 or 
                   vim.fn.filereadable(root .. '/build.gradle.kts') == 1 then
                build_system = 'Gradle'
                build_file = vim.fn.filereadable(root .. '/build.gradle') == 1 and 'build.gradle' or 'build.gradle.kts'
            elseif vim.fn.filereadable(root .. '/build.xml') == 1 then
                build_system = 'Ant'
                build_file = 'build.xml'
            end
            
            if build_system then
                -- Set buffer-local variable for easy reference
                vim.b.java_build_system = build_system
                vim.b.java_build_file = build_file
                vim.b.java_project_root = root
            end
        end
    end
})

-- Add keybindings for building Java projects
vim.api.nvim_create_autocmd("FileType", {
    pattern = "java",
    callback = function(args)
        local bufnr = args.buf
        local opts = { buffer = bufnr, noremap = true, silent = true }
        
        -- Build project based on detected build system
        vim.keymap.set('n', '<leader>jb', function()
            local build_system = vim.b.java_build_system
            local root = vim.b.java_project_root
            
            if not build_system or not root then
                vim.notify("No build system detected", vim.log.levels.WARN)
                return
            end
            
            local cmd
            if build_system == 'Maven' then
                cmd = 'cd ' .. root .. ' && mvn compile'
            elseif build_system == 'Gradle' then
                cmd = 'cd ' .. root .. ' && ./gradlew build'
            elseif build_system == 'Ant' then
                cmd = 'cd ' .. root .. ' && ant compile'
            end
            
            if cmd then
                vim.cmd('split | terminal ' .. cmd)
            end
        end, vim.tbl_extend('force', opts, { desc = 'Build Java project' }))
        
        -- Run tests
        vim.keymap.set('n', '<leader>jt', function()
            local build_system = vim.b.java_build_system
            local root = vim.b.java_project_root
            
            if not build_system or not root then
                vim.notify("No build system detected", vim.log.levels.WARN)
                return
            end
            
            local cmd
            if build_system == 'Maven' then
                cmd = 'cd ' .. root .. ' && mvn test'
            elseif build_system == 'Gradle' then
                cmd = 'cd ' .. root .. ' && ./gradlew test'
            elseif build_system == 'Ant' then
                cmd = 'cd ' .. root .. ' && ant test'
            end
            
            if cmd then
                vim.cmd('split | terminal ' .. cmd)
            end
        end, vim.tbl_extend('force', opts, { desc = 'Run Java tests' }))
        
        -- Clean project
        vim.keymap.set('n', '<leader>jc', function()
            local build_system = vim.b.java_build_system
            local root = vim.b.java_project_root
            
            if not build_system or not root then
                vim.notify("No build system detected", vim.log.levels.WARN)
                return
            end
            
            local cmd
            if build_system == 'Maven' then
                cmd = 'cd ' .. root .. ' && mvn clean'
            elseif build_system == 'Gradle' then
                cmd = 'cd ' .. root .. ' && ./gradlew clean'
            elseif build_system == 'Ant' then
                cmd = 'cd ' .. root .. ' && ant clean'
            end
            
            if cmd then
                vim.cmd('split | terminal ' .. cmd)
            end
        end, vim.tbl_extend('force', opts, { desc = 'Clean Java project' }))
        
        -- Clean JDTLS workspace cache for current project
        vim.keymap.set('n', '<leader>jw', function()
            local root = vim.b.java_project_root
            if not root then
                vim.notify("No Java project detected", vim.log.levels.WARN)
                return
            end
            
            local project_name = vim.fn.fnamemodify(root, ':p:h:t')
            local workspace_dir = vim.fn.stdpath('cache') .. '/jdtls/' .. project_name
            
            -- Stop LSP first
            vim.cmd('LspStop jdtls')
            
            -- Delete workspace cache
            vim.fn.system('rm -rf ' .. workspace_dir)
            
            vim.notify("Cleaned JDTLS workspace for " .. project_name, vim.log.levels.INFO)
            vim.notify("Restart Neovim or run :LspStart to reinitialize", vim.log.levels.INFO)
        end, vim.tbl_extend('force', opts, { desc = 'Clean JDTLS workspace cache' }))
    end
})

return {
    on_attach = on_attach,
    handlers = handlers,
}
