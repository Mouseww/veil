import { useEffect, useState } from "react";
import { api, type ClientRoute, type StatusBody, type TrafficEvent } from "../api";
import { useT } from "../i18n";
import { TrafficTable } from "./Traffic";

export default function DashboardPage() {
  const { t } = useT();
  const [s, setS] = useState<StatusBody | null>(null);
  const [routes, setRoutes] = useState<ClientRoute[]>([]);
  const [events, setEvents] = useState<TrafficEvent[]>([]);
  const [err, setErr] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [upd, setUpd] = useState<{ current: string; latest: string; newer: boolean } | null>(null);
  const [updMsg, setUpdMsg] = useState("");
  useEffect(() => {
    api.status().then(setS).catch((e) => setErr(String(e)));
    api.upstream().then((u) => setRoutes(u.routes ?? [])).catch(() => undefined);
    api.traffic().then((e) => setEvents(e.slice(-8))).catch(() => undefined);
  }, []);
  if (err) return <p className="err">{t.statusErr}{err}</p>;
  if (!s) return <p className="muted">{t.loading}</p>;
  const base = "http://" + s.bind + ":" + s.proxy_port;
  return (
    <section>
      <h1>{t.dashTitle}</h1>
      <p className="help">{t.helpDash}</p>
      <div className="cards">
        <div className="card"><div className="label">{t.version}</div><div className="value">v{s.version ?? "?"}</div></div>
        <div className="card"><div className="label">{t.ruleCount}</div><div className="value">{s.rule_count}</div></div>
        <div className="card"><div className="label">{t.masterKey}</div><div className="value">{s.master_key_set ? t.keyReady : t.keyMissing}</div></div>
        <div className="card"><div className="label">{t.lastError}</div><div className="value">{s.last_error_class ?? t.none}</div></div>
      </div>
      <h2>{t.connectedApps}</h2>
      {routes.length === 0 ? (
        <p className="muted">{t.noApps}</p>
      ) : (
        <table>
          <thead><tr><th>{t.colId}</th><th>port</th><th>{t.tabUpstream}</th></tr></thead>
          <tbody>
            {routes.map((r) => (
              <tr key={r.id}>
                <td>{r.label || r.id}</td>
                <td><code>127.0.0.1:{r.port}</code></td>
                <td className="mono">{r.upstream}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <h2>{t.recentTraffic}</h2>
      <TrafficTable events={events} />
      <div style={{ marginTop: 16 }}>
        <button type="button" onClick={async () => {
          try {
            const u = await api.checkUpdate();
            setUpd(u);
            setUpdMsg(u.newer ? t.updateAvail : t.upToDate);
          } catch (e) { setUpdMsg(String(e)); }
        }}>{t.checkUpdate}</button>
        {upd?.newer && (
          <button type="button" onClick={async () => {
            setUpdMsg(t.updating);
            try { await api.applyUpdate(); } catch (e) { setUpdMsg(String(e)); }
          }}>{t.updateNow}</button>
        )}
        {updMsg && <p className="flash">{updMsg}</p>}
      </div>
      <h2>{t.startTitle}</h2>
      <ol className="steps">
        <li>
          <strong>{t.step1}</strong>
          <div className="muted">{t.step1sub(base, s.bind, s.management_port)}</div>
        </li>
        <li>
          <strong>{t.step2}</strong>
          <p>{t.step2p} <code>veil setup</code>{t.step2or}</p>
          <pre className="copybox">ANTHROPIC_BASE_URL={base}</pre>
          <button type="button" onClick={async () => {
            await navigator.clipboard.writeText("ANTHROPIC_BASE_URL=" + base);
            setCopied(true);
          }}>{copied ? t.copied : t.copy}</button>
          <p className="muted">{t.step2hint}</p>
        </li>
        <li>
          <strong>{t.step3}</strong>
          <p className="muted">{t.step3p}</p>
        </li>
      </ol>
    </section>
  );
}
