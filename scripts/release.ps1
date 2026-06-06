# Usage: .\scripts\release.ps1 0.3.0
# Updates version everywhere, commits, tags, and pushes to trigger CI.

param(
    [Parameter(Mandatory)][string]$Version
)

# Validate semver-ish format
if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    Write-Error "Version must be x.y.z (e.g. 0.3.0)"; exit 1
}

$tag = "v$Version"

Write-Host "Releasing $tag..." -ForegroundColor Cyan

# ── Patch tauri.conf.json ────────────────────────────────────────────────────
$confPath = "$PSScriptRoot\..\src-tauri\tauri.conf.json"
$content = (Get-Content $confPath -Raw) -replace '"version":\s*"[^"]+"', "`"version`": `"$Version`""
[System.IO.File]::WriteAllText($confPath, $content, [System.Text.UTF8Encoding]::new($false))
Write-Host "  tauri.conf.json → $Version"

# ── Patch Cargo.toml (first version line = [package]) ───────────────────────
$cargoPath = "$PSScriptRoot\..\src-tauri\Cargo.toml"
$patched = $false
(Get-Content $cargoPath | ForEach-Object {
    if (-not $patched -and $_ -match '^version\s*=\s*"') {
        $patched = $true; "version = `"$Version`""
    } else { $_ }
}) | Out-File -FilePath $cargoPath -Encoding utf8
Write-Host "  Cargo.toml       → $Version"

# ── Commit, tag, push ────────────────────────────────────────────────────────
git add src-tauri/tauri.conf.json src-tauri/Cargo.toml
git commit -m "chore: release $tag"
git tag $tag
git push origin main
git push origin $tag

Write-Host ""
Write-Host "Done! Watch the CI at:" -ForegroundColor Green
Write-Host "  https://github.com/$(git remote get-url origin | Select-String -Pattern 'github\.com[:/](.+?)(?:\.git)?$' | ForEach-Object { $_.Matches[0].Groups[1].Value })/actions"
