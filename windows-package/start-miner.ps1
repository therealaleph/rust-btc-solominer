# Bitcoin Solo Miner PowerShell Launcher
Write-Host "Starting Bitcoin Solo Miner..." -ForegroundColor Green
Write-Host ""
Write-Host "Make sure you have configured your wallet address in config.ini" -ForegroundColor Yellow
Write-Host ""
Write-Host "Press any key to continue..." -ForegroundColor Cyan
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")

try {
    & ".\bitcoin-solo-miner.exe"
} catch {
    Write-Host "Error starting miner: $_" -ForegroundColor Red
}

Write-Host ""
Write-Host "Press any key to exit..." -ForegroundColor Cyan
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
