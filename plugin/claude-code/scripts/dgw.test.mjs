import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";
import {
  chainBaseUrl,
  downloadUrl,
  ensureBinary,
  pinnedVersion,
  resolveBinary,
  restoreBaseUrl,
  verifySha256,
} from "./dgw.mjs";

test("resolveBinary prefers DGW_BIN then cache then PATH", () => {
  const hits = new Set();
  const env = { DGW_BIN: "/opt/custom/dgw", PATH: "/usr/bin" };
  assert.equal(
    resolveBinary({ env, dataDir: "/data", pathExists: (p) => p === "/opt/custom/dgw" }),
    "/opt/custom/dgw"
  );
  assert.equal(
    resolveBinary({
      env: { PATH: "/usr/bin" },
      dataDir: "/data",
      pathExists: (p) => p.replaceAll("\\", "/").endsWith("/data/bin/dgw") || p.replaceAll("\\", "/").endsWith("/data/bin/dgw.exe"),
    }).replaceAll("\\", "/").includes("/data/bin/dgw"),
    true
  );
});

test("pinnedVersion reads plugin.json dgw.version not latest", () => {
  assert.equal(pinnedVersion({ version: "9.9.9", dgw: { version: "0.1.0" } }), "0.1.0");
  assert.notEqual(pinnedVersion({ dgw: { version: "0.1.0" } }), "latest");
});

test("downloadUrl is github tag asset and rejects other hosts", () => {
  const url = downloadUrl("0.1.0", "linux", "x64", "acme/DesensitizationGateway");
  assert.equal(
    url,
    "https://github.com/acme/DesensitizationGateway/releases/download/v0.1.0/dgw-linux-x64"
  );
  assert.equal(new URL(url).host, "github.com");
  const win = downloadUrl("0.1.0", "win32", "x64", "acme/DesensitizationGateway");
  assert.ok(win.endsWith(".exe"));
});

test("verifySha256 matches named asset", () => {
  const bytes = Buffer.from("hello");
  const hex = createHash("sha256").update(bytes).digest("hex");
  const sums = `${hex}  dgw-linux-x64\nother  skip`;
  assert.equal(verifySha256(bytes, sums, "dgw-linux-x64"), true);
  assert.equal(verifySha256(bytes, `${hex}  other`, "dgw-linux-x64"), false);
  assert.equal(verifySha256(Buffer.from("nope"), sums, "dgw-linux-x64"), false);
});

test("chainBaseUrl saves previous custom URL", () => {
  const proxy = "http://127.0.0.1:18787";
  assert.deepEqual(chainBaseUrl({ current: "", proxyUrl: proxy }), {
    client: proxy,
    saveUpstream: null,
  });
  assert.deepEqual(chainBaseUrl({ current: proxy, proxyUrl: proxy }), {
    client: proxy,
    saveUpstream: null,
  });
  assert.deepEqual(chainBaseUrl({ current: "https://litellm.example", proxyUrl: proxy }), {
    client: proxy,
    saveUpstream: "https://litellm.example",
  });
});

test("restoreBaseUrl puts back saved when still on proxy", () => {
  const proxy = "http://127.0.0.1:18787";
  assert.equal(restoreBaseUrl({ saved: "https://x", proxyUrl: proxy, current: proxy }), "https://x");
  assert.equal(restoreBaseUrl({ saved: "https://x", proxyUrl: proxy, current: "https://other" }), "https://other");
});

test("DGW_NO_DOWNLOAD missing binary does not fetch", async () => {
  let fetched = 0;
  const r = await ensureBinary({
    env: { DGW_NO_DOWNLOAD: "1" },
    dataDir: "/tmp/none",
    pathExists: () => false,
    fetchFn: async () => {
      fetched += 1;
      return new Response("no");
    },
  });
  assert.equal(r.ok, false);
  assert.equal(r.reason, "no_binary");
  assert.equal(fetched, 0);
});
