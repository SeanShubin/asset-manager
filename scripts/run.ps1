# Launch asset-manager pointed at the seans-assets sibling directory.

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Split-Path -Parent $ScriptDir
$DataDir = Join-Path $RepoRoot "seans-assets"

if (-not (Test-Path $DataDir)) {
    New-Item -ItemType Directory -Path $DataDir -Force | Out-Null
    Write-Host "Created data directory: $DataDir"
}

Push-Location $RepoRoot
try {
    cargo run -- $DataDir
} finally {
    Pop-Location
}
