import { useState } from "react";
import StatusPage from "./pages/Status";
import RulesPage from "./pages/Rules";
import UpstreamPage from "./pages/Upstream";
import KeysPage from "./pages/Keys";
import TrafficPage from "./pages/Traffic";
import { setAdminToken } from "./api";

const TABS = ["status", "rules", "upstream", "keys", "traffic"] as const;
type Tab = (typeof TABS)[number];

export default function App() {
  const [tab, setTab] = useState<Tab>("status");
  const [token, setToken] = useState("");
  return (
    <>
      <header>
        <strong>dgw</strong>
        <span className="muted">desensitization gateway console</span>
        <nav>
          {TABS.map((t) => (
            <button key={t} className={tab === t ? "active" : ""} onClick={() => setTab(t)}>
              {t}
            </button>
          ))}
        </nav>
        <input
          placeholder="admin token"
          value={token}
          onChange={(e) => {
            setToken(e.target.value);
            setAdminToken(e.target.value);
          }}
          style={{ maxWidth: 220 }}
        />
      </header>
      <main>
        {tab === "status" && <StatusPage />}
        {tab === "rules" && <RulesPage />}
        {tab === "upstream" && <UpstreamPage />}
        {tab === "keys" && <KeysPage />}
        {tab === "traffic" && <TrafficPage />}
      </main>
    </>
  );
}
