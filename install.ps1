# Bootstrap wf-core from the latest GitHub release, or build from source with -FromSource.

[CmdletBinding()]
param(
    [ValidateSet("stable", "next", "insiders", "both")]
    [string]$Channel = $(if ($env:WF_CORE_CHANNEL) { $env:WF_CORE_CHANNEL } else { "both" }),

    [ValidateSet("windsurf", "devin", "all")]
    [string]$Target = $(if ($env:WF_CORE_TARGET) { $env:WF_CORE_TARGET } else { "all" }),

    [string]$Version = $(if ($env:WF_CORE_VERSION) { $env:WF_CORE_VERSION } else { "latest" }),

    [string]$Repository = $(if ($env:WF_CORE_REPOSITORY) { $env:WF_CORE_REPOSITORY } else { "UntaDotMy/wf_core" }),

    [string]$Cargo = "",

    [switch]$ModifyShellProfile,

    [switch]$FromSource
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = "Stop"

function Normalize-ReleaseTag {
    param([string]$RawVersion)
    if ($RawVersion -match "^(v|bootstrap-)") { return $RawVersion }
    if ($RawVersion -match "^[0-9]") { return "v$RawVersion" }
    return $RawVersion
}

function Get-AssetVersion {
    param([string]$ReleaseTag)
    if ($ReleaseTag -match "^v[0-9]") { return $ReleaseTag.Substring(1) }
    return $ReleaseTag
}

function Get-NormalizedArchitecture {
    $architecture = $env:PROCESSOR_ARCHITECTURE
    if ([string]::IsNullOrWhiteSpace($architecture)) {
        $architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    }
    switch -Regex ($architecture.ToLowerInvariant()) {
        "^(amd64|x64|x86_64)$" { return "amd64" }
        "^(arm64|aarch64)$" { return "arm64" }
        default { throw "Unsupported architecture: $architecture" }
    }
}

function Get-LatestReleaseTag {
    param([string]$RepositorySlug)
    $headers = @{
        Accept = "application/vnd.github+json"
        "User-Agent" = "wf-core-installer"
    }
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$RepositorySlug/releases/latest" -Headers $headers
    return $release.tag_name
}

function Get-ActivationChannel {
    switch ($Channel) {
        "stable" { return "stable" }
        "insiders" { return "insiders" }
        default { return "next" }
    }
}

function Get-InstalledBinaryPath {
    param([string]$ActivationChannel)
    if ($Target -eq "devin") {
        return (Join-Path $env:APPDATA "devin\wf-core\wf-core.exe")
    }
    $userProfileRoot = [Environment]::GetFolderPath("UserProfile")
    switch ($ActivationChannel) {
        "stable" { return (Join-Path $userProfileRoot ".codeium\windsurf\wf-core\wf-core.exe") }
        "insiders" { return (Join-Path $userProfileRoot ".codeium\windsurf-insiders\wf-core\wf-core.exe") }
        default { return (Join-Path $userProfileRoot ".codeium\windsurf-next\wf-core\wf-core.exe") }
    }
}

function Invoke-WfCoreInstall {
    param(
        [string]$Binary,
        [string]$SourceRoot
    )
    $activationChannel = Get-ActivationChannel
    $activationBinary = Get-InstalledBinaryPath -ActivationChannel $activationChannel

    & $Binary install --target $Target --channel $Channel --source-root $SourceRoot
    if ($LASTEXITCODE -ne 0) { throw "wf-core global install failed with exit code $LASTEXITCODE" }

    & $Binary verify --target $Target --channel $Channel
    if ($LASTEXITCODE -ne 0) { throw "wf-core verify failed with exit code $LASTEXITCODE" }

    & $Binary doctor --proxy --target $Target --channel $Channel
    if ($LASTEXITCODE -ne 0) {
        Write-Warning "wf-core proxy doctor reported warnings. Activate shell proxy with the command printed below."
    }

    Write-Host ""
    Write-Host "wf-core installed. Activate proxy mode:"
    Write-Host "  & `"$activationBinary`" shell init --channel $activationChannel --shell powershell | Invoke-Expression"
    Write-Host "  & `"$activationBinary`" doctor --proxy --channel $activationChannel"

    if ($ModifyShellProfile) {
        Update-ShellProfile -ActivationBinary $activationBinary -ActivationChannel $activationChannel
    } else {
        Write-Host "Shell profile not modified. Pass -ModifyShellProfile to append/replace a managed block."
    }
}

function Update-ShellProfile {
    param(
        [string]$ActivationBinary,
        [string]$ActivationChannel
    )
    $hadProfile = Test-Path $PROFILE
    if (-not $hadProfile) {
        New-Item -ItemType File -Path $PROFILE -Force | Out-Null
    }
    $backup = ""
    if ($hadProfile) {
        $backup = "$PROFILE.wf-core.bak.$(Get-Date -Format yyyyMMddHHmmss)"
        Copy-Item $PROFILE $backup -Force
    }

    $managedBlock = @"
# wf-core managed:start
& "$ActivationBinary" shell init --channel $ActivationChannel --shell powershell | Invoke-Expression
# wf-core managed:end
"@
    $content = Get-Content -Path $PROFILE -Raw
    if ($null -eq $content) { $content = "" }
    $managedBlockRegex = [regex]"(?s)# wf-core managed:start.*?# wf-core managed:end"
    if ($managedBlockRegex.IsMatch($content)) {
        $replacement = [System.Text.RegularExpressions.MatchEvaluator]{ param($match) $managedBlock }
        $content = $managedBlockRegex.Replace($content, $replacement, 1)
    } elseif ([string]::IsNullOrWhiteSpace($content)) {
        $content = "$managedBlock`r`n"
    } else {
        $content = $content.TrimEnd() + "`r`n`r`n$managedBlock`r`n"
    }
    Set-Content -Path $PROFILE -Value $content -NoNewline
    if ($hadProfile) {
        Write-Host "Updated $PROFILE (backup: $backup)"
    } else {
        Write-Host "Updated $PROFILE (no previous file to back up)"
    }
}

function Install-FromSource {
    $scriptRoot = Split-Path -Parent $MyInvocation.ScriptName
    if ([string]::IsNullOrWhiteSpace($scriptRoot)) {
        $scriptRoot = Split-Path -Parent $PSCommandPath
    }
    if ([string]::IsNullOrWhiteSpace($Cargo)) { $Cargo = "cargo" }

    & $Cargo build --release --locked --manifest-path (Join-Path $scriptRoot "Cargo.toml")
    if ($LASTEXITCODE -ne 0) { throw "wf-core cargo build failed with exit code $LASTEXITCODE" }

    $binary = Join-Path $scriptRoot "target\release\wf-core.exe"
    if (-not (Test-Path $binary -PathType Leaf)) {
        $binary = Join-Path $scriptRoot "target\release\wf-core"
    }
    Invoke-WfCoreInstall -Binary $binary -SourceRoot $scriptRoot
}

function Install-FromRelease {
    if ($Version -eq "latest") {
        $releaseTag = Get-LatestReleaseTag -RepositorySlug $Repository
        if ([string]::IsNullOrWhiteSpace($releaseTag)) {
            throw "Unable to resolve latest wf-core release for $Repository"
        }
    } else {
        $releaseTag = Normalize-ReleaseTag -RawVersion $Version
    }

    $assetVersion = Get-AssetVersion -ReleaseTag $releaseTag
    $architecture = Get-NormalizedArchitecture
    $archiveName = "wf-core_${assetVersion}_windows_${architecture}.zip"
    $downloadUrl = "https://github.com/$Repository/releases/download/$releaseTag/$archiveName"
    $temporaryDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ("wf-core-install-" + [System.Guid]::NewGuid().ToString("N"))

    try {
        New-Item -ItemType Directory -Path $temporaryDirectory | Out-Null
        $archivePath = Join-Path $temporaryDirectory $archiveName
        $extractDirectory = Join-Path $temporaryDirectory "extract"
        New-Item -ItemType Directory -Path $extractDirectory | Out-Null

        Write-Host "Downloading wf-core $releaseTag for windows-$architecture..."
        Invoke-WebRequest -Uri $downloadUrl -OutFile $archivePath -Headers @{ "User-Agent" = "wf-core-installer" }
        Expand-Archive -Path $archivePath -DestinationPath $extractDirectory -Force

        $installerBinary = Get-ChildItem -Path $extractDirectory -Filter "wf-core.exe" -File -Recurse | Select-Object -First 1
        if ($null -eq $installerBinary) {
            throw "Release archive did not contain wf-core.exe."
        }

        Invoke-WfCoreInstall -Binary $installerBinary.FullName -SourceRoot $installerBinary.Directory.FullName
    } finally {
        if (Test-Path $temporaryDirectory) {
            Remove-Item -Path $temporaryDirectory -Recurse -Force
        }
    }
}

if ($FromSource) {
    Install-FromSource
} else {
    Install-FromRelease
}
