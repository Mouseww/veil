#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Audit real captured requests/SSE, independently score privacy and restoration."""
import collections, json, pathlib, re, statistics, sys
from run_codex_e2e import load, dump, strings, spans, restore_map, PH
PREFIX={'PHONE':'phone','IDCARD':'id_number','IP':'ip_address','APIKEY':'api_key','EMAIL':'email'}
ENABLED={'phone','id_number','ip_address','api_key'}
def norm(t):return 'ip_address' if t=='public_ip' else t

def events(raw,timing):
 result=[]
 chunks=timing.get('chunks',[])
 for m in re.finditer(rb'(?:^|\n)data:\s*([^\r\n]*)',raw):
  try:v=json.loads(m.group(1))
  except (ValueError,UnicodeDecodeError):continue
  at=next((c['at_us'] for c in chunks if c['offset']+c['bytes']>=m.end()),None)
  result.append((v,at))
 return result

def text_events(es):return ''.join(v.get('delta','') for v,_ in es if v.get('type')=='response.output_text.delta')
def text_time(es):return next((at for v,at in es if v.get('type')=='response.output_text.delta' and v.get('delta')),None)
def complete(es):return next(((v,at) for v,at in es if v.get('type')=='response.completed'),(None,None))
def fmt(n):return '—' if n is None else f'{n:.3f}'
def percent(n,d):return '—' if not d else f'{n/d*100:.2f}%'
def table(head,rows):return '\n'.join(['| '+' | '.join(head)+' |','| '+' | '.join(['---']*len(head))+' |']+['| '+' | '.join(map(str,r))+' |' for r in rows])

