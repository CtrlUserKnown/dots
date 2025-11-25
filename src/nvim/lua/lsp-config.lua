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
end

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
                        -- Support for different Java versions
                        runtimes = {
                            -- Add your Java installations here if needed
                            -- {
                            --     name = "JavaSE-17",
                            --     path = "/path/to/jdk-17",
                            -- },
                        },
                    },
                    maven = {
                        downloadSources = true,
                        updateSnapshots = false,
                    },
                    -- Referenced libraries for all build systems
                    project = {
                        referencedLibraries = {
                            -- Ant libraries
                            "lib/**/*.jar",
                            "**/lib/*.jar",
                            -- Maven local repository
                            vim.fn.expand("~/.m2/repository/**/*.jar"),
                            -- Gradle cache
                            vim.fn.expand("~/.gradle/caches/**/*.jar"),
                            -- Common library patterns
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
                
                -- Optional: Print build system detection
                -- vim.notify("Detected " .. build_system .. " project at " .. root, vim.log.levels.INFO)
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
    end
})

return {
    on_attach = on_attach,
    handlers = handlers,
}
