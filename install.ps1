# Build and install the Rust-native wf-core global surfaces for Windsurf, Windsurf Next, and Devin.

[CmdletBinding()]
param(
    [ValidateSet("stable", "next", "insiders", "both")]
    [string]$Channel = "both",

    [ValidateSet("windsurf", "devin", "all")]
    [string]$Target = "all",

    [string]$Cargo = ""
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
