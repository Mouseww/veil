# Veil

**敏感信息不出网。** 本地跑一个小代理，Claude Code / Cursor 照常聊天，手机号、身份证、数据库连接串、AK/SK 会在出网前被换成占位符，模型回复回来再自动还原。

One-liner: *Keep secrets on your machine. The model never sees them.*

## 30 秒上手（Windows）

1. 从 [Releases](https://github.com/Mouseww/DesensitizationGateway/releases) 下载 `dgw-windows-x64.exe`，放到任意目录。
2. **双击它**（或在终端运行 `dgw`）。浏览器会打开管理页。
3. 再打开一个终端，运行：

```bat
dgw setup
```

4. **重启 Claude Code**。之后正常聊天即可。官方订阅不用填 API Key。

一键安装（PowerShell）：

```powershell
irm https://raw.githubusercontent.com/Mouseww/DesensitizationGateway/main/scripts/install.ps1 | iex
```

## 它帮你挡什么

内网 IP、手机号、身份证、数据库连接串、密钥/Token。规则可在管理页改，也可以试跑。

## 命令

| 命令 | 作用 |
|---|---|
| `dgw` | 启动并打开浏览器 |
| `dgw setup` | 一键写入 Claude Code 配置 |
| `dgw stop` | 停止 |
| `dgw status` | 看是否在跑 |

代理地址：`http://127.0.0.1:18787` 　管理页：`http://127.0.0.1:18788`

## 开发

```bash
cargo test --workspace
cd ui && npm test && npm run build
node --test plugin/claude-code/scripts/dgw.test.mjs
```

Apache-2.0. 无任何出网遥测。
