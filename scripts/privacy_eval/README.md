# 本地隐私识别评测

## 大请求性能测试（1MB / 5MB / 10MB）

离线测完整 JSON 请求体，使用默认规则与独立临时加密 SQLite，覆盖无敏感信息、每 64KiB 四个敏感项、每 4KiB 四个敏感项。MB 按 1,000,000 字节计算。每条件测 3 次空映射首次脱敏、20 次热映射脱敏及独立识别，并校验命中数、占位符数量和完整还原。计时包括 JSON 解析和输出序列化，不包括网络和模型。

```bash
SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk cargo build --release --offline -p veil-store --example request_size_bench
mkdir -p reports/privacy-eval/request-size
target/release/examples/request_size_bench 20 > reports/privacy-eval/request-size/raw.jsonl
python3 scripts/privacy_eval/summarize_request_size.py reports/privacy-eval/request-size
```

输出 `raw.jsonl`、`summary.json`、`REPORT.md`。其他机器使用其正常 SDK。汇总脚本当前记录本轮机器与基础提交信息，换机器复测时需同步更新报告环境描述。

进一步拆分热映射的解析、识别、SQLite 映射、字符串替换与输出编码，并对照一次拼接替换算法、测量完整还原与 10MB 单规则耗时：

```bash
SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk cargo build --release --offline -p veil-store --example request_stage_bench
target/release/examples/request_stage_bench > reports/privacy-eval/request-size/stages.jsonl
python3 scripts/privacy_eval/summarize_request_stages.py reports/privacy-eval/request-size
```

阶段诊断每条件 5 次，单规则每条件 3 次，输出 `STAGES.md`。诊断先获取全部映射、再执行替换，逐次检查与生产脱敏输出精确相等。单规则耗时受规则优先级和剩余间隙上下文影响，不可直接相加为生产识别耗时。

## 9MB 历史 + 1MB 新增的历史复用实验

对比全量处理与消息级缓存原型，包含完整 JSON 解析、完整消息对象 SHA-256 指纹校验、缓存复制、未命中脱敏和输出编码。覆盖固定 9MB 历史的重复请求对照，以及滚动对话中复用 8MB、重新处理新助手回复和当前用户消息共 2MB 的场景。每条路径映射均预热；缓存首次填充单列。只增加离线示例，不给生产代理启用缓存。

```bash
SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk cargo build --release --offline -p veil-store --example history_reuse_bench
mkdir -p reports/privacy-eval/history-reuse
target/release/examples/history_reuse_bench 20 > reports/privacy-eval/history-reuse/raw.jsonl
python3 scripts/privacy_eval/summarize_history_reuse.py reports/privacy-eval/history-reuse
```

每种密度、场景测 20 轮成对请求并交替执行顺序，逐次验证输出字节相等、完整还原和占位符数量。另验证正文、角色、工具字段、用户身份、实际规则变化和实际映射清理后的缓存失效。版本失效由示例显式通知；实际接入还需映射生命周期通知和缓存容量管理。汇总输出 `summary.json` 与 `REPORT.md`。

生产实现复测（含 SQLite 单事务映射验证、密文校验及访问时间刷新，使用 Responses/Codex `input[]`）：

```bash
SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk VEIL_SKIP_UI_BUILD=1 cargo build --release --offline -p veil --example history_cache_bench
mkdir -p reports/privacy-eval/history-cache-production
target/release/examples/history_cache_bench 20 > reports/privacy-eval/history-cache-production/raw.jsonl
python3 scripts/privacy_eval/summarize_history_cache.py reports/privacy-eval/history-cache-production
```

