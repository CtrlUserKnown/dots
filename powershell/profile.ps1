# run this line to set the execution policy for the current user to RemoteSigned before using this profile
#
# > Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser -Force

# initialize starship prompt
Invoke-Expression (&starship init powershell)

# show options menu
Set-PSReadlineKeyHandler -Key Tab -Function MenuComplete
Set-PSReadlineKeyHandler -Key Shift+Tab -Function MenuCompleteBackwards

# enable syntax highlighting
Import-Module PSReadLine

# set colors for syntax highlighting to rose-pine-themed colors
Set-PSReadLineOption -Colors @{
        "Command" = "#ebbcba"
        "Parameter" = "#31748f"
        "String" = "#9ccfd8"
        "Number" = "#f6c177"
        "Operator" = "#c4a7e7"
        "Type" = "#f6c177"
        "Variable" = "#ebbcba"
}

# enable vi mode
Set-PSReadLineOption -EditMode Vi

