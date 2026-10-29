#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Serial real Codex -> production Veil gateway/SQLite -> ChatGPT model evaluation."""
import argparse, collections, datetime, hashlib, json, os, pathlib, re, subprocess, tempfile, time
ROOT=pathlib.Path(__file__).resolve().parents[2]
PH=re.compile(r'\{\{[A-Z][A-Z0-9_]*_[0-9A-HJKMNP-TV-Z]{26}\}\}')
def dump(p,v):p.write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def load(p):return json.loads(p.read_text())
def readrows(p):return [json.loads(l) for l in p.read_text().splitlines() if l.strip()]
def strings(v,path=()):
 if isinstance(v,str):yield path,v
 elif isinstance(v,list):
  for i,x in enumerate(v):yield from strings(x,path+(i,))
 elif isinstance(v,dict):
  for k,x in v.items():yield from strings(x,path+(k,))
def restore_map(text,mapping):return PH.sub(lambda m:mapping.get(m.group(),m.group()),text)
def spans(text,mapping):
 out=[];pos=0;cursor=0
 for m in PH.finditer(text):
  pos+=len(text[cursor:m.start()]);value=mapping.get(m.group())
  if value is not None:out.append((pos,pos+len(value),m.group()));pos+=len(value)
  else:pos+=len(m.group())
  cursor=m.end()
 return out

def prepare(base):
 formal=readrows(base/'data/pii-bench-zh/data/pii_bench_zh.jsonl');chat=readrows(base/'data/pii-bench-zh/data/pii_bench_zh_chat.jsonl');ext=readrows(base/'data/pii_bench_zh_extension/all.jsonl')
 for row in formal+chat+ext:
  for e in row['entities']:assert row['text'][e['start']:e['end']]==e['text']
 byid={r['id']:r for r in formal+chat+ext}
 chosen=[];covered=set();needed={'person','phone','id_number','address','email','bank_card','passport','license_plate'}
 while needed-covered:
  r=max(formal,key=lambda r:len(({e['type'] for e in r['entities']}&needed)-covered))
  assert r not in chosen
  chosen.append(r);covered|={e['type'] for e in r['entities']}
 singles=chosen+[byid[i] for i in ('zh_chat_000001','zh_chat_000051','zh_ext_public_domain_194','zh_ext_api_key_171','zh_ext_public_ip_022')]
 goodip=next(r for r in ext if r['entities'] and all(e['type'] in ('public_ip','ip_address') for e in r['entities']) and 'ssh ops@' in r['text'])
 singles.append(goodip)
 continuous=[r for r in formal if {'phone','id_number'}<={e['type'] for e in r['entities']}][:3]
 formatted=[byid['zh_chat_000001'],byid['zh_chat_000051'],byid['zh_chat_000018']]
 extension=[byid['zh_ext_api_key_171'],goodip,byid['zh_ext_api_key_183']]
 sessions=[('continuous',continuous),('formatted_chat',formatted),('credentials_ip',extension)]
 lengths=[(n,readrows(base/f'performance/data/constructed/data/high_{n}.jsonl')[0]) for n in (1000,5000,30000)]
 return singles,sessions,lengths,formal,chat,ext

SYSTEM='这些内容均来自合成隐私测试集，不是真实客户资料。不要使用工具、读取文件或执行命令。请严格遵循输出格式；任何 {{...}} 形式的占位符必须逐字保留，不得解释、展开或修改。'
def full_prompt(row):return SYSTEM+'\n请原样返回 BEGIN_DATA 与 END_DATA 之间的完整文本，不要输出边界、解释或代码围栏。\nBEGIN_DATA\n'+row['text']+'\nEND_DATA'
def memory_prompt(label,row,keys):return SYSTEM+'\n请将下面整段原文记为 '+label+'（若已存在则覆盖其当前值）。\nBEGIN_DATA\n'+row['text']+'\nEND_DATA\n只返回 JSON 对象，键为 '+', '.join(keys)+'，各键的值必须为对应资料的完整原文，不能概括。'