生产缓存按全局预算执行 LRU/空闲过期，保留近期历史而非每轮只保留当前请求，汇总会报告缓存计费峰值。映射对两条路径均预热，首次消息缓存填充单列。配置项和失效行为见 [使用说明](../../docs/使用.md#历史消息缓存)。

## 本地语料评测

本工具复用 Presidio 的中文归档，调用 Veil 真实引擎、JSON 脱敏/还原及加密 SQLite 存储。完整结果与解释见 [实测报告](../../reports/privacy-eval/2026-10-28/REPORT.md) 和 [定位分析](../../reports/privacy-eval/2026-10-28/ANALYSIS.md)。生产规则不在本任务中修改。

```bash
SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk cargo build --release --offline -p veil-store --example privacy_probe
python3 scripts/privacy_eval/run.py --presidio /Users/caic/code/presidio --output reports/privacy-eval/new-run
python3 scripts/privacy_eval/render.py reports/privacy-eval/new-run
python3 scripts/privacy_eval/verify.py reports/privacy-eval/new-run
```

输出目录必须不存在。Python 仅需标准库，Rust 依赖沿用 workspace。SDKROOT 为本机链接器兼容性设置，其他机器可使用其正常 SDK。`--duration-ms` 控制每条件循环窗口，默认 1000 ms，并发条件至少 3000 ms。实际总时间还包含全量计分、冷映射写入和原始样本压缩，不止窗口时间。

- `run.py`：加载并校验全部数据、评分、规则消融、密度扫描和线程并发诊断。
- `render.py`：从 JSON 重建数值报告，不重新运行识别。
- `verify.py`：用中文/emoji 字符范围、边界错、负例验证计分语义，复核全部预测的字节/字符范围、还原结果及原始延迟分位数。
- `crates/veil-store/examples/privacy_probe.rs`：只负责调用生产代码，JSONL 交互；不修改真实配置、不访问网络、不使用用户数据库。

每条效果记录保存全部标注、完整预测和错误片段；终端只打印组名和进度。原始数据包含合成隐私值，结果在本地保存。性能原始记录只保存调用耗时，条件中保留负载哈希；可按固定 seed 从来源数据重建。长文本效果集与性能六条抽样集的口径不同，不能混用。

默认规则以 `builtin_ruleset()` 为准。邮箱开启为独立消融，不自动启用其他缺失类别。精确实体与敏感字符覆盖分别报告，不把规则设计上的整字段替换全部当成隐私泄漏。所有调用均是本机离线管线；线程测试不是实际 HTTP 服务容量验收。


## Codex 真实模型补充测试

使用现有 ChatGPT 登录，通过独立临时 SQLite 和项目真实代理链路串行执行 31 轮，覆盖单轮、多轮记忆/更正、1,000/5,000/30,000 字符文档。需要允许本机端口与网络，并由 Codex 保存用于 resume 的测试会话。

```bash
SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk cargo build --release --offline -p veil --example codex_e2e_gateway
python3 scripts/privacy_eval/run_codex_e2e.py --output reports/privacy-eval/new-codex-e2e
python3 scripts/privacy_eval/summarize_codex_e2e.py reports/privacy-eval/new-codex-e2e
python3 scripts/privacy_eval/verify_codex_e2e.py reports/privacy-eval/new-codex-e2e
```

`--smoke` 仅运行一条真实模型请求；正式报告使用完整 31 轮。观察记录器会保存合成测试正文、映射与原始响应，排除凭据头部，额外引入诊断开销。模型及 reasoning effort 沿用用户顶层配置，不修改基础配置文件。

将全部结果合为一个可离线打开的 HTML，并嵌入全量原始结果 ZIP：

```bash
python3 scripts/privacy_eval/consolidate.py reports/privacy-eval/2026-10-28 --output reports/privacy-eval/2026-10-28-complete.html
```


## 完整客户端 TTFT

通过 Codex App Server 的 `turn/start` → `item/agentMessage/delta` 在同一个测试客户端、同一单调时钟采集时间；区别于 Veil 收到请求到首文本的代理侧指标。另列建会话及首次进程启动等待，不包含桌面 UI 绘制。

```bash
python3 scripts/privacy_eval/run_codex_ttft.py --prior reports/privacy-eval/2026-10-28/codex-e2e --output reports/privacy-eval/new-codex-ttft
python3 scripts/privacy_eval/summarize_codex_ttft.py reports/privacy-eval/new-codex-ttft
```

本轮曾因容量繁忙和格式校验异常分批继续。完整报告保留三批原始日志、采样脚本快照及所有失败；`merge_codex_ttft.py` 针对本轮三批记录合并，不是通用自动重试器。`--start-index` 用于明确继续场景，若处于多轮会话中须从该会话首轮重新建立历史。
