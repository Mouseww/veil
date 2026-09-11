# Veil

[English](README.md) · [下载](https://github.com/Mouseww/veil/releases/latest) · Apache-2.0 · 无遥测

**你的编程助手其实已经见过生产环境密码了。**

排障时贴一段连接串。写正则时贴客户手机号。看堆栈时把 `.env` 整段丢进去。这些字离开电脑，进了模型厂商的日志，**撤不回来**。

Veil 跑在本机，夹在 IDE 和模型之间。出网前把机密换成占位符，模型用占位符回答，回来再还原成原文。你还是那样聊。**模型从头到尾没见过明文。**

```
Claude / Codex / Trae / PI / Hermes / Grok
        |
        v
   Veil（这台电脑）      出网脱敏 --> 回程还原
        |
        v
   Anthropic / OpenAI / LiteLLM / 你们公司的网关
```

不用注册。不用再给 Veil 一份 Key。官方 Claude 登录照旧。第三方网关照旧。鉴权头原样转发。

## 安装（Windows，大约 30 秒）

```powershell
irm https://raw.githubusercontent.com/Mouseww/veil/main/scripts/install.ps1 | iex
```

浏览器弹出管理页后，**另开一个终端**：

```bat
veil setup
```

勾选你真正在用的应用，重启它们，该怎么聊怎么聊。

也可以从 [Releases](https://github.com/Mouseww/veil/releases/latest) 下载 `veil-windows-x64.exe` 双击。

macOS / Linux：下载对应二进制，`./veil` 然后 `./veil setup`。

代理 `http://127.0.0.1:18787` · 管理页 `http://127.0.0.1:18788`

## 如何更新

已经是 **v0.2+**（`veil.exe`）：

```bat
veil update
```

或在管理页点 **检查更新** → **立即更新**。会从 GitHub 拉最新包、校验 SHA-256、替换自身并重启。

如果 `veil update` 提示连不上 `github.com/releases/download`（国内常见），程序会改走 GitHub API 和镜像；也可设置 `VEIL_GITHUB_MIRROR=https://ghfast.top/`。或：

```bat
gh release download v0.3.3 --repo Mouseww/veil -p veil-windows-x64.exe
```

还在用 **v0.1**（`dgw.exe`）：再跑一遍安装脚本，装上 `veil.exe`。以后用 `veil update`。

```powershell
irm https://raw.githubusercontent.com/Mouseww/veil/main/scripts/install.ps1 | iex
```

## 什么时候真的用得上

**对着真实数据排障。** 生产库挂了，助手需要 DSN、Redis、JWT。让它看见结构，不必看见秘密。

**提示词里有客户资料。** 日志里的手机号、身份证、邮箱。你要的是摘要，不是一次泄露事故。

**内网环境。** 工单里全是 `10.x`、VPN、后台地址。公网 IP 也会挡；本机回环放行。

**`.env` 和证书。** 你问「为啥发版失败」，私钥就在报错上面三行。

**非官方 / 公司模型网关。** LiteLLM、自建反代、转售 API。你也不想让*那台*服务器存客户手机号。`--upstream` 指过去即可。

**官方订阅。** 登录留在这台电脑上。Veil 只是本地这一跳，不会动 OAuth。

**同时开好几个编程助手。** 这边 Claude Code，那边 Codex，再开 Trae。`veil setup` 分别写配置。一个代理，多个应用。

**演示和教学。** 分享会现场写代码，别把个人 Token 打在投影仪上。

**默认失败关闭。** 无法证明这段请求干净，就**不转发**。502 总比泄密好。

## `veil setup` 能接入的应用

| id | 应用 |
|---|---|
| `claude` | Claude Code |
| `codex` | Codex |
| `pi` | PI Agent |
| `codebuddy` | CodeBuddy（Workbuddy） |
| `grok` | Grok Builder |
| `hermes` | Hermes |
| `trae` | Trae |

```bat
veil setup --clients claude,codex,trae
veil setup --clients all --upstream https://你的网关
```

直接 `veil setup` 会列出这台机器上已安装的。改完请重启对应应用。

## 官方 API 还是自己的网关

| 你怎么用模型 | 怎么配 |
|---|---|
| 官方 Anthropic / Claude 登录 | 只跑 `veil setup` |
| 已经有 `ANTHROPIC_BASE_URL` | `veil setup` 会把它留下当上游 |
| 第三方，从来没设过 Base URL | `veil setup --upstream https://…` |
| OpenAI 兼容客户端 | Base URL 填 `http://127.0.0.1:18787`（OpenAI 路径带 `/v1`） |

API Key / Token **永远用上游那份**，填在 IDE 里。Veil 透传 `Authorization`、`x-api-key`、`api-key`，不会再要你一份。

需要微调时打开管理页 **「上游」**。

## 会挡什么

内置：手机号、身份证、可解析 IP（含公网；`127.0.0.1` / `localhost` 白名单）、PEM、`sk-` Token、数据库连接串。

管理页可加正则或词表，先试跑再保存。占位符形如 `{{PHONE_01ARZ3NDEKTSV4RRFFQ69G5FAV}}`，只对同一把 API Key（Creator）还原。

## 命令

| | |
|---|---|
| `veil` | 启动并打开管理页 |
| `veil setup` | 选择应用并写入 Base URL |
| `veil update` | 安装 GitHub 上的最新版 |
| `veil status` / `veil stop` | 查看 / 停止 |

进阶：`VEIL_DATA_DIR`、`VEIL_MASTER_KEY`、`VEIL_BIND`、`VEIL_MODE=server`。旧的 `DGW_*` 仍然有效。

## 许可证

Apache-2.0。没有任何出网遥测。映射表落盘加密。

