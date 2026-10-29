#!/usr/bin/env python3
"""Verify and report the real proxy message-cache benchmark, stdlib only."""
import argparse
import hashlib
import json
import math
import pathlib
import platform
import re
import statistics
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument("directory", type=pathlib.Path)
args = parser.parse_args()
directory = args.directory
rows = [json.loads(line) for line in (directory / "raw.jsonl").read_text().splitlines()]
groups = {(p, s): [r for r in rows if (r["profile"], r["scenario"]) == (p, s)] for p in ("none", "sparse", "dense") for s in ("stable", "rolling")}
assert sum(map(len, groups.values())) == len(rows)
turns = len(next(iter(groups.values())))
assert turns >= 20 and all(len(group) == turns for group in groups.values())
results = []
for (profile, scenario), records in groups.items():
    assert [r["turn"] for r in records] == list(range(1, turns + 1))
    assert len({r["prime_ns"] for r in records}) == 1
    for r in records:
        hits = 8 if scenario == "rolling" and r["turn"] > 1 else 9
        assert r["hit_messages"] == hits and r["miss_messages"] == 10 - hits
        assert r["outputs_equal"] and r["roundtrip_ok"]
        assert r["placeholders"] == {"none": 0, "sparse": 600, "dense": 9760}[profile]
        assert 10_000_000 < r["wire_bytes"] < 10_001_000
        assert 0 < r["charged_bytes"] <= 64 * 1024 * 1024
        assert 10 <= r["entries"] <= 4096
        assert r["order"] == ("full_first" if r["turn"] % 2 else "cache_first")
    steady = records if scenario == "stable" else records[1:]
    result = {"profile": profile, "scenario": scenario, "samples": len(steady)}
    for stage in ("full", "cache"):
        values = sorted(r[stage + "_ns"] for r in steady)
        assert all(v > 0 for v in values)
        result[stage + "_median_ms"] = statistics.median(values) / 1e6
        result[stage + "_p95_ms"] = values[math.ceil(.95 * len(values)) - 1] / 1e6
    result["reduction_pct"] = 100 * (1 - result["cache_median_ms"] / result["full_median_ms"])
    result["prime_ms"] = records[0]["prime_ns"] / 1e6
    result["charged_peak_MiB"] = max(r["charged_bytes"] for r in records) / (1024 * 1024)
    result["session_reduction_including_prime_pct"] = 100 * (1 - (sum(r["cache_ns"] for r in records) + records[0]["prime_ns"]) / sum(r["full_ns"] for r in records))
    results.append(result)
sources = ["crates/veil/src/proxy/message_cache.rs", "crates/veil-store/src/sqlite.rs", "crates/veil-engine/src/rules.rs", "crates/veil/examples/history_cache_bench.rs"]
metadata = {"base_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(), "macOS": platform.mac_ver()[0], "architecture": platform.machine(), "sources_sha256": {p: hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest() for p in sources}, "paired_samples": len(rows), "results": results}
log = directory / "tests.log"
if log.exists():
    metadata["tests_passed"] = sum(map(int, re.findall(r"test result: ok\. (\d+) passed", log.read_text())))
(directory / "summary.json").write_text(json.dumps(metadata, indent=2) + "\n")
names = {"none": "无敏感", "sparse": "稀疏", "dense": "密集"}
scenarios = {"stable": "固定历史，复用9MB", "rolling": "滚动多轮，复用8MB"}
lines = ["# 生产消息缓存：9MB 历史 + 1MB 新增复测", "",
    f'基线提交 `{metadata["base_commit"]}`，本机 macOS {metadata["macOS"]} / {metadata["architecture"]}，release 构建。使用真实 MessageCache、规则引擎和加密 SQLite；不是此前的简化缓存原型。', "",
    "## 方法", "",
    f"- 每条件 {turns} 组成对测量，交替先执行全量处理与缓存处理。输入使用 Responses/Codex 的 input[]，9条历史消息各1MB，当前用户消息1MB；1MB=1,000,000字节。固定历史为重复请求对照；滚动场景加入本轮用户消息和1MB新助手回复，淘汰最老两条历史，保持9MB历史。",
    "- 固定场景每轮命中9条；滚动首轮命中9条，之后命中8条，新增助手回复和当前用户消息共2MB均重新处理。滚动稳态排除首轮。",
    "- 两条路径均包含完整JSON解析和输出编码。缓存路径还包括完整消息指纹、LRU访问、映射ID去重、真实SQLite单事务批量查找/密文验证/访问时间刷新，以及未命中消息的脱敏和缓存写入。计时不含HTTP、alias hint、网络、模型、还原校验。",
    "- 两条路径涉及的映射先预热，避免先执行路径的新插入使后一条路径获利；本轮不量化新增映射冷写成本。9MB缓存填充基于已解析历史、映射已预热，耗时单列。",
    "- 缓存使用默认64MiB全局预算、16MiB单条上限、10分钟空闲过期、最多4096条。与原型每轮只保留当前10条不同，生产LRU会保留近期较老消息，直到容量淘汰或空闲过期；计费包含已脱敏JSON、映射引用和元数据估算，进程RSS还包含请求与临时对象。", "",
    "## 稳态中位耗时（ms）", "",
    "| 密度 | 场景 | 全量处理 | 生产缓存 | 降幅 | 缓存计费峰值 MiB |",
    "|---|---|---:|---:|---:|---:|",
]
for r in results:
    lines.append(f'| {names[r["profile"]]} | {scenarios[r["scenario"]]} | {r["full_median_ms"]:.2f} | {r["cache_median_ms"]:.2f} | {r["reduction_pct"]:.1f}% | {r["charged_peak_MiB"]:.2f} |')
lines += ["", "## 验证与边界", "",
    f'- {len(rows)} 组输出字节相等、完整JSON相等、占位符数量和完整还原验证通过。',
    f'- Rust测试通过数量：{metadata.get("tests_passed", "未提供日志")}。覆盖Chat/Codex代理链路、用户及规则隔离、角色/工具/内容修改、映射缺失重建、实际清理版本、密文损坏错误与事务回滚、LRU和容量/空闲过期、禁用及超大条目、并发请求。',
    '- 规则指纹与不可变编译策略版本共同限定缓存；映射代次与存储实例也限定缓存。命中时验证映射存在、属于当前creator、密文可解密，并批量更新访问时间；缺失重做脱敏，存储异常保留错误。',
    '- 生产规则和缓存配置沿用启动时配置快照，重启后使用新配置；本次没有扩展管理接口的规则热更新或映射清理功能。SqliteStore实际purge会更新缓存代次；外部删除而未通知代次时，批量验证仍会发现缺失。', "",
    "## 复现", "", "```sh",
    "SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk VEIL_SKIP_UI_BUILD=1 cargo build --release --offline -p veil --example history_cache_bench",
    f"target/release/examples/history_cache_bench {turns} > {directory}/raw.jsonl",
    f"python3 scripts/privacy_eval/summarize_history_cache.py {directory}", "```", "",
    "原始样本：[raw.jsonl](raw.jsonl)；汇总及源文件指纹：[summary.json](summary.json)；测试日志：[tests.log](tests.log)。", ""]
(directory / "REPORT.md").write_text("\n".join(lines))
print("\n".join(lines[lines.index("## 稳态中位耗时（ms）"):lines.index("## 验证与边界")]))
