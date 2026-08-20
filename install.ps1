#!/usr/bin/env pwsh
# fimod installer - https://github.com/pytgaen/fimod
#
# Usage (two-step to avoid antivirus false positives on pipe-to-execute pattern):
#   Invoke-RestMethod https://raw.githubusercontent.com/pytgaen/fimod/main/install.ps1 -OutFile "$env:TEMP\fimod-install.ps1"
#   & "$env:TEMP\fimod-install.ps1"
#
# Options (environment variables):
#   $env:FIMOD_VARIANT   standard (default), slim (without HTTP), or fast (speed optimized)
#   $env:FIMOD_SET_DEFAULT yes=also install slim/fast as the default `fimod` command, no=skip, unset=interactive prompt
#   $env:FIMOD_INSTALL   install directory (default: ~\.local\bin)
#   $env:FIMOD_VERSION   specific version to install (default: latest)
#   $env:FIMOD_SOURCE    github (default) or gitlab
#   $env:FIMOD_SKIP_DOWNLOAD  set to 1 to skip download (binary must already be installed)
#   $env:FIMOD_SETUP_REGISTRY yes=setup registries, no=skip, unset=prompt if needed
#   $env:FIMOD_SETUP_SANDBOX  yes=setup sandbox, no=skip, unset=prompt if needed
#   $env:FIMOD_SETUP_ALL      yes|no default for both when granulars are unset

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

