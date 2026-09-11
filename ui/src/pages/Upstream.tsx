import { useEffect, useState } from "react";
import { api } from "../api";
import { useT } from "../i18n";

export default function UpstreamPage() {
  const { t } = useT();
  const [a, setA] = useState("");
  const [c, setC] = useState("");
  const [r, setR] = useState("");
  const [ttl, setTtl] = useState(90);
  const [lim, setLim] = useState(32);
  const [bind, setBind] = useState("127.0.0.1");
  const [proxy, setProxy] = useState(18787);
  const [flash, setFlash] = useState("");
  useEffect(() => {
    api.upstream().then((u) => { setA(u.anthropic_upstream); setC(u.openai_completions_upstream); setR(u.openai_responses_upstream); }).catch(() => undefined);
    api.status().then((s) => { setTtl(s.mapping_ttl_days); setLim(s.request_body_limit_mib); setBind(s.bind); setProxy(s.proxy_port); }).catch(() => undefined);
  }, []);
  const base = "http://" + bind + ":" + proxy;
  return (
    <section>
      <h1>{t.upTitle}</h1>
      <p className="help">{t.helpUp}</p>
      {flash && <p className="flash">{flash}</p>}
      <label>Anthropic<input value={a} onChange={(e) => setA(e.target.value)} /></label>
      <label>OpenAI chat<input value={c} onChange={(e) => setC(e.target.value)} /></label>
      <label>OpenAI responses<input value={r} onChange={(e) => setR(e.target.value)} /></label>
      <button type="button" onClick={async () => { try { await api.putUpstream({ anthropic_upstream: a, openai_completions_upstream: c, openai_responses_upstream: r }); setFlash(t.saved); } catch { setFlash(t.failed); } }}>{t.saveUp}</button>
      <div className="row">
        <label>{t.ttl}<input type="number" value={ttl} onChange={(e) => setTtl(Number(e.target.value))} /></label>
        <label>{t.bodyMib}<input type="number" value={lim} onChange={(e) => setLim(Number(e.target.value))} /></label>
      </div>
      <button type="button" onClick={async () => { try { await api.putSettings({ mapping_ttl_days: ttl, request_body_limit_mib: lim }); setFlash(t.saved); } catch { setFlash(t.failed); } }}>{t.saveLim}</button>
      <p>{t.clientBase} <code>{base}</code></p>
    </section>
  );
}
