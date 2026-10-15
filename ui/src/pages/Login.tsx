import { FormEvent, useState } from "react";
import { setAdminToken, verifyAdminToken } from "../api";
import { LangSwitch, useT } from "../i18n";
import { Ico } from "../icons";

type Props = {
  initialToken?: string;
  onSuccess: (token: string) => void;
};

export default function LoginPage({ initialToken = "", onSuccess }: Props) {
  const { t } = useT();
  const [token, setToken] = useState(initialToken);
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    const value = token.trim();
    setBusy(true);
    setErr("");
    try {
      await verifyAdminToken(value);
      setAdminToken(value);
      onSuccess(value);
    } catch {
      setErr(value ? t.loginBadToken : t.loginNeedToken);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="login-shell">
      <div className="login-card card">
        <div className="login-brand">
          <Ico.shield />
          <div>
            <h1>Veil</h1>
            <p className="muted">{t.loginSubtitle}</p>
          </div>
        </div>
        <p className="help">{t.loginHelp}</p>
        <form onSubmit={submit}>
          <label>
            {t.loginTokenLabel}
            <input
              type="password"
              autoComplete="current-password"
              autoFocus
              value={token}
              placeholder={t.loginTokenPh}
              onChange={(e) => setToken(e.target.value)}
            />
          </label>
          {err && <p className="err">{err}</p>}
          <button type="submit" disabled={busy}>
            {busy ? t.loading : t.loginSubmit}
          </button>
        </form>
        <p className="muted login-hint">{t.loginHint}</p>
        <div className="login-foot">
          <LangSwitch />
        </div>
      </div>
    </div>
  );
}
