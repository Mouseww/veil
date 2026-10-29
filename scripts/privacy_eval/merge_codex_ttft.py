#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Preserve failed attempts and combine independently verified TTFT batches."""
import json,pathlib,statistics,sys
from run_codex_e2e import load,dump
from summarize_codex_e2e import table,fmt

def main():
 root=pathlib.Path(sys.argv[1]);batches=[('',root),('recovery',root/'recovery'),('recovery2',root/'recovery2')]
 if not (root/'batch1_REPORT.md').exists():(root/'batch1_REPORT.md').write_bytes((root/'REPORT.md').read_bytes())
 records=[];startup=[]
 for name,path in batches:
  xs=load(path/'request_results.json');e=load(path/'environment.json')
  for r in xs:records.append(dict(r,batch=name or 'batch1',evidence_directory=name or '.'))
  startup.append(dict(batch=name or 'batch1',initialize_ms=e['appserver_initialize_ms'],process_to_first_text_ms=xs[0]['process_to_first_text_ms']))
 failed=load(root/'failed_attempt.json');event=failed['event'];assert event['message']['params']['turn']['status']=='failed';assert event['message']['params']['turn']['error']['codexErrorInfo']=='serverOverloaded';assert failed['client_ttft_ms'] is None
 for name,path in batches:
  for f in (path/'turns').glob('*.failed.json'):
   r=load(f);failedrow=dict(index=r['index'],case=r['case'],batch=name or 'batch1',client_ttft_ms=None,error=r['error'])
   records.append(failedrow)
 success=[r for r in records if r.get('first_client_text_ns') is not None]
 covered={r['index'] for r in records}|{failed['index']};assert covered==set(range(1,32))
 complete={r['index'] for r in success if r.get('action_ns') is not None}
 summary=dict(planned_cases=31,covered_cases=len(covered),attempts=len(records)+1,text_received_attempts=len(success),full_action_ttft_case_coverage=len(complete),reply_correct_attempts=sum(r['exact_reply'] for r in success),reply_incorrect_attempts=sum(not r['exact_reply'] for r in success),no_text_failures=sum(r.get('first_client_text_ns') is None for r in records)+1,capacity_failures=1,initial_capacity_failure=failed,process_startup=startup)
 dump(root/'combined_request_results.json',records);dump(root/'summary.json',summary)
 dump(root/'verification.json',dict(status='passed',all_31_cases_attempted=True,cases_with_complete_client_ttft=len(complete),actual_received_text_events_verified=len(success),failed_attempts_preserved=summary['no_text_failures'],semantic_reply_failures_preserved=summary['reply_incorrect_attempts'],note='Each batch separately verifies original client RPC timestamps, actual model SSE and source snapshot; failed or invalid-format replies are not disguised as successes.'))
 base=(root/'batch1_REPORT.md').read_text();definitions=base.split('## 在哪里看、计时从哪里开始',1)[1].split('## 实测结果',1)[0];method=base.split('## 采集与复核方式',1)[1]
 lines=['# 完整客户端 TTFT 补测','', '分段耗时与原因定位详见 [首文本延迟原因分析](LATENCY_ANALYSIS.md)。', '', '本报告来自真实 Codex App Server 测试客户端接收文本事件的时间戳，已补齐代理日志不能覆盖的客户端部分。模型为 gpt-6.1-sol、reasoning effort 为 medium，与前一轮相同；串行实际请求模型。', '', '## 在哪里看、计时从哪里开始',definitions,'## 完整结果与失败保留','',table(['指标','结果'],[['原定场景',summary['planned_cases']],['已尝试场景',summary['covered_cases']],['已采到完整客户端 TTFT 的场景',summary['full_action_ttft_case_coverage']],['实际模型请求次数（含重试/重建会话）',summary['attempts']],['收到首文本的请求',summary['text_received_attempts']],['未产生首文本的失败请求',summary['no_text_failures']],['收到文本但回复不符合预期',summary['reply_incorrect_attempts']]]),'', '首次采集在第 12 个场景遇到模型 serverOverloaded，无首文本，TTFT 为空；继续采集后第 24 个场景触发 JSON 格式问题，采样脚本提前退出。第 24 轮的 turn/start TTFT 已从原始客户端事件恢复，但准备阶段起点没有落盘，因此其“含建会话 TTFT”留空。随后从第 22 轮重新建立完整凭据会话并完成长上下文。所有批次与异常保留，不按延迟高低或回复正确与否筛掉样本。', '', '## 逐次实测：客户端首文本延迟','', '相同场景重复出现代表重试或重新建立上下文，不是独立样本。所有数字为毫秒。先前 codex exec 的整轮耗时与本次长驻 App Server 客户端的 TTFT 不是同一个指标。','']
 rows=[]
 for batch,_ in batches:
  label=batch or 'batch1'
  for r in records:
   if r['batch']!=label:continue
   if r.get('first_client_text_ns') is None:rows.append([label,r['index'],r['case'],'—','—','无文本',str(r.get('error'))]);continue
   rows.append([label,r['index'],r['case'],fmt(r['action_to_first_text_ms']),fmt(r['client_ttft_ms']),fmt(r['proxy_received_to_first_text_ms']),'通过' if r['exact_reply'] else '格式失败'])
  if not batch:rows.append(['batch1',12,failed['case'],'—','—','无文本','serverOverloaded'])
 lines.append(table(['批次','场景号','场景','客户端完整 TTFT（含建会话）ms','turn/start 后 TTFT ms','Veil 收到请求后首文本 ms','结果'],rows))
 lines+=['','## 首次启动等待','',table(['批次','进程启动至 initialize ms','进程启动至本批首次文本 ms'],[[r['batch'],fmt(r['initialize_ms']),fmt(r['process_to_first_text_ms'])] for r in startup]),'', '这是进程启动到本批首次文本事件的实测等待，不计入后续每轮提交后的 TTFT；不能将进程初始化时间等同于首次模型请求的全部冷启动成本。','', '## 多轮新增 badcase：历史 JSON 被二次脱敏破坏','', '在 recovery 批次的 credentials_ip_add_B 场景（编号 24），模型原始输出是合法 JSON；客户端收到的文本却在 A 字段末尾多出引号和花括号。实际映射审计表明 APIKEY 原文包含了 `API-Key: dummy-ak-…"}`：字段正则的非空白匹配把历史 assistant JSON 的闭合符一起吞入占位符。模型在新的 JSON 值中回显该占位符，正常还原后就引入了额外闭合符。', '', '该请求真实首文本已产生，所以有 TTFT；但预期回复验证失败。上游文本按真实映射展开后与下游一致，说明不能只靠“占位符字符串还原一致”证明结构化输出正确。必须把用户任务结果/嵌套 JSON 格式也纳入多轮回归。原始证据：recovery/format_badcase.json、recovery/wire/0016.upstream.response.bin、recovery/wire/0016.client.response.bin 和 recovery/wire/mapping_audit.json。', '', '修复方向是限制凭据值的边界、避免将历史结构化文本的闭合引号/括号吞入映射，并验证不同 JSON 排版及多轮重放。此次任务保留原规则，未混入修复后的测量。', '', '## 采集与复核方式',method,'', '额外文件：combined_request_results.json 汇总全部有文本的采样及恢复记录；failed_attempt.json 保存首次容量失败；runner_snapshot.py 和各恢复目录内同名文件保存各批次实际采样代码，哈希与对应 environment.json 核对。']
 (root/'REPORT.md').write_text('\n'.join(lines)+'\n');print(json.dumps({k:v for k,v in summary.items() if k not in ('initial_capacity_failure','process_startup')},ensure_ascii=False,indent=2))
if __name__=='__main__':main()
