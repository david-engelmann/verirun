<#
.SYNOPSIS
Install a prebuilt verirun release binary with SHA-256 verification.

.DESCRIPTION
Downloads the Windows release zip and SHA256SUMS from GitHub Releases,
verifies the archive SHA-256 digest before extraction, and installs
verirun.exe into the destination directory. The staged binary is
version-checked before it replaces any existing install, so a bad
download leaves a working install standing.

.PARAMETER Version
Release tag to install, as vX.Y.Z or X.Y.Z. Defaults to the latest
release when omitted.

.PARAMETER InstallDir
Destination directory. Defaults to $HOME/.local/bin.

.PARAMETER BaseUrl
Release root URL. Defaults to the verirun GitHub Releases root. A
local directory holding the release layout also works, which makes
the script testable offline.

.EXAMPLE
./install.ps1 -Version v0.2.2 -InstallDir "$HOME/.local/bin"

.NOTES
Environment fallbacks: VERIRUN_VERSION, INSTALL_DIR, and
VERIRUN_BASE_URL supply the defaults when the matching parameter is
omitted. When the script is downloaded and run in memory, pass the
settings through those environment variables.
#>

[CmdletBinding()]
param(
    [string]$Version,
    [string]$InstallDir,
    [string]$BaseUrl
)

$ErrorActionPreference = 'Stop'
$DefaultBaseUrl = 'https://github.com/david-engelmann/verirun/releases'

function Fail([string]$Message) {
    [Console]::Error.WriteLine("install.ps1: error: $Message")
    if ($PSCommandPath) { exit 1 }
    throw "install.ps1: error: $Message"
}

function Test-ValidVersion([string]$Text) {
    # A version is valid only as exactly three dot-separated numeric
    # components, with no leading zero on any component.
    return $Text -match '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\z'
}

function ConvertTo-LocalPath([string]$Url) {
    if ($Url.StartsWith('file://')) {
        return ([Uri]$Url).LocalPath
    }
    if (Test-Path -LiteralPath $Url -PathType Container) { return $Url }
    return $null
}

function Resolve-LatestTag([string]$Root) {
    $local = ConvertTo-LocalPath $Root
    if ($null -ne $local) {
        $latestFile = Join-Path $local 'latest'
        if (Test-Path -LiteralPath $latestFile -PathType Leaf) {
            return (Get-Content -LiteralPath $latestFile -Raw).Trim()
        }
        Fail "-Version <vX.Y.Z> is required when installing from a local base"
    }
    $latestUrl = $Root.TrimEnd('/') + '/latest'
    try {
        $response = Invoke-WebRequest -Uri $latestUrl -UseBasicParsing
    } catch {
        Fail "failed to resolve latest release from $latestUrl"
    }
    $base = $response.BaseResponse
    $effective = $null
    if ($base.PSObject.Properties['ResponseUri'] -and $base.ResponseUri) {
        $effective = $base.ResponseUri.AbsoluteUri
    } elseif ($base.RequestMessage -and $base.RequestMessage.RequestUri) {
        $effective = $base.RequestMessage.RequestUri.AbsoluteUri
    }
    if ($effective) {
        $resolvedTag = $effective.TrimEnd('/').Split('/')[-1]
        if ($resolvedTag.StartsWith('v') -and (Test-ValidVersion $resolvedTag.Substring(1))) {
            return $resolvedTag
        }
    }
    Fail "could not parse release tag from redirect URL: $effective"
}

function Fetch-ToFile([string]$Source, [string]$Destination, [bool]$IsLocal) {
    if ($IsLocal) {
        if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) {
            Fail "local release file not found: $Source"
        }
        Copy-Item -LiteralPath $Source -Destination $Destination
        return
    }
    try {
        Invoke-WebRequest -Uri $Source -OutFile $Destination -UseBasicParsing
    } catch {
        Fail "failed to download $Source"
    }
}

$versionGiven = $PSBoundParameters.ContainsKey('Version')
if (-not $versionGiven) { $Version = $env:VERIRUN_VERSION }
if ($PSBoundParameters.ContainsKey('InstallDir')) {
    if ([string]::IsNullOrEmpty($InstallDir)) {
        Fail "install directory must not be empty (set -InstallDir or INSTALL_DIR, or set HOME)"
    }
} else {
    $InstallDir = $env:INSTALL_DIR
    if ([string]::IsNullOrEmpty($InstallDir) -and -not [string]::IsNullOrEmpty($HOME)) {
        $InstallDir = Join-Path $HOME '.local/bin'
    }
}
if ($PSBoundParameters.ContainsKey('BaseUrl')) {
    if ([string]::IsNullOrEmpty($BaseUrl)) {
        Fail "base URL must not be empty"
    }
} else {
    $BaseUrl = $env:VERIRUN_BASE_URL
    if ([string]::IsNullOrEmpty($BaseUrl)) { $BaseUrl = $DefaultBaseUrl }
}

if ($versionGiven -and [string]::IsNullOrEmpty($Version)) {
    Fail "-Version must not be empty (omit it to install the latest release)"
}
if ([string]::IsNullOrEmpty($InstallDir)) {
    Fail "install directory must not be empty (set -InstallDir or INSTALL_DIR, or set HOME)"
}

