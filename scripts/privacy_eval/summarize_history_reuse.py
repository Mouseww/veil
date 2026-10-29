#!/usr/bin/env python3
"""Verify paired history-reuse evidence and write a local report (stdlib only)."""
import argparse
import hashlib
import json
import math
import pathlib
import platform
import statistics
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument("directory", type=pathlib.Path)
args = parser.parse_args()
directory = args.directory
raw = [json.loads(line) for line in (directory / "raw.jsonl").read_text().splitlines()]
checks = [r for r in raw if r["kind"] == "validation"]
assert len(checks) == 1
assert all(checks[0][key] is True for key in (
    "content_edit", "role_edit", "tool_field_edit", "creator_change",
    "rule_generation_change", "mapping_generation_change", "actual_rule_change", "actual_mapping_purge",
))
samples = [r for r in raw if r["kind"] == "sample"]
groups = {(p, s): [r for r in samples if (r["profile"], r["scenario"]) == (p, s)]
          for p in ("none", "sparse", "dense") for s in ("stable", "rolling")}
assert sum(map(len, groups.values())) == len(samples)
turns = len(next(iter(groups.values())))
assert turns >= 20 and all(len(group) == turns for group in groups.values())
summary = []
for (profile, scenario), records in groups.items():
    assert [r["turn"] for r in records] == list(range(1, turns + 1))
    assert len({r["cache_prime_ns"] for r in records}) == 1
    for r in records:
        expected_hits = 8 if scenario == "rolling" and r["turn"] > 1 else 9
        assert r["hit_messages"] == expected_hits and r["miss_messages"] == 10 - expected_hits
        assert r["hit_bytes"] == expected_hits * 1_000_000
        assert r["miss_bytes"] == (10 - expected_hits) * 1_000_000
        assert r["history_content_bytes"] == 9_000_000 and r["new_user_content_bytes"] == 1_000_000
        assert 10_000_000 < r["wire_bytes"] < 10_001_000
        assert r["retained_messages"] == 10
        assert r["outputs_equal"] and r["roundtrip_ok"]
        assert r["placeholders"] == {"none": 0, "sparse": 600, "dense": 9760}[profile]
        assert r["order"] == ("full_first" if r["turn"] % 2 == 1 else "cache_first")
        assert all(r[k] > 0 for k in ("full_ns", "cache_ns", "key_ns", "parse_ns", "encode_ns"))
    steady = records if scenario == "stable" else records[1:]
    result = {"profile": profile, "scenario": scenario, "measured_turns": turns,
              "steady_samples": len(steady), "reused_MB": steady[0]["hit_bytes"] / 1e6,
              "processed_MB": steady[0]["miss_bytes"] / 1e6}
    for key in ("full_ns", "cache_ns", "key_ns", "clone_ns", "redact_ns", "parse_ns", "encode_ns"):
        result[key.removesuffix("_ns") + "_median_ms"] = statistics.median(r[key] for r in steady) / 1e6
    for key in ("full_ns", "cache_ns"):
        values = sorted(r[key] for r in steady)
        result[key.removesuffix("_ns") + "_p95_ms"] = values[math.ceil(.95 * len(values)) - 1] / 1e6
    result["reduction_pct"] = 100 * (1 - result["cache_median_ms"] / result["full_median_ms"])
    result["cache_prime_ms"] = records[0]["cache_prime_ns"] / 1e6
    result["extra_prime_break_even_reuses"] = math.ceil(result["cache_prime_ms"] / (result["full_median_ms"] - result["cache_median_ms"])) if result["full_median_ms"] > result["cache_median_ms"] else None
    result["session_reduction_including_extra_prime_pct"] = 100 * (1 - (sum(r["cache_ns"] for r in records) + records[0]["cache_prime_ns"]) / sum(r["full_ns"] for r in records))
    result["cache_content_MB"] = statistics.median(r["retained_masked_content_bytes"] for r in steady) / 1e6
    summary.append(result)
source = pathlib.Path("crates/veil-store/examples/history_reuse_bench.rs")
metadata = {"production_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
            "macOS": platform.mac_ver()[0], "architecture": platform.machine(),
            "benchmark_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
            "validation": checks[0], "paired_samples": len(samples), "results": summary}
