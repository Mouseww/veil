import { useState } from "react";
import StatusPage from "./pages/Status";
import RulesPage from "./pages/Rules";
import UpstreamPage from "./pages/Upstream";
import KeysPage from "./pages/Keys";
import TrafficPage from "./pages/Traffic";
import { setAdminToken } from "./api";
import { LangSwitch, useT } from "./i18n";

type Tab = "status" | "rules" | "upstream" | "keys" | "traffic";

export default function App() {
  const { t } = useT();
  const [tab, setTab] = useState<Tab>("status");
  const [token, setToken] = useState("");
  const tabs: { id: Tab; label: string }[] = [
    { id: "status", label: t.tabStart },
    { id: "rules", label: t.tabRules },
    { id: "upstream", label: t.tabUpstream },
    { id: "keys", label: t.tabKeys },
    { id: "traffic", label: t.tabTraffic },
  ];
  return (
    <div className="shell">
      <aside>
        <div>
          <div className="brand">Veil</div>
          <div className="tag">{t.tagline}</div>
        </div>
        <nav>
          {tabs.map((item) => (
            <button key={item.id} className={tab === item.id ? "active" : ""} onClick={() => setTab(item.id)}>
              {item.label}
            </button>
          ))}
        </nav>
        <LangSwitch />
        <input
          placeholder={t.tokenPh}
          value={token}
          onChange={(e) => {
            setToken(e.target.value);
            setAdminToken(e.target.value);
          }}
        />
      </aside>
      <main className="workspace">
        {tab === "status" && <StatusPage />}
        {tab === "rules" && <RulesPage />}
        {tab === "upstream" && <UpstreamPage />}
        {tab === "keys" && <KeysPage />}
        {tab === "traffic" && <TrafficPage />}
      </main>
    </div>
  );
}
