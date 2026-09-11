import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));

export function pinnedVersion(pluginJson) {
  const doc = pluginJson ?? JSON.parse(readFileSync(join(here, "../plugin.json"), "utf8"));
  return doc.veil?.version ?? doc.dgw?.version ?? doc.version;
}

export function downloadUrl(version, platform, arch, repo) {
  const os =
    platform === "win32" ? "windows" : platform === "darwin" ? "macos" : "linux";
  const cpu = arch === "arm64" ? "arm64" : "x64";
  const ext = os === "windows" ? ".exe" : "";
  const url = `https://github.com/${repo}/releases/download/v${version}/veil-${os}-${cpu}${ext}`;
  const host = new URL(url).host;
  if (host !== "github.com") throw new Error("refusing non-github host");
  return url;
}

export function verifySha256(fileBytes, sumsText, assetName) {
  const hex = createHash("sha256").update(fileBytes).digest("hex");
  for (const line of String(sumsText).split(/\r?\n/)) {
    const m = line.trim().match(/^([a-fA-F0-9]{64})\s+\*?(.+)$/);
    if (!m) continue;
    if (m[2] === assetName) return m[1].toLowerCase() === hex;
  }
  return false;
}

export function resolveBinary({ env = {}, dataDir, pathExists, pathList } = {}) {
  const hinted = env.VEIL_BIN || env.DGW_BIN;
  if (hinted && pathExists?.(hinted)) return hinted;
  if (hinted) return hinted;
  const names = process.platform === "win32" ? ["veil.exe", "dgw.exe"] : ["veil", "dgw"];
  for (const exe of names) {
    const cached = join(dataDir ?? ".", "bin", exe);
    if (pathExists?.(cached)) return cached;
  }
  const pathEnv = env.PATH ?? env.Path ?? "";
  const parts = pathList ?? pathEnv.split(process.platform === "win32" ? ";" : ":");
  for (const dir of parts) {
    for (const exe of names) {
      const p = join(dir, exe);
      if (pathExists?.(p)) return p;
    }
  }
  return null;
}

export function chainBaseUrl({ current, proxyUrl }) {
  if (!current || current === proxyUrl) {
    return { client: proxyUrl, saveUpstream: null };
  }
  return { client: proxyUrl, saveUpstream: current };
}

export function restoreBaseUrl({ saved, proxyUrl, current }) {
  if (current === proxyUrl) return saved ?? "";
  return current;
}

export async function ensureBinary({ env = {}, dataDir, pathExists, fetchFn } = {}) {
  const found = resolveBinary({ env, dataDir, pathExists });
  if (found) return { ok: true, path: found };
  if (env.VEIL_NO_DOWNLOAD === "1" || env.DGW_NO_DOWNLOAD === "1") return { ok: false, reason: "no_binary" };
  if (!fetchFn) return { ok: false, reason: "no_binary" };
  return { ok: false, reason: "no_binary" };
}

async function main(argv = process.argv.slice(2)) {
  try {
    const cmd = argv[0] ?? "status";
    if (cmd === "start") {
      const r = await ensureBinary({
        env: process.env,
        dataDir: process.env.VEIL_DATA_DIR || process.env.DGW_DATA_DIR,
      });
      if (!r.ok) {
        console.error("veil: not running, traffic is not desensitized");
        process.exitCode = 0;
        return;
      }
    }
    console.log(cmd);
  } catch (err) {
    console.error("veil: not running, traffic is not desensitized");
    console.error(String(err));
    process.exitCode = 0;
  }
}

if (process.argv[1]?.endsWith("veil.mjs")) {
  main();
}
