<#
.SYNOPSIS
    Builds the release binary and refreshes dist\.

.DESCRIPTION
    Three steps: cargo build --release, the test suite, then publishing.

    Publishing copies target\release\moneytracker4.exe to dist\MoneyTracker4.exe and makes sure
    dist\data\moneytracker.db exists. An existing database is never overwritten unless you ask for
    it: it holds the real data. The first run seeds it from this project's development copy
    (data\moneytracker.db).

.PARAMETER SkipTests
    Build and publish without running cargo test.

.PARAMETER ForceDb
    Replace dist\data\moneytracker.db with the development copy (data\moneytracker.db).

.PARAMETER DbSource
    Publish a specific database file instead of the development copy. Implies -ForceDb.

.PARAMETER SkipSmokeTest
    Do not launch the published executable once to check it renders a frame.

.EXAMPLE
    .\package.ps1

.EXAMPLE
    .\package.ps1 -SkipTests

.EXAMPLE
    .\package.ps1 -DbSource 'C:\Users\me\backup\moneytracker.db'
#>
[CmdletBinding()]
param(
    [switch]$SkipTests,
    [switch]$ForceDb,
    [string]$DbSource,
    [switch]$SkipSmokeTest
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = $PSScriptRoot
$dist = Join-Path $root 'dist'
$distData = Join-Path $dist 'data'
$distExe = Join-Path $dist 'MoneyTracker4.exe'
$distDb = Join-Path $distData 'moneytracker.db'
$devDb = Join-Path $root 'data\moneytracker.db'
$builtExe = Join-Path $root 'target\release\moneytracker4.exe'

function Write-Step([string]$Text) {
    Write-Host ''
    Write-Host "==> $Text" -ForegroundColor Cyan
}

function Format-Size([long]$Bytes) {
    if ($Bytes -ge 1MB) { return '{0:N1} MB' -f ($Bytes / 1MB) }
    if ($Bytes -ge 1KB) { return '{0:N0} KB' -f ($Bytes / 1KB) }
    return "$Bytes B"
}

function Invoke-Cargo([string[]]$CargoArgs) {
    Push-Location $root
    try {
        & cargo @CargoArgs
        if ($LASTEXITCODE -ne 0) {
            throw "cargo $($CargoArgs -join ' ') failed with exit code $LASTEXITCODE."
        }
    }
    finally {
        Pop-Location
    }
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'cargo was not found on PATH. Install Rust (rustup) and try again.'
}

# ---------------------------------------------------------------- build

Write-Step 'Building the release binary'
Invoke-Cargo @('build', '--release')

if (-not (Test-Path -LiteralPath $builtExe)) {
    throw "cargo finished but $builtExe does not exist."
}

# ---------------------------------------------------------------- tests

if (-not $SkipTests) {
    Write-Step 'Running the test suite'
    Invoke-Cargo @('test')
}

# ---------------------------------------------------------------- publish

Write-Step "Publishing to $dist"
New-Item -ItemType Directory -Force -Path $distData | Out-Null

try {
    Copy-Item -LiteralPath $builtExe -Destination $distExe -Force
}
catch {
    throw "Could not replace $distExe. Close MoneyTracker4 if it is running, then try again. $($_.Exception.Message)"
}

if ($DbSource) {
    $source = (Resolve-Path -LiteralPath $DbSource).ProviderPath
    Copy-Item -LiteralPath $source -Destination $distDb -Force
    Write-Host "Database replaced from $source"
}
elseif ($ForceDb) {
    if (-not (Test-Path -LiteralPath $devDb)) {
        throw "-ForceDb was given but the development database $devDb does not exist."
    }
    Copy-Item -LiteralPath $devDb -Destination $distDb -Force
    Write-Host "Database replaced from $devDb"
}
elseif (Test-Path -LiteralPath $distDb) {
    Write-Host 'Database kept (use -ForceDb or -DbSource to replace it).'
}
elseif (Test-Path -LiteralPath $devDb) {
    Copy-Item -LiteralPath $devDb -Destination $distDb
    Write-Host "Database seeded from $devDb"
}
else {
    Write-Warning "No database at $devDb. The published app will create an empty one on first run."
}

# ---------------------------------------------------------------- smoke test

if (-not $SkipSmokeTest) {
    Write-Step 'Checking the published executable'

    # Start-Process rather than "&": Rune reports its frame timings on stderr, and PowerShell 5.1
    # turns native stderr into errors, which $ErrorActionPreference = 'Stop' would throw on.
    $temp = [System.IO.Path]::GetTempPath()
    $id = [guid]::NewGuid().ToString('N')
    $shot = Join-Path $temp "moneytracker4-smoke-$id.png"
    $outLog = Join-Path $temp "moneytracker4-smoke-$id.out.log"
    $errLog = Join-Path $temp "moneytracker4-smoke-$id.err.log"

    try {
        $proc = Start-Process -FilePath $distExe `
            -ArgumentList @('--snap', "`"$shot`"", '--theme', 'dark', '--size', '1180x760', '--view', 'dashboard') `
            -NoNewWindow -Wait -PassThru `
            -RedirectStandardOutput $outLog -RedirectStandardError $errLog

        if ($proc.ExitCode -eq 0 -and (Test-Path -LiteralPath $shot)) {
            Write-Host 'The executable rendered a frame. OK'
        }
        else {
            Write-Warning "The executable did not render a frame (exit code $($proc.ExitCode)). The build is still published."
            if (Test-Path -LiteralPath $errLog) {
                Get-Content -LiteralPath $errLog -Tail 20 | ForEach-Object { Write-Host "    $_" }
            }
        }
    }
    catch {
        Write-Warning "Could not run the published executable: $($_.Exception.Message)"
    }
    finally {
        foreach ($path in @($shot, $outLog, $errLog)) {
            Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
        }
    }
}

# ---------------------------------------------------------------- summary

Write-Step 'Done'
Get-ChildItem -LiteralPath $dist -Recurse -File |
    ForEach-Object {
        [pscustomobject]@{
            File = $_.FullName.Substring($dist.Length + 1)
            Size = Format-Size $_.Length
        }
    } |
    Format-Table -AutoSize | Out-Host

Write-Host "Run: $distExe"
Write-Host 'Set MT_DB_FILE before launching to point the app at another database.'
