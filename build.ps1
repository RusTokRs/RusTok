$ErrorActionPreference = "Stop"

Write-Host "--- RusToK Single Binary Build Script ---" -ForegroundColor Cyan

# 1. Build Admin UI
Write-Host "[1/3] Building Admin UI (Leptos CSR)..." -ForegroundColor Yellow
Set-Location "apps/admin"
if (Get-Command "trunk" -ErrorAction SilentlyContinue) {
    trunk build --release
} else {
    Write-Warning "Trunk not found. Skipping Admin UI build. Ensure 'apps/admin/dist' contains index.html."
}
Set-Location "../.."

# 2. Build Storefront Assets (if any)
Write-Host "[2/3] Preparing Storefront assets..." -ForegroundColor Yellow
# Storefront is currently integrated as a library (SSR), 
# so it's built as part of the server.

# 3. Build Backend Binary
Write-Host "[3/3] Building Server with embedded assets..." -ForegroundColor Yellow
cargo build --release -p rustok-server

$BinaryPath = "target/release/rustok-server.exe"
if (Test-Path $BinaryPath) {
    Write-Host "`nSuccess! Single binary created at: $BinaryPath" -ForegroundColor Green
    Write-Host "You can now run it from any directory. It will use SQLite by default if no DATABASE_URL is set."
} else {
    Write-Error "Build failed. Binary not found."
}
