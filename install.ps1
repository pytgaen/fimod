#!/usr/bin/env pwsh
# fimod installer - https://github.com/pytgaen/fimod
#
# Usage (two-step to avoid antivirus false positives on pipe-to-execute pattern):
#   Invoke-RestMethod https://raw.githubusercontent.com/pytgaen/fimod/main/install.ps1 -OutFile "$env:TEMP\fimod-install.ps1"
#   & "$env:TEMP\fimod-install.ps1"
#
# Options (environment variables):
#   $env:FIMOD_VARIANT   standard (default, includes HTTP mold loading) or slim (without HTTP)
#   $env:FIMOD_INSTALL   install directory (default: ~\.local\bin)
#   $env:FIMOD_VERSION   specific version to install (default: latest)
#   $env:FIMOD_SOURCE    github (default) or gitlab
#   $env:FIMOD_SKIP_DOWNLOAD  set to 1 to skip download (binary must already be installed)
#   $env:FIMOD_SETUP_REGISTRY yes=auto-setup registries, no=skip, unset=fall through
#   $env:FIMOD_SETUP_SANDBOX  yes=auto-setup sandbox, no=skip, unset=fall through (fimod >= 0.5.0)
#   $env:FIMOD_SETUP_ALL      yes|no default for both when granulars are unset; unset=interactive prompt

$ErrorActionPreference = "Stop"

$Repo = "pytgaen/fimod"
$Source = $env:FIMOD_SOURCE
if ([string]::IsNullOrWhiteSpace($Source)) { $Source = "github" }

$GlProjectPath = "pytgaen-group%2Ffimod"
$GlPkgBase     = "https://gitlab.com/api/v4/projects/$GlProjectPath/packages/generic/fimod"

if ($Source -eq "gitlab") {
    $BaseUrl = $GlPkgBase
} else {
    $BaseUrl = "https://github.com/$Repo/releases"
}

$Variant = $env:FIMOD_VARIANT
if ([string]::IsNullOrWhiteSpace($Variant)) {
    $Variant = "standard"
}

# -- Detect platform --------------------------------------------------

$OsName = "windows"
$Architecture = $env:PROCESSOR_ARCHITECTURE
if ($Architecture -eq "AMD64" -or $Architecture -eq "IA64") {
    $Arch = "x86_64"
} elseif ($Architecture -eq "ARM64") {
    $Arch = "aarch64"
} else {
    $Arch = "unsupported"
}

# -- Map to Rust target triple -----------------------------------------

if ($Arch -eq "x86_64") {
    $Target = "x86_64-pc-windows-msvc"
    $Ext = "zip"
} else {
    Write-Error "Error: no pre-built binary for Windows/$Arch`nBuild from source: cargo install --git https://github.com/$Repo"
    exit 1
}

# -- Resolve version ---------------------------------------------------

$Version = $env:FIMOD_VERSION
$DownloadTag = $null
if ([string]::IsNullOrWhiteSpace($Version)) {
    Write-Host "Fetching latest version..."
    if ($Source -eq "gitlab") {
        try {
            $Version = (Invoke-RestMethod -Uri "$GlPkgBase/latest/VERSION" -UseBasicParsing).Trim()
            $DownloadTag = $Version
        } catch {
            Write-Error "Error: could not fetch latest version from GitLab"
            exit 1
        }
    } else {
        # Try 1: GitHub's stable-release redirect (works for non-pre-releases)
        try {
            $Version = (Invoke-RestMethod -Uri "$BaseUrl/latest/download/VERSION" -UseBasicParsing).Trim()
            $DownloadTag = $Version
        } catch {
            $Version = $null
        }
        if ([string]::IsNullOrWhiteSpace($Version)) {
            # Try 2: direct "latest" tag (works when the release tag is literally "latest")
            try {
                $Version = (Invoke-RestMethod -Uri "$BaseUrl/download/latest/VERSION" -UseBasicParsing).Trim()
                $DownloadTag = "latest"
            } catch {
                $Version = $null
            }
        }
        if ([string]::IsNullOrWhiteSpace($Version)) {
            Write-Host "(trying GitHub API...)" -ForegroundColor DarkGray
            # Try 3: API - may be rate-limited for anonymous requests (60 req/h)
            try {
                $Releases = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases" -UseBasicParsing
                $DownloadTag = $Releases[0].tag_name
                $Version = (Invoke-RestMethod -Uri "$BaseUrl/download/$DownloadTag/VERSION" -UseBasicParsing).Trim()
            } catch {
                Write-Error "Error: could not fetch latest version from GitHub"
                exit 1
            }
        }
    }
} else {
    $DownloadTag = $Version
}

