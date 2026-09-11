# Veil install / update for Windows (download to .new, then swap the PATH binary)
$ErrorActionPreference = "Stop"
$repo = "Mouseww/veil"
$dir = Join-Path $env:LOCALAPPDATA "veil"
$bin = Join-Path $dir "bin"
New-Item -ItemType Directory -Force -Path $bin | Out-Null
$target = Join-Path $bin "veil.exe"
$tmp = "$target.new"

$rel = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
$asset = $rel.assets | Where-Object { $_.name -eq "veil-windows-x64.exe" } | Select-Object -First 1
if (-not $asset) { throw "Release has no veil-windows-x64.exe yet." }

$ua = @{ "User-Agent" = "veil-installer"; Accept = "application/octet-stream" }
$urls = @(
  $asset.url,
  ("https://ghfast.top/" + $asset.browser_download_url),
  ("https://ghproxy.net/" + $asset.browser_download_url),
  $asset.browser_download_url
) | Where-Object { $_ }

$ok = $false
foreach ($u in $urls) {
  Write-Host "Downloading $u"
  try {
    Invoke-WebRequest -Uri $u -Headers $ua -OutFile $tmp
    if ((Get-Item $tmp).Length -gt 1000000) { $ok = $true; break }
  } catch { Write-Host $_ -ForegroundColor Yellow }
}
if (-not $ok) {
  throw "Download failed. Try: gh release download $($rel.tag_name) --repo $repo -p veil-windows-x64.exe -D $bin"
}

Get-Process veil,dgw -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1
Move-Item -Force $tmp $target
Copy-Item $target (Join-Path $dir "veil.exe") -Force

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$bin*") {
  [Environment]::SetEnvironmentVariable("Path", "$userPath;$bin", "User")
  $env:Path += ";$bin"
}
Write-Host "Installed $($rel.tag_name) to $target"
Start-Process $target
Write-Host "When the browser opens, run:  veil setup"
