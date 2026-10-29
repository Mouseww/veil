#!/usr/bin/env python3
"""Summarize diagnostic stages and isolated rules, retaining raw samples."""
import json
import pathlib
import statistics
import sys

directory = pathlib.Path(sys.argv[1])
rows = [json.loads(line) for line in (directory / "stages.jsonl").read_text().splitlines()]
stages = [r for r in rows if r["kind"] == "stages"]
isolated = [r for r in rows if r["kind"] == "isolated_rule"]
assert len(stages) == 9 and len(isolated) == 30
summary = []
for row in stages:
    assert row["production_equal"] and row["append_equal"] and row["roundtrip_ok"]
    assert len(row["samples"]) == 5
    for sample in row["samples"]:
        assert sample["total_ns"] == sum(sample[k + "_ns"] for k in ("parse", "detect", "mapping", "replace", "encode"))
    summary.append({"mb": row["mb"], "profile": row["profile"], "hits": row["hits"], **{
        k.removesuffix("_ns") + "_ms": statistics.median(s[k] for s in row["samples"]) / 1e6
        for k in row["samples"][0]
    }})
rule_summary = [{"profile": r["profile"], "rule": r["rule"], "median_ms": statistics.median(r["samples_ns"]) / 1e6} for r in isolated]
(directory / "stages_summary.json").write_text(json.dumps({"stages": summary, "isolated_rules_10MB": rule_summary}, indent=2) + "\n")
names = {"none": "无敏感", "sparse": "稀疏", "dense": "密集"}
lines = ["# 请求隐私处理阶段诊断", "",
    "沿用前轮同机 release 构建、完全相同的合成请求和默认规则。本次每个阶段条件测 5 次，表中为各阶段中位数，单位 ms。", "",
    "诊断程序复现单条 messages 请求的处理顺序，直接调用生产 find_hits、SQLite get_or_insert、占位符格式化及 String::replace_range；为了独立计时，先逆序获取全部映射，再逆序替换。每次输出均与生产 desensitize_json 结果精确比较，并完整还原验证。它是阶段诊断，不是前轮生产管线重测；各阶段中位数之和不必等于总耗时中位数。", "",
    "映射阶段包含 HMAC、连接锁、SQL 查询、AES-GCM 验证解密和 last_access 更新，未进一步把加密与 SQL 分离。JSON 输出阶段包括构造结果对象和序列化。数据库已预热，阶段总耗时不含还原、候选算法及校验。", "",
    "| 请求 | 密度 | JSON 解析 | 识别与区间处理 | SQLite 映射 | 占位符格式化与替换 | 输出构造/编码 | 阶段总耗时 |",
    "|---|---|---:|---:|---:|---:|---:|---:|",
]
for r in summary:
    values = " | ".join(f'{r[k + "_ms"]:.2f}' for k in ("parse", "detect", "mapping", "replace", "encode", "total"))
    lines.append(f'| {r["mb"]}MB | {names[r["profile"]]} | {values} |')
lines += ["", "## 一次拼接替换的局部对比", "",
    "候选算法使用同一组命中和映射，正序追加原文片段和占位符，预留 String 容量，避免每次 replace_range 移动尾部。每次均检查输出与原算法精确相等。此对比只覆盖字符串构造，不代表端到端提速；生产代码未修改。", "",
    "| 请求 | 密度 | 原替换 ms | 一次拼接 ms | 局部倍数 | 生产还原 ms |",
    "|---|---|---:|---:|---:|---:|",
]
for r in summary:
    if r["profile"] != "none":
        lines.append(f'| {r["mb"]}MB | {names[r["profile"]]} | {r["replace_ms"]:.2f} | {r["append_ms"]:.2f} | {r["replace_ms"] / r["append_ms"]:.1f} | {r["restore_ms"]:.2f} |')
lines += ["", "## 10MB 单规则独立识别", "",
    "每个单规则条件测 3 次。规则单独作用于全文，与完整管线中按优先级扫描剩余间隙的上下文不同；这些耗时不可相加，也不可直接当作移除规则的收益。", "",
    "| 规则 | 无敏感 ms | 稀疏 ms | 密集 ms |", "|---|---:|---:|---:|",
]
for rule in sorted({r["rule"] for r in rule_summary}):
    times = [next(r["median_ms"] for r in rule_summary if r["profile"] == p and r["rule"] == rule) for p in ("none", "sparse", "dense")]
    lines.append("| " + rule + " | " + " | ".join(f"{t:.2f}" for t in times) + " |")
lines += ["", "## 复现", "", "```sh",
    "SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk cargo build --release --offline -p veil-store --example request_stage_bench",
    "target/release/examples/request_stage_bench > reports/privacy-eval/2026-10-29-request-size/stages.jsonl",
    "python3 scripts/privacy_eval/summarize_request_stages.py reports/privacy-eval/2026-10-29-request-size",
    "```", "", "原始数据：[stages.jsonl](stages.jsonl)；汇总：[stages_summary.json](stages_summary.json)。", ""]
(directory / "STAGES.md").write_text("\n".join(lines))
print("\n".join(lines))
