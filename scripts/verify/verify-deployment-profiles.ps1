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

function Write-Skip {
    param([string]$Label)
    Write-Host "  SKIP $Label" -ForegroundColor Yellow
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

function Get-HttpStatus {
    param(
        [string]$Method,
        [string]$Url,
        [string]$Body = $null
    )

    $arguments = @("-sS", "--max-time", "20", "-o", "NUL", "-w", "%{http_code}", "-X", $Method)
    if ($null -ne $Body) {
        $arguments += @("-H", "content-type: application/json", "--data", $Body)
    }
    $arguments += $Url

    $status = & curl.exe @arguments
    if ($LASTEXITCODE -ne 0) {
        return $null
    }

    return "$status".Trim()
}

function Save-HttpResponse {
    param(
        [string]$Method,
        [string]$Url,
        [string]$HeadersPath,
        [string]$BodyPath
    )

    $arguments = @("-sS", "--max-time", "20", "-D", $HeadersPath, "-o", $BodyPath, "-X", $Method, $Url)
    & curl.exe @arguments | Out-Null
    return $LASTEXITCODE -eq 0
}

function Test-HttpStatus {
    param(
        [string]$Method,
        [string]$Url,
        [string]$ExpectedStatus,
        [string]$Body = $null
    )

    (Get-HttpStatus -Method $Method -Url $Url -Body $Body) -eq $ExpectedStatus
}

function Test-HeaderPresent {
    param(
        [string]$Url,
        [string]$HeaderName,
        [string]$HeadersPath,
        [string]$BodyPath
    )

    if (-not (Save-HttpResponse -Method "GET" -Url $Url -HeadersPath $HeadersPath -BodyPath $BodyPath)) {
        return $false
    }

    return Select-String -Path $HeadersPath -Pattern "^${HeaderName}:" -CaseSensitive:$false -Quiet
}

function Test-BodyMatches {
    param(
        [string]$Url,
        [string]$Pattern,
        [string]$HeadersPath,
        [string]$BodyPath
    )

    if (-not (Save-HttpResponse -Method "GET" -Url $Url -HeadersPath $HeadersPath -BodyPath $BodyPath)) {
        return $false
    }

    return Select-String -Path $BodyPath -Pattern $Pattern -Quiet
}

function Test-BodyNotContains {
    param(
        [string]$Url,
        [string]$Pattern,
        [string]$HeadersPath,
        [string]$BodyPath
    )

    if (-not (Save-HttpResponse -Method "GET" -Url $Url -HeadersPath $HeadersPath -BodyPath $BodyPath)) {
        return $false
    }

    return -not (Select-String -Path $BodyPath -Pattern $Pattern -Quiet)
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

Write-Header "External registry-only smoke"

$externalBaseUrl = $env:RUSTOK_REGISTRY_BASE_URL
if ([string]::IsNullOrWhiteSpace($externalBaseUrl)) {
    Write-Skip "set RUSTOK_REGISTRY_BASE_URL=https://modules.rustok.dev to verify a deployed dedicated catalog host"
} else {
    $externalBaseUrl = $externalBaseUrl.TrimEnd("/")
    $smokeSlug = if ([string]::IsNullOrWhiteSpace($env:RUSTOK_REGISTRY_SMOKE_SLUG)) {
        "blog"
    } else {
        $env:RUSTOK_REGISTRY_SMOKE_SLUG.Trim()
    }
    $evidenceDir = $env:RUSTOK_REGISTRY_EVIDENCE_DIR
    if ([string]::IsNullOrWhiteSpace($evidenceDir)) {
        $tempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("rustok-registry-smoke-" + [System.Guid]::NewGuid().ToString("N"))
        $cleanupTempDir = $true
    } else {
        $tempDir = $evidenceDir
        $cleanupTempDir = $false
    }
    New-Item -ItemType Directory -Path $tempDir -Force | Out-Null

    try {
        if (Test-HttpStatus -Method "GET" -Url "$externalBaseUrl/health/ready" -ExpectedStatus "200") {
            Write-Pass "external /health/ready returns 200"
        } else {
            Write-Fail "external /health/ready returns 200"
        }

        if (Test-HttpStatus -Method "GET" -Url "$externalBaseUrl/health/modules" -ExpectedStatus "200") {
            Write-Pass "external /health/modules returns 200"
        } else {
            Write-Fail "external /health/modules returns 200"
        }

        if (Test-BodyMatches -Url "$externalBaseUrl/health/runtime" -Pattern '"host_mode"\s*:\s*"registry_only"' -HeadersPath (Join-Path $tempDir "runtime-headers.txt") -BodyPath (Join-Path $tempDir "runtime-body.json")) {
            Write-Pass "external /health/runtime advertises registry_only"
        } else {
            Write-Fail "external /health/runtime advertises registry_only"
        }

        if (Test-BodyMatches -Url "$externalBaseUrl/health/runtime" -Pattern '"runtime_dependencies_enabled"\s*:\s*false' -HeadersPath (Join-Path $tempDir "runtime-headers.txt") -BodyPath (Join-Path $tempDir "runtime-body.json")) {
            Write-Pass "external /health/runtime disables runtime dependencies"
        } else {
            Write-Fail "external /health/runtime disables runtime dependencies"
        }

        if (Test-HeaderPresent -Url "$externalBaseUrl/v1/catalog?limit=1" -HeaderName "etag" -HeadersPath (Join-Path $tempDir "catalog-headers.txt") -BodyPath (Join-Path $tempDir "catalog-body.json")) {
            Write-Pass "external /v1/catalog exposes ETag"
        } else {
            Write-Fail "external /v1/catalog exposes ETag"
        }

        if (Test-HeaderPresent -Url "$externalBaseUrl/v1/catalog?limit=1" -HeaderName "cache-control" -HeadersPath (Join-Path $tempDir "catalog-headers.txt") -BodyPath (Join-Path $tempDir "catalog-body.json")) {
            Write-Pass "external /v1/catalog exposes Cache-Control"
        } else {
            Write-Fail "external /v1/catalog exposes Cache-Control"
        }

        if (Test-HeaderPresent -Url "$externalBaseUrl/v1/catalog?limit=1" -HeaderName "x-total-count" -HeadersPath (Join-Path $tempDir "catalog-headers.txt") -BodyPath (Join-Path $tempDir "catalog-body.json")) {
            Write-Pass "external /v1/catalog exposes X-Total-Count"
        } else {
            Write-Fail "external /v1/catalog exposes X-Total-Count"
        }

        if (Test-HttpStatus -Method "GET" -Url "$externalBaseUrl/v1/catalog/$smokeSlug" -ExpectedStatus "200") {
            Write-Pass "external /v1/catalog/{slug} returns 200"
        } else {
            Write-Fail "external /v1/catalog/{slug} returns 200"
        }

        if (Test-BodyMatches -Url "$externalBaseUrl/api/openapi.json" -Pattern '"/v1/catalog/\{slug\}"' -HeadersPath (Join-Path $tempDir "openapi-headers.txt") -BodyPath (Join-Path $tempDir "openapi-body.json")) {
            Write-Pass "external reduced OpenAPI keeps catalog detail path"
        } else {
            Write-Fail "external reduced OpenAPI keeps catalog detail path"
        }

        if (Test-BodyMatches -Url "$externalBaseUrl/api/openapi.yaml" -Pattern '/v1/catalog/\{slug\}' -HeadersPath (Join-Path $tempDir "openapi-yaml-headers.txt") -BodyPath (Join-Path $tempDir "openapi-yaml-body.yaml")) {
            Write-Pass "external reduced OpenAPI YAML keeps catalog detail path"
        } else {
            Write-Fail "external reduced OpenAPI YAML keeps catalog detail path"
        }

        if (Test-BodyNotContains -Url "$externalBaseUrl/api/openapi.json" -Pattern '"/v2/catalog/publish"' -HeadersPath (Join-Path $tempDir "openapi-headers.txt") -BodyPath (Join-Path $tempDir "openapi-body.json")) {
            Write-Pass "external reduced OpenAPI hides V2 publish routes"
        } else {
            Write-Fail "external reduced OpenAPI hides V2 publish routes"
        }

        if (Test-BodyNotContains -Url "$externalBaseUrl/api/openapi.json" -Pattern '"/api/graphql"|"/api/auth/login"' -HeadersPath (Join-Path $tempDir "openapi-headers.txt") -BodyPath (Join-Path $tempDir "openapi-body.json")) {
            Write-Pass "external reduced OpenAPI hides GraphQL/auth routes"
        } else {
            Write-Fail "external reduced OpenAPI hides GraphQL/auth routes"
        }

        if (Test-HttpStatus -Method "POST" -Url "$externalBaseUrl/v2/catalog/publish" -ExpectedStatus "404" -Body "{}") {
            Write-Pass "external write publish path returns 404"
        } else {
            Write-Fail "external write publish path returns 404"
        }

        if (Test-HttpStatus -Method "POST" -Url "$externalBaseUrl/v2/catalog/publish/rpr_smoke/validate" -ExpectedStatus "404" -Body "{}") {
            Write-Pass "external write validate path returns 404"
        } else {
            Write-Fail "external write validate path returns 404"
        }

        if (Test-HttpStatus -Method "POST" -Url "$externalBaseUrl/v2/catalog/publish/rpr_smoke/stages" -ExpectedStatus "404" -Body "{}") {
            Write-Pass "external write stages path returns 404"
        } else {
            Write-Fail "external write stages path returns 404"
        }

        if (Test-HttpStatus -Method "POST" -Url "$externalBaseUrl/v2/catalog/publish/rpr_smoke/request-changes" -ExpectedStatus "404" -Body "{}") {
            Write-Pass "external request-changes path returns 404"
        } else {
            Write-Fail "external request-changes path returns 404"
        }

        if (Test-HttpStatus -Method "POST" -Url "$externalBaseUrl/v2/catalog/publish/rpr_smoke/hold" -ExpectedStatus "404" -Body "{}") {
            Write-Pass "external hold path returns 404"
        } else {
            Write-Fail "external hold path returns 404"
        }

        if (Test-HttpStatus -Method "POST" -Url "$externalBaseUrl/v2/catalog/publish/rpr_smoke/resume" -ExpectedStatus "404" -Body "{}") {
            Write-Pass "external resume path returns 404"
        } else {
            Write-Fail "external resume path returns 404"
        }

        if (Test-HttpStatus -Method "POST" -Url "$externalBaseUrl/v2/catalog/runner/claim" -ExpectedStatus "404" -Body "{}") {
            Write-Pass "external runner claim path returns 404"
        } else {
            Write-Fail "external runner claim path returns 404"
        }

        if (Test-HttpStatus -Method "POST" -Url "$externalBaseUrl/v2/catalog/owner-transfer" -ExpectedStatus "404" -Body "{}") {
            Write-Pass "external owner-transfer path returns 404"
        } else {
            Write-Fail "external owner-transfer path returns 404"
        }

        if (Test-HttpStatus -Method "POST" -Url "$externalBaseUrl/v2/catalog/yank" -ExpectedStatus "404" -Body "{}") {
            Write-Pass "external yank path returns 404"
        } else {
            Write-Fail "external yank path returns 404"
        }

        if (Test-HttpStatus -Method "GET" -Url "$externalBaseUrl/admin" -ExpectedStatus "404") {
            Write-Pass "external /admin returns 404"
        } else {
            Write-Fail "external /admin returns 404"
        }

        if (-not [string]::IsNullOrWhiteSpace($evidenceDir)) {
            @(
                "base_url=$externalBaseUrl"
                "smoke_slug=$smokeSlug"
                "captured_at_utc=$([DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ'))"
            ) | Set-Content -LiteralPath (Join-Path $evidenceDir "registry-smoke-metadata.txt")
            Write-Pass "external smoke evidence saved to $evidenceDir"
        }
    } finally {
        if ($cleanupTempDir) {
            Remove-Item -LiteralPath $tempDir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

Write-Host ""
if ($errors -eq 0) {
    Write-Host "All deployment profile smoke checks passed." -ForegroundColor Green
    exit 0
}

Write-Host "$errors deployment profile check(s) failed." -ForegroundColor Red
exit $errors
