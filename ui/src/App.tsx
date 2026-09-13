import { useEffect, useState } from "react";
import DashboardPage from "./pages/Dashboard";
import RulesPage from "./pages/Rules";
import UpstreamPage from "./pages/Upstream";
import KeysPage from "./pages/Keys";
import TrafficPage from "./pages/Traffic";
import { api, setAdminToken } from "./api";
import { LangSwitch, useT } from "./i18n";
import { Ico } from "./icons";

type Tab = "status" | "rules" | "upstream" | "keys" | "traffic";

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
  const [token, setToken] = useState(() => sessionStorage.getItem("veil_admin") ?? "");
  const [running, setRunning] = useState(true);
  const [ver, setVer] = useState("");
  useEffect(() => {
    // Pick up token injected by the desktop launcher via ?token=xxx
    const params = new URLSearchParams(window.location.search);
    const urlToken = params.get("token");
    if (urlToken) {
      setToken(urlToken);
      setAdminToken(urlToken);
      window.history.replaceState({}, "", window.location.pathname);
    }
    api.status().then((s) => { setRunning(true); setVer(s.version ?? ""); }).catch(() => setRunning(false));
  }, []);
  const tabs: { id: Tab; label: string; icon: typeof Ico.dash }[] = [
    { id: "status", label: t.tabDash, icon: Ico.dash },
    { id: "rules", label: t.tabRules, icon: Ico.scan },
    { id: "upstream", label: t.tabUpstream, icon: Ico.net },
    { id: "keys", label: t.tabKeys, icon: Ico.key },
    { id: "traffic", label: t.tabTraffic, icon: Ico.list },
  ];
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
          <input className="token" placeholder={t.tokenPh} value={token} onChange={(e) => { setToken(e.target.value); setAdminToken(e.target.value); }} />
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
