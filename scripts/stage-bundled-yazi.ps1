[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$YaziDirectory,
    [string]$TargetDirectory = (Join-Path $PSScriptRoot "..\target\release")
)

$source = [System.IO.Path]::GetFullPath((Resolve-Path -LiteralPath $YaziDirectory).Path)
$target = [System.IO.Path]::GetFullPath($TargetDirectory)
$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$expectedVersion = "26.8.15"

if (-not (Test-Path -LiteralPath (Join-Path $source "yazi.exe") -PathType Leaf)) {
    throw "yazi.exe was not found in $source"
}
if (-not (Test-Path -LiteralPath (Join-Path $source "ya.exe") -PathType Leaf)) {
    throw "ya.exe was not found in $source"
}
if (-not (Test-Path -LiteralPath (Join-Path $repositoryRoot "assets\yazi") -PathType Container)) {
    throw "runtime yazi configuration was not found in the repository"
}

foreach ($binaryName in @("yazi.exe", "ya.exe")) {
    $versionOutput = & (Join-Path $source $binaryName) --version 2>&1 | Out-String
    if ($versionOutput -notmatch ("Version:\s+" + [regex]::Escape($expectedVersion) + "\b")) {
        throw "$binaryName is not yazi $expectedVersion"
    }
}

New-Item -ItemType Directory -Force -Path $target | Out-Null
Copy-Item -LiteralPath (Join-Path $source "yazi.exe") -Destination (Join-Path $target "yazi.exe") -Force
Copy-Item -LiteralPath (Join-Path $source "ya.exe") -Destination (Join-Path $target "ya.exe") -Force
$configSource = Join-Path $repositoryRoot "assets\yazi"
$configTarget = Join-Path $target "assets\yazi"
New-Item -ItemType Directory -Force -Path $configTarget | Out-Null
Get-ChildItem -LiteralPath $configSource -Force | Copy-Item -Destination $configTarget -Recurse -Force

Write-Host "Staged yazi $expectedVersion and runtime configuration in $target"
