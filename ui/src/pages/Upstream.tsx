import { useEffect, useState } from "react";
import { api, type ClientRoute } from "../api";
import { useT } from "../i18n";

export default function UpstreamPage() {
  const { t } = useT();
  const [a, setA] = useState("");
  const [c, setC] = useState("");
  const [r, setR] = useState("");
  const [routes, setRoutes] = useState<ClientRoute[]>([]);
  const [ttl, setTtl] = useState(90);
  const [lim, setLim] = useState(32);
  const [flash, setFlash] = useState("");
  useEffect(() => {
    api.upstream().then((u) => {
      setA(u.anthropic_upstream);
      setC(u.openai_completions_upstream);
      setR(u.openai_responses_upstream);
      setRoutes(u.routes ?? []);
    }).catch(() => undefined);
    api.status().then((s) => { setTtl(s.mapping_ttl_days); setLim(s.request_body_limit_mib); }).catch(() => undefined);
  }, []);
  return (
    <section>
      <h1>{t.upTitle}</h1>
      <p className="help">{t.helpUp}</p>
      {flash && <p className="flash">{flash}</p>}
      {routes.length > 0 && (
        <>
          <h2>{t.perApp}</h2>
          {routes.map((row, i) => (
            <div className="card" key={row.id} style={{ marginBottom: 12 }}>
              <div className="label">{row.label || row.id} · :{row.port} · {row.kind}</div>
              <input value={row.upstream} onChange={(e) => {
                const next = routes.slice();
                next[i] = { ...row, upstream: e.target.value };
                setRoutes(next);
              }} />
              <p className="muted">Base URL in the app: <code>http://127.0.0.1:{row.port}{row.kind === "anthropic" ? "" : "/v1"}</code></p>
            </div>
          ))}
        </>
      )}
      <h2>{t.fallbackUp}</h2>
      <label>Anthropic<input value={a} onChange={(e) => setA(e.target.value)} /></label>
      <label>OpenAI chat<input value={c} onChange={(e) => setC(e.target.value)} /></label>
      <label>OpenAI responses<input value={r} onChange={(e) => setR(e.target.value)} /></label>
      <button type="button" onClick={async () => {
        try {
          await api.putUpstream({ anthropic_upstream: a, openai_completions_upstream: c, openai_responses_upstream: r, routes });
          setFlash(t.saved);
        } catch { setFlash(t.failed); }
      }}>{t.saveUp}</button>
      <div className="row">
        <label>{t.ttl}<input type="number" value={ttl} onChange={(e) => setTtl(Number(e.target.value))} /></label>
        <label>{t.bodyMib}<input type="number" value={lim} onChange={(e) => setLim(Number(e.target.value))} /></label>
      </div>
      <button type="button" onClick={async () => { try { await api.putSettings({ mapping_ttl_days: ttl, request_body_limit_mib: lim }); setFlash(t.saved); } catch { setFlash(t.failed); } }}>{t.saveLim}</button>
    </section>
  );
}
