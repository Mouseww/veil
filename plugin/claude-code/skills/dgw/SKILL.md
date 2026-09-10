---
name: dgw
description: 启动本地 Veil 脱敏网关，让 Claude Code 流量先脱敏再出网。
---

# Veil / dgw

用户如果还没装网关：提示去 GitHub Releases 下载 `dgw-windows-x64.exe` 并双击，或运行 PowerShell：

`irm https://raw.githubusercontent.com/Mouseww/DesensitizationGateway/main/scripts/install.ps1 | iex`

然后：

1. `/dgw start` 或双击 exe（会打开浏览器）
2. 终端执行 `dgw setup`
3. 重启 Claude Code

不要改用户的 API Key / 登录态。网关起不来时只警告「当前没走脱敏」，不要中断对话。
