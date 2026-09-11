import { useEffect, useState } from "react";
import { api, type DryHit, type RuleView } from "../api";
import { DryRun } from "../DryRun";
import { useT } from "../i18n";

export default function RulesPage() {
  const { t } = useT();
  const [rules, setRules] = useState<RuleView[]>([]);
  const [open, setOpen] = useState<string | null>(null);
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
  const patch = (id: string, part: Partial<RuleView>) =>
    saveRules(rules.map((x) => (x.id === id ? { ...x, ...part } : x)));
  return (
    <section>
      <h1>{t.rulesTitle}</h1>
      <p className="help">{t.helpRules}</p>
      {flash && <p className="flash">{flash}</p>}
      {rules.map((r) => (
        <div className="rule" key={r.id}>
          <div className="rule-head" onClick={() => setOpen(open === r.id ? null : r.id)} role="button" tabIndex={0}>
            <span className={"switch" + (r.enabled ? " on" : "")} onClick={(e) => { e.stopPropagation(); void patch(r.id, { enabled: !r.enabled }); }} />
            <strong>{r.id}</strong>
            <span className="chip">{r.type_prefix}</span>
            <span className="meta">{r.kind} · prio {r.priority} · {r.source}</span>
          </div>
          {open === r.id && (
            <div className="rule-body">
              {r.kind === "ip" && <p className="help">{t.builtinHint}</p>}
              <div className="row">
                <div className="field"><label>{t.colType}</label><input value={r.type_prefix} onChange={(e) => setRules(rules.map((x) => x.id === r.id ? { ...x, type_prefix: e.target.value } : x))} /></div>
                <div className="field"><label>{t.colPrio}</label><input type="number" value={r.priority} onChange={(e) => setRules(rules.map((x) => x.id === r.id ? { ...x, priority: Number(e.target.value) } : x))} /></div>
              </div>
              {r.kind !== "ip" && r.kind !== "dictionary" && (
                <div className="field"><label>{t.pattern}</label><textarea rows={3} value={r.pattern ?? ""} onChange={(e) => setRules(rules.map((x) => x.id === r.id ? { ...x, pattern: e.target.value } : x))} /></div>
              )}
              {r.kind === "dictionary" && (
                <div className="field"><label>{t.words}</label><textarea rows={4} value={(r.words ?? []).join("\n")} onChange={(e) => setRules(rules.map((x) => x.id === r.id ? { ...x, words: e.target.value.split(/\n/).map((w) => w.trim()).filter(Boolean) } : x))} /></div>
              )}
              <button className="btn" type="button" onClick={() => void saveRules(rules)}>{t.saveRule}</button>
              {r.source === "custom" && (
                <button className="danger" type="button" onClick={() => { setOpen(null); void saveRules(rules.filter((x) => x.id !== r.id)); }}>{t.deleteRule}</button>
              )}
            </div>
          )}
        </div>
      ))}
      <h2>{t.addRule}</h2>
      <div className="row">
        <div className="field"><label>{t.colId}</label><input value={nid} onChange={(e) => setNid(e.target.value)} /></div>
        <div className="field"><label>{t.colType}</label><input value={ntp} onChange={(e) => setNtp(e.target.value)} /></div>
      </div>
      <div className="field"><label>{t.pattern}</label><input value={npat} onChange={(e) => setNpat(e.target.value)} placeholder="(?<!\\d)1[3-9]\\d{9}(?!\\d)" /></div>
      <button className="btn" type="button" onClick={() => {
        if (!nid || !ntp || !npat) return;
        void saveRules([...rules, { id: nid, type_prefix: ntp, enabled: true, priority: 50, kind: "regex", source: "custom", pattern: npat }]);
        setNpat("");
      }}>{t.addRule}</button>
      <h2>{t.allowlist}</h2>
      <p className="help">{t.helpRules}</p>
      <textarea value={allow} onChange={(e) => setAllow(e.target.value)} rows={4} />
      <button className="btn" type="button" onClick={async () => { try { await api.putAllowlist(allow.split(/\s+/).filter(Boolean)); setFlash(t.saved); } catch { setFlash(t.failed); } }}>{t.saveAllow}</button>
      <h2>{t.dryRun}</h2>
      <textarea value={sample} onChange={(e) => setSample(e.target.value)} rows={3} />
      <button className="btn" type="button" onClick={async () => setHits(await api.dryRun(sample))}>{t.run}</button>
      <DryRun sample={sample} hits={hits} />
    </section>
  );
}
