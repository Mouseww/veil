#!/usr/bin/env python3
"""Check scoring semantics and recompute saved percentiles/counts independently."""
import gzip, json, pathlib, sys, tempfile
from run import Probe, evaluate

def main():
    probe=Probe()
    try:
        rows=[]
        for i,(text,label,value) in enumerate([('中🙂电话13800138000。','phone','13800138000'),('电话138 0013 8000','phone','138 0013 8000'),('api_key=sk-abcdefghijklmnopqrstuvwxyz','api_key','sk-abcdefghijklmnopqrstuvwxyz'),('api_key=<YOUR_KEY_HERE>',None,None)]):
            entities=[] if label is None else [dict(type=label,text=value,start=text.index(value),end=text.index(value)+len(value))]
            rows.append(dict(id=str(i),text=text,entities=entities))
        with tempfile.TemporaryDirectory() as d:
            s=evaluate(probe,'constructed',rows,pathlib.Path(d))
            assert s['scopes']['all']['micro']==dict(tp=1,fp=2,fn=2,precision=1/3,recall=1/3,f1=1/3)
            assert s['negative_records']['all_false_positive_records']==1
            first=json.loads((pathlib.Path(d)/'constructed.default.records.jsonl').read_text().splitlines()[0])
            assert first['predictions'][0]['start']==4 and first['predictions'][0]['end']==15
            assert first['roundtrip_ok']
    finally: probe.close()
    out=pathlib.Path(sys.argv[1]); perf=json.loads((out/'performance_summary.json').read_text())
    samples_count=0
    for r in perf:
        with gzip.open(out/'performance'/(r['label']+'.samples.json.gz'),'rt') as f: samples=json.load(f)
        assert len(samples)==r['requests'] and min(samples)>=0
        samples.sort(); samples_count+=len(samples)
        assert abs(sum(samples)/len(samples)/1e6-r['mean_ms'])<1e-10
        for q,key in ((.5,'p50_ms'),(.95,'p95_ms'),(.99,'p99_ms')):
            import math
            assert samples[math.ceil(q*len(samples))-1]/1e6==r[key]
    records=0; roundtrips=0
    for f in (out/'quality').glob('*.records.jsonl'):
        with f.open() as stream:
            for line in stream:
                r=json.loads(line); records+=1; roundtrips+=r['roundtrip_ok']
                for h in r['predictions']:
                    assert r['text'][h['start']:h['end']]==h['text']
                    assert r['text'].encode()[h['byte_start']:h['byte_end']].decode()==h['text']
    assert roundtrips==records
    result=dict(scoring_semantics='passed',quality_records_checked=records,roundtrip_successes=roundtrips,performance_conditions_checked=len(perf),latency_samples_checked=samples_count)
    (out/'verification.json').write_text(json.dumps(result,indent=2)+'\n'); print(json.dumps(result))
if __name__=='__main__': main()
