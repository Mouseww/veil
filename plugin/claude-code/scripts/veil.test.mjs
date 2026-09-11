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
} from "./veil.mjs";

test("resolveBinary prefers VEIL_BIN then cache then PATH", () => {
  assert.equal(
    resolveBinary({ env: { VEIL_BIN: "/opt/custom/veil" }, dataDir: "/data", pathExists: (p) => p === "/opt/custom/veil" }),
    "/opt/custom/veil"
  );
  const found = resolveBinary({
    env: { PATH: "/usr/bin" },
    dataDir: "/data",
    pathExists: (p) => p.replaceAll("\\", "/").includes("/data/bin/veil"),
  });
  assert.ok(String(found).replaceAll("\\", "/").includes("/data/bin/veil"));
});

test("pinnedVersion reads plugin.json veil.version not latest", () => {
  assert.equal(pinnedVersion({ version: "9.9.9", veil: { version: "0.1.0" } }), "0.1.0");
  assert.notEqual(pinnedVersion({ veil: { version: "0.1.0" } }), "latest");
});

test("downloadUrl is github tag asset and rejects other hosts", () => {
  const url = downloadUrl("0.1.0", "linux", "x64", "acme/veil");
  assert.equal(url, "https://github.com/acme/veil/releases/download/v0.1.0/veil-linux-x64");
  assert.equal(new URL(url).host, "github.com");
  const win = downloadUrl("0.1.0", "win32", "x64", "acme/veil");
  assert.ok(win.endsWith(".exe"));
});

test("verifySha256 matches named asset", () => {
  const bytes = Buffer.from("hello");
  const hex = createHash("sha256").update(bytes).digest("hex");
  const sums = hex + "  veil-linux-x64\nother  skip";
  assert.equal(verifySha256(bytes, sums, "veil-linux-x64"), true);
  assert.equal(verifySha256(bytes, hex + "  other", "veil-linux-x64"), false);
  assert.equal(verifySha256(Buffer.from("nope"), sums, "veil-linux-x64"), false);
});

test("chainBaseUrl saves previous custom URL", () => {
  const proxy = "http://127.0.0.1:18787";
  assert.deepEqual(chainBaseUrl({ current: "", proxyUrl: proxy }), { client: proxy, saveUpstream: null });
  assert.deepEqual(chainBaseUrl({ current: proxy, proxyUrl: proxy }), { client: proxy, saveUpstream: null });
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

test("VEIL_NO_DOWNLOAD missing binary does not fetch", async () => {
  let fetched = 0;
  const r = await ensureBinary({
    env: { VEIL_NO_DOWNLOAD: "1" },
    dataDir: "/tmp/none",
    pathExists: () => false,
    fetchFn: async () => { fetched += 1; return new Response("no"); },
  });
  assert.equal(r.ok, false);
  assert.equal(r.reason, "no_binary");
  assert.equal(fetched, 0);
});
