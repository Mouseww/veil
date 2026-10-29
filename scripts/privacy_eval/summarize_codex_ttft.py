#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Audit client-side TTFT against raw JSON-RPC notifications and real model SSE."""
import collections, hashlib, json, pathlib, statistics, sys
from run_codex_e2e import load,dump,strings,restore_map,PH,ROOT
from summarize_codex_e2e import events,text_events,text_time,complete,table,fmt

def main():
 out=pathlib.Path(sys.argv[1]);wire=out/'wire';env=load(out/'environment.json');turns=load(out/'turn_results.json');mapping={r['placeholder']:r['plaintext'] for r in load(wire/'mapping_audit.json')};records=[]
 assert digest_file(out/'runner_snapshot.py' if (out/'runner_snapshot.py').exists() else ROOT/'scripts/privacy_eval/run_codex_ttft.py')==env['runner_sha256']
 assert digest_file(ROOT/'target/release/examples/codex_e2e_gateway')==env['gateway_binary_sha256']
 submitted={}
 for line in (out/'client_sent.jsonl').read_text().splitlines():
  r=json.loads(line)
  if r['message'].get('method')=='turn/start':submitted[(r['message']['params']['threadId'],r['sent_ns'])]=r
 all_received=[json.loads(l) for l in (out/'client_received.jsonl').read_text().splitlines()]
 for t in turns:
  key=(t['thread_id'],t['submit_ns']);assert key in submitted
  ns=t['first_client_text_ns'];observed=[r for r in all_received if r['received_ns']>=t['submit_ns'] and r['received_ns']<=t['turn_completed_ns'] and r['message'].get('method')=='item/agentMessage/delta' and r['message'].get('params',{}).get('threadId')==t['thread_id'] and r['message']['params'].get('delta')]
  assert observed and observed[0]['received_ns']==ns
  assert all(r['message']['params']['turnId']==t['turn_id'] for r in observed)
  assert abs((ns-t['submit_ns'])/1e6-t['client_ttft_ms'])<1e-9
  if t['action_ns'] is not None:assert abs((ns-t['action_ns'])/1e6-t['action_to_first_text_ms'])<1e-9
  assert (t['action_ns'] or t['submit_ns'])<=t['submit_ns']<=t['flushed_ns']<=ns<=t['turn_completed_ns']
  assert t['delta_matches_final']
  assert len(t['request_ids'])==1
  rid=t['request_ids'][0];stem=f'{rid:04}';ct=load(wire/(stem+'.client.timing.json'));ut=load(wire/(stem+'.upstream.timing.json'));cm=load(wire/(stem+'.client.request.meta.json'))
  ce=events((wire/(stem+'.client.response.bin')).read_bytes(),ct);ue=events((wire/(stem+'.upstream.response.bin')).read_bytes(),ut);uc,_=complete(ue);cc,_=complete(ce)
  assert uc and cc and uc['response']['model']==env['model'] and ut['status']==200
  assert text_events(ce)==restore_map(text_events(ue),mapping)
  assert not PH.findall(text_events(ce))
  ci=load(wire/(stem+'.client.request.json'));ui=dict(strings(load(wire/(stem+'.upstream.request.json'))))
  for p,s in strings(ci):assert restore_map(ui[p],mapping)==s
  gateway_ms=(text_time(ce)-cm['started_us'])/1000
  # Different timestamp origins; only subtract elapsed durations, never raw clocks.
  row=dict(t,proxy_received_to_first_text_ms=gateway_ms,client_pre_post_overhead_ms=t['client_ttft_ms']-gateway_ms,actual_response_model=uc['response']['model'],input_items=len(ci.get('input',[])),input_roles=[v.get('role') for v in ci.get('input',[]) if isinstance(v,dict) and v.get('type','message')=='message'])
  assert row['client_pre_post_overhead_ms']>=-1
  records.append(row)
 bygroup={g:[r for r in records if r['group']==g] for g in dict.fromkeys(r['group'] for r in records)}
 for name in ('continuous','formatted_chat','credentials_ip'):
  if name not in bygroup:continue
  rs=bygroup[name]
  if len(rs)!=4:continue
  assert len({r['thread_id'] for r in rs})==1
  n=rs[0]['input_roles'].count('user');assert [r['input_roles'].count('user') for r in rs]==list(range(n,n+4))
  assert [r['input_roles'].count('assistant') for r in rs]==[0,1,2,3]
 summary=dict(turns=len(records),verified_first_text_events=len(records),exact_reply_turns=sum(t['exact_reply'] for t in records),stream_restoration_exact=len(records),client_clock='perf_counter_ns',first_process_to_text_ms=records[0]['process_to_first_text_ms'],appserver_initialize_ms=env['appserver_initialize_ms'],base_config_unchanged=env['base_config_unchanged'],groups={g:dict(turns=len(rs),median_client_ttft_ms=statistics.median(r['client_ttft_ms'] for r in rs),median_action_to_first_text_ms=statistics.median(r['action_to_first_text_ms'] for r in rs if r['action_to_first_text_ms'] is not None),min_client_ttft_ms=min(r['client_ttft_ms'] for r in rs),max_client_ttft_ms=max(r['client_ttft_ms'] for r in rs)) for g,rs in bygroup.items()})
 assert env['base_config_unchanged']
 dump(out/'request_results.json',records);dump(out/'summary.json',summary);dump(out/'verification.json',dict(status='passed',client_timestamps_recomputed=len(records),first_nonempty_text_notifications_verified=len(records),real_upstream_responses_verified=len(records),source_binary_hashes='matched',multi_turn_history='verified',config_unchanged=True))
 lines=['# 完整客户端 TTFT 补测','',f"时间：{env['started_utc']} 至 {env.get('finished_utc','未完成')}；模型 `{env['model']}`，reasoning effort `{env['reasoning_effort']}`。复用上一轮的同一批输入与预期，串行实际请求模型。",'', '## 在哪里看、计时从哪里开始','', '**下表“客户端完整 TTFT（含建会话）”是测试客户端开始准备这一轮消息，到该客户端实际收到首个非空助手文本通知的时间。** 新会话时包含 thread/start；后续轮沿用当前会话。为了方便和标准推理请求比较，同时列出“turn/start 后 TTFT”。', '', '| 指标 | 起点 | 终点 |', '| --- | --- | --- |', '| 客户端完整 TTFT（含建会话） | 客户端开始这一轮消息准备，包含新建会话及诊断准备 | 客户端读到首个非空 item/agentMessage/delta |', '| turn/start 后 TTFT | 向 Codex 写入 turn/start 请求之前 | 同上 |', '| 启动到首文本（仅首轮） | 启动 Codex App Server 进程之前 | 首轮同一文本事件 |', '| Veil 收到请求到首文本 | Veil 收到 HTTP 推理请求 | Veil 发出还原后的首个非空文本事件 |', '', '客户端完整 TTFT 包含 Codex 会话准备、调度、HTTP 发送、Veil 脱敏/SQLite、网络与模型首文本生成、SSE 还原、Codex 解码以及 JSON-RPC 传输到测试客户端。warm turn/start 指标不包含进程启动/初始化和它之前的建会话；这些费用另列，不冒充已计入。', '', '停止计时的是实际客户端读取到的文本事件，排除状态、思考摘要、工具和完成通知。一次文本 delta 可能包含多个 token，故这是通常所说的首段文本可用 TTFT，并非逐 tokenizer token 的精确计时。测试客户端采用 Codex App Server，与 CLI exec 是不同接口；**不包含桌面 UI 绘制或屏幕刷新，不声称测得桌面界面像素出现时间。**', '', '## 实测结果','',f"真实模型轮次：{summary['turns']}；首文本事件独立复核：{summary['verified_first_text_events']}；预期回复正确：{summary['exact_reply_turns']}；真实流式还原一致：{summary['stream_restoration_exact']}。",'',f"Codex App Server 启动至 initialize 响应：{env['appserver_initialize_ms']:.3f} ms；首次启动至第一轮客户端首文本：{summary['first_process_to_text_ms']:.3f} ms。后续轮次复用该进程，没有每轮重启。",'']
 lines.append(table(['轮次','场景','客户端完整 TTFT（含建会话）ms','turn/start 后 TTFT ms','Veil 收到请求后首文本 ms','客户端整轮完成 ms','回复'],[[r['index'],r['case'],fmt(r['action_to_first_text_ms']),fmt(r['client_ttft_ms']),fmt(r['proxy_received_to_first_text_ms']),fmt((r['turn_completed_ns']-r['action_ns'])/1e6 if r['action_ns'] is not None else None),'通过' if r['exact_reply'] else '失败'] for r in records]))
 lines+=['','## 分组描述统计','', '这里只列本轮场景中位数和范围。用例与长度不同、各组轮数很少，不计算可用于生产 SLO 的 p95/p99；不能把跨场景混合分位数当成稳定性能上界。','']
 lines.append(table(['场景组','轮次','客户端完整 TTFT 中位数 ms','turn/start TTFT 中位数 ms','turn/start TTFT 范围 ms'],[[g,s['turns'],fmt(s['median_action_to_first_text_ms']),fmt(s['median_client_ttft_ms']),fmt(s['min_client_ttft_ms'])+'–'+fmt(s['max_client_ttft_ms'])] for g,s in summary['groups'].items()]))
 lines+=['','## 采集与复核方式','', '使用 Codex 官方 App Server stdio JSON-RPC：初始化后 thread/start 创建会话，以 turn/start 提交消息，监听 item/agentMessage/delta。读线程在收到完整通知行后立即用 perf_counter_ns 打点，先打点再 JSON 解码和写日志；计时起终点都在同一个客户端进程、同一单调时钟上。没有使用代理日志时间冒充客户端接收时间，也没有用整轮耗时推算 TTFT。方法见 [OpenAI 官方 App Server 文档](https://learn.chatgpt.com/docs/app-server)。', '', '后续轮次使用同一 threadId 的真实历史；复核 user/assistant 角色数量增长。模型、effort、输入资料与上一轮保持一致，但 App Server 复用进程、默认说明和会话生命周期可能与每轮启动 codex exec 不同，加之实时网络/模型波动，两轮不能直接相减推断某项优化收益。', '', '继续使用项目真实代理处理代码、隔离的加密 SQLite、实际 ChatGPT 上游；沿用先前的代理前后观察记录器。它的正文记录、映射审计、客户端测试准备、事件日志和额外本机跳转会增加开销。未修改真实 Codex 基础配置；禁用无关插件和 MCP 集成。', '', '本次补测关注 TTFT 与实际还原，不重新将回显成功当成全部隐私遮盖成功。默认规则存在的识别缺口仍按先前效果报告解释，脱敏前后正文和实际模型流保留供复查。', '', '## 原始证据与复现','', '- turn_results.json：每轮提交、首文本和完成的原始纳秒时间以及派生 TTFT。', '- client_sent.jsonl / client_received.jsonl：真正写入和真正收到的 JSON-RPC 消息及客户端时间戳。', '- turns/*.first_text.json：每轮实际触发 TTFT 停表的首个非空文本通知。', '- turns/*.received.jsonl：每轮流式事件；wire/ 保存实际模型出入站正文、SSE 与分块时间。', '- request_results.json / summary.json / verification.json：复核后的指标、分组统计与检查结果。', '', '```bash', 'python3 scripts/privacy_eval/run_codex_ttft.py --prior reports/privacy-eval/2026-10-28/codex-e2e --output reports/privacy-eval/new-codex-ttft', 'python3 scripts/privacy_eval/summarize_codex_ttft.py reports/privacy-eval/new-codex-ttft', '```', '', '需要真实网络、现有 Codex 登录、本机端口和已编译的 codex_e2e_gateway。--smoke 仅验证第一条请求，正式补测为全部 31 轮。']
 (out/'REPORT.md').write_text('\n'.join(lines)+'\n');print(json.dumps(summary,ensure_ascii=False,indent=2))
def digest_file(path):return hashlib.sha256(path.read_bytes()).hexdigest()
if __name__=='__main__':main()
