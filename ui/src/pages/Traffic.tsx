import type { TrafficEvent } from "../api";

const COLUMNS = ["ts", "method", "path_template", "status", "streaming", "hit_types", "latency_ms", "error_class", "creator_prefix8"] as const;

export function TrafficTable({ events }: { events: TrafficEvent[] }) {
  return (
    <section>
      <h2>live traffic</h2>
      <table>
        <thead>
          <tr>
            {COLUMNS.map((c) => (
              <th key={c}>{c}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {events.map((e, i) => (
            <tr key={i}>
              <td>{e.ts}</td>
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
  return <TrafficTable events={[]} />;
}

export { COLUMNS };
