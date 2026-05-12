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
Write-Host "  & `"$Binary`" shell init --channel next --shell powershell | Invoke-Expression"
Write-Host "  & `"$Binary`" doctor --proxy --channel next"

if ($ModifyShellProfile) {
    if (-not (Test-Path $PROFILE)) {
        New-Item -ItemType File -Path $PROFILE -Force | Out-Null
    }
    $Backup = "$PROFILE.wf-core.bak.$(Get-Date -Format yyyyMMddHHmmss)"
    Copy-Item $PROFILE $Backup -Force
    Add-Content -Path $PROFILE -Value ""
    Add-Content -Path $PROFILE -Value "# wf-core managed:start"
    Add-Content -Path $PROFILE -Value "& `"$Binary`" shell init --channel next --shell powershell | Invoke-Expression"
    Add-Content -Path $PROFILE -Value "# wf-core managed:end"
    Write-Host "Updated $PROFILE (backup: $Backup)"
} else {
    Write-Host "Shell profile not modified. Pass -ModifyShellProfile to append a managed block."
}
