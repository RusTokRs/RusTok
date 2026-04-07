$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..\\..")
Set-Location $root

$python = $null
foreach ($candidate in @("py", "python")) {
    $command = Get-Command $candidate -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($command -and $command.Source -notlike "*\\WindowsApps\\*") {
        $python = $command.Source
        break
    }
}

if (-not $python) {
    Write-Error "Python runtime not found. Install Python 3.11+ and ensure a real 'py' or 'python' executable is available in PATH, not only the WindowsApps stub."
}

& $python "scripts/architecture_dependency_guard.py"
