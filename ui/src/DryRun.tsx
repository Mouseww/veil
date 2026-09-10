import type { DryHit } from "./api";

export function DryRun({ sample, hits }: { sample: string; hits: DryHit[] }) {
  return (
    <div className="dry">
      <div className="kicker">dry-run / sample stays local</div>
      {hits.length === 0 ? (
        <p className="muted">no hits</p>
      ) : (
        <ul>
          {hits.map((h, i) => (
            <li key={i}>
              <span className="chip">{h.type_prefix}</span>{" "}
              <code>{sample.slice(h.start, h.end) || h.matched}</code>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