def main():
 out=pathlib.Path(sys.argv[1]);wire=out/'wire';turns=load(out/'turn_results.json');env=load(out/'environment.json');selected=load(out/'selected_data.json');audit=load(wire/'mapping_audit.json')
 mapping={r['placeholder']:r['plaintext'] for r in audit};requests=[];entity_records=[];unique={};stable=collections.defaultdict(set)
 for r in audit:stable[r['plaintext']].add(r['placeholder'])
 for turn in turns:
  for rid in turn['request_ids']:
   stem=f'{rid:04}';client=load(wire/(stem+'.client.request.json'));up=load(wire/(stem+'.upstream.request.json'))
   cm=load(wire/(stem+'.client.request.meta.json'));um=load(wire/(stem+'.upstream.request.meta.json'))
   ct=load(wire/(stem+'.client.timing.json'));ut=load(wire/(stem+'.upstream.timing.json'))
   ue=events((wire/(stem+'.upstream.response.bin')).read_bytes(),ut);ce=events((wire/(stem+'.client.response.bin')).read_bytes(),ct)
   uptext=text_events(ue);cltext=text_events(ce);uc,uc_at=complete(ue);cc,cc_at=complete(ce)
   firstup=text_time(ue);firstcl=text_time(ce);headers=load(wire/(stem+'.upstream.headers.meta.json'))
   base=cm['started_us'];
   ms=lambda at:(at-base)/1000 if at is not None else None
   ups=dict(strings(up));changed=0;unexplained=[];seen=set();occurrences=[]
   for path,original in strings(client):
    masked=ups.get(path)
    if masked is None:continue
    if original!=masked:changed+=1
    restored=restore_map(masked,mapping)
    if restored!=original:unexplained.append(list(path))
    ranges=spans(masked,mapping) if restored==original else []
    for row in selected:
     cursor=0
     while True:
      start=original.find(row['text'],cursor)
      if start<0:break
      cursor=start+len(row['text'])
      for e in row['entities']:
       s=start+e['start'];en=start+e['end'];t=norm(e['type']);key=(path,s,en,t)
       if key in seen:continue
       seen.add(key)
       covered=sum(any(rs<=i<re for rs,re,_ in ranges) for i in range(s,en))
       exact=any(rs==s and re==en and PREFIX.get(ph[2:].split('_',1)[0])==t for rs,re,ph in ranges)
       item=dict(request_id=rid,turn=turn['index'],case=turn['case'],source_id=row['id'],type=t,path=list(path),start=s,end=en,text=original[s:en],covered_chars=covered,total_chars=en-s,fully_masked=covered==en-s,exact_typed_span=exact,placeholders=[ph for rs,re,ph in ranges if rs<en and re>s])
       occurrences.append(item);entity_records.append(item)
       uid=(row['id'],t,e['start'],e['end'])
       if uid not in unique:unique[uid]=dict(source_id=row['id'],type=t,start=e['start'],end=e['end'],text=e['text'],fully_masked_every_time=True,observations=0)
       unique[uid]['fully_masked_every_time'] &= item['fully_masked'];unique[uid]['observations']+=1
   record=dict(response_model=uc.get('response',{}).get('model') if uc else None,response_id=uc.get('response',{}).get('id') if uc else None,request_id=rid,turn=turn['index'],case=turn['case'],group=turn['group'],model=up.get('model'),status=ut['status'],client_stream_complete=bool(cc),upstream_stream_complete=bool(uc),restore_delta_exact=bool(uc and cc) and cltext==restore_map(uptext,mapping),cli_matches_client_delta=turn['reply']==cltext.strip(),upstream_text=uptext,client_text=cltext,unknown_upstream_placeholders=[p for p in PH.findall(uptext) if p not in mapping],remaining_client_placeholders=PH.findall(cltext),changed_request_strings=changed,unexplained_request_changes=unexplained,client_request_bytes=cm['body_bytes'],upstream_request_bytes=um['body_bytes'],request_input_items=len(client.get('input',[])),input_message_roles=[x.get('role') for x in client.get('input',[]) if isinstance(x,dict) and x.get('type','message')=='message'],annotated_occurrences=len(occurrences),fully_masked_occurrences=sum(x['fully_masked'] for x in occurrences),gateway_pre_upstream_ms=(um['started_us']-base)/1000,upstream_headers_ms=ms(headers['at_us']),upstream_first_text_ms=ms(firstup),client_first_text_ms=ms(firstcl),first_text_restore_gap_ms=(firstcl-firstup)/1000 if firstcl is not None and firstup is not None else None,gateway_response_completed_ms=ms(cc_at),cli_wall_ms=turn['cli_wall_seconds']*1000,usage=uc.get('response',{}).get('usage',{}) if uc else {})
   requests.append(record)
 dump(out/'request_results.json',requests);dump(out/'entity_observations.json',entity_records);dump(out/'unique_entities.json',list(unique.values()))
 per=collections.defaultdict(collections.Counter)
 for r in entity_records:per[r['type']].update(observations=1,fully_masked=r['fully_masked'],exact_span=r['exact_typed_span'],covered_chars=r['covered_chars'],total_chars=r['total_chars'])
 perunique=collections.defaultdict(collections.Counter)
 for r in unique.values():perunique[r['type']].update(entities=1,fully_masked_every_time=r['fully_masked_every_time'])
 for counters in (per,perunique):
  for counter in counters.values():
   for k in counter:counter[k]=int(counter[k])
 summary=dict(turns=len(turns),requests=len(requests),model_completed_turns=sum(t['turn_completed'] for t in turns),exact_reply_turns=sum(t['exact_reply'] for t in turns),http_2xx=sum(200<=r['status']<300 for r in requests),complete_streams=sum(r['client_stream_complete'] and r['upstream_stream_complete'] for r in requests),restoration_exact_requests=sum(r['restore_delta_exact'] for r in requests),cli_delta_exact_requests=sum(r['cli_matches_client_delta'] for r in requests),unexpected_tool_calls=sum(t['unexpected_tool_calls'] for t in turns),remaining_client_placeholders=sum(len(r['remaining_client_placeholders']) for r in requests),unexplained_request_changes=sum(len(r['unexplained_request_changes']) for r in requests),privacy_by_type=dict(per),unique_privacy_by_type=dict(perunique),unstable_plaintexts=[p for p,phs in stable.items() if len(phs)>1],mapping_entries=len(mapping),base_config_unchanged=env.get('base_config_unchanged'),token_usage_sum={k:sum(r['usage'].get(k,0) for r in requests) for k in ('input_tokens','output_tokens','total_tokens')})
 dump(out/'summary.json',summary)
 lines=['# Codex 真实模型端到端补充测试','',f"运行时间：{env['started_utc']} 至 {env.get('finished_utc','未完成')}。模型：`{env['model']}`；reasoning effort：`{env['reasoning_effort']}`；CLI：`{env['cli_version']}`。",'', '## 实际链路与范围','', '真实 Codex CLI → 本机诊断入口 → 项目生产 proxy_handler / JSON 脱敏 / 加密 SqliteStore / SSE 还原 → 本机上游记录器 → HTTPS ChatGPT Codex 后端 → 实际模型。没有模拟模型响应，没有并发。使用现有 ChatGPT 登录；请求头中的凭据、Cookie 与账户 ID 不写入记录。测试数据库和加密主密钥独立临时创建，结束后清理；不读取真实映射库、不修改 Codex 基础配置。', '', '测试在 production 路由前后加入观察记录器，并对 SqliteStore 作透传包装记录实际映射。正文抓取、映射审计和额外 loopback 跳转会增加开销，性能是这条带诊断链路的实际端到端耗时，不等于未插桩生产代理的净开销。全量离线测试使用 MemoryStore 的历史结果与本补充测试分开报告。', '', 'Codex 每次调用使用独立命令行配置覆盖，模型沿用用户基础配置；`--ignore-user-config` 排除其他用户配置干扰。多轮通过同一 thread ID 的 `codex exec resume` 延续真实历史，未手工拼接历史冒充续聊。执行工作目录为空临时目录，提示禁止工具，并检查 CLI 未执行工具。方法参考 [OpenAI 官方非交互模式文档](https://learn.chatgpt.com/docs/non-interactive-mode) 与 [CLI 命令文档](https://learn.chatgpt.com/docs/developer-commands)。', '', '## 总体结果','']
 lines.append(table(['项目','结果'],[['实际模型完成轮次',f"{summary['model_completed_turns']}/{summary['turns']}"],['最终回复符合预期',f"{summary['exact_reply_turns']}/{summary['turns']}"],['实际推理请求 HTTP 2xx',f"{summary['http_2xx']}/{summary['requests']}"],['上下游均收到 response.completed',f"{summary['complete_streams']}/{summary['requests']}"],['实际 SSE 文本还原精确一致',f"{summary['restoration_exact_requests']}/{summary['requests']}"],['CLI 回复与下游流文本一致',f"{summary['cli_delta_exact_requests']}/{summary['requests']}"],['客户端残留占位符',summary['remaining_client_placeholders']],['无法由审计映射解释的请求变化',summary['unexplained_request_changes']],['相同原文出现不同占位符的数量',len(summary['unstable_plaintexts'])],['意外工具调用',summary['unexpected_tool_calls']],['基础配置保持不变',summary['base_config_unchanged']]]))
 lines+=['', '“回复符合预期”同时依赖模型遵循回显/记忆指令与代理正确还原；“SSE 文本还原精确一致”独立比较真实上游输出经审计映射展开后的文本与真实下游输出，用于区分模型生成差异和代理还原缺陷。', '', '## 逐轮效果与端到端性能','', 'CLI 总耗时从启动 codex 命令到进程退出，含初始化、模型目录访问、网络及生成。首文本/完成从本机收到该推理请求起算，首文本按收到完整的非空 output_text.delta 事件所在 chunk 的时间记录；不是首个 SSE 控制事件。完成按 response.completed 到达时间记录。原始 chunk 时间支持复算，但不是 token 级精密计时。','']
 rows=[]
 for turn in turns:
  rr=[r for r in requests if r['turn']==turn['index']];r=rr[-1] if rr else {}
  rows.append([turn['index'],turn['case'],'通过' if turn['exact_reply'] else '失败',fmt(turn['cli_wall_seconds']*1000),fmt(r.get('upstream_first_text_ms')),fmt(r.get('client_first_text_ms')),fmt(r.get('gateway_response_completed_ms')),r.get('request_input_items','—')])
 lines.append(table(['轮次','场景','预期回复','CLI ms','上游首文本 ms','还原后首文本 ms','推理完成 ms','input 项数'],rows))
 lines+=['','## 多轮上下文','', '三个四轮会话分别覆盖规整 PII、聊天分组格式、凭据与 IP：第一轮记住 A，第二轮不重发值而回忆 A，第三轮新增 B 并回忆 A/B，第四轮覆盖 A 并保留 B。长上下文分别加载 1,000 / 5,000 / 30,000 字符高密度既有构造文本，再在第二轮回忆第一行。', '']
 lines.append(table(['会话组','轮数','精确回复','thread ID 数','后续轮重放历史 input 项数'],[[g,len(ts),sum(t['exact_reply'] for t in ts),len({t['session_id'] for t in ts}),', '.join(str(r['request_input_items']) for r in requests if r['group']==g)] for g in dict.fromkeys(t['group'] for t in turns if t['group'] not in ('single','control')) for ts in [[t for t in turns if t['group']==g]]]))
 lines+=['','## 模型实际输入中的隐私遮盖','', '根据实际客户端正文与上游正文按 JSON 路径逐字段对齐，使用 SqliteStore 审计中的真实占位符→原文映射验证替换，逐个标注范围计算遮盖。下表“出现次数”包含多轮重发的历史 user/assistant 内容；同一实体多次出现，不是独立样本。独立实体指选中源记录内的标注范围，长文本中重复拼接仍有相关性；“每次完整遮盖”要求所有被观察到的出现均完整覆盖。', '', '完整遮盖允许外层字段或更大范围一起替换，不等于类别/边界精确匹配。没有被覆盖的已知合成隐私值实际发送到了模型；它们是本轮要记录的识别缺口。默认邮箱关闭及未支持类别保持出厂行为，不在此测试中偷偷补齐。','']
 lines.append(table(['类别','出现次数','完整遮盖次数','完整遮盖率','字符覆盖率','独立标注范围','每次均完整遮盖'],[[t,c['observations'],c['fully_masked'],percent(c['fully_masked'],c['observations']),percent(c['covered_chars'],c['total_chars']),perunique[t]['entities'],perunique[t]['fully_masked_every_time']] for t,c in sorted(per.items())]))
 lines+=['','## 处理阶段诊断','', '前处理计时为收到客户端推理请求至上游记录器收到转发请求，含本机正文记录/解析、脱敏、SQLite 和本机转发开销；不等同于纯规则时间。首文本间隔为还原后首个非空文本到达时间减上游首个非空文本到达时间，包含占位符跨 delta 缓冲、SQLite 还原和转发。','']
 lines.append(table(['场景','前处理 ms','首文本还原间隔 ms','客户端输入字节','模型输入字节','input tokens','output tokens'],[[r['case'],fmt(r['gateway_pre_upstream_ms']),fmt(r['first_text_restore_gap_ms']),r['client_request_bytes'],r['upstream_request_bytes'],r['usage'].get('input_tokens','—'),r['usage'].get('output_tokens','—')] for r in requests]))
 lines+=['', '## 实测结论与边界', '', '任务成功与真实流式还原的逐轮核对结果见上表；它们不代表标注隐私全部被遮盖。实际请求核对仍发现分组手机号/身份证、IP 边界与模拟凭据格式漏检，邮箱默认关闭，其他未支持类别保持原文。应将模型任务成功率与出站隐私覆盖分开评价。', '', '每次启动 Codex 除推理外还会访问模型目录；CLI 总耗时包含这些初始化访问和进程生命周期，不能把 CLI 总耗时与推理完成时间的差值全部归为 Veil 开销。上下文长度指加入的既有测试文档，真实请求另有 Codex 默认说明与历史，所以实际请求字节数另列。', '', '多轮请求中实际历史 input 项数、三档文档加载和回忆的耗时分别见上表。部分轮次 CLI 额外插入 developer 或 reasoning 项，因此复核同时检查 user/assistant 历史角色数量和原文存在性，不以 input 总数固定递增二作为唯一依据。长文本每条件只有一次，不能据此宣称冷/热性能差异或稳定尾延迟。', '', '首文本还原间隔多数小于 1 ms，只描述第一个非空文本事件；若第一个事件是普通文字，它无法代表后续占位符跨 delta 缓冲的最大延迟。源码/二进制与数据哈希保存于本轮 environment.json，原离线测试环境归档不覆盖。', '', '现有 Codex 代理集成测试 subscription_routes_preserve_auth_redact_and_restore_stream 额外复跑通过（模拟上游的功能回归，与本轮 31 个真实模型调用分开）。']
 lines+=['','## 文件与复现','', '- `turn_results.json`：每轮预期、实际回复、CLI 耗时、thread ID 与推理请求编号。', '- `request_results.json`：每个真实推理请求的上下游文本、还原核对、阶段计时、usage 和历史 input 项数。', '- `entity_observations.json` / `unique_entities.json`：按原文字符范围逐实体核对的遮盖结果。', '- `selected_data.json`：从既有数据选出的原始文本与标注；`environment.json` 保存来源哈希、源码/二进制哈希和参数。', '- `wire/`：脱敏前后请求、真实响应流、分块时间、映射审计；头部仅保留 Content-Type/Encoding。', '- `turns/`：每次 Codex 调用的输入、期望、实际末条回复、JSONL 事件和诊断日志。', '', '```bash', 'SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk cargo build --release --offline -p veil --example codex_e2e_gateway', 'python3 scripts/privacy_eval/run_codex_e2e.py --output reports/privacy-eval/new-codex-e2e', 'python3 scripts/privacy_eval/summarize_codex_e2e.py reports/privacy-eval/new-codex-e2e', '```', '', '需要允许本机端口、真实网络、Codex 使用现有登录并保存测试会话。全部用例串行；本轮选样不是 9,008 条全量模型调用，不能把小样本回显成功率当成生产准确率。未运行并发、持续负载或固定到达率；单次长文本场景不计算生产 p95/p99，也不以 CLI 延迟减离线引擎均值推断网络耗时。']
 (out/'REPORT.md').write_text('\n'.join(lines)+'\n')
 print(json.dumps(summary,ensure_ascii=False,indent=2))
if __name__=='__main__':main()
