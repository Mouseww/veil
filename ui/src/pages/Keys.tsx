import { useState } from "react";
import { api, setAdminToken } from "../api";
import { useT } from "../i18n";

export default function KeysPage() {
  const { t } = useT();
  const [hex, setHex] = useState("");
  const [prefix, setPrefix] = useState("");
  const [msg, setMsg] = useState("");
  const [err, setErr] = useState("");
  return (
    <section>
      <h1>{t.keysTitle}</h1>
      <p className="help">{t.helpKeys}</p>
      {msg && <p className="flash">{msg}</p>}
      {err && <p className="err">{err}</p>}
      <label>{t.newKey}<input value={hex} onChange={(e) => setHex(e.target.value)} /></label>
      <button type="button" onClick={async () => { try { await api.rotateKey(hex); setHex(""); setMsg(t.rotated); setErr(""); } catch { setErr(t.failed); } }}>{t.rotate}</button>
      <label>{t.purgePrefix}<input value={prefix} onChange={(e) => setPrefix(e.target.value)} /></label>
      <button type="button" onClick={async () => { try { await api.purge(prefix || undefined); setMsg(t.purged); setErr(""); } catch { setErr(t.failed); } }}>{t.purge}</button>
      <button type="button" onClick={async () => { try { const tok = await api.resetToken(); setAdminToken(tok.token); setMsg(t.tokenStored); setErr(""); } catch { setErr(t.failed); } }}>{t.resetToken}</button>
    </section>
  );
}
