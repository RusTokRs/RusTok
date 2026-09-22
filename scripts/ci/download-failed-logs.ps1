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

$workflowRunsJson = Invoke-GitHubCli -Arguments @(
    'api',
    "/repos/$repository/actions/runs?status=completed&per_page=$RunLimit"
)
$workflowRuns = (($workflowRunsJson -join [Environment]::NewLine) | ConvertFrom-Json).workflow_runs
$failureConclusions = @('failure', 'timed_out', 'cancelled', 'action_required', 'startup_failure', 'stale')
$selectedRun = $workflowRuns |
    Where-Object {
        $_.name -ne 'Failed workflow diagnostics' -and
        $failureConclusions -contains $_.conclusion
    } |
    Sort-Object updated_at -Descending |
    Select-Object -First 1

if (-not $selectedRun) {
    throw 'No completed failed GitHub Actions run was found.'
}

$temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) "rustok-ci-errors-$([guid]::NewGuid().ToString('N'))"
$temporaryRootFull = [System.IO.Path]::GetFullPath($temporaryRoot)
$temporaryPrefix = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())

if (-not $temporaryRootFull.StartsWith($temporaryPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to use a temporary directory outside the system temp path: $temporaryRootFull"
}

try {
    New-Item -ItemType Directory -Path $temporaryRootFull | Out-Null
    $archivePath = Join-Path $temporaryRootFull 'workflow-logs.zip'
    $logsRoot = Join-Path $temporaryRootFull 'logs'
    $githubToken = ((Invoke-GitHubCli -Arguments @('auth', 'token')) -join [Environment]::NewLine).Trim()
    if ([string]::IsNullOrWhiteSpace($githubToken)) {
        throw 'Could not read the GitHub CLI authentication token.'
    }
    $requestHeaders = @{ Authorization = "Bearer $githubToken"; Accept = 'application/vnd.github+json' }
    Invoke-WebRequest -Uri "https://api.github.com/repos/$repository/actions/runs/$($selectedRun.id)/logs" -Headers $requestHeaders -OutFile $archivePath -MaximumRedirection 5
    Expand-Archive -LiteralPath $archivePath -DestinationPath $logsRoot -Force

    $logFiles = @(Get-ChildItem -LiteralPath $logsRoot -File -Recurse)
    if ($logFiles.Count -eq 0) {
        throw 'The downloaded workflow log archive contains no files.'
    }

    if (Test-Path -LiteralPath $destinationRoot) {
        Remove-Item -LiteralPath $destinationRoot -Recurse -Force
    }
    New-Item -ItemType Directory -Path $destinationRoot | Out-Null
    foreach ($logFile in $logFiles) {
        $relativePath = $logFile.FullName.Substring($logsRoot.Length).TrimStart([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
        $flatName = $relativePath -replace '[\\/]', '__'
        Copy-Item -LiteralPath $logFile.FullName -Destination (Join-Path $destinationRoot $flatName) -Force
    }

    [ordered]@{
        source_run_id = $selectedRun.id
        source_workflow = $selectedRun.name
        source_conclusion = $selectedRun.conclusion
        source_url = $selectedRun.html_url
        source_updated_at = $selectedRun.updated_at
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $destinationRoot 'metadata.json') -Encoding utf8

    Write-Host "Downloaded logs for $($selectedRun.name) run $($selectedRun.id) to $destinationRoot"
}
finally {
    if (Test-Path -LiteralPath $temporaryRootFull) {
        Remove-Item -LiteralPath $temporaryRootFull -Recurse -Force
    }
}
