# Veil

**化名出门，真名回家。**

你把 `.env` 丢进对话框，是为了让它帮你改代码。它改是改了——密码也已经躺在别人的服务器上。

Veil 挂在这台电脑的门口。机密先换成化名再出门；模型的回答进门，化名再变回真名。你看到的还是原文。窗外那台模型，从头到尾只看见化名。

[下载](https://github.com/Mouseww/veil/releases/latest) · [English](README.md) · Apache-2.0 · 无遥测

---

## 一眼看懂

你输入：

```
查一下 mysql://root:Pass123@10.1.2.3:3306/app，电话打 13800138000
```

模型实际收到：

```
查一下 {{CONNSTR_01ARZ3NDEKTSV4RRFFQ69G5FAV}}，电话打 {{PHONE_01ARZ3NDEKTSV4RRFFQ69G5FAV}}
```

它照样能推理、给建议。回答回到你屏幕上时，连接串和手机号已经变回原文。

同一段机密，整场对话都用同一个化名，它不会把一个人认成两个。

**Key 不用交给 Veil。** 登录、Token 还在原来的软件里。纱帘只拦正文，证件原样出门。

---

## 一行装上，就能用

```powershell
irm https://raw.githubusercontent.com/Mouseww/veil/main/scripts/install.ps1 | iex
```

浏览器弹出 http://127.0.0.1:18788 就是装好了。再在新终端跑一句：

```bat
veil setup
```

勾选你正在用的软件。Veil 会读走每个软件**现在的 API 地址**（官方或自定义中转都行），给它单独开一个本机端口，再把软件指过来。不同工具、不同上游，互不覆盖。然后重启这些软件，照常聊天。官方 Claude 登录不用另填 Key。

也可以去 [Releases](https://github.com/Mouseww/veil/releases/latest) 下载 `veil-windows-x64.exe` 双击，效果一样。

---

## 名单里没有的，自己指过来

Veil 不绑死某几个 Agent。只要工具能改 **Base URL**（或环境变量），就能走这道帘子。

把工具里的 API 地址改成下面其中一个，**Key 还用原来的**：

| 工具讲的协议 | 填这个 |
|---|---|
| Anthropic（Claude Code 这类） | `http://127.0.0.1:18787` |
| OpenAI 兼容（Cursor、Codex、大多数中转） | `http://127.0.0.1:18787/v1` |

管理页给自己看：http://127.0.0.1:18788

走公司中转 / LiteLLM 时：管理页「上游」填中转根地址，工具里的 Key 用中转那把。

Cursor、Python、环境变量等细接法见 [使用说明](docs/使用.md)。

---

## 谁化名，你说了算

打开管理页 → **规则**。

出厂会拦：私钥、API Key、Token、数据库连接串、登录密码、手机号、身份证、IP。邮箱默认关掉，误伤太多。

点开关就能停一条。点开一行能改正则、词表、优先级。底下可以加自己的：项目代号、内部人名，化名就会变成 `{{CODENAME_…}}`。

白名单里的字永远不替换（`localhost` 已经在里面）。扫不干净就 **502，宁可不聊也不出网**。

规则怎么写，见 [使用说明 · 规则](docs/使用.md#规则)。

---

## UI
<img width="1439" height="729" alt="image" src="https://github.com/user-attachments/assets/4aa16ca0-6942-48de-a86a-c1b6e67486aa" />
<img width="1426" height="726" alt="image" src="https://github.com/user-attachments/assets/afdf6327-34ae-4b26-bc45-9001d635cbad" />
<img width="1152" height="632" alt="image" src="https://github.com/user-attachments/assets/9ef630e2-8c44-462d-af70-22a39734eaa8" />
<img width="1399" height="648" alt="image" src="https://github.com/user-attachments/assets/004ef3a7-b335-49b3-889a-d008098da82a" />




管理页点「检查更新」。或 `veil update`。

双击即启动。`veil stop` 停掉。映射表加密落在本机，没有任何统计往外打。

