-- aws.lua — show the AWS identity/region you're pointed at.
--
-- Requires the AWS CLI (`aws`) configured with credentials. This is a
-- read-only status pane; it never changes anything in your account.

ui.pane{
  id      = "aws",
  title   = "AWS",
  size    = 2,          -- twice as tall as a normal pane
  refresh = 300,        -- identity rarely changes; check every 5 minutes
  render  = function()
    local account = dots.sh("aws sts get-caller-identity --query Account --output text 2>/dev/null")
    if account == "" then
      return { "aws not configured", "run: aws configure" }
    end
    local arn    = dots.sh("aws sts get-caller-identity --query Arn --output text 2>/dev/null")
    local region = dots.env("AWS_REGION")
    if region == "" then
      region = dots.sh("aws configure get region 2>/dev/null")
    end
    return {
      "account " .. account,
      "region  " .. (region == "" and "unset" or region),
      arn,
    }
  end,
}
