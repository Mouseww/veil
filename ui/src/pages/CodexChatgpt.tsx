import { useEffect, useState } from "react";
import { api, type CodexChatgptConnection } from "../api";
import { useT } from "../i18n";

export default function CodexChatgpt() {
  const { t } = useT();
  const [connection, setConnection] = useState<CodexChatgptConnection | null>(null);
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    api.codexChatgpt().then((c) => { if (active) setConnection(c); })
      .catch((e) => { if (active) setError(String(e.message)); });
    return () => { active = false; };
  }, []);
  const connect = async () => {
    setBusy(true);
    setError("");
    try {
      setConnection(await api.connectCodexChatgpt());
      setSaved(true);
    } catch (e) { setError(String((e as Error).message)); }
    finally { setBusy(false); }
  };
  return (
    <div className="card" style={{ marginBottom: 16 }}>
      <h2>{t.codexChatgptTitle}</h2>
      <p className="help">{t.codexChatgptHelp}</p>
      <p>{connection?.configured ? t.codexChatgptConfigured : t.codexChatgptNotConfigured}</p>
      {connection && <>
        <p className="muted">{t.clientBase}: <code>{connection.base_url}</code></p>
        <p className="muted">{t.codexProfilePath}: <code>{connection.profile_path}</code></p>
        <pre className="copybox">{connection.launch_command}</pre>
      </>}
      <button type="button" disabled={busy || !connection} onClick={() => void connect()}>
        {busy ? t.loading : t.codexChatgptConnect}
      </button>
      {saved && <p className="ok" role="status">{t.codexChatgptSaved}</p>}
      {error && <p className="err" role="alert">{t.failed}: {error}</p>}
      <p className="help">{t.codexChatgptVerify}</p>
      <pre className="copybox">请原样重复：13800138000</pre>
    </div>
  );
}
