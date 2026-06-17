$ErrorActionPreference = "Stop"
Set-Location -LiteralPath $PSScriptRoot

$env:RUSTUP_HOME = Join-Path $PSScriptRoot ".rustup"
$env:CARGO_HOME = Join-Path $PSScriptRoot ".cargo"
$env:CARGO_TARGET_DIR = Join-Path $PSScriptRoot "target"
$env:NPM_CONFIG_CACHE = Join-Path $PSScriptRoot ".npm-cache"
$env:PATH = (Join-Path $env:CARGO_HOME "bin") + ";" + $env:PATH

if (-not (Test-Path -LiteralPath ".cargo\bin\cargo.exe")) {
  throw "Rust/Cargo is not installed in this project. Install rustup with RUSTUP_HOME=.rustup and CARGO_HOME=.cargo first."
}

if (-not (Test-Path -LiteralPath "node_modules\@tauri-apps\cli")) {
  npm install
}

npm run tauri build

Copy-Item -LiteralPath "target\release\mynote.exe" -Destination "Mynote.exe" -Force
Remove-Item -LiteralPath "MynoteStop.exe" -Force -ErrorAction SilentlyContinue

Write-Host "Built Tauri app:"
Write-Host (Join-Path $PSScriptRoot "Mynote.exe")

