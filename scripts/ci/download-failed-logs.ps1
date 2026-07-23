[CmdletBinding()]
param(
    [ValidateRange(1, 100)]
    [int]$RunLimit = 100
)

$ErrorActionPreference = 'Stop'

function Invoke-GitHubCli {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    $output = & gh @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "GitHub CLI command failed: gh $($Arguments -join ' ')"
    }

    return $output
}

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw 'GitHub CLI (gh) is required. Install it and authenticate with gh auth login.'
}

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$destinationRoot = [System.IO.Path]::GetFullPath((Join-Path $repositoryRoot 'errors'))

if (-not $destinationRoot.Equals((Join-Path $repositoryRoot 'errors'), [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to write outside the repository errors directory: $destinationRoot"
}

$repository = (Invoke-GitHubCli -Arguments @('repo', 'view', '--json', 'nameWithOwner', '--jq', '.nameWithOwner')).Trim()
if ([string]::IsNullOrWhiteSpace($repository)) {
    throw 'Could not resolve the current GitHub repository.'
}

$diagnosticRunsJson = Invoke-GitHubCli -Arguments @(
    'run', 'list',
    '--repo', $repository,
    '--workflow', 'Failed workflow diagnostics',
    '--limit', $RunLimit,
    '--json', 'databaseId,status,conclusion,updatedAt'
)
$diagnosticRuns = ($diagnosticRunsJson -join [Environment]::NewLine) | ConvertFrom-Json

$selectedRun = $null
$selectedArtifact = $null
foreach ($diagnosticRun in $diagnosticRuns) {
    if ($diagnosticRun.status -ne 'completed' -or $diagnosticRun.conclusion -ne 'success') {
        continue
    }

    $artifactsJson = Invoke-GitHubCli -Arguments @(
        'api',
        "/repos/$repository/actions/runs/$($diagnosticRun.databaseId)/artifacts?per_page=100"
    )
    $artifacts = (($artifactsJson -join [Environment]::NewLine) | ConvertFrom-Json).artifacts
    $artifact = $artifacts |
        Where-Object { $_.name -like 'ci-errors-*' -and -not $_.expired } |
        Select-Object -First 1

    if ($artifact) {
        $selectedRun = $diagnosticRun
        $selectedArtifact = $artifact
        break
    }
}

if (-not $selectedArtifact) {
    throw 'No retained CI error artifact was found. Run the failed workflow first, then wait for Failed workflow diagnostics to complete.'
}

$temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) "rustok-ci-errors-$([guid]::NewGuid().ToString('N'))"
$temporaryRootFull = [System.IO.Path]::GetFullPath($temporaryRoot)
$temporaryPrefix = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())

if (-not $temporaryRootFull.StartsWith($temporaryPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to use a temporary directory outside the system temp path: $temporaryRootFull"
}

try {
    New-Item -ItemType Directory -Path $temporaryRootFull | Out-Null
    Invoke-GitHubCli -Arguments @(
        'run', 'download', $selectedRun.databaseId,
        '--repo', $repository,
        '--name', $selectedArtifact.name,
        '--dir', $temporaryRootFull
    ) | Out-Null

    $metadata = Get-ChildItem -LiteralPath $temporaryRootFull -Filter 'metadata.json' -File -Recurse |
        Select-Object -First 1
    if (-not $metadata) {
        throw 'The downloaded artifact does not contain metadata.json.'
    }

    $bundleRoot = $metadata.Directory.FullName
    $logsRoot = Join-Path $bundleRoot 'logs'
    if (-not (Test-Path -LiteralPath $logsRoot -PathType Container)) {
        throw 'The downloaded artifact does not contain the logs directory.'
    }

    if (Test-Path -LiteralPath $destinationRoot) {
        Remove-Item -LiteralPath $destinationRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Path $destinationRoot | Out-Null
    Get-ChildItem -LiteralPath $logsRoot -Force |
        Copy-Item -Destination $destinationRoot -Recurse -Force
    Copy-Item -LiteralPath $metadata.FullName -Destination (Join-Path $destinationRoot 'metadata.json') -Force

    Write-Host "Downloaded $($selectedArtifact.name) to $destinationRoot"
}
finally {
    if (Test-Path -LiteralPath $temporaryRootFull) {
        Remove-Item -LiteralPath $temporaryRootFull -Recurse -Force
    }
}
