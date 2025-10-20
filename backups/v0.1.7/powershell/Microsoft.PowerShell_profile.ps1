Invoke-Expression (&starship init powershell)

## show options menu
Set-PSReadlineKeyHandler -Key Tab -Function MenuComplete
