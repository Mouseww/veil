import { useEffect, useState } from "react";
import { api, type StatusBody } from "../api";
import { useT } from "../i18n";

export default function StatusPage() {
  const { t } = useT();
  const [s, setS] = useState<StatusBody | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [upd, setUpd] = useState<{ current: string; latest: string; newer: boolean } | null>(null);
  const [updMsg, setUpdMsg] = useState("");
  useEffect(() => {
    api.status().then(setS).catch((e) => setErr(String(e)));
  }, []);
  if (err) return <p className="err">{t.statusErr}{err}</p>;
  if (!s) return <p className="muted">{t.loading}</p>;
  const base = "http://" + s.bind + ":" + s.proxy_port;
  return (
    <section>
      <h1>{t.startTitle}</h1>
      <p className="help">{t.helpStart}</p>
      <ol className="steps">
        <li>
          <strong>{t.step1}</strong>
          <div className="muted">{t.step1sub(base, s.bind, s.management_port)}</div>
        </li>
        <li>
          <strong>{t.step2}</strong>
          <p>
            {t.step2p} <code>veil setup</code>
            {t.step2or}
          </p>
          <pre className="copybox">ANTHROPIC_BASE_URL={base}</pre>
          <button
            type="button"
            onClick={async () => {
              await navigator.clipboard.writeText("ANTHROPIC_BASE_URL=" + base);
              setCopied(true);
            }}
          >
            {copied ? t.copied : t.copy}
          </button>
          <p className="muted">{t.step2hint}</p>
        </li>
        <li>
          <strong>{t.step3}</strong>
          <p className="muted">{t.step3p}</p>
        </li>
      </ol>
      <h3>{t.statusNow}</h3>
      <dl className="grid">
        <dt>{t.product}</dt><dd>{s.product}</dd>
        <dt>{t.masterKey}</dt><dd>{s.master_key_set ? t.keyReady : t.keyMissing}</dd>
        <dt>{t.ruleCount}</dt><dd>{s.rule_count}</dd>
        <dt>{t.lastError}</dt><dd>{s.last_error_class ?? t.none}</dd>
        <dt>{t.version}</dt><dd>{s.version ?? "?"}{upd ? (upd.newer ? ` → ${upd.latest} (${t.updateAvail})` : ` · ${t.upToDate}`) : ""}</dd>
      </dl>
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
          try {
            await api.applyUpdate();
          } catch (e) { setUpdMsg(String(e)); }
        }}>{t.updateNow}</button>
      )}
      {updMsg && <p className="flash">{updMsg}</p>}
    </section>
  );
}
