-- github.lua — show your open GitHub PRs and notifications on the dashboard.
--
-- Requires the GitHub CLI (`gh`) to be installed and authenticated
-- (`gh auth login`). Drop this file in ~/.dots/plugins/ and run `dots`.

ui.pane{
  id      = "github",
  title   = "GitHub",
  span    = 2,          -- full-width row (spans both columns of a 2-col grid)
  refresh = 120,        -- re-check every 2 minutes
  render  = function()
    -- gh returns "" if not installed / not logged in, so these degrade nicely.
    local prs    = dots.sh("gh pr list --author @me --state open | wc -l | tr -d ' '")
    local review = dots.sh("gh pr list --search 'review-requested:@me state:open' | wc -l | tr -d ' '")
    local notifs = dots.sh("gh api notifications --jq 'length' 2>/dev/null")

    if prs == "" then
      return { "gh not available", "run: gh auth login" }
    end
    return {
      prs    .. " PR(s) open by you",
      review .. " awaiting your review",
      (notifs == "" and "0" or notifs) .. " unread notification(s)",
    }
  end,
  -- Press enter on the pane to open your GitHub dashboard in the browser.
  on_enter = function()
    dots.sh("gh browse --no-browser >/dev/null 2>&1; xdg-open https://github.com/pulls >/dev/null 2>&1 &")
  end,
}
