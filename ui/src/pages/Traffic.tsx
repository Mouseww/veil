import { useEffect, useState } from "react";
import { api, type TrafficEvent } from "../api";
import { useT } from "../i18n";

const COLUMNS = ["ts", "method", "path_template", "status", "streaming", "hit_types", "latency_ms", "error_class", "creator_prefix8"] as const;

export function TrafficTable({ events }: { events: TrafficEvent[] }) {
  const { t } = useT();
  return (
    <section>
      <h2><span className="live" />{t.trafficTitle}</h2>
      <table>
        <thead>
          <tr>{COLUMNS.map((c) => <th key={c}>{c}</th>)}</tr>
        </thead>
        <tbody>
          {events.length === 0 ? (
            <tr><td colSpan={COLUMNS.length} className="muted">{t.none}</td></tr>
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
    </section>
  );
}

export default function TrafficPage() {
  const [events, setEvents] = useState<TrafficEvent[]>([]);
  useEffect(() => {
    let on = true;
    const tick = () => { api.traffic().then((e) => { if (on) setEvents(e); }).catch(() => undefined); };
    tick();
    const id = setInterval(tick, 2000);
    return () => { on = false; clearInterval(id); };
  }, []);
  return <TrafficTable events={events} />;
}

export { COLUMNS };
