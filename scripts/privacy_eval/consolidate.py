#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Build one standalone HTML report containing all results as an embedded ZIP."""
import argparse
import base64
import hashlib
import html
import io
import json
from pathlib import Path
import re
import zipfile


def inline(text):
    text = html.escape(text)
    text = re.sub(r'`([^`]+)`', r'<code>\1</code>', text)
    def link(m):
        label, target = m.groups()
        anchors = {'LATENCY_ANALYSIS.md':'ttft-analysis', 'codex-ttft/LATENCY_ANALYSIS.md':'ttft-analysis', 'codex-ttft/REPORT.md':'ttft', 'codex-e2e/REPORT.md':'e2e', 'REPORT.md':'report', 'ANALYSIS.md':'analysis', 'badcase_catalog.json':'appendix-badcase_catalog', 'supplemental_probes.json':'appendix-supplemental_probes', 'environment.json':'appendix-environment', 'verification.json':'appendix-verification'}
        if target in anchors:
            return '<a href="#' + anchors[target] + '">' + label + '</a>'
        if target.startswith('http'):
            return '<a href="' + target + '">' + label + '</a>'
        if 'crates/' in target or 'scripts/' in target:
            return label + ' <small>（源码：' + target.replace('../../../', '') + '）</small>'
        return label + ' <small>（见内嵌原始附件）</small>'
    text = re.sub(r'\[([^\]]+)\]\(([^)]+)\)', link, text)
    text = re.sub(r'\*\*(.+?)\*\*', r'<strong>\1</strong>', text)
    return text


