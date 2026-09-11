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
    running: "Proxy running",
    stopped: "Cannot reach gateway",
    helpStart: "This is the home page. Veil is already the local hop. Point your coding apps at it, then chat as usual. Official Claude login needs no extra key.",
    helpRules: "Rules decide what gets replaced with placeholders before a request leaves your machine. Toggle a rule, expand it to edit the regex or dictionary, or add your own. Allowlist values are never replaced. Dry-run only runs locally — nothing is sent to the model.",
    helpUp: "Upstream is where Veil forwards traffic AFTER redacting. Official Anthropic stays as api.anthropic.com unless you use a company/LiteLLM gateway. Coding apps should use the client Base URL below (localhost), not the upstream URL.",
    helpKeys: "The master key encrypts the mapping table on disk. Rotating it makes old placeholders unrestorable. Purge deletes mappings for one API-key prefix. Resetting the admin token only affects this console.",
    helpTraffic: "Live requests through the proxy. No bodies, no query strings, no plaintext secrets — only method, path, status, which rule types hit, and a short creator prefix.",
    edit: "Edit",
    saveRule: "Save rule",
    deleteRule: "Delete",
    words: "dictionary words (one per line)",
    builtinHint: "Built-in IP matching has no regex — it parses every IPv4/IPv6 token. You can still disable it.",
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
    running: "代理运行中",
    stopped: "连不上网关",
    helpStart: "这是首页。Veil 已经是本机这一跳：把编程助手指过来，然后照常聊天。官方 Claude 登录不用另填 Key。",
    helpRules: "规则决定哪些内容在出网前被换成占位符。可以开关、展开编辑正则/词表，或自己加一条。白名单里的值永远不替换。试跑只在本机执行，不会发给模型。",
    helpUp: "上游是脱敏之后流量要去的真实 API。官方 Anthropic 默认 api.anthropic.com；公司网关 / LiteLLM 才改这里。编程助手应填写下面的客户端 Base URL（本机），不要填上游地址。",
    helpKeys: "主密钥用来加密磁盘上的映射表。轮换后旧占位符无法还原。清除会删掉某个 API Key 前缀下的映射。重置管理令牌只影响本控制台。",
    helpTraffic: "经过代理的实时请求。没有请求体、没有 query、没有明文机密，只有方法、路径、状态、命中的规则类型，以及 Creator 短前缀。",
    edit: "编辑",
    saveRule: "保存规则",
    deleteRule: "删除",
    words: "词表（一行一个）",
    builtinHint: "内置 IP 规则没有正则，它会解析每一个 IPv4/IPv6。仍可以关掉。",
  },
} as const;

type Dict = {
  [K in keyof (typeof STR)["en"]]: (typeof STR)["en"][K] extends string ? string : (typeof STR)["en"][K];
};

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
  const value = useMemo(() => ({ lang, setLang, t: STR[lang] as Dict }), [lang]);
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
