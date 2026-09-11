import { createContext, useContext, useMemo, useState, type ReactNode } from "react";

export type Lang = "zh" | "en";

const STR = {
  en: {
    tagline: "secrets stay on this machine",
    tabStart: "Start",
    tabRules: "Rules",
    tabUpstream: "Upstream",
    tabKeys: "Keys",
    tabTraffic: "Traffic",
    tokenPh: "admin token (optional on localhost)",
    startTitle: "Get started",
    startKicker: "three steps. the model never sees plaintext.",
    step1: "Veil is running on this machine",
    step1sub: (base: string, bind: string, port: number) =>
      `proxy ${base}  ·  console ${bind}:${port}`,
    step2: "Point your coding app at Veil",
    step2p: "In a terminal run",
    step2or: ", or add this to the app settings / user env:",
    copy: "Copy this line",
    copied: "Copied",
    step2hint: "Official Claude login needs no extra API key. Restart the app after setup.",
    step3: "Chat as usual",
    step3p: "Phones, connection strings, and keys become placeholders before they leave. Replies are restored on the way back.",
    statusNow: "Status",
    product: "product",
    masterKey: "master key",
    keyReady: "ready (never shown)",
    keyMissing: "missing",
    ruleCount: "rules",
    lastError: "last error",
    none: "none",
    loading: "loading…",
    statusErr: "cannot reach the gateway: ",
    rulesTitle: "Rules",
    colId: "id",
    colType: "type",
    colPrio: "prio",
    colOn: "on",
    colKind: "kind",
    yes: "yes",
    no: "no",
    allowlist: "allowlist",
    saveAllow: "save allowlist",
    dryRun: "dry-run",
    run: "run",
    dryKicker: "dry-run / sample stays local",
    noHits: "no hits",
    saved: "saved",
    failed: "failed",
    addRule: "add custom regex",
    pattern: "pattern",
    upTitle: "Upstream & limits",
    saveUp: "save upstream",
    ttl: "ttl days",
    bodyMib: "body MiB",
    saveLim: "save limits",
    clientBase: "client Base URL",
    keysTitle: "Keys / danger",
    keysHint: "the current master key is never displayed.",
    newKey: "new master key (64 hex)",
    rotate: "rotate",
    rotated: "rotated",
    purgePrefix: "purge creator prefix",
    purge: "purge",
    purged: "purged",
    resetToken: "reset admin token",
    tokenStored: "new admin token stored in this session",
    trafficTitle: "Live traffic",
    version: "version",
    checkUpdate: "Check for updates",
    updateNow: "Update now",
    upToDate: "up to date",
    updateAvail: "update available",
    updating: "downloading and restarting…",
  },
  zh: {
    tagline: "敏感信息不出网",
    tabStart: "开始",
    tabRules: "规则",
    tabUpstream: "上游",
    tabKeys: "密钥",
    tabTraffic: "流量",
    tokenPh: "管理令牌（本机可空）",
    startTitle: "开始使用",
    startKicker: "三步，敏感信息就不会出网",
    step1: "网关已在本机运行",
    step1sub: (base: string, bind: string, port: number) =>
      `代理 ${base}　·　管理 ${bind}:${port}`,
    step2: "让编程助手走网关",
    step2p: "在终端执行",
    step2or: "，或把下面这一行加到应用设置 / 用户环境变量：",
    copy: "复制这一行",
    copied: "已复制",
    step2hint: "官方订阅不用另填 API Key。改完请重启对应应用。",
    step3: "正常聊天即可",
    step3p: "手机号、连接串、密钥会先换成占位符再发给模型，回复里自动变回原文。",
    statusNow: "当前状态",
    product: "产品",
    masterKey: "主密钥",
    keyReady: "已就绪（不显示明文）",
    keyMissing: "缺失",
    ruleCount: "规则数",
    lastError: "最近错误",
    none: "无",
    loading: "加载中…",
    statusErr: "管理页连不上网关：",
    rulesTitle: "规则",
    colId: "id",
    colType: "类型",
    colPrio: "优先级",
    colOn: "启用",
    colKind: "种类",
    yes: "是",
    no: "否",
    allowlist: "白名单",
    saveAllow: "保存白名单",
    dryRun: "试跑",
    run: "运行",
    dryKicker: "试跑 / 样本只留在本机",
    noHits: "无命中",
    saved: "已保存",
    failed: "失败",
    addRule: "添加自定义正则",
    pattern: "正则",
    upTitle: "上游与限额",
    saveUp: "保存上游",
    ttl: "TTL（天）",
    bodyMib: "请求体 MiB",
    saveLim: "保存限额",
    clientBase: "客户端 Base URL",
    keysTitle: "密钥 / 危险操作",
    keysHint: "当前主密钥永远不会显示。",
    newKey: "新主密钥（64 位十六进制）",
    rotate: "轮换",
    rotated: "已轮换",
    purgePrefix: "按 Creator 前缀清除",
    purge: "清除",
    purged: "已清除",
    resetToken: "重置管理令牌",
    tokenStored: "新管理令牌已存入本会话",
    trafficTitle: "实时流量",
    version: "版本",
    checkUpdate: "检查更新",
    updateNow: "立即更新",
    upToDate: "已是最新",
    updateAvail: "有新版本",
    updating: "正在下载并重启…",
  },
} as const;

type Dict = (typeof STR)["en"];

const Ctx = createContext<{ lang: Lang; setLang: (l: Lang) => void; t: Dict }>({
  lang: "en",
  setLang: () => undefined,
  t: STR.en,
});

function detect(): Lang {
  try {
    const saved = localStorage.getItem("veil_lang");
    if (saved === "zh" || saved === "en") return saved;
  } catch {
    /* ignore */
  }
  const nav = typeof navigator !== "undefined" ? navigator.language : "en";
  return nav.toLowerCase().startsWith("zh") ? "zh" : "en";
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>(detect);
  const setLang = (l: Lang) => {
    setLangState(l);
    try {
      localStorage.setItem("veil_lang", l);
    } catch {
      /* ignore */
    }
  };
  const value = useMemo(() => ({ lang, setLang, t: STR[lang] }), [lang]);
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useT() {
  return useContext(Ctx);
}

export function LangSwitch() {
  const { lang, setLang } = useT();
  return (
    <span className="lang">
      <button type="button" className={lang === "en" ? "active" : ""} onClick={() => setLang("en")}>
        EN
      </button>
      <button type="button" className={lang === "zh" ? "active" : ""} onClick={() => setLang("zh")}>
        中文
      </button>
    </span>
  );
}
