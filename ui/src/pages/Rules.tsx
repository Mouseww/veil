import { useEffect, useState } from "react";
import { api, type DryHit, type RuleView } from "../api";
import { DryRun } from "../DryRun";

export default function RulesPage() {
  const [rules, setRules] = useState<RuleView[]>([]);
  const [allow, setAllow] = useState("");
  const [sample, setSample] = useState("call 13800138000");
  const [hits, setHits] = useState<DryHit[]>([]);
  useEffect(() => {
    api.rules().then(setRules).catch(() => undefined);
    api.allowlist().then((l) => setAllow(l.join("\n"))).catch(() => undefined);
  }, []);
  return (
    <section>
      <h2>rules</h2>
      <table>
        <thead>
          <tr><th>id</th><th>type</th><th>prio</th><th>on</th><th>kind</th></tr>
        </thead>
        <tbody>
          {rules.map((r) => (
            <tr key={r.id}>
              <td>{r.id}</td>
              <td>{r.type_prefix}</td>
              <td>{r.priority}</td>
              <td>{r.enabled ? "yes" : "no"}</td>
              <td>{r.kind}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <h3>allowlist</h3>
      <textarea value={allow} onChange={(e) => setAllow(e.target.value)} rows={4} />
      <button type="button" onClick={() => api.putAllowlist(allow.split(/\s+/).filter(Boolean))}>save allowlist</button>
      <h3>dry-run</h3>
      <textarea value={sample} onChange={(e) => setSample(e.target.value)} rows={3} />
      <button type="button" onClick={async () => setHits(await api.dryRun(sample))}>run</button>
      <DryRun sample={sample} hits={hits} />
    </section>
  );
}
