import { useState } from "react";
import { api, setAdminToken } from "../api";

export default function KeysPage() {
  const [hex, setHex] = useState("");
  const [prefix, setPrefix] = useState("");
  const [msg, setMsg] = useState("");
  return (
    <section>
      <h2>keys / danger</h2>
      <p className="muted">current master key is never displayed.</p>
      <label>new master key (64 hex)<input value={hex} onChange={(e) => setHex(e.target.value)} /></label>
      <button type="button" onClick={async () => { await api.rotateKey(hex); setHex(""); setMsg("rotated"); }}>rotate</button>
      <label>purge creator prefix<input value={prefix} onChange={(e) => setPrefix(e.target.value)} /></label>
      <button type="button" onClick={async () => { await api.purge(prefix || undefined); setMsg("purged"); }}>purge</button>
      <button type="button" onClick={async () => { const t = await api.resetToken(); setAdminToken(t.token); setMsg("new admin token stored in session"); }}>reset admin token</button>
      <p>{msg}</p>
    </section>
  );
}
