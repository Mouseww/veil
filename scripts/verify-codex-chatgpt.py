#!/usr/bin/env python3
"""Send one synthetic request using Codex's existing ChatGPT login (no token access)."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time
import urllib.request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--proxy", default="http://127.0.0.1:18787")
    parser.add_argument("--management", default="http://127.0.0.1:18788")
    parser.add_argument("--model", help="Defaults to the model in your Codex config")
    parser.add_argument("--profile", help="Use an existing Codex profile instead of temporary provider overrides")
    parser.add_argument("--request-path", default="/codex/responses", help="Expected traffic path (use /v1/responses for API gateways)")
    parser.add_argument("--output", default="target/codex-verification")
    args = parser.parse_args()
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    config = Path(os.environ.get("CODEX_HOME", str(Path.home() / ".codex"))) / "config.toml"
    original_hash = hashlib.sha256(config.read_bytes()).hexdigest() if config.exists() else None
    model = args.model
    if not model and config.exists():
        # Only read the top-level model; never read auth.json or extract credentials.
        for line in config.read_text().splitlines():
            if line.lstrip().startswith("["):
                break
            match = re.match(r'''^\s*model\s*=\s*["']([^"']+)["']''', line)
            if match:
                model = match.group(1)
    expected = "VEIL_CHECK 13800138000 END"
    prompt = ("Do not use tools or read files. Reply with exactly the following text, "
              "preserving all characters and any placeholders including braces. "
              "No explanation or code fences:\n" + expected)
    started = int(time.time() * 1000)
    with tempfile.TemporaryDirectory(prefix="veil-codex-check-") as work:
        cmd = ["codex", "exec", "--ephemeral", "--skip-git-repo-check",
               "--sandbox", "read-only", "-C", work, "--json"]
        if args.profile:
            cmd += ["--profile", args.profile]
        else:
            cmd += ["--ignore-user-config",
               "-c", 'model_provider="veil_chatgpt"',
               "-c", 'model_providers.veil_chatgpt.name="Veil ChatGPT"',
               "-c", "model_providers.veil_chatgpt.base_url=" + json.dumps(args.proxy.rstrip("/") + "/codex"),
               "-c", 'model_providers.veil_chatgpt.wire_api="responses"',
               "-c", "model_providers.veil_chatgpt.requires_openai_auth=true",
               "-c", "model_providers.veil_chatgpt.supports_websockets=false"]
        cmd += ["-o", str(output / "reply.txt")]
        if model:
            cmd += ["-m", model]
        cmd += ["-"]
        # Clear the previous output so a failed run cannot reuse stale evidence.
        (output / "reply.txt").write_text("")
        with (output / "events.jsonl").open("w") as out, (output / "stderr.log").open("w") as err:
            try:
                result = subprocess.run(cmd, input=prompt, text=True, stdout=out, stderr=err, timeout=120)
            except subprocess.TimeoutExpired:
                raise SystemExit("Codex timed out; see " + str(output / "stderr.log"))
    events = []
    for line in (output / "events.jsonl").read_text().splitlines():
        try:
            events.append(json.loads(line))
        except ValueError:
            pass
    reply = (output / "reply.txt").read_text().strip()
    # Loopback read-only traffic snapshots require no admin token by default.
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with opener.open(args.management.rstrip("/") + "/api/traffic", timeout=5) as response:
        snapshot = response.read().decode()
    traffic = json.loads(next(line[6:] for line in snapshot.splitlines() if line.startswith("data: ")))
    matching = [e for e in traffic if e["ts"] >= started and e["path_template"] == args.request_path]
    checks = {
        "codex_exit_ok": result.returncode == 0,
        "turn_completed": any(e.get("type") == "turn.completed" for e in events),
        "request_through_veil": bool(matching),
        "phone_rule_hit": any("PHONE" in e["hit_types"] for e in matching),
        "streaming_2xx": any(e["streaming"] and 200 <= e["status"] < 300 for e in matching),
        "reply_restored_exactly": reply == expected,
        "base_config_unchanged": (hashlib.sha256(config.read_bytes()).hexdigest() if config.exists() else None) == original_hash,
    }
    evidence = {"checks": checks, "traffic": matching, "reply": reply}
    (output / "verification.json").write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + "\n")
    for name, passed in checks.items():
        print(("PASS " if passed else "FAIL ") + name)
    print("Evidence: " + str(output / "verification.json"))
    raise SystemExit(0 if all(checks.values()) else 1)


if __name__ == "__main__":
    main()
