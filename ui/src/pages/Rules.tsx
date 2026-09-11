import { useEffect, useState } from "react";
import { api, type DryHit, type RuleView } from "../api";
import { DryRun } from "../DryRun";
import { useT } from "../i18n";

export default function RulesPage() {
  const { t } = useT();
  const [rules, setRules] = useState<RuleView[]>([]);
  const [allow, setAllow] = useState("");
  const [sample, setSample] = useState("call 13800138000");
  const [hits, setHits] = useState<DryHit[]>([]);
  const [flash, setFlash] = useState("");
  const [nid, setNid] = useState("custom1");
  const [ntp, setNtp] = useState("CUSTOM");
  const [npat, setNpat] = useState("");
  const load = () => {
    api.rules().then(setRules).catch(() => undefined);
    api.allowlist().then((l) => setAllow(l.join("\n"))).catch(() => undefined);
  };
  useEffect(load, []);
  const saveRules = async (next: RuleView[]) => {
    try {
      await api.putRules(next);
      setRules(next);
      setFlash(t.saved);
    } catch {
      setFlash(t.failed);
    }
  };
  return (
    <section>
      <h2>{t.rulesTitle}</h2>
      {flash && <p className="flash">{flash}</p>}
      <table>
        <thead>
          <tr>
            <th>{t.colId}</th><th>{t.colType}</th><th>{t.colPrio}</th><th>{t.colOn}</th><th>{t.colKind}</th>
          </tr>
        </thead>
        <tbody>
          {rules.map((r) => (
            <tr key={r.id}>
              <td>{r.id}</td>
              <td>{r.type_prefix}</td>
              <td>{r.priority}</td>
              <td>
                <button className="ghost" type="button" onClick={() => saveRules(rules.map((x) => x.id === r.id ? { ...x, enabled: !x.enabled } : x))}>
                  {r.enabled ? t.yes : t.no}
                </button>
              </td>
              <td>{r.kind}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <h3>{t.addRule}</h3>
      <div className="row">
        <label>{t.colId}<input value={nid} onChange={(e) => setNid(e.target.value)} /></label>
        <label>{t.colType}<input value={ntp} onChange={(e) => setNtp(e.target.value)} /></label>
      </div>
      <label>{t.pattern}<input value={npat} onChange={(e) => setNpat(e.target.value)} placeholder="1[3-9]\\d{9}" /></label>
      <button type="button" onClick={() => {
        if (!nid || !ntp || !npat) return;
        void saveRules([...rules, { id: nid, type_prefix: ntp, enabled: true, priority: 50, kind: "regex", source: "custom", pattern: npat }]);
        setNpat("");
      }}>{t.addRule}</button>
      <h3>{t.allowlist}</h3>
      <textarea value={allow} onChange={(e) => setAllow(e.target.value)} rows={4} />
      <button type="button" onClick={async () => { try { await api.putAllowlist(allow.split(/\s+/).filter(Boolean)); setFlash(t.saved); } catch { setFlash(t.failed); } }}>{t.saveAllow}</button>
      <h3>{t.dryRun}</h3>
      <textarea value={sample} onChange={(e) => setSample(e.target.value)} rows={3} />
      <button type="button" onClick={async () => setHits(await api.dryRun(sample))}>{t.run}</button>
      <DryRun sample={sample} hits={hits} />
    </section>
  );
}
