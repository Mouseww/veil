import { useEffect, useState } from "react";
import { api } from "../api";

export default function UpstreamPage() {
  const [a, setA] = useState("");
  const [c, setC] = useState("");
  const [r, setR] = useState("");
  const [ttl, setTtl] = useState(90);
  const [lim, setLim] = useState(32);
  const [bind, setBind] = useState("127.0.0.1");
  const [proxy, setProxy] = useState(18787);
  useEffect(() => {
    api.upstream().then((u) => {
      setA(u.anthropic_upstream);
      setC(u.openai_completions_upstream);
      setR(u.openai_responses_upstream);
    }).catch(() => undefined);
    api.status().then((s) => {
      setTtl(s.mapping_ttl_days);
      setLim(s.request_body_limit_mib);
      setBind(s.bind);
      setProxy(s.proxy_port);
    }).catch(() => undefined);
  }, []);
  const base = "http://" + bind + ":" + proxy;
  return (
    <section>
      <h2>upstream &amp; access</h2>
      <label>anthropic<input value={a} onChange={(e) => setA(e.target.value)} /></label>
      <label>openai chat<input value={c} onChange={(e) => setC(e.target.value)} /></label>
      <label>openai responses<input value={r} onChange={(e) => setR(e.target.value)} /></label>
      <button type="button" onClick={() => api.putUpstream({ anthropic_upstream: a, openai_completions_upstream: c, openai_responses_upstream: r })}>save upstream</button>
      <label>ttl days<input type="number" value={ttl} onChange={(e) => setTtl(Number(e.target.value))} /></label>
      <label>body MiB<input type="number" value={lim} onChange={(e) => setLim(Number(e.target.value))} /></label>
      <button type="button" onClick={() => api.putSettings({ mapping_ttl_days: ttl, request_body_limit_mib: lim })}>save limits</button>
      <p>client Base URL <code>{base}</code></p>
    </section>
  );
}
