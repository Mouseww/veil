#!/usr/bin/env python3
"""Validate and summarize request_size_bench JSONL (stdlib only)."""
import json
import math
import pathlib
import statistics
import sys

directory = pathlib.Path(sys.argv[1])
rows = [json.loads(line) for line in (directory / "raw.jsonl").read_text().splitlines()]
assert {(r["mb"], r["profile"]) for r in rows} == {
    (mb, profile) for mb in (1, 5, 10) for profile in ("none", "sparse", "dense")
}
assert len(rows) == 9
summary = []
for row in rows:
    assert row["bytes"] == row["mb"] * 1_000_000 and row["roundtrip_ok"]
    overhead = len(json.dumps({"messages": [{"role": "user", "content": ""}]}, separators=(",", ":")).encode())
    expected = (row["bytes"] - overhead) // row["stride"] * 4 if row["stride"] else 0
    assert row["hits"] == expected
    assert len(row["cold_ns"]) == 3
    assert len(row["warm_ns"]) == len(row["detect_ns"]) >= 20
    result = {k: row[k] for k in ("mb", "profile", "bytes", "hits", "unique_mappings")}
    for stage in ("detect", "cold", "warm"):
        samples = row[stage + "_ns"]
        assert all(v > 0 for v in samples)
        result[stage + "_median_ms"] = statistics.median(samples) / 1e6
        if stage != "cold":
            result[stage + "_p95_ms"] = sorted(samples)[math.ceil(len(samples) * .95) - 1] / 1e6
    result["warm_MB_s"] = row["mb"] * 1000 / result["warm_median_ms"]
    summary.append(result)
(directory / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
names = {"none": "无敏感信息", "sparse": "稀疏", "dense": "密集"}
lines = [
    "# 1MB / 5MB / 10MB 请求隐私处理性能实测", "",
    "日期：2026-10-29。本机 macOS 26.6、arm64，Rust 1.97.1，release 优化构建。",
    "基础提交：`adb43b4e2ef523ce017c386e0a9dadac73095df4`。使用默认内置规则（邮箱默认关闭）与独立临时加密 SQLite；不修改真实配置、数据库或生产逻辑。", "",
    "## 测量口径", "",
    "- 1MB = 1,000,000 字节，尺寸为完整 UTF-8 JSON 请求体，形如 messages / role=user / content。正文为确定性合成英文文档。",
    "- 稀疏：每 65,536 字节正文插入 4 个敏感项；密集：每 4,096 字节插入 4 个。每块包含唯一手机号、IPv4、API key，以及重复的时区。尾部不足一块仅填充普通文本。",
    "- 识别：仅 find_hits；脱敏管线：JSON 解析 → 默认规则识别 → 占位符替换与 SQLite 加密映射读写 → JSON 序列化。输入构造、规则编译、数据库建表均在计时外。",
    "- 冷映射：3 次，每次使用新建空数据库，报告中位数。热映射：预热一次后串行测 20 次；识别同样测 20 次，报告中位数和最近秩 P95（第 19 个排序样本）。P95 样本量有限，仅供本机比较。",
    "- 各条件使用一个固定合成正文，按尺寸和密度顺序执行，未随机交错；结果不代表所有自然语言、代码或图片负载。吞吐按输入字节 / 热脱敏中位耗时计算，不代表并发吞吐。",
    "- 每个条件的 3 次冷测均检查占位符数量和完整 JSON 还原相等；另检查每次独立识别的命中数符合构造预期。校验耗时不计入脱敏耗时。",
    "- 这是单请求本机隐私处理耗时，不含 HTTP 接收/转发、认证、alias hint、模型推理、流式响应和网络；不是 HTTP 并发容量测试。冷映射指映射未命中，不代表进程/磁盘冷缓存。默认请求上限 32MiB，三种尺寸均低于上限。", "",
    "## 结果", "",
    "| 请求 | 密度 | 命中数 | 识别中位数 ms | 首次脱敏中位数 ms | 热脱敏中位数 ms | 热脱敏 P95 ms | 热脱敏 MB/s |",
    "|---|---|---:|---:|---:|---:|---:|---:|",
]
for r in summary:
    lines.append(f'| {r["mb"]}MB | {names[r["profile"]]} | {r["hits"]} | {r["detect_median_ms"]:.2f} | {r["cold_median_ms"]:.2f} | {r["warm_median_ms"]:.2f} | {r["warm_p95_ms"]:.2f} | {r["warm_MB_s"]:.2f} |')
lines += ["", "## 复现", "", "在仓库根目录执行：", "", "```sh",
    "SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk cargo build --release --offline -p veil-store --example request_size_bench",
    "target/release/examples/request_size_bench 20 > reports/privacy-eval/2026-10-29-request-size/raw.jsonl",
    "python3 scripts/privacy_eval/summarize_request_size.py reports/privacy-eval/2026-10-29-request-size",
    "```", "", "其他机器使用本机有效的 SDK。原始纳秒样本见 [raw.jsonl](raw.jsonl)，派生指标见 [summary.json](summary.json)。", ""]
(directory / "REPORT.md").write_text("\n".join(lines))
print("\n".join(lines[lines.index("## 结果"):lines.index("## 复现")]))
