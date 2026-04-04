param()

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$rootDir = (Resolve-Path (Join-Path $scriptDir "..\..")).Path
Set-Location $rootDir

$errors = 0

function Write-Header {
    param([string]$Title)
    Write-Host ""
    Write-Host "=== $Title ===" -ForegroundColor Cyan
}

function Write-Pass {
    param([string]$Label)
    Write-Host "  PASS $Label" -ForegroundColor Green
}

function Write-Fail {
    param([string]$Label)
    Write-Host "  FAIL $Label" -ForegroundColor Red
    $script:errors++
}

function Invoke-Check {
    param(
        [string]$Label,
        [string[]]$Command
    )

    Write-Host "  > $($Command -join ' ')"
    & $Command[0] $Command[1..($Command.Length - 1)]
    if ($LASTEXITCODE -eq 0) {
        Write-Pass $Label
    } else {
        Write-Fail $Label
    }
}

Write-Header "Deployment profile smoke validation"

Invoke-Check "monolith cargo check" @(
    "cargo", "check", "--manifest-path", "$rootDir\Cargo.toml", "-p", "rustok-server", "--lib", "--bins"
)

Invoke-Check "monolith startup smoke" @(
    "cargo", "test", "--manifest-path", "$rootDir\Cargo.toml", "-p", "rustok-server",
    "app::tests::startup_smoke_builds_router_and_runtime_shared_state", "--lib"
)

Invoke-Check "server+admin cargo check" @(
    "cargo", "check", "--manifest-path", "$rootDir\Cargo.toml", "-p", "rustok-server", "--lib", "--bins",
    "--no-default-features", "--features", "redis-cache,embed-admin"
)

Invoke-Check "server+admin router smoke" @(
    "cargo", "test", "--manifest-path", "$rootDir\Cargo.toml", "-p", "rustok-server",
    "services::app_router::tests::mount_application_shell_supports_server_with_admin_profile", "--lib",
    "--no-default-features", "--features", "redis-cache,embed-admin"
)

Invoke-Check "headless-api cargo check" @(
    "cargo", "check", "--manifest-path", "$rootDir\Cargo.toml", "-p", "rustok-server", "--lib", "--bins",
    "--no-default-features", "--features", "redis-cache"
)

Invoke-Check "headless-api router smoke" @(
    "cargo", "test", "--manifest-path", "$rootDir\Cargo.toml", "-p", "rustok-server",
    "services::app_router::tests::mount_application_shell_skips_admin_and_storefront_for_headless_profile", "--lib",
    "--no-default-features", "--features", "redis-cache"
)

Invoke-Check "registry-only env override parse" @(
    "cargo", "test", "--manifest-path", "$rootDir\Cargo.toml", "-p", "rustok-server",
    "common::settings::tests::env_overrides_runtime_host_mode", "--lib",
    "--no-default-features", "--features", "redis-cache"
)

Invoke-Check "registry-only runtime smoke" @(
    "cargo", "test", "--manifest-path", "$rootDir\Cargo.toml", "-p", "rustok-server",
    "app::tests::registry_only_host_mode_limits_exposed_surface", "--lib",
    "--no-default-features", "--features", "redis-cache"
)

Invoke-Check "registry v1 detail smoke" @(
    "cargo", "test", "--manifest-path", "$rootDir\Cargo.toml", "-p", "rustok-server",
    "app::tests::registry_catalog_detail_endpoint_serves_module_contract", "--lib",
    "--no-default-features", "--features", "redis-cache"
)

Invoke-Check "registry v1 cache smoke" @(
    "cargo", "test", "--manifest-path", "$rootDir\Cargo.toml", "-p", "rustok-server",
    "app::tests::registry_catalog_endpoint_honors_if_none_match", "--lib",
    "--no-default-features", "--features", "redis-cache"
)

Invoke-Check "registry-only openapi smoke" @(
    "cargo", "test", "--manifest-path", "$rootDir\Cargo.toml", "-p", "rustok-server",
    "controllers::swagger::tests::registry_only_openapi_filters_non_registry_surface", "--lib",
    "--no-default-features", "--features", "redis-cache"
)

Write-Host ""
if ($errors -eq 0) {
    Write-Host "All deployment profile smoke checks passed." -ForegroundColor Green
    exit 0
}

Write-Host "$errors deployment profile check(s) failed." -ForegroundColor Red
exit $errors
