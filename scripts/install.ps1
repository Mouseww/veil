# Veil one-click install for Windows
$ErrorActionPreference = "Stop"
$repo = "Mouseww/veil"
$dir = Join-Path $env:LOCALAPPDATA "veil"
$exe = Join-Path $dir "veil.exe"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$rel = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
$asset = $rel.assets | Where-Object { $_.name -eq "veil-windows-x64.exe" } | Select-Object -First 1
if (-not $asset) { throw "Release has no veil-windows-x64.exe yet. Download it from GitHub Releases." }
Write-Host "Downloading $($asset.browser_download_url)"
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $exe
$bin = Join-Path $dir "bin"
New-Item -ItemType Directory -Force -Path $bin | Out-Null
Copy-Item $exe (Join-Path $bin "veil.exe") -Force
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$bin*") {
  [Environment]::SetEnvironmentVariable("Path", "$userPath;$bin", "User")
  $env:Path += ";$bin"
}
Write-Host "Installed to $exe"
Start-Process $exe
Write-Host "When the browser opens, run:  veil setup"
Write-Host "Then restart Claude Code."