def main():
 ap=argparse.ArgumentParser();ap.add_argument('--output',type=pathlib.Path,required=True);ap.add_argument('--presidio',type=pathlib.Path,default=pathlib.Path('/Users/caic/code/presidio'));ap.add_argument('--smoke',action='store_true');ap.add_argument('--timeout',type=int,default=180);args=ap.parse_args()
 out=args.output.resolve();out.mkdir(parents=True,exist_ok=False);wire=out/'wire';wire.mkdir();turnsdir=out/'turns';turnsdir.mkdir()
 config=pathlib.Path.home()/'.codex/config.toml';before=digest(config) if config.exists() else None
 text=config.read_text() if config.exists() else '';top=text.split('\n[',1)[0]
 model=re.search(r'^model\s*=\s*"([^"]+)"',top,re.M);model=model.group(1) if model else None
 effort=re.search(r'^model_reasoning_effort\s*=\s*"([^"]+)"',top,re.M);effort=effort.group(1) if effort else None
 base=args.presidio/'.vscode/test/zh-baseline-extension';singles,sessions,lengths,formal,chat,ext=prepare(base)
 env=dict(started_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),model=model,reasoning_effort=effort or 'model_default',transport='Codex CLI HTTP/SSE, ChatGPT login',upstream='https://chatgpt.com/backend-api/codex',store='isolated temporary SqliteStore',serial=True,smoke=args.smoke,cli_version=subprocess.check_output(['codex','--version'],text=True,stderr=subprocess.DEVNULL).strip(),base_config_sha256=before,sources={str(p.relative_to(ROOT)):digest(p) for p in list((ROOT/'crates').glob('*/src/**/*.rs'))+[ROOT/'crates/veil/examples/codex_e2e_gateway.rs',pathlib.Path(__file__)]},data_sha256={str(p.relative_to(args.presidio)):digest(p) for p in [base/'data/pii-bench-zh/data/pii_bench_zh.jsonl',base/'data/pii-bench-zh/data/pii_bench_zh_chat.jsonl',base/'data/pii_bench_zh_extension/all.jsonl']},gateway_binary_sha256=digest(ROOT/'target/release/examples/codex_e2e_gateway'))
 dump(out/'environment.json',env);results=[];allrows={r['id']:r for r in formal+chat+ext}
 for _,r in lengths:allrows[r['id']]=r
 dump(out/'selected_data.json',list({r['id']:r for r in singles+[r for _,rs in sessions for r in rs]+[r for _,r in lengths]}.values()))
 with tempfile.TemporaryDirectory(prefix='veil-codex-e2e-') as temp:
  temp=pathlib.Path(temp);work=temp/'work';work.mkdir()
  with (out/'gateway.stderr.log').open('w') as err:
   gateway=subprocess.Popen([str(ROOT/'target/release/examples/codex_e2e_gateway'),str(wire),str(temp/'mappings.db')],stdout=err,stderr=err)
   try:
    deadline=time.monotonic()+20
    while not (wire/'ready.json').exists():
     if gateway.poll() is not None:raise RuntimeError('Gateway exited before ready')
     if time.monotonic()>deadline:raise RuntimeError('Gateway did not start')
     time.sleep(.1)
    endpoint='http://127.0.0.1:'+str(load(wire/'ready.json')['proxy_port'])+'/codex'
    def turn(case,prompt,expected,session=None,group='single',source_ids=()):
     idx=len(results)+1;stem=f'{idx:03}.{case}';replypath=turnsdir/(stem+'.reply.txt');replypath.write_text('')
     startids={p.name for p in wire.glob('*.client.request.meta.json')};started=time.perf_counter();events_path=turnsdir/(stem+'.events.jsonl');error_path=turnsdir/(stem+'.stderr.log')
     cmd=['codex','exec','--ignore-user-config','--skip-git-repo-check','--sandbox','read-only','-C',str(work),'--json','-o',str(replypath)]
     options={'model_provider':'veil_e2e','model_providers.veil_e2e.name':'Veil E2E','model_providers.veil_e2e.base_url':endpoint,'model_providers.veil_e2e.wire_api':'responses','model_providers.veil_e2e.requires_openai_auth':True,'model_providers.veil_e2e.supports_websockets':False,'model_providers.veil_e2e.request_max_retries':0,'model_providers.veil_e2e.stream_max_retries':0,'web_search':'disabled'}
     if effort:options['model_reasoning_effort']=effort
     for k,v in options.items():cmd+=['-c',k+'='+json.dumps(v)]
     if model:cmd+=['-m',model]
     cmd+=['resume',session,'-'] if session else ['-']
     dump(turnsdir/(stem+'.input.json'),dict(case=case,prompt=prompt,expected=expected,session=session,source_ids=source_ids,command=cmd))
     print('turn',idx,case,'resume' if session else 'new',flush=True)
     with events_path.open('w') as stdout,error_path.open('w') as stderr:
      try:run=subprocess.run(cmd,input=prompt,text=True,stdout=stdout,stderr=stderr,timeout=args.timeout);code=run.returncode
      except subprocess.TimeoutExpired:code=-999
     elapsed=time.perf_counter()-started;reply=replypath.read_text().strip();events=[]
     for l in events_path.read_text().splitlines():
      try:events.append(json.loads(l))
      except ValueError:pass
     sid=next((e.get('thread_id') for e in events if e.get('type')=='thread.started'),session)
     completed=any(e.get('type')=='turn.completed' for e in events)
     tools=[e for e in events if e.get('item',{}).get('type') in ('command_execution','mcp_tool_call','web_search','file_change')]
     reqids=[]
     for p in sorted(wire.glob('*.client.request.meta.json')):
      if p.name not in startids and load(p)['path'].endswith('/responses'):reqids.append(load(p)['request_id'])
     deadline=time.monotonic()+3
     while reqids and not all((wire/f'{rid:04}.client.timing.json').exists() for rid in reqids) and time.monotonic()<deadline:time.sleep(.05)
     if isinstance(expected,dict):
      try:exact=json.loads(reply)==expected
      except ValueError:exact=False
     else:exact=reply==expected
     record=dict(index=idx,case=case,group=group,session_id=sid,resumed_session=session,source_ids=list(source_ids),exit_code=code,turn_completed=completed,unexpected_tool_calls=len(tools),cli_wall_seconds=elapsed,reply=reply,expected=expected,exact_reply=exact,request_ids=reqids)
     results.append(record);dump(out/'turn_results.json',results)
     print('  completed',completed,'exact',exact,'seconds',round(elapsed,3),'requests',len(reqids),flush=True)
     if not completed or code!=0 or not sid:raise RuntimeError('Codex model call failed; see saved diagnostic files for '+stem)
     if tools:raise RuntimeError('Unexpected model tool execution')
     return sid
    if args.smoke:
     r=singles[0];turn('smoke',full_prompt(r),r['text'],source_ids=[r['id']])
    else:
     for i in range(3):turn(f'control_{i+1}',SYSTEM+'\n只回复 E2E_OK。','E2E_OK',group='control')
     for row in singles:turn('single_'+row['id'],full_prompt(row),row['text'],source_ids=[row['id']])
     for name,rows in sessions:
      a,b,c=rows
      sid=turn(name+'_remember_A',memory_prompt('A',a,['A']),{'A':a['text']},group=name,source_ids=[a['id']])
      turn(name+'_recall_A',SYSTEM+'\n回忆刚才记住的 A。只返回 JSON 对象，A 的值是完整原文。',{'A':a['text']},sid,group=name,source_ids=[a['id']])
      turn(name+'_add_B',memory_prompt('B',b,['A','B']),{'A':a['text'],'B':b['text']},sid,group=name,source_ids=[a['id'],b['id']])
      turn(name+'_replace_A',memory_prompt('A',c,['A','B']),{'A':c['text'],'B':b['text']},sid,group=name,source_ids=[a['id'],b['id'],c['id']])
     for n,row in lengths:
      target=next(line for line in row['text'].splitlines() if line.strip())
      prompt=SYSTEM+f'\n请记住下面文档，稍后我会询问其第一行。现在只回复 E2E_CONTEXT_OK。\nBEGIN_CONTEXT\n'+row['text']+'\nEND_CONTEXT'
      sid=turn(f'context_{n}_load',prompt,'E2E_CONTEXT_OK',group=f'context_{n}',source_ids=[row['id']])
      turn(f'context_{n}_recall',SYSTEM+'\n回忆刚才文档的第一行，只返回 JSON 对象 {"first_line":"第一行完整原文"}。不要概括。',{'first_line':target},sid,group=f'context_{n}',source_ids=[row['id']])
   finally:
    gateway.terminate()
    try:gateway.wait(timeout=10)
    except subprocess.TimeoutExpired:gateway.kill();gateway.wait()
 env['finished_utc']=datetime.datetime.now(datetime.timezone.utc).isoformat();env['base_config_unchanged']=(digest(config) if config.exists() else None)==before;dump(out/'environment.json',env)
 print('Completed real model calls:',len(results),flush=True)

if __name__=='__main__':main()
