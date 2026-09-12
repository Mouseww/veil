export type RuleView = {
  id: string;
  type_prefix: string;
  enabled: boolean;
  priority: number;
  kind: string;
  source: string;
  pattern?: string | null;
  words?: string[] | null;
};

export type TrafficEvent = {
  ts: number;
  method: string;
  path_template: string;
  status: number;
  streaming: boolean;
  hit_types: string[];
  hit_counts: number[];
  latency_ms: number;
  error_class: string | null;
  creator_prefix8: string;
};

export type ClientRoute = {
  id: string;
  label: string;
  kind: string;
  port: number;
  upstream: string;
};

export type StatusBody = {
  product: string;
  bind: string;
  proxy_port: number;
  management_port: number;
  anthropic_upstream: string;
  openai_completions_upstream: string;
  openai_responses_upstream: string;
  mapping_ttl_days: number;
  request_body_limit_mib: number;
  master_key_set: boolean;
  rule_count: number;
  last_error_class: string | null;
  version?: string;
};

let adminToken = sessionStorage.getItem("veil_admin") ?? "";

export function setAdminToken(token: string) {
  adminToken = token;
  if (token) sessionStorage.setItem("veil_admin", token);
  else sessionStorage.removeItem("veil_admin");
}

async function req<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  if (adminToken) headers.set("x-veil-admin-token", adminToken);
  if (init.body && !headers.has("content-type")) headers.set("content-type", "application/json");
  const res = await fetch(path, { ...init, headers });
  if (!res.ok) throw new Error(String(res.status));
  if (res.status === 204) return undefined as T;
  const ct = res.headers.get("content-type") ?? "";
  if (ct.includes("json")) return res.json() as Promise<T>;
  return res.text() as unknown as T;
}

export const api = {
  status: () => req<StatusBody>("/api/status"),
  rules: () => req<RuleView[]>("/api/rules"),
  putRules: (rules: RuleView[]) => req<void>("/api/rules", { method: "PUT", body: JSON.stringify(rules) }),
  allowlist: () => req<string[]>("/api/allowlist"),
  putAllowlist: (list: string[]) => req<void>("/api/allowlist", { method: "PUT", body: JSON.stringify(list) }),
  upstream: () => req<{ anthropic_upstream: string; openai_completions_upstream: string; openai_responses_upstream: string; routes?: ClientRoute[] }>("/api/upstream"),
  putUpstream: (u: { anthropic_upstream: string; openai_completions_upstream: string; openai_responses_upstream: string; routes?: ClientRoute[] }) =>
    req<void>("/api/upstream", { method: "PUT", body: JSON.stringify(u) }),
  putSettings: (s: { mapping_ttl_days?: number; request_body_limit_mib?: number }) =>
    req<void>("/api/settings", { method: "PUT", body: JSON.stringify(s) }),
  rotateKey: (new_key: string) => req<{ rotated: boolean }>("/api/master-key", { method: "POST", body: JSON.stringify({ new_key }) }),
  purge: (creator_prefix?: string) => req<{ purged: boolean }>("/api/mappings/purge", { method: "POST", body: JSON.stringify({ creator_prefix }) }),
  resetToken: () => req<{ token: string }>("/api/admin-token/reset", { method: "POST" }),
  checkUpdate: () => req<{ current: string; latest: string; newer: boolean; asset: string; url: string }>("/api/update"),
  applyUpdate: () => req<{ restarting?: boolean; latest?: string; current?: string; newer?: boolean }>("/api/update", { method: "POST" }),
  traffic: async () => {
    const text = await req<string>("/api/traffic");
    const line = String(text).split("\n").find((l) => l.startsWith("data: "));
    if (!line) return [] as TrafficEvent[];
    try { return JSON.parse(line.slice(6)) as TrafficEvent[]; } catch { return []; }
  },
};
