-- --- config:start ---
local wezterm = require 'wezterm'
local config = wezterm.config_builder()

-- --- appearance ---
-- --- appearance:font ---
config.font = wezterm.font('JetBrains Mono')
config.font_size = 14.0

-- --- appearance:theme ---
config.color_scheme = 'rose-pine'
config.window_background_opacity = 0.8
config.macos_window_background_blur = 20

-- fancy tab bar style (hovering buttons)
config.use_fancy_tab_bar = true

-- make tab bar and title bar colors follow the color scheme
config.show_tab_index_in_tab_bar = false
config.window_frame = {
    font = wezterm.font({ family = 'JetBrains Mono', weight = 'Bold' }),
    font_size = 12.0,
}

-- --- appearance:window ---
config.initial_cols = 120
config.initial_rows = 50

-- --- appearance:padding ---
config.window_padding = {
    left = 2,
    right = 2,
    top = 2,
    bottom = 2,
}

-- --- config:end ---
return config
