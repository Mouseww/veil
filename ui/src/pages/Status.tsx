import { useEffect, useState } from "react";
import { api, type StatusBody } from "../api";

export default function StatusPage() {
  const [s, setS] = useState<StatusBody | null>(null);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    api.status().then(setS).catch((e) => setErr(String(e)));
  }, []);
  if (err) return <p className="err">{err}</p>;
  if (!s) return <p className="muted">loading…</p>;
  return (
    <section>
      <h2>status</h2>
      <dl className="grid">
        <dt>product</dt><dd>{s.product}</dd>
        <dt>bind</dt><dd>{s.bind}</dd>
        <dt>proxy</dt><dd>{s.proxy_port}</dd>
        <dt>mgmt</dt><dd>{s.management_port}</dd>
        <dt>anthropic</dt><dd>{s.anthropic_upstream}</dd>
        <dt>openai chat</dt><dd>{s.openai_completions_upstream}</dd>
        <dt>openai responses</dt><dd>{s.openai_responses_upstream}</dd>
        <dt>ttl days</dt><dd>{s.mapping_ttl_days}</dd>
        <dt>body MiB</dt><dd>{s.request_body_limit_mib}</dd>
        <dt>key</dt><dd>{s.master_key_set ? "set" : "missing"}</dd>
        <dt>rules</dt><dd>{s.rule_count}</dd>
        <dt>last error</dt><dd>{s.last_error_class ?? "—"}</dd>
      </dl>
    </section>
  );
}
