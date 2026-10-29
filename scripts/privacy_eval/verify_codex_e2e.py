#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Verify E2E evidence integrity without another model call."""
import collections, hashlib, json, pathlib, re, sys
from run_codex_e2e import load, dump, strings
ROOT=pathlib.Path(__file__).resolve().parents[2]
def main():
 out=pathlib.Path(sys.argv[1]);env=load(out/'environment.json');turns=load(out/'turn_results.json');requests=load(out/'request_results.json');summary=load(out/'summary.json');wire=out/'wire'
 for name,want in env['sources'].items():assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==want,name
 assert hashlib.sha256((ROOT/'target/release/examples/codex_e2e_gateway').read_bytes()).hexdigest()==env['gateway_binary_sha256']
 assert len(turns)==31 and len(requests)==31
 for t in turns:
  assert t['exit_code']==0 and t['turn_completed'] and t['unexpected_tool_calls']==0
  stem=f"{t['index']:03}.{t['case']}";actual=(out/'turns'/(stem+'.reply.txt')).read_text().strip();expected=load(out/'turns'/(stem+'.input.json'))['expected']
  assert (json.loads(actual) if isinstance(expected,dict) else actual)==expected
 for r in requests:
  assert 200<=r['status']<300 and r['response_model']==env['model'] and r['response_id']
  assert r['client_stream_complete'] and r['upstream_stream_complete'] and r['restore_delta_exact'] and r['cli_matches_client_delta']
  assert not r['remaining_client_placeholders'] and not r['unknown_upstream_placeholders'] and not r['unexplained_request_changes']
  assert 0<=r['gateway_pre_upstream_ms']<=r['upstream_first_text_ms']<=r['client_first_text_ms']<=r['gateway_response_completed_ms']<=r['cli_wall_ms']
 for group in ('continuous','formatted_chat','credentials_ip'):
  ts=[t for t in turns if t['group']==group];assert len(ts)==4 and len({t['session_id'] for t in ts})==1
  sid=ts[0]['session_id'];assert all(t['resumed_session']==sid for t in ts[1:])
  rs=[r for r in requests if r['group']==group];n=rs[0]['input_message_roles'].count('user');assert [r['input_message_roles'].count('user') for r in rs]==[n,n+1,n+2,n+3];assert [r['input_message_roles'].count('assistant') for r in rs]==[0,1,2,3]
  replay=load(wire/f"{rs[1]['request_id']:04}.client.request.json")['input'];lastuser=next(x for x in reversed(replay) if x.get('role')=='user')
  source=load(out/'selected_data.json');doc=next(r for r in source if r['id']==ts[0]['source_ids'][0])['text']
  assert doc not in '\n'.join(v for _,v in strings(lastuser))
  assert doc in '\n'.join(v for _,v in strings(replay))
 for n in (1000,5000,30000):
  t=next(t for t in turns if t['case']==f'context_{n}_load');row=next(r for r in load(out/'selected_data.json') if r['id']==t['source_ids'][0]);assert len(row['text'])==n
  rid=t['request_ids'][0];req=load(wire/f'{rid:04}.client.request.json');assert any(row['text'] in v for _,v in strings(req))
 for f in wire.glob('*.meta.json'):
  data=load(f)
  if 'headers' in data:assert set(data['headers'])<={'content_type','content_encoding'}
 obs=load(out/'entity_observations.json');unique=load(out/'unique_entities.json');misses=[r for r in obs if not r['fully_masked']]
 with (out/'privacy_badcases.jsonl').open('w') as f:
  for r in misses:f.write(json.dumps(r,ensure_ascii=False)+'\n')
 assert sum(r['fully_masked'] for r in obs)==sum(c['fully_masked'] for c in summary['privacy_by_type'].values())
 assert env['base_config_unchanged'] and not summary['unstable_plaintexts']
 result=dict(status='passed',real_model_turns_checked=len(turns),real_model_responses_checked=len(requests),four_turn_sessions_checked=3,length_conditions_checked=[1000,5000,30000],source_and_binary_hashes='matched',header_metadata_excludes_credentials=True,annotated_occurrences=len(obs),fully_masked_occurrences=sum(r['fully_masked'] for r in obs),not_fully_masked_occurrences=len(misses),source_annotation_ranges=len(unique),note='Passed verifies evidence and observed behaviors; privacy coverage is reported separately and is not 100%.')
 dump(out/'verification.json',result);print(json.dumps(result,ensure_ascii=False,indent=2))
if __name__=='__main__':main()