Write-Host "Installing fimod $Version ($Variant) for $OsName/$Arch..."

# -- Build asset name --------------------------------------------------

if ($Variant -eq "slim") {
    $Prefix = "fimod-slim"
} else {
    $Prefix = "fimod"
}

$Asset = "$Prefix-$Version-$Target.$Ext"
if ($Source -eq "gitlab") {
    $Url = "$GlPkgBase/$Version/$Asset"
} else {
    $Url = "$BaseUrl/download/$DownloadTag/$Asset"
}

# -- Choose install directory -------------------------------------------

$InstallDir = $env:FIMOD_INSTALL
if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $InstallDir = Join-Path $env:USERPROFILE ".local\bin"
}
if (-not (Test-Path -Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

# -- Download and install -----------------------------------------------

$BinName = "fimod.exe"
$TargetBin = Join-Path $InstallDir $BinName

if ($env:FIMOD_SKIP_DOWNLOAD -eq "1") {
    Write-Host "Skipping download (FIMOD_SKIP_DOWNLOAD=1)"
    if (-not (Test-Path -Path $TargetBin)) {
        Write-Error "Error: $TargetBin not found - cannot skip download"
        exit 1
    }
} else {
    $TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
    New-Item -ItemType Directory -Path $TmpDir -Force | Out-Null

    try {
        Write-Host "Downloading $Url..."
        $TmpZip = Join-Path $TmpDir $Asset
        Invoke-WebRequest -Uri $Url -OutFile $TmpZip -UseBasicParsing

        # -- SHA256 verification --
        $SumsFile = "fimod-$Version-sha256sums.txt"
        if ($Source -eq "gitlab") {
            $SumsUrl = "$GlPkgBase/$Version/$SumsFile"
        } else {
            $SumsUrl = "$BaseUrl/download/$DownloadTag/$SumsFile"
        }

        $TmpSums = Join-Path $TmpDir $SumsFile
        try {
            Invoke-WebRequest -Uri $SumsUrl -OutFile $TmpSums -UseBasicParsing
            $AssetName = [System.IO.Path]::GetFileName($Asset)
            $Expected = (Get-Content $TmpSums | Where-Object { $_ -match $AssetName }) -replace '\s+.*$',''
            if ($Expected) {
                $Actual = (Get-FileHash -Path $TmpZip -Algorithm SHA256).Hash.ToLower()
                if ($Actual -ne $Expected) {
                    Write-Error "SHA256 mismatch!`n  expected: $Expected`n  got:      $Actual"
                    exit 1
                }
                Write-Host "SHA256 verified"
            } else {
                Write-Host "Warning: asset not found in checksums file, skipping verification" -ForegroundColor Yellow
            }
        } catch {
            Write-Host "Warning: could not download checksums file, skipping verification" -ForegroundColor Yellow
        }

        # Use Expand-Archive for zip
        Expand-Archive -Path $TmpZip -DestinationPath $TmpDir -Force

        $ExtractedBin = Join-Path $TmpDir $BinName

        if (Test-Path -Path $TargetBin) {
            Remove-Item -Path $TargetBin -Force
        }

        Move-Item -Path $ExtractedBin -Destination $TargetBin -Force
    } catch {
        Write-Error "Error: download failed - check that version $Version exists`nAvailable releases: $BaseUrl"
        exit 1
    } finally {
        Remove-Item -Path $TmpDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# -- Verify ------------------------------------------------------------

Write-Host ""
Write-Host "fimod installed to $TargetBin"

$PathDirs = ($env:PATH -split ';') | ForEach-Object { $_.TrimEnd('\') }
$InstallDirNorm = $InstallDir.TrimEnd('\')

if ($PathDirs -notcontains $InstallDirNorm) {
    Write-Host ""
    Write-Host "WARNING: $InstallDir is not in your PATH. Add it permanently:"
    Write-Host "   [Environment]::SetEnvironmentVariable('PATH', '$InstallDir;' + `$env:PATH, 'User')"
    Write-Host "   And for this session:"
    Write-Host "   `$env:PATH = `"$InstallDir;`$env:PATH`""
} else {
    try {
        $Installed = & $TargetBin --version
        Write-Host "   $Installed"
    } catch {
        Write-Host "   (Installed, but could not run --version)"
    }
}

Write-Host ""

# -- Post-install setup (registry + sandbox) ---------------------------
#
# Two independent blocks: registry (community molds) and sandbox (policy file).
# Each resolves its preference in order:
#   1. FIMOD_SETUP_<CAT>=yes|no   (granular, wins over the rest)
#   2. FIMOD_SETUP_ALL=yes|no     (default for both when granular unset)
#   3. interactive prompt
#
# The command path depends on the installed fimod version:
#   >= 0.5.0  -> `fimod setup registry defaults` and `fimod setup sandbox defaults`
#   <  0.5.0  -> only `fimod registry setup` (sandbox unavailable)

$VersionOutput = ""
try { $VersionOutput = (& $TargetBin --version) 2>$null } catch { $VersionOutput = "" }
$VersionMatch = [regex]::Match($VersionOutput, '(\d+)\.(\d+)\.(\d+)')
if ($VersionMatch.Success) {
    $InstalledVersion = $VersionMatch.Value
    $InstalledNum = ([int]$VersionMatch.Groups[1].Value * 10000) +
                    ([int]$VersionMatch.Groups[2].Value * 100) +
                    ([int]$VersionMatch.Groups[3].Value)
} else {
    $InstalledVersion = "0.0.0"
    $InstalledNum = 0
}

if ($InstalledNum -ge 500) {
    $RegistryCmdArgs = @("setup", "registry", "defaults")
    $RegistryHint    = "fimod setup registry defaults"
    $SandboxAvailable = $true
} else {
    $RegistryCmdArgs = @("registry", "setup")
    $RegistryHint    = "fimod registry setup"
    $SandboxAvailable = $false
}

function Resolve-SetupPref([string]$specific) {
    if ($specific -eq "yes" -or $specific -eq "no") { return $specific }
    $all = $env:FIMOD_SETUP_ALL
    if ($all -eq "yes" -or $all -eq "no") { return $all }
    return "ask"
}

$RegPref = Resolve-SetupPref $env:FIMOD_SETUP_REGISTRY
$SbPref  = Resolve-SetupPref $env:FIMOD_SETUP_SANDBOX

Write-Host "-----------------------------------------------"
Write-Host "Registry"
switch ($RegPref) {
    "yes" {
        Write-Host "  Installing community registries..."
        & $TargetBin @RegistryCmdArgs --yes
    }
    "no" {
        Write-Host "  Skipped. Run '$RegistryHint' at any time."
    }
    default {
        Write-Host "  Install community registries? [Y/n]"
        $Reply = Read-Host "  >"
        if ($Reply -match '^[nN]') {
            Write-Host "  Skipped. Run '$RegistryHint' at any time."
        } else {
            & $TargetBin @RegistryCmdArgs --yes
        }
    }
}

Write-Host ""
Write-Host "Sandbox"
if (-not $SandboxAvailable) {
    if ($SbPref -eq "yes") {
        Write-Host "  Requires fimod >= 0.5.0 (installed $InstalledVersion) - skipped."
    } else {
        Write-Host "  Requires fimod >= 0.5.0 (installed $InstalledVersion)."
    }
} else {
    switch ($SbPref) {
        "yes" {
            Write-Host "  Installing recommended sandbox policy..."
            & $TargetBin setup sandbox defaults --yes
        }
        "no" {
            Write-Host "  Skipped. Run 'fimod setup sandbox defaults' at any time."
        }
        default {
            Write-Host "  Install recommended sandbox policy? [Y/n]"
            $Reply = Read-Host "  >"
            if ($Reply -match '^[nN]') {
                Write-Host "  Skipped. Run 'fimod setup sandbox defaults' at any time."
            } else {
                & $TargetBin setup sandbox defaults --yes
            }
        }
    }
}
Write-Host "-----------------------------------------------"
