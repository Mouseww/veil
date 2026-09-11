import type { DryHit } from "./api";
import { useT } from "./i18n";

export function DryRun({ sample, hits }: { sample: string; hits: DryHit[] }) {
  const { t } = useT();
  return (
    <div className="dry">
      <div className="kicker">{t.dryKicker}</div>
      {hits.length === 0 ? (
        <p className="muted">{t.noHits}</p>
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
