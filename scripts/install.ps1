# Veil one-click install for Windows
$ErrorActionPreference = "Stop"
$repo = "Mouseww/veil"
$dir = Join-Path $env:LOCALAPPDATA "veil"
$exe = Join-Path $dir "veil.exe"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
Get-Process veil,dgw -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
if (Get-Command veil -ErrorAction SilentlyContinue) { try { veil stop } catch {} }
$rel = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
$asset = $rel.assets | Where-Object { $_.name -eq "veil-windows-x64.exe" } | Select-Object -First 1
if (-not $asset) { throw "Release has no veil-windows-x64.exe yet. Download it from GitHub Releases." }
$ua = @{ "User-Agent" = "veil-installer"; Accept = "application/octet-stream" }
$urls = @(
  $asset.url,
  $asset.browser_download_url,
  ("https://ghfast.top/" + $asset.browser_download_url),
  ("https://ghproxy.net/" + $asset.browser_download_url)
) | Where-Object { $_ }
$ok = $false
foreach ($u in $urls) {
  Write-Host "Downloading $u"
  try {
    Invoke-WebRequest -Uri $u -Headers $ua -OutFile $exe
    if ((Get-Item $exe).Length -gt 1000000) { $ok = $true; break }
  } catch {
    Write-Host $_ -ForegroundColor Yellow
  }
}
if (-not $ok) { throw "Download failed. If GitHub is blocked, run: gh release download v$($rel.tag_name.TrimStart('v')) --repo $repo -p veil-windows-x64.exe -O `$exe" }
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
