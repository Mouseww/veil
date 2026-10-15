import { useEffect, useState } from "react";
import DashboardPage from "./pages/Dashboard";
import RulesPage from "./pages/Rules";
import UpstreamPage from "./pages/Upstream";
import KeysPage from "./pages/Keys";
import TrafficPage from "./pages/Traffic";
import LoginPage from "./pages/Login";
import { api, getAdminToken, setAdminToken, verifyAdminToken } from "./api";
import { LangSwitch, useT } from "./i18n";
import { Ico } from "./icons";

type Tab = "status" | "rules" | "upstream" | "keys" | "traffic";
type AuthState = "checking" | "needed" | "ok";

function useTheme() {
  const [dark, setDark] = useState(() => localStorage.getItem("veil_theme") === "dark");
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", dark ? "dark" : "light");
    localStorage.setItem("veil_theme", dark ? "dark" : "light");
  }, [dark]);
  return { dark, toggle: () => setDark((d) => !d) };
}

export default function App() {
  const { t } = useT();
  const { dark, toggle } = useTheme();
  const [tab, setTab] = useState<Tab>("status");
  const [auth, setAuth] = useState<AuthState>("checking");
  const [loginSeed, setLoginSeed] = useState("");
  const [running, setRunning] = useState(true);
  const [ver, setVer] = useState("");

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const urlToken = params.get("token");
    if (urlToken) {
      setAdminToken(urlToken);
      window.history.replaceState({}, "", window.location.pathname);
    }
    const candidate = urlToken || getAdminToken();
    let cancelled = false;
    (async () => {
      try {
        const s = await verifyAdminToken(candidate);
        if (cancelled) return;
        if (candidate) setAdminToken(candidate);
        setRunning(true);
        setVer(s.version ?? "");
        setAuth("ok");
      } catch (e) {
        if (cancelled) return;
        const status = (e as Error & { status?: number }).status;
        const msg = String((e as Error).message || e);
        if (status === 401 || msg === "401") {
          setLoginSeed(candidate);
          setAuth("needed");
          setRunning(false);
        } else {
          // Gateway unreachable — still show shell so user sees stopped state.
          setRunning(false);
          setAuth("ok");
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  if (auth === "checking") {
    return (
      <div className="login-shell">
        <p className="muted">{t.loading}</p>
      </div>
    );
  }

  if (auth === "needed") {
    return (
      <LoginPage
        initialToken={loginSeed}
        onSuccess={(tok) => {
          setAdminToken(tok);
          setAuth("ok");
          setRunning(true);
          api.status().then((s) => setVer(s.version ?? "")).catch(() => undefined);
        }}
      />
    );
  }

  const tabs: { id: Tab; label: string; icon: typeof Ico.dash }[] = [
    { id: "status", label: t.tabDash, icon: Ico.dash },
    { id: "rules", label: t.tabRules, icon: Ico.scan },
    { id: "upstream", label: t.tabUpstream, icon: Ico.net },
    { id: "keys", label: t.tabKeys, icon: Ico.key },
    { id: "traffic", label: t.tabTraffic, icon: Ico.list },
  ];

  async function logout() {
    setAdminToken("");
    setLoginSeed("");
    try {
      const s = await verifyAdminToken("");
      setRunning(true);
      setVer(s.version ?? "");
      setAuth("ok");
    } catch {
      setRunning(false);
      setAuth("needed");
    }
  }

  return (
    <div className="shell">
      <aside className="nav">
        <div className="logo"><Ico.shield /> Veil</div>
        {tabs.map((item) => (
          <button key={item.id} className={"navbtn" + (tab === item.id ? " active" : "")} onClick={() => setTab(item.id)}>
            {item.icon()} {item.label}
          </button>
        ))}
        <div className="nav-foot">
          {getAdminToken() ? (
            <button type="button" className="ghost nav-logout" onClick={logout}>
              {t.logout}
            </button>
          ) : (
            <button type="button" className="ghost nav-logout" onClick={() => setAuth("needed")}>
              {t.loginSubmit}
            </button>
          )}
        </div>
      </aside>
      <div>
        <div className="topbar">
          <span className={"pill " + (running ? "ok" : "off")}>
            <span className="dot" /> {running ? t.running : t.stopped}{ver ? ` · v${ver}` : ""}
          </span>
          <div className="top-actions">
            <LangSwitch />
            <button className="iconbtn" type="button" onClick={toggle} title="theme">{dark ? <Ico.sun /> : <Ico.moon />}</button>
          </div>
        </div>
        <main className="workspace">
          {tab === "status" && <DashboardPage />}
          {tab === "rules" && <RulesPage />}
          {tab === "upstream" && <UpstreamPage />}
          {tab === "keys" && <KeysPage />}
          {tab === "traffic" && <TrafficPage />}
        </main>
      </div>
    </div>
  );
}
