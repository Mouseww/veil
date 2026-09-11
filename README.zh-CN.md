# Veil

[English](README.md)

**你把数据库密码贴进 Claude。** 它离开了你的电脑，进了厂商日志。模型没学到什么，你却多了一处泄露。

Veil 是跑在本机的小代理。把 Claude Code（或任何 OpenAI 兼容客户端）指过来。手机号、连接串、密钥、证件号在出网前被换成占位符；模型的回复回来后再变回原文。**模型从头到尾没见过明文。**

```
you  -->  Veil (localhost)  -->  Anthropic / OpenAI / LiteLLM
        redact before send         restore on the way back
```

不用注册。没有遥测。官方 Claude 订阅照常用，登录态不会被改掉。

## 30 秒上手

**Windows**

```powershell
irm https://raw.githubusercontent.com/Mouseww/veil/main/scripts/install.ps1 | iex
```

再开一个终端：

```bat
veil setup
```

重启 Claude Code。该怎么聊怎么聊。

或者从 [Releases](https://github.com/Mouseww/veil/releases) 下载 `veil-windows-x64.exe`，双击，再执行 `veil setup`。

**从源码**

```bash
cargo run -p veil
veil setup
```

代理：`http://127.0.0.1:18787`  ·  管理页：`http://127.0.0.1:18788`

## 官方订阅 vs 非官方 / 自定义 API

一键安装脚本**只负责把 Veil 跑起来**。流量接下来打到哪家，叫上游（upstream），需要单独说清楚。

**官方 Anthropic 订阅**（Claude Code 里登录 Pro / Max / Team）：执行 `veil setup` 即可。登录态不动。Veil 默认转发到 `https://api.anthropic.com`。

**第三方 / 公司网关**（LiteLLM、自建反代、已经设过 `ANTHROPIC_BASE_URL`）：

1. 先启动 Veil（双击或安装脚本）。
2. 如果你本来就有自定义 Base URL，`veil setup` 会**把它留下当上游**，再把客户端改成本机。
3. 如果从来没设过，请显式指定：

```bat
veil setup --upstream https://your-gateway.example
```

也可以打开管理页的 **「上游」** 标签粘贴（`http://127.0.0.1:18788`）。

**OpenAI 兼容客户端：** 把客户端 Base URL 设成 `http://127.0.0.1:18787`，再在同一页填 Veil 的 OpenAI 上游。密钥仍留在客户端，Veil 原样转发。

## 会挡什么

内置：手机号、身份证、可解析 IP（含公网，本机回环在白名单）、PEM、`sk-` Token、数据库连接串。管理页可加自己的正则或词表，先试跑再保存。

如果 Veil 没法证明这段内容是干净的，**就不会转发**。

## 命令

| | |
|---|---|
| `veil` | 启动并打开管理页 |
| `veil setup` | 一键写入 Claude Code 用户配置 |
| `veil status` / `veil stop` | 查看 / 停止 |

可选环境变量：`VEIL_DATA_DIR`、`VEIL_MASTER_KEY`、`VEIL_BIND`、`VEIL_MODE=server`。旧的 `DGW_*` 仍然有效。

## 许可证

Apache-2.0。没有任何出网遥测。

