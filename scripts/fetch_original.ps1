# PowerShell equivalent of scripts/fetch_original.sh.
#
# Added after a real Windows run showed the bash version doesn't work
# out of the box there: no WSL distribution installed, and no Git Bash
# on PATH, are both default states on a fresh Windows machine (see
# DECISIONS.md for the full writeup of what that run surfaced). This
# script does the same job without needing either.
#
# Usage:
#   powershell -File scripts/fetch_original.ps1
#
# Requires: git, node (both already required by the rest of this
# project; nothing extra to install for this script specifically).

$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
$Repo = "https://github.com/npm/node-semver"
$Vendor = Join-Path $Root "vendor\node-semver"

Write-Host "==> Cloning $Repo"
if (Test-Path $Vendor) { Remove-Item -Recurse -Force $Vendor }
New-Item -ItemType Directory -Force -Path (Join-Path $Root "vendor") | Out-Null
git clone --depth 1 -q $Repo $Vendor
if ($LASTEXITCODE -ne 0) { throw "git clone failed" }

Write-Host "==> Pinning the original's sources and test suite"
$TestsOriginal = Join-Path $Root "tests\original"
if (Test-Path $TestsOriginal) { Remove-Item -Recurse -Force $TestsOriginal }
New-Item -ItemType Directory -Force -Path $TestsOriginal | Out-Null
Copy-Item -Recurse -Force (Join-Path $Vendor "test\*") $TestsOriginal

foreach ($d in @("internal", "functions", "classes", "ranges")) {
    $src = Join-Path $Vendor $d
    if (Test-Path $src) {
        Copy-Item -Recurse -Force $src (Join-Path $TestsOriginal "js-$d")
    }
}
$indexJs = Join-Path $Vendor "index.js"
if (Test-Path $indexJs) {
    Copy-Item -Force $indexJs $TestsOriginal
}

Set-Location $Root

# Reproduce the exact format `sha256sum` writes -- lowercase hex, two
# spaces, forward-slash relative paths, sorted -- so kickoff.hash is
# byte-identical to what the bash script and CI both produce.
# Get-FileHash alone does NOT match this: its default output uses
# backslash paths and a different column order, which would make this
# script's hash file look like a diff against Linux/macOS-generated
# ones even when the underlying files are identical.
$files = Get-ChildItem -Recurse -Path $TestsOriginal -Filter "*.js" |
    ForEach-Object {
        $relPath = $_.FullName.Substring($Root.Length + 1).Replace('\', '/')
        $hash = (Get-FileHash -Path $_.FullName -Algorithm SHA256).Hash.ToLower()
        "$hash  $relPath"
    } | Sort-Object

# -Encoding utf8 writes a UTF-8 BOM on Windows PowerShell 5.1 (a
# well-known quirk), which corrupts the first line of kickoff.hash for
# any tool that reads it byte-for-byte. UTF8Encoding($false) avoids
# the BOM on both 5.1 and 7+.
[System.IO.File]::WriteAllText(
    (Join-Path $Root "kickoff.hash"),
    (($files -join "`r`n") + "`r`n"),
    (New-Object System.Text.UTF8Encoding $false)
)

$fileCount = $files.Count
Write-Host "    $fileCount files pinned"
$fixtureCount = (Get-ChildItem -Path (Join-Path $TestsOriginal "fixtures") -Filter "*.js" -ErrorAction SilentlyContinue).Count
Write-Host "    $fixtureCount fixture files"

Write-Host "==> Exporting fixtures to JSON"
node scripts/export_fixtures.js | Select-Object -Last 3

Write-Host ""
Write-Host "Commit kickoff.hash in your first commit. Judges compare it against the"
Write-Host "suite at submission to confirm no test file was edited."
Write-Host ""
Write-Host "Next:"
Write-Host "  cargo test --test fixtures -- --nocapture"
Write-Host "  cargo run --release --bin fuzz-harness -- --cases 50000"
