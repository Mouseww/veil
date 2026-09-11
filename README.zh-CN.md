# Veil · 给大模型戴上面具

跟 AI 聊天，等于把电脑里的字送到别人家里。密码、手机号、数据库地址，模型服务商都能看见。

**Veil 是一层面纱。** 出门（出网）先戴面具，回家（回复）再摘下来。你还是看原文；外面那台模型从头到尾只看见面具。

[English](README.md) · [下载](https://github.com/Mouseww/veil/releases/latest) · Apache-2.0 · 无遥测

任意能改 Base URL 的工具都能戴：Claude Code、Cursor、Codex、Trae、自写脚本、公司中转……不限名单。灵感致谢 [Maskit 数据面具](https://github.com/xiaYuTian11/maskit)。

---

## 面具怎么戴、怎么摘

把机密想成一张没戴面具的脸：

| | 你这边 | 面具（占位符） | 外面的模型 |
|---|---|---|---|
| 出门 | `mysql://root:Pass123@10.1.2.3/app`，`13800138000` | `{{CONNSTR_…}}` `{{PHONE_…}}` | 只看见面具，能推理，看不见真脸 |
| 回家 | 你看到的回答里已经是真连接串、真手机号 | 同一张面具始终对应同一张脸 | 它一直以为自己在讨论面具 |

同一段密码，整场对话都戴同一张面具，模型不会把「张三」认成两个人。

Key 不用交给 Veil：登录、API Key 还在原来的软件里，Veil 只给**正文**戴面具，证件（鉴权头）原样出门。

```
你的工具  --改个地址-->  本机 Veil（戴/摘面具）  --再出门-->  官方 / 中转 / LiteLLM
```

---

## 1. 安装（Windows）

1. 打开 [Releases](https://github.com/Mouseww/veil/releases/latest)，下载 `veil-windows-x64.exe`。
2. 放到任意目录，**双击**。应弹出浏览器：http://127.0.0.1:18788
3. 若双击没反应，在该目录打开终端：

```bat
veil.exe
```

国内若 GitHub 下不动，用（走 API，一般能过）：

```powershell
gh release download v0.3.5 --repo Mouseww/veil -p veil-windows-x64.exe -D $env:LOCALAPPDATA\veil\bin
Copy-Item $env:LOCALAPPDATA\veil\bin\veil-windows-x64.exe $env:LOCALAPPDATA\veil\bin\veil.exe -Force
$env:LOCALAPPDATA\veil\bin\veil.exe
```

或：

```powershell
irm https://raw.githubusercontent.com/Mouseww/veil/main/scripts/install.ps1 | iex
```

| 地址 | 用途 |
|---|---|
| http://127.0.0.1:18787 | **给 AI 工具填的 Base URL**（代理） |
| http://127.0.0.1:18788 | 管理页（规则 / 上游 / 流量） |

顶栏应显示「代理运行中」。以后更新：管理页 **检查更新**，或 `veil update`。

---

## 2. 使用教程：万能接入（任意能改 Base URL 的产品）

**Veil 不绑定某几个 Agent。** 只要工具能设置 API Base URL（或环境变量），就可以走脱敏。

记住两条：

| 工具协议 | 填到工具里的 Base URL |
|---|---|
| **Anthropic**（`/v1/messages`） | `http://127.0.0.1:18787` |
| **OpenAI 兼容**（`/v1/chat/completions`、`/v1/responses`） | `http://127.0.0.1:18787/v1` |

Key / Token **仍填上游那份**（官方 Key、中转 Key、公司网关 Key 都可以）。

### 2.1 通用步骤（推荐先看这个）

1. 确认 Veil 已启动（18788 能打开）。
2. 打开你的工具 → 设置 → 找到 **Base URL / API 地址 / 自定义端点**。
3. 按上表改成本机地址。
4. Key 不要改成 Veil 的（Veil 没有 Key）。
5. 管理页 **上游**：填这个工具真正要访问的 API 根地址。
   - 官方 Anthropic：`https://api.anthropic.com`（默认）
   - 官方 OpenAI：`https://api.openai.com`
   - 中转 / LiteLLM / New API：填中转商给你的地址，例如 `https://api.your-relay.com`
6. 重启该工具，发一句带手机号的测试：`联系 13800138000`。管理页 **流量** 里应出现请求；上游不应看到真号。

`veil setup` 只是给部分已识别的软件**自动改配置**。没出现在列表里的，按上面手动改即可，效果一样。

### 2.2 常见工具（都是同一套方法）

**Claude Code**

```bat
veil setup --clients claude
```

或自己设：

```powershell
$env:ANTHROPIC_BASE_URL = "http://127.0.0.1:18787"
claude
```

官方订阅（登录 Claude）不用填 Key。自定义 API 再填 Key，并在管理页上游写中转地址。

**Codex / OpenAI CLI**

```powershell
$env:OPENAI_BASE_URL = "http://127.0.0.1:18787/v1"
$env:OPENAI_API_KEY = "你的上游Key"
```

管理页上游的 OpenAI chat / responses 填真正的 OpenAI 或中转根地址。

**Cursor**

Settings → Models → OpenAI Base URL：`http://127.0.0.1:18787/v1`，API Key 填上游 Key。

**Trae / PI / Hermes / CodeBuddy / Grok Builder / 其它 IDE**

在设置里搜 `Base URL`、`API 地址`、`兼容 OpenAI`、`Anthropic endpoint`。
- OpenAI 兼容 → `http://127.0.0.1:18787/v1`
- Anthropic 兼容 → `http://127.0.0.1:18787`

也可用：

```bat
veil setup
```

勾选本机已安装的；没有列出的就手动填。

**Python / Node / LangChain**

```python
from openai import OpenAI
client = OpenAI(base_url="http://127.0.0.1:18787/v1", api_key="你的上游Key")
client.chat.completions.create(model="gpt-4o", messages=[{"role":"user","content":"查 mysql://root:Pass@10.0.0.5/db"}])
```

```js
const openai = new OpenAI({ baseURL: "http://127.0.0.1:18787/v1", apiKey: process.env.OPENAI_API_KEY })
```

**公司中转 / LiteLLM / New API / One API**

1. 管理页 **上游** 填中转根 URL（不要填 Veil 自己）。
2. 工具 Base URL 仍是 `http://127.0.0.1:18787` 或 `/v1`。
3. 工具里的 Key 用中转商发给你的 Key。

从未设过 Base URL 时也可以：

```bat
veil setup --upstream https://api.your-relay.com
```

---

## 3. 规则配置教程

打开 http://127.0.0.1:18788 → **规则**。

规则决定：**哪些字符串在出网前被换成** `{{类型_ULID}}`。同一把 API Key（Creator）下，同一明文始终对应同一占位符，多轮对话不会串。

### 3.1 内置类型（开箱即用）

| 类型 | 会匹配什么 |
|---|---|
| `PEM` | `BEGIN PRIVATE KEY` 整段证书 |
| `APIKEY` | `sk-` / `sk-ant-` / `AKIA` / `ghp_` / `api_key=` 等 |
| `TOKEN` | `Bearer`、JWT `eyJ`、`access_token=`、`xoxb-` |
| `CONNSTR` | `mysql://` `postgres://` `jdbc:` 以及 `Password=` `Server=` |
| `PASSWORD` | `password:` `passwd=` `口令` `登录密码` |
| `PHONE` | 大陆 11 位手机号 |
| `IDCARD` | 18 位身份证 |
| `IP` | 能解析的 IPv4/IPv6（含公网；`127.0.0.1` `localhost` 默认白名单） |
| `EMAIL` | 邮箱（**默认关闭**，误报较多） |

### 3.2 怎么改一条规则

1. 点规则左侧 **开关**：立刻启用/禁用。
2. 再点这一行 **展开**：
   - **类型**：占位符前缀，如 `PHONE` → `{{PHONE_…}}`
   - **优先级**：数字越大越先匹配；重叠时只保留高优先级
   - **正则**：Rust/fancy-regex，可用 lookbehind，如 `(?<!\d)1[3-9]\d{9}(?!\d)`
   - **词表**：一行一个固定词（项目代号、人名）
3. 点 **保存规则**。
4. 自定义规则可以 **删除**；内置 IP 没有正则（按地址解析），只能开关。

### 3.3 加自己的规则

下面「添加自定义正则」：

| 字段 | 示例 |
|---|---|
| id | `project`（仅本机配置用） |
| 类型 | `CODENAME` |
| 正则 | `内部项目[甲乙丙]` 或 `\bAcmeInternal\b` |

保存后，命中会变成 `{{CODENAME_…}}`。

### 3.4 白名单

一行一个**永远不替换**的字符串。默认已有 `127.0.0.1`、`localhost`。把公司对外文档里允许出现的测试号、示例 IP 加进去。

### 3.5 试跑（必做）

把一段真实日志贴进试跑框，点运行。只在本机扫描，**不会发给模型**。确认手机号/连接串被标出、普通英文没被误伤，再保存规则。

### 3.6 建议

- 先试跑再上生产对话。
- 正则尽量加边界，避免 `task-abc` 被当成 `sk-`。
- 脱敏失败会 **502、不转发**（宁可不聊天也不泄密）。

---

## 命令

| 命令 | 作用 |
|---|---|
| `veil` / 双击 | 启动并打开管理页 |
| `veil setup` | 可选：自动改本机已识别软件的 Base URL |
| `veil setup --upstream https://…` | 同时记下自定义上游 |
| `veil update` | 从 GitHub 更新（国内失败时用管理页或上面的 `gh release download`） |
| `veil status` / `veil stop` | 查看 / 停止 |

`VEIL_DATA_DIR`、`VEIL_MASTER_KEY`、`VEIL_BIND`、`VEIL_MODE=server`、`VEIL_GITHUB_MIRROR`。旧 `DGW_*` 仍可用。

---

## 许可

Apache-2.0。映射表落盘加密。无出网遥测。