switch ($Variant) {
    "standard" {
        $Prefix = "fimod"
        $BinBase = "fimod"
    }
    "slim" {
        $Prefix = "fimod-slim"
        $BinBase = "fimod-slim"
    }
    "fast" {
        $Prefix = "fimod-fast"
        $BinBase = "fimod-fast"
    }
    default {
        Write-Error "Error: unsupported FIMOD_VARIANT=$Variant`nSupported variants: standard, slim, fast"
        exit 1
    }
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

$DownloadTag = $null
$Version = $env:FIMOD_VERSION
if ($env:FIMOD_SKIP_DOWNLOAD -eq "1") {
    # Binary already installed - skip version resolution and all network access.
    $Version = "(skip)"
} elseif ([string]::IsNullOrWhiteSpace($Version)) {
    if ($Source -eq "gitlab") {
        [Console]::Error.WriteLine("Error: FIMOD_SOURCE=gitlab requires an explicit FIMOD_VERSION because the mirror may lag behind GitHub")
        exit 1
    }
    Write-Host "Fetching latest version..."
    if ($Source -ne "gitlab") {
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

$BinName = "$BinBase.exe"
$CanonicalBin = "fimod.exe"
$TargetBin = Join-Path $InstallDir $BinName
$CanonicalTarget = Join-Path $InstallDir $CanonicalBin
$DefaultInstalled = $false

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
        try {
            Invoke-WebRequest -Uri $Url -OutFile $TmpZip -UseBasicParsing
        } catch {
            [Console]::Error.WriteLine("Error: download failed - check that version $Version exists")
            [Console]::Error.WriteLine("Available releases: $BaseUrl")
            exit 1
        }

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
        } catch {
            [Console]::Error.WriteLine("Error: could not download required checksums file: $SumsUrl")
            exit 1
        }

        $AssetName = [System.IO.Path]::GetFileName($Asset)
        $ChecksumEntries = @(
            Get-Content $TmpSums | ForEach-Object {
                $Parts = $_.Trim() -split '\s+', 2
                if ($Parts.Count -eq 2) {
                    $EntryName = $Parts[1].TrimStart([char]'*')
                    if ($EntryName -ceq $AssetName) {
                        $Parts[0]
                    }
                }
            }
        )
        if ($ChecksumEntries.Count -ne 1) {
            [Console]::Error.WriteLine("Error: asset $AssetName not found exactly once in checksums file")
            exit 1
        }

        $Expected = ([string]$ChecksumEntries[0]).ToLowerInvariant()
        if ($Expected -notmatch '^[0-9a-f]{64}$') {
            [Console]::Error.WriteLine("Error: invalid SHA256 checksum for $AssetName")
            exit 1
        }

        $Actual = (Get-FileHash -Path $TmpZip -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($Actual -ne $Expected) {
            [Console]::Error.WriteLine("SHA256 mismatch!")
            [Console]::Error.WriteLine("  expected: $Expected")
            [Console]::Error.WriteLine("  got:      $Actual")
            exit 1
        }
        Write-Host "SHA256 verified"

        # Use Expand-Archive for zip
        Expand-Archive -Path $TmpZip -DestinationPath $TmpDir -Force

        $ExtractedBin = Join-Path $TmpDir $BinName
        if (-not (Test-Path -Path $ExtractedBin)) {
            $FallbackBin = Join-Path $TmpDir $CanonicalBin
            if (Test-Path -Path $FallbackBin) {
                # Backward compatibility for older slim archives that contained fimod.exe.
                $ExtractedBin = $FallbackBin
            }
        }
        if (-not (Test-Path -Path $ExtractedBin)) {
            Write-Error "Error: archive did not contain $BinName"
            exit 1
        }

        if (Test-Path -Path $TargetBin) {
            Remove-Item -Path $TargetBin -Force
        }

        Move-Item -Path $ExtractedBin -Destination $TargetBin -Force
    } finally {
        Remove-Item -Path $TmpDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# -- Optional default command copy for slim/fast ------------------------

function Resolve-DefaultPref {
    if ($env:FIMOD_SET_DEFAULT -eq "yes" -or $env:FIMOD_SET_DEFAULT -eq "no") {
        return $env:FIMOD_SET_DEFAULT
    }
    return "ask"
}

function Copy-AsDefault {
    Copy-Item -Path $TargetBin -Destination $CanonicalTarget -Force
    $script:DefaultInstalled = $true
}

if ($Variant -ne "standard") {
    $DefaultPref = Resolve-DefaultPref
    switch ($DefaultPref) {
        "yes" {
            Copy-AsDefault
        }
        "no" {}
        default {
            if ([Environment]::UserInteractive) {
                Write-Host ""
                Write-Host "Install the $Variant variant as the default 'fimod' command too? [y/N]"
                $Reply = Read-Host "  >"
                if ($Reply -match '^[yY]') {
                    Copy-AsDefault
                }
            }
        }
    }
}

# -- Verify ------------------------------------------------------------

Write-Host ""
Write-Host "$BinBase installed to $TargetBin"
try {
    $Installed = & $TargetBin --version
    Write-Host "   $Installed"
} catch {
    Write-Host "   (Installed, but could not run --version)"
}

if ($DefaultInstalled) {
    Write-Host "fimod installed to $CanonicalTarget"
    try {
        $DefaultVersion = & $CanonicalTarget --version
        Write-Host "   $DefaultVersion"
    } catch {
        Write-Host "   (Installed, but could not run --version)"
    }
}

$PathDirs = ($env:PATH -split ';') | ForEach-Object { $_.TrimEnd('\') }
$InstallDirNorm = $InstallDir.TrimEnd('\')

if ($PathDirs -notcontains $InstallDirNorm) {
    Write-Host ""
    Write-Host "WARNING: $InstallDir is not in your PATH. Add it permanently:"
    Write-Host "   [Environment]::SetEnvironmentVariable('PATH', '$InstallDir;' + `$env:PATH, 'User')"
    Write-Host "   And for this session:"
    Write-Host "   `$env:PATH = `"$InstallDir;`$env:PATH`""
}

Write-Host ""

# -- Post-install setup (registry + sandbox) ---------------------------

Write-Host "-----------------------------------------------"
Write-Host "Post-install setup"
& $TargetBin setup all defaults --if-needed
if ($LASTEXITCODE -ne 0) {
    Write-Host "Warning: post-install setup did not complete." -ForegroundColor Yellow
    Write-Host "Run '$BinBase setup all defaults --if-needed' later to configure registries and sandbox." -ForegroundColor Yellow
}
$global:LASTEXITCODE = 0
Write-Host "-----------------------------------------------"
