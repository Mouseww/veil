import { useState } from "react";
import { api, setAdminToken } from "../api";
import { useT } from "../i18n";

function randHex64() {
  const b = new Uint8Array(32);
  crypto.getRandomValues(b);
  return [...b].map((x) => x.toString(16).padStart(2, "0")).join("");
}

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
      <p className="flash">{t.keysLeaveAlone}</p>
      {msg && <p className="ok">{msg}</p>}
      {err && <p className="err">{err}</p>}

      <div className="card" style={{ marginBottom: 16 }}>
        <h2>{t.keyMasterTitle}</h2>
        <p className="help">{t.keyMasterWhat}</p>
        <p><strong>{t.keysWhen}</strong> {t.keyMasterWhen}</p>
        <p className="muted"><strong>{t.keysWhenNot}</strong> {t.keyMasterWhenNot}</p>
        <p className="err">{t.keyMasterImpact}</p>
        <label>{t.newKey}<input value={hex} onChange={(e) => setHex(e.target.value)} /></label>
        <button type="button" className="ghost" onClick={() => setHex(randHex64())}>{t.genKey}</button>
        <button type="button" onClick={async () => {
          try {
            await api.rotateKey(hex);
            setHex("");
            setMsg(t.rotated);
            setErr("");
          } catch { setErr(t.failed); }
        }}>{t.rotate}</button>
      </div>

      <div className="card" style={{ marginBottom: 16 }}>
        <h2>{t.keyPurgeTitle}</h2>
        <p className="help">{t.keyPurgeWhat}</p>
        <p><strong>{t.keysWhen}</strong> {t.keyPurgeWhen}</p>
        <p className="muted"><strong>{t.keysWhenNot}</strong> {t.keyPurgeWhenNot}</p>
        <p className="err">{t.keyPurgeImpact}</p>
        <label>{t.purgePrefix}<input value={prefix} onChange={(e) => setPrefix(e.target.value)} /></label>
        <button type="button" onClick={async () => {
          try {
            await api.purge(prefix || undefined);
            setMsg(t.purged);
            setErr("");
          } catch { setErr(t.failed); }
        }}>{t.purge}</button>
      </div>

      <div className="card">
        <h2>{t.keyAdminTitle}</h2>
        <p className="help">{t.keyAdminWhat}</p>
        <p><strong>{t.keysWhen}</strong> {t.keyAdminWhen}</p>
        <p className="muted"><strong>{t.keysWhenNot}</strong> {t.keyAdminWhenNot}</p>
        <button type="button" onClick={async () => {
          try {
            const tok = await api.resetToken();
            setAdminToken(tok.token);
            setMsg(t.tokenStored);
            setErr("");
          } catch { setErr(t.failed); }
        }}>{t.resetToken}</button>
      </div>
    </section>
  );
}
