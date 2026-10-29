#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Measure client-side TTFT using real Codex App Server streaming notifications."""
import argparse, datetime, hashlib, json, pathlib, queue, re, subprocess, tempfile, threading, time
from run_codex_e2e import ROOT, dump, load, digest

class Client:
 def __init__(self,command,out):
  self.out=out;self.queue=queue.Queue();self.serial=0;self.rows=[];self.error=(out/'appserver.stderr.log').open('w')
  self.started_ns=time.perf_counter_ns();self.process=subprocess.Popen(command,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=self.error,text=True,bufsize=1)
  self.log=(out/'client_received.jsonl').open('w')
  self.reader=threading.Thread(target=self.read,daemon=True);self.reader.start()
 def read(self):
  for line in self.process.stdout:
   at=time.perf_counter_ns()
   try:v=json.loads(line)
   except ValueError:continue
   record=dict(received_ns=at,message=v);self.rows.append(record);self.queue.put((at,v))
   self.log.write(json.dumps(record,ensure_ascii=False)+'\n');self.log.flush()
  self.queue.put((time.perf_counter_ns(),{'_eof':True}))
 def send(self,method,params,notification=False):
  self.serial+=1;rid=self.serial;v=dict(method=method,params=params)
  if not notification:v['id']=rid
  encoded=json.dumps(v,ensure_ascii=False)+'\n';at=time.perf_counter_ns();self.process.stdin.write(encoded);self.process.stdin.flush();flushed=time.perf_counter_ns()
  with (self.out/'client_sent.jsonl').open('a') as f:f.write(json.dumps(dict(sent_ns=at,flushed_ns=flushed,message=v),ensure_ascii=False)+'\n')
  return rid,at,flushed
 def receive(self,timeout=180):
  at,v=self.queue.get(timeout=timeout)
  if v.get('_eof'):raise RuntimeError('Codex App Server exited')
  if 'id' in v and 'method' in v:
   # No approval or tool action is accepted by the evaluation client.
   self.process.stdin.write(json.dumps({'id':v['id'],'error':{'code':-32601,'message':'Unsupported in privacy evaluation'}})+'\n');self.process.stdin.flush()
   raise RuntimeError('Unexpected server request: '+v['method'])
  return at,v
 def call(self,method,params):
  rid,started,_=self.send(method,params)
  deadline=time.monotonic()+180
  while time.monotonic()<deadline:
   at,v=self.receive()
   if v.get('id')==rid:
    if 'error' in v:raise RuntimeError(method+' failed: '+json.dumps(v['error']))
    return v['result'],started,at
  raise TimeoutError(method)
 def close(self):
  self.process.terminate()
  try:self.process.wait(timeout=10)
  except subprocess.TimeoutExpired:self.process.kill();self.process.wait()
  self.reader.join(timeout=5);self.log.close();self.error.close()

