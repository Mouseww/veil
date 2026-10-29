#!/usr/bin/env python3
"""Reproduce local Veil quality/performance evidence from archived Presidio data."""
import argparse, collections, datetime, gzip, hashlib, json, math, os, pathlib, platform, random, resource, subprocess, time
ROOT = pathlib.Path(__file__).resolve().parents[2]
MAP = {'PHONE':'phone','IDCARD':'id_number','EMAIL':'email','IP':'ip_address','APIKEY':'api_key'}
ORIGINAL = {'person','phone','id_number','address','email','bank_card','passport','license_plate'}
EXTENSION = {'public_domain','ip_address','api_key'}
ENABLED = {'phone','id_number','ip_address','api_key'}
def norm(t): return 'ip_address' if t == 'public_ip' else t
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def dump(p, obj): p.write_text(json.dumps(obj, ensure_ascii=False, indent=2)+'\n')
def metric(tp,fp,fn):
    p=tp/(tp+fp) if tp+fp else None; r=tp/(tp+fn) if tp+fn else None
    return dict(tp=tp,fp=fp,fn=fn,precision=p,recall=r,f1=2*tp/(2*tp+fp+fn) if 2*tp+fp+fn else None)
def pct(a,q):
    a=sorted(a); return a[max(0, math.ceil(q*len(a))-1)] if a else None
