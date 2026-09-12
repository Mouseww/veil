import { useEffect, useState } from "react";
import { api, type ClientInfo, type ClientRoute } from "../api";
import { useT } from "../i18n";

export default function UpstreamPage() {
  const { t } = useT();
  const [a, setA] = useState("");
  const [c, setC] = useState("");
  const [r, setR] = useState("");
  const [routes, setRoutes] = useState<ClientRoute[]>([]);
  const [apps, setApps] = useState<ClientInfo[]>([]);
  const [ttl, setTtl] = useState(90);
  const [lim, setLim] = useState(32);
  const [flash, setFlash] = useState("");
  const [busy, setBusy] = useState("");
  const [tab, setTab] = useState("general");
  const load = () => {
    api.upstream().then((u) => {
      setA(u.anthropic_upstream);
      setC(u.openai_completions_upstream);
      setR(u.openai_responses_upstream);
      setRoutes(u.routes ?? []);
    }).catch(() => undefined);
    api.clients().then(setApps).catch(() => undefined);
    api.status().then((s) => { setTtl(s.mapping_ttl_days); setLim(s.request_body_limit_mib); }).catch(() => undefined);
  };
  useEffect(load, []);
  const save = async () => {
    try {
      await api.putUpstream({ anthropic_upstream: a, openai_completions_upstream: c, openai_responses_upstream: r, routes });
      setFlash(t.saved);
    } catch { setFlash(t.failed); }
  };
  const install = async (id: string) => {
    setBusy(id);
    setFlash("");
    try {
      await api.setupClient(id);
      setFlash(t.installedHint);
      load();
      setTab(id);
    } catch { setFlash(t.failed); }
    setBusy("");
  };
  const active = apps.find((x) => x.id === tab);
  const route = routes.find((x) => x.id === tab);
  return (
    <section>
      <h1>{t.upTitle}</h1>
      <p className="help">{t.helpUp}</p>
      {flash && <p className="flash">{flash}</p>}
      <div className="ptabs">
        <button type="button" className={tab === "general" ? "on" : ""} onClick={() => setTab("general")}>{t.tabGeneral}</button>
        {apps.map((row) => (
          <button key={row.id} type="button" className={tab === row.id ? "on" : ""} onClick={() => setTab(row.id)}>
            {row.label}{row.detected ? "" : ""}
          </button>
        ))}
      </div>
      {tab === "general" && (
        <>
          <p className="help">{t.helpGeneralUp}</p>
          <label>Anthropic<input value={a} onChange={(e) => setA(e.target.value)} /></label>
          <label>OpenAI chat<input value={c} onChange={(e) => setC(e.target.value)} /></label>
          <label>OpenAI responses<input value={r} onChange={(e) => setR(e.target.value)} /></label>
          <button type="button" onClick={() => void save()}>{t.saveUp}</button>
          <div className="row">
            <label>{t.ttl}<input type="number" value={ttl} onChange={(e) => setTtl(Number(e.target.value))} /></label>
            <label>{t.bodyMib}<input type="number" value={lim} onChange={(e) => setLim(Number(e.target.value))} /></label>
          </div>
          <button type="button" onClick={async () => {
            try {
              await api.putSettings({ mapping_ttl_days: ttl, request_body_limit_mib: lim });
              setFlash(t.saved);
            } catch { setFlash(t.failed); }
          }}>{t.saveLim}</button>
        </>
      )}
      {active && (
        <>
          <p className="help">
            {active.detected ? t.appFound : t.appMissing}
            {active.wired ? " · " + t.appWired : " · " + t.appNotWired}
          </p>
          {active.app_base_url && (
            <p className="muted">{t.appCurrentUrl}: <code>{active.app_base_url}</code></p>
          )}
          {active.wired && route ? (
            <>
              <p className="help">{t.helpAppUp(active.label, String(route.port))}</p>
              <label>{t.tabUpstream}
                <input value={route.upstream} onChange={(e) => {
                  setRoutes(routes.map((x) => x.id === route.id ? { ...x, upstream: e.target.value } : x));
                }} />
              </label>
              <p className="muted">{t.clientBase}: <code>{active.veil_url}</code></p>
              <button type="button" onClick={() => void save()}>{t.saveUp}</button>
            </>
          ) : (
            <p className="help">{t.installLead}</p>
          )}
          <button type="button" disabled={busy === active.id} onClick={() => void install(active.id)}>
            {busy === active.id ? t.loading : (active.wired ? t.reinstallApp : t.installApp)}
          </button>
        </>
      )}
    </section>
  );
}