def main():
 ap=argparse.ArgumentParser();ap.add_argument('--prior',type=pathlib.Path,default=ROOT/'reports/privacy-eval/2026-10-28/codex-e2e');ap.add_argument('--output',type=pathlib.Path,required=True);ap.add_argument('--smoke',action='store_true');ap.add_argument('--start-index',type=int,default=1);args=ap.parse_args()
 out=args.output.resolve();out.mkdir(parents=True,exist_ok=False);wire=out/'wire';wire.mkdir();eventsdir=out/'turns';eventsdir.mkdir()
 prior=args.prior.resolve();plan=load(prior/'turn_results.json');priorenv=load(prior/'environment.json');model=priorenv['model'];effort=priorenv['reasoning_effort']
 plan=[t for t in plan if t['index']>=args.start_index]
 if args.smoke:plan=plan[:1]
 config=pathlib.Path.home()/'.codex/config.toml';configtext=config.read_text();original=digest(config)
 env=dict(started_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),model=model,reasoning_effort=effort,method='Codex App Server stdio JSON-RPC; turn/start -> client received item/agentMessage/delta',client_clock='Python perf_counter_ns, stamped immediately after reading a complete notification line',base_config_sha256=original,prior_plan_sha256=digest(prior/'turn_results.json'),selected_data_sha256=digest(prior/'selected_data.json'),gateway_binary_sha256=digest(ROOT/'target/release/examples/codex_e2e_gateway'),runner_sha256=digest(pathlib.Path(__file__)),serial=True,smoke=args.smoke)
 (out/'selected_data.json').write_bytes((prior/'selected_data.json').read_bytes());dump(out/'environment.json',env);(out/'runner_snapshot.py').write_bytes(pathlib.Path(__file__).read_bytes());results=[];threads={}
 with tempfile.TemporaryDirectory(prefix='veil-ttft-') as temp:
  temp=pathlib.Path(temp);work=temp/'work';work.mkdir()
  with (out/'gateway.stderr.log').open('w') as err:
   gateway=subprocess.Popen([str(ROOT/'target/release/examples/codex_e2e_gateway'),str(wire),str(temp/'mappings.db')],stdout=err,stderr=err);client=None
   try:
    deadline=time.monotonic()+20
    while not (wire/'ready.json').exists():
     if gateway.poll() is not None or time.monotonic()>deadline:raise RuntimeError('Gateway failed to start')
     time.sleep(.05)
    endpoint='http://127.0.0.1:'+str(load(wire/'ready.json')['proxy_port'])+'/codex'
    options={'model':model,'model_provider':'veil_ttft','model_providers.veil_ttft.name':'Veil TTFT','model_providers.veil_ttft.base_url':endpoint,'model_providers.veil_ttft.wire_api':'responses','model_providers.veil_ttft.requires_openai_auth':True,'model_providers.veil_ttft.supports_websockets':False,'model_providers.veil_ttft.request_max_retries':0,'model_providers.veil_ttft.stream_max_retries':0,'web_search':'disabled','approval_policy':'never','sandbox_mode':'read-only','analytics.enabled':False}
    if effort!='model_default':options['model_reasoning_effort']=effort
    for name in re.findall(r'^\[mcp_servers\.("[^"]+"|[^.\]]+)\]',configtext,re.M):options['mcp_servers.'+name+'.enabled']=False
    for name in re.findall(r'^\[plugins\.([^\]]+)\]',configtext,re.M):options['plugins.'+name+'.enabled']=False
    command=['codex','app-server','--stdio']
    for k,v in options.items():command+=['-c',k+'='+json.dumps(v)]
    dump(out/'launch.json',dict(command=command));client=Client(command,out)
    _,_,initialized=client.call('initialize',{'clientInfo':{'name':'veil_ttft_client','title':'Veil TTFT evaluation','version':'1.0'},'capabilities':{'experimentalApi':True}})
    client.send('initialized',{},notification=True);env['appserver_initialize_ms']=(initialized-client.started_ns)/1e6;dump(out/'environment.json',env)
    for old in plan:
     action_ns=time.perf_counter_ns();beforefiles={p.name for p in wire.glob('*.client.request.meta.json')};index=old['index'];case=old['case'];stem=f'{index:03}.{case}'
     saved=load(prior/'turns'/(stem+'.input.json'));prompt=saved['prompt'];expected=saved['expected']
     if old['session_id'] not in threads:
      params=dict(model=model,modelProvider='veil_ttft',cwd=str(work),approvalPolicy='never',sandbox='read-only',ephemeral=True)
      response,_,_=client.call('thread/start',params);threads[old['session_id']]=response['thread']['id']
     tid=threads[old['session_id']];params={'threadId':tid,'input':[{'type':'text','text':prompt,'text_elements':[]}]}
     if effort!='model_default':params['effort']=effort
     print('turn',index,case,flush=True)
     rpc_id,submit_ns,flushed_ns=client.send('turn/start',params);first=None;first_event=None;received=[];items=[];deltas={};completed=None;started=None;turnid=None
     deadline=time.monotonic()+180
     while time.monotonic()<deadline:
      at,v=client.receive(timeout=max(1,deadline-time.monotonic()));received.append(dict(received_ns=at,message=v))
      if v.get('id')==rpc_id:
       if 'error' in v:raise RuntimeError('turn/start failed: '+json.dumps(v['error']))
       turnid=v['result']['turn']['id']
      method=v.get('method');p=v.get('params',{})
      if p.get('threadId') not in (None,tid):continue
      if method=='turn/started':started=at
      if method=='item/agentMessage/delta' and p.get('delta'):
       if first is None:first=at;first_event=dict(received_ns=at,message=v)
       deltas[p['itemId']]=deltas.get(p['itemId'],'')+p['delta']
      if method=='item/started' and p.get('item',{}).get('type') in ('commandExecution','mcpToolCall','fileChange','webSearch'):
       raise RuntimeError('Unexpected tool execution')
      if method=='item/completed' and p.get('item',{}).get('type')=='agentMessage':items.append(p['item'])
      if method=='turn/completed':completed=(at,p['turn']);break
     if completed is None:raise RuntimeError('No complete turn event')
     if first is None or completed[1]['status']!='completed':
      result=dict(index=index,case=case,group=old['group'],thread_id=tid,turn_id=turnid,submit_ns=submit_ns,first_client_text_ns=first,client_ttft_ms=None,status=completed[1]['status'],error=completed[1].get('error'),received=received)
      dump(eventsdir/(stem+'.failed.json'),result);print(' FAILED',completed[1].get('error',{}).get('codexErrorInfo'),flush=True);continue
     reply=items[-1]['text'] if items else ''.join(deltas.values())
     try:expected_ok=(json.loads(reply)==expected) if isinstance(expected,dict) else reply.strip()==expected
     except ValueError:expected_ok=False
     assert completed[1]['status']=='completed'
     (eventsdir/(stem+'.reply.txt')).write_text(reply)
     dump(eventsdir/(stem+'.input.json'),dict(prompt=prompt,expected=expected,thread_id=tid));dump(eventsdir/(stem+'.first_text.json'),first_event)
     with (eventsdir/(stem+'.received.jsonl')).open('w') as f:
      for e in received:f.write(json.dumps(e,ensure_ascii=False)+'\n')
     reqids=[load(p)['request_id'] for p in sorted(wire.glob('*.client.request.meta.json')) if p.name not in beforefiles and load(p)['path'].endswith('/responses')]
     deadline=time.monotonic()+3
     while reqids and not all((wire/f'{r:04}.client.timing.json').exists() for r in reqids) and time.monotonic()<deadline:time.sleep(.02)
     result=dict(index=index,case=case,group=old['group'],thread_id=tid,turn_id=turnid,source_ids=old['source_ids'],request_ids=reqids,action_ns=action_ns,submit_ns=submit_ns,flushed_ns=flushed_ns,server_turn_started_ns=started,first_client_text_ns=first,turn_completed_ns=completed[0],client_ttft_ms=(first-submit_ns)/1e6,action_to_first_text_ms=(first-action_ns)/1e6,submit_flush_ms=(flushed_ns-submit_ns)/1e6,client_turn_ms=(completed[0]-submit_ns)/1e6,process_to_first_text_ms=(first-client.started_ns)/1e6 if not results else None,exact_reply=expected_ok,reply=reply,expected=expected,delta_matches_final=bool(items) and deltas.get(items[-1]['id'])==reply)
     results.append(result);dump(out/'turn_results.json',results)
     print(' TTFT ms',round(result['client_ttft_ms'],3),'exact',expected_ok,flush=True)
   finally:
    if client is not None:client.close()
    gateway.terminate()
    try:gateway.wait(timeout=10)
    except subprocess.TimeoutExpired:gateway.kill();gateway.wait()
 env['finished_utc']=datetime.datetime.now(datetime.timezone.utc).isoformat();env['base_config_unchanged']=digest(config)==original;dump(out/'environment.json',env)
 print('completed',len(results),flush=True)
if __name__=='__main__':main()
