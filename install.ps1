#!/usr/bin/env pwsh
# fimod installer - https://github.com/pytgaen/fimod
#
# Usage (two-step to avoid antivirus false positives on pipe-to-execute pattern):
#   Invoke-RestMethod https://raw.githubusercontent.com/pytgaen/fimod/main/install.ps1 -OutFile "$env:TEMP\fimod-install.ps1"
#   & "$env:TEMP\fimod-install.ps1"
#
# Options (environment variables):
#   $env:FIMOD_VARIANT   standard (default) or full (includes HTTP mold loading)
#   $env:FIMOD_INSTALL   install directory (default: ~\.local\bin)
#   $env:FIMOD_VERSION   specific version to install (default: latest)
#   $env:FIMOD_SOURCE    github (default) or gitlab
#   $env:FIMOD_SKIP_DOWNLOAD  set to 1 to skip download (binary must already be installed)
#   $env:FIMOD_SETUP_REGISTRY yes=auto-setup, no=skip, unset=interactive prompt

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

if ($Variant -eq "full") {
    $Prefix = "fimod-full"
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

# -- Migrate "official" -> "examples" ----------------------------------
# If the old "official" registry points to the bundled molds URL,
# remove it and re-add as "examples" (no default, no priority).
$OldOfficialUrl = "https://github.com/pytgaen/fimod/tree/main/molds"

$CurrentUrl = ""
try {
    $CurrentUrl = (& $TargetBin registry list --output-format json 2>$null `
        | & $TargetBin shape -e 'dp_get([s for s in data if s["name"] == "official"], "0.location", "")' --output-format txt 2>$null)
} catch {}

if ($CurrentUrl -eq $OldOfficialUrl) {
    Write-Host "  Migrating registry 'official' -> 'examples'..."
    try { & $TargetBin registry remove official 2>$null } catch {}
    try { & $TargetBin registry add examples $OldOfficialUrl --default 2>$null } catch {}
    Write-Host "  Done: renamed 'official' to 'examples'"
    Write-Host ""
}

# -- Registry setup ----------------------------------------------------
# FIMOD_SETUP_REGISTRY=yes  → auto-setup (CI-friendly, no prompt)
# FIMOD_SETUP_REGISTRY=no   → skip setup (CI-friendly, no prompt)
# unset                      → interactive prompt (default)
Write-Host "-----------------------------------------------"
$SetupRegistry = $env:FIMOD_SETUP_REGISTRY
if ($SetupRegistry -eq "yes") {
    Write-Host "  Setting up registry (FIMOD_SETUP_REGISTRY=yes)..."
    & $TargetBin registry setup --yes
} elseif ($SetupRegistry -eq "no") {
    Write-Host "  Skipped registry setup (FIMOD_SETUP_REGISTRY=no)."
    Write-Host "  Run 'fimod registry setup' at any time."
} else {
    Write-Host "  Run 'fimod registry setup' to configure the example mold registry? [Y/n]"
    $Reply = Read-Host "  >"
    Write-Host ""
    if ($Reply -match '^[nN]') {
        Write-Host "  Skipped. Run 'fimod registry setup' at any time."
    } else {
        Write-Host "  Setting up registry..."
        & $TargetBin registry setup --yes
    }
}
Write-Host "-----------------------------------------------"