def render(md):
    out, lines, i = [], md.splitlines(), 0
    while i < len(lines):
        line = lines[i]
        if not line.strip():
            i += 1
            continue
        if line.startswith('```'):
            code = []
            i += 1
            while i < len(lines) and not lines[i].startswith('```'):
                code.append(lines[i]); i += 1
            out.append('<pre><code>' + html.escape('\n'.join(code)) + '</code></pre>')
        elif line.startswith('|'):
            rows = []
            while i < len(lines) and lines[i].startswith('|'):
                cells = lines[i].strip().strip('|').split('|')
                if not all(re.fullmatch(r'\s*:?-+:?\s*', cell) for cell in cells):
                    rows.append(cells)
                i += 1
            out.append('<div class="table"><table>')
            for n, row in enumerate(rows):
                tag = 'th' if n == 0 else 'td'
                out.append('<tr>' + ''.join('<'+tag+'>'+inline(cell.strip())+'</'+tag+'>' for cell in row)+'</tr>')
            out.append('</table></div>')
            continue
        elif re.match(r'^#{1,6} ', line):
            level = min(len(line.split(' ')[0])+1, 6)
            out.append(f'<h{level}>' + inline(line.lstrip('# ')) + f'</h{level}>')
        elif re.match(r'^(?:- |\d+\. )', line):
            items = []
            ordered = bool(re.match(r'^\d+\. ', line))
            pattern = r'^\d+\. ' if ordered else r'^- '
            while i < len(lines) and re.match(pattern, lines[i]):
                items.append('<li>'+inline(re.sub(pattern, '', lines[i]))+'</li>'); i += 1
            tag = 'ol' if ordered else 'ul'
            out.append('<'+tag+'>'+''.join(items)+'</'+tag+'>')
            continue
        else:
            out.append('<p>' + inline(line) + '</p>')
        i += 1
    return '\n'.join(out)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('results', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    source = args.results.resolve()
    target = args.output.resolve()
    if source == target or source in target.parents:
        raise ValueError('Output must be outside the result directory to avoid recursive archives')
    files = sorted(p for p in source.rglob('*') if p.is_file())
    buf = io.BytesIO()
    manifest = []
    with zipfile.ZipFile(buf, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=6) as z:
        for path in files:
            data = path.read_bytes()
            name = path.relative_to(source).as_posix()
            z.writestr(name, data)
            manifest.append(dict(file=name, bytes=len(data), sha256=hashlib.sha256(data).hexdigest()))
    archive = buf.getvalue()
    # Validate every archived byte before embedding it.
    with zipfile.ZipFile(io.BytesIO(archive)) as z:
        assert z.testzip() is None
        for record in manifest:
            assert hashlib.sha256(z.read(record['file'])).hexdigest() == record['sha256']
    report = render((source/'REPORT.md').read_text())
    analysis = render((source/'ANALYSIS.md').read_text())
    e2e_path = source/'codex-e2e/REPORT.md'
    e2e = render(e2e_path.read_text()) if e2e_path.exists() else '<p>未执行真实模型测试。</p>'
    ttft_path = source/'codex-ttft/REPORT.md'
    ttft = render(ttft_path.read_text()) if ttft_path.exists() else '<p>未执行客户端 TTFT 补测。</p>'
    ttft_analysis_path = source/'codex-ttft/LATENCY_ANALYSIS.md'
    ttft_analysis = render(ttft_analysis_path.read_text()) if ttft_analysis_path.exists() else ''
    appendix = []
    for path in sorted(source.glob('*.json')) + sorted((source/'codex-e2e').glob('summary.json')) + sorted((source/'codex-ttft').glob('summary.json')):
        value = json.loads(path.read_text())
        appendix_id = path.stem if path.parent == source else str(path.relative_to(source).with_suffix('')).replace('/','-')
        appendix.append('<details id="appendix-'+appendix_id+'"><summary>'+html.escape(str(path.relative_to(source)))+'</summary><pre>'+html.escape(json.dumps(value,ensure_ascii=False,indent=2))+'</pre></details>')
    page = '''<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Veil 隐私识别测试完整结果 · 2026-10-28</title>
<style>
:root{font-family:system-ui,-apple-system,"PingFang SC",sans-serif;color:#1e293b;background:#f1f5f9;line-height:1.75}body{margin:0}main{max-width:1160px;margin:auto;background:white;padding:40px 48px}h1,h2,h3,h4{line-height:1.4;color:#0f172a}h2{margin-top:48px;border-top:1px solid #dbe3ec;padding-top:24px}a{color:#0369a1}nav{display:flex;gap:18px;flex-wrap:wrap}button{background:#075985;border:0;color:white;border-radius:7px;padding:12px 20px;font:inherit;cursor:pointer}.intro{padding:20px;background:#f0f9ff;border-left:4px solid #0284c7;margin:24px 0}.table{overflow-x:auto;margin:20px 0}table{border-collapse:collapse;width:100%;font-size:14px}th,td{padding:9px 12px;border:1px solid #dbe3ec;text-align:left;vertical-align:top}th{background:#eaf1f8}tr:nth-child(even){background:#f8fafc}code{background:#f1f5f9;padding:2px 4px;border-radius:3px;overflow-wrap:anywhere}pre{overflow:auto;background:#f8fafc;padding:16px;font-size:12px;line-height:1.6;white-space:pre-wrap;overflow-wrap:anywhere;max-height:640px}small{color:#64748b}details{margin:12px 0;border:1px solid #dbe3ec;padding:12px}summary{cursor:pointer;font-weight:600}section{scroll-margin-top:20px}@media(max-width:700px){main{padding:20px 16px}th,td{padding:7px}}@media print{main{padding:0}.download,nav{display:none}pre{max-height:none}details{break-inside:avoid}}
</style><main>
<h1>Veil 隐私识别测试完整结果</h1><p>测试日期：2026-10-28</p>
<nav><a href="#report">完整测试报告</a><a href="#analysis">Badcase 与瓶颈分析</a><a href="#e2e">Codex 真实模型测试</a><a href="#ttft">完整客户端 TTFT</a><a href="#ttft-analysis">延迟原因分析</a><a href="#appendix">结构化结果</a><a href="#archive">全部原始数据</a></nav>
<div class="intro">本文件包含完整报告、问题定位、全部结构化汇总，以及内嵌的全量原始结果。可离线打开；无需同目录文件或网络。默认配置覆盖 9,224 条文本，邮箱消融覆盖 9,008 条，性能共 138 个条件。另含 Codex 真实模型端到端补充测试，覆盖多轮上下文与三档文档长度，并补充完整客户端 TTFT 实测及失败记录。全部逐条记录、真实请求/响应和计时样本保存在本文件内嵌的 ZIP 附件中。</div>
<section id="report">REPORT</section><section id="analysis">ANALYSIS</section>
<section id="e2e">E2E_CONTENT</section><section id="ttft">TTFT_CONTENT</section><section id="ttft-analysis">TTFT_ANALYSIS_CONTENT</section><section id="appendix"><h2>结构化结果与典型样例</h2><p>点击文件名展开查看。逐条全量预测、错误与性能计时样本可通过下方按钮导出。</p>APPENDIX</section>
<section id="archive"><h2>全部原始结果附件</h2><p>附件包含 COUNT 个原始结果文件。导出后可按原目录结构复核；这是报告内嵌数据，不需要联网下载。</p><button class="download" id="download">导出全部原始结果（ZIP）</button><p><small>ZIP SHA-256：DIGEST</small></p><details><summary>文件清单与校验和</summary><pre>MANIFEST</pre></details></section>
</main><script id="embedded-archive" type="application/octet-stream">ARCHIVE</script>
<script>document.getElementById('download').addEventListener('click',function(){const raw=atob(document.getElementById('embedded-archive').textContent.trim());const data=new Uint8Array(raw.length);for(let i=0;i<raw.length;i++)data[i]=raw.charCodeAt(i);const url=URL.createObjectURL(new Blob([data],{type:'application/zip'}));const a=document.createElement('a');a.href=url;a.download='veil-privacy-eval-2026-10-28-all-results.zip';a.click();setTimeout(()=>URL.revokeObjectURL(url),60000);});</script></html>'''
    # Apply placeholders once to prevent replacement within report content.
    values = dict(TTFT_ANALYSIS_CONTENT=ttft_analysis, TTFT_CONTENT=ttft, E2E_CONTENT=e2e, REPORT=report, ANALYSIS=analysis, APPENDIX='\n'.join(appendix), COUNT=str(len(files)), DIGEST=hashlib.sha256(archive).hexdigest(), MANIFEST=html.escape(json.dumps(manifest,ensure_ascii=False,indent=2)), ARCHIVE=base64.b64encode(archive).decode())
    page = re.sub(r'TTFT_ANALYSIS_CONTENT|TTFT_CONTENT|E2E_CONTENT|REPORT|ANALYSIS|APPENDIX|COUNT|DIGEST|MANIFEST|ARCHIVE', lambda m:values[m.group()], page)
    target.write_text(page,encoding='utf-8')
    # Verify the attachment actually written to the final HTML, not just the input ZIP.
    embedded = re.search(r'id="embedded-archive"[^>]*>([^<]+)</script>',target.read_text()).group(1)
    assert base64.b64decode(embedded) == archive
    print(json.dumps(dict(file=str(target),size_bytes=target.stat().st_size,embedded_files=len(files),archive_sha256=hashlib.sha256(archive).hexdigest()),ensure_ascii=False))

if __name__ == '__main__':
    main()
