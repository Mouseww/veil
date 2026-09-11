import { useEffect, useState } from "react";
import { api, type TrafficEvent } from "../api";
import { useT } from "../i18n";

const COLUMNS = ["ts", "method", "path_template", "status", "streaming", "hit_types", "latency_ms", "error_class", "creator_prefix8"] as const;

export function TrafficTable({ events }: { events: TrafficEvent[] }) {
  return (
    <table>
      <thead>
        <tr>{COLUMNS.map((c) => <th key={c}>{c}</th>)}</tr>
      </thead>
      <tbody>
        {events.length === 0 ? (
          <tr><td colSpan={COLUMNS.length} className="muted">—</td></tr>
        ) : events.slice().reverse().map((e, i) => (
          <tr key={i}>
            <td>{new Date(e.ts).toLocaleTimeString()}</td>
            <td>{e.method}</td>
            <td>{e.path_template}</td>
            <td>{e.status}</td>
            <td>{e.streaming ? "sse" : ""}</td>
            <td>{e.hit_types.join(",")}</td>
            <td>{e.latency_ms}</td>
            <td>{e.error_class ?? ""}</td>
            <td>{e.creator_prefix8}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export default function TrafficPage() {
  const { t } = useT();
  const [events, setEvents] = useState<TrafficEvent[]>([]);
  const [live, setLive] = useState(true);
  const [sec, setSec] = useState(2);
  useEffect(() => {
    let on = true;
    const tick = () => { api.traffic().then((e) => { if (on) setEvents(e); }).catch(() => undefined); };
    tick();
    if (!live) return () => { on = false; };
    const id = setInterval(tick, Math.max(1, sec) * 1000);
    return () => { on = false; clearInterval(id); };
  }, [live, sec]);
  return (
    <section>
      <h1>{t.trafficTitle}</h1>
      <p className="help">{t.helpTraffic}</p>
      <div className="row" style={{ alignItems: "center", marginBottom: 12 }}>
        <label style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span className={"switch" + (live ? " on" : "")} onClick={() => setLive(!live)} />
          {t.autoRefresh}
        </label>
        <label>{t.everySec}
          <select value={sec} onChange={(e) => setSec(Number(e.target.value))} disabled={!live}>
            {[1, 2, 5, 10, 30].map((n) => <option key={n} value={n}>{n}s</option>)}
          </select>
        </label>
      </div>
      <TrafficTable events={events} />
    </section>
  );
}

export { COLUMNS };
