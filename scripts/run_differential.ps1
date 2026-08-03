# PowerShell equivalent of scripts/run_differential.sh.
#
# The underlying fuzz-harness binary is plain Rust and already runs
# fine from PowerShell directly (`cargo run --release --bin
# fuzz-harness -- --cases N --seed S`) -- this script only automates
# the multi-seed loop the bash version does, for the same convenience
# on a machine without bash.
#
# Usage:
#   powershell -File scripts/run_differential.ps1
#   $env:CASES=100000; $env:SEEDS="1 2 3"; powershell -File scripts/run_differential.ps1

$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

$Cases = if ($env:CASES) { $env:CASES } else { "50000" }
$Seeds = if ($env:SEEDS) { $env:SEEDS -split '\s+' } else { @("1", "7", "42", "99", "555", "1337", "2026", "8888", "31337", "99999") }
$Log = "fuzz/differential-multiseed.log"
$Bin = ".\target\release\fuzz-harness.exe"

if (-not (Test-Path "vendor\node-semver")) {
    Write-Error "vendor/node-semver missing -- run: powershell -File scripts/fetch_original.ps1"
    exit 2
}

cargo build --release --bin fuzz-harness
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$rustcVersion = (rustc --version)
$nodeVersion = (node --version)
$dateStart = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

$header = @"
semver-rs differential session
date:   $dateStart
rustc:  $rustcVersion
node:   $nodeVersion
oracle: vendor/node-semver, run live
cases:  $Cases per seed
seeds:  $($Seeds -join ' ')

Both implementations receive the same generated input; any
disagreement is a real behavioural divergence, not a guess.
==================================================================
"@
# Set-Content -Encoding utf8NoBOM only exists in PowerShell 7+ (pwsh).
# Windows PowerShell 5.1 (the default `powershell.exe`) throws
# "Cannot convert value utf8NoBOM..." and aborts before the fuzzer
# ever runs. .NET's UTF8Encoding($false) writes BOM-less UTF-8 on
# both 5.1 and 7+, so use that directly instead.
[System.IO.File]::WriteAllText((Join-Path $Root $Log), $header, (New-Object System.Text.UTF8Encoding $false))

$total = 0
$failed = $false

foreach ($seed in $Seeds) {
    Write-Host "==> seed $seed"
    Add-Content -Path $Log -Value "`n=== seed $seed ==="

    & $Bin --cases $Cases --seed $seed *>> $Log
    if ($LASTEXITCODE -eq 0) {
        Write-Host "    agreed on all $Cases cases"
    } else {
        Write-Host "    DIVERGENCES -- see $Log"
        $failed = $true
    }
    $total += [int]$Cases
}

$dateEnd = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
Add-Content -Path $Log -Value "`n==================================================================`ntotal cases: $total`nsession end: $dateEnd"

Write-Host ""
Write-Host "$total cases; log in $Log"
if ($failed) {
    exit 1
} else {
    Write-Host "The port and the original agreed on every case."
}
