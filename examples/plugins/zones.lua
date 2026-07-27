-- zones.lua — give a plugin its own region of the dashboard.
--
-- Panes normally land in the dashboard's catch-all zone, mixed in with the
-- built-in tiles. When a plugin owns several related panes it usually wants
-- them grouped instead: declare a zone, then point the panes at it by id.
--
-- Drop this file in ~/.dots/plugins/, then run `dots layout show` to see the
-- placement before opening the TUI.

-- A titled zone draws a labelled border around whatever lands in it. `span` is
-- how many top-level columns it takes, `size` its height relative to other
-- zones, and `columns` how its own panes are arranged inside it.
ui.zone{
  id      = "dev",
  title   = "Dev",
  span    = 2,      -- full width
  size    = 1,
  columns = 2,      -- the two panes below sit side by side
}

ui.pane{
  id      = "git-status",
  title   = "Repo",
  zone    = "dev",
  refresh = 15,
  render  = function()
    local branch = dots.sh("git -C " .. dots.dir() .. " rev-parse --abbrev-ref HEAD")
    if branch == "" then return { "not a git checkout" } end
    local dirty = dots.sh("git -C " .. dots.dir() .. " status --porcelain | wc -l | tr -d ' '")
    return {
      "branch " .. branch,
      (dirty == "0" and "clean" or dirty .. " file(s) changed"),
    }
  end,
}

ui.pane{
  id      = "disk",
  title   = "Disk",
  zone    = "dev",
  refresh = 60,
  render  = function()
    local used = dots.sh("df -h $HOME | awk 'NR==2 {print $5\" of \"$2}'")
    return { used == "" and "unknown" or used }
  end,
}

-- Nothing here forces a layout on you: if ~/.dots/layout.toml already defines a
-- zone called "dev", yours is used as written and this declaration is ignored.
