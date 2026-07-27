param(
    [string]$BinaryPath,
    [switch]$Harness
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $false
$RootDir = Split-Path -Parent $PSScriptRoot
$Installer = Join-Path $RootDir "install.ps1"

if ($Harness) {
    function Invoke-RestMethod {
        throw "network access is forbidden in installer tests"
    }

    function Invoke-WebRequest {
        param(
            [Parameter(Mandatory = $true)][string]$Uri,
            [Parameter(Mandatory = $true)][string]$OutFile,
            [switch]$UseBasicParsing
        )

        if ($env:TEST_CHECKSUM_MODE -eq "forbid") {
            throw "Invoke-WebRequest must not be called"
        }

        $Leaf = [System.IO.Path]::GetFileName(([Uri]$Uri).AbsolutePath)
        if ($Leaf -ceq $env:TEST_ASSET_NAME) {
            Copy-Item -Path $env:TEST_ASSET_FILE -Destination $OutFile -Force
            return
        }
        if ($Leaf -notlike "*-sha256sums.txt") {
            throw "unexpected URL: $Uri"
        }

        switch ($env:TEST_CHECKSUM_MODE) {
            "valid" {
                "$env:TEST_ASSET_HASH  $env:TEST_ASSET_NAME" |
                    Set-Content -Path $OutFile -Encoding ascii
            }
            "missing-sums" {
                throw "checksums unavailable"
            }
            "asset-absent" {
                "$env:TEST_ASSET_HASH  $env:TEST_ASSET_NAME.sig" |
                    Set-Content -Path $OutFile -Encoding ascii
            }
            "mismatch" {
                "$('0' * 64)  $env:TEST_ASSET_NAME" |
                    Set-Content -Path $OutFile -Encoding ascii
            }
            default {
                throw "unexpected checksum mode: $env:TEST_CHECKSUM_MODE"
            }
        }
    }

    & $Installer
    exit $LASTEXITCODE
}

if ([string]::IsNullOrWhiteSpace($BinaryPath)) {
    throw "BinaryPath is required"
}

$ResolvedBinary = (Resolve-Path $BinaryPath).Path
$TestRoot = Join-Path ([System.IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
$FixtureDir = Join-Path $TestRoot "fixture"
$PackageDir = Join-Path $FixtureDir "package"
$Version = "v0.0.0-test"
$Asset = "fimod-$Version-x86_64-pc-windows-msvc.zip"
$AssetPath = Join-Path $FixtureDir $Asset
New-Item -ItemType Directory -Path $PackageDir -Force | Out-Null
Copy-Item -Path $ResolvedBinary -Destination (Join-Path $PackageDir "fimod.exe")
Compress-Archive -Path (Join-Path $PackageDir "fimod.exe") -DestinationPath $AssetPath
$AssetHash = (Get-FileHash -Path $AssetPath -Algorithm SHA256).Hash.ToLowerInvariant()

function Invoke-InstallerCase {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$Mode,
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][bool]$ShouldSucceed,
        [Parameter(Mandatory = $true)][string]$ExpectedOutput
    )

    $CaseDir = Join-Path $TestRoot $Name
    $InstallDir = Join-Path $CaseDir "bin"
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null

    $env:TEST_INSTALLER = $Installer
    $env:TEST_ASSET_FILE = $AssetPath
    $env:TEST_ASSET_HASH = $AssetHash
    $env:TEST_ASSET_NAME = $Asset
    $env:TEST_CHECKSUM_MODE = $Mode
    $env:FIMOD_INSTALL = $InstallDir
    $env:FIMOD_VERSION = $Version
    $env:FIMOD_SOURCE = $Source
    $env:FIMOD_SETUP_ALL = "no"
    $env:FIMOD_SKIP_DOWNLOAD = $null

    $Output = & pwsh -NoProfile -File $PSCommandPath -Harness 2>&1
    $Status = $LASTEXITCODE
    $OutputText = ($Output | Out-String)
    $InstalledBinary = Join-Path $InstallDir "fimod.exe"

    if ($ShouldSucceed) {
        if ($Status -ne 0) {
            throw "$Name returned $Status`n$OutputText"
        }
        if (-not (Test-Path $InstalledBinary)) {
            throw "$Name did not install fimod.exe"
        }
    } else {
        if ($Status -eq 0) {
            throw "$Name unexpectedly succeeded`n$OutputText"
        }
        if (Test-Path $InstalledBinary) {
            throw "$Name installed an unverified binary"
        }
    }

    if (-not $OutputText.Contains($ExpectedOutput)) {
        throw "$Name output did not contain '$ExpectedOutput'`n$OutputText"
    }

    return $InstalledBinary
}

try {
    $ValidCase = @{
        Name = "valid"
        Mode = "valid"
        Source = "github"
        ShouldSucceed = $true
        ExpectedOutput = "SHA256 verified"
    }
    $InstalledBinary = Invoke-InstallerCase @ValidCase

    $Smoke = '{"a":1}' | & $InstalledBinary shape -e 'data' --output-format json-compact
    if (($Smoke | Out-String).Trim() -ne '{"a":1}') {
        throw "installed binary smoke test failed: $Smoke"
    }

    $NegativeCases = @(
        @{
            Name = "gitlab-missing-sums"
            Mode = "missing-sums"
            Source = "gitlab"
            ExpectedOutput = "could not download required checksums file"
        },
        @{
            Name = "exact-asset-required"
            Mode = "asset-absent"
            Source = "github"
            ExpectedOutput = "not found exactly once in checksums file"
        },
        @{
            Name = "mismatch"
            Mode = "mismatch"
            Source = "github"
            ExpectedOutput = "SHA256 mismatch"
        }
    )
    foreach ($Case in $NegativeCases) {
        $Case.ShouldSucceed = $false
        Invoke-InstallerCase @Case | Out-Null
    }

    $env:TEST_INSTALLER = $Installer
    $env:TEST_CHECKSUM_MODE = "forbid"
    $env:FIMOD_INSTALL = Join-Path $TestRoot "gitlab-unpinned\bin"
    $env:FIMOD_VERSION = $null
    $env:FIMOD_SOURCE = "gitlab"
    $env:FIMOD_SETUP_ALL = "no"
    $env:FIMOD_SKIP_DOWNLOAD = $null
    New-Item -ItemType Directory -Path $env:FIMOD_INSTALL -Force | Out-Null

    $UnpinnedOutput = & pwsh -NoProfile -File $PSCommandPath -Harness 2>&1
    if ($LASTEXITCODE -eq 0) {
        throw "unpinned GitLab install unexpectedly succeeded"
    }
    if (-not ($UnpinnedOutput | Out-String).Contains("requires an explicit FIMOD_VERSION")) {
        throw "unpinned GitLab install did not explain the pin requirement"
    }

    $SkipDir = Join-Path $TestRoot "skip\bin"
    New-Item -ItemType Directory -Path $SkipDir -Force | Out-Null
    Copy-Item -Path $ResolvedBinary -Destination (Join-Path $SkipDir "fimod.exe")
    $env:TEST_INSTALLER = $Installer
    $env:TEST_CHECKSUM_MODE = "forbid"
    $env:FIMOD_INSTALL = $SkipDir
    $env:FIMOD_VERSION = $null
    $env:FIMOD_SOURCE = "github"
    $env:FIMOD_SETUP_ALL = "no"
    $env:FIMOD_SKIP_DOWNLOAD = "1"

    $SkipOutput = & pwsh -NoProfile -File $PSCommandPath -Harness 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "FIMOD_SKIP_DOWNLOAD failed`n$($SkipOutput | Out-String)"
    }
    if (-not ($SkipOutput | Out-String).Contains("Skipping download (FIMOD_SKIP_DOWNLOAD=1)")) {
        throw "FIMOD_SKIP_DOWNLOAD was not reported"
    }

    Write-Host "PowerShell installer checksum and binary smoke tests passed"
} finally {
    Remove-Item -Path $TestRoot -Recurse -Force -ErrorAction SilentlyContinue
    $env:TEST_INSTALLER = $null
    $env:TEST_ASSET_FILE = $null
    $env:TEST_ASSET_HASH = $null
    $env:TEST_ASSET_NAME = $null
    $env:TEST_CHECKSUM_MODE = $null
    $env:FIMOD_INSTALL = $null
    $env:FIMOD_VERSION = $null
    $env:FIMOD_SOURCE = $null
    $env:FIMOD_SETUP_ALL = $null
    $env:FIMOD_SKIP_DOWNLOAD = $null
}
