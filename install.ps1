# Build and install the Rust-native wf-core global surfaces for Windsurf, Windsurf Next, and Devin.

[CmdletBinding()]
param(
    [ValidateSet("stable", "next", "insiders", "both")]
    [string]$Channel = "both",

    [ValidateSet("windsurf", "devin", "all")]
    [string]$Target = "all",

    [string]$Cargo = "",

    [switch]$ModifyShellProfile
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = "Stop"

$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path

if ([string]::IsNullOrWhiteSpace($Cargo)) {
    $Cargo = "cargo"
}

& $Cargo build --release --locked --manifest-path (Join-Path $ScriptRoot "Cargo.toml")
if ($LASTEXITCODE -ne 0) {
    throw "wf-core cargo build failed with exit code $LASTEXITCODE"
}

$Binary = Join-Path $ScriptRoot "target\release\wf-core.exe"
if (-not (Test-Path $Binary -PathType Leaf)) {
    $Binary = Join-Path $ScriptRoot "target\release\wf-core"
}

$ActivationChannel = switch ($Channel) {
    "stable" { "stable" }
    "insiders" { "insiders" }
    default { "next" }
}

$UserProfileRoot = [Environment]::GetFolderPath("UserProfile")
$ActivationBinary = Join-Path $UserProfileRoot ".codeium\windsurf-next\wf-core\wf-core.exe"
if ($ActivationChannel -eq "stable") {
    $ActivationBinary = Join-Path $UserProfileRoot ".codeium\windsurf\wf-core\wf-core.exe"
} elseif ($ActivationChannel -eq "insiders") {
    $ActivationBinary = Join-Path $UserProfileRoot ".codeium\windsurf-insiders\wf-core\wf-core.exe"
}
if ($Target -eq "devin") {
    $ActivationBinary = Join-Path $env:APPDATA "devin\wf-core\wf-core.exe"
}

& $Binary install --target $Target --channel $Channel --source-root $ScriptRoot
if ($LASTEXITCODE -ne 0) {
    throw "wf-core global install failed with exit code $LASTEXITCODE"
}

& $Binary verify --target $Target --channel $Channel
if ($LASTEXITCODE -ne 0) {
    throw "wf-core verify failed with exit code $LASTEXITCODE"
}

& $Binary doctor --proxy --target $Target --channel $Channel
if ($LASTEXITCODE -ne 0) {
    Write-Warning "wf-core proxy doctor reported warnings. Activate shell proxy with the command printed below."
}

Write-Host ""
Write-Host "wf-core proxy activation:"
Write-Host "  & `"$ActivationBinary`" shell init --channel $ActivationChannel --shell powershell | Invoke-Expression"
Write-Host "  & `"$ActivationBinary`" doctor --proxy --channel $ActivationChannel"

if ($ModifyShellProfile) {
    $HadProfile = Test-Path $PROFILE
    if (-not $HadProfile) {
        New-Item -ItemType File -Path $PROFILE -Force | Out-Null
    }
    $Backup = ""
    if ($HadProfile) {
        $Backup = "$PROFILE.wf-core.bak.$(Get-Date -Format yyyyMMddHHmmss)"
        Copy-Item $PROFILE $Backup -Force
    }

    $ManagedBlock = @"
# wf-core managed:start
& "$ActivationBinary" shell init --channel $ActivationChannel --shell powershell | Invoke-Expression
# wf-core managed:end
"@
    $Content = Get-Content -Path $PROFILE -Raw
    $ManagedBlockRegex = [regex]"(?s)# wf-core managed:start.*?# wf-core managed:end"
    if ($ManagedBlockRegex.IsMatch($Content)) {
        $Replacement = [System.Text.RegularExpressions.MatchEvaluator]{ param($Match) $ManagedBlock }
        $Content = $ManagedBlockRegex.Replace($Content, $Replacement, 1)
    } elseif ([string]::IsNullOrWhiteSpace($Content)) {
        $Content = "$ManagedBlock`r`n"
    } else {
        $Content = $Content.TrimEnd() + "`r`n`r`n$ManagedBlock`r`n"
    }
    Set-Content -Path $PROFILE -Value $Content -NoNewline
    if ($HadProfile) {
        Write-Host "Updated $PROFILE (backup: $Backup)"
    } else {
        Write-Host "Updated $PROFILE (no previous file to back up)"
    }
} else {
    Write-Host "Shell profile not modified. Pass -ModifyShellProfile to append a managed block."
}