(directory / "summary.json").write_text(json.dumps(metadata, indent=2, ensure_ascii=False) + "\n")
names = {"none": "无敏感", "sparse": "稀疏", "dense": "密集"}
scenarios = {"stable": "固定历史", "rolling": "滚动多轮"}
lines = ["# 9MB 历史 + 1MB 新增：历史复用实验", "",
    f'生产基线：`{metadata["production_commit"]}`；本机 macOS {metadata["macOS"]} / {metadata["architecture"]}，release 构建。只增加离线诊断示例，没有给生产代理启用缓存。', "",
    "## 条件与计时", "",
    "- 每次完整请求包含 9 条历史消息和 1 条新增用户消息，每条正文精确 1,000,000 字节，总正文 10MB；完整 JSON 另含少量结构开销。历史角色包含 user 和 assistant。它与此前单条 10MB 正文的结构不同，绝对耗时不可直接混用。",
    f"- 每条件 {turns} 轮成对测量；同一请求交替先执行全量处理和缓存处理，降低顺序偏差。使用同一临时加密 SQLite，预热两条路径涉及的映射，排除先执行路径新插入映射给后一条路径带来的优势。本轮不量化新增映射冷写成本。",
    "- 固定历史：9MB 历史保持不变，每轮当前消息均改变；这是重复历史对照，不是累积对话。缓存命中 9MB，仅重新脱敏当前 1MB。",
    "- 滚动多轮：完成一轮后删除最老的两条历史，加入本轮用户消息和新生成的 1MB 助手回复，历史仍为 9MB。首轮命中 9MB，后续每轮命中 8MB；新增助手回复和当前用户消息合计 2MB 必须重新脱敏。稳态结果排除首轮。",
    "- 缓存路径计时包含完整请求 JSON 解析、逐消息完整对象序列化与 SHA-256 指纹、缓存查找与复制、未命中消息脱敏、非 messages 顶层字段脱敏、输出组装和完整 JSON 序列化。缓存只保留当前请求涉及的消息指纹与已脱敏消息。",
    "- 映射缓存与消息缓存分开：9MB 消息首次填充耗时单列，基于已解析历史，不含输入解析和输出编码，映射本身已预热。两条路径均不含 HTTP、alias hint、模型、网络及还原。还原与输出相等校验在计时外。",
    "- 每条消息的敏感值随消息编号变化，避免所有轮次只是重放同一条正文。无敏感正文也含唯一消息编号。稀疏/密集每条各 60/976 个敏感项，与前轮相同类别和密度。", "",
    "## 稳态结果（ms，中位数）", "",
    "| 密度 | 场景 | 复用 MB | 重新处理 MB | 全量处理 | 历史复用 | 耗时降低 |",
    "|---|---|---:|---:|---:|---:|---:|",
]
for r in summary:
    lines.append(f'| {names[r["profile"]]} | {scenarios[r["scenario"]]} | {r["reused_MB"]:.0f} | {r["processed_MB"]:.0f} | {r["full_median_ms"]:.2f} | {r["cache_median_ms"]:.2f} | {r["reduction_pct"]:.1f}% |')
lines += ["", "## 缓存路径分项（中位数 ms）", "",
    "各项中位数之和不要求等于总中位数；总耗时还包含控制逻辑、非消息字段处理和对象释放等。", "",
    "| 密度 | 场景 | 解析 | 指纹生成 | 缓存复制/插入 | 未命中脱敏 | 输出编码 |",
    "|---|---|---:|---:|---:|---:|---:|",
]
for r in summary:
    values = " | ".join(f'{r[k + "_median_ms"]:.2f}' for k in ("parse", "key", "clone", "redact", "encode"))
    lines.append(f'| {names[r["profile"]]} | {scenarios[r["scenario"]]} | {values} |')
lines += ["", "## 首次填充、摊销与内存", "",
    "首次填充为一次观测，不是稳定延迟分位数。保守地把这次额外 9MB 预填充也计入缓存方案，比较本轮全部请求累计耗时。缓存正文大小不含 HashMap、JSON 容器等开销，也不是进程峰值内存。", "",
    "| 密度 | 场景 | 9MB 填充 ms | 含额外填充的全会话降幅 | 缓存正文 MB |",
    "|---|---|---:|---:|---:|",
]
for r in summary:
    lines.append(f'| {names[r["profile"]]} | {scenarios[r["scenario"]]} | {r["cache_prime_ms"]:.2f} | {r["session_reduction_including_extra_prime_pct"]:.1f}% | {r["cache_content_MB"]:.2f} |')
lines += ["", "## 投入判断", "",
    "对本轮历史保持稳定、缓存可持续命中的 10MB 多轮负载，历史复用值得继续投入。它同时省去历史的规则匹配、区间维护、映射访问和替换；完整解析、指纹核验与输出成本已包含在结果中。滚动场景仍需重新处理新助手回复，不能把最后一条用户消息之外的内容全部跳过。",
    "推荐先实现有全局容量上限的消息级缓存原型，并接入真实客户端采集命中率。只有确认映射失效与版本更新可正确联动后再进入生产；本轮不是现成的生产缓存实现，也不代表任意业务请求都有相同命中率。", ""]
lines += ["", "## 正确性与落地条件", "",
    f"- {len(samples)} 组完整输出字节相等、JSON 相等、完整还原和占位符数量校验全部通过。",
    "- 缓存指纹覆盖完整消息对象。正文、role、工具字段变化必须未命中；creator、规则版本、映射版本变化清空缓存。另实际关闭规则、实际清理映射后验证重建输出与还原。",
    "- 版本变化由诊断程序显式传入；生产目前没有接入这套缓存及自动失效通知。接入前必须绑定规则更新、映射 TTL/清理/密钥轮换等事件，并解决缓存命中不触碰映射访问时间的问题。",
    "- 必须处理新增助手、工具和附件内容，不能简单跳过除最后一条用户消息以外的原文。缓存范围按 creator 隔离，顶层 system/tools 等变化仍需处理。",
    "- 当前每个实验缓存保留至多当前请求的 10 条消息；实际产品还需全局容量限制、淘汰策略及多请求并发验证。每个活跃大上下文约 10MB 缓存正文，缓存收益和内存预算需要一起评估。",
    "- 本轮验证了保持历史较稳定时的收益，不代表任意客户端、历史重写、提示词压缩或频繁规则/映射失效时仍有相同命中率。", "",
    "## 复现", "", "```sh",
    "SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk cargo build --release --offline -p veil-store --example history_reuse_bench",
    f"target/release/examples/history_reuse_bench {turns} > {directory}/raw.jsonl",
    f"python3 scripts/privacy_eval/summarize_history_reuse.py {directory}", "```", "",
    "原始记录：[raw.jsonl](raw.jsonl)；汇总：[summary.json](summary.json)。", ""]
(directory / "REPORT.md").write_text("\n".join(lines))
print("\n".join(lines[lines.index("## 稳态结果（ms，中位数）"):lines.index("## 缓存路径分项（中位数 ms）")]))
