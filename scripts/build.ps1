# Usage: .\scripts\build.ps1 0.3.0
# Patches version locally and builds the MSI. No git, no push, no tags.

param(
    [Parameter(Mandatory)][string]$Version
)

if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    Write-Error "Version must be x.y.z (e.g. 0.3.0)"; exit 1
}

Write-Host "Building v$Version (local only)..." -ForegroundColor Cyan

# Patch tauri.conf.json
$confPath = "$PSScriptRoot\..\src-tauri\tauri.conf.json"
$content = (Get-Content $confPath -Raw) -replace '"version":\s*"[^"]+"', "`"version`": `"$Version`""
[System.IO.File]::WriteAllText($confPath, $content, [System.Text.UTF8Encoding]::new($false))
Write-Host "  tauri.conf.json → $Version"

# Patch Cargo.toml
$cargoPath = "$PSScriptRoot\..\src-tauri\Cargo.toml"
$patched = $false
(Get-Content $cargoPath | ForEach-Object {
    if (-not $patched -and $_ -match '^version\s*=\s*"') {
        $patched = $true; "version = `"$Version`""
    } else { $_ }
}) | Out-File -FilePath $cargoPath -Encoding utf8
Write-Host "  Cargo.toml       → $Version"

# Disable updater artifacts so no signing key is needed for local builds
$confRaw = Get-Content $confPath -Raw
$confPatched = $confRaw -replace '"createUpdaterArtifacts":\s*true', '"createUpdaterArtifacts": false'
[System.IO.File]::WriteAllText($confPath, $confPatched, [System.Text.UTF8Encoding]::new($false))

Write-Host ""
Write-Host "Running tauri build..." -ForegroundColor Cyan
npm run tauri build
$buildResult = $LASTEXITCODE

# Restore createUpdaterArtifacts
[System.IO.File]::WriteAllText($confPath, $confRaw, [System.Text.UTF8Encoding]::new($false))

if ($buildResult -eq 0) {
    $msi = Get-ChildItem "src-tauri\target\release\bundle\msi\*.msi" |
             Where-Object { $_.Name -notlike "*.zip*" } |
             Select-Object -First 1
    Write-Host ""
    Write-Host "Build complete!" -ForegroundColor Green
    if ($msi) { Write-Host "  MSI: $($msi.FullName)" }
} else {
    Write-Host "Build failed." -ForegroundColor Red; exit 1
}

