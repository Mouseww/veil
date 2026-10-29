#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Render collected measurements; does not rerun detection or load a model."""
import collections, json, pathlib, statistics, sys
out=pathlib.Path(sys.argv[1]); q=json.loads((out/'quality_summary.json').read_text()); perf=json.loads((out/'performance_summary.json').read_text()); env=json.loads((out/'environment.json').read_text())
def pct(v): return '—' if v is None else f'{v*100:.2f}%'
def table(headers,rows): return '\n'.join(['| '+' | '.join(headers)+' |','| '+' | '.join(['---']*len(headers))+' |']+['| '+' | '.join(map(str,row))+' |' for row in rows])
lines=['# Veil 离线隐私识别效果与性能实测','',f"测试启动：{env['started_utc']}；Git HEAD：`{env['git_head']}`。",'', '本节为最初的离线引擎与管线基线；新增的真实模型、多轮上下文测试另见 [Codex 端到端报告](codex-e2e/REPORT.md)，两轮测量范围和指标分别记录。', '', '## 测试范围与口径','', '复用 Presidio `.vscode/test/zh-baseline-extension` 归档：正式集 5,000 条、聊天集 3,000 条、扩展集 1,008 条，以及六个长度/密度构造组共 216 条。数据集原文件未修改；程序逐条验证标注片段与 text[start:end] 一致，其中 start/end 为 Unicode 字符索引，不代表人工逐条审核标注语义。数据文件哈希、源码哈希和工作区状态保存在 `environment.json`，规则快照保存在 `rules.json`。测试使用当时工作区中的引擎代码；walk.rs 的原有修改仅新增注释，不改变行为。forward.rs 的原有修改也仅为注释，该代理文件未在本轮离线测试中执行，仅作源码留档与静态分析。', '', '调用项目真实 `RuleSet::find_hits`、JSON 脱敏/还原和 `SqliteStore`。全量效果与还原验证使用 MemoryStore；SQLite 用于独立管线性能及现有存储功能测试。默认出厂规则与白名单；补充邮箱开启消融。未读取用户运行中配置/数据库，测试数据库为临时文件。没有模型、上游或外部网络调用。', '', '精确实体计分要求类别、Unicode 起止位置全部相同；边界错计 FP+FN。APIKEY→api_key、IDCARD→id_number、PHONE→phone、EMAIL→email、IP→ip_address，public_ip 合并为 ip_address。正式/聊天仅计其八类标注，扩展仅计三类，构造组计全部类别；非本数据来源标注类别不计 FP。TOKEN 不并入 APIKEY。额外报告实际脱敏敏感字符覆盖率，不以整段覆盖代替精确实体。没有约定实体 TN，不计算普通分类准确率。', '', '“已启用类别”限 phone、id_number、ip_address、api_key，邮箱开启时另加 email；这不代表出厂全部安全能力，也不能消除支持类别内规则覆盖不足。人名、地址、银行卡、护照、车牌、域名缺少内置识别；邮箱默认关闭。', '', '数据是已用于规则开发的合成集；高密度拼接继承未完整标注的外围内容，误报须复核。正式/聊天没有整条空标注负例，扩展有 72 条。不能作为真实生产泛化或有效身份证校验验收。', '', '## 效果结果','']
rows=[]
for name,s in q.items():
 a=s['scopes']['all']['micro']; e=s['scopes']['enabled']['micro']
 rows.append([name,s['records'],s['entities'],a['tp'],a['fp'],a['fn'],pct(a['precision']),pct(a['recall']),pct(a['f1']),pct(e['f1']),pct(s['sensitive_character_recall'])])
lines.append(table(['数据/配置','文本','标注','TP','FP','FN','P','R','F1','启用类 F1','敏感字符覆盖'],rows))
lines+=['','### 默认配置逐类别','']
rows=[]
for name in ('formal','chat','extension'):
 for t,m in q[name]['scopes']['all']['per_type'].items(): rows.append([name,t,m['tp'],m['fp'],m['fn'],pct(m['precision']),pct(m['recall']),pct(m['f1'])])
