# Veil / dgw one-click install for Windows
$ErrorActionPreference = "Stop"
$repo = "Mouseww/DesensitizationGateway"
$dir = Join-Path $env:LOCALAPPDATA "dgw"
$exe = Join-Path $dir "dgw.exe"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$rel = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
$asset = $rel.assets | Where-Object { $_.name -eq "dgw-windows-x64.exe" } | Select-Object -First 1
if (-not $asset) { throw "release 里没有 dgw-windows-x64.exe，请稍后再试或手动下载。" }
Write-Host "下载 $($asset.browser_download_url)"
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $exe
$bin = Join-Path $dir "bin"
New-Item -ItemType Directory -Force -Path $bin | Out-Null
Copy-Item $exe (Join-Path $bin "dgw.exe") -Force
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$bin*") {
  [Environment]::SetEnvironmentVariable("Path", "$userPath;$bin", "User")
  $env:Path += ";$bin"
}
Write-Host "已安装到 $exe"
Write-Host "正在启动…"
Start-Process $exe
Write-Host "浏览器打开后，另开终端执行:  dgw setup"
Write-Host "然后重启 Claude Code。"
