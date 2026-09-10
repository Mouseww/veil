import { useState } from "react";
import StatusPage from "./pages/Status";
import RulesPage from "./pages/Rules";
import UpstreamPage from "./pages/Upstream";
import KeysPage from "./pages/Keys";
import TrafficPage from "./pages/Traffic";
import { setAdminToken } from "./api";

const TABS = [
  { id: "status", label: "开始" },
  { id: "rules", label: "规则" },
  { id: "upstream", label: "上游" },
  { id: "keys", label: "密钥" },
  { id: "traffic", label: "流量" },
] as const;

type Tab = (typeof TABS)[number]["id"];

export default function App() {
  const [tab, setTab] = useState<Tab>("status");
  const [token, setToken] = useState("");
  return (
    <>
      <header>
        <strong>Veil</strong>
        <span className="muted">敏感信息不出网</span>
        <nav>
          {TABS.map((t) => (
            <button key={t.id} className={tab === t.id ? "active" : ""} onClick={() => setTab(t.id)}>
              {t.label}
            </button>
          ))}
        </nav>
        <input
          placeholder="管理令牌（本机可空）"
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