if ([string]::IsNullOrEmpty($Version)) {
    $Version = Resolve-LatestTag $BaseUrl
}

$versionNum = $Version -creplace '^v', ''
if (-not (Test-ValidVersion $versionNum)) {
    Fail "invalid version '$Version' (expected vX.Y.Z or X.Y.Z)"
}
$tag = "v$versionNum"

$arch = $env:PROCESSOR_ARCHITECTURE
if ($arch -eq 'x86' -and $env:PROCESSOR_ARCHITEW6432) {
    $arch = $env:PROCESSOR_ARCHITEW6432
}
if ($arch -ne 'AMD64') {
    Fail "unsupported CPU architecture '$arch' (supported: AMD64)"
}

$stage = "verirun-v$versionNum-windows-x86_64"
$archive = "$stage.zip"

$trimmedBase = $BaseUrl.TrimEnd('/')
$localBase = ConvertTo-LocalPath $trimmedBase
$localReleaseDir = $null
$releaseUrl = $null
if ($null -ne $localBase) {
    $candidate = Join-Path (Join-Path $localBase 'download') $tag
    if (Test-Path -LiteralPath $candidate -PathType Container) {
        $localReleaseDir = $candidate
    } elseif (Test-Path -LiteralPath (Join-Path $localBase 'SHA256SUMS') -PathType Leaf) {
        $localReleaseDir = $localBase
    } else {
        $localReleaseDir = $candidate
    }
} else {
    $releaseUrl = "$trimmedBase/download/$tag"
}

$tmpDir = Join-Path ([IO.Path]::GetTempPath()) ("verirun-install-" + [Guid]::NewGuid().ToString('N'))
$stagedBin = $null
try {
    New-Item -ItemType Directory -Path $tmpDir | Out-Null
    $archivePath = Join-Path $tmpDir $archive
    $sumsPath = Join-Path $tmpDir 'SHA256SUMS'
    if ($null -ne $localReleaseDir) {
        Fetch-ToFile (Join-Path $localReleaseDir $archive) $archivePath $true
        Fetch-ToFile (Join-Path $localReleaseDir 'SHA256SUMS') $sumsPath $true
    } else {
        Fetch-ToFile "$releaseUrl/$archive" $archivePath $false
        Fetch-ToFile "$releaseUrl/SHA256SUMS" $sumsPath $false
    }

    $expectedSha = $null
    foreach ($line in Get-Content -LiteralPath $sumsPath) {
        $parts = $line -split '\s+'
        if ($parts.Count -ge 2) {
            $name = $parts[1]
            if ($name.StartsWith('*')) { $name = $name.Substring(1) }
            if ($name -ceq $archive) {
                $expectedSha = $parts[0].ToLower()
                break
            }
        }
    }
    if ([string]::IsNullOrEmpty($expectedSha)) {
        Fail "archive '$archive' was not listed in SHA256SUMS"
    }
    if ($expectedSha -notmatch '^[0-9a-f]{64}\z') {
        Fail "malformed SHA-256 digest in SHA256SUMS for '$archive'"
    }

    $actualSha = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLower()
    if ($actualSha -ne $expectedSha) {
        Fail "SHA-256 verification failed for '$archive' (expected $expectedSha, got $actualSha)"
    }

    Expand-Archive -LiteralPath $archivePath -DestinationPath $tmpDir
    $extractedBin = Join-Path (Join-Path $tmpDir $stage) 'verirun.exe'
    if (-not (Test-Path -LiteralPath $extractedBin -PathType Leaf)) {
        Fail "expected binary '$stage/verirun.exe' was not found inside '$archive'"
    }

    if (-not (Test-Path -LiteralPath $InstallDir -PathType Container)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }
    $stagedBin = Join-Path $InstallDir ".verirun.install.$PID.exe"
    Copy-Item -LiteralPath $extractedBin -Destination $stagedBin

    $stagedVersion = $null
    try {
        $stagedVersion = & $stagedBin --version
    } catch {
        Fail "the downloaded binary does not run"
    }
    if ($LASTEXITCODE -ne 0) {
        Fail "the downloaded binary does not run"
    }
    $stagedVersion = "$stagedVersion".Trim()
    if ($stagedVersion -cne "verirun $versionNum") {
        Fail "the downloaded binary reports '$stagedVersion' (expected 'verirun $versionNum')"
    }

    Move-Item -LiteralPath $stagedBin -Destination (Join-Path $InstallDir 'verirun.exe') -Force
    $stagedBin = $null

    Write-Output "Installed verirun $tag to $InstallDir\verirun.exe (SHA-256 verified: $actualSha)"
    $pathEntries = $env:PATH -split ';'
    if ($pathEntries -notcontains $InstallDir) {
        Write-Output "Note: $InstallDir is not currently on your PATH."
    }
} finally {
    if ($stagedBin -and (Test-Path -LiteralPath $stagedBin -PathType Leaf)) {
        Remove-Item -LiteralPath $stagedBin -Force
    }
    if (Test-Path -LiteralPath $tmpDir -PathType Container) {
        Remove-Item -LiteralPath $tmpDir -Recurse -Force
    }
}