lines.append(table(['数据','类别','TP','FP','FN','P','R','F1'],rows))
lines+=['','### 整条完全正确与负例','']
lines.append(table(['数据','全类整条正确率','启用类整条正确率','负例条数','负例误报条数'],[[n,pct(s['scopes']['all']['record_accuracy']),pct(s['scopes']['enabled']['record_accuracy']),s['negative_records'].get('records',0),s['negative_records'].get('all_false_positive_records',0)] for n,s in q.items()]))
lines+=['','## 性能结果','', '所有性能用 release 优化构建。每组固定种子 4826 打乱后取六条，三个重复轮次，每条件至少 1 秒；样本 SHA-256 随条件保存。各组全量效果不抽样。预热及数据库初次插入排除在正式延迟之外。冷插入轮次另存 cold_pass_ns，包含六条的识别、JSON 和存储，不能视为纯写入耗时。', '', 'detect 是真实规则检测；memory / sqlite 包含 JSON 请求构建、编码、解码、遍历、检测、映射和输出编码，不包含还原。SQLite 为真实临时磁盘数据库、WAL、默认同步选项；每条件重新创建，正式循环复用已存在映射。后续请求仍会解密并执行 last_access 更新。每次检测/管线调用计时不包含 Python IPC；墙钟吞吐还包含循环/计时和字符计数开销。原始纳秒延迟保存为 gzip JSON 数组，可重算分位数。', '', '下表吞吐和分位数均为三个独立轮次结果的中位数，非合并样本分位数；最大值为三个轮次最大值。p99 来自这些短时、六文本循环，不能当成生产尾延迟上界。不是 HTTP/代理端到端性能，不能直接与历史 Presidio HTTP 吞吐比较。','']
rows=[]
for name in ['formal','chat','extension','low_1000','high_1000','low_5000','high_5000','low_30000','high_30000']:
 for mode in ('detect','memory','sqlite'):
  rs=[r for r in perf if r['label'].startswith(name+'.'+mode+'.r')]
  if not rs: continue
  med=lambda k:statistics.median(r[k] for r in rs)
  rows.append([name,mode,f"{med('rps'):.1f}",f"{med('p50_ms'):.4f}",f"{med('p95_ms'):.4f}",f"{med('p99_ms'):.4f}",f"{max(r['max_ms'] for r in rs):.3f}",sum(r['requests'] for r in rs)])
lines.append(table(['数据','路径','条/秒','p50 ms','p95 ms','p99 ms','最大 ms','次数'],rows))
lines+=['','### 独立规则/移除规则诊断','', '对 formal 和 high_30000 固定前六条运行单规则及逐条移除消融，每条件至少一秒；full 使用同一组文本。单规则包含该 RuleSet 自身匹配、提交和占用范围维护开销，不能相加当作生产各阶段占比。无命中的极短单规则测量可能接近时钟分辨率并记录 0 ns，不据此判断规则绝对成本。移除高优先级规则会改变后续可见文本和命中数量，耗时变化并非该规则的严格独立贡献。','']
rows=[]
for r in perf:
 if r['label'].startswith('profile.'):
  rows.append([r['cohort'],r.get('only','—'),r.get('omit','—'),f"{r['mean_ms']:.4f}",f"{r['p95_ms']:.4f}",r['requests']])
lines.append(table(['数据','仅启用','移除','均值 ms','p95 ms','次数'],rows))
lines+=['','### 并发与长度/密度诊断','', '混合负载 90 条短文本、9 条 high_5000、1 条 high_30000，固定种子生成，循环闭环运行；线程数 1/2/4/8，每条件至少 3 秒。同进程共享 SQLite 或相同规则的线程克隆。是本机线程可扩展性诊断，未做 HTTP 到达率、长时稳态、500 用户容量或故障恢复验收。','']
lines.append(table(['路径','线程','条/秒','p50 ms','p95 ms','p99 ms','次数'],[[r['mode'],r['threads'],f"{r['rps']:.1f}",f"{r['p50_ms']:.3f}",f"{r['p95_ms']:.3f}",f"{r['p99_ms']:.3f}",r['requests']] for r in perf if r['label'].startswith('concurrency.')]))
lines+=['','同一手机号加换行重复 16–1024 次，避免不同文本/不同类别造成密度混淆。这是人工压力诊断，不计入效果指标。','']
lines.append(table(['命中数','字符数','均值 ms','p95 ms','条/秒'],[[r['hits'],12*r['hits'],f"{r['mean_ms']:.4f}",f"{r['p95_ms']:.4f}",f"{r['rps']:.1f}"] for r in perf if r['label'].startswith('scaling.')]))
lines+=['','## 文件索引与复现','', '- `quality/*.records.jsonl`：完整文本、全部标注、预测范围和片段、漏报原因、误报、脱敏跳过与还原验证。', '- `quality/*.errors.jsonl`：只保留发生精确识别错误的记录。范围采用 Unicode 字符，不采用 Rust 字节偏移；预测也保留字节范围供源码定位。', '- `quality_summary.json` / `performance_summary.json`：结构化指标与所有条件。', '- `performance/*.samples.json.gz`：每次调用的纳秒延迟。', '- `supplemental_probes.json`：原始数据未覆盖的凭据/二进制启发式复核。', '- `ANALYSIS.md`：badcase 分类、源码定位、优化优先级及验证建议。', '', '```bash', 'SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk cargo build --release --offline -p veil-store --example privacy_probe', 'python3 scripts/privacy_eval/run.py --presidio /Users/caic/code/presidio --output reports/privacy-eval/new-run', 'python3 scripts/privacy_eval/render.py reports/privacy-eval/new-run', '```', '', '输出目录必须不存在；默认每条件一秒、并发至少三秒。初始 SDK 27.0 与本机链接器不兼容，使用本机已安装的 26.5 SDK 完成构建。未增加正式规则或修改生产识别实现。']
(out/'REPORT.md').write_text('\n'.join(lines)+'\n')
print(out/'REPORT.md')