class Probe:
    def __init__(self): self.p=subprocess.Popen([str(ROOT/'target/release/examples/privacy_probe')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,bufsize=1)
    def call(self,**kw):
        self.p.stdin.write(json.dumps(kw,ensure_ascii=False)+'\n'); self.p.stdin.flush()
        line=self.p.stdout.readline()
        if not line: raise RuntimeError('Rust probe terminated')
        return json.loads(line)
    def close(self): self.p.stdin.close(); assert self.p.wait()==0

def evaluate(probe,name,rows,out,email=False):
    scopes={'all': (ORIGINAL if name in ('formal','chat') else EXTENSION if name=='extension' else ORIGINAL|EXTENSION), 'enabled':ENABLED|({'email'} if email else set())}
    counters={s:collections.defaultdict(collections.Counter) for s in scopes}; exact=collections.Counter(); cover=collections.Counter(); neg=collections.Counter(); errors=collections.Counter(); latency=[]
    variant='email_on' if email else 'default'; path=out/f'{name}.{variant}'
    with open(str(path)+'.records.jsonl','w') as records, open(str(path)+'.errors.jsonl','w') as bad:
        for row in rows:
            text=row['text']; result=probe.call(op='predict',text=text,email=email); latency.append(result['detect_ns'])
            gold={(norm(e['type']),e['start'],e['end']) for e in row['entities']}
            pred={(MAP.get(h['type'],h['type']),h['start'],h['end']) for h in result['hits']}
            active_hits=[] if result['masked']==text else result['hits']
            goldchars={i for _,s,e in gold for i in range(s,e)}
            covered={i for h in active_hits for i in range(h['start'],h['end'])}
            cover['gold']+=len(goldchars); cover['covered']+=len(goldchars&covered)
            cover['unannotated_masked_chars']+=len(covered-goldchars)
            for scope,types in scopes.items():
                types=types & (ORIGINAL if name in ('formal','chat') else EXTENSION if name=='extension' else ORIGINAL|EXTENSION)
                g={x for x in gold if x[0] in types}; p={x for x in pred if x[0] in types}
                exact[scope]+=g==p
                for t in types:
                    gt={x for x in g if x[0]==t}; pt={x for x in p if x[0]==t}
                    counters[scope][t].update(tp=len(gt&pt),fp=len(pt-gt),fn=len(gt-pt))
                if not gold:
                    neg['records']+=scope=='all'; neg[scope+'_false_positive_records']+=bool(p)
            types=scopes['all']; g={x for x in gold if x[0] in types}; p={x for x in pred if x[0] in types}
            missing=g-p; extra=p-g
            details=[]
            for t,s,e in sorted(missing):
                reason='unsupported_category' if t not in ENABLED|{'email'} else 'disabled_email' if t=='email' and not email else 'boundary_mismatch' if any(pt==t and ps<e and pe>s for pt,ps,pe in pred) else 'missed'
                errors[reason]+=1; details.append(dict(type=t,start=s,end=e,text=text[s:e],reason=reason))
            record=dict(id=row['id'],text=text,groundtruth=row['entities'],predictions=[dict(h,text=text[h['start']:h['end']]) for h in result['hits']],missing_details=details,extra_details=[dict(type=t,start=s,end=e,text=text[s:e]) for t,s,e in sorted(extra)],roundtrip_ok=result['roundtrip_ok'],pipeline_skipped=bool(result['hits']) and not active_hits,detect_ns=result['detect_ns'])
            records.write(json.dumps(record,ensure_ascii=False)+'\n')
            if missing or extra: bad.write(json.dumps(record,ensure_ascii=False)+'\n')
    summary=dict(records=len(rows),entities=sum(len(r['entities']) for r in rows),coverage=dict(cover),sensitive_character_recall=cover['covered']/cover['gold'] if cover['gold'] else None,negative_records=dict(neg),missing_reasons=dict(errors),scopes={})
    for scope in scopes:
        per={t:metric(c['tp'],c['fp'],c['fn']) for t,c in sorted(counters[scope].items())}
        summary['scopes'][scope]=dict(micro=metric(*[sum(c[k] for c in counters[scope].values()) for k in ('tp','fp','fn')]),per_type=per,exact_records=exact[scope],record_accuracy=exact[scope]/len(rows))
    dump(pathlib.Path(str(path)+'.summary.json'),summary)
    return summary

def bench(probe, out, label, texts, **kw):
    result=probe.call(op='bench',texts=texts,**kw); samples=result.pop('samples_ns'); n=len(samples)
    result.update(label=label,workload_sha256=[hashlib.sha256(t.encode()).hexdigest() for t in texts],**kw,requests=n,rps=n/result['wall_seconds'],chars_per_second=result['characters']/result['wall_seconds'],mean_ms=sum(samples)/n/1e6,p50_ms=pct(samples,.5)/1e6,p95_ms=pct(samples,.95)/1e6,p99_ms=pct(samples,.99)/1e6,max_ms=max(samples)/1e6)
    with gzip.open(out/(label+'.samples.json.gz'),'wt') as f: json.dump(samples,f)
    return result

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('--presidio',type=pathlib.Path,default=pathlib.Path('/Users/caic/code/presidio')); ap.add_argument('--output',type=pathlib.Path,required=True); ap.add_argument('--duration-ms',type=int,default=1000); args=ap.parse_args()
    out=args.output.resolve(); out.mkdir(parents=True,exist_ok=False); quality=out/'quality'; quality.mkdir(); perf=out/'performance'; perf.mkdir()
    base=args.presidio/'.vscode/test/zh-baseline-extension'
    paths={'formal':base/'data/pii-bench-zh/data/pii_bench_zh.jsonl','chat':base/'data/pii-bench-zh/data/pii_bench_zh_chat.jsonl','extension':base/'data/pii_bench_zh_extension/all.jsonl'}
    manifest=json.loads((base/'performance/data/constructed/manifest.json').read_text())
    for c in manifest['cohorts']: paths[c['name']]=base/'performance/data/constructed'/c['file']
    datasets={}; meta={}
    for name,path in paths.items():
        rows=[json.loads(line) for line in path.read_text().splitlines() if line.strip()]
        assert len({r['id'] for r in rows})==len(rows)
        for r in rows:
            for e in r['entities']: assert r['text'][e['start']:e['end']]==e['text'],(name,r['id'])
        datasets[name]=rows; meta[name]=dict(path=str(path),sha256=sha(path),records=len(rows),entities=sum(len(r['entities']) for r in rows))
    source_files=list((ROOT/'crates').glob('*/src/**/*.rs'))+[ROOT/'Cargo.lock',ROOT/'Cargo.toml',ROOT/'crates/veil-store/examples/privacy_probe.rs',pathlib.Path(__file__)]
    env=dict(started_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),platform=platform.platform(),cpu_count=os.cpu_count(),python=platform.python_version(),git_head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),git_status=subprocess.check_output(['git','status','--short'],cwd=ROOT,text=True),rustc=subprocess.check_output(['rustc','--version'],text=True).strip(),sdkroot='/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk',datasets=meta,source_sha256={str(p.relative_to(ROOT)):sha(p) for p in source_files},binary_sha256=sha(ROOT/'target/release/examples/privacy_probe'))
    dump(out/'environment.json',env); probe=Probe(); dump(out/'rules.json',probe.call(op='rules'))
    summaries={}
    for name,rows in datasets.items():
        print('quality',name,len(rows),flush=True); summaries[name]=evaluate(probe,name,rows,quality)
    for name in ('formal','chat','extension'):
        print('quality email_on',name,flush=True); summaries[name+'.email_on']=evaluate(probe,name,datasets[name],quality,email=True)
    dump(out/'quality_summary.json',summaries)
    results=[]
    def save(r): results.append(r); dump(out/'performance_summary.json',results)
    for name,rows in datasets.items():
        texts=[r['text'] for r in rows]; rng=random.Random(4826); rng.shuffle(texts)
        # Same fixed six records for detection and pipeline comparisons; quality remains full corpus.
        texts=texts[:6]
        for repeat in range(3):
            for mode in ('detect','memory','sqlite'):
                label=f'{name}.{mode}.r{repeat+1}'; print('bench',label,flush=True)
                save(bench(probe,perf,label,texts,mode=mode,threads=1,duration_ms=args.duration_ms,cohort=name,repeat=repeat+1))
    for name in ('formal','high_30000'):
        texts=[r['text'] for r in datasets[name]][:6]
        for r in probe.call(op='rules'):
            if not r['enabled']: continue
            for kind in ('only','omit'):
                label=f'profile.{name}.{kind}.{r["id"]}'; print(label,flush=True)
                save(bench(probe,perf,label,texts,mode='detect',threads=1,duration_ms=args.duration_ms,cohort=name,**{kind:r['id']}))
        save(bench(probe,perf,'profile.'+name+'.full',texts,mode='detect',threads=1,duration_ms=args.duration_ms,cohort=name))
    # 90/9/1 fixed synthetic mixed pool, matching the reference plan's mix.
    rng=random.Random(4826); short=sum([datasets[n] for n in ('formal','chat','extension')],[])
    mixed=[rng.choice(short)['text'] for _ in range(90)]+[rng.choice(datasets['high_5000'])['text'] for _ in range(9)]+[rng.choice(datasets['high_30000'])['text']]; rng.shuffle(mixed)
    for mode in ('detect','sqlite'):
        for threads in (1,2,4,8):
            label=f'concurrency.{mode}.t{threads}'; print(label,flush=True)
            save(bench(probe,perf,label,mixed,mode=mode,threads=threads,duration_ms=max(3000,args.duration_ms),cohort='mixed_90_9_1'))
    # Controlled density scaling, same rule, no annotation or context confounder.
    for n in (16,32,64,128,256,512,1024):
        label=f'scaling.phone.{n}'; print(label,flush=True)
        save(bench(probe,perf,label,['13800138000\n'*n],mode='detect',threads=1,duration_ms=args.duration_ms,cohort='synthetic_phone',hits=n))
    # Added probes target built-in credentials and JSON binary heuristics absent from the corpus.
    probes=[('api_field','api_key=sk-abcdefghijklmnopqrstuvwxyz1234'),('slack','xoxb-1234567890-abcdefghij'),('gitlab','glpat-abcdefghijklmnopqrstuvwxyz'),('npm','npm_abcdefghijklmnopqrstuvwxyz123456'),('jwt','eyJ'+'a'*30+'.'+'b'*20+'.'+'c'*30),('long_ascii_secret','sk-'+ 'a'*210),('id_invalid','000000000000000000'),('phone_order','订单号：13800138000'),('id_card_conflict','银行卡号：622202123456789012')]
    dump(out/'supplemental_probes.json',[dict(id=i,text=t,**probe.call(op='predict',text=t)) for i,t in probes])
    probe.close(); env['finished_utc']=datetime.datetime.now(datetime.timezone.utc).isoformat(); env['child_maxrss_platform_units']=resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss; dump(out/'environment.json',env)
    print('done',out,flush=True)
if __name__=='__main__': main()
