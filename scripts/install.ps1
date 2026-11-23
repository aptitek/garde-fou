<#
.SYNOPSIS
    Cryptographic one-liner installer for Garde-fou (`garde.exe`) on Windows.
.EXAMPLE
    irm https://github.com/aptitek/garde-fou/releases/latest/download/install.ps1 | iex
.EXAMPLE
    pwsh -File scripts/install.ps1 -FromDir .\dist -InstallDir .\bin -NoModifyPath
#>
[CmdletBinding()]
param(
    [string]$Version = "latest",
    [string]$InstallDir = "",
    [string]$Repo = "",
    [string]$FromDir = "",
    [switch]$NoModifyPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$DefaultRepo = "aptitek/garde-fou"

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    if (-not [string]::IsNullOrWhiteSpace($env:GARDE_INSTALL_DIR)) {
        $InstallDir = $env:GARDE_INSTALL_DIR
    } else {
        $InstallDir = Join-Path $HOME ".local\bin"
    }
}

function Resolve-GardeRepo {
    if (-not [string]::IsNullOrWhiteSpace($Repo)) {
        return $Repo
    }
    if (-not [string]::IsNullOrWhiteSpace($env:GARDE_REPO)) {
        return $env:GARDE_REPO
    }
    $gitCmd = Get-Command git -ErrorAction SilentlyContinue
    if ($null -ne $gitCmd) {
        try {
            $origin = (& git remote get-url origin 2>$null)
            if ($LASTEXITCODE -eq 0 -and $origin -match 'github\.com[:/]([^/]+/g[u]?arde-fou)(?:\.git)?$') {
                return $Matches[1]
            }
        } catch {
            # Fallback to default repository outside a Garde-fou checkout.
        }
    }
    return $DefaultRepo
}

function Resolve-GardeTarget {
    if (-not [string]::IsNullOrWhiteSpace($env:GARDE_TARGET)) {
        return $env:GARDE_TARGET
    }
    $arch = $env:PROCESSOR_ARCHITECTURE
    if ($arch -eq "AMD64" -or $arch -eq "x86_64") {
        return "x86_64-pc-windows-msvc"
    }
    throw "Unsupported Windows architecture '$arch' (expected AMD64 / x86_64)."
}

$ResolvedRepo = Resolve-GardeRepo
$Target = Resolve-GardeTarget
$ArchiveName = "garde-$Target.zip"

$TempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("garde-install-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $TempRoot -Force | Out-Null

try {
    $ArchivePath = Join-Path $TempRoot $ArchiveName
    $SumsPath = Join-Path $TempRoot "SHA256SUMS"

    if (-not [string]::IsNullOrWhiteSpace($FromDir)) {
        $SrcArchive = Join-Path $FromDir $ArchiveName
        $SrcSums = Join-Path $FromDir "SHA256SUMS"
        if (-not (Test-Path -LiteralPath $SrcArchive)) {
            throw "Missing offline archive: $SrcArchive"
        }
        if (-not (Test-Path -LiteralPath $SrcSums)) {
            throw "Missing offline checksum manifest: $SrcSums"
        }
        Copy-Item -LiteralPath $SrcArchive -Destination $ArchivePath -Force
        Copy-Item -LiteralPath $SrcSums -Destination $SumsPath -Force
    } else {
        if ($Version -eq "latest") {
            $BaseUrl = "https://github.com/$ResolvedRepo/releases/latest/download"
        } else {
            $Tag = if ($Version.StartsWith("v")) { $Version } else { "v$Version" }
            $BaseUrl = "https://github.com/$ResolvedRepo/releases/download/$Tag"
        }
        Write-Host "garde-install: downloading $ArchiveName from $ResolvedRepo ($Version)..."
        Invoke-WebRequest -UseBasicParsing -Uri "$BaseUrl/$ArchiveName" -OutFile $ArchivePath
        Invoke-WebRequest -UseBasicParsing -Uri "$BaseUrl/SHA256SUMS" -OutFile $SumsPath
    }

    $ExpectedHash = $null
    foreach ($line in (Get-Content -LiteralPath $SumsPath)) {
        $trimmed = $line.Trim()
        if ([string]::IsNullOrWhiteSpace($trimmed)) { continue }
        $parts = $trimmed -split '\s+', 2
        if ($parts.Count -ge 2) {
            $candidateFile = $parts[1].TrimStart('*')
            if ($candidateFile -eq $ArchiveName) {
                $ExpectedHash = $parts[0].ToLowerInvariant()
                break
            }
        }
    }

    if ([string]::IsNullOrWhiteSpace($ExpectedHash)) {
        throw "Entry for '$ArchiveName' not found in SHA256SUMS; refusing installation."
    }

    $ActualHash = (Get-FileHash -LiteralPath $ArchivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($ActualHash -ne $ExpectedHash) {
        throw "SHA-256 mismatch for $ArchiveName! Expected $ExpectedHash, got $ActualHash."
    }

    $ExtractDir = Join-Path $TempRoot "extracted"
    Expand-Archive -LiteralPath $ArchivePath -DestinationPath $ExtractDir -Force
    $ExtractedExe = Join-Path $ExtractDir "garde.exe"
    if (-not (Test-Path -LiteralPath $ExtractedExe)) {
        throw "Archive '$ArchiveName' did not contain garde.exe at root."
    }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    $DestExe = Join-Path $InstallDir "garde.exe"
    Copy-Item -LiteralPath $ExtractedExe -Destination $DestExe -Force

    if (-not $NoModifyPath) {
        $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
        $Entries = @()
        if (-not [string]::IsNullOrWhiteSpace($UserPath)) {
            $Entries = $UserPath -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
        }
        $AlreadyInPath = $false
        foreach ($entry in $Entries) {
            if ($entry.TrimEnd('\') -ieq $InstallDir.TrimEnd('\')) {
                $AlreadyInPath = $true
                break
            }
        }
        if (-not $AlreadyInPath) {
            $NewUserPath = if ([string]::IsNullOrWhiteSpace($UserPath)) { $InstallDir } else { "$InstallDir;$UserPath" }
            [Environment]::SetEnvironmentVariable("Path", $NewUserPath, "User")
            $env:Path = "$InstallDir;$env:Path"
            Write-Host "garde-install: added '$InstallDir' to User PATH."
        }
    }

    $VersionOut = (& $DestExe --version)
    if ($LASTEXITCODE -ne 0) {
        throw "Installed garde.exe failed '--version' verification."
    }
    Write-Host "garde-install: installed $VersionOut to $DestExe (SHA-256 verified)"
} finally {
    if (Test-Path -LiteralPath $TempRoot) {
        Remove-Item -LiteralPath $TempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}
